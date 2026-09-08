// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The solar flare rain state machine (NIGHT-special-4, the tenth
//! style) — the orchestration pass set: pool, spawn, advance, draw.
//!
//! The style composes the two halves of the corona (see mod.rs for
//! the full derivation): `CoronaArcade` (the magnetic carpet, the
//! flux ladder and the granulation surface) and `SolarDrop` (the
//! coronal rain). This struct owns the pool lifecycle and the
//! per-frame passes, following the structured family contract
//! (lorenz/vortex/dragon/physarum/black_hole/aeolian):
//!
//! - pool: one drop per column (the family lane model),
//! - spawn: the deficit-bounded fractional accumulator, with the
//!   condensation target chosen by a hot-loop tournament (law 2's
//!   concentration arm — the arcade members that have been rained on
//!   collect the next condensations),
//! - advance: one global clock (dt clamped by `max_sim_delta`,
//!   eased by `resume_blend`, scaled to sim-time by
//!   chars_per_sec), star first then drops (the eruption return
//!   converts riders to ejecta between the two),
//! - draw: the surface first (granulation + footpoint glow), then
//!   the arcs, then the rain (riding heads + comet trails + ejecta),
//!   all through the monolith drawn-cell diff cleanup
//!   (generation-tagged).
//!
//! The advance pass is a stochastic pass in the family sense: the
//! wind re-rolls, the breath re-anchors, the flare clock re-rolls,
//! the granulation walk and the spawn tournament all ride the RNG
//! bundle.

use std::time::{Duration, Instant};

use rand::distr::Distribution;

use crate::constants::{
    SOLAR_ACTIVE_BASE, SOLAR_ACTIVE_DENSITY_MULT, SOLAR_ACTIVE_MAX, SOLAR_DEPOSIT_RATE,
    SOLAR_DEPOSIT_SEED, SOLAR_EJECTA_BURST, SOLAR_EJECTA_FLING, SOLAR_EJECTA_G, SOLAR_EJECTA_LIFE,
    SOLAR_EJECTA_RISE, SOLAR_EJECTA_SPEED_MAX, SOLAR_EJECTA_SPEED_MIN, SOLAR_EJECTA_SPREAD,
    SOLAR_FLASH_SECS, SOLAR_LEG_G, SOLAR_MAX_AGE_SECS, SOLAR_RAIN_SPAN, SOLAR_RAIN_V0,
    SOLAR_SIM_TIME_PER_CPS, SOLAR_SPAWN_RATE_FLOOR, SOLAR_SPAWN_RATE_MULT, SOLAR_SPLASH_HEAT,
    SOLAR_TOURNAMENT, SPAWN_REMAINDER_CAP,
};

use super::drops::{DropMode, Leg, SolarDrop};
use super::loops::{CoronaArcade, SolarRandom};

/// One drawn cell (col, line) — the diff-cleanup currency (same
/// shape as `AuroraCell`/`AeolianCell`/`LorenzCell`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SolarCell {
    pub(crate) col: u16,
    pub(crate) line: u16,
}

/// Spawn inputs (mirrors `AuroraSpawnParams` — the bundle keeps
/// clippy's `too_many_arguments` threshold respected).
pub(crate) struct SolarFlareSpawnParams {
    pub(crate) cols: u16,
    pub(crate) lines: u16,
    pub(crate) density: f32,
    pub(crate) active_palette_slot: u8,
    pub(crate) spawn_scale: f32,
}

/// Per-frame step inputs (mirrors `AuroraStep`). Carries viewport
/// geometry — the drop physics needs bounds for the surface landing
/// and the ejecta walls.
pub(crate) struct SolarFlareStep {
    pub(crate) now: Instant,
    /// chars_per_sec already multiplied by the terminal speed_mult.
    /// Scales the whole corona on one clock (dt_sim) — the family
    /// speed contract: trajectory shapes survive the speed keys.
    pub(crate) chars_per_sec: f32,
    pub(crate) cols: u16,
    pub(crate) max_sim_delta: Duration,
    pub(crate) resume_blend: f32,
}

