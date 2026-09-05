// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Chinese-mythology dragon rain for the cosmic_dragon scene
//! (NIGHT-research-5, the fifth rain style — a serpentine dragon
//! renderer inspired by Chinese mythology, not Western).
//!
//! Motion DNA — 100% distinct from cascade (`cinematic`), pillars
//! (`monolith`), polar-orbit (`vortex`), and strange-attractor
//! (`lorenz`): each dragon is a CHAIN of segments (head + body + tail)
//! following a path-generating head via FABRIK distance constraints
//! (snake kinematics — each segment maintains fixed spacing to the
//! previous, so the body trails the head's path organically). The
//! Chinese dragon's signature serpentine silhouette emerges from
//! this chain dynamic without any procedural body animation.
//!
//! Head motion: a two-state machine producing the "free flight, then
//! circle, then free again" cadence the owner specified:
//!
//! - [`DragonState::Soar`]: smooth random-walk turn rate driven by
//!   layered sine waves (two frequencies, randomized phase per
//!   dragon). Produces organic, non-repeating free flight — the
//!   dragon goes wherever the noise takes it.
//! - [`DragonState::Circle`]: constant-magnitude turn rate
//!   (clockwise or counter-clockwise, randomized per state entry)
//!   producing a perfect circular orbit. Radius emerges naturally
//!   from speed / turn_rate.
//!
//! State transitions are stochastic: SOAR lasts 4-8s, CIRCLE lasts
//! 3-6s, with weighted transitions (after SOAR → 50/50 SOAR/CIRCLE;
//! after CIRCLE → 70% SOAR / 30% CIRCLE — favoring free flight).
//!
//! Wall bounce: when the head hits a viewport edge, velocity reflects
//! and the state snaps to SOAR (escape any circle that would push
//! the dragon back into the wall).
//!
//! Brightness gradient along the body: head = Core (brightest),
//! first third of body = Hot, middle third = Mid, tail third =
//! Ghost. This serpentine fade is the Chinese-dragon body's visible
//! signature — the head leads brightly, the tail fades into mist.
//!
//! Glyphs: matrix-style re-roll on cell change (mutation tied to
//! motion, parity with vortex/lorenz). Each segment carries its own
//! glyph; segments don't share glyph state (independent shimmer).
//!
//! LOC note: the render/level/noise helpers live in
//! `dragon_helpers.rs` since NIGHT-hunter-10 (the main file reached
//! the 800-line hard cap when the pace-linearity fix + heading LTS
//! wrap landed — the split mirrors the monolith family split).
//! Motion, spawn, chain solver and diff cleanup stay here as one
//! algorithm.
//!
//! Cleanup follows the monolith/vortex/lorenz three-pass diff
//! pattern: draw into `current_cells`, tag with the `drawn_gen`
//! generation counter, then clear only previous cells NOT redrawn
//! this frame (phosphor metadata and frame blank). No spine
//! equivalent, so no extra phosphor pass.

use std::time::{Duration, Instant};

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::frame::Frame;

use super::dragon_helpers::{
    dragon_noise_roll, dragon_state_duration, draw_dragon_cell, level_for_segment, pick_pool_char,
};
use super::monolith_helpers::clear_cell;
use super::render::DrawCtx;

/// Heading-accumulator wrap threshold (radians, 64 turns). See the
/// amortized wrap in `advance` — keeps the unbounded f32 accumulator
/// inside a range where the ulp stays far below visual resolution.
const DRAGON_HEADING_WRAP_LIMIT: f32 = 64.0 * std::f32::consts::TAU;

/// One drawn cell (col, line). Own struct instead of reusing
/// monolith's `DrawnCell` because dragon has no Segment/Spine kind
/// distinction (same shape as `VortexCell` / `LorenzCell`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DragonCell {
    pub(crate) col: u16,
    pub(crate) line: u16,
}

