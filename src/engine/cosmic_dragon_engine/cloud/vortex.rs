// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Polar-orbit rain for the vortex scene (task-18, third rain style).
//!
//! Motion model — 100% distinct DNA from `cinematic` (column cascade)
//! and `monolith` (segmented pillars): each mote is a glyph in polar
//! coordinates `(angle, radius)` spiraling inward toward the screen
//! center. Angular speed follows a Keplerian profile `omega ∝ 1/r`
//! (constant cells/sec along the orbit), so the rim rotates slowly and
//! the core spins fast — the "drain" look. Spawn angles are biased
//! toward 3 slowly-precessing arm centers; differential rotation then
//! shears those radial arms into living spirals within a few seconds.
//!
//! Motes are absorbed at the core (`radius <= VORTEX_CORE_R`, the
//! "event horizon") and respawn at the rim, giving a perpetual galaxy
//! drain. Trails (last `VORTEX_TRAIL_LEN` cell positions per mote)
//! render as dimming comet streaks.
//!
//! LOC note: this file exceeds the 500-line soft target (~560) as a
//! single self-contained style system (state, spawn, advance, draw and
//! diff cleanup are one algorithm; splitting them mirrors the monolith
//! family split only once the file approaches the 800 hard cap). Well
//! under the hard limit.
//!
//! Cleanup follows the monolith three-pass diff pattern: draw into
//! `current_cells`, tag with the `drawn_gen` generation counter, then
//! clear only previous cells NOT redrawn this frame (phosphor metadata
//! and frame blank). Cells here play the same role monolith's Segment
//! cells do; there is no spine equivalent, so no extra phosphor pass.

use std::time::{Duration, Instant};

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::frame::Frame;

use super::monolith::BrightnessLevel;
use super::monolith_helpers::{bold_for_level, clear_cell, color_for_level};
use super::render::DrawCtx;

/// Trail depth per mote (comet streak length in cells).
pub(crate) const VORTEX_TRAIL_LEN: usize = 4;

/// One drawn cell (col, line). Own struct instead of reusing monolith's
/// `DrawnCell` because vortex has no Segment/Spine kind distinction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct VortexCell {
    pub(crate) col: u16,
    pub(crate) line: u16,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct VortexMote {
    pub(crate) active: bool,
    /// Orbit angle in radians (unbounded accumulator — wraps naturally
    /// via f32 precision; 2π wrap handled by trig, not by the value).
    pub(crate) angle: f32,
    /// Normalized orbit radius: 0.0 = center, 1.0 = rim. Slightly above
    /// 1.0 at spawn (motes clip into view as they drift inward).
    pub(crate) radius: f32,
    /// Per-mote angular velocity multiplier (0.85..1.15).
    pub(crate) spin: f32,
    /// Per-mote inward drift multiplier (0.8..1.25).
    pub(crate) fall: f32,
    /// Glyph carried by the mote; re-rolled matrix-style when the head
    /// crosses into a new cell (see the shimmer gate in `draw`).
    pub(crate) ch: char,
    /// Palette slot adopted at spawn / palette transition.
    pub(crate) palette_slot: u8,
    /// Ring buffer of the last `VORTEX_TRAIL_LEN` head cell positions.
    /// Shift-left layout: index 0 = oldest, len-1 = most recent.
    trail: [(u16, u16); VORTEX_TRAIL_LEN],
    trail_len: u8,
}

impl VortexMote {
    const fn vacant() -> Self {
        Self {
            active: false,
            angle: 0.0,
            radius: 0.0,
            spin: 1.0,
            fall: 1.0,
            ch: '0',
            palette_slot: 0,
            trail: [(0, 0); VORTEX_TRAIL_LEN],
            trail_len: 0,
        }
    }

    fn push_trail(&mut self, col: u16, line: u16) {
        if self.trail_len as usize >= VORTEX_TRAIL_LEN {
            for i in 0..VORTEX_TRAIL_LEN - 1 {
                self.trail[i] = self.trail[i + 1];
            }
            self.trail[VORTEX_TRAIL_LEN - 1] = (col, line);
        } else {
            let idx = self.trail_len as usize;
            self.trail[idx] = (col, line);
            self.trail_len += 1;
        }
    }
}

