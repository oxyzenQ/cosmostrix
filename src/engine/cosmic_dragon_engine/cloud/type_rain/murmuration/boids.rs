// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The bird and the neighbor window — the physics half of the
//! murmuration style (NIGHT-research-7).
//!
//! This module owns the Reynolds triad (law 1), the spatial hash
//! (law 2), the anchor and wall steering (law 3), and the startle
//! impulse (law 5) in executable form. The full derivation essay
//! lives in `type_rain/murmuration/mod.rs`; the flock state and
//! the per-frame orchestration live in `murmuration.rs`.
//!
//! The hash: a flat bucket store (`Vec<Vec<u16>>`) sized to the
//! viewport's bucket grid, rebuilt each advance (clear + insert —
//! the allocations persist, only the contents churn). Each bird
//! scans its 3x3 bucket neighborhood: O(n) pair checks per frame
//! instead of O(n^2) — the reason a 200-bird flock costs the same
//! as a 30-bird naive scan.

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::constants::{
    MURM_ALIGN_W, MURM_ANCHOR_W, MURM_COH_W, MURM_NEIGHBOR_R, MURM_PANIC_IMPULSE, MURM_PANIC_R,
    MURM_SEP_R, MURM_SEP_W, MURM_SPEED_MAX, MURM_SPEED_MIN, MURM_TRAIL_LEN, MURM_WALL_MARGIN,
    MURM_WALL_W,
};

/// RNG bundle (the advance pass is a stochastic pass in the family
/// sense: the jitter walk, the anchor target re-rolls and the
/// startle clock all ride along).
pub(crate) struct BirdRandom<'a> {
    pub(crate) rng: &'a mut StdRng,
    pub(crate) rand_chance: &'a Uniform<f32>,
}

/// One bird: a glyph carrier with a position and a heading — no
/// leader, no global state, nothing else (the Reynolds premise:
/// flocking is local).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Bird {
    pub(crate) active: bool,
    /// Column position, fractional.
    pub(crate) x: f32,
    /// Line position, fractional (y grows downward).
    pub(crate) y: f32,
    /// Velocity in cells per sim-second.
    pub(crate) vx: f32,
    pub(crate) vy: f32,
    /// Glyph carried by the bird; re-rolled matrix-style when the
    /// head crosses into a new cell (the family shimmer).
    pub(crate) ch: char,
    /// Palette slot adopted at spawn / palette transition.
    pub(crate) palette_slot: u8,
    /// Sim-seconds since activation (the entry stagger's read).
    pub(crate) age: f32,
    /// Sim-seconds since the last startle hit (f32::MAX when calm
    /// — the panic speed floor's window read).
    pub(crate) panic_age: f32,
    /// Ring buffer of the last MURM_TRAIL_LEN head cells,
    /// shift-left layout (index 0 = oldest).
    trail: [(u16, u16); MURM_TRAIL_LEN],
    trail_len: u8,
}

impl Bird {
    pub(crate) const fn vacant() -> Self {
        Self {
            active: false,
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
            ch: '0',
            palette_slot: 0,
            age: 0.0,
            panic_age: f32::MAX,
            trail: [(0, 0); MURM_TRAIL_LEN],
            trail_len: 0,
        }
    }

    /// Activate at an edge position flying inward (the staggered
    /// entry: the flock assembles as birds arrive). The heading
    /// banded around the inward normal — a bird from the top edge
    /// flies down-ish, from the left edge right-ish.
    pub(crate) fn activate_at_edge(&mut self, edge: Edge, along: f32, palette_slot: u8, roll: f32) {
        let speed = MURM_SPEED_MIN + roll * (MURM_SPEED_MAX - MURM_SPEED_MIN) * 0.6;
        let heading = match edge {
            Edge::Top => std::f32::consts::FRAC_PI_2 + (roll - 0.5) * 0.8,
            Edge::Bottom => -std::f32::consts::FRAC_PI_2 + (roll - 0.5) * 0.8,
            Edge::Left => (roll - 0.5) * 0.8,
            Edge::Right => std::f32::consts::PI + (roll - 0.5) * 0.8,
        };
        let (sin_h, cos_h) = heading.sin_cos();
        self.active = true;
        self.x = match edge {
            Edge::Left => 1.0,
            Edge::Right => -1.0,
            _ => along,
        };
        self.y = match edge {
            Edge::Top => 1.0,
            Edge::Bottom => -1.0,
            _ => along,
        };
        self.vx = cos_h * speed;
        self.vy = sin_h * speed;
        self.palette_slot = palette_slot;
        self.age = 0.0;
        self.panic_age = f32::MAX;
        self.trail_len = 0;
    }