/// Head motion state machine — the dragon's behavior mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DragonState {
    /// Free flight: smooth random-walk turn rate from layered sine
    /// noise. Produces organic, non-repeating motion.
    Soar,
    /// Orbital: constant-magnitude turn rate producing a circular
    /// path. Direction (CW/CCW) is randomized per state entry.
    Circle,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DragonSegment {
    /// Continuous position (cell space, fractional for smooth motion).
    pub(crate) x: f32,
    pub(crate) y: f32,
    /// Glyph carried by this segment; re-rolled matrix-style when the
    /// segment crosses into a new cell (matrix-shimmer tied to motion).
    pub(crate) ch: char,
    /// Last cell (col, line) this segment occupied — used to detect
    /// cell crossings for the shimmer gate.
    last_col: i32,
    last_line: i32,
    /// Set true on first frame after activation so the shimmer gate
    /// initializes the glyph without falsely detecting a crossing.
    first_frame: bool,
}

impl DragonSegment {
    const fn vacant() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            ch: '0',
            last_col: -1,
            last_line: -1,
            first_frame: true,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Dragon {
    pub(crate) active: bool,
    /// Chain of segments: [0] = head, [N-1] = tail.
    pub(crate) segments: Vec<DragonSegment>,
    /// Current motion state (Soar or Circle).
    pub(crate) state: DragonState,
    /// Time remaining in the current state (seconds).
    pub(crate) state_timer: f32,
    /// Head heading angle (radians). Body inherits via chain constraint.
    pub(crate) heading: f32,
    /// Per-dragon pace multiplier (0.85..1.15) — scales the dragon's
    /// motion-clock rate (translation, turn rate, state timer and
    /// aging all advance pace-times faster; linear, never squared).
    pub(crate) pace: f32,
    /// Circle direction: +1 = clockwise, -1 = counter-clockwise.
    /// Re-rolled on each CIRCLE state entry.
    pub(crate) circle_dir: i8,
    /// Per-dragon noise phase offset for SOAR layered-sine turn rate.
    pub(crate) noise_phase: f32,
    /// Simulation age (seconds, dt-integrated). Drives absorption.
    pub(crate) sim_age: f32,
    /// Per-dragon lifetime cap (variance per spawn for staggered refresh).
    pub(crate) lifetime: f32,
    /// Palette slot adopted at spawn / palette transition.
    pub(crate) palette_slot: u8,
}

impl Dragon {
    const fn vacant() -> Self {
        Self {
            active: false,
            segments: Vec::new(),
            state: DragonState::Soar,
            state_timer: 0.0,
            heading: 0.0,
            pace: 1.0,
            circle_dir: 1,
            noise_phase: 0.0,
            sim_age: 0.0,
            lifetime: 0.0,
            palette_slot: 0,
        }
    }
}

/// Spawn inputs (mirrors `VortexSpawnParams` / `LorenzSpawnParams`).
pub(crate) struct DragonSpawnParams {
    pub(crate) cols: u16,
    pub(crate) lines: u16,
    pub(crate) density: f32,
    pub(crate) active_palette_slot: u8,
    pub(crate) spawn_scale: f32,
}

/// RNG bundle (mirrors `VortexRandom` / `LorenzRandom`).
pub(crate) struct DragonRandom<'a> {
    pub(crate) rng: &'a mut StdRng,
    pub(crate) rand_chance: &'a Uniform<f32>,
}

/// Per-frame step inputs for the advance pass. Viewport geometry is
/// carried here so the wall-bounce reflection in the advance pass
/// can clamp the head to actual viewport bounds (the draw pass
/// derives the same geometry from DrawCtx, but advance needs it
/// before draw to keep the chain inside the visible region).
pub(crate) struct DragonStep {
    pub(crate) now: Instant,
    /// chars_per_sec already multiplied by the terminal speed_mult.
    /// Drives head translation speed so ↑/↓ speed keys feel native.
    pub(crate) chars_per_sec: f32,
    pub(crate) cols: u16,
    pub(crate) lines: u16,
    pub(crate) max_sim_delta: Duration,
    pub(crate) resume_blend: f32,
}

