// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The aurora rain state machine (NIGHT-special-3, the tenth
//! style) — the orchestration pass set: pool, spawn, advance, draw.
//!
//! The style composes the two halves of the veil (see mod.rs for
//! the full derivation): `AuroraSky` (the ray lattice) and
//! `AuroraDrop` (the precipitation). This struct owns the pool
//! lifecycle and the per-frame passes, following the structured
//! family contract (lorenz/vortex/dragon/physarum/black_hole/
//! aeolian):
//!
//! - pool: one drop per column (the family lane model),
//! - spawn: the deficit-bounded fractional accumulator,
//! - advance: one global clock (dt clamped by `max_sim_delta`,
//!   eased by `resume_blend`, scaled to sim-time by
//!   chars_per_sec), sky first then drops,
//! - draw: veil cells first (fabric + fringe + flank spill), then
//!   the rain (head + comet trail), all through the monolith
//!   drawn-cell diff cleanup (generation-tagged).
//!
//! The advance pass is the family's second stochastic pass (the
//! aeolian capture coin was the first): the wind re-rolls, the
//! anchor flips and the dwell re-rolls ride the RNG bundle.

use std::time::{Duration, Instant};

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::frame::Frame;

use crate::constants::{
    AURORA_ACTIVE_BASE, AURORA_ACTIVE_DENSITY_MULT, AURORA_ACTIVE_MAX, AURORA_BODY_FACTOR_FRINGE,
    AURORA_BODY_FACTOR_TOP, AURORA_BODY_TAU, AURORA_CHARGE_RATE, AURORA_CORONA_DEPTH,
    AURORA_DROP_GRAVITY, AURORA_DROP_TERMINAL, AURORA_FRINGE_BOOST, AURORA_GLOW_LEVEL_HOT,
    AURORA_MAX_AGE_SECS, AURORA_SEEK_DRAG, AURORA_SEEK_GAIN, AURORA_SEEK_RANGE,
    AURORA_SHIMMER_BODY, AURORA_SHIMMER_FRINGE, AURORA_SIM_TIME_PER_CPS, AURORA_SPAWN_RATE_FLOOR,
    AURORA_SPAWN_RATE_MULT, SPAWN_REMAINDER_CAP,
};

use super::super::super::render::DrawCtx;
use super::super::monolith::monolith_helpers::{
    bold_for_level, clear_cell, color_for_level, pick_pool_char,
};
use super::super::monolith::{BrightnessLevel, MonolithCleanup};

use super::drops::AuroraDrop;
use super::rays::{fringe_level, AuroraRandom, AuroraSky};

/// One drawn cell (col, line) — the diff-cleanup currency (same
/// shape as `AeolianCell`/`LorenzCell`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AuroraCell {
    pub(crate) col: u16,
    pub(crate) line: u16,
}

/// Spawn inputs (mirrors `AeolianSpawnParams` — the bundle keeps
/// clippy's `too_many_arguments` threshold respected).
pub(crate) struct AuroraSpawnParams {
    pub(crate) cols: u16,
    pub(crate) lines: u16,
    pub(crate) density: f32,
    pub(crate) active_palette_slot: u8,
    pub(crate) spawn_scale: f32,
}

/// Per-frame step inputs (mirrors `AeolianStep`). Carries viewport
/// geometry — the drop physics needs bounds for the wall clamp and
/// the ground absorption.
pub(crate) struct AuroraStep {
    pub(crate) now: Instant,
    /// chars_per_sec already multiplied by the terminal speed_mult.
    /// Scales the whole veil on one clock (dt_sim) — the family
    /// speed contract: trajectory shapes survive the speed keys.
    pub(crate) chars_per_sec: f32,
    pub(crate) cols: u16,
    pub(crate) lines: u16,
    pub(crate) max_sim_delta: Duration,
    pub(crate) resume_blend: f32,
}

