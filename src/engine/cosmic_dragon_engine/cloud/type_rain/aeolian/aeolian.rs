// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The aeolian rain state machine (NIGHT-special-2, the ninth
//! style) — the orchestration pass set: pool, spawn, advance, draw.
//!
//! The style composes the two halves of the weave (see mod.rs for
//! the full derivation): `AeolianStrings` (the instrument) and
//! `AeolianDrop` (the weather). This struct owns the pool lifecycle
//! and the per-frame passes, following the structured-family
//! contract (lorenz/vortex/dragon/physarum/black_hole):
//!
//! - pool: one drop per column (the family lane model),
//! - spawn: the deficit-bounded fractional accumulator,
//! - advance: one global clock (dt clamped by `max_sim_delta`,
//!   eased by `resume_blend`, scaled to sim-time by chars_per_sec),
//! - draw: string cells + drop heads with comet trails, all through
//!   the monolith drawn-cell diff cleanup (generation-tagged).
//!
//! The advance pass is the family's first stochastic pass: the
//! capture/through split at a string is a probability the local
//! field brightness weights (law 5 — bright antinodes eat rain,
//! silent strings let it pass), so it takes the RNG bundle.

use std::time::{Duration, Instant};

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::frame::Frame;

use crate::constants::{
    AEOLIAN_ACTIVE_BASE, AEOLIAN_ACTIVE_DENSITY_MULT, AEOLIAN_ACTIVE_MAX, AEOLIAN_CAPTURE_BASE,
    AEOLIAN_CAPTURE_GAIN, AEOLIAN_CHARGE_RATE, AEOLIAN_DRAW_FLOOR, AEOLIAN_DROP_GRAVITY,
    AEOLIAN_DROP_TERMINAL, AEOLIAN_ECHO_CHANCE, AEOLIAN_ECHO_GAIN, AEOLIAN_LEVEL_HOT,
    AEOLIAN_MAX_AGE_SECS, AEOLIAN_PASS_GRAZE, AEOLIAN_PLUCK_GAIN, AEOLIAN_SEEK_DRAG,
    AEOLIAN_SEEK_GAIN, AEOLIAN_SEEK_RANGE, AEOLIAN_SEEK_VX_LIMIT, AEOLIAN_SHIMMER_CHANCE,
    AEOLIAN_SIM_TIME_PER_CPS, AEOLIAN_SPAWN_RATE_FLOOR, AEOLIAN_SPAWN_RATE_MULT, AEOLIAN_SURF_KICK,
    SPAWN_REMAINDER_CAP,
};

use super::super::super::render::DrawCtx;
use super::super::monolith::monolith_helpers::{
    bold_for_level, clear_cell, color_for_level, pick_pool_char,
};
use super::super::monolith::{BrightnessLevel, MonolithCleanup};

use super::drops::AeolianDrop;
use super::strings::{is_knot, level_for_amplitude, AeolianStrings};

/// One drawn cell (col, line) — the diff-cleanup currency (same
/// shape as `LorenzCell`/`VortexCell`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AeolianCell {
    pub(crate) col: u16,
    pub(crate) line: u16,
}

/// Spawn inputs (mirrors `BlackHoleSpawnParams` — the bundle keeps
/// clippy's `too_many_arguments` threshold respected).
pub(crate) struct AeolianSpawnParams {
    pub(crate) cols: u16,
    pub(crate) lines: u16,
    pub(crate) density: f32,
    pub(crate) active_palette_slot: u8,
    pub(crate) spawn_scale: f32,
}

/// RNG bundle (mirrors `BlackHoleRandom`).
pub(crate) struct AeolianRandom<'a> {
    pub(crate) rng: &'a mut StdRng,
    pub(crate) rand_chance: &'a Uniform<f32>,
}

/// Per-frame step inputs. Carries viewport geometry (cols, lines)
/// like `DragonStep` — the drop physics needs bounds for the wall
/// clamp and the ground absorption.
pub(crate) struct AeolianStep {
    pub(crate) now: Instant,
    /// chars_per_sec already multiplied by the terminal speed_mult.
    /// Scales the whole weave on one clock (dt_sim) — the family
    /// speed contract: trajectory shapes survive the speed keys.
    pub(crate) chars_per_sec: f32,
    pub(crate) cols: u16,
    pub(crate) lines: u16,
    pub(crate) max_sim_delta: Duration,
    pub(crate) resume_blend: f32,
}