pub(crate) struct DragonRain {
    pub(crate) dragons: Vec<Dragon>,
    active_count: usize,
    /// Rotating scan cursor for amortized O(1) free-slot search
    /// (mirrors `VortexRain::spawn_scan_idx` / `LorenzRain`).
    spawn_scan_idx: usize,
    /// Global motion clock. dt = now - last_step clamped by
    /// max_sim_delta; a fully-paused run simply stops integrating
    /// (rain_at early-return), so no pause-time shift is required.
    last_step: Option<Instant>,
    current_cells: Vec<DragonCell>,
    previous_cells: Vec<DragonCell>,
    drawn_gen: Vec<u32>,
    drawn_gen_counter: u32,
}

impl DragonRain {
    pub(crate) fn new() -> Self {
        Self {
            dragons: Vec::new(),
            active_count: 0,
            spawn_scan_idx: 0,
            last_step: None,
            current_cells: Vec::new(),
            previous_cells: Vec::new(),
            drawn_gen: Vec::new(),
            drawn_gen_counter: 0,
        }
    }

    /// Rebuild the dragon pool for a new viewport (or style entry).
    /// NIGHT-research-5 owner directive: pool size is fixed at
    /// DRAGON_POOL_MAX (3 dragons) regardless of viewport width —
    /// the three dragons match the three dragon engines in
    /// cosmostrix (cosmic_dragon_engine, crystal_dragon_engine,
    /// chroma_dragon_engine).
    pub(crate) fn reset(&mut self, _cols: u16) {
        let pool_size = crate::constants::DRAGON_POOL_MAX;
        self.dragons.clear();
        self.dragons.resize_with(pool_size, Dragon::vacant);
        self.active_count = 0;
        self.spawn_scan_idx = 0;
        self.last_step = None;
        self.clear_draw_history();
    }

    pub(crate) fn active_count(&self) -> usize {
        self.active_count
    }

    /// Palette transition completion: all active dragons adopt the
    /// new slot (mirrors `VortexRain::adopt_palette_slot`).
    pub(crate) fn adopt_palette_slot(&mut self, palette_slot: u8) {
        for d in &mut self.dragons {
            if d.active {
                d.palette_slot = palette_slot;
            }
        }
    }

    /// Drop the diff-cleanup history (semantic invalidation / forced
    /// redraw). The next draw pass rebuilds it from an empty baseline.
    pub(crate) fn clear_draw_history(&mut self) {
        self.current_cells.clear();
        self.previous_cells.clear();
        self.drawn_gen.clear();
        self.drawn_gen_counter = 0;
    }

    /// Steady-state active-dragon target. NIGHT-research-5 owner
    /// directive: fixed at DRAGON_FIXED_ACTIVE (3) regardless of
    /// pool size or density — the three dragons are a signature,
    /// matching the three dragon engines in cosmostrix. Density
    /// now only influences spawn timing (deficit-bounded), not the
    /// steady-state count.
    fn target_active_count(_pool_size: usize, _density: f32) -> usize {
        crate::constants::DRAGON_FIXED_ACTIVE
    }

    /// Amortized free-slot scan (rotating cursor — mirrors vortex/
    /// lorenz `find_inactive_mote`).
    fn find_inactive_dragon(&mut self) -> Option<usize> {
        let len = self.dragons.len();
        if len == 0 {
            return None;
        }
        for step in 0..len {
            let idx = (self.spawn_scan_idx + step) % len;
            if !self.dragons[idx].active {
                self.spawn_scan_idx = (idx + 1) % len;
                return Some(idx);
            }
        }
        None
    }