/// Spawn inputs (mirrors `MonolithSpawnParams` — bundle keeps clippy's
/// `too_many_arguments` threshold respected at the call site).
pub(crate) struct VortexSpawnParams {
    pub(crate) cols: u16,
    pub(crate) lines: u16,
    pub(crate) density: f32,
    pub(crate) active_palette_slot: u8,
    pub(crate) spawn_scale: f32,
}

/// RNG bundle (mirrors `MonolithRandom`).
pub(crate) struct VortexRandom<'a> {
    pub(crate) rng: &'a mut StdRng,
    pub(crate) rand_chance: &'a Uniform<f32>,
}

/// Per-frame step inputs for the advance pass. Viewport geometry is
/// NOT carried here — the draw pass derives it from DrawCtx (the
/// authoritative frame dimensions); advance only needs time + speed.
pub(crate) struct VortexStep {
    pub(crate) now: Instant,
    /// chars_per_sec already multiplied by the terminal speed_mult.
    pub(crate) chars_per_sec: f32,
    pub(crate) max_sim_delta: Duration,
    pub(crate) resume_blend: f32,
}

pub(crate) struct VortexRain {
    pub(crate) motes: Vec<VortexMote>,
    active_count: usize,
    /// Rotating scan cursor for amortized O(1) free-slot search.
    spawn_scan_idx: usize,
    /// Arm center precession phase (radians) — advances slowly so the
    /// spiral arms drift around the screen instead of staying pinned.
    arm_phase: f32,
    /// Global motion clock. dt = now - last_step clamped by max_sim_delta;
    /// a fully-paused run simply stops integrating (rain_at early-return),
    /// so no pause-time shift is required (unlike per-droplet last_time).
    last_step: Option<Instant>,
    current_cells: Vec<VortexCell>,
    previous_cells: Vec<VortexCell>,
    drawn_gen: Vec<u32>,
    drawn_gen_counter: u32,
}

impl VortexRain {
    pub(crate) fn new() -> Self {
        Self {
            motes: Vec::new(),
            active_count: 0,
            spawn_scan_idx: 0,
            arm_phase: 0.0,
            last_step: None,
            current_cells: Vec::new(),
            previous_cells: Vec::new(),
            drawn_gen: Vec::new(),
            drawn_gen_counter: 0,
        }
    }

    /// Rebuild the mote pool for a new viewport (or style entry). The pool
    /// is sized one mote per column (mirroring monolith's lane model); the
    /// active target is a density-driven ratio of that pool.
    pub(crate) fn reset(&mut self, cols: u16) {
        let lanes = cols.max(1) as usize;
        self.motes.clear();
        self.motes.resize_with(lanes, VortexMote::vacant);
        self.active_count = 0;
        self.spawn_scan_idx = 0;
        self.arm_phase = 0.0;
        self.last_step = None;
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
        let ratio = (crate::constants::VORTEX_ACTIVE_BASE
            + density.clamp(0.01, 5.0) * crate::constants::VORTEX_ACTIVE_DENSITY_MULT)
            .clamp(0.02, crate::constants::VORTEX_ACTIVE_MAX);
        ((lanes as f32 * ratio).round() as usize).clamp(1, lanes)
    }

    /// Amortized free-slot scan (rotating cursor, mirrors monolith's
    /// `find_inactive_lane` minus the mouse-lane avoidance — vortex motes
    /// are transient, not lane-bound).
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

    /// Spawn pass — accumulator pattern identical to `MonolithRain::spawn`
    /// (deficit-bounded budget + fractional remainder carry).
    pub(crate) fn spawn(
        &mut self,
        elapsed: Duration,
        spawn_remainder: &mut f32,
        params: &VortexSpawnParams,
        random: &mut VortexRandom<'_>,
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
        let spawn_rate = (target as f32 * crate::constants::VORTEX_SPAWN_RATE_MULT
            + crate::constants::VORTEX_SPAWN_RATE_FLOOR)
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

        let pool = random.rand_chance.sample(random.rng);
        for _ in 0..to_spawn {
            let Some(idx) = self.find_inactive_mote() else {
                break;
            };
            self.activate_mote(
                idx,
                params.active_palette_slot,
                pool,
                random.rand_chance,
                random.rng,
            );
            self.active_count += 1;
        }
    }

