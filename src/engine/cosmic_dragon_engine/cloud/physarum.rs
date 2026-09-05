// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Physarum slime-mold rain for the physarum scene (NIGHT-research-6,
//! the sixth rain style — bio-inspired emergent network patterns).
//!
//! Motion DNA — 100% distinct from cascade (`cinematic`), pillars
//! (`monolith`), polar-orbit (`vortex`), strange-attractor
//! (`lorenz`), and serpentine chain (`cosmic_dragon`): each particle
//! is a glyph agent following the Jeff Jones 2010 slime-mold model.
//! Three rules per frame (sense / decide / move-deposit) plus
//! exponential trail decay produce emergent NETWORK patterns —
//! vein-like structures that self-organize from random initial
//! conditions, with no central planner.
//!
//! Stigmergy is the key mechanic: agents leave a chemical trail
//! (deposited at their current cell each step), and agents sample
//! the trail at three sensor positions (left-front, front, right-
//! front) to steer toward the strongest signal. Positive feedback
//! between deposition and sensing creates the network — paths
//! that get used attract more traffic, unused paths decay.
//!
//! Terminal-limit exploitation — the masterpiece contract:
//!
//! - The terminal's discrete cell grid IS the slime-mold substrate
//!   (a 2D chemical concentration field, one f32 per cell). No
//!   sub-pixel motion, no anti-aliasing — the medium matches the
//!   algorithm exactly.
//!
//! - The trail field is INTERNAL (used for sensor sampling only);
//!   the visible vein network emerges from the engine's existing
//!   phosphor decay system. Cells that particles visit often
//!   accumulate phosphor (existing slow fade), creating the
//!   persistent network look — the terminal's "slow refresh"
//!   limitation BECOMES the slime mold's chemical memory.
//!
//! - Particle head brightness is driven by the trail field value
//!   at the head position (high trail = bright vein cell; low
//!   trail = exploring dim cell), so the network is visible via
//!   the heads themselves — no direct trail field iteration
//!   needed (keeps draw cost O(N) not O(cells)).
//!
//! - Wraparound edges (toroidal substrate): particles that exit
//!   one side reappear on the opposite side. Slime mold in nature
//!   grows on surfaces; the torus topology is the standard Jeff
//!   Jones implementation, prevents corner clustering, and lets
//!   networks span the full viewport organically.
//!
//! Masterpiece engineering / future-proof legacy:
//!
//! The algorithm is parameter-driven (sensor angle, sensor distance,
//! deposit amount, decay rate, turn speed). The same code produces
//! vastly different emergent patterns — branching trees (small
//! sensor angle), spirals (high turn speed), mazes (low decay),
//! rings (high deposit). This file is the project's first bio-
//! inspired renderer; it bridges biology (slime mold intelligence
//! — Physarum polycephalum solves mazes without nervous system),
//! computer science (stigmergy / multi-agent swarms), and
//! generative art (network aesthetics). The pattern sets a
//! reusable standard for future bio-inspired styles (ant colonies,
//! flocking birds, schooling fish could all reuse the trail-field
//! + sense-decide-move substrate).
//!
//! LOC note: at ~700 lines this matches the vortex/lorenz/dragon
//! file budgets — a single self-contained style system (state,
//! spawn, advance, draw, and diff cleanup are one algorithm;
//! splitting them mirrors the monolith family split only once the
//! file approaches the 800-line hard cap). Well under the hard
//! limit.
//!
//! Cleanup follows the monolith/vortex/lorenz/dragon three-pass
//! diff pattern: draw into `current_cells`, tag with the
//! `drawn_gen` generation counter, then clear only previous cells
//! NOT redrawn this frame (phosphor metadata and frame blank).

use std::time::{Duration, Instant};

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::frame::Frame;

use super::monolith_helpers::{clear_cell, pick_pool_char};
use super::physarum_helpers::{draw_physarum_cell, level_for_trail, sample_random, sample_trail};
use super::render::DrawCtx;

