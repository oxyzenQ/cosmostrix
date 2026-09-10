// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The DNA helix draw pass (NIGHT-research-7) — the presentation
//! half of the orchestration, split from `dna_helix.rs` to honor
//! the 800-LOC hard cap (the monolith family's
//! `monolith_glyphs.rs` split pattern: the state machine keeps
//! the physics, this file keeps the render).
//!
//! Order of the passes (see `dna_helix/mod.rs` law 2): the
//! strands first — the back half (depth < 0) at the dim ladder,
//! the front half (depth >= 0) one rung up — then the rungs
//! (span cells with the depth-interpolated ladder; the END cells
//! show the Watson-Crick bases, the bond cells the pool), then
//! the rain (heads + comet trails), then the monolith drawn-cell
//! diff cleanup (generation-tagged). Field cells keep the glyph
//! the frame already carries (the fabric identity — the
//! molecule's structure never re-dirts its cells), re-rolling at
//! the charge-laddered shimmer rate.

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::constants::{DNA_SHIMMER_HOT, DNA_SHIMMER_QUIET};
use crate::frame::Frame;

use super::super::super::render::DrawCtx;
use super::super::monolith::monolith_helpers::{
    bold_for_level, clear_cell, clear_phosphor_metadata, color_for_level, pick_pool_char,
};
use super::super::monolith::{BrightnessLevel, MonolithCleanup};

use super::dna_helix::{DnaCell, DnaHelixRain};
use super::helix::{charge_level, fork_sigma, rung_depth_blend, rung_line_for_idx, ForkPhase};

/// Law 1's front-deep Hot threshold: a strand crossing reads Hot
/// once its depth passes this band (the crossing X's bright front
/// strand over the dim back one).
const STRAND_DEPTH_HOT: f32 = 0.55;

/// Law 2's depth-blend band: a rung cell steps its level up or
/// down once the blended depth crosses this fraction of the
/// strand depth (the 3D read without a z-buffer). Symmetric
/// about zero — the two comparison sites share it.
const RUNG_DEPTH_BLEND_BAND: f32 = 0.30;

impl DnaHelixRain {
    /// Draw pass — the strands (back then front), then the rungs
    /// (dissolved ones skipped — the fork's window reads as the
    /// open Y), then the rain (heads + comet trails), then the
    /// monolith drawn-cell diff cleanup (generation-tagged).
    ///
    /// Strand cells: the ladder reads the depth (law 1) — back
    /// cells Ghost, front cells Mid, deep-front cells Hot (the
    /// crossing X reads as a bright front strand passing a dim
    /// back strand). Rung cells: the ladder reads the rung charge
    /// (law 3), stepped up on the front half of the depth blend
    /// and down on the back half (law 2's 3D read). Field cells
    /// keep the glyph the frame already carries (the fabric
    /// identity) and re-roll at the charge-laddered shimmer rate.
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

        // Pass A — the strands: one glyph cell per strand per
        // line, back half first (so the front half overdraws at
        // the crossing cells — the occlusion read). Law 0's soup
        // gate: through the primordial dwell the molecule draws
        // NOTHING (the sky is rain alone); the first molecule
        // cells appear with the ladder — the axis spine at zero
        // radius, splitting apart as the radius grows.
        if self.genome.molecule_visible() {
            for line in 0..ctx.lines {
                let line_f = line as f32;
                // One projection read per line: strand A plus the
                // mirror (the pair contract — strand_a evaluated
                // once, not once per strand).
                let ((ax, ad), (bx, bd)) = self.genome.strand_pair(line_f);
                // The fork's lead-in bow also lifts the strands above
                // the fork slightly (the Y's arms rise) — a small
                // upward bow reads as the strands peeling apart.
                let bow_lift = self.strand_bow_lift(line_f);
                for (x, depth) in [(ax, ad), (bx, bd)] {
                    if let Some(cell) = draw_strand_cell(
                        ctx,
                        frame,
                        (x, line_f - bow_lift),
                        depth,
                        self.field_palette_slot,
                        rng,
                        rand_chance,
                    ) {
                        self.current_cells.push(cell);
                    }
                }
            }
        }