/// The solar flare rain: the corona arcade plus the drop pool, one
/// state machine.
#[derive(Debug)]
pub(crate) struct SolarFlareRain {
    pub(crate) drops: Vec<SolarDrop>,
    /// The star (the invented arcade physics — see loops.rs).
    pub(crate) arcade: CoronaArcade,
    active_count: usize,
    /// Rotating scan cursor for amortized O(1) free-slot search
    /// (mirrors `VortexRain::spawn_scan_idx`).
    spawn_scan_idx: usize,
    /// Global motion clock (dt = now - last_step, clamped by
    /// max_sim_delta and eased by resume_blend — the family
    /// contract; a fully-paused run simply stops advancing).
    last_step: Option<Instant>,
    /// Palette slot of the arcade (one body, one slot — adopted at
    /// palette-transition completion, mirroring the aeolian field).
    pub(super) field_palette_slot: u8,
    pub(super) current_cells: Vec<SolarCell>,
    pub(super) previous_cells: Vec<SolarCell>,
    pub(super) drawn_gen: Vec<u32>,
    pub(super) drawn_gen_counter: u32,
    /// Total landed drops since reset (test-only speed dial).
    #[cfg(test)]
    landings_for_test: usize,
}

impl SolarFlareRain {
    pub(crate) fn new() -> Self {
        Self {
            drops: Vec::new(),
            arcade: CoronaArcade::new(),
            active_count: 0,
            spawn_scan_idx: 0,
            last_step: None,
            field_palette_slot: 0,
            current_cells: Vec::new(),
            previous_cells: Vec::new(),
            drawn_gen: Vec::new(),
            drawn_gen_counter: 0,
            #[cfg(test)]
            landings_for_test: 0,
        }
    }

    /// Rebuild the pool + arcade for a new viewport (or style
    /// entry). The pool is one drop per column; the arcade is
    /// re-counted and re-spread for the new width, every loop
    /// Emerging (a fresh star grows its corona).
    pub(crate) fn reset(&mut self, cols: u16, lines: u16) {
        let lanes = cols.max(1) as usize;
        self.drops.clear();
        self.drops.resize_with(lanes, SolarDrop::vacant);
        self.arcade.reset(cols, lines);
        self.active_count = 0;
        self.spawn_scan_idx = 0;
        self.last_step = None;
        #[cfg(test)]
        {
            self.landings_for_test = 0;
        }
        self.clear_draw_history();
    }

    pub(crate) fn active_count(&self) -> usize {
        self.active_count
    }

    /// Palette transition completion: the arcade adopts the new slot
    /// (one body, one slot), and every active drop adopts it
    /// individually (mirrors the family contract).
    pub(crate) fn adopt_palette_slot(&mut self, palette_slot: u8) {
        self.field_palette_slot = palette_slot;
        for d in &mut self.drops {
            if d.active {
                d.palette_slot = palette_slot;
            }
        }
    }

    /// Drop the diff-cleanup history (semantic invalidation / forced
    /// redraw). The next draw pass rebuilds it from an empty
    /// baseline. The arcade itself is simulation state and is
    /// preserved (wiping it would erase a painted corona).
    pub(crate) fn clear_draw_history(&mut self) {
        self.current_cells.clear();
        self.previous_cells.clear();
        self.drawn_gen.clear();
        self.drawn_gen_counter = 0;
    }

    /// Steady-state active-drop target from pool size + density
    /// (mirrors `AuroraRain::target_active_count`): the calm-sky
    /// dial family — a sparse ambient coronal rain, never a
    /// downpour (the stage-4 DNA).
    fn target_active_count(lanes: usize, density: f32) -> usize {
        if lanes == 0 {
            return 0;
        }
        let ratio = (SOLAR_ACTIVE_BASE + density.clamp(0.01, 5.0) * SOLAR_ACTIVE_DENSITY_MULT)
            .clamp(0.02, SOLAR_ACTIVE_MAX);
        ((lanes as f32 * ratio).round() as usize).clamp(1, lanes)
    }

    /// Amortized free-slot scan (rotating cursor — mirrors the
    /// family).
    fn find_inactive_drop(&mut self) -> Option<usize> {
        let len = self.drops.len();
        if len == 0 {
            return None;
        }
        for step in 0..len {
            let idx = (self.spawn_scan_idx + step) % len;
            if !self.drops[idx].active {
                self.spawn_scan_idx = (idx + 1) % len;
                return Some(idx);
            }
        }
        None
    }