/// Heading-accumulator wrap threshold (radians, 64 turns). The
/// steering integrator adds the per-frame turn into a bare f32; the
/// amortized wrap in `advance` folds it back into [0, TAU) once it
/// drifts past this limit so a multi-day session cannot degrade the
/// turn-rate resolution (trig-equivalent — cos/sin are 2π-periodic).
const PHYSARUM_HEADING_WRAP_LIMIT: f32 = 64.0 * std::f32::consts::TAU;

/// Reference cadence (steps per simulated second) the
/// PHYSARUM_TRAIL_DECAY per-step constant is quoted against. The
/// advance pass raises the constant to (dt × this) so the decay is
/// frame-rate independent (see the trail-decay block in `advance`).
const PHYSARUM_TRAIL_DECAY_REF_HZ: f32 = 60.0;

/// One drawn cell (col, line). Own struct instead of reusing
/// monolith's `DrawnCell` because physarum has no Segment/Spine
/// kind distinction (same shape as the other style cells).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PhysarumCell {
    pub(crate) col: u16,
    pub(crate) line: u16,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PhysarumParticle {
    pub(crate) active: bool,
    /// Continuous position (cell space, fractional for smooth motion).
    pub(crate) x: f32,
    pub(crate) y: f32,
    /// Heading angle (radians). Steered each frame by sensor sampling.
    pub(crate) heading: f32,
    /// Per-particle speed multiplier (0.85..1.15).
    pub(crate) pace: f32,
    /// Simulation age (seconds, dt-integrated). Drives absorption.
    pub(crate) sim_age: f32,
    /// Per-particle lifetime cap (variance per spawn for staggered refresh).
    pub(crate) lifetime: f32,
    /// Glyph carried by the particle; re-rolled matrix-style when the
    /// particle crosses into a new cell (matrix-shimmer tied to motion).
    pub(crate) ch: char,
    /// Last cell (col, line) this particle occupied — used to detect
    /// cell crossings for the shimmer gate.
    last_col: i32,
    last_line: i32,
    /// Set true on first frame after activation so the shimmer gate
    /// initializes the glyph without falsely detecting a crossing.
    first_frame: bool,
    /// Palette slot adopted at spawn / palette transition.
    pub(crate) palette_slot: u8,
}

impl PhysarumParticle {
    const fn vacant() -> Self {
        Self {
            active: false,
            x: 0.0,
            y: 0.0,
            heading: 0.0,
            pace: 1.0,
            sim_age: 0.0,
            lifetime: 0.0,
            ch: '0',
            last_col: -1,
            last_line: -1,
            first_frame: true,
            palette_slot: 0,
        }
    }
}

/// Spawn inputs (mirrors `VortexSpawnParams` / `LorenzSpawnParams` /
/// `DragonSpawnParams`).
pub(crate) struct PhysarumSpawnParams {
    pub(crate) cols: u16,
    pub(crate) lines: u16,
    pub(crate) density: f32,
    pub(crate) active_palette_slot: u8,
    pub(crate) spawn_scale: f32,
}

/// RNG bundle (mirrors the other structured styles).
pub(crate) struct PhysarumRandom<'a> {
    pub(crate) rng: &'a mut StdRng,
    pub(crate) rand_chance: &'a Uniform<f32>,
}

/// Per-frame step inputs for the advance pass. Viewport geometry is
/// carried here so the trail field can be sampled at sensor offsets
/// using actual viewport bounds (wraparound needs the modulus).
pub(crate) struct PhysarumStep {
    pub(crate) now: Instant,
    /// chars_per_sec already multiplied by the terminal speed_mult.
    /// Drives particle translation speed so ↑/↓ speed keys feel
    /// native on the stigmergic network.
    pub(crate) chars_per_sec: f32,
    pub(crate) cols: u16,
    pub(crate) lines: u16,
    pub(crate) max_sim_delta: Duration,
    pub(crate) resume_blend: f32,
}