        // Pass B — the rungs: each expands into its projected
        // span of cells (the dissolve window's rungs are skipped —
        // the fork reads as the open Y; the unbuilt rungs are
        // skipped too — the genesis assembly wave writes the
        // ladder top-down, and a rung draws only once written).
        for (idx, rung) in self.genome.rungs().iter().enumerate() {
            if self.genome.rung_dissolved(idx) {
                continue;
            }
            if !self.genome.rung_built(idx) {
                continue;
            }
            let rung_line = rung_line_for_idx(idx, ctx.lines);
            // The rung's geometry in one projection (span ends +
            // the strand A depth the per-cell blend reads): the
            // per-rung invariant, hoisted out of the span-cell
            // loop — the per-cell work is the blend alone.
            let Some((left, right, ad)) = self.genome.rung_geometry(idx) else {
                continue;
            };
            let level = charge_level(rung.charge);
            let shimmer = if matches!(level, BrightnessLevel::Hot | BrightnessLevel::Core) {
                DNA_SHIMMER_HOT
            } else {
                DNA_SHIMMER_QUIET
            };
            let col_lo = left.round().floor().max(0.0) as u16;
            let col_hi = right
                .round()
                .ceil()
                .min((ctx.cols.saturating_sub(1)) as f32) as u16;
            let span = (right - left).max(1.0);
            let (base_l, base_r) = rung.pair.end_glyphs();
            let width = col_hi.saturating_sub(col_lo);
            for col in col_lo..=col_hi {
                // The dashed-bond rung (the iconic ladder read): the
                // two END cells carry the bases (law 2's identity
                // glyphs, riding ON the backbone), and the interior
                // carries bond glyphs at a stride — a solid bar would
                // read as a wall of text, the dashes read as the
                // hydrogen bonds between the paired bases. At the
                // crossing (width <= 2) the rung is the bases alone.
                let offset = col - col_lo;
                let is_bond = width > 2 && offset > 0 && offset < width && (offset % 4) == 2;
                if offset > 0 && offset < width && !is_bond {
                    continue;
                }
                let t = ((col as f32 - left) / span).clamp(0.0, 1.0);
                let depth = rung_depth_blend(ad, t);
                let cell_level = depth_level(level, depth);
                let ch = if offset == 0 {
                    base_l
                } else if offset == width {
                    base_r
                } else {
                    fabric_glyph(ctx, frame, col, rung_line, shimmer, rng, rand_chance)
                };
                draw_dna_cell(
                    ctx,
                    frame,
                    col,
                    rung_line,
                    ch,
                    self.field_palette_slot,
                    cell_level,
                );
                self.current_cells.push(DnaCell {
                    col,
                    line: rung_line,
                });
            }
        }