    /// Spawn pass — accumulator pattern identical to vortex/lorenz
    /// (deficit-bounded budget + fractional remainder carry).
    pub(crate) fn spawn(
        &mut self,
        elapsed: Duration,
        spawn_remainder: &mut f32,
        params: &DragonSpawnParams,
        random: &mut DragonRandom<'_>,
    ) {
        if params.cols == 0 || params.lines == 0 || self.dragons.is_empty() {
            *spawn_remainder = 0.0;
            return;
        }

        let target = Self::target_active_count(self.dragons.len(), params.density);
        if self.active_count >= target {
            *spawn_remainder = (*spawn_remainder).min(crate::constants::SPAWN_REMAINDER_CAP);
            return;
        }

        let deficit = target - self.active_count;
        let spawn_rate = (target as f32 * crate::constants::DRAGON_SPAWN_RATE_MULT
            + crate::constants::DRAGON_SPAWN_RATE_FLOOR)
            * params.spawn_scale;
        let budget = elapsed.as_secs_f32() * spawn_rate
            + (*spawn_remainder).min(crate::constants::SPAWN_REMAINDER_CAP);
        if !budget.is_finite() || budget <= 0.0 {
            *spawn_remainder = 0.0;
            return;
        }

        let to_spawn = (budget.floor() as usize).min(deficit);
        *spawn_remainder = (budget - to_spawn as f32).min(crate::constants::SPAWN_REMAINDER_CAP);
        if to_spawn == 0 {
            return;
        }

        for _ in 0..to_spawn {
            let Some(idx) = self.find_inactive_dragon() else {
                break;
            };
            self.activate_dragon(
                idx,
                params.cols,
                params.lines,
                params.active_palette_slot,
                random.rand_chance,
                random.rng,
            );
            self.active_count += 1;
        }
    }

    /// Activate a vacant dragon at a random viewport position with a
    /// random heading. Body segments are stretched behind the head
    /// along the opposite heading direction (so the dragon spawns
    /// already in a clean serpentine line, not a clumped point).
    fn activate_dragon(
        &mut self,
        idx: usize,
        cols: u16,
        lines: u16,
        palette_slot: u8,
        rand_chance: &Uniform<f32>,
        rng: &mut StdRng,
    ) {
        let cols_f = cols as f32;
        let lines_f = lines as f32;
        // Spawn within the inner 60% of the viewport so the dragon
        // has room to fly before hitting a wall.
        let hx = cols_f * 0.2 + rand_chance.sample(rng) * cols_f * 0.6;
        let hy = lines_f * 0.2 + rand_chance.sample(rng) * lines_f * 0.6;
        let heading = rand_chance.sample(rng) * std::f32::consts::TAU;
        let pace = 0.85 + rand_chance.sample(rng) * 0.30;
        let circle_dir: i8 = if rand_chance.sample(rng) < 0.5 { 1 } else { -1 };
        let noise_phase = rand_chance.sample(rng) * std::f32::consts::TAU;
        let lifetime =
            crate::constants::DRAGON_LIFETIME_SECS * (0.85 + rand_chance.sample(rng) * 0.30);

        // Initial state: 70% SOAR / 30% CIRCLE — favor free flight
        // on entry so the dragon doesn't immediately pin to a circle.
        let state = if rand_chance.sample(rng) < 0.7 {
            DragonState::Soar
        } else {
            DragonState::Circle
        };
        let state_timer = match state {
            DragonState::Soar => dragon_state_duration(
                crate::constants::DRAGON_SOAR_MIN_DURATION,
                crate::constants::DRAGON_SOAR_MAX_DURATION,
                rand_chance,
                rng,
            ),
            DragonState::Circle => dragon_state_duration(
                crate::constants::DRAGON_CIRCLE_MIN_DURATION,
                crate::constants::DRAGON_CIRCLE_MAX_DURATION,
                rand_chance,
                rng,
            ),
        };

        // Allocate body segments if not already sized.
        let d = &mut self.dragons[idx];
        if d.segments.len() != crate::constants::DRAGON_BODY_LEN {
            d.segments.clear();
            d.segments
                .resize_with(crate::constants::DRAGON_BODY_LEN, DragonSegment::vacant);
        }
        // Stretch body behind the head along the opposite heading —
        // clean serpentine spawn, not a clumped point.
        let spacing = crate::constants::DRAGON_SEGMENT_SPACING;
        let back_x = -heading.cos();
        let back_y = -heading.sin();
        for (i, seg) in d.segments.iter_mut().enumerate() {
            seg.x = hx + back_x * spacing * (i as f32);
            seg.y = hy + back_y * spacing * (i as f32);
            seg.ch = '0';
            seg.last_col = -1;
            seg.last_line = -1;
            seg.first_frame = true;
        }
        d.active = true;
        d.state = state;
        d.state_timer = state_timer;
        d.heading = heading;
        d.pace = pace;
        d.circle_dir = circle_dir;
        d.noise_phase = noise_phase;
        d.sim_age = 0.0;
        d.lifetime = lifetime;
        d.palette_slot = palette_slot;
    }

