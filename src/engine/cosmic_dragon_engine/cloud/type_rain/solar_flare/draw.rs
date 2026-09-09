// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The solar flare draw pass (NIGHT-special-4) — the presentation
//! half of the orchestration, split from `solar_flare.rs` to honor
//! the 800-LOC hard cap (the monolith family's `monolith_glyphs.rs`
//! split pattern: the state machine keeps the physics, this file
//! keeps the render).
//!
//! Order of the passes (see `solar_flare/mod.rs` law 5): the
//! granulation surface first (granule ladder + the footpoint glow
//! overlay), then the arcs (parabola filaments with the phase-aware
//! ladders — Emerging fades in, Erupting burns Core-then-Hot,
//! Detaching dims while it lifts), then the rain (riding heads +
//! comet trails + ejecta sparks), then the monolith drawn-cell diff
//! cleanup (generation-tagged). Field cells keep the glyph the
//! frame already carries (the fabric identity — the static arc
//! body never re-dirties its cells).

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::frame::Frame;

use crate::constants::{
    SOLAR_FLASH_SECS, SOLAR_FLUX_LEVEL_HOT, SOLAR_SHIMMER_ARC, SOLAR_SHIMMER_HOT,
    SOLAR_SHIMMER_SURFACE,
};

use super::super::super::render::DrawCtx;
use super::super::monolith::monolith_helpers::{
    bold_for_level, clear_cell, color_for_level, pick_pool_char,
};
use super::super::monolith::{BrightnessLevel, MonolithCleanup};

use super::loops;
use super::solar_flare::{SolarCell, SolarFlareRain};

impl SolarFlareRain {
    /// Draw pass — the surface first (granulation + footpoint glow),
    /// then the arcs (parabola filaments, phase-aware ladders),
    /// then the rain (riding heads + comet trails + ejecta), then
    /// the monolith drawn-cell diff cleanup (generation-tagged).
    ///
    /// Arc cells: the loop's ladder reads the flux (law 3); the
    /// footpoint cells step up while the flash is fresh (the landing
    /// punch); the apex cells step up while the loop runs Hot or
    /// brighter (the condensation glow); Emerging arcs fade in;
    /// Detaching arcs dim to nothing while they lift. Field cells
    /// keep the glyph the frame already carries (law 5's fabric
    /// identity — the static arc body never re-dirties its cells)
    /// and re-roll at the shimmer rate, hot loops faster.
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

        let surface_top = self.arcade.surface_top();
        let surface_top_f = surface_top as f32;

        // Pass A — the granulation surface (law 5): the top line
        // reads the granule ladder, the second line one rung dimmer.
        for col in 0..ctx.cols {
            let heat = self.arcade.granule_heat(col as usize);
            let level = loops::granule_level(heat);
            let ch = fabric_glyph(
                ctx,
                frame,
                col,
                surface_top,
                SOLAR_SHIMMER_SURFACE,
                rng,
                rand_chance,
            );
            draw_solar_cell(
                ctx,
                frame,
                SolarCellPaint {
                    col,
                    line: surface_top,
                    ch,
                    palette_slot: self.field_palette_slot,
                    level,
                    factor: 0.6 + heat * 0.4,
                },
            );
            self.current_cells.push(SolarCell {
                col,
                line: surface_top,
            });
            if surface_top + 1 < ctx.lines {
                let dim = step_down_level(level, 1);
                let ch = fabric_glyph(
                    ctx,
                    frame,
                    col,
                    surface_top + 1,
                    SOLAR_SHIMMER_SURFACE,
                    rng,
                    rand_chance,
                );
                draw_solar_cell(
                    ctx,
                    frame,
                    SolarCellPaint {
                        col,
                        line: surface_top + 1,
                        ch,
                        palette_slot: self.field_palette_slot,
                        level: dim,
                        factor: 0.5 + heat * 0.3,
                    },
                );
                self.current_cells.push(SolarCell {
                    col,
                    line: surface_top + 1,
                });
            }
        }

        // Pass A2 — the footpoint glow: the loop feet sit ON the
        // surface; their cells read the loop ladder, one rung hotter
        // while the flash is fresh (the landing punch, law 3).
        for lp in self.arcade.loops() {
            if lp.h < 0.5 || lp.phase == loops::LoopPhase::Detaching {
                continue;
            }
            let foot_level = loops::loop_level(lp.flux, lp.flare_age);
            for s_foot in [0.0_f32, 1.0_f32] {
                let col = lp
                    .arc_x(s_foot)
                    .round()
                    .clamp(0.0, (ctx.cols as f32 - 1.0).max(0.0)) as u16;
                let boosted = if lp.flare_age < SOLAR_FLASH_SECS {
                    step_up_level(foot_level)
                } else {
                    foot_level
                };
                let shimmer = if matches!(boosted, BrightnessLevel::Hot | BrightnessLevel::Core) {
                    SOLAR_SHIMMER_HOT
                } else {
                    SOLAR_SHIMMER_ARC
                };
                let ch = fabric_glyph(ctx, frame, col, surface_top, shimmer, rng, rand_chance);
                draw_solar_cell(
                    ctx,
                    frame,
                    SolarCellPaint {
                        col,
                        line: surface_top,
                        ch,
                        palette_slot: self.field_palette_slot,
                        level: boosted,
                        factor: 1.0,
                    },
                );
                self.current_cells.push(SolarCell {
                    col,
                    line: surface_top,
                });
            }
        }

