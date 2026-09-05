// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Liquid matrix rain for the flux scene (task-19, fourth rain style
//! replacement — supersedes the rejected water-surface ripple style).
//!
//! Style DNA — 100% distinct from all three surviving styles: glyphs
//! are FLUID PARTICLES falling through a living incompressible
//! liquid. Every simulated tick runs the full particle-grid hybrid
//! pipeline in [`super::flux_field`] (P2G splat, gravity, pressure
//! projection, FLIP/PIC readback), so the falling glyph jets push the
//! surrounding fluid aside, shear against each other and curl into
//! eddies — emergent Kelvin-Helmholtz structure that no scripted
//! effect can reproduce and no competitor renderer has. The minimal
//! charset renders it as falling nabla glyphs — the gradient operator
//! itself, the exact symbol the projection step computes.
//!
//! Motion model: each mote carries its own velocity in screen space
//! (units of one column width per second on both axes; one vertical
//! unit spans two cell lines). Spawning injects fresh downward
//! momentum at the top edge; gravity (scaled by the terminal
//! speed multiplier so the up/down speed keys feel native)
//! accelerates the fall; the incompressibility projection converts
//! the jets' collective momentum into lateral spread and swirl.
//! Motes exit at the bottom (open boundary) or expire by lifetime,
//! then the spawn budget refills from the top — a perpetual liquid
//! rain cycle.
//!
//! Determinism and rate independence: physics advances on a FIXED
//! timestep (FLUX_SIM_DT) fed by a wall-clock accumulator, capped at
//! FLUX_MAX_STEPS_PER_FRAME — the game-physics fixed-step pattern.
//! A 144 Hz terminal runs two identical 60 Hz solver steps at most
//! per rendered frame; the benchmark's uniform stepping hits exactly
//! one step per frame; a slow terminal drops backlog instead of
//! teleporting particles. The resume easing scales the accumulator
//! growth so an unpause wakes the liquid in slow motion, matching
//! the engine-wide resume philosophy.
//!
//! Cleanup follows the monolith three-pass drawn-cell diff pattern
//! (current cells, generation tag, clear stale) — same contract as
//! the vortex style. Brightness maps particle speed: fast jets glow
//! hot, slow eddies dim — the terminal-scale analog of Doppler flow
//! visualization, which is exactly what makes the emergent flow
//! structure readable.

use std::time::{Duration, Instant};

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::frame::Frame;

use super::flux_field::{FluxField, FluxVel};
use super::monolith::BrightnessLevel;
use super::monolith_helpers::{bold_for_level, clear_cell, color_for_level};
use super::render::DrawCtx;

/// Trail depth per mote (comet streak length in cells).
pub(crate) const FLUX_TRAIL_LEN: usize = 3;

/// One drawn cell (col, line). Own struct instead of reusing the
/// vortex DrawnCell because flux trails carry a brightness depth
/// resolved at draw time from speed, not radius.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FluxCell {
    pub(crate) col: u16,
    pub(crate) line: u16,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FluxMote {
    pub(crate) active: bool,
    /// Screen-space position: x in column widths, y in column widths
    /// (two cell lines per unit). x in [0, cols), y in [-1, lines/2].
    pub(crate) x: f32,
    pub(crate) y: f32,
    /// Screen-space velocity (units per second).
    pub(crate) vx: f32,
    pub(crate) vy: f32,
    /// Age and lifetime in simulated seconds (lifetime churns
    /// eddy-trapped motes so the pool keeps flowing).
    pub(crate) age: f32,
    pub(crate) lifetime: f32,
    /// Glyph carried by the mote; re-rolled matrix-style when the
    /// head crosses into a new cell (shimmer gate in `draw`).
    pub(crate) ch: char,
    /// Palette slot adopted at spawn / palette transition.
    pub(crate) palette_slot: u8,
    /// Ring buffer of the last FLUX_TRAIL_LEN head cell positions.
    /// Shift-left layout: index 0 = oldest, len-1 = most recent.
    trail: [(u16, u16); FLUX_TRAIL_LEN],
    trail_len: u8,
}

impl FluxMote {
    const fn vacant() -> Self {
        Self {
            active: false,
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
            age: 0.0,
            lifetime: 0.0,
            ch: '0',
            palette_slot: 0,
            trail: [(0, 0); FLUX_TRAIL_LEN],
            trail_len: 0,
        }
    }