    /// Law 2's concentration arm: the hot-loop tournament. Samples
    /// SOLAR_TOURNAMENT candidates among the rainable loops (Stable
    /// or Emerging) and returns the flux-richest — the arcade
    /// members that have been rained on collect the next
    /// condensations (the self-organization loop, the aurora
    /// funnel's arcade heir). A league of 4 over an 8-loop arcade
    /// concentrates the weather without starving the rest.
    fn pick_rain_loop(&self, random: &mut SolarRandom<'_>) -> Option<usize> {
        let len = self.arcade.loops().len();
        if len == 0 {
            return None;
        }
        let mut best: Option<usize> = None;
        let mut best_flux = f32::NEG_INFINITY;
        for _ in 0..SOLAR_TOURNAMENT {
            let roll = random.rand_chance.sample(random.rng);
            let idx = ((roll * len as f32) as usize).min(len - 1);
            let lp = &self.arcade.loops()[idx];
            if !matches!(
                lp.phase,
                super::loops::LoopPhase::Stable | super::loops::LoopPhase::Emerging
            ) {
                continue;
            }
            // A fresh flash breaks ties: the footpoint that just
            // absorbed a drop reads hotter than its flux alone.
            let hot = lp.flux
                + if lp.flare_age < SOLAR_FLASH_SECS {
                    0.3
                } else {
                    0.0
                };
            if hot > best_flux {
                best_flux = hot;
                best = Some(idx);
            }
        }
        best
    }

    /// Spawn pass — the accumulator contract identical to the
    /// structured family (deficit-bounded budget + fractional
    /// remainder carry; the equilibrium is a trickle).
    pub(crate) fn spawn(
        &mut self,
        elapsed: Duration,
        spawn_remainder: &mut f32,
        params: &SolarFlareSpawnParams,
        random: &mut SolarRandom<'_>,
    ) {
        if params.cols == 0 || params.lines == 0 || self.drops.is_empty() {
            *spawn_remainder = 0.0;
            return;
        }

        let target = Self::target_active_count(self.drops.len(), params.density);
        if self.active_count >= target {
            *spawn_remainder = (*spawn_remainder).min(SPAWN_REMAINDER_CAP);
            return;
        }

        let deficit = target - self.active_count;
        let spawn_rate =
            (target as f32 * SOLAR_SPAWN_RATE_MULT + SOLAR_SPAWN_RATE_FLOOR) * params.spawn_scale;
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

        let mut spawned = 0usize;
        for _ in 0..to_spawn {
            // The condensation target: a hot loop (tournament). With
            // no rainable loop (the star is mid-flare), the spawn
            // quietly waits — the spent budget simply expires.
            let Some(li) = self.pick_rain_loop(random) else {
                break;
            };
            let Some(idx) = self.find_inactive_drop() else {
                break;
            };
            let s = 0.5 + (random.rand_chance.sample(random.rng) - 0.5) * SOLAR_RAIN_SPAN;
            let leg = if random.rand_chance.sample(random.rng) < 0.5 {
                Leg::Left
            } else {
                Leg::Right
            };
            let lifetime =
                SOLAR_MAX_AGE_SECS * (0.85 + random.rand_chance.sample(random.rng) * 0.30);
            let d = &mut self.drops[idx];
            d.active = true;
            d.seed_rider(li, s, leg, lifetime);
            d.palette_slot = params.active_palette_slot;
            // The position (x, y) and the glyph are picked at the
            // first advance/draw pass (the lorenz contract).
            spawned += 1;
        }
        self.active_count += spawned;
    }