    /// Activate a vacant mote at the rim with an arm-biased angle.
    ///
    /// Arm bias: `arm_phase` precesses slowly; each spawn picks the arm
    /// nearest a uniform roll (mod 3) and offsets within
    /// ±VORTEX_ARM_SPREAD radians. Differential rotation shears the
    /// resulting radial concentrations into spirals.
    fn activate_mote(
        &mut self,
        idx: usize,
        palette_slot: u8,
        pool_roll: f32,
        rand_chance: &Uniform<f32>,
        rng: &mut StdRng,
    ) {
        let arm = (pool_roll * crate::constants::VORTEX_ARMS as f32).floor();
        let arm_center =
            self.arm_phase + arm * std::f32::consts::TAU / crate::constants::VORTEX_ARMS as f32;
        let spread = (rand_chance.sample(rng) - 0.5) * 2.0 * crate::constants::VORTEX_ARM_SPREAD;

        // Rim entry just outside the visible radius (clips in immediately).
        let radius = 1.0 + rand_chance.sample(rng) * crate::constants::VORTEX_RIM_JITTER;
        let spin = 0.85 + rand_chance.sample(rng) * 0.30;
        let fall = 0.80 + rand_chance.sample(rng) * 0.45;

        let m = &mut self.motes[idx];
        m.active = true;
        m.angle = arm_center + spread;
        m.radius = radius;
        m.spin = spin;
        m.fall = fall;
        m.palette_slot = palette_slot;
        m.trail_len = 0;
    }

    /// Motion pass — the polar physics core.
    ///
    /// `omega(r) = K / max(r, VORTEX_MIN_R) * spin * speed_scale`:
    /// Keplerian differential rotation. The inward drift accelerates
    /// slightly toward the core (`fall` profile), and motes below
    /// `VORTEX_CORE_R` are absorbed (deactivated → free slot).
    pub(crate) fn advance(&mut self, step: &VortexStep) {
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

        // Radial journey speed: chars_per_sec mapped onto normalized
        // radius units via VORTEX_JOURNEY_ROWS (a full rim→core trip at
        // cps=1 takes VORTEX_JOURNEY_ROWS seconds — same semantics as
        // rows/sec for column-falling styles).
        let vr = step.chars_per_sec.max(0.0) / crate::constants::VORTEX_JOURNEY_ROWS;

        let mut absorbed = 0usize;
        for m in &mut self.motes {
            if !m.active {
                continue;
            }
            // Keplerian angular speed: constant cells/sec along the orbit
            // (v = omega * r * max_rx = K * max_rx — independent of r).
            let r_safe = m.radius.max(crate::constants::VORTEX_MIN_R);
            let omega = crate::constants::VORTEX_KEPLER_K / r_safe
                * m.spin
                * crate::constants::VORTEX_SPEED_SCALE;
            // Inward drift with a mild core acceleration.
            let inward = vr
                * (crate::constants::VORTEX_FALL_BASE
                    + crate::constants::VORTEX_FALL_CORE_BOOST * (1.0 - m.radius.clamp(0.0, 1.0)))
                * m.fall;

            m.angle += omega * dt;
            m.radius -= inward * dt;

            if m.radius <= crate::constants::VORTEX_CORE_R {
                m.active = false;
                absorbed += 1;
                m.trail_len = 0;
            }
        }
        if absorbed > 0 {
            self.active_count = self.active_count.saturating_sub(absorbed);
        }

        // Slow arm precession so the spiral pattern drifts around the rim.
        self.arm_phase += crate::constants::VORTEX_ARM_PRECESSION * dt;
    }