        // Pass C — the rain: heads + comet trails.
        for d in &mut self.drops {
            if !d.active {
                continue;
            }
            let col = d.x.round();
            let line = d.y.round();
            if col < 0.0 || line < 0.0 || col >= ctx.cols as f32 || line >= lines_f {
                // Off-screen: skip the draw AND the trail push
                // (the trail resumes from the last visible cell).
                continue;
            }
            let (col, line) = (col as u16, line as u16);

            // Matrix shimmer: mutate the glyph when the head lands
            // on a new cell.
            if d.trail_len() > 0 {
                if let Some((prev_col, prev_line)) = d.trail_cell(d.trail_len() as usize - 1) {
                    if (prev_col != col || prev_line != line)
                        && rand_chance.sample(rng) < DNA_SHIMMER_HOT
                    {
                        d.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
                    }
                }
            } else {
                d.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
            }

            let head_level = d.age_level();
            draw_dna_cell(ctx, frame, col, line, d.ch, d.palette_slot, head_level);
            self.current_cells.push(DnaCell { col, line });

            // Comet trail: previously occupied cells (the fall
            // leaves them above the head), one rung dimmer each.
            for t in 0..d.trail_len() as usize {
                if let Some((tc, tl)) = d.trail_cell(t) {
                    if tc >= ctx.cols || tl >= ctx.lines {
                        continue;
                    }
                    let depth = (d.trail_len() as usize - t).min(3) as u8;
                    let trail_level = step_down_level(head_level, depth);
                    draw_dna_cell(ctx, frame, tc, tl, d.ch, d.palette_slot, trail_level);
                    self.current_cells.push(DnaCell { col: tc, line: tl });
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

    /// The fork's peel lift (law 4's Y read): the strands above
    /// the fork rise a little where the bow is strongest — the
    /// arms of the Y. Bounded by half a rung spacing (the peel
    /// reads, the ladder's line registry never shifts).
    fn strand_bow_lift(&self, line: f32) -> f32 {
        if self.genome.fork_phase != ForkPhase::Traveling {
            return 0.0;
        }
        let sigma = fork_sigma(self.genome.lines());
        let d = line - self.genome.fork_y;
        let env = (-d * d / (2.0 * sigma * sigma)).exp();
        env * crate::constants::DNA_RUNG_STEP as f32 * 0.5
    }
}

/// Draw one strand cell: the ladder reads the depth (back Ghost,
/// front Mid, deep-front Hot — the 3D read without a z-buffer),
/// the glyph the pool with the fabric identity. Returns the drawn
/// cell for the caller's diff bookkeeping (None when the cell
/// lands outside the viewport).
fn draw_strand_cell(
    ctx: &DrawCtx<'_>,
    frame: &mut Frame,
    pos: (f32, f32),
    depth: f32,
    palette_slot: u8,
    rng: &mut StdRng,
    rand_chance: &Uniform<f32>,
) -> Option<DnaCell> {
    let (x, y) = pos;
    let col = x.round();
    let line = y.round();
    if col < 0.0 || line < 0.0 || col >= ctx.cols as f32 || line >= ctx.lines as f32 {
        return None;
    }
    let (col, line) = (col as u16, line as u16);
    let level = strand_level(depth);
    let ch = fabric_glyph(ctx, frame, col, line, DNA_SHIMMER_QUIET, rng, rand_chance);
    draw_dna_cell(ctx, frame, col, line, ch, palette_slot, level);
    Some(DnaCell { col, line })
}

/// Law 1's ladder read: the strand depth to brightness rung.
pub(crate) fn strand_level(depth: f32) -> BrightnessLevel {
    if depth > STRAND_DEPTH_HOT {
        BrightnessLevel::Hot
    } else if depth >= 0.0 {
        BrightnessLevel::Mid
    } else {
        BrightnessLevel::Ghost
    }
}

/// Law 2's depth blend: the rung's base level (from the charge)
/// stepped up on the front half, down on the back half.
fn depth_level(level: BrightnessLevel, depth: f32) -> BrightnessLevel {
    if depth > RUNG_DEPTH_BLEND_BAND {
        step_up_level(level)
    } else if depth < -RUNG_DEPTH_BLEND_BAND {
        step_down_level(level, 1)
    } else {
        level
    }
}

/// Step a brightness level down (toward Ghost) by `depth` ladder
/// rungs (mirrors the aeolian/solar `step_down_level`).
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
/// front-half rung glow).
fn step_up_level(level: BrightnessLevel) -> BrightnessLevel {
    match level {
        BrightnessLevel::Ghost | BrightnessLevel::Dim => BrightnessLevel::Mid,
        BrightnessLevel::Mid => BrightnessLevel::Hot,
        BrightnessLevel::Hot | BrightnessLevel::Core => BrightnessLevel::Core,
    }
}

/// The fabric glyph read (law 2's identity arm): a field cell
/// keeps the glyph the frame already carries (the fabric
/// identity); a fresh cell picks from the pool; cells carrying a
/// glyph re-roll at the shimmer chance.
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

/// Render one DNA cell (palette-aware color + bold, mono-safe).
fn draw_dna_cell(
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
