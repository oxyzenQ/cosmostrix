// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Black hole glyph infall (NIGHT-special-1 stage 3): the third act —
//! the rain itself becomes the accretion material. The module owns
//! the whole infall layer (the mote pool AND its physics, the spawn /
//! advance / draw passes), split from `black_hole.rs` the way
//! `ring.rs` and `halo.rs` split theirs — the orchestrator keeps only
//! the field and the pass calls, so the LOC cap debt of the main file
//! does not grow with the third pool.
//!
//! Motion DNA — gravitational capture, 100% distinct from every other
//! style in the engine: glyphs spawn above the viewport and fall
//! through the hole's field. The field is inverse-square gravity
//! toward the hole (a = G / r^2 in ball-outer-radius units), its
//! magnitude smoothstep-blended to zero at the influence edge — just
//! past the disk's reach — so rain far from the system falls straight
//! (the ambient matrix read the scene keeps at its edges) and rain
//! crossing the disk's reach starts to curve. Inside the capture
//! radius an accretion brake bleeds the TANGENTIAL velocity off
//! exponentially (infalling material shocks against the disk and
//! radiates angular momentum away — the radial plunge is untouched),
//! which turns hyperbolic fly-bys into tightening inspirals: a
//! captured glyph whips around the shadow, sheds its sideways speed
//! lap by lap, and plunges. A mote that crosses inside the event
//! horizon is eaten — deactivated, never drawn inside the empty core
//! — its final flash on the photon ring.
//!
//! Brightness is speed-graded (kinetic heat — the faster the mote,
//! the brighter the base: the physical read of accretion heating)
//! composed with the family's shared proximity ladder, so the far
//! ambient rain reads Ghost while the periapsis whip reads Core
//! white. Comet trails streak the arcs; the glyph re-rolls on new
//! cells through the motion-gated shimmer (the family life sign).
//!
//! Sim-time: one sim-second is one wall-second at the scene's
//! reference 12 cps; dt_sim = dt_wall x cps x SIM_TIME_PER_CPS. The
//! speed keys therefore scale positions, velocities AND gravity
//! together — trajectory shapes are invariant under the speed
//! setting (the family's speed contract, expressed as one scalar).
//! Integration is sub-stepped semi-implicit Euler (velocity first,
//! then position — the symplectic ordering that keeps orbits from
//! pumping energy), the substep capped so a lag-spike frame can
//! never tunnel a mote through the horizon.
//!
//! Geometry: every length runs in ball-outer-radius units (positions
//! x, y relative to the hole center, y down), every speed in outer
//! radii per sim-second — the physics is fully normalized, so it
//! reads identically on any screen size; the projection multiplies
//! through the cached outer radius at draw time (the
//! dynamic-screen-size contract). Family contracts honored: one mote
//! per column (lane model), deficit-bounded spawn accumulator with
//! its own fractional remainder, lifetime absorption with ±15%
//! variance, per-mote pace variance, palette adoption, comet trail,
//! three-pass diff cleanup (the drawn cells join the orchestrator's
//! unified current_cells stream).

use std::time::Duration;

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use super::super::super::render::DrawCtx;
use super::super::monolith::monolith_helpers::pick_pool_char;
use super::super::monolith::BrightnessLevel;
use super::ball_helpers::draw_ball_cell;
use super::black_hole::{BlackHoleCell, CELL_ASPECT_DIVISOR};
use super::ring::{proximity_level, step_down_level, BlackHoleRandom, BlackHoleSpawnParams};
use crate::frame::Frame;

/// Maximum physics substep in sim-seconds (numerical hygiene, not a
/// visual tuning knob, so it lives here instead of style_rain.rs):
/// integration splits any flight segment into chunks of at most this
/// size, capped in count, so a lag-spike frame or a cranked speed key
/// can never step a mote far enough to tunnel through the horizon —
/// the absorption check runs after every substep.
const INFALL_MAX_SUBSTEP: f32 = 0.02;

