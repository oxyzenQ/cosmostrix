// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Ball cell rendering helpers (NIGHT-special-1 stage 2.1): the
//! free functions that turn one annulus cell into a frame cell —
//! split from `black_hole.rs` the way `monolith_helpers.rs` splits
//! from the monolith body, keeping the main file under the LOC cap.
//! Owns: the radial band ladder (the photon-ring gradient), the rim
//! conveyor's deterministic glyph hash, the Doppler-style lobe's
//! brightness bump, and the shared cell renderer (palette-aware
//! color + bold, mono-safe).

use crate::frame::Frame;

use super::super::super::render::DrawCtx;
use super::super::monolith::monolith_helpers::{bold_for_level, color_for_level};
use super::super::monolith::BrightnessLevel;

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

/// Deterministic conveyor character for a rim bucket: a multiplicative
/// hash of the bucket index folded onto the active charset pool. The
/// same bucket always yields the same glyph, so the pattern translates
/// coherently around the rim as the spin phase advances (on the binary
/// charset it reads as an alternating 0/1 stream circulating the
/// horizon). Deterministic on purpose — no RNG, no per-frame flicker.
pub(crate) fn conveyor_char(pool: &[char], bucket: i32) -> char {
    if pool.is_empty() {
        return '0';
    }
    let h = (bucket as i64).wrapping_mul(31);
    pool[h.rem_euclid(pool.len() as i64) as usize]
}

/// Step a brightness level up (+1) or down (-1) one ladder rung,
/// clamped to [Ghost, Core] — the Doppler-style lobe's band shift.
pub(crate) fn bump_level(level: BrightnessLevel, delta: i8) -> BrightnessLevel {
    let rank = match level {
        BrightnessLevel::Ghost => 0i8,
        BrightnessLevel::Dim => 1,
        BrightnessLevel::Mid => 2,
        BrightnessLevel::Hot => 3,
        BrightnessLevel::Core => 4,
    };
    match (rank + delta).clamp(0, 4) {
        0 => BrightnessLevel::Ghost,
        1 => BrightnessLevel::Dim,
        2 => BrightnessLevel::Mid,
        3 => BrightnessLevel::Hot,
        _ => BrightnessLevel::Core,
    }
}

/// Render one ball cell (palette-aware color + bold, mono-safe).
pub(crate) fn draw_ball_cell(
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