/// The aurora rain: the ray lattice plus the drop pool, one state
/// machine.
#[derive(Debug)]
pub(crate) struct AuroraRain {
    pub(crate) drops: Vec<AuroraDrop>,
    /// The sky (the invented veil physics — see rays.rs).
    pub(crate) sky: AuroraSky,
    active_count: usize,
    /// Rotating scan cursor for amortized O(1) free-slot search
    /// (mirrors `VortexRain::spawn_scan_idx`).
    spawn_scan_idx: usize,
    /// Global motion clock (dt = now - last_step, clamped by
    /// max_sim_delta and eased by resume_blend — the family
    /// contract; a fully-paused run simply stops advancing).
    last_step: Option<Instant>,
    /// Palette slot of the veil (one body, one slot — adopted at
    /// palette-transition completion, mirroring the aeolian field).
    field_palette_slot: u8,
    current_cells: Vec<AuroraCell>,
    previous_cells: Vec<AuroraCell>,
    drawn_gen: Vec<u32>,
    drawn_gen_counter: u32,
}

impl AuroraRain {
    pub(crate) fn new() -> Self {
        Self {
            drops: Vec::new(),
            sky: AuroraSky::new(),
            active_count: 0,
            spawn_scan_idx: 0,
            last_step: None,
            field_palette_slot: 0,
            current_cells: Vec::new(),
            previous_cells: Vec::new(),
            drawn_gen: Vec::new(),
            drawn_gen_counter: 0,
        }
    }

    /// Rebuild the pool + lattice for a new viewport (or style
    /// entry). The pool is one drop per column; the lattice is
    /// re-counted and re-spread for the new width, all fringes
    /// quiet (a fresh sky waits to be painted).
    pub(crate) fn reset(&mut self, cols: u16, lines: u16) {
        let lanes = cols.max(1) as usize;
        self.drops.clear();
        self.drops.resize_with(lanes, AuroraDrop::vacant);
        self.sky.reset(cols, lines);
        self.active_count = 0;
        self.spawn_scan_idx = 0;
        self.last_step = None;
        self.clear_draw_history();
    }

    pub(crate) fn active_count(&self) -> usize {
        self.active_count
    }

    /// Palette transition completion: the veil adopts the new slot
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
    /// baseline. The ray lattice itself is simulation state and is
    /// preserved (wiping it would erase a painted sky).
    pub(crate) fn clear_draw_history(&mut self) {
        self.current_cells.clear();
        self.previous_cells.clear();
        self.drawn_gen.clear();
        self.drawn_gen_counter = 0;
    }

