// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The murmuration flock state machine (NIGHT-research-7, the
//! twelfth style) — the orchestration pass set: pool, spawn,
//! advance, draw.
//!
//! The style is the Reynolds flock (see mod.rs for the full
//! derivation): a pool of birds (viewport-derived,
//! density-dialed), a staggered entry through the spawn
//! accumulator (the family contract — birds fly in from the
//! edges, the flock assembles over the first seconds), and one
//! advance pass per frame: the spatial hash rebuild, then the
//! force accumulation + integration per bird (the triad, the
//! anchor, the banking, the jitter), the breathing oscillator
//! (law 4), the anchor's walk (law 3) and the startle clock
//! (law 5). The draw pass (draw.rs) renders the birds + comet
//! trails + the predator flash through the monolith drawn-cell
//! diff cleanup.
//!
//! Birds are immortal after activation (a murmuration does not
//! churn members): the accumulator idles once the flock is at its
//! target, and the pool only turns over on viewport resets and
//! style transitions.

use std::time::{Duration, Instant};

use rand::distr::Distribution;

use crate::constants::{
    MURM_BREATH_AMP, MURM_BREATH_BASE, MURM_BREATH_RATE, MURM_JITTER_W, MURM_MAX_BIRDS,
    MURM_MIN_BIRDS, MURM_PANIC_FLASH_SECS, MURM_SIM_TIME_PER_CPS, MURM_SPAWN_RATE_FLOOR,
    MURM_SPAWN_RATE_MULT, MURM_STARTLE_CLOCK_MEAN, SPAWN_REMAINDER_CAP,
};

use super::boids::{
    flock_forces, integrate_bird, startle, Anchor, Bird, BirdRandom, Edge, FlockCtx, MurmHash,
};

/// One drawn cell (col, line) — the diff-cleanup currency (same
/// shape as the family's cells).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MurmCell {
    pub(crate) col: u16,
    pub(crate) line: u16,
}

/// Spawn inputs (mirrors the family bundles — the accumulator
/// contract's parameter set).
pub(crate) struct MurmSpawnParams {
    pub(crate) cols: u16,
    pub(crate) lines: u16,
    /// The density dial's read for the pool sizing (the flock
    /// target — the pool is sized at reset from the scene's
    /// density; the spawn pass fills it to the brim).
    pub(crate) density: f32,
    pub(crate) active_palette_slot: u8,
    pub(crate) spawn_scale: f32,
}

/// Per-frame step inputs (mirrors `DnaStep`/`SolarFlareStep`).
pub(crate) struct MurmStep {
    pub(crate) now: Instant,
    /// chars_per_sec already multiplied by the terminal
    /// speed_mult. Scales the whole flock on one clock (dt_sim) —
    /// the family speed contract: the forces, the anchor, the
    /// breathing and the startle all survive the speed keys
    /// together.
    pub(crate) chars_per_sec: f32,
    pub(crate) cols: u16,
    pub(crate) lines: u16,
    pub(crate) max_sim_delta: Duration,
    pub(crate) resume_blend: f32,
}

/// The murmuration: the bird pool, the thought, the breathing
/// oscillator and the startle clock — one state machine.
#[derive(Debug)]
pub(crate) struct MurmurationRain {
    pub(crate) birds: Vec<Bird>,
    /// The flock's thought (the roaming anchor, law 3).
    pub(crate) anchor: Anchor,
    /// The breathing oscillator's phase (law 4, radians).
    breath_phase: f32,
    /// Sim-seconds until the next startle (law 5's clock).
    startle_clock: f32,
    /// The active predator's state (position + flash window);
    /// None while the sky is calm.
    predator: Option<(f32, f32, f32)>,
    active_count: usize,
    /// Rotating scan cursor for amortized O(1) free-slot search
    /// (mirrors the family).
    spawn_scan_idx: usize,
    /// The spatial hash (rebuilt each advance).
    hash: MurmHash,
    /// The per-frame neighbor scratch buffer (reused allocation).
    neighbor_scratch: Vec<u16>,
    /// Global motion clock (dt = now - last_step, clamped by
    /// max_sim_delta and eased by resume_blend — the family
    /// contract; a fully-paused run simply stops advancing).
    last_step: Option<Instant>,
    pub(super) current_cells: Vec<MurmCell>,
    pub(super) previous_cells: Vec<MurmCell>,
    pub(super) drawn_gen: Vec<u32>,
    pub(super) drawn_gen_counter: u32,
    /// Palette slot of the predator flash (one body, one slot).
    pub(super) field_palette_slot: u8,
    /// Total startles since reset (test-only dial).
    #[cfg(test)]
    startles_for_test: usize,
}