/// Substep count ceiling: at the substep size above, 64 substeps
/// cover 1.28 sim-seconds at full fidelity (every realistic frame —
/// max_sim_delta already clamps the wall dt). A segment beyond the
/// ceiling splits its time across the same 64 substeps — the step
/// widens evenly instead of the flight being clamped, so the physics
/// stays complete at bounded per-frame work (the test harness's long
/// flight legs ride the same path).
const INFALL_MAX_SUBSTEPS: u32 = 64;

/// One infalling glyph: a free-falling body in the hole's field.
/// Plain-old-data so the pool stays one flat Vec (cache-friendly, no
/// per-frame allocation — the family pool contract). Positions and
/// velocities are in ball-outer-radius units (x right, y down,
/// relative to the hole center; speeds in outer radii per
/// sim-second) — the normalization that keeps the physics identical
/// on every screen size.
#[derive(Clone, Copy, Debug)]
pub(crate) struct InfallMote {
    pub(crate) active: bool,
    /// Horizontal position in ball-outer-radius units relative to the
    /// hole center (positive right).
    pub(crate) x: f32,
    /// Vertical position in ball-outer-radius units (positive DOWN —
    /// screen convention, the same handedness the ring projection
    /// uses).
    pub(crate) y: f32,
    /// Horizontal velocity (outer radii per sim-second).
    pub(crate) vx: f32,
    /// Vertical velocity (outer radii per sim-second; positive falls).
    pub(crate) vy: f32,
    /// Simulation age in seconds (drives the lifetime backstop).
    pub(crate) sim_age: f32,
    /// Per-mote lifetime cap (±15% variance at spawn).
    pub(crate) lifetime: f32,
    /// Per-mote pace multiplier (0.85..1.15) — scales this mote's
    /// sim-dt so no two glyphs fall in lockstep.
    pub(crate) pace: f32,
    /// Glyph carried by the mote; re-rolled matrix-style when the
    /// head lands on a new cell (mutation tied to motion).
    pub(crate) ch: char,
    /// Palette slot adopted at spawn / palette transition.
    pub(crate) palette_slot: u8,
    /// Ring buffer of the last head cell positions (oldest first).
    pub(crate) trail: [(u16, u16); crate::constants::BLACK_HOLE_INFALL_TRAIL_LEN],
    pub(crate) trail_len: u8,
}

impl InfallMote {
    pub(crate) const fn vacant() -> Self {
        Self {
            active: false,
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
            sim_age: 0.0,
            lifetime: 0.0,
            pace: 1.0,
            ch: '0',
            palette_slot: 0,
            trail: [(0, 0); crate::constants::BLACK_HOLE_INFALL_TRAIL_LEN],
            trail_len: 0,
        }
    }

    /// Shift-left ring-buffer push (the family trail contract, mirrors
    /// `RingMote::push_trail`): drop the oldest position, append the
    /// newest at the tail.
    pub(crate) fn push_trail(&mut self, col: u16, line: u16) {
        let len = crate::constants::BLACK_HOLE_INFALL_TRAIL_LEN;
        if self.trail_len as usize >= len {
            for i in 0..len - 1 {
                self.trail[i] = self.trail[i + 1];
            }
            self.trail[len - 1] = (col, line);
        } else {
            let idx = self.trail_len as usize;
            self.trail[idx] = (col, line);
            self.trail_len += 1;
        }
    }
}

/// The infall layer: the third mote pool (one lane per column, the
/// family lane model) plus its own spawn accumulator remainder and
/// its cached spawn/exit geometry. The orchestrator (`black_hole.rs`)
/// owns the shared clock and calls the three passes; this struct owns
/// everything else about the rain.
pub(crate) struct InfallStream {
    /// The mote pool: one lane per column (a wider viewport hosts more
    /// ambient rain — the pool resizes with the screen).
    motes: Vec<InfallMote>,
    /// Active mote count (the spawn target's deficit baseline).
    active_infall: usize,
    /// Rotating scan cursor for amortized O(1) free-slot search (the
    /// same contract as the ring and halo pools).
    scan_idx: usize,
    /// Fractional spawn-remainder carry (this pool budgets
    /// independently of the ring and halo pools).
    spawn_remainder: f32,
    /// Cached spawn geometry: horizontal half-extent of the viewport
    /// in ball-outer-radius units (fresh glyphs spawn anywhere across
    /// it). Zero on a degenerate viewport — the spawn pass's gate.
    spawn_half_w: f32,
    /// Cached spawn geometry: the vertical half-extent in outer-radius
    /// units; fresh glyphs enter just above it (drift in, never pop).
    spawn_half_h: f32,
    /// Cached despawn margin beyond the viewport, in outer-radius
    /// units: a mote that leaves this envelope is gone.
    exit_margin: f32,
}