    /// Steady-state active-drop target from pool size + density
    /// (mirrors `AeolianRain::target_active_count`): the calm-sky
    /// dial family — a sparse ambient precipitation, never a
    /// downpour (the stage-4 DNA).
    fn target_active_count(lanes: usize, density: f32) -> usize {
        if lanes == 0 {
            return 0;
        }
        let ratio = (AURORA_ACTIVE_BASE + density.clamp(0.01, 5.0) * AURORA_ACTIVE_DENSITY_MULT)
            .clamp(0.02, AURORA_ACTIVE_MAX);
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

    /// Spawn pass — the accumulator contract identical to the
    /// structured family (deficit-bounded budget + fractional
    /// remainder carry; the equilibrium is a trickle).
    pub(crate) fn spawn(
        &mut self,
        elapsed: Duration,
        spawn_remainder: &mut f32,
        params: &AuroraSpawnParams,
        random: &mut AuroraRandom<'_>,
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
            (target as f32 * AURORA_SPAWN_RATE_MULT + AURORA_SPAWN_RATE_FLOOR) * params.spawn_scale;
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

        for _ in 0..to_spawn {
            let Some(idx) = self.find_inactive_drop() else {
                break;
            };
            self.activate_drop(idx, params.active_palette_slot, random);
            self.active_count += 1;
        }
    }

    /// Activate a vacant drop at its lane column, top edge, with
    /// the calm-entry seed (slow Ghost fall, slight lateral drift,
    /// charge floor) and the family's stagger variance.
    fn activate_drop(&mut self, idx: usize, palette_slot: u8, random: &mut AuroraRandom<'_>) {
        let drift_roll = (random.rand_chance.sample(random.rng) - 0.5) * 2.0;
        let pace = 0.85 + random.rand_chance.sample(random.rng) * 0.30;
        let lifetime = AURORA_MAX_AGE_SECS * (0.85 + random.rand_chance.sample(random.rng) * 0.30);

        let d = &mut self.drops[idx];
        d.active = true;
        d.x = idx as f32;
        d.y = 0.0;
        d.seed_motion(drift_roll, pace, lifetime);
        d.palette_slot = palette_slot;
        d.clear_trail();
        // The glyph is picked at the first draw pass (trail_len == 0
        // -> fresh pool pick, the lorenz contract).
    }

    /// Motion pass — the veil's physics core (laws 3-4 live here).
    ///
    /// 1. The ray lattice takes one breath (laws 1-2, 4a in
    ///    `AuroraSky::advance`).
    /// 2. Each drop: glow-weighted funnel seeking toward the
    ///    nearest ray within SEEK_RANGE of its depth (law 3),
    ///    lateral drag, position integration, gravity (capped
    ///    terminal), then the absorption state test against the
    ///    nearest ray's depth (law 4 — y only grows, depth is
    ///    bounded, no tunneling at any dt), and the ground
    ///    absorption at the bottom edge.
    pub(crate) fn advance(&mut self, step: &AuroraStep, random: &mut AuroraRandom<'_>) {
        let has_sky = !self.sky.rays().is_empty();
        if self.active_count == 0 && !has_sky {
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

        // The family speed contract: one sim clock for rain, lattice
        // and glow alike (shapes invariant under the speed keys).
        let dt_sim = dt_wall * step.chars_per_sec.max(0.0) * AURORA_SIM_TIME_PER_CPS;

        // 1. The sky breathes (laws 1, 2, 4a).
        self.sky.advance(dt_sim, random);

        if self.active_count == 0 {
            return;
        }

        let cols_f = step.cols.max(1) as f32;
        let lines_f = step.lines.max(1) as f32;
        let drag = (-AURORA_SEEK_DRAG * dt_sim).exp();
        let mut absorbed = 0usize;

        for i in 0..self.drops.len() {
            if !self.drops[i].active {
                continue;
            }

            // The funnel target: the ray nearest the drop's
            // pre-integration column (a one-tick lag on the
            // nearest-ray evaluation — the drop steers toward where
            // the ray was when the tick opened).
            let near = self.sky.nearest_ray(self.drops[i].x);
            let (ray_x, ray_depth, ray_glow) = match near {
                Some(ri) => {
                    let r = &self.sky.rays()[ri];
                    (r.x, r.depth, r.glow)
                }
                None => (self.drops[i].x, lines_f, 0.0),
            };
            {
                // Law 3 — the precipitation funnel: within
                // SEEK_RANGE above the target ray's emission depth,
                // bend toward its column with a glow-weighted gain
                // (bright fringes attract the weather harder — the
                // self-organization loop).
                let d = &mut self.drops[i];
                let dist_to_depth = ray_depth - d.y;
                if dist_to_depth > 0.0 && dist_to_depth < AURORA_SEEK_RANGE {
                    let glow_norm = (ray_glow / AURORA_GLOW_LEVEL_HOT).min(1.0);
                    let dir = (ray_x - d.x).clamp(-1.0, 1.0);
                    d.vx += AURORA_SEEK_GAIN * (0.35 + 0.65 * glow_norm) * dir * dt_sim;
                    // Lateral velocity stays a bend, never a slide:
                    // clamp to a fraction of the fall.
                    d.vx = d.vx.clamp(-2.5, 2.5);
                }
                // Lateral drag + integration, wall-clamped.
                d.vx *= drag;
                d.x += d.vx * dt_sim;
                if d.x < 0.0 {
                    d.x = 0.0;
                    d.vx = 0.0;
                }
                if d.x > cols_f - 1.0 {
                    d.x = cols_f - 1.0;
                    d.vx = 0.0;
                }
                // Gravity (capped) + kinetic charge accumulation.
                d.vy = (d.vy + AURORA_DROP_GRAVITY * dt_sim).min(AURORA_DROP_TERMINAL);
                d.y += d.vy * d.pace * dt_sim;
                d.charge += d.vy.abs() * AURORA_CHARGE_RATE * dt_sim;
            }

            // Law 4 — the absorption state test: the drop is inside
            // the emission layer when its depth reaches the target
            // ray's (y only grows, depth is bounded — the test can
            // only become more true; no tunneling at any dt).
            let mut died = false;
            if let Some(ri) = near {
                if self.drops[i].y >= ray_depth {
                    // Absorbed: the drop becomes the light. Its
                    // charge flares the fringe.
                    let charge = self.drops[i].charge;
                    self.sky.absorb(ri, charge);
                    died = true;
                }
            }
            if !died && self.drops[i].y >= lines_f - 1.0 {
                // The ground: through-rain ends at the bottom edge.
                died = true;
            }
            if died {
                let d = &mut self.drops[i];
                d.active = false;
                d.clear_trail();
                absorbed += 1;
            }
        }
        if absorbed > 0 {
            self.active_count = self.active_count.saturating_sub(absorbed);
        }

        // Lifetime backstop (the family contract): sweep strays.
        let mut aged_out = 0usize;
        for d in &mut self.drops {
            if !d.active {
                continue;
            }
            d.sim_age += dt_sim;
            if d.sim_age >= d.lifetime {
                d.active = false;
                d.clear_trail();
                aged_out += 1;
            }
        }
        if aged_out > 0 {
            self.active_count = self.active_count.saturating_sub(aged_out);
        }
    }

    /// Draw pass — the veil first (fabric + fringe + flank spill),
    /// then the rain (head + comet trail), then the monolith
    /// drawn-cell diff cleanup (generation-tagged).
    ///
    /// Veil cells: the fringe reads the glow ladder (law 4); the
    /// corona above it steps down from the fringe's rung; the body
    /// is Ghost on the exponential fabric ramp (bright near the
    /// fringe, diffusing to the top edge — a curtain, not a
    /// border). Curtain cells keep the glyph the frame already
    /// carries (the fabric identity — law 5) and re-roll at the
    /// shimmer rate, the fringe faster than the body. While the
    /// fringe is Hot or brighter it spills into its flanking
    /// columns one rung dimmer (the emission spreading along the
    /// layer).
    pub(crate) fn draw(
        &mut self,
        ctx: &DrawCtx<'_>,
        frame: &mut Frame,
        cleanup: &mut MonolithCleanup<'_>,
        rng: &mut StdRng,
        rand_chance: &Uniform<f32>,
    ) {
        let lines_us = ctx.lines as usize;
        self.current_cells.clear();

        // Pass A — the veil.
        for ray in self.sky.rays() {
            let col = ray.x.round().clamp(0.0, (ctx.cols as f32 - 1.0).max(0.0)) as u16;
            let fringe_line =
                (ray.depth.round() as i32).clamp(1, (ctx.lines as i32 - 2).max(1)) as u16;
            let level = fringe_level(ray.glow, ray.flare_age);
            let bright_zone = matches!(level, BrightnessLevel::Hot | BrightnessLevel::Core);

            // The fabric: lines 1..fringe_line, Ghost with a corona
            // near the fringe, all fading upward on the shared
            // exponential ramp (k = distance above the fringe).
            for line in 1..fringe_line {
                let k_u8 = (fringe_line - line) as u8;
                let k = k_u8 as f32;
                let factor = AURORA_BODY_FACTOR_TOP
                    + (AURORA_BODY_FACTOR_FRINGE - AURORA_BODY_FACTOR_TOP)
                        * (-k / AURORA_BODY_TAU).exp();
                let (cell_level, shimmer) = if k_u8 <= AURORA_CORONA_DEPTH {
                    (step_down_level(level, k_u8), AURORA_SHIMMER_FRINGE)
                } else {
                    (BrightnessLevel::Ghost, AURORA_SHIMMER_BODY)
                };
                let ch = fabric_glyph(ctx, frame, col, line, shimmer, rng, rand_chance);
                draw_aurora_cell(
                    ctx,
                    frame,
                    col,
                    line,
                    ch,
                    self.field_palette_slot,
                    cell_level,
                    factor,
                );
                self.current_cells.push(AuroraCell { col, line });
            }

            // The fringe: the emission layer itself, glow-boosted.
            let fringe_factor = 1.0 + ray.glow * AURORA_FRINGE_BOOST;
            let ch = fabric_glyph(
                ctx,
                frame,
                col,
                fringe_line,
                AURORA_SHIMMER_FRINGE,
                rng,
                rand_chance,
            );
            draw_aurora_cell(
                ctx,
                frame,
                col,
                fringe_line,
                ch,
                self.field_palette_slot,
                level,
                fringe_factor,
            );
            self.current_cells.push(AuroraCell {
                col,
                line: fringe_line,
            });

            // The flank spill: a Hot-or-brighter fringe spreads into
            // its flanking columns one rung dimmer.
            if bright_zone {
                let spill = step_down_level(level, 1);
                for side in [col.wrapping_sub(1), col.saturating_add(1)] {
                    if side >= ctx.cols || side == col {
                        continue;
                    }
                    let ch = fabric_glyph(
                        ctx,
                        frame,
                        side,
                        fringe_line,
                        AURORA_SHIMMER_FRINGE,
                        rng,
                        rand_chance,
                    );
                    draw_aurora_cell(
                        ctx,
                        frame,
                        side,
                        fringe_line,
                        ch,
                        self.field_palette_slot,
                        spill,
                        AURORA_BODY_FACTOR_FRINGE,
                    );
                    self.current_cells.push(AuroraCell {
                        col: side,
                        line: fringe_line,
                    });
                }
            }
        }

        // Pass B — the rain: head + comet trail.
        for d in &mut self.drops {
            if !d.active {
                continue;
            }
            let col = d.x.round();
            let line = d.y.round();
            if col < 0.0 || line < 0.0 || col >= ctx.cols as f32 || line >= ctx.lines as f32 {
                // Off-screen: skip the draw AND the trail push (the
                // trail resumes from the last visible cell).
                continue;
            }
            let (col, line) = (col as u16, line as u16);

            // Matrix shimmer: mutate the glyph when the head lands
            // on a new cell.
            if d.trail_len() > 0 {
                if let Some((prev_col, prev_line)) = d.trail_cell(d.trail_len() as usize - 1) {
                    if (prev_col != col || prev_line != line)
                        && rand_chance.sample(rng) < AURORA_SHIMMER_FRINGE
                    {
                        d.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
                    }
                }
            } else {
                d.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
            }

            let head_level = d.kinetic_level();
            draw_aurora_cell(ctx, frame, col, line, d.ch, d.palette_slot, head_level, 1.0);
            self.current_cells.push(AuroraCell { col, line });

            // Comet trail: previously occupied cells (the fall
            // leaves them above the head), one rung dimmer each.
            for t in 0..d.trail_len() as usize {
                if let Some((tc, tl)) = d.trail_cell(t) {
                    if tc >= ctx.cols || tl >= ctx.lines {
                        continue;
                    }
                    let depth = (d.trail_len() as usize - t).min(3) as u8;
                    let trail_level = step_down_level(head_level, depth);
                    draw_aurora_cell(ctx, frame, tc, tl, d.ch, d.palette_slot, trail_level, 1.0);
                    self.current_cells.push(AuroraCell { col: tc, line: tl });
                }
            }

            d.push_trail(col, line);
        }

        // Pass C — the generation-tagged diff cleanup (monolith
        // pattern: u32 counter bump instead of clearing the array).
        self.drawn_gen_counter = self.drawn_gen_counter.wrapping_add(1);
        let gen = self.drawn_gen_counter;
        let need_len = self.drops.len().saturating_mul(lines_us.max(1));
        if self.drawn_gen.len() != need_len {
            self.drawn_gen.resize(need_len, 0);
        }
        for cell in &self.current_cells {
            let idx = cell.col as usize * lines_us + cell.line as usize;
            if idx < self.drawn_gen.len() {
                self.drawn_gen[idx] = gen;
            }
        }

        // Clear previous cells NOT redrawn this frame.
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

    // -- Test-only diagnostics (mirrors the family *_for_test API) --

    #[cfg(test)]
    pub(crate) fn drop_states_for_test(&self) -> Vec<(f32, f32, f32, f32)> {
        self.drops
            .iter()
            .filter(|d| d.active)
            .map(|d| (d.x, d.y, d.vx, d.vy))
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn sky_for_test(&self) -> &AuroraSky {
        &self.sky
    }

    #[cfg(test)]
    pub(crate) fn drawn_cells_for_test(&self) -> &[AuroraCell] {
        &self.current_cells
    }
}

/// The fabric glyph read (law 5): a curtain cell keeps the glyph the
/// frame already carries (the fabric identity); a fresh cell picks
/// from the pool; cells carrying a glyph re-roll at the shimmer
/// chance.
fn fabric_glyph(
    ctx: &DrawCtx<'_>,
    frame: &Frame,
    col: u16,
    line: u16,
    shimmer: f32,
    rng: &mut StdRng,
    rand_chance: &Uniform<f32>,
) -> char {
    let existing = frame
        .index(col, line)
        .map(|i| frame.cell_at_index_ref(i).ch)
        .filter(|&c| c != ' ');
    let mut ch = existing.unwrap_or_else(|| pick_pool_char(ctx.char_pool, rand_chance, rng));
    if existing.is_some() && rand_chance.sample(rng) < shimmer {
        ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
    }
    ch
}

/// Step a brightness level down (toward Ghost) by `depth` ladder
/// rungs (mirrors aeolian's `step_down_level`).
fn step_down_level(level: BrightnessLevel, depth: u8) -> BrightnessLevel {
    match level {
        BrightnessLevel::Core if depth >= 2 => BrightnessLevel::Mid,
        BrightnessLevel::Core => BrightnessLevel::Hot,
        BrightnessLevel::Hot if depth >= 2 => BrightnessLevel::Ghost,
        BrightnessLevel::Hot => BrightnessLevel::Mid,
        BrightnessLevel::Mid => BrightnessLevel::Ghost,
        BrightnessLevel::Ghost | BrightnessLevel::Dim => BrightnessLevel::Ghost,
    }
}

/// Render one aurora cell (palette-aware color + bold, mono-safe).
/// 8 args exceeds clippy's default `too_many_arguments` threshold (7) —
/// the factor arg is the aurora addition (the fabric ramp) over the
/// aeolian draw helper's 7; a struct bundle for one private fn would
/// be overkill (the bench_io.rs precedent).
#[allow(clippy::too_many_arguments)]
fn draw_aurora_cell(
    ctx: &DrawCtx<'_>,
    frame: &mut Frame,
    col: u16,
    line: u16,
    ch: char,
    palette_slot: u8,
    level: BrightnessLevel,
    factor: f32,
) {
    if line >= ctx.lines || col >= ctx.cols {
        return;
    }
    let fg = color_for_level(ctx, palette_slot, line, col, level, factor);
    let bold = bold_for_level(ctx.bold_mode, level, line, col);
    let cell = crate::cell::Cell {
        ch,
        fg,
        bg: ctx.bg,
        bold,
    };
    frame.set(col, line, cell);
}