impl MurmurationRain {
    pub(crate) fn new() -> Self {
        Self {
            birds: Vec::new(),
            anchor: Anchor::new(),
            breath_phase: 0.0,
            startle_clock: MURM_STARTLE_CLOCK_MEAN,
            predator: None,
            active_count: 0,
            spawn_scan_idx: 0,
            hash: MurmHash::default(),
            neighbor_scratch: Vec::new(),
            last_step: None,
            current_cells: Vec::new(),
            previous_cells: Vec::new(),
            drawn_gen: Vec::new(),
            drawn_gen_counter: 0,
            field_palette_slot: 0,
            #[cfg(test)]
            startles_for_test: 0,
        }
    }

    /// The flock's steady-state population for a viewport + the
    /// density dial (the hero population: unlike the calm-sky
    /// rain styles, the flock IS the scene — the dial reads as
    /// the flock size, roughly 0.9 birds per column at 1.0,
    /// clamped to the legibility band).
    pub(crate) fn flock_target(cols: u16, density: f32) -> usize {
        let n = cols.max(1) as f32 * (0.35 + density.clamp(0.05, 3.0) * 0.9);
        (n.round() as usize).clamp(MURM_MIN_BIRDS, MURM_MAX_BIRDS)
    }

    /// Rebuild the pool for a new viewport (or style entry): the
    /// pool sized to the target, every slot vacant (the staggered
    /// entry refills it), the anchor recentered, the clocks
    /// re-armed.
    pub(crate) fn reset(&mut self, cols: u16, lines: u16) {
        let target = Self::flock_target(cols, 0.55);
        self.birds.clear();
        self.birds.resize_with(target, Bird::vacant);
        self.anchor.reset(cols, lines);
        self.breath_phase = 0.0;
        self.startle_clock = MURM_STARTLE_CLOCK_MEAN;
        self.predator = None;
        self.active_count = 0;
        self.spawn_scan_idx = 0;
        self.last_step = None;
        self.neighbor_scratch.clear();
        #[cfg(test)]
        {
            self.startles_for_test = 0;
        }
        self.clear_draw_history();
    }

    pub(crate) fn active_count(&self) -> usize {
        self.active_count
    }

    /// Palette transition completion: every active bird adopts
    /// the new slot (the family contract; the predator flash
    /// follows the field slot).
    pub(crate) fn adopt_palette_slot(&mut self, palette_slot: u8) {
        self.field_palette_slot = palette_slot;
        for b in &mut self.birds {
            if b.active {
                b.palette_slot = palette_slot;
            }
        }
    }

    /// Drop the diff-cleanup history (semantic invalidation /
    /// forced redraw). The next draw pass rebuilds it from an
    /// empty baseline. The flock state itself is simulation state
    /// and is preserved.
    pub(crate) fn clear_draw_history(&mut self) {
        self.current_cells.clear();
        self.previous_cells.clear();
        self.drawn_gen.clear();
        self.drawn_gen_counter = 0;
    }

    /// The breathing oscillator's current cohesion multiplier
    /// (law 4): base + amplitude on a slow sine — the flock's
    /// tighten/loosen cycle. Bounded by construction.
    pub(crate) fn breath_value(&self) -> f32 {
        MURM_BREATH_BASE + MURM_BREATH_AMP * self.breath_phase.sin()
    }

    /// Amortized free-slot scan (rotating cursor — mirrors the
    /// family).
    fn find_inactive_bird(&mut self) -> Option<usize> {
        let len = self.birds.len();
        if len == 0 {
            return None;
        }
        for step in 0..len {
            let idx = (self.spawn_scan_idx + step) % len;
            if !self.birds[idx].active {
                self.spawn_scan_idx = (idx + 1) % len;
                return Some(idx);
            }
        }
        None
    }