/// Draw-pass inputs (the bundle keeps clippy's too-many-arguments
/// threshold respected at the call site — mirrors the spawn/step
/// param bundles the family ships): the render context, the target
/// frame, the RNG pair, the ball anchor geometry, and the
/// orchestrator's unified current-cells stream.
pub(crate) struct InfallDrawArgs<'a> {
    pub(crate) ctx: &'a DrawCtx<'a>,
    pub(crate) frame: &'a mut Frame,
    pub(crate) rng: &'a mut StdRng,
    pub(crate) rand_chance: &'a Uniform<f32>,
    /// Ball anchor column as a float (the projection's x origin).
    pub(crate) cx: f32,
    /// Ball anchor line as a float (the projection's y origin).
    pub(crate) cy: f32,
    /// Ball outer radius in line-height units (the projection scale).
    pub(crate) ball_outer_r: f32,
    /// The orchestrator's unified drawn-cells stream (diff cleanup's
    /// input — the rain's cells join the ball's, the ring's and the
    /// halo's in one pipeline).
    pub(crate) current_cells: &'a mut Vec<BlackHoleCell>,
}

impl InfallStream {
    pub(crate) const fn new() -> Self {
        Self {
            motes: Vec::new(),
            active_infall: 0,
            scan_idx: 0,
            spawn_remainder: 0.0,
            spawn_half_w: 0.0,
            spawn_half_h: 0.0,
            exit_margin: 0.0,
        }
    }

    /// Rebuild the pool for a new viewport: one lane per column, all
    /// vacant, and the spawn/exit envelope re-cached from the ball
    /// anchor (the outer radius the ball raster computed). Degenerate
    /// viewports (no ball anchor) leave the pool empty and the
    /// envelope zeroed — the spawn gate.
    pub(crate) fn reset_pool(&mut self, cols: u16, lines: u16, ball_outer_r: f32) {
        self.motes.clear();
        if cols > 0 && lines > 0 && ball_outer_r >= 1.0 {
            self.motes.resize_with(cols as usize, InfallMote::vacant);
            self.spawn_half_w = (cols as f32 / (CELL_ASPECT_DIVISOR * 2.0)) / ball_outer_r * 0.98;
            self.spawn_half_h = (lines as f32 / 2.0) / ball_outer_r;
            // A quarter-radius of grace beyond the edge: strays that
            // slingshot outward get a beat to curve back before the
            // envelope drops them.
            self.exit_margin = 0.25;
        } else {
            self.spawn_half_w = 0.0;
            self.spawn_half_h = 0.0;
            self.exit_margin = 0.0;
        }
        self.active_infall = 0;
        self.scan_idx = 0;
        self.spawn_remainder = 0.0;
    }

    /// Zero the fractional spawn-remainder carry (the orchestrator's
    /// degenerate-viewport and pre-formation gates call this — the
    /// rain's budget twin of the halo remainder zeroing, so no pool
    /// banks credit while the hole is half-born).
    pub(crate) fn hold_spawn_budget(&mut self) {
        self.spawn_remainder = 0.0;
    }

    /// Steady-state active-mote count (the HUD metric's infall term).
    pub(crate) fn active_count(&self) -> usize {
        self.active_infall
    }

    /// Palette transition completion: every active infall mote adopts
    /// the new slot (the family adoption contract).
    pub(crate) fn adopt_palette_slot(&mut self, palette_slot: u8) {
        for m in &mut self.motes {
            if m.active {
                m.palette_slot = palette_slot;
            }
        }
    }