/// The aeolian rain: the string field plus the drop pool, one state
/// machine.
#[derive(Debug)]
pub(crate) struct AeolianRain {
    pub(crate) drops: Vec<AeolianDrop>,
    /// The instrument (invented string physics — see strings.rs).
    pub(crate) strings: AeolianStrings,
    active_count: usize,
    /// Rotating scan cursor for amortized O(1) free-slot search
    /// (mirrors `VortexRain::spawn_scan_idx`).
    spawn_scan_idx: usize,
    /// Global motion clock (dt = now - last_step, clamped by
    /// max_sim_delta and eased by resume_blend — the family
    /// contract; a fully-paused run simply stops advancing).
    last_step: Option<Instant>,
    /// Palette slot of the string field (one body, one slot —
    /// adopted at palette-transition completion, mirroring the
    /// black hole ball).
    field_palette_slot: u8,
    current_cells: Vec<AeolianCell>,
    previous_cells: Vec<AeolianCell>,
    drawn_gen: Vec<u32>,
    drawn_gen_counter: u32,
}

impl AeolianRain {
    pub(crate) fn new() -> Self {
        Self {
            drops: Vec::new(),
            strings: AeolianStrings::new(),
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

    /// Rebuild the pool + strings for a new viewport (or style
    /// entry). The pool is one drop per column; the strings are
    /// re-tiered for the new height, all channels silent (a fresh
    /// instrument waits to be played).
    pub(crate) fn reset(&mut self, cols: u16, lines: u16) {
        let lanes = cols.max(1) as usize;
        self.drops.clear();
        self.drops.resize_with(lanes, AeolianDrop::vacant);
        self.strings.reset(cols, lines);
        self.active_count = 0;
        self.spawn_scan_idx = 0;
        self.last_step = None;
        self.clear_draw_history();
    }

    pub(crate) fn active_count(&self) -> usize {
        self.active_count
    }

    /// Palette transition completion: the string field adopts the
    /// new slot (one body, one slot), and every active drop adopts
    /// it individually (mirrors the family contract).
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
    /// baseline. The string field itself is simulation state and is
    /// preserved (wiping it would silence a ringing instrument).
    pub(crate) fn clear_draw_history(&mut self) {
        self.current_cells.clear();
        self.previous_cells.clear();
        self.drawn_gen.clear();
        self.drawn_gen_counter = 0;
    }

    /// Steady-state active-drop target from pool size + density
    /// (mirrors `VortexRain::target_active_count`): the calm-sky
    /// dial family — a sparse ambient drizzle, never a downpour.
    fn target_active_count(lanes: usize, density: f32) -> usize {
        if lanes == 0 {
            return 0;
        }
        let ratio = (AEOLIAN_ACTIVE_BASE + density.clamp(0.01, 5.0) * AEOLIAN_ACTIVE_DENSITY_MULT)
            .clamp(0.02, AEOLIAN_ACTIVE_MAX);
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
        params: &AeolianSpawnParams,
        random: &mut AeolianRandom<'_>,
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
        let spawn_rate = (target as f32 * AEOLIAN_SPAWN_RATE_MULT + AEOLIAN_SPAWN_RATE_FLOOR)
            * params.spawn_scale;
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
    fn activate_drop(&mut self, idx: usize, palette_slot: u8, random: &mut AeolianRandom<'_>) {
        let drift_roll = (random.rand_chance.sample(random.rng) - 0.5) * 2.0;
        let pace = 0.85 + random.rand_chance.sample(random.rng) * 0.30;
        let lifetime = AEOLIAN_MAX_AGE_SECS * (0.85 + random.rand_chance.sample(random.rng) * 0.30);

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

    /// Motion pass — the weave's physics core (laws 5-6 live here).
    ///
    /// 1. The string field takes one conduction sweep (laws 1-4 in
    ///    `AeolianStrings::advance`).
    /// 2. Each drop: gravity (capped terminal), resonance seeking
    ///    (lateral bend toward the next string's local crest within
    ///    SEEK_RANGE), lateral drag, position integration, then the
    ///    capture test on any string row crossed THIS tick (interval
    ///    containment — no tunneling at any dt), and the ground
    ///    absorption at the bottom edge.
    pub(crate) fn advance(&mut self, step: &AeolianStep, random: &mut AeolianRandom<'_>) {
        let has_field = !self.strings.string_rows().is_empty();
        if self.active_count == 0 && !has_field {
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

        // The family speed contract: one sim clock for rain, string
        // conduction and decay alike (shapes invariant under the
        // speed keys).
        let dt_sim = dt_wall * step.chars_per_sec.max(0.0) * AEOLIAN_SIM_TIME_PER_CPS;

        // 1. The instrument breathes (laws 1-4).
        self.strings.advance(dt_sim);

        if self.active_count == 0 {
            return;
        }

        let cols_f = step.cols.max(1) as f32;
        let lines_f = step.lines.max(1) as f32;
        let rows = self.strings.string_rows().to_vec();
        let drag = (-AEOLIAN_SEEK_DRAG * dt_sim).exp();
        let mut absorbed = 0usize;

        for i in 0..self.drops.len() {
            if !self.drops[i].active {
                continue;
            }
            let prev_y = self.drops[i].y;

            // Law 6a — resonance seeking: within SEEK_RANGE above the
            // next string below, bend toward the field's local crest
            // (the gradient of the combined amplitude).
            {
                let d = &mut self.drops[i];
                if let Some(s_idx) = rows.iter().position(|&r| r as f32 >= d.y) {
                    let dist = rows[s_idx] as f32 - d.y;
                    if dist > 0.0 && dist < AEOLIAN_SEEK_RANGE {
                        let col = d.x.round().clamp(0.0, cols_f - 1.0) as usize;
                        let slope = self.strings.slope(s_idx, col);
                        d.vx += AEOLIAN_SEEK_GAIN * slope * dt_sim;
                        // Lateral velocity stays a bend, never a
                        // slide: clamp to a fraction of the fall.
                        d.vx = d.vx.clamp(-AEOLIAN_SEEK_VX_LIMIT, AEOLIAN_SEEK_VX_LIMIT);
                    }
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
                d.vy = (d.vy + AEOLIAN_DROP_GRAVITY * dt_sim).min(AEOLIAN_DROP_TERMINAL);
                d.y += d.vy * d.pace * dt_sim;
                d.charge += d.vy.abs() * AEOLIAN_CHARGE_RATE * dt_sim;
            }

            // Law 5 — the capture test: any string row crossed this
            // tick (interval containment — no tunneling at any dt).
            // A drop crosses at most one row per tick at the capped
            // terminal speed, but the loop keeps the contract true
            // at any dt (rows are >= 13 lines apart).
            let col = self.drops[i].x.round().clamp(0.0, cols_f - 1.0) as usize;
            let mut died = false;
            for (s_idx, &row) in rows.iter().enumerate() {
                let row_f = row as f32;
                if prev_y < row_f && self.drops[i].y >= row_f {
                    let u = self.strings.combined(s_idx, col);
                    let bright = (u / AEOLIAN_LEVEL_HOT).min(1.0);
                    let capture_p = AEOLIAN_CAPTURE_BASE + AEOLIAN_CAPTURE_GAIN * bright;
                    if random.rand_chance.sample(random.rng) < capture_p {
                        // Captured: the drop becomes the note. Pluck
                        // the string it landed on...
                        let amp = self.drops[i].charge * AEOLIAN_PLUCK_GAIN;
                        self.strings.pluck(s_idx, col, amp);
                        // ...and with ECHO_CHANCE the impact resonates
                        // the string below (the aftershock cascade).
                        if s_idx + 1 < rows.len()
                            && random.rand_chance.sample(random.rng) < AEOLIAN_ECHO_CHANCE
                        {
                            let echo =
                                self.drops[i].charge * AEOLIAN_PLUCK_GAIN * AEOLIAN_ECHO_GAIN;
                            self.strings.pluck(s_idx + 1, col, echo);
                        }
                        died = true;
                    } else {
                        // Through-rain: a faint graze keeps the
                        // instrument murmuring, and the surf kick
                        // flings the drop onward faster (the
                        // interference streak).
                        let graze = self.drops[i].charge * AEOLIAN_PLUCK_GAIN * AEOLIAN_PASS_GRAZE;
                        self.strings.pluck(s_idx, col, graze);
                        self.drops[i].vy =
                            (self.drops[i].vy + AEOLIAN_SURF_KICK * u).min(AEOLIAN_DROP_TERMINAL);
                    }
                    break;
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

    /// Draw pass — strings first (the instrument revealed only where
    /// it rings), then the rain (head + comet trail), then the
    /// monolith drawn-cell diff cleanup (generation-tagged).
    ///
    /// String cells: the combined amplitude picks the ladder rung;
    /// interference knots (both channels strong — counter-propagating
    /// packets crossing) read Core white. A ringing cell keeps the
    /// glyph the frame already carries there (the note identity —
    /// strings do not move, so their glyphs persist) and re-rolls
    /// matrix-style only while the passage is bright.
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

        // Pass A — the strings. Cells below the draw floor are not
        // drawn at all: the instrument is invisible where silent.
        let rows = self.strings.string_rows().to_vec();
        for (s_idx, &row) in rows.iter().enumerate() {
            if row >= ctx.lines {
                continue;
            }
            for col in 0..ctx.cols {
                let u = self.strings.combined(s_idx, col as usize);
                if u <= AEOLIAN_DRAW_FLOOR {
                    continue;
                }
                let knot = self.strings.knot(s_idx, col as usize);
                let level = if is_knot(knot) {
                    BrightnessLevel::Core
                } else {
                    level_for_amplitude(u)
                };

                // Note identity: reuse the glyph already on the frame
                // (a string cell was drawn by this pass last frame); a
                // fresh cell (or one vacated to a space) picks from
                // the pool. Bright cells re-roll with the shimmer
                // chance as the packet passes through them.
                let existing = frame
                    .index(col, row)
                    .map(|i| frame.cell_at_index_ref(i).ch)
                    .filter(|&c| c != ' ');
                let mut ch =
                    existing.unwrap_or_else(|| pick_pool_char(ctx.char_pool, rand_chance, rng));
                if matches!(
                    level,
                    BrightnessLevel::Mid | BrightnessLevel::Hot | BrightnessLevel::Core
                ) && rand_chance.sample(rng) < AEOLIAN_SHIMMER_CHANCE
                {
                    ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
                }

                draw_aeolian_cell(ctx, frame, col, row, ch, self.field_palette_slot, level);
                self.current_cells.push(AeolianCell { col, line: row });
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
                        && rand_chance.sample(rng) < AEOLIAN_SHIMMER_CHANCE
                    {
                        d.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
                    }
                }
            } else {
                d.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
            }

            let head_level = d.kinetic_level();
            draw_aeolian_cell(ctx, frame, col, line, d.ch, d.palette_slot, head_level);
            self.current_cells.push(AeolianCell { col, line });

            // Comet trail: previously occupied cells (the fall
            // leaves them above the head), one rung dimmer each.
            for t in 0..d.trail_len() as usize {
                if let Some((tc, tl)) = d.trail_cell(t) {
                    if tc >= ctx.cols || tl >= ctx.lines {
                        continue;
                    }
                    let depth = (d.trail_len() as usize - t).min(3) as u8;
                    let trail_level = step_down_level(head_level, depth);
                    draw_aeolian_cell(ctx, frame, tc, tl, d.ch, d.palette_slot, trail_level);
                    self.current_cells.push(AeolianCell { col: tc, line: tl });
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
    pub(crate) fn strings_for_test(&self) -> &AeolianStrings {
        &self.strings
    }

    #[cfg(test)]
    pub(crate) fn strings_mut_for_test(&mut self) -> &mut AeolianStrings {
        &mut self.strings
    }

    #[cfg(test)]
    pub(crate) fn drawn_cells_for_test(&self) -> &[AeolianCell] {
        &self.current_cells
    }
}

/// Step a brightness level down (toward Ghost) by `depth` ladder
/// rungs (mirrors lorenz's `step_down_level`).
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

/// Render one aeolian cell (palette-aware color + bold, mono-safe).
fn draw_aeolian_cell(
    ctx: &DrawCtx<'_>,
    frame: &mut Frame,
    col: u16,
    line: u16,
    ch: char,
    palette_slot: u8,
    level: BrightnessLevel,
) {
    if line >= ctx.lines || col >= ctx.cols {
        return;
    }
    let fg = color_for_level(ctx, palette_slot, line, col, level, 1.0);
    let bold = bold_for_level(ctx.bold_mode, level, line, col);
    let cell = crate::cell::Cell {
        ch,
        fg,
        bg: ctx.bg,
        bold,
    };
    frame.set(col, line, cell);
}