    /// Spawn pass — the staggered entry through the family's
    /// accumulator contract: the budget activates birds from the
    /// pool's vacant slots at the edges, flying inward. Once the
    /// flock is at its target the accumulator idles (birds are
    /// immortal — no turnover in steady state).
    pub(crate) fn spawn(
        &mut self,
        elapsed: Duration,
        spawn_remainder: &mut f32,
        params: &MurmSpawnParams,
        random: &mut BirdRandom<'_>,
    ) {
        if params.cols == 0 || params.lines == 0 {
            *spawn_remainder = 0.0;
            return;
        }

        // The live density dial: the flock target is a function of
        // the viewport and the dial — a mid-session density change
        // re-sizes the pool (growing appends vacant slots for the
        // staggered entry to fill, shrinking drops the inactive
        // tail first, then the excess actives — the diff cleanup
        // sweeps any dropped bird's cells on the next frame).
        let dial_target = Self::flock_target(params.cols, params.density);
        if self.birds.len() != dial_target {
            if dial_target < self.birds.len() {
                // Deactivate from the tail before shrinking so the
                // count stays honest.
                let excess = self.birds.len() - dial_target;
                let dropped_active = self
                    .birds
                    .iter()
                    .rev()
                    .take(excess)
                    .filter(|b| b.active)
                    .count();
                self.active_count = self.active_count.saturating_sub(dropped_active);
            }
            self.birds.resize(dial_target, Bird::vacant());
            *spawn_remainder = 0.0;
        }

        let target = self.birds.len();
        if target == 0 || self.active_count >= target {
            *spawn_remainder = (*spawn_remainder).min(SPAWN_REMAINDER_CAP);
            return;
        }

        let deficit = target - self.active_count;
        let spawn_rate =
            (target as f32 * MURM_SPAWN_RATE_MULT + MURM_SPAWN_RATE_FLOOR) * params.spawn_scale;
        let budget =
            elapsed.as_secs_f32() * spawn_rate + (*spawn_remainder).min(SPAWN_REMAINDER_CAP);
        if !budget.is_finite() || budget <= 0.0 {
            *spawn_remainder = 0.0;
            return;
        }

        let to_spawn = (budget.floor() as usize).min(deficit);
        *spawn_remainder = (budget - to_spawn as f32).min(SPAWN_REMAINDER_CAP);
        if to_spawn == 0 {
            return;
        }

        let cols_f = params.cols as f32;
        let lines_f = params.lines as f32;
        for _ in 0..to_spawn {
            let Some(idx) = self.find_inactive_bird() else {
                break;
            };
            let roll_a = random.rand_chance.sample(random.rng);
            let roll_b = random.rand_chance.sample(random.rng);
            let roll_c = random.rand_chance.sample(random.rng);
            // The entry edge: any of the four, weighted evenly.
            let edge = match (roll_a * 4.0).floor() as u32 {
                0 => Edge::Top,
                1 => Edge::Bottom,
                2 => Edge::Left,
                _ => Edge::Right,
            };
            let along = match edge {
                Edge::Top | Edge::Bottom => roll_b * (cols_f - 1.0),
                Edge::Left | Edge::Right => roll_b * (lines_f - 1.0),
            };
            let b = &mut self.birds[idx];
            b.activate_at_edge(edge, along, params.active_palette_slot, roll_c);
            // The glyph is picked at the first draw pass (the
            // lorenz contract).
            self.active_count += 1;
        }
    }