pub(crate) struct PhysarumRain {
    pub(crate) particles: Vec<PhysarumParticle>,
    active_count: usize,
    /// Rotating scan cursor for amortized O(1) free-slot search
    /// (mirrors `VortexRain::spawn_scan_idx` / `LorenzRain` /
    /// `DragonRain`).
    spawn_scan_idx: usize,
    /// Stigmergy trail field: one concentration value per cell
    /// (flat layout: `col * lines + line`). Used by sensor sampling
    /// for particle steering. NOT drawn directly — the visible vein
    /// network emerges from the engine's existing phosphor decay
    /// system. This is the masterpiece contract: the terminal's
    /// discrete cell grid IS the slime-mold substrate.
    trail_field: Vec<f32>,
    trail_cols: u16,
    trail_lines: u16,
    /// Global motion clock. dt = now - last_step clamped by
    /// max_sim_delta; a fully-paused run simply stops integrating
    /// (rain_at early-return), so no pause-time shift is required.
    last_step: Option<Instant>,
    current_cells: Vec<PhysarumCell>,
    previous_cells: Vec<PhysarumCell>,
    drawn_gen: Vec<u32>,
    drawn_gen_counter: u32,
}

impl PhysarumRain {
    pub(crate) fn new() -> Self {
        Self {
            particles: Vec::new(),
            active_count: 0,
            spawn_scan_idx: 0,
            trail_field: Vec::new(),
            trail_cols: 0,
            trail_lines: 0,
            last_step: None,
            current_cells: Vec::new(),
            previous_cells: Vec::new(),
            drawn_gen: Vec::new(),
            drawn_gen_counter: 0,
        }
    }

    /// Rebuild the particle pool + trail field for a new viewport
    /// (or style entry). Pool is sized one particle per column
    /// (mirrors vortex/lorenz); active target is a density-driven
    /// ratio of that pool. Trail field is resized to cols*lines
    /// (cleared to zero — fresh substrate).
    pub(crate) fn reset(&mut self, cols: u16) {
        let lanes = cols.max(1) as usize;
        self.particles.clear();
        self.particles.resize_with(lanes, PhysarumParticle::vacant);
        self.active_count = 0;
        self.spawn_scan_idx = 0;
        self.last_step = None;
        self.clear_draw_history();
        // Trail field is allocated lazily on first advance() call
        // (which has access to both cols and lines via PhysarumStep).
        // reset() only knows cols; lines comes from the step struct.
        self.trail_field.clear();
        self.trail_cols = 0;
        self.trail_lines = 0;
    }

    pub(crate) fn active_count(&self) -> usize {
        self.active_count
    }