    /// Speed magnitude (the kinetic ladder's input).
    pub(crate) fn speed(&self) -> f32 {
        (self.vx * self.vx + self.vy * self.vy).sqrt()
    }

    /// The panic window: a startled bird flies floored near max
    /// while its panic age is fresh.
    pub(crate) fn panicked(&self) -> bool {
        self.panic_age < crate::constants::MURM_PANIC_FLOOR_SECS
    }

    /// Push a head cell into the trail ring buffer (shift-left
    /// when full — the family comet mechanics).
    pub(crate) fn push_trail(&mut self, col: u16, line: u16) {
        if self.trail_len as usize >= MURM_TRAIL_LEN {
            for i in 0..MURM_TRAIL_LEN - 1 {
                self.trail[i] = self.trail[i + 1];
            }
            self.trail[MURM_TRAIL_LEN - 1] = (col, line);
        } else {
            let idx = self.trail_len as usize;
            self.trail[idx] = (col, line);
            self.trail_len += 1;
        }
    }

    /// Trail cell read by age (0 = oldest kept, len-1 = newest).
    pub(crate) fn trail_cell(&self, idx: usize) -> Option<(u16, u16)> {
        if idx < self.trail_len as usize {
            Some(self.trail[idx])
        } else {
            None
        }
    }

    pub(crate) fn trail_len(&self) -> u8 {
        self.trail_len
    }
}

/// The spawn edge (the staggered entry's origin).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

/// The flock-context bundle (the force pass's read-only world:
/// the bird list, the thought, the breathing value, the viewport).
/// Bundled so the force signature stays under clippy's 7-arg
/// threshold (the family's params-struct pattern).
pub(crate) struct FlockCtx<'a> {
    pub(crate) birds: &'a [Bird],
    pub(crate) anchor: &'a Anchor,
    /// The breathing oscillator's current cohesion multiplier
    /// (law 4 — the tighten/loosen cycle).
    pub(crate) coh_weight: f32,
    pub(crate) cols: u16,
    pub(crate) lines: u16,
}

/// The spatial hash — the neighbor window (law 2). One bucket per
/// NEIGHBOR_R x NEIGHBOR_R cell block; a bird's 3x3 bucket scan is
/// its neighbor set (the topological window sized by the shipped
/// population: the average bird scans the starling number).
#[derive(Debug, Default)]
pub(crate) struct MurmHash {
    /// Bucket store: bucket index -> bird indices. Rebuilt each
    /// advance (clear + insert; the allocation persists).
    buckets: Vec<Vec<u16>>,
    bucket_cols: usize,
}

impl MurmHash {
    /// Rebuild the bucket store for the current active set.
    pub(crate) fn rebuild(&mut self, birds: &[Bird], cols: u16, lines: u16) {
        self.bucket_cols = ((cols.max(1) as f32 / MURM_NEIGHBOR_R).ceil().max(1.0)) as usize;
        let bucket_rows = ((lines.max(1) as f32 / MURM_NEIGHBOR_R).ceil().max(1.0)) as usize;
        let need = self.bucket_cols * bucket_rows;
        if self.buckets.len() != need {
            self.buckets.clear();
            self.buckets.resize_with(need, Vec::new);
        } else {
            for b in &mut self.buckets {
                b.clear();
            }
        }
        let rows = bucket_rows;
        for (i, b) in birds.iter().enumerate() {
            if !b.active {
                continue;
            }
            let bx = ((b.x.max(0.0) / MURM_NEIGHBOR_R).floor() as usize).min(self.bucket_cols - 1);
            let by = ((b.y.max(0.0) / MURM_NEIGHBOR_R).floor() as usize).min(rows - 1);
            self.buckets[by * self.bucket_cols + bx].push(i as u16);
        }
    }

    /// The 3x3 bucket scan around a position: the neighbor indices
    /// (the caller's own index included — the force pass filters
    /// by identity and the separation radius).
    pub(crate) fn neighbors(&self, x: f32, y: f32, out: &mut Vec<u16>) {
        out.clear();
        let bc = self.bucket_cols as isize;
        let rows = (self.buckets.len() / self.bucket_cols.max(1)) as isize;
        let bx = (((x.max(0.0) / MURM_NEIGHBOR_R).floor() as isize).min(bc - 1)).max(0);
        let by = (((y.max(0.0) / MURM_NEIGHBOR_R).floor() as isize).min(rows - 1)).max(0);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let nx = bx + dx;
                let ny = by + dy;
                if nx < 0 || ny < 0 || nx >= bc || ny >= rows {
                    continue;
                }
                let idx = ny as usize * self.bucket_cols + nx as usize;
                if idx < self.buckets.len() {
                    out.extend_from_slice(&self.buckets[idx]);
                }
            }
        }
    }
}