    /// Motion pass — the corona's physics core (laws 2-4 live here).
    ///
    /// 1. The arcade takes one breath (laws 1, 3a, 4 in
    ///    `CoronaArcade::advance`); a fresh eruption returns the
    ///    erupting loop index.
    /// 2. The eruption arm: riders on the erupting loop are flung
    ///    as ejecta (the tangent fling) and a burst spawns at the
    ///    apex (law 4).
    /// 3. Each drop: riding integration (the closed-form
    ///    energy-conserving speed + the metric), the landing state
    ///    test (law 3 — s motion is monotone, no tunneling at any
    ///    dt), or the ejecta ballistics (gravity, walls, the
    ///    surface splash).
    pub(crate) fn advance(&mut self, step: &SolarFlareStep, random: &mut SolarRandom<'_>) {
        let has_star = !self.arcade.loops().is_empty();
        if self.active_count == 0 && !has_star {
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

        // The family speed contract: one sim clock for rain, arcade
        // and flux alike (shapes invariant under the speed keys).
        let dt_sim = dt_wall * step.chars_per_sec.max(0.0) * SOLAR_SIM_TIME_PER_CPS;

        // 1. The star breathes (laws 1, 3a, 4).
        let erupted = self.arcade.advance(dt_sim, random);

        // 2. The eruption arm (law 4): fling the riders, burst at
        //    the apex.
        if let Some(ei) = erupted {
            let (apex_x, apex_y) = {
                let lp = &self.arcade.loops()[ei];
                let surface_top = self.arcade.surface_top() as f32;
                (lp.arc_x(0.5), surface_top - lp.arc_height(0.5))
            };
            for d in &mut self.drops {
                if !d.active {
                    continue;
                }
                if let DropMode::Riding { loop_idx, s, leg } = d.mode {
                    if loop_idx != ei {
                        continue;
                    }
                    // The tangent fling: the rider's along-arc motion
                    // becomes a ballistic velocity, plus an upward
                    // kick (the flare throws plasma off the star).
                    let lp = &self.arcade.loops()[ei];
                    let (tx, ty) = lp.arc_tangent(s);
                    let dir = leg.sign();
                    let spread = (random.rand_chance.sample(random.rng) - 0.5) * 2.0;
                    let speed = d.v * SOLAR_EJECTA_FLING;
                    let x = lp.arc_x(s);
                    let y = self.arcade.surface_top() as f32 - lp.arc_height(s);
                    d.seed_ejecta(
                        x,
                        y,
                        tx * dir * speed + spread * SOLAR_EJECTA_SPREAD,
                        ty * dir * speed - SOLAR_EJECTA_RISE,
                        SOLAR_EJECTA_LIFE * (0.75 + random.rand_chance.sample(random.rng) * 0.5),
                    );
                }
            }
            // The apex burst (law 4's spray): fresh ejecta above the
            // stretched loop top, fanning outward and upward.
            for _ in 0..SOLAR_EJECTA_BURST {
                let Some(idx) = self.find_inactive_drop() else {
                    break;
                };
                let roll = random.rand_chance.sample(random.rng);
                let speed = SOLAR_EJECTA_SPEED_MIN
                    + roll * (SOLAR_EJECTA_SPEED_MAX - SOLAR_EJECTA_SPEED_MIN);
                let vx = (random.rand_chance.sample(random.rng) - 0.5) * 2.0 * SOLAR_EJECTA_SPREAD;
                let d = &mut self.drops[idx];
                d.active = true;
                d.seed_ejecta(
                    apex_x,
                    apex_y,
                    vx,
                    -speed,
                    SOLAR_EJECTA_LIFE * (0.75 + random.rand_chance.sample(random.rng) * 0.5),
                );
                d.palette_slot = self.field_palette_slot;
                self.active_count += 1;
            }
        }

        if self.active_count == 0 {
            return;
        }

        let cols_f = step.cols.max(1) as f32;
        let surface_top = self.arcade.surface_top() as f32;

        // 3. The drop physics. Landings and splashes are collected
        //    and applied after the loop (the arcade needs the &mut
        //    while the drop loop holds the pool).
        let mut landings: Vec<(usize, f32)> = Vec::new();
        let mut splashes: Vec<(usize, f32)> = Vec::new();
        let mut died = 0usize;

        {
            let arcade = &self.arcade;
            let loops = arcade.loops();
            for d in &mut self.drops {
                if !d.active {
                    continue;
                }
                d.sim_age += dt_sim;
                match d.mode {
                    DropMode::Riding { loop_idx, s, leg } => {
                        let Some(lp) = loops.get(loop_idx) else {
                            // The loop vanished (viewport rebuild
                            // races a straggler): retire the rider.
                            d.active = false;
                            d.clear_trail();
                            died += 1;
                            continue;
                        };
                        // Law 2 — the closed-form energy-conserving
                        // speed: v = sqrt(v0^2 + 2 G h (2|s-0.5|)^2)
                        // (bounded by construction; exact at any dt).
                        let dev = 2.0 * (s - 0.5).abs();
                        d.v = (SOLAR_RAIN_V0 * SOLAR_RAIN_V0
                            + 2.0 * SOLAR_LEG_G * lp.h * dev * dev)
                            .sqrt();
                        // The metric converts v to parameter motion;
                        // s is strictly monotone toward the foot.
                        let s_new = s + leg.sign() * d.v / lp.arc_metric(s) * dt_sim;
                        // Law 3 — the landing state test (monotone:
                        // v >= v0 > 0; no tunneling at any dt).
                        let landed = match leg {
                            Leg::Left => s_new <= 0.0,
                            Leg::Right => s_new >= 1.0,
                        };
                        if landed {
                            let charge = d.v * SOLAR_DEPOSIT_RATE + SOLAR_DEPOSIT_SEED;
                            landings.push((loop_idx, charge));
                            d.active = false;
                            d.clear_trail();
                            died += 1;
                            continue;
                        }
                        d.mode = DropMode::Riding {
                            loop_idx,
                            s: s_new,
                            leg,
                        };
                        d.x = lp.arc_x(s_new);
                        d.y = surface_top - lp.arc_height(s_new);
                    }
                    DropMode::Ejecta { age } => {
                        // Law 4's ballistics: stellar gravity
                        // decelerates the rise; age fades the spark.
                        d.x += d.vx * dt_sim;
                        d.y += d.vy * dt_sim;
                        d.vy += SOLAR_EJECTA_G * dt_sim;
                        d.mode = DropMode::Ejecta { age: age + dt_sim };
                        // The splash: an ejecta falling back onto the
                        // photosphere heats the granule it hits.
                        if d.y >= surface_top {
                            splashes.push((
                                d.x.round().clamp(0.0, cols_f - 1.0) as usize,
                                SOLAR_SPLASH_HEAT,
                            ));
                            d.active = false;
                            d.clear_trail();
                            died += 1;
                            continue;
                        }
                        // The walls and the sky's edge retire the
                        // spark; the lifetime backstop sweeps the
                        // rest.
                        if d.x < 0.0 || d.x > cols_f - 1.0 || d.y < -2.0 {
                            d.active = false;
                            d.clear_trail();
                            died += 1;
                            continue;
                        }
                    }
                }
                if d.active && d.sim_age >= d.lifetime {
                    d.active = false;
                    d.clear_trail();
                    died += 1;
                }
            }
        }

        // Apply the landings (law 3) and the splashes (law 5's
        // surface arm).
        for (li, charge) in landings {
            self.arcade.absorb(li, charge);
            #[cfg(test)]
            {
                self.landings_for_test += 1;
            }
        }
        for (col, heat) in splashes {
            self.arcade.splash(col, heat);
        }
        if died > 0 {
            self.active_count = self.active_count.saturating_sub(died);
        }
    }

    // -- Test-only diagnostics (mirrors the family *_for_test API) --

    #[cfg(test)]
    pub(crate) fn drop_states_for_test(&self) -> Vec<(f32, f32, f32, f32)> {
        self.drops
            .iter()
            .filter(|d| d.active)
            .map(|d| {
                (
                    d.x,
                    d.y,
                    d.v,
                    match d.mode {
                        DropMode::Riding { .. } => d.v,
                        DropMode::Ejecta { .. } => d.vy,
                    },
                )
            })
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn rider_speeds_for_test(&self) -> Vec<(f32, f32)> {
        self.drops
            .iter()
            .filter(|d| d.active)
            .filter_map(|d| match d.mode {
                DropMode::Riding { s, .. } => Some((s, d.v)),
                DropMode::Ejecta { .. } => None,
            })
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn ejecta_count_for_test(&self) -> usize {
        self.drops
            .iter()
            .filter(|d| d.active && matches!(d.mode, DropMode::Ejecta { .. }))
            .count()
    }

    #[cfg(test)]
    pub(crate) fn landings_for_test(&self) -> usize {
        self.landings_for_test
    }

    #[cfg(test)]
    pub(crate) fn arcade_for_test(&self) -> &CoronaArcade {
        &self.arcade
    }

    #[cfg(test)]
    pub(crate) fn arcade_mut_for_test(&mut self) -> &mut CoronaArcade {
        &mut self.arcade
    }

    #[cfg(test)]
    pub(crate) fn drawn_cells_for_test(&self) -> &[SolarCell] {
        &self.current_cells
    }
}
