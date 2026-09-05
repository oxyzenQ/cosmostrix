// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Dragon-style support helpers (NIGHT-hunter-10 split, mirrors the
//! monolith family split): glyph pool picks, body brightness zoning,
//! the deterministic state-transition noise roll, the state duration
//! roll, and the single-cell renderer. Extracted from `dragon.rs`
//! when the main file reached the 800-line hard cap — the state
//! machine, chain solver and diff-cleanup stay there.

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::frame::Frame;

use super::dragon::Dragon;
use super::monolith::BrightnessLevel;
use super::monolith_helpers::{bold_for_level, color_for_level};
use super::render::DrawCtx;

/// Brightness zone by segment index along the body (head=Core,
/// first third=Hot, middle third=Mid, tail third=Ghost). The
/// serpentine fade is the Chinese-dragon body's visible signature.
pub(super) fn level_for_segment(index: usize, body_len: usize) -> BrightnessLevel {
    if body_len == 0 {
        return BrightnessLevel::Core;
    }
    let i = index.min(body_len - 1);
    let third = body_len / 3;
    if i == 0 {
        BrightnessLevel::Core
    } else if i <= third {
        BrightnessLevel::Hot
    } else if i <= third * 2 {
        BrightnessLevel::Mid
    } else {
        BrightnessLevel::Ghost
    }
}

/// Roll a state-transition random number from the dragon's sim_age
/// (deterministic per-dragon — avoids the borrow-checker issues of
/// passing an RNG into the advance loop where dragon.iter_mut()
/// already borrows self mutably). The owner mandate is "free flight
/// then circle then free again" — the stochastic transitions only
/// need a per-dragon per-frame roll, and sin-age-hash provides that.
pub(super) fn dragon_noise_roll(d: &Dragon) -> f32 {
    let s = (d.sim_age * 7.3 + d.noise_phase).sin();
    (s + 1.0) * 0.5
}

/// State duration roll — extracted as a free function so the
/// activate_dragon path can use it cleanly without the borrow
/// complexity of an RNG inside the dragon iter loop.
pub(super) fn dragon_state_duration(
    min: f32,
    max: f32,
    rand_chance: &Uniform<f32>,
    rng: &mut StdRng,
) -> f32 {
    min + rand_chance.sample(rng) * (max - min)
}

/// Render one dragon cell (palette-aware color + bold, mono-safe).
pub(super) fn draw_dragon_cell(
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