    /// Draw pass — head cell + comet trail + monolith-style diff cleanup.
    ///
    /// Matrix-style glyph mutation: when the head crosses into a new cell,
    /// the glyph re-rolls with probability VORTEX_SHIMMER_CHANCE. This is
    /// the authentic matrix-rain shimmer (mutation tied to motion), not a
    /// timer — it stays deterministic under the bench's uniform stepping.
    pub(crate) fn draw(
        &mut self,
        ctx: &DrawCtx<'_>,
        frame: &mut Frame,
        cleanup: &mut super::monolith::MonolithCleanup<'_>,
        rng: &mut StdRng,
        rand_chance: &Uniform<f32>,
    ) {
        let lines_us = ctx.lines as usize;
        let cols_f = ctx.cols as f32;
        let lines_f = ctx.lines as f32;
        // Screen-circular orbit: terminal cells are ~1:2 (w:h), so a circle
        // on the physical screen is an ellipse in cell space with the
        // vertical semi-axis halved.
        let max_rx = cols_f * 0.5;
        let max_ry = lines_f * 0.5;
        let cx = max_rx - 0.5;
        let cy = max_ry - 0.5;

        self.current_cells.clear();
        for m in &mut self.motes {
            if !m.active {
                continue;
            }
            let cos_a = m.angle.cos();
            let sin_a = m.angle.sin();
            let col_f = cx + m.radius * cos_a * max_rx;
            let line_f = cy + m.radius * sin_a * max_ry;
            let col = col_f.round() as i32;
            let line = line_f.round() as i32;
            if col < 0 || line < 0 || col >= ctx.cols as i32 || line >= ctx.lines as i32 {
                continue;
            }
            let (col, line) = (col as u16, line as u16);

            // Matrix shimmer: mutate the glyph when the head lands on a
            // new cell (previous trail head differs) with a chance gate.
            if m.trail_len > 0 {
                let (prev_col, prev_line) = m.trail[(m.trail_len - 1) as usize];
                if (prev_col != col || prev_line != line)
                    && rand_chance.sample(rng) < crate::constants::VORTEX_SHIMMER_CHANCE
                {
                    m.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
                }
            } else {
                m.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
            }

            let head_level = level_for_radius(m.radius);
            draw_vortex_cell(ctx, frame, col, line, m.ch, m.palette_slot, head_level);
            self.current_cells.push(VortexCell { col, line });

            // Comet trail: previously occupied cells, one brightness step
            // dimmer each, drawn only while in bounds.
            for t in 0..m.trail_len as usize {
                let (tc, tl) = m.trail[t];
                if tc >= ctx.cols || tl >= ctx.lines {
                    continue;
                }
                let depth = (m.trail_len as usize - t).min(4) as u8;
                let trail_level = step_down_level(head_level, depth);
                draw_vortex_cell(ctx, frame, tc, tl, m.ch, m.palette_slot, trail_level);
                self.current_cells.push(VortexCell { col: tc, line: tl });
            }

            m.push_trail(col, line);
        }

        // Pass 2: generation-tag every drawn cell (monolith pattern —
        // u32 counter bump instead of clearing the array).
        self.drawn_gen_counter = self.drawn_gen_counter.wrapping_add(1);
        let gen = self.drawn_gen_counter;
        let need_len = self.motes.len().saturating_mul(lines_us.max(1));
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

    // -- Test-only diagnostics (mirrors monolith's *_for_test API) --

    #[cfg(test)]
    pub(crate) fn active_radii_for_test(&self) -> Vec<f32> {
        self.motes
            .iter()
            .filter(|m| m.active)
            .map(|m| m.radius)
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn drawn_cells_for_test(&self) -> &[VortexCell] {
        &self.current_cells
    }
}

/// Pick a char from the pool via a uniform roll (defensive fallback '0'
/// for the degenerate empty-pool case — production always initializes).
fn pick_pool_char(pool: &[char], rand_chance: &Uniform<f32>, rng: &mut StdRng) -> char {
    if pool.is_empty() {
        return '0';
    }
    let idx = (rand_chance.sample(rng) * pool.len() as f32) as usize;
    pool[idx.min(pool.len() - 1)]
}

/// Brightness zone by normalized radius: rim dim → core hot. The three
/// zone boundaries give the drain a visible luminance gradient even in
/// Color16 mode (palette index selection, not blend math).
pub(crate) fn level_for_radius(radius: f32) -> BrightnessLevel {
    if radius > 0.66 {
        BrightnessLevel::Ghost
    } else if radius > 0.33 {
        BrightnessLevel::Mid
    } else if radius > crate::constants::VORTEX_CORE_R {
        BrightnessLevel::Hot
    } else {
        BrightnessLevel::Core
    }
}

/// Step a brightness level down (toward Ghost) by `depth` ladder rungs.
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

/// Render one vortex cell (palette-aware color + bold, mono-safe).
fn draw_vortex_cell(
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
