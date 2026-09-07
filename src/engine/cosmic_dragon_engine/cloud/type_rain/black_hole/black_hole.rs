// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Black hole rain for the sorgonemous_intrascals scene (NIGHT-special-1,
//! the eighth rain style — stage 1: the event-horizon ball).
//!
//! Motion DNA — 100% distinct from every existing style: the screen hosts
//! a single gravitating body, not a particle field. Stage 1 renders the
//! body itself: a medium round ball centered on the viewport with a black
//! empty core (the event horizon) and a bright photon-ring rim that fades
//! outward into the dark. The visual reference is the intro logo emblem —
//! a density-shaded round mark — re-expressed through the engine's own
//! brightness ladder and the active charset pool.
//!
//! Geometry: terminal cells are roughly 1:2 (width:height), so a circle
//! on the physical screen is an ellipse in cell space. All radius math
//! runs in line-height units: a cell offset (dx cols, dy lines) sits at
//! screen distance sqrt((dx / 2)^2 + dy^2). The ball outer radius is a
//! fraction of the viewport's limiting half-extent, so it scales with any
//! screen size from 80x24 to 400x100 without touching the constants.
//!
//! Radial brightness: the annulus between the core radius and the outer
//! radius is banded like the vortex drain, but inverted — the brightest
//! zone hugs the event horizon (the accretion photon ring) and dims
//! toward the outer rim, the way a real black hole silhouette reads:
//! a dark core wrapped in a thin blazing edge.
//!
//! Stage roadmap (owner-approved staged rollout, one commit per stage):
//! - stage 1 (this file): the ball — owner visual verification.
//! - stage 2: the orbital ring — 3D ring particles integrated with the
//!   same RK4 machinery the lorenz style ships (the integrator is
//!   attractor-agnostic; see `type_rain/lorenz/lorenz.rs`).
//! - stage 3: glyph rain infall — falling glyphs that bend elegantly
//!   into the core when they approach the ring's capture radius.
//!
//! The glyph pool for each cell re-rolls through a low-probability
//! shimmer gate (matrix DNA: mutation is the engine's life sign), and a
//! full re-roll is armed whenever the charset changes so the ball never
//! carries stale glyphs from a previous pool.
//!
//! Cleanup follows the monolith/vortex three-pass diff pattern: draw
//! into `current_cells`, tag with the `drawn_gen` generation counter,
//! then clear only previous cells NOT redrawn this frame (phosphor
//! metadata and frame blank). The stage-1 ball is static, so every cell
//! is redrawn each frame and the diff is a no-op; the pattern is kept
//! because stage 2's moving ring and stage 3's infalling glyphs will
//! vacate cells and need exactly this cleanup.

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::frame::Frame;

use super::super::super::render::DrawCtx;
use super::super::monolith::monolith_helpers::{
    bold_for_level, clear_cell, color_for_level, pick_pool_char,
};
use super::super::monolith::{BrightnessLevel, MonolithCleanup};

/// One drawn ball cell: grid position plus its radial brightness band.
/// Own struct instead of reusing monolith's `DrawnCell` because the ball
/// has no Segment/Spine kind distinction (same shape as `VortexCell`,
/// plus the precomputed band level — the band is static geometry, so it
/// is computed once at reset instead of per frame).
#[derive(Clone, Copy, Debug)]
pub(crate) struct BlackHoleCell {
    pub(crate) col: u16,
    pub(crate) line: u16,
    pub(crate) level: BrightnessLevel,
}

/// Cell-space aspect divisor: terminal cells are ~1:2 (width:height), so
/// one column of travel covers half a line-height of screen distance.
const CELL_ASPECT_DIVISOR: f32 = 2.0;

/// Shorthand rank for the brightness ladder (Ghost = 0 ... Core = 4).
/// BrightnessLevel carries no PartialEq, so band tests compare ranks.
#[cfg(test)]
pub(crate) fn level_rank(level: BrightnessLevel) -> u8 {
    match level {
        BrightnessLevel::Ghost => 0,
        BrightnessLevel::Dim => 1,
        BrightnessLevel::Mid => 2,
        BrightnessLevel::Hot => 3,
        BrightnessLevel::Core => 4,
    }
}