    /// Motion pass — the flock's physics core.
    ///
    /// 1. The clocks: the breathing oscillator advances (law 4),
    ///    the anchor walks (law 3), the startle clock counts
    ///    (law 5) — a fire re-rolls the predator position, kicks
    ///    every bird inside the panic radius and arms the flash.
    /// 2. The hash rebuild (law 2) over the active set.
    /// 3. Per bird: the force accumulation (the triad + anchor +
    ///    banking, the breathing-scaled cohesion) + the jitter +
    ///    the integration with the speed clamps.
    pub(crate) fn advance(&mut self, step: &MurmStep, random: &mut BirdRandom<'_>) {
        if self.active_count == 0 && self.birds.is_empty() {
            self.last_step = Some(step.now);
            return;
        }
        let dt_wall = match self.last_step {
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
        if dt_wall <= 0.0 {
            return;
        }

        // The family speed contract: one sim clock for the flock,
        // the thought, the breathing and the startle alike.
        let dt_sim = dt_wall * step.chars_per_sec.max(0.0) * MURM_SIM_TIME_PER_CPS;

        // 1. The clocks.
        self.breath_phase += MURM_BREATH_RATE * dt_sim;
        if self.breath_phase > std::f32::consts::TAU * 64.0 {
            self.breath_phase = self.breath_phase.rem_euclid(std::f32::consts::TAU);
        }
        self.anchor.advance(dt_sim, step.cols, step.lines, random);

        // The panic window ages (the speed floor's release).
        for b in &mut self.birds {
            if b.active && b.panic_age < f32::MAX {
                b.panic_age += dt_sim;
            }
            if b.active {
                b.age += dt_sim;
            }
        }

        // The predator flash window.
        if let Some((_, _, flash)) = &mut self.predator {
            *flash += dt_sim;
            if *flash >= MURM_PANIC_FLASH_SECS {
                self.predator = None;
            }
        }

        // The startle clock (law 5): a fire re-rolls the predator
        // position and kicks the flock.
        if self.active_count > 0 {
            self.startle_clock -= dt_sim;
            if self.startle_clock <= 0.0 {
                self.startle_clock =
                    MURM_STARTLE_CLOCK_MEAN * (0.6 + random.rand_chance.sample(random.rng) * 0.8);
                let roll_a = random.rand_chance.sample(random.rng);
                let roll_b = random.rand_chance.sample(random.rng);
                let px = roll_a * (step.cols.max(1) as f32 - 1.0);
                let py = roll_b * (step.lines.max(1) as f32 - 1.0);
                for b in &mut self.birds {
                    if b.active {
                        startle(b, px, py);
                    }
                }
                self.predator = Some((px, py, 0.0));
                #[cfg(test)]
                {
                    self.startles_for_test += 1;
                }
            }
        }

        if self.active_count == 0 {
            return;
        }

        // 2. The hash rebuild (law 2).
        self.hash.rebuild(&self.birds, step.cols, step.lines);

        // 3. The force pass: accumulate (read-only over the
        //    flock) then integrate (the write). Split borrows via
        //    the snapshot of the anchor + breathing value.
        let anchor = self.anchor;
        let coh_weight = self.breath_value();
        let cols = step.cols;
        let lines = step.lines;

        // The accumulation snapshot: forces depend on the CURRENT
        // state (synchronous update — the whole flock steps on
        // the same frame, the classic boids contract).
        let mut accels: Vec<(f32, f32)> = vec![(0.0, 0.0); self.birds.len()];
        let ctx = FlockCtx {
            birds: &self.birds,
            anchor: &anchor,
            coh_weight,
            cols,
            lines,
        };
        for (i, b) in self.birds.iter().enumerate() {
            if !b.active {
                continue;
            }
            self.hash.neighbors(b.x, b.y, &mut self.neighbor_scratch);
            let (mut ax, mut ay) = flock_forces(b, &self.neighbor_scratch, i as u16, &ctx);
            // The jitter: the clamped random walk (the organic
            // wobble — the caller holds the RNG).
            let jroll_a = random.rand_chance.sample(random.rng);
            let jroll_b = random.rand_chance.sample(random.rng);
            ax += (jroll_a - 0.5) * 2.0 * MURM_JITTER_W;
            ay += (jroll_b - 0.5) * 2.0 * MURM_JITTER_W;
            accels[i] = (ax, ay);
        }

        // The integration pass (the write half).
        for (i, b) in self.birds.iter_mut().enumerate() {
            if !b.active {
                continue;
            }
            let (ax, ay) = accels[i];
            integrate_bird(b, ax, ay, dt_sim, cols, lines);
        }
    }

    /// The active predator's draw state (position + flash age),
    /// if any (the draw pass renders the flash glyph).
    pub(crate) fn predator(&self) -> Option<(f32, f32, f32)> {
        self.predator
    }

    // -- Test-only diagnostics (mirrors the family *_for_test API) --

    #[cfg(test)]
    pub(crate) fn bird_states_for_test(&self) -> Vec<(f32, f32, f32, f32)> {
        self.birds
            .iter()
            .filter(|b| b.active)
            .map(|b| (b.x, b.y, b.vx, b.vy))
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn startles_for_test(&self) -> usize {
        self.startles_for_test
    }

    #[cfg(test)]
    pub(crate) fn predator_for_test(&self) -> Option<(f32, f32, f32)> {
        self.predator
    }

    /// Plant the startle clock (the startle tests' deterministic
    /// arm).
    #[cfg(test)]
    pub(crate) fn arm_startle_for_test(&mut self, clock: f32) {
        self.startle_clock = clock;
    }

    #[cfg(test)]
    pub(crate) fn breath_for_test(&self) -> f32 {
        self.breath_value()
    }

    #[cfg(test)]
    pub(crate) fn drawn_cells_for_test(&self) -> &[MurmCell] {
        &self.current_cells
    }
}
