// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Ball cell rendering helpers (NIGHT-special-1 stage 2.1): the
//! free functions that turn one annulus cell into a frame cell —
//! split from `black_hole.rs` the way `monolith_helpers.rs` splits
//! from the monolith body, keeping the main file under the LOC cap.
//! Owns: the radial band ladder (the soft photon-ring gradient plus
//! the NIGHT-research-10 rim photon line, both re-pinned soft at
//! NIGHT-research-12), the rim conveyor's deterministic glyph hash,
//! the Doppler-style lobe's brightness bump, and the shared cell
//! renderer (palette-aware color + bold, mono-safe).

use crate::frame::Frame;

use super::super::super::render::DrawCtx;
use super::super::monolith::monolith_helpers::{bold_for_level, color_for_level};
use super::super::monolith::BrightnessLevel;

/// Radial brightness band for the annulus: `t` is the normalized radial
/// position, 0.0 at the core edge (event horizon) and 1.0 at the outer
/// rim; `annulus_width` is the annulus's width in line-height units
/// (the rim photon line's thickness floor keys on it). Inverted from
/// the vortex drain — the photon ring hugs the hole: the innermost
/// band leads the ladder, fading outward through the body. NIGHT-
/// research-10 (the owner's Interstellar/NASA imagery read): the outer
/// band flips back UP — the thin bright photon LINE at the shadow's
/// edge, the trademark of the EHT photographs and Gargantua's render.
/// The line's band width is the larger of a `PHOTON_RIM_FRACTION`
/// share of the annulus and a one-cell floor (`PHOTON_RIM_MIN_CELLS`),
/// so the line stays thin on every terminal class yet never collapses
/// to a sub-cell sliver the raster misses; the body runs right up to
/// the line so the edge reads sharp against the sky. NIGHT-research-12
/// (the owner's 9.8/10 soft-ball ruling — the white blend still
/// strained his eyes after the glyph-head fix, because the BALL's own
/// photon structures kept burning Core): both thin edge structures
/// drop one rung to Hot — the soft warm ceiling every glyph head
/// already carries — over a Mid body, so the whole steady-state ball
/// reads as two thin warm lines wrapping a calm dim body (the Core
/// white blend survives only in the formation's transient collapse
/// flash and the infall's whip, never on the standing surface).
pub(crate) fn level_for_ring_band(t: f32, annulus_width: f32) -> BrightnessLevel {
    let rim_width = (annulus_width * crate::constants::BLACK_HOLE_PHOTON_RIM_FRACTION)
        .max(crate::constants::BLACK_HOLE_PHOTON_RIM_MIN_CELLS);
    let rim_t = 1.0 - rim_width / annulus_width;
    // The two photon structures share the Hot band: the horizon
    // ring at the inner edge and the rim line at the shadow's edge
    // (the NIGHT-research-12 soft ceiling — Core retired from the
    // standing surface).
    if t < 0.18 || t >= rim_t {
        BrightnessLevel::Hot
    } else {
        BrightnessLevel::Mid
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