pub(crate) struct BlackHoleRain {
    /// Precomputed ring annulus cells (geometry is static at stage 1, so
    /// it is built once per reset instead of rescanned per frame).
    ring_cells: Vec<BlackHoleCell>,
    /// Parallel glyph per ring cell, drawn from the active charset pool.
    glyphs: Vec<char>,
    /// Set when the glyph pool may have changed (charset switch, style
    /// entry); the next draw re-rolls every glyph from the live pool.
    glyphs_stale: bool,
    /// Palette slot adopted at entry / palette transition (the ball is a
    /// single body, so one slot covers every cell).
    palette_slot: u8,
    /// Last frame's drawn cells (diff cleanup input).
    previous_cells: Vec<BlackHoleCell>,
    /// This frame's drawn cells (diff cleanup output).
    current_cells: Vec<BlackHoleCell>,
    /// Generation tags for the diff cleanup pass (flat: col * lines +
    /// line), rebuilt when the viewport dimensions change.
    drawn_gen: Vec<u32>,
    drawn_gen_counter: u32,
    drawn_gen_dims: (u16, u16),
}

impl BlackHoleRain {
    pub(crate) fn new() -> Self {
        Self {
            ring_cells: Vec::new(),
            glyphs: Vec::new(),
            glyphs_stale: true,
            palette_slot: 0,
            previous_cells: Vec::new(),
            current_cells: Vec::new(),
            drawn_gen: Vec::new(),
            drawn_gen_counter: 0,
            drawn_gen_dims: (0, 0),
        }
    }

    /// Rebuild the ball geometry for a new viewport (or style entry).
    ///
    /// The outer radius is a fraction of the viewport's limiting
    /// half-extent (line-height units): `unit = min(cols / 4, lines / 2)`
    /// — `cols / 4` is the half-width expressed in line units through the
    /// cell aspect, `lines / 2` the half-height. A medium ball reads at
    /// roughly a third of the screen's short axis on every terminal size.
    pub(crate) fn reset(&mut self, cols: u16, lines: u16) {
        self.ring_cells.clear();
        if cols == 0 || lines == 0 {
            self.glyphs.clear();
            self.glyphs_stale = true;
            self.clear_draw_history();
            return;
        }

        let unit = (cols as f32 / (CELL_ASPECT_DIVISOR * 2.0)).min(lines as f32 / 2.0);
        let outer_r = unit * crate::constants::BLACK_HOLE_BALL_FRACTION;
        let core_r = outer_r * crate::constants::BLACK_HOLE_CORE_FRACTION;

        // Degenerate viewport guard: a ball thinner than one cell of
        // annulus width draws nothing (prevents a zero-width division
        // below and an empty-shell flash on tiny terminals).
        if outer_r - core_r < 1.0 {
            self.glyphs.clear();
            self.glyphs_stale = true;
            self.clear_draw_history();
            return;
        }

        // Integer center cell (the grid's discrete midpoint). Even
        // dimensions land the true center between cells; (n - 1) / 2
        // picks the lower-mid cell so the scan offsets are exact
        // integers and the distance math stays consistent with the
        // rasterized geometry.
        let cx = ((cols - 1) / 2) as i32;
        let cy = ((lines - 1) / 2) as i32;
        let half_w_cells = (outer_r * CELL_ASPECT_DIVISOR).ceil() as i32;
        let half_h_cells = outer_r.ceil() as i32;
        let annulus_width = outer_r - core_r;

        for dy in -half_h_cells..=half_h_cells {
            for dx in -half_w_cells..=half_w_cells {
                let col = (cx + dx) as u16;
                let line = (cy + dy) as u16;
                if col >= cols || line >= lines {
                    continue;
                }
                let dist = ((dx as f32 / CELL_ASPECT_DIVISOR).powi(2) + (dy as f32).powi(2)).sqrt();
                if dist < core_r || dist > outer_r {
                    continue;
                }
                let t = ((dist - core_r) / annulus_width).clamp(0.0, 1.0);
                self.ring_cells.push(BlackHoleCell {
                    col,
                    line,
                    level: level_for_ring_band(t),
                });
            }
        }

        self.glyphs.clear();
        self.glyphs.resize_with(self.ring_cells.len(), || '0');
        self.glyphs_stale = true;
        self.clear_draw_history();
    }

    /// Steady-state drawn-glyph count (the HUD active metric). The ball
    /// is the whole visual at stage 1, so its cell count is the honest
    /// active figure.
    pub(crate) fn active_count(&self) -> usize {
        self.ring_cells.len()
    }

