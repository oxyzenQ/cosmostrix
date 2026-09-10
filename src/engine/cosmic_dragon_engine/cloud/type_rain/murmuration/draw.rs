// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The murmuration draw pass (NIGHT-research-7) — the
//! presentation half of the orchestration.
//!
//! Order of the passes: the birds (head cells + comet trails, the
//! kinetic ladder reading each bird's speed — fast edges bright,
//! slow cores dim, the black-hole infall's speed-graded read),
//! then the predator flash (one glyph at Core brightness inside
//! its window — the visible cause of the panic), then the
//! monolith drawn-cell diff cleanup (generation-tagged). Bird
//! glyphs re-roll matrix-style on cell crossing (the family
//! shimmer — the motion-gated mutation, never a timer).

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::constants::{MURM_SPEED_CORE, MURM_SPEED_GHOST, MURM_SPEED_MID};
use crate::frame::Frame;

use super::super::super::render::DrawCtx;
use super::super::monolith::monolith_helpers::{
    bold_for_level, clear_cell, clear_phosphor_metadata, color_for_level, pick_pool_char,
};
use super::super::monolith::{BrightnessLevel, MonolithCleanup};

use super::murmuration::{MurmCell, MurmurationRain};

impl MurmurationRain {
    /// Draw pass — the birds (heads + comet trails), then the
    /// predator flash, then the generation-tagged diff cleanup.
    pub(crate) fn draw(
        &mut self,
        ctx: &DrawCtx<'_>,
        frame: &mut Frame,
        cleanup: &mut MonolithCleanup<'_>,
        rng: &mut StdRng,
        rand_chance: &Uniform<f32>,
    ) {
        let lines_us = ctx.lines as usize;
        let lines_f = ctx.lines as f32;
        self.current_cells.clear();

        // Pass A — the birds: head cell + comet trail, the kinetic
        // ladder reading the speed (the wheeling flock reads as a
        // living gradient: fast bright, slow dim).
        for b in &mut self.birds {
            if !b.active {
                continue;
            }
            let col = b.x.round();
            let line = b.y.round();
            if col < 0.0 || line < 0.0 || col >= ctx.cols as f32 || line >= lines_f {
                // Off-screen: skip the draw AND the trail push
                // (the trail resumes from the last visible cell).
                continue;
            }
            let (col, line) = (col as u16, line as u16);

            // Matrix shimmer: mutate the glyph when the head lands
            // on a new cell.
            if b.trail_len() > 0 {
                if let Some((prev_col, prev_line)) = b.trail_cell(b.trail_len() as usize - 1) {
                    if (prev_col != col || prev_line != line)
                        && rand_chance.sample(rng) < crate::constants::MURM_SHIMMER_CHANCE
                    {
                        b.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
                    }
                }
            } else {
                b.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
            }

            let head_level = speed_level(b.speed(), b.panicked());
            draw_murm_cell(ctx, frame, col, line, b.ch, b.palette_slot, head_level);
            self.current_cells.push(MurmCell { col, line });

            // Comet trail: previously occupied cells (the flight
            // leaves them behind the head), one rung dimmer each.
            for t in 0..b.trail_len() as usize {
                if let Some((tc, tl)) = b.trail_cell(t) {
                    if tc >= ctx.cols || tl >= ctx.lines {
                        continue;
                    }
                    let depth = (b.trail_len() as usize - t).min(3) as u8;
                    let trail_level = step_down_level(head_level, depth);
                    draw_murm_cell(ctx, frame, tc, tl, b.ch, b.palette_slot, trail_level);
                    self.current_cells.push(MurmCell { col: tc, line: tl });
                }
            }

            b.push_trail(col, line);
        }

        // Pass B — the predator flash: one glyph at Core
        // brightness inside its window (the visible cause of the
        // panic — the raptor's silhouette where the scatter
        // began).
        if let Some((px, py, _)) = self.predator() {
            let col = px.round();
            let line = py.round();
            if (0.0..ctx.cols as f32).contains(&col) && (0.0..lines_f).contains(&line) {
                let (col, line) = (col as u16, line as u16);
                let ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
                draw_murm_cell(
                    ctx,
                    frame,
                    col,
                    line,
                    ch,
                    self.field_palette_slot,
                    BrightnessLevel::Core,
                );
                self.current_cells.push(MurmCell { col, line });
            }
        }

        // Pass C — the generation-tagged diff cleanup (monolith
        // pattern: u32 counter bump instead of clearing the array).
        self.drawn_gen_counter = self.drawn_gen_counter.wrapping_add(1);
        let gen = self.drawn_gen_counter;
        let need_len = self.birds.len().saturating_mul(lines_us.max(1));
        if self.drawn_gen.len() != need_len {
            self.drawn_gen.resize(need_len, 0);
        }
        for cell in &self.current_cells {
            let idx = cell.col as usize * lines_us + cell.line as usize;
            if idx < self.drawn_gen.len() {
                self.drawn_gen[idx] = gen;
            }
            // NIGHT-hunter-29 (the phosphor ownership rule — the
            // doc on clear_phosphor_metadata): a cell drawn this
            // frame is owned by the draw, so its phosphor state
            // is zeroed every frame — no ghost-vs-draw strobe at
            // the freeze/resume window (the black hole's owner
            // report generalized to the whole family tree).
            clear_phosphor_metadata(cleanup, cell.col, cell.line);
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
}

/// The kinetic ladder (the draw read): speed to brightness rung —
/// the flock reads as a living gradient. A panicked bird reads
/// Core during its flash window (the scatter burns).
pub(crate) fn speed_level(speed: f32, panicked: bool) -> BrightnessLevel {
    if panicked {
        return BrightnessLevel::Core;
    }
    if speed > MURM_SPEED_CORE {
        BrightnessLevel::Core
    } else if speed > MURM_SPEED_MID {
        BrightnessLevel::Hot
    } else if speed > MURM_SPEED_GHOST {
        BrightnessLevel::Mid
    } else {
        BrightnessLevel::Ghost
    }
}

/// Step a brightness level down (toward Ghost) by `depth` ladder
/// rungs (mirrors the family's `step_down_level`).
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

/// Render one murmuration cell (palette-aware color + bold,
/// mono-safe).
fn draw_murm_cell(
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