    /// Motion pass — the dragon state machine + body chain core.
    ///
    /// Head: state-dependent turn rate drives heading update; speed
    /// drives translation. Wall bounce reflects velocity and snaps
    /// to SOAR (escape any pinning circle). State timer counts down;
    /// on expiry a new state is stochastically selected.
    ///
    /// Body: each segment maintains fixed spacing to the previous
    /// (FABRIK distance constraint, snake kinematics). The chain
    /// follows the head's path organically — the dragon's
    /// serpentine silhouette is the visible signature of this
    /// constraint solver, not procedural body animation.
    pub(crate) fn advance(&mut self, step: &DragonStep) {
        if self.active_count == 0 {
            self.last_step = Some(step.now);
            return;
        }
        let dt = match self.last_step {
            Some(last) => {
                step.now
                    .saturating_duration_since(last)
                    .as_secs_f32()
                    .min(step.max_sim_delta.as_secs_f32())
                    .max(0.0)
                    * step.resume_blend.clamp(0.0, 1.0)
            }
            None => 0.0,
        };
        self.last_step = Some(step.now);
        if dt <= 0.0 {
            return;
        }

        // Speed in cells/sec — drives head translation. The body
        // inherits the head's path through the chain constraint, so
        // only the head needs explicit speed scaling.
        let speed = step.chars_per_sec.max(0.0) * crate::constants::DRAGON_SPEED_SCALE;

        // Per-frame constants hoisted out of the per-dragon loop (they
        // only depend on the step geometry / fixed constants).
        let max_x = (step.cols as f32) - 0.5;
        let max_y = (step.lines as f32) - 0.5;
        let mid_soar = (crate::constants::DRAGON_SOAR_MIN_DURATION
            + crate::constants::DRAGON_SOAR_MAX_DURATION)
            * 0.5;
        let mid_circle = (crate::constants::DRAGON_CIRCLE_MIN_DURATION
            + crate::constants::DRAGON_CIRCLE_MAX_DURATION)
            * 0.5;

        let mut absorbed = 0usize;
        for d in &mut self.dragons {
            if !d.active {
                continue;
            }
            let dt_d = dt * d.pace;

            // ── Head motion ───────────────────────────────────────
            // State-dependent turn rate.
            let turn_rate = match d.state {
                DragonState::Soar => {
                    // Layered sine: two frequencies, randomized phase.
                    // Produces organic, non-repeating free flight.
                    let t = d.sim_age + d.noise_phase;
                    let s = (t * 0.7_f32).sin() * 0.4 + (t * 0.3_f32).sin() * 0.3;
                    s * crate::constants::DRAGON_SOAR_TURN_RATE
                }
                DragonState::Circle => {
                    // Constant-magnitude turn rate producing a circle.
                    // Direction (CW/CCW) randomized per state entry.
                    d.circle_dir as f32 * crate::constants::DRAGON_CIRCLE_TURN_RATE
                }
            };
            d.heading += turn_rate * dt_d;

            // Translate head along heading. NIGHT-hunter-10: pace
            // enters ONLY through dt_d — the previous form multiplied
            // it into the velocity vector as well, so head translation
            // scaled with pace squared (0.85..1.15 became 0.72..1.32
            // effective) while turn rate, state timer and sim_age all
            // scaled linearly, drifting each dragon off its documented
            // speed profile and shrinking the FABRIK stability margin
            // for fast dragons.
            let vx = d.heading.cos() * speed;
            let vy = d.heading.sin() * speed;
            // Head is segments[0].
            if let Some(head) = d.segments.first_mut() {
                head.x += vx * dt_d;
                head.y += vy * dt_d;
            }

            // ── Wall bounce ───────────────────────────────────
            // Reflect velocity component on wall hit; snap state to
            // SOAR so the dragon escapes any circle that would push
            // it back into the wall. Uses the viewport bounds from
            // DragonStep (passed in via the step struct) so the head
            // stays in the visible region and the chain remains
            // drawable. State timer reset to mid-range — the next
            // stochastic transition (when state_timer expires) will
            // roll fresh values via dragon_noise_roll.
            if let Some(head) = d.segments.first_mut() {
                if head.x < 0.5 {
                    head.x = 0.5;
                    // Reflect horizontal component: heading = PI - heading.
                    d.heading = std::f32::consts::PI - d.heading;
                    d.state = DragonState::Soar;
                    d.state_timer = mid_soar;
                } else if head.x > max_x {
                    head.x = max_x;
                    d.heading = std::f32::consts::PI - d.heading;
                    d.state = DragonState::Soar;
                    d.state_timer = mid_soar;
                }
                if head.y < 0.5 {
                    head.y = 0.5;
                    // Reflect vertical component: heading = -heading.
                    d.heading = -d.heading;
                    d.state = DragonState::Soar;
                    d.state_timer = mid_soar;
                } else if head.y > max_y {
                    head.y = max_y;
                    d.heading = -d.heading;
                    d.state = DragonState::Soar;
                    d.state_timer = mid_soar;
                }
            }

            // ── Body chain update (FABRIK distance constraint) ────
            // Each segment maintains fixed spacing to the previous.
            // Snake follow-the-leader: the chain trails the head's
            // path organically.
            let spacing = crate::constants::DRAGON_SEGMENT_SPACING;
            for i in 1..d.segments.len() {
                let (px, py) = {
                    let prev = &d.segments[i - 1];
                    (prev.x, prev.y)
                };
                let seg = &mut d.segments[i];
                let dx = seg.x - px;
                let dy = seg.y - py;
                let dist = (dx * dx + dy * dy).sqrt().max(0.001);
                let factor = (dist - spacing) / dist;
                seg.x -= dx * factor;
                seg.y -= dy * factor;
            }

            // ── State machine transitions ────────────────────────
            d.state_timer -= dt_d;
            if d.state_timer <= 0.0 {
                // Stochastic state transition.
                // After SOAR: 50% SOAR, 50% CIRCLE.
                // After CIRCLE: 70% SOAR, 30% CIRCLE (favor free flight).
                let roll = dragon_noise_roll(d);
                let new_state = match d.state {
                    DragonState::Soar => {
                        if roll < 0.5 {
                            DragonState::Soar
                        } else {
                            DragonState::Circle
                        }
                    }
                    DragonState::Circle => {
                        if roll < 0.7 {
                            DragonState::Soar
                        } else {
                            DragonState::Circle
                        }
                    }
                };
                d.circle_dir = if (d.sim_age * 13.7).sin() < 0.0 {
                    1
                } else {
                    -1
                };
                d.state = new_state;
                d.state_timer = match new_state {
                    DragonState::Soar => mid_soar,
                    DragonState::Circle => mid_circle,
                };
            }

            d.sim_age += dt_d;
            if d.sim_age >= d.lifetime {
                d.active = false;
                absorbed += 1;
            }

            // LTS (NIGHT-hunter-10): the heading is an unbounded f32
            // accumulator — turn-rate integration, wall reflections
            // and state transitions all add to it. Over a multi-day
            // session the accumulated magnitude grows until the f32
            // ulp degrades the turn-rate resolution (at ~130K rad the
            // ulp is ~0.0078 rad). The amortized wrap folds the value
            // back into [0, TAU) whenever it drifts past 64 turns —
            // trig-equivalent (cos/sin are 2π-periodic), the branch is
            // almost never taken, and the wrap threshold keeps the ulp
            // below 5e-5 rad for the whole wrapped range.
            if d.heading.abs() > DRAGON_HEADING_WRAP_LIMIT {
                d.heading = d.heading.rem_euclid(std::f32::consts::TAU);
            }
        }
        if absorbed > 0 {
            self.active_count = self.active_count.saturating_sub(absorbed);
        }
    }