/// The anchor — the flock's thought (law 3): a point that
/// random-walks across the sky. The drift clamps at the roam
/// speed; the target re-rolls on the hold; the position reflects
/// off the walls with a damped bounce.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Anchor {
    pub(crate) x: f32,
    pub(crate) y: f32,
    vx: f32,
    vy: f32,
    /// Sim-seconds until the next target re-roll.
    hold: f32,
}

impl Anchor {
    pub(crate) fn new() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
            hold: 0.0,
        }
    }

    /// Seed at the sky's center (the reset contract: the thought
    /// starts where the flock assembles).
    pub(crate) fn reset(&mut self, cols: u16, lines: u16) {
        self.x = (cols.max(1) as f32 - 1.0) * 0.5;
        self.y = (lines.max(1) as f32 - 1.0) * 0.4;
        self.vx = 0.0;
        self.vy = 0.0;
        self.hold = 0.0;
    }

    /// Advance the thought one sim-tick.
    pub(crate) fn advance(&mut self, dt: f32, cols: u16, lines: u16, random: &mut BirdRandom<'_>) {
        self.hold -= dt;
        if self.hold <= 0.0 {
            let roll_a = random.rand_chance.sample(random.rng);
            let roll_b = random.rand_chance.sample(random.rng);
            let heading = roll_a * std::f32::consts::TAU;
            let speed = crate::constants::MURM_ANCHOR_SPEED * (0.5 + roll_b * 0.8);
            self.vx = heading.cos() * speed;
            self.vy = heading.sin() * speed;
            self.hold = crate::constants::MURM_ANCHOR_HOLD
                * (0.6 + random.rand_chance.sample(random.rng) * 0.8);
        }
        self.x += self.vx * dt;
        self.y += self.vy * dt;
        let w = (cols.max(1) as f32 - 1.0).max(1.0);
        let h = (lines.max(1) as f32 - 1.0).max(1.0);
        if self.x < 0.0 {
            self.x = 0.0;
            self.vx = -self.vx;
        } else if self.x > w {
            self.x = w;
            self.vx = -self.vx;
        }
        if self.y < 0.0 {
            self.y = 0.0;
            self.vy = -self.vy;
        } else if self.y > h {
            self.y = h;
            self.vy = -self.vy;
        }
    }
}

/// The wall-banking steering for a bird near the margins (law 3's
/// banking: a soft inward force inside the margin — birds curve
/// along the edge, never hit it). Shared by the force pass.
fn wall_steer(x: f32, y: f32, cols: u16, lines: u16) -> (f32, f32) {
    let w = cols.max(1) as f32 - 1.0;
    let h = lines.max(1) as f32 - 1.0;
    let m = MURM_WALL_MARGIN;
    let mut ax = 0.0;
    let mut ay = 0.0;
    if x < m {
        ax += (m - x) / m * MURM_WALL_W;
    } else if x > w - m {
        ax -= (x - (w - m)) / m * MURM_WALL_W;
    }
    if y < m {
        ay += (m - y) / m * MURM_WALL_W;
    } else if y > h - m {
        ay -= (y - (h - m)) / m * MURM_WALL_W;
    }
    (ax, ay)
}

