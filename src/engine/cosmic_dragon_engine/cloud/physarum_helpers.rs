// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Physarum-style support helpers (NIGHT-hunter-10 split, mirrors the
//! monolith/dragon family splits): toroidal trail-field sampling, the
//! render/level helpers, the deterministic tie-break roll, and the
//! test-diagnostic hooks. Extracted from `physarum.rs` when the main
//! file reached the 800-line hard cap — the stigmergic core
//! (sense/decide/move/deposit + trail decay) stays there as one
//! algorithm.

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::frame::Frame;

use super::monolith::BrightnessLevel;
use super::monolith_helpers::{bold_for_level, color_for_level};
use super::render::DrawCtx;

/// Sample the trail field at a continuous (x, y) position with
/// wraparound. Returns 0.0 if the field is empty or the position is
/// degenerate. The wraparound lets sensor samples near viewport
/// edges "see" the opposite side — the toroidal topology keeps
/// networks from clustering at corners (the standard Jeff Jones
/// model uses wraparound for this reason).
#[inline]
pub(super) fn sample_trail(field: &[f32], cols: usize, lines: usize, x: f32, y: f32) -> f32 {
    if field.is_empty() || cols == 0 || lines == 0 {
        return 0.0;
    }
    let cols_f = cols as f32;
    let lines_f = lines as f32;
    // Wraparound modulo (toroidal substrate). In-range samples (the
    // common case — sensors sit within sensor_dist of an in-bounds
    // particle) take the branch fast path; the general modulo handles
    // the rare far-out-of-band input with identical semantics.
    let wx = {
        let mut v = x;
        if v < 0.0 || v >= cols_f {
            v %= cols_f;
            if v < 0.0 {
                v += cols_f;
            }
        }
        v
    };
    let wy = {
        let mut v = y;
        if v < 0.0 || v >= lines_f {
            v %= lines_f;
            if v < 0.0 {
                v += lines_f;
            }
        }
        v
    };
    let cx = wx.round() as usize;
    let cy = wy.round() as usize;
    let cx = cx.min(cols - 1);
    let cy = cy.min(lines - 1);
    field[cx * lines + cy]
}

/// Pick a char from the pool via a uniform roll (defensive fallback
/// '0' for the degenerate empty-pool case — production always
/// initializes). Mirrors vortex/lorenz/dragon.
pub(super) fn pick_pool_char(pool: &[char], rand_chance: &Uniform<f32>, rng: &mut StdRng) -> char {
    if pool.is_empty() {
        return '0';
    }
    let idx = (rand_chance.sample(rng) * pool.len() as f32) as usize;
    pool[idx.min(pool.len() - 1)]
}

/// Brightness zone by trail field value at the head position. The
/// threshold values are tuned so that the network veins (cells with
/// accumulated trail from many particle passes) read as Core/Hot,
/// while exploring particles (cells with low trail) read as Ghost.
/// This makes the network visible via the heads themselves — the
/// pattern emerges from the brightness distribution across active
/// particles, not from a separate trail visualization pass.
pub(super) fn level_for_trail(trail_val: f32) -> BrightnessLevel {
    if trail_val > crate::constants::PHYSARUM_BRIGHTNESS_HOT {
        BrightnessLevel::Core
    } else if trail_val > crate::constants::PHYSARUM_BRIGHTNESS_MID {
        BrightnessLevel::Hot
    } else if trail_val > crate::constants::PHYSARUM_BRIGHTNESS_DIM {
        BrightnessLevel::Mid
    } else {
        BrightnessLevel::Ghost
    }
}

/// Deterministic pseudo-random roll from a particle's sim_age +
/// heading — used for the random tie-break in the decide pass.
/// Avoids the borrow complexity of threading an RNG into the
/// advance loop (mirrors `dragon_noise_roll`).
pub(super) fn sample_random(sim_age: f32) -> f32 {
    let s = (sim_age * 17.31).sin();
    (s + 1.0) * 0.5
}

/// Render one physarum cell (palette-aware color + bold, mono-safe).
pub(super) fn draw_physarum_cell(
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