    /// Draw pass — head + body chain + monolith-style diff cleanup
    /// (mirrors `VortexRain::draw` / `LorenzRain::draw`).
    ///
    /// Brightness gradient: head = Core, first third of body = Hot,
    /// middle third = Mid, tail third = Ghost. This serpentine fade
    /// is the Chinese-dragon body's visible signature.
    ///
    /// Matrix-style glyph mutation: when a segment crosses into a
    /// new cell, the glyph re-rolls with probability
    /// DRAGON_SHIMMER_CHANCE (mutation tied to motion, like classic
    /// matrix rain — parity with vortex/lorenz).
    pub(crate) fn draw(
        &mut self,
        ctx: &DrawCtx<'_>,
        frame: &mut Frame,
        cleanup: &mut super::monolith::MonolithCleanup<'_>,
        rng: &mut StdRng,
        rand_chance: &Uniform<f32>,
    ) {
        let lines_us = ctx.lines as usize;
        let body_len = crate::constants::DRAGON_BODY_LEN;

        self.current_cells.clear();
        for d in &mut self.dragons {
            if !d.active {
                continue;
            }
            for (i, seg) in d.segments.iter_mut().enumerate() {
                let col = seg.x.round() as i32;
                let line = seg.y.round() as i32;
                if col < 0 || line < 0 || col >= ctx.cols as i32 || line >= ctx.lines as i32 {
                    continue;
                }
                let (col, line) = (col as u16, line as u16);

                // Matrix shimmer: mutate the glyph when the segment
                // crosses into a new cell with a chance gate.
                if seg.first_frame {
                    seg.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
                    seg.first_frame = false;
                } else if (seg.last_col != col as i32 || seg.last_line != line as i32)
                    && rand_chance.sample(rng) < crate::constants::DRAGON_SHIMMER_CHANCE
                {
                    seg.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
                }
                seg.last_col = col as i32;
                seg.last_line = line as i32;

                let level = level_for_segment(i, body_len);
                draw_dragon_cell(ctx, frame, col, line, seg.ch, d.palette_slot, level);
                self.current_cells.push(DragonCell { col, line });
            }
        }

        // Pass 2: generation-tag every drawn cell (monolith pattern —
        // u32 counter bump instead of clearing the array).
        self.drawn_gen_counter = self.drawn_gen_counter.wrapping_add(1);
        let gen = self.drawn_gen_counter;
        // Dragon chains are sparse (a few hundred cells at most), so
        // the drawn_gen array is sized to cols*lines like the other
        // structured styles for direct indexing.
        let need_len = (ctx.cols as usize) * lines_us;
        if self.drawn_gen.len() != need_len {
            self.drawn_gen.resize(need_len, 0);
        }
        for cell in &self.current_cells {
            let idx = cell.col as usize * lines_us + cell.line as usize;
            if idx < self.drawn_gen.len() {
                self.drawn_gen[idx] = gen;
            }
        }

        // Pass 3: clear previous cells NOT redrawn this frame.
        let drawn_gen = &self.drawn_gen[..];
        for cell in &self.previous_cells {
            let idx = cell.col as usize * lines_us + cell.line as usize;
            if idx < drawn_gen.len() && drawn_gen[idx] == gen {
                continue;
            }
            clear_cell(frame, cleanup, cell.col, cell.line);
        }

        std::mem::swap(&mut self.previous_cells, &mut self.current_cells);
    }

    // -- Test-only diagnostics (mirrors vortex/lorenz *_for_test API) --

    #[cfg(test)]
    pub(crate) fn active_segments_for_test(&self) -> Vec<(f32, f32)> {
        let mut out = Vec::new();
        for d in &self.dragons {
            if d.active {
                for seg in &d.segments {
                    out.push((seg.x, seg.y));
                }
            }
        }
        out
    }

    #[cfg(test)]
    pub(crate) fn drawn_cells_for_test(&self) -> &[DragonCell] {
        &self.current_cells
    }

    #[cfg(test)]
    pub(crate) fn active_states_for_test(&self) -> Vec<DragonState> {
        self.dragons
            .iter()
            .filter(|d| d.active)
            .map(|d| d.state)
            .collect()
    }
}