/// Law 1's force accumulation for one bird (the Reynolds triad +
/// the anchor + the banking — pure function of the state, no dt:
/// the caller integrates). Returns the acceleration (ax, ay) in
/// cells per sim-second squared. The jitter is the caller's (it
/// holds the RNG).
pub(crate) fn flock_forces(
    bird: &Bird,
    neighbors: &[u16],
    self_idx: u16,
    ctx: &FlockCtx<'_>,
) -> (f32, f32) {
    // The triad accumulators (Reynolds 1987).
    let mut sep_x = 0.0;
    let mut sep_y = 0.0;
    let mut ali_vx = 0.0;
    let mut ali_vy = 0.0;
    let mut coh_x = 0.0;
    let mut coh_y = 0.0;
    let mut count = 0usize;

    for &ni in neighbors {
        if ni == self_idx {
            continue;
        }
        let other = &ctx.birds[ni as usize];
        if !other.active {
            continue;
        }
        let dx = other.x - bird.x;
        let dy = other.y - bird.y;
        let d2 = dx * dx + dy * dy;
        if d2 > MURM_NEIGHBOR_R * MURM_NEIGHBOR_R || d2 < 1e-9 {
            continue;
        }
        count += 1;
        ali_vx += other.vx;
        ali_vy += other.vy;
        coh_x += other.x;
        coh_y += other.y;
        let d = d2.sqrt();
        if d < MURM_SEP_R {
            // Separation: inverse-distance weighted push.
            let w = (MURM_SEP_R - d) / MURM_SEP_R;
            sep_x -= dx / d * w;
            sep_y -= dy / d * w;
        }
    }

    let mut ax = 0.0;
    let mut ay = 0.0;
    if count > 0 {
        let inv = 1.0 / count as f32;
        // Alignment: steer toward the mean heading.
        ax += (ali_vx * inv - bird.vx) * MURM_ALIGN_W;
        ay += (ali_vy * inv - bird.vy) * MURM_ALIGN_W;
        // Cohesion: the weak spring toward the local centroid,
        // its weight the breathing oscillator's value (law 4).
        ax += (coh_x * inv - bird.x) * MURM_COH_W * ctx.coh_weight;
        ay += (coh_y * inv - bird.y) * MURM_COH_W * ctx.coh_weight;
    }
    // Separation (the strongest local rule — birds never overlap).
    ax += sep_x * MURM_SEP_W;
    ay += sep_y * MURM_SEP_W;

    // The thought: the weak anchor attraction (law 3).
    ax += (ctx.anchor.x - bird.x) * MURM_ANCHOR_W;
    ay += (ctx.anchor.y - bird.y) * MURM_ANCHOR_W;

    // The banking: soft walls (law 3).
    let (wx, wy) = wall_steer(bird.x, bird.y, ctx.cols, ctx.lines);
    ax += wx;
    ay += wy;

    (ax, ay)
}

/// Integrate one bird over the sim dt (semi-implicit Euler: the
/// acceleration lands on the velocity, the velocity clamps into
/// the flight band, the position integrates, the hard re-project
/// backstop closes). A panicked bird floors near max speed (the
/// scatter reads fast because it IS fast).
pub(crate) fn integrate_bird(bird: &mut Bird, ax: f32, ay: f32, dt: f32, cols: u16, lines: u16) {
    bird.vx += ax * dt;
    bird.vy += ay * dt;
    let min = if bird.panicked() {
        MURM_SPEED_MAX * 0.8
    } else {
        MURM_SPEED_MIN
    };
    clamp_speed(bird, min, MURM_SPEED_MAX);
    bird.x += bird.vx * dt;
    bird.y += bird.vy * dt;
    // The hard re-project backstop (never fires in practice —
    // the banking turns before the margin; belt and braces).
    let w = cols.max(1) as f32 - 1.0;
    let h = lines.max(1) as f32 - 1.0;
    bird.x = bird.x.clamp(0.0, w);
    bird.y = bird.y.clamp(0.0, h);
}

/// Clamp a bird's speed into [min, max], preserving the heading
/// (a starling never hovers, never teleports).
pub(crate) fn clamp_speed(bird: &mut Bird, min: f32, max: f32) {
    let s = bird.speed();
    if s < 1e-6 {
        // Degenerate: re-seed a straight heading at min speed.
        bird.vx = min;
        bird.vy = 0.0;
        return;
    }
    if s < min {
        let k = min / s;
        bird.vx *= k;
        bird.vy *= k;
    } else if s > max {
        let k = max / s;
        bird.vx *= k;
        bird.vy *= k;
    }
}

/// Law 5: the startle — a bird inside the panic radius takes an
/// outward velocity kick scaled by proximity (an impulse, not a
/// force: the scatter is instant; the subsequent clamp saturates
/// it at V_MAX). The predator position is drawn by the draw pass
/// during its flash window.
pub(crate) fn startle(bird: &mut Bird, px: f32, py: f32) {
    let dx = bird.x - px;
    let dy = bird.y - py;
    let d = (dx * dx + dy * dy).sqrt();
    if d >= MURM_PANIC_R || d < 1e-6 {
        return;
    }
    let w = 1.0 - d / MURM_PANIC_R;
    let k = MURM_PANIC_IMPULSE * w / d;
    bird.vx += dx * k;
    bird.vy += dy * k;
    bird.panic_age = 0.0;
}