    /// Steady-state active target from pool size + density — the
    /// infall twin of the ring's and halo's targets: a low base (the
    /// rain is an AMBIENT layer, the hole stays the hero), the same
    /// density sensitivity so the slider moves both layers
    /// proportionally, capped below the disk's population.
    fn target_active(lanes: usize, density: f32) -> usize {
        if lanes == 0 {
            return 0;
        }
        let ratio = (crate::constants::BLACK_HOLE_INFALL_ACTIVE_BASE
            + density.clamp(0.01, 5.0) * crate::constants::BLACK_HOLE_INFALL_ACTIVE_DENSITY_MULT)
            .clamp(0.02, crate::constants::BLACK_HOLE_INFALL_ACTIVE_MAX);
        ((lanes as f32 * ratio).round() as usize).clamp(1, lanes)
    }

    /// Spawn pass — the family's deficit-bounded accumulator (budget
    /// = elapsed x rate + remainder, spawns capped by the deficit).
    /// Fresh glyphs enter just above the viewport top at a uniform
    /// horizontal position (the full-width read: rain everywhere, the
    /// bending zone only around the system), with the fall speed
    /// straight down plus a uniform drift that spreads the impact
    /// parameters — some glyphs fall dead-center, most pass offset,
    /// the spread that keeps every capture unique. The formation gate
    /// runs in the orchestrator (no rain before the hole is whole).
    pub(crate) fn spawn(
        &mut self,
        elapsed: Duration,
        params: &BlackHoleSpawnParams,
        random: &mut BlackHoleRandom<'_>,
    ) {
        if self.motes.is_empty()
            || self.spawn_half_w <= 0.0
            || self.spawn_half_h <= 0.0
            || params.cols == 0
            || params.lines == 0
        {
            self.spawn_remainder = 0.0;
            return;
        }
        let target = Self::target_active(self.motes.len(), params.density);
        if self.active_infall >= target {
            self.spawn_remainder = self
                .spawn_remainder
                .min(crate::constants::SPAWN_REMAINDER_CAP);
            return;
        }
        let deficit = target - self.active_infall;
        let rate = (target as f32 * crate::constants::BLACK_HOLE_INFALL_SPAWN_RATE_MULT
            + crate::constants::BLACK_HOLE_INFALL_SPAWN_RATE_FLOOR)
            * params.spawn_scale;
        let budget = elapsed.as_secs_f32() * rate
            + self
                .spawn_remainder
                .min(crate::constants::SPAWN_REMAINDER_CAP);
        if !budget.is_finite() || budget <= 0.0 {
            self.spawn_remainder = 0.0;
            return;
        }
        let to_spawn = (budget.floor() as usize).min(deficit);
        self.spawn_remainder =
            (budget - to_spawn as f32).min(crate::constants::SPAWN_REMAINDER_CAP);
        for _ in 0..to_spawn {
            let len = self.motes.len();
            if len == 0 {
                break;
            }
            // Amortized free-slot scan (rotating cursor).
            let mut idx = None;
            for step in 0..len {
                let cand = (self.scan_idx + step) % len;
                if !self.motes[cand].active {
                    self.scan_idx = (cand + 1) % len;
                    idx = Some(cand);
                    break;
                }
            }
            let Some(idx) = idx else {
                break;
            };
            activate_infall_mote(
                &mut self.motes[idx],
                self.spawn_half_w,
                self.spawn_half_h,
                params.active_palette_slot,
                random.rand_chance,
                random.rng,
            );
            self.active_infall += 1;
        }
    }