    /// Palette transition completion: all active particles adopt the
    /// new slot (mirrors `VortexRain::adopt_palette_slot`).
    pub(crate) fn adopt_palette_slot(&mut self, palette_slot: u8) {
        for p in &mut self.particles {
            if p.active {
                p.palette_slot = palette_slot;
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

    /// Steady-state active-particle target from pool size + density.
    fn target_active_count(pool_size: usize, density: f32) -> usize {
        if pool_size == 0 {
            return 0;
        }
        let ratio = (crate::constants::PHYSARUM_ACTIVE_BASE
            + density.clamp(0.01, 5.0) * crate::constants::PHYSARUM_ACTIVE_DENSITY_MULT)
            .clamp(0.02, crate::constants::PHYSARUM_ACTIVE_MAX);
        ((pool_size as f32 * ratio).round() as usize).clamp(1, pool_size)
    }

    /// Amortized free-slot scan (rotating cursor — mirrors the
    /// other structured styles' `find_inactive_*`).
    fn find_inactive_particle(&mut self) -> Option<usize> {
        let len = self.particles.len();
        if len == 0 {
            return None;
        }
        for step in 0..len {
            let idx = (self.spawn_scan_idx + step) % len;
            if !self.particles[idx].active {
                self.spawn_scan_idx = (idx + 1) % len;
                return Some(idx);
            }
        }
        None
    }

    /// Spawn pass — accumulator pattern identical to vortex/lorenz/
    /// dragon (deficit-bounded budget + fractional remainder carry).
    pub(crate) fn spawn(
        &mut self,
        elapsed: Duration,
        spawn_remainder: &mut f32,
        params: &PhysarumSpawnParams,
        random: &mut PhysarumRandom<'_>,
    ) {
        if params.cols == 0 || params.lines == 0 || self.particles.is_empty() {
            *spawn_remainder = 0.0;
            return;
        }

        let target = Self::target_active_count(self.particles.len(), params.density);
        if self.active_count >= target {
            *spawn_remainder = (*spawn_remainder).min(crate::constants::SPAWN_REMAINDER_CAP);
            return;
        }

        let deficit = target - self.active_count;
        let spawn_rate = (target as f32 * crate::constants::PHYSARUM_SPAWN_RATE_MULT
            + crate::constants::PHYSARUM_SPAWN_RATE_FLOOR)
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
            let Some(idx) = self.find_inactive_particle() else {
                break;
            };
            self.activate_particle(
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

    /// Activate a vacant particle at a random viewport position with
    /// a random heading. Particles spawn uniform-randomly across the
    /// full viewport (NOT at the center, like vortex/lorenz) —
    /// random initial distribution is critical for emergent network
    /// patterns (a clustered spawn would bias the trail field toward
    /// one location and break the symmetry that produces the
    /// branching veins).
    fn activate_particle(
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
        let x = rand_chance.sample(rng) * cols_f;
        let y = rand_chance.sample(rng) * lines_f;
        let heading = rand_chance.sample(rng) * std::f32::consts::TAU;
        let pace = 0.85 + rand_chance.sample(rng) * 0.30;
        let lifetime =
            crate::constants::PHYSARUM_LIFETIME_SECS * (0.85 + rand_chance.sample(rng) * 0.30);

        let p = &mut self.particles[idx];
        p.active = true;
        p.x = x;
        p.y = y;
        p.heading = heading;
        p.pace = pace;
        p.sim_age = 0.0;
        p.lifetime = lifetime;
        p.ch = '0';
        p.last_col = -1;
        p.last_line = -1;
        p.first_frame = true;
        p.palette_slot = palette_slot;
    }

    /// Motion pass — the Jeff Jones slime-mold model core.
    ///
    /// Per particle per frame:
    /// 1. Sample trail field at three sensor positions (left-front,
    ///    front, right-front) using the particle's heading.
    /// 2. Decide turn direction: turn toward the strongest signal
    ///    (with random tie-break for equal signals).
    /// 3. Move one step in the (possibly updated) heading direction.
    ///    Wraparound on edge crossing (toroidal substrate).
    /// 4. Deposit trail chemical at the new cell position.
    ///
    /// After all particles advance, the trail field decays
    /// exponentially (the 60 Hz reference constant raised to dt×60,
    /// so the decay is frame-rate independent — see the trail-decay
    /// block). This is the negative feedback that lets unused paths
    /// fade — without it, the field saturates and the network
    /// disappears.
    pub(crate) fn advance(&mut self, step: &PhysarumStep) {
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

        // Lazily (re)allocate the trail field on first advance or
        // viewport resize. Trail is cleared to zero — fresh substrate.
        let cols_us = step.cols as usize;
        let lines_us = step.lines as usize;
        let total = cols_us * lines_us;
        if self.trail_cols != step.cols || self.trail_lines != step.lines {
            self.trail_field.clear();
            self.trail_field.resize(total, 0.0);
            self.trail_cols = step.cols;
            self.trail_lines = step.lines;
        } else if self.trail_field.len() != total {
            self.trail_field.clear();
            self.trail_field.resize(total, 0.0);
        }

        let cols_f = step.cols as f32;
        let lines_f = step.lines as f32;

        // Per-frame step distance — drives particle translation.
        // Scaled by chars_per_sec (so speed keys feel native) and dt.
        let step_dist = step.chars_per_sec.max(0.0) * crate::constants::PHYSARUM_STEP_PER_CPS * dt;

        let sensor_angle = crate::constants::PHYSARUM_SENSOR_ANGLE;
        let sensor_dist = crate::constants::PHYSARUM_SENSOR_DISTANCE;
        let turn_rate = crate::constants::PHYSARUM_TURN_RATE;
        let deposit = crate::constants::PHYSARUM_DEPOSIT_AMOUNT;

        // Sensor direction ladder (NIGHT-hunter-10): the three sample
        // directions are the heading rotated by -a, 0 and +a. The trig
        // for the CONSTANT sensor angle is hoisted — one pair per
        // advance call instead of two per particle — and the
        // per-particle sensor directions come from the angle-addition
        // identities (cos(h±a) = cos_h·cos_a ∓ sin_h·sin_a and the
        // matching sine form): exact math for any sensor-angle tuning,
        // two trig calls per particle where the previous form spent
        // six (plus two for the move).
        let sensor_cos = sensor_angle.cos();
        let sensor_sin = sensor_angle.sin();

        let mut absorbed = 0usize;
        for p in &mut self.particles {
            if !p.active {
                continue;
            }
            let dt_p = dt * p.pace;

            // ── 1. Sense ──────────────────────────────────────────
            // Three sensor positions: left-front, front, right-front.
            // Sensor samples the trail field at offset distance from
            // the particle, in directions offset by ±sensor_angle.
            let cos_h = p.heading.cos();
            let sin_h = p.heading.sin();
            // Angle-addition ladder (see the hoist comment above):
            // identical sensor angles to the trig form, four fewer
            // trig calls per particle.
            let left_dx = cos_h * sensor_cos + sin_h * sensor_sin;
            let left_dy = sin_h * sensor_cos - cos_h * sensor_sin;
            let right_dx = cos_h * sensor_cos - sin_h * sensor_sin;
            let right_dy = sin_h * sensor_cos + cos_h * sensor_sin;
            let s_front = sample_trail(
                &self.trail_field,
                cols_us,
                lines_us,
                p.x + cos_h * sensor_dist,
                p.y + sin_h * sensor_dist,
            );
            let s_left = sample_trail(
                &self.trail_field,
                cols_us,
                lines_us,
                p.x + left_dx * sensor_dist,
                p.y + left_dy * sensor_dist,
            );
            let s_right = sample_trail(
                &self.trail_field,
                cols_us,
                lines_us,
                p.x + right_dx * sensor_dist,
                p.y + right_dy * sensor_dist,
            );

            // ── 2. Decide ─────────────────────────────────────────
            // Turn toward strongest signal; random tie-break.
            // The turn is rate-bounded (max PHYSARUM_TURN_RATE per
            // frame) so particles don't snap to new headings — the
            // smooth turn produces the curved vein signature.
            let turn = if s_front > s_left && s_front > s_right {
                0.0
            } else if s_left > s_right {
                -turn_rate * dt_p
            } else if s_right > s_left {
                turn_rate * dt_p
            } else {
                // All equal: random walk nudge (keeps the simulation
                // from settling into a static equilibrium — the
                // random tie-break is what gives slime mold its
                // continuous exploratory behavior).
                (sample_random(p.sim_age) - 0.5) * turn_rate * dt_p * 2.0
            };
            p.heading += turn;

            // LTS (NIGHT-hunter-10): amortized heading wrap. The
            // steering integrator accumulates the per-frame turn into
            // a bare f32; past the wrap limit the ulp starts degrading
            // the turn resolution. Folding into [0, TAU) is
            // trig-equivalent and the branch is almost never taken.
            if p.heading.abs() > PHYSARUM_HEADING_WRAP_LIMIT {
                p.heading = p.heading.rem_euclid(std::f32::consts::TAU);
            }

            // ── 3. Move (wraparound toroidal substrate) ───────────
            let dist = step_dist * p.pace;
            p.x += p.heading.cos() * dist;
            p.y += p.heading.sin() * dist;
            // Wraparound: modulo arithmetic keeps the particle on
            // the torus. The trail field is also indexed with
            // wraparound in sample_trail, so sensing and motion
            // are consistent across edges.
            if p.x < 0.0 {
                p.x += cols_f;
            } else if p.x >= cols_f {
                p.x -= cols_f;
            }
            if p.y < 0.0 {
                p.y += lines_f;
            } else if p.y >= lines_f {
                p.y -= lines_f;
            }

            // ── 4. Deposit ────────────────────────────────────────
            // Leave trail chemical at the new cell. The accumulation
            // across particles + frames creates the network pattern.
            let cx = p.x.round() as i32;
            let cy = p.y.round() as i32;
            if cx >= 0 && cy >= 0 && cx < cols_us as i32 && cy < lines_us as i32 {
                let idx = cx as usize * lines_us + cy as usize;
                self.trail_field[idx] += deposit * dt_p;
            }

            p.sim_age += dt;
            if p.sim_age >= p.lifetime {
                p.active = false;
                absorbed += 1;
            }
        }
        if absorbed > 0 {
            self.active_count = self.active_count.saturating_sub(absorbed);
        }

        // ── Trail decay (negative feedback) ─────────────────────
        // Rate-independent exponential decay (NIGHT-hunter-10): the
        // per-step multiplier is the 60 Hz reference constant raised
        // to (dt × 60), so a 144 Hz terminal and a 30 Hz terminal apply
        // the SAME per-second decay and the field's equilibrium — and
        // therefore the brightness grading against the absolute
        // PHYSARUM_BRIGHTNESS_* thresholds — is frame-rate invariant,
        // the family rate-independence contract flux enforces with its
        // fixed-step solver. The previous form multiplied the constant
        // once per advance call, so the equilibrium scaled with the
        // frame rate (a 144 Hz terminal ran the veins dimmer:
        // single-particle cells settled near 0.042 instead of ~0.083,
        // and multi-particle veins graded Hot where 60 Hz graded
        // Core). dt is the blended sim clock, so the decay also slows
        // during the resume ramp, consistent with the engine-wide
        // pause-in-slow-motion philosophy. Without decay the field
        // saturates and the network pattern disappears (every cell
        // equal → no gradient → no steering → no pattern); the decay
        // lets unused paths fade so the network stays alive.
        let decay = crate::constants::PHYSARUM_TRAIL_DECAY.powf(dt * PHYSARUM_TRAIL_DECAY_REF_HZ);
        for v in &mut self.trail_field {
            *v *= decay;
        }
    }

    /// Draw pass — particle heads + monolith-style diff cleanup
    /// (mirrors `VortexRain::draw` / `LorenzRain::draw` /
    /// `DragonRain::draw`).
    ///
    /// Brightness is driven by the trail field value at the head
    /// position: high trail = Core (bright vein cell), low trail =
    /// Ghost (exploring new territory). This makes the network
    /// visible via the heads themselves — no direct trail field
    /// iteration needed (keeps draw cost O(N), not O(cells)).
    ///
    /// The persistent vein look emerges from the engine's existing
    /// phosphor decay system — cells particles visit often accumulate
    /// phosphor, creating the slow fade that's the slime mold's
    /// chemical memory made visible.
    ///
    /// Matrix-style glyph mutation: when a particle crosses into a
    /// new cell, the glyph re-rolls with probability
    /// PHYSARUM_SHIMMER_CHANCE (mutation tied to motion — parity
    /// with vortex/lorenz/dragon).
    pub(crate) fn draw(
        &mut self,
        ctx: &DrawCtx<'_>,
        frame: &mut Frame,
        cleanup: &mut super::monolith::MonolithCleanup<'_>,
        rng: &mut StdRng,
        rand_chance: &Uniform<f32>,
    ) {
        let lines_us = ctx.lines as usize;
        let cols_us = ctx.cols as usize;
        // Re-sync trail field dimensions if the draw viewport differs
        // from the advance viewport (shouldn't happen in normal use
        // but defensive against edge cases).
        if self.trail_cols != ctx.cols || self.trail_lines != ctx.lines {
            self.trail_field.clear();
            self.trail_field.resize(cols_us * lines_us, 0.0);
            self.trail_cols = ctx.cols;
            self.trail_lines = ctx.lines;
        }

        self.current_cells.clear();
        for p in &mut self.particles {
            if !p.active {
                continue;
            }
            let col = p.x.round() as i32;
            let line = p.y.round() as i32;
            if col < 0 || line < 0 || col >= ctx.cols as i32 || line >= ctx.lines as i32 {
                continue;
            }
            let (col, line) = (col as u16, line as u16);

            // Matrix shimmer: mutate the glyph when the head crosses
            // into a new cell with a chance gate.
            if p.first_frame {
                p.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
                p.first_frame = false;
            } else if (p.last_col != col as i32 || p.last_line != line as i32)
                && rand_chance.sample(rng) < crate::constants::PHYSARUM_SHIMMER_CHANCE
            {
                p.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
            }
            p.last_col = col as i32;
            p.last_line = line as i32;

            // Brightness from trail field at head position.
            let trail_val = {
                let idx = col as usize * lines_us + line as usize;
                if idx < self.trail_field.len() {
                    self.trail_field[idx]
                } else {
                    0.0
                }
            };
            let level = level_for_trail(trail_val);
            draw_physarum_cell(ctx, frame, col, line, p.ch, p.palette_slot, level);
            self.current_cells.push(PhysarumCell { col, line });
        }

        // Pass 2: generation-tag every drawn cell (monolith pattern —
        // u32 counter bump instead of clearing the array).
        self.drawn_gen_counter = self.drawn_gen_counter.wrapping_add(1);
        let gen = self.drawn_gen_counter;
        let need_len = self.particles.len().saturating_mul(lines_us.max(1));
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

    // -- Test-only diagnostics (mirrors vortex/lorenz/dragon
    // *_for_test API) --

    #[cfg(test)]
    pub(crate) fn active_positions_for_test(&self) -> Vec<(f32, f32)> {
        self.particles
            .iter()
            .filter(|p| p.active)
            .map(|p| (p.x, p.y))
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn drawn_cells_for_test(&self) -> &[PhysarumCell] {
        &self.current_cells
    }

    #[cfg(test)]
    pub(crate) fn trail_max_for_test(&self) -> f32 {
        self.trail_field.iter().copied().fold(0.0_f32, f32::max)
    }

    /// Trail field read hook (decay-cadence contract tests watch a
    /// chosen cell without driving particles through it).
    #[cfg(test)]
    pub(crate) fn trail_value_for_test(&self, col: u16, line: u16) -> Option<f32> {
        let idx = col as usize * self.trail_lines as usize + line as usize;
        self.trail_field.get(idx).copied()
    }

    /// Trail field seed hook (builds targeted trail landscapes for the
    /// sensor-steering contracts; allocates the field when empty).
    #[cfg(test)]
    pub(crate) fn seed_trail_for_test(
        &mut self,
        cols: u16,
        lines: u16,
        col: u16,
        line: u16,
        value: f32,
    ) {
        let total = cols as usize * lines as usize;
        if self.trail_field.len() != total || self.trail_cols != cols || self.trail_lines != lines {
            self.trail_field.clear();
            self.trail_field.resize(total, 0.0);
            self.trail_cols = cols;
            self.trail_lines = lines;
        }
        let idx = col as usize * lines as usize + line as usize;
        if idx < self.trail_field.len() {
            self.trail_field[idx] = value;
        }
    }

    /// Direct particle write hook (mirrors flux's set_mote_for_test —
    /// builds targeted sensor/steering scenarios; repairs the active
    /// count bookkeeping after the direct writes).
    #[cfg(test)]
    pub(crate) fn set_particle_for_test(&mut self, idx: usize, x: f32, y: f32, heading: f32) {
        let p = &mut self.particles[idx];
        p.active = true;
        p.x = x;
        p.y = y;
        p.heading = heading;
        p.pace = 1.0;
        p.sim_age = 0.0;
        p.lifetime = 1.0e9;
        p.ch = '0';
        p.last_col = -1;
        p.last_line = -1;
        p.first_frame = true;
        p.palette_slot = 0;
        self.active_count = self.particles.iter().filter(|pp| pp.active).count();
    }
}