        // Pass B — the arcs: each loop expands into its parabola of
        // glyphs (~one cell per step along the arc).
        for lp in self.arcade.loops() {
            if lp.h < 0.5 {
                continue;
            }
            let base = match lp.phase {
                loops::LoopPhase::Erupting => {
                    // The flare: the whole arc reads Core for the
                    // flash window, then Hot (law 4).
                    if lp.phase_clock < SOLAR_FLASH_SECS {
                        BrightnessLevel::Core
                    } else {
                        BrightnessLevel::Hot
                    }
                }
                _ => loops::loop_level(lp.flux, lp.flare_age),
            };
            let fade = match lp.phase {
                loops::LoopPhase::Emerging => {
                    (lp.phase_clock / crate::constants::SOLAR_EMERGE_SECS).clamp(0.0, 1.0) * 0.75
                        + 0.25
                }
                loops::LoopPhase::Detaching => {
                    (1.0 - lp.phase_clock / crate::constants::SOLAR_DETACH_SECS).clamp(0.0, 1.0)
                }
                _ => 1.0,
            };
            let steps = ((lp.w + 2.0 * (lp.h + lp.lift)) as usize).max(4);
            let shimmer = if matches!(base, BrightnessLevel::Hot | BrightnessLevel::Core) {
                SOLAR_SHIMMER_HOT
            } else {
                SOLAR_SHIMMER_ARC
            };
            for i in 0..=steps {
                let s = i as f32 / steps.max(1) as f32;
                let x = lp.arc_x(s);
                let y = surface_top_f - lp.arc_height(s);
                let col = x.round().clamp(0.0, (ctx.cols as f32 - 1.0).max(0.0)) as u16;
                let line = y.round().clamp(0.0, (ctx.lines as f32 - 1.0).max(0.0)) as u16;
                // Foot cells: the landing punch zone (two cells each
                // side). Apex cells: the condensation glow zone while
                // Hot or brighter.
                let near_foot = i <= 1 || i + 1 >= steps;
                let near_apex = (s - 0.5).abs() <= 2.0 / steps.max(1) as f32;
                let mut level = base;
                let foot_flash = near_foot
                    && lp.flare_age < SOLAR_FLASH_SECS
                    && lp.phase != loops::LoopPhase::Erupting;
                let apex_glow = near_apex && lp.flux > SOLAR_FLUX_LEVEL_HOT;
                if foot_flash || apex_glow {
                    level = step_up_level(level);
                }
                let ch = fabric_glyph(ctx, frame, col, line, shimmer, rng, rand_chance);
                draw_solar_cell(
                    ctx,
                    frame,
                    SolarCellPaint {
                        col,
                        line,
                        ch,
                        palette_slot: self.field_palette_slot,
                        level,
                        factor: fade,
                    },
                );
                self.current_cells.push(SolarCell { col, line });
            }
        }

        // Pass C — the rain: riding heads + comet trails, then the
        // ejecta sparks.
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
                        && rand_chance.sample(rng) < SOLAR_SHIMMER_HOT
                    {
                        d.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
                    }
                }
            } else {
                d.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
            }

            let head_level = d.kinetic_level();
            draw_solar_cell(
                ctx,
                frame,
                SolarCellPaint {
                    col,
                    line,
                    ch: d.ch,
                    palette_slot: d.palette_slot,
                    level: head_level,
                    factor: 1.0,
                },
            );
            self.current_cells.push(SolarCell { col, line });

            // Comet trail: previously occupied cells (the descent
            // leaves them above/behind the head), one rung dimmer
            // each.
            for t in 0..d.trail_len() as usize {
                if let Some((tc, tl)) = d.trail_cell(t) {
                    if tc >= ctx.cols || tl >= ctx.lines {
                        continue;
                    }
                    let depth = (d.trail_len() as usize - t).min(3) as u8;
                    let trail_level = step_down_level(head_level, depth);
                    draw_solar_cell(
                        ctx,
                        frame,
                        SolarCellPaint {
                            col: tc,
                            line: tl,
                            ch: d.ch,
                            palette_slot: d.palette_slot,
                            level: trail_level,
                            factor: 1.0,
                        },
                    );
                    self.current_cells.push(SolarCell { col: tc, line: tl });
                }
            }

            d.push_trail(col, line);
        }

        // Pass D — the generation-tagged diff cleanup (monolith
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
}

/// The fabric glyph read (law 5): a field cell keeps the glyph the
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
/// rungs (mirrors the aeolian/aurora `step_down_level`).
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

/// Step a brightness level up (toward Core) by one rung (the
/// footpoint flash and the apex condensation glow).
fn step_up_level(level: BrightnessLevel) -> BrightnessLevel {
    match level {
        BrightnessLevel::Ghost | BrightnessLevel::Dim => BrightnessLevel::Mid,
        BrightnessLevel::Mid => BrightnessLevel::Hot,
        BrightnessLevel::Hot | BrightnessLevel::Core => BrightnessLevel::Core,
    }
}

/// One solar cell's paint request — the value object carried by
/// `draw_solar_cell` (NIGHT-hunter-25 part 2).
///
/// The six previous positionals had two same-typed neighbor hazards
/// (`col`/`line` both `u16`) across six call sites in this file; a
/// swapped pair compiled cleanly and painted the wrong cell. Named
/// fields make every call site self-documenting and drop the function
/// under the lint threshold (ctx + frame + bundle). All fields are
/// `Copy`, so the bundle scalar-replaces to the same register passing
/// as the positional form (A/B verified).
#[derive(Clone, Copy)]
struct SolarCellPaint {
    col: u16,
    line: u16,
    ch: char,
    palette_slot: u8,
    level: BrightnessLevel,
    factor: f32,
}

/// Render one solar cell (palette-aware color + bold, mono-safe).
fn draw_solar_cell(ctx: &DrawCtx<'_>, frame: &mut Frame, paint: SolarCellPaint) {
    let SolarCellPaint {
        col,
        line,
        ch,
        palette_slot,
        level,
        factor,
    } = paint;
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