    /// Advance pass: integrate every active mote through the field
    /// (sub-stepped semi-implicit Euler), retire the eaten (inside the
    /// event horizon), the departed (outside the envelope) and the
    /// expired (lifetime backstop). Absorbed motes stop drawing — the
    /// diff cleanup clears their streaks on the next frame.
    pub(crate) fn advance(&mut self, dt_wall: f32, chars_per_sec: f32) {
        if self.active_infall == 0 || dt_wall <= 0.0 {
            return;
        }
        let dt_sim_base =
            dt_wall * chars_per_sec.max(0.0) * crate::constants::BLACK_HOLE_INFALL_SIM_TIME_PER_CPS;
        let mut absorbed = 0usize;
        for m in &mut self.motes {
            if !m.active {
                continue;
            }
            m.sim_age += dt_wall;
            if m.sim_age >= m.lifetime {
                m.active = false;
                m.trail_len = 0;
                absorbed += 1;
                continue;
            }
            let dt_sim = dt_sim_base * m.pace;
            if advance_infall_mote(
                m,
                dt_sim,
                self.spawn_half_w + self.exit_margin,
                self.spawn_half_h + self.exit_margin,
            ) {
                absorbed += 1;
            }
        }
        if absorbed > 0 {
            self.active_infall = self.active_infall.saturating_sub(absorbed);
        }
    }

    /// Draw pass: project every active mote onto the screen (the
    /// cached ball anchor + outer radius), grade the head's brightness
    /// by speed (kinetic heat) composed with the shared proximity
    /// ladder, draw head + comet trail, and record the cells into the
    /// orchestrator's unified current_cells stream. The empty core is
    /// never painted — a head inside the event horizon's screen
    /// distance is not drawn (the absorption owns the disappearance,
    /// and the stage-1 contract that the core is never drawn holds for
    /// the rain too).
    pub(crate) fn draw(&mut self, args: InfallDrawArgs<'_>) {
        let InfallDrawArgs {
            ctx,
            frame,
            rng,
            rand_chance,
            cx,
            cy,
            ball_outer_r,
            current_cells,
        } = args;
        if self.active_infall == 0 || ball_outer_r < 1.0 {
            return;
        }
        for m in &mut self.motes {
            if !m.active {
                continue;
            }
            let col_f = cx + m.x * ball_outer_r * CELL_ASPECT_DIVISOR;
            let line_f = cy + m.y * ball_outer_r;
            let col = col_f.round() as i32;
            let line = line_f.round() as i32;
            if col < 0 || line < 0 || col >= ctx.cols as i32 || line >= ctx.lines as i32 {
                // Off-screen: skip the draw AND the trail push — the
                // trail keeps its last in-bounds positions (the ring
                // contract: no phantom trail cells).
                continue;
            }
            let (col, line) = (col as u16, line as u16);

            // Kinetic heat composed with the proximity grade: the
            // mote's speed and screen distance (in ball radii — the
            // physics already carries it as r) feed the family's two
            // ladders. The whip near the shadow reads deep white, the
            // far ambient rain reads Ghost through the fade ladder.
            let speed = (m.vx * m.vx + m.vy * m.vy).sqrt();
            let dist_norm = (m.x * m.x + m.y * m.y).sqrt();
            let head_level = proximity_level(level_for_speed(speed), dist_norm);

            // The empty core is never painted (stage-1 contract): a
            // head whose physics position has already crossed inside
            // the event horizon waits for the absorption retire
            // instead of drawing a glyph on the hole itself.
            if dist_norm < crate::constants::BLACK_HOLE_CORE_FRACTION {
                continue;
            }

            // Motion-gated shimmer: mutate the glyph when the head
            // lands on a new cell, gated by the family chance — the
            // same life sign the ring and halo heads carry.
            if m.trail_len > 0 {
                let (prev_col, prev_line) = m.trail[(m.trail_len - 1) as usize];
                if (prev_col != col || prev_line != line)
                    && rand_chance.sample(rng) < crate::constants::BLACK_HOLE_INFALL_SHIMMER_CHANCE
                {
                    m.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
                }
            } else {
                m.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
            }

            // Head: the graded level (the rain reads in front of the
            // system — no occlusion path for the infall).
            draw_ball_cell(ctx, frame, col, line, m.ch, m.palette_slot, head_level);
            current_cells.push(BlackHoleCell {
                col,
                line,
                level: head_level,
            });

            // Comet trail: previously occupied cells, one brightness
            // rung dimmer each, drawn only while in bounds (the
            // streak follows the arc's curvature — the whip reads as
            // a streak, the calm fall as a soft tail).
            for t in 0..m.trail_len as usize {
                let (tc, tl) = m.trail[t];
                if tc >= ctx.cols || tl >= ctx.lines {
                    continue;
                }
                let depth = (m.trail_len as usize - t)
                    .min(crate::constants::BLACK_HOLE_INFALL_TRAIL_LEN)
                    as u8;
                let trail_level = step_down_level(head_level, depth);
                draw_ball_cell(ctx, frame, tc, tl, m.ch, m.palette_slot, trail_level);
                current_cells.push(BlackHoleCell {
                    col: tc,
                    line: tl,
                    level: trail_level,
                });
            }

            m.push_trail(col, line);
        }
    }