    /// Palette transition completion: the ball adopts the new slot.
    pub(crate) fn adopt_palette_slot(&mut self, palette_slot: u8) {
        self.palette_slot = palette_slot;
    }

    /// Drop the diff-cleanup history and arm a full glyph re-roll
    /// (semantic invalidation: charset switch, palette change, forced
    /// redraw). The next draw pass rebuilds the baseline from scratch.
    pub(crate) fn clear_draw_history(&mut self) {
        self.previous_cells.clear();
        self.current_cells.clear();
        self.drawn_gen.clear();
        self.drawn_gen_counter = 0;
        self.drawn_gen_dims = (0, 0);
        self.glyphs_stale = true;
    }

    /// Draw pass — full annulus render + monolith-style diff cleanup.
    ///
    /// Stage 1 motion budget: every ring cell is redrawn each frame (the
    /// ball is static), so the diff pass is a no-op today; it exists for
    /// stage 2 (the moving ring) and stage 3 (infalling glyphs), which
    /// will vacate cells and rely on exactly this cleanup contract.
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
        self.current_cells.reserve(self.ring_cells.len());

        // Charset switches and style entries arm a full re-roll so the
        // ball never carries glyphs from a stale pool.
        if self.glyphs_stale {
            for g in &mut self.glyphs {
                *g = pick_pool_char(ctx.char_pool, rand_chance, rng);
            }
            self.glyphs_stale = false;
        }

        for (idx, cell) in self.ring_cells.iter().enumerate() {
            if cell.col >= ctx.cols || cell.line >= ctx.lines {
                // Viewport shrank without a reset (live resize window):
                // skip out-of-bounds geometry this frame; the resize
                // handler calls reset() and rebuilds the annulus.
                continue;
            }
            // Matrix shimmer: low-probability glyph mutation is the
            // engine's life sign (every style carries one); the ball
            // stays calm, a slow surface flicker at the event horizon.
            if self.glyphs.len() > idx
                && rand_chance.sample(rng) < crate::constants::BLACK_HOLE_SHIMMER_CHANCE
            {
                self.glyphs[idx] = pick_pool_char(ctx.char_pool, rand_chance, rng);
            }
            let ch = if self.glyphs.len() > idx {
                self.glyphs[idx]
            } else {
                '0'
            };
            draw_ball_cell(
                ctx,
                frame,
                cell.col,
                cell.line,
                ch,
                self.palette_slot,
                cell.level,
            );
            self.current_cells.push(*cell);
        }

        // Pass 2: generation-tag every drawn cell (monolith pattern —
        // u32 counter bump instead of clearing the array). Rebuilt when
        // the viewport dimensions change; sized to the full grid because
        // the flat index col * lines + line spans exactly cols * lines.
        let need_dims = (ctx.cols, ctx.lines);
        let need_len = ctx.cols as usize * lines_us.max(1);
        if self.drawn_gen_dims != need_dims || self.drawn_gen.len() != need_len {
            self.drawn_gen.resize(need_len, 0);
            self.drawn_gen_dims = need_dims;
        }
        self.drawn_gen_counter = self.drawn_gen_counter.wrapping_add(1);
        let gen = self.drawn_gen_counter;
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

    // -- Test-only diagnostics (mirrors the monolith/vortex *_for_test API) --

    #[cfg(test)]
    pub(crate) fn ring_cells_for_test(&self) -> &[BlackHoleCell] {
        &self.ring_cells
    }

    #[cfg(test)]
    pub(crate) fn drawn_cells_for_test(&self) -> &[BlackHoleCell] {
        &self.previous_cells
    }
}

/// Radial brightness band for the annulus: `t` is the normalized radial
/// position, 0.0 at the core edge (event horizon) and 1.0 at the outer
/// rim. Inverted from the vortex drain — the photon ring hugs the hole:
/// the innermost band is Core (brightest), fading outward to Ghost.
pub(crate) fn level_for_ring_band(t: f32) -> BrightnessLevel {
    if t < 0.18 {
        BrightnessLevel::Core
    } else if t < 0.45 {
        BrightnessLevel::Hot
    } else if t < 0.75 {
        BrightnessLevel::Mid
    } else {
        BrightnessLevel::Ghost
    }
}

/// Render one ball cell (palette-aware color + bold, mono-safe).
fn draw_ball_cell(
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