    fn push_trail(&mut self, col: u16, line: u16) {
        if self.trail_len as usize >= FLUX_TRAIL_LEN {
            for i in 0..FLUX_TRAIL_LEN - 1 {
                self.trail[i] = self.trail[i + 1];
            }
            self.trail[FLUX_TRAIL_LEN - 1] = (col, line);
        } else {
            let idx = self.trail_len as usize;
            self.trail[idx] = (col, line);
            self.trail_len += 1;
        }
    }
}

/// Spawn inputs (mirrors `VortexSpawnParams`).
pub(crate) struct FluxSpawnParams {
    pub(crate) cols: u16,
    pub(crate) lines: u16,
    pub(crate) density: f32,
    pub(crate) active_palette_slot: u8,
    pub(crate) spawn_scale: f32,
    /// chars_per_sec already multiplied by the terminal speed_mult —
    /// sets the entry velocity so the speed keys feel native.
    pub(crate) chars_per_sec: f32,
}

/// RNG bundle (mirrors `VortexRandom`).
pub(crate) struct FluxRandom<'a> {
    pub(crate) rng: &'a mut StdRng,
    pub(crate) rand_chance: &'a Uniform<f32>,
}

/// Per-frame step inputs for the advance pass. Viewport geometry is
/// NOT carried here — the field owns its dimensions from `reset`;
/// draw derives geometry from DrawCtx (the authoritative frame size).
pub(crate) struct FluxStep {
    pub(crate) now: Instant,
    /// chars_per_sec already multiplied by the terminal speed_mult.
    pub(crate) chars_per_sec: f32,
    pub(crate) max_sim_delta: Duration,
    pub(crate) resume_blend: f32,
}

pub(crate) struct FluxRain {
    pub(crate) motes: Vec<FluxMote>,
    field: FluxField,
    active_count: usize,
    /// Rotating scan cursor for amortized O(1) free-slot search.
    spawn_scan_idx: usize,
    /// Fixed-step accumulator: simulated seconds banked from wall
    /// clock, consumed one FLUX_SIM_DT at a time.
    sim_accumulator: f32,
    /// Global motion clock (wall). dt = now - last_step clamped by
    /// max_sim_delta; a fully-paused run simply stops integrating
    /// (rain_at early-returns before the advance pass).
    last_step: Option<Instant>,
    /// Simulated solver steps consumed since reset — the fixed-step
    /// determinism hook for tests.
    sim_steps: u64,
    /// Viewport extents in screen space, snapshotted at reset (the
    /// authoritative geometry; the field is sized from the same
    /// values).
    x_max: f32,
    y_max: f32,
    current_cells: Vec<FluxCell>,
    previous_cells: Vec<FluxCell>,
    drawn_gen: Vec<u32>,
    drawn_gen_counter: u32,
}

impl FluxRain {
    pub(crate) fn new() -> Self {
        Self {
            motes: Vec::new(),
            field: FluxField::new(1, 1),
            active_count: 0,
            spawn_scan_idx: 0,
            sim_accumulator: 0.0,
            last_step: None,
            sim_steps: 0,
            x_max: 1.0,
            y_max: 1.0,
            current_cells: Vec::new(),
            previous_cells: Vec::new(),
            drawn_gen: Vec::new(),
            drawn_gen_counter: 0,
        }
    }

    /// Rebuild the mote pool and velocity field for a new viewport
    /// (or style entry). The pool is sized one mote per column
    /// (mirroring the monolith lane model); the active target is a
    /// density-driven ratio of that pool.
    pub(crate) fn reset(&mut self, cols: u16, lines: u16) {
        let lanes = cols.max(1) as usize;
        self.motes.clear();
        self.motes.resize_with(lanes, FluxMote::vacant);
        self.field.reset(cols, lines);
        self.active_count = 0;
        self.spawn_scan_idx = 0;
        self.sim_accumulator = 0.0;
        self.last_step = None;
        self.sim_steps = 0;
        self.x_max = cols.max(1) as f32;
        self.y_max = (lines.max(1) as f32) * 0.5;
        self.clear_draw_history();
    }

    pub(crate) fn active_count(&self) -> usize {
        self.active_count
    }