    // -- Test-only diagnostics (the family's *_for_test contract) --

    #[cfg(test)]
    /// The steady-state active target for a pool size + density (the
    /// ambient-cap contract's observable).
    pub(crate) fn target_active_for_test(lanes: usize, density: f32) -> usize {
        Self::target_active(lanes, density)
    }

    #[cfg(test)]
    pub(crate) fn motes_for_test(&self) -> &[InfallMote] {
        &self.motes
    }

    #[cfg(test)]
    pub(crate) fn active_infall_for_test(&self) -> usize {
        self.active_infall
    }
}

/// Activate a vacant mote above the viewport: a uniform horizontal
/// position across the spawn envelope, the entry line just past the
/// top edge (drift in — never a pop-in), the fall speed straight down
/// plus a uniform drift fraction of it (the impact-parameter spread),
/// and the family's per-mote lifetime / pace variance.
pub(crate) fn activate_infall_mote(
    m: &mut InfallMote,
    spawn_half_w: f32,
    spawn_half_h: f32,
    palette_slot: u8,
    rand_chance: &Uniform<f32>,
    rng: &mut StdRng,
) {
    m.active = true;
    m.x = (rand_chance.sample(rng) * 2.0 - 1.0) * spawn_half_w;
    m.y = -(spawn_half_h + 0.25);
    let drift =
        crate::constants::BLACK_HOLE_INFALL_DRIFT_FRACTION * (rand_chance.sample(rng) * 2.0 - 1.0);
    m.vx = crate::constants::BLACK_HOLE_INFALL_FALL_SPEED * drift;
    m.vy = crate::constants::BLACK_HOLE_INFALL_FALL_SPEED;
    m.sim_age = 0.0;
    m.lifetime =
        crate::constants::BLACK_HOLE_INFALL_MAX_AGE_SECS * (0.85 + rand_chance.sample(rng) * 0.30);
    m.pace = 0.85 + rand_chance.sample(rng) * 0.30;
    m.palette_slot = palette_slot;
    m.trail_len = 0;
}

/// Advance one mote through the field by `dt_sim` sim-seconds
/// (sub-stepped INTERNALLY: the segment splits into chunks of at most
/// `INFALL_MAX_SUBSTEP` sim-seconds while the count ceiling allows,
/// the time spread evenly across the ceiling's chunks beyond it —
/// complete physics at bounded work, and the per-substep absorption
/// check means no lag spike can ever tunnel a mote through the
/// horizon). The physics, in ball-outer-radius units:
///
/// 1. Gravity: inverse-square toward the hole, blended to zero at the
///    influence edge (smoothstep) — straight rain beyond the system,
///    arcs inside it.
/// 2. Accretion brake: inside the capture radius the TANGENTIAL
///    velocity decays exponentially (angular momentum radiated into
///    the disk; the radial plunge untouched) — fly-bys become
///    inspirals.
/// 3. Semi-implicit Euler (velocity first, then position — the
///    symplectic ordering that keeps orbits from pumping energy).
///
/// Retires the mote (returns true) when it crosses the event horizon
/// (eaten) or leaves the exit envelope (departed); returns false
/// while it lives.
pub(crate) fn advance_infall_mote(
    m: &mut InfallMote,
    dt_sim: f32,
    exit_half_w: f32,
    exit_half_h: f32,
) -> bool {
    // The substep split: chunks of at most MAX_SUBSTEP while the count
    // allows, the time spread evenly across the ceiling's 64 chunks
    // beyond it (complete physics, bounded work — see the constants'
    // docs above).
    let total = dt_sim.max(0.0);
    let steps = if total <= 0.0 {
        0
    } else {
        ((total / INFALL_MAX_SUBSTEP).ceil() as u32).clamp(1, INFALL_MAX_SUBSTEPS)
    };
    let dt = if steps > 0 { total / steps as f32 } else { 0.0 };
    for _ in 0..steps {
        let r = (m.x * m.x + m.y * m.y).sqrt();
        // The gravity blend: full inside the inner blend point, a
        // smoothstep fade to zero at the influence edge. One (r - G)
        // smoothstep over the fade span reads gentle at the edge and
        // lands flat inside — the field feels monolithic, not ringed.
        let influence = crate::constants::BLACK_HOLE_INFALL_INFLUENCE_FRACTION;
        let blend = if r >= influence {
            0.0
        } else if r <= influence * 0.55 {
            1.0
        } else {
            let t = ((r - influence * 0.55) / (influence * 0.45)).clamp(0.0, 1.0);
            1.0 - (t * t * (3.0 - 2.0 * t))
        };
        if blend > 0.0 && r > 0.05 {
            let g = crate::constants::BLACK_HOLE_INFALL_GRAVITY * blend / (r * r * r);
            m.vx += -m.x * g * dt;
            m.vy += -m.y * g * dt;
        }

        // The accretion brake: inside the capture radius the
        // tangential speed decays. Decompose the velocity into radial
        // and tangential parts, decay the tangential part only, and
        // recompose — the plunge survives, the sideways drift dies,
        // and the orbit tightens.
        let capture = crate::constants::BLACK_HOLE_INFALL_CAPTURE_FRACTION;
        if r < capture && r > 0.05 {
            let t = ((capture - r) / (capture * 0.5)).clamp(0.0, 1.0);
            let strength = t * t * (3.0 - 2.0 * t);
            let decay = (-crate::constants::BLACK_HOLE_INFALL_DRAG_RATE * strength * dt).exp();
            let rx = m.x / r;
            let ry = m.y / r;
            let v_r = m.vx * rx + m.vy * ry;
            let v_t = -m.vx * ry + m.vy * rx;
            let v_t = v_t * decay;
            m.vx = v_r * rx - v_t * ry;
            m.vy = v_r * ry + v_t * rx;
        }

        // Semi-implicit Euler: the position advances on the updated
        // velocity (the symplectic ordering).
        m.x += m.vx * dt;
        m.y += m.vy * dt;

        // The horizon eats: a mote that crosses inside the event
        // horizon's radius is gone — checked per substep so a fast
        // whip cannot tunnel through.
        let r_now = (m.x * m.x + m.y * m.y).sqrt();
        if r_now < crate::constants::BLACK_HOLE_CORE_FRACTION {
            m.active = false;
            m.trail_len = 0;
            return true;
        }
    }
    // The envelope drops the departed: strays that slingshot or drift
    // beyond the spawn envelope plus the exit margin leave the scene.
    if m.x.abs() > exit_half_w || m.y.abs() > exit_half_h {
        m.active = false;
        m.trail_len = 0;
        return true;
    }
    false
}

/// Kinetic-heat brightness ladder: the mote's speed (outer radii per
/// sim-second) maps the base level — slow distant rain reads Ghost,
/// the accelerating fall reads Mid, the approach Hot, the periapsis
/// whip Core. Composed with the proximity grade by the caller, so
/// near the shadow the whip lands deep white.
pub(crate) fn level_for_speed(speed: f32) -> BrightnessLevel {
    if speed > crate::constants::BLACK_HOLE_INFALL_SPEED_CORE {
        BrightnessLevel::Core
    } else if speed > crate::constants::BLACK_HOLE_INFALL_SPEED_MID {
        BrightnessLevel::Hot
    } else if speed > crate::constants::BLACK_HOLE_INFALL_SPEED_GHOST {
        BrightnessLevel::Mid
    } else {
        BrightnessLevel::Ghost
    }
}