    /// Palette transition completion: all motes adopt the new slot.
    pub(crate) fn adopt_palette_slot(&mut self, palette_slot: u8) {
        for m in &mut self.motes {
            if m.active {
                m.palette_slot = palette_slot;
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

    /// Steady-state active-mote target from pool size + density.
    fn target_active_count(lanes: usize, density: f32) -> usize {
        if lanes == 0 {
            return 0;
        }
        let ratio = (crate::constants::FLUX_ACTIVE_BASE
            + density.clamp(0.01, 5.0) * crate::constants::FLUX_ACTIVE_DENSITY_MULT)
            .clamp(0.02, crate::constants::FLUX_ACTIVE_MAX);
        ((lanes as f32 * ratio).round() as usize).clamp(1, lanes)
    }

    /// Amortized free-slot scan (rotating cursor, mirrors the vortex).
    fn find_inactive_mote(&mut self) -> Option<usize> {
        let len = self.motes.len();
        if len == 0 {
            return None;
        }
        for step in 0..len {
            let idx = (self.spawn_scan_idx + step) % len;
            if !self.motes[idx].active {
                self.spawn_scan_idx = (idx + 1) % len;
                return Some(idx);
            }
        }
        None
    }

    /// Spawn pass — accumulator pattern identical to the vortex and
    /// monolith styles (deficit-bounded budget + fractional carry).
    pub(crate) fn spawn(
        &mut self,
        elapsed: Duration,
        spawn_remainder: &mut f32,
        params: &FluxSpawnParams,
        random: &mut FluxRandom<'_>,
    ) {
        if params.cols == 0 || params.lines == 0 || self.motes.is_empty() {
            *spawn_remainder = 0.0;
            return;
        }

        let target = Self::target_active_count(self.motes.len(), params.density);
        if self.active_count >= target {
            *spawn_remainder = (*spawn_remainder).min(crate::constants::SPAWN_REMAINDER_CAP);
            return;
        }

        let deficit = target - self.active_count;
        let spawn_rate = (target as f32 * crate::constants::FLUX_SPAWN_RATE_MULT
            + crate::constants::FLUX_SPAWN_RATE_FLOOR)
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
            let Some(idx) = self.find_inactive_mote() else {
                break;
            };
            self.activate_mote(idx, params, random);
            self.active_count += 1;
        }
    }

    /// Activate a vacant mote just above the top edge with fresh
    /// downward momentum. The entry velocity maps chars_per_sec
    /// (cell rows per second for cascade styles) into screen units
    /// — one unit per two cell lines — so the speed keys feel native
    /// against the cascade styles; a small roll keeps stream heads
    /// from entering in lockstep.
    fn activate_mote(&mut self, idx: usize, params: &FluxSpawnParams, random: &mut FluxRandom<'_>) {
        let roll = random.rand_chance.sample(random.rng);
        let jitter =
            (random.rand_chance.sample(random.rng) - 0.5) * 2.0 * crate::constants::FLUX_ENTRY_VX;
        let vy_roll = (random.rand_chance.sample(random.rng) - 0.5) * 4.0;

        let m = &mut self.motes[idx];
        m.active = true;
        m.x = (roll * params.cols as f32).clamp(0.5, (params.cols.max(2) - 1) as f32);
        m.y = -0.5 - roll * 0.5;
        m.vx = jitter;
        m.vy = (params.chars_per_sec.max(2.0) + vy_roll) * 0.5;
        m.age = 0.0;
        m.lifetime = crate::constants::FLUX_MOTE_LIFETIME
            * (0.70 + random.rand_chance.sample(random.rng) * 0.60);
        m.palette_slot = params.active_palette_slot;
        m.trail_len = 0;
    }

    /// Advance pass — wall-clock dt into the fixed-step accumulator,
    /// then whole solver steps. Slow terminals drop backlog (the cap
    /// branch zeroes the accumulator) rather than bursting multiple
    /// steps and teleporting motes.
    pub(crate) fn advance(&mut self, step: &FluxStep) {
        if self.active_count == 0 {
            self.last_step = Some(step.now);
            self.sim_accumulator = 0.0;
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
        self.sim_accumulator += dt;

        let sim_dt = crate::constants::FLUX_SIM_DT;
        let mut steps = 0_u32;
        while self.sim_accumulator >= sim_dt && steps < crate::constants::FLUX_MAX_STEPS_PER_FRAME {
            self.solver_step(sim_dt, step.chars_per_sec);
            self.sim_accumulator -= sim_dt;
            steps += 1;
        }
        // Backlog drop: a terminal slower than the catch-up budget
        // skips simulated time (stays real-time-true, never bursts).
        if steps == crate::constants::FLUX_MAX_STEPS_PER_FRAME && self.sim_accumulator >= sim_dt {
            self.sim_accumulator = 0.0;
        }
    }

    /// One fixed solver step: the full PIC/FLIP pipeline plus mote
    /// integration and recycling. `chars_per_sec` scales gravity so
    /// the up/down speed keys change the rain energy, not just the
    /// spawn pace.
    fn solver_step(&mut self, dt: f32, chars_per_sec: f32) {
        self.sim_steps += 1;
        let cols_f = self.x_max;
        let y_max = self.y_max;

        // Step 1: P2G — splat every mote's momentum into the grid.
        self.field.begin_p2g();
        for m in &self.motes {
            if m.active {
                self.field.splat(m.x, m.y, FluxVel { vx: m.vx, vy: m.vy });
            }
        }
        self.field.finish_p2g();

        // Steps 2-3: gravity on weight-carrying nodes + snapshot +
        // projection + walls. Gravity scales with the speed keys
        // (clamped like the ripple ring speed scale).
        let gravity = crate::constants::FLUX_GRAVITY
            * (chars_per_sec.max(0.0) / crate::constants::FLUX_SPEED_REF_CPS).clamp(0.25, 3.0);
        self.field.apply_gravity_snapshot_project(dt, gravity);

        // Step 4 + advection: FLIP/PIC readback, damping, integration,
        // recycling. Split borrows: the field is only read here.
        let pic = crate::constants::FLUX_PIC_BLEND;
        let damping = (1.0 - crate::constants::FLUX_PARTICLE_DAMPING * dt).max(0.0);
        let mut retired = 0_usize;
        for m in &mut self.motes {
            if !m.active {
                continue;
            }
            let after = self.field.sample(m.x, m.y);
            let before = self.field.sample_prev(m.x, m.y);
            // FLIP: particle velocity + the local field change (force
            // and projection); PIC: snap to the sampled field value.
            let flip_vx = m.vx + (after.vx - before.vx);
            let flip_vy = m.vy + (after.vy - before.vy);
            m.vx = (pic * after.vx + (1.0 - pic) * flip_vx) * damping;
            m.vy = (pic * after.vy + (1.0 - pic) * flip_vy) * damping;
            // Velocity clamp: numerical safety, far above visual range.
            let speed2 = m.vx * m.vx + m.vy * m.vy;
            if speed2 > crate::constants::FLUX_MAX_SPEED * crate::constants::FLUX_MAX_SPEED {
                let s = crate::constants::FLUX_MAX_SPEED / speed2.sqrt();
                m.vx *= s;
                m.vy *= s;
            }

            m.x += m.vx * dt;
            m.y += m.vy * dt;
            m.age += dt;

            // Walls: clamp inside, kill lateral motion (no through-flow).
            if m.x < 0.5 {
                m.x = 0.5;
                m.vx = m.vx.max(0.0);
            } else if m.x > cols_f - 0.5 {
                m.x = cols_f - 0.5;
                m.vx = m.vx.min(0.0);
            }

            // Recycle: bottom exit (open boundary) or lifetime expiry.
            if m.y > y_max + crate::constants::FLUX_EXIT_MARGIN || m.age >= m.lifetime {
                m.active = false;
                m.trail_len = 0;
                retired += 1;
            }
        }
        if retired > 0 {
            self.active_count = self.active_count.saturating_sub(retired);
        }
    }

    /// Draw pass — head cell + comet trail + speed-graded brightness
    /// + monolith-style diff cleanup.
    ///
    /// Matrix-style glyph mutation: when the head crosses into a new
    /// cell, the glyph re-rolls with probability FLUX_SHIMMER_CHANCE
    /// (mutation tied to motion — authentic matrix-rain shimmer, and
    /// deterministic under the bench's uniform stepping).
    pub(crate) fn draw(
        &mut self,
        ctx: &DrawCtx<'_>,
        frame: &mut Frame,
        cleanup: &mut super::monolith::MonolithCleanup<'_>,
        rng: &mut StdRng,
        rand_chance: &Uniform<f32>,
    ) {
        let lines_us = ctx.lines as usize;

        self.current_cells.clear();
        for m in &mut self.motes {
            if !m.active {
                continue;
            }
            // Screen space to cells: column rounds directly; one
            // screen unit vertically spans two cell lines.
            let col = m.x.round() as i32;
            let line = (m.y * 2.0).round() as i32;
            if col < 0 || line < 0 || col >= ctx.cols as i32 || line >= ctx.lines as i32 {
                // Off-screen motes (just spawned above the rim) keep
                // their trail bookkeeping consistent by skipping the
                // draw entirely this frame.
                continue;
            }
            let (col, line) = (col as u16, line as u16);

            // Matrix shimmer: mutate the glyph when the head lands on
            // a new cell (previous trail head differs) with a gate.
            if m.trail_len > 0 {
                let (prev_col, prev_line) = m.trail[(m.trail_len - 1) as usize];
                if (prev_col != col || prev_line != line)
                    && rand_chance.sample(rng) < crate::constants::FLUX_SHIMMER_CHANCE
                {
                    m.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
                }
            } else {
                m.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
            }

            let head_level = level_for_speed(m.vx, m.vy);
            draw_flux_cell(ctx, frame, col, line, m.ch, m.palette_slot, head_level);
            self.current_cells.push(FluxCell { col, line });

            // Comet trail: previously occupied cells, one brightness
            // step dimmer each, drawn only while in bounds.
            for t in 0..m.trail_len as usize {
                let (tc, tl) = m.trail[t];
                if tc >= ctx.cols || tl >= ctx.lines {
                    continue;
                }
                let depth = (m.trail_len as usize - t).min(3) as u8;
                let trail_level = step_down_level(head_level, depth);
                draw_flux_cell(ctx, frame, tc, tl, m.ch, m.palette_slot, trail_level);
                self.current_cells.push(FluxCell { col: tc, line: tl });
            }

            m.push_trail(col, line);
        }

        // Pass 2: generation-tag every drawn cell (monolith pattern —
        // u32 counter bump instead of clearing the array).
        self.drawn_gen_counter = self.drawn_gen_counter.wrapping_add(1);
        let gen = self.drawn_gen_counter;
        let need_len = ctx.cols as usize * lines_us.max(1);
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

    // -- Test-only diagnostics (mirrors the vortex *_for_test API) --

    #[cfg(test)]
    pub(crate) fn active_positions_for_test(&self) -> Vec<(f32, f32)> {
        self.motes
            .iter()
            .filter(|m| m.active)
            .map(|m| (m.x, m.y))
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn active_speeds_for_test(&self) -> Vec<(f32, f32)> {
        self.motes
            .iter()
            .filter(|m| m.active)
            .map(|m| (m.vx, m.vy))
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn sim_steps_for_test(&self) -> u64 {
        self.sim_steps
    }

    /// Steady-state active target hook (density contract tests).
    #[cfg(test)]
    pub(crate) fn target_for_test(lanes: usize, density: f32) -> usize {
        Self::target_active_count(lanes, density)
    }

    #[cfg(test)]
    pub(crate) fn drawn_cells_for_test(&self) -> &[FluxCell] {
        &self.current_cells
    }

    /// Direct mote write hook (builds targeted physics scenarios).
    #[cfg(test)]
    pub(crate) fn set_mote_for_test(&mut self, idx: usize, x: f32, y: f32, vx: f32, vy: f32) {
        let m = &mut self.motes[idx];
        m.active = true;
        m.x = x;
        m.y = y;
        m.vx = vx;
        m.vy = vy;
        m.age = 0.0;
        m.lifetime = 60.0;
        m.trail_len = 0;
        // Bookkeeping repair: a full recount keeps active_count exact
        // after direct writes (tests reset before use; this stays O(n)
        // and test-only).
        self.active_count = self.motes.iter().filter(|mm| mm.active).count();
    }
}

/// Pick a char from the pool via a uniform roll (defensive fallback
/// '0' for the degenerate empty-pool case — production always
/// initializes).
fn pick_pool_char(pool: &[char], rand_chance: &Uniform<f32>, rng: &mut StdRng) -> char {
    if pool.is_empty() {
        return '0';
    }
    let idx = (rand_chance.sample(rng) * pool.len() as f32) as usize;
    pool[idx.min(pool.len() - 1)]
}

/// Brightness grade by particle speed (screen units per second):
/// fast jets hot, mid swirls mid, calm drift ghost. The thresholds
/// sit against the gravity-driven terminal velocity of a falling
/// jet (tens of units per second) so the luminance maps the flow
/// structure the way Doppler imaging maps real fluids.
pub(crate) fn level_for_speed(vx: f32, vy: f32) -> BrightnessLevel {
    let speed = (vx * vx + vy * vy).sqrt();
    if speed > crate::constants::FLUX_BRIGHT_HOT {
        BrightnessLevel::Hot
    } else if speed > crate::constants::FLUX_BRIGHT_MID {
        BrightnessLevel::Mid
    } else {
        BrightnessLevel::Ghost
    }
}

/// Step a brightness level down (toward Ghost) by `depth` rungs.
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

/// Render one flux cell (palette-aware color + bold, mono-safe).
fn draw_flux_cell(
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
