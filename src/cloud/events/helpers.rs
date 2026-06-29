// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared rendering utilities for atmospheric events.
//!
//! Future-proof: functions are for upcoming event types.
#![allow(dead_code)]
//!
//! Pure functions with no state — used by event `render()` methods
//! to avoid duplicating gaussian falloff, radial gradient, and
//! screen-space line drawing logic.

use rand::distr::Distribution;

/// Compute a 1D gaussian falloff factor for a distance from center.
/// `dist` is the distance, `sigma` controls spread.
/// Returns a value in [0.0, 1.0] — 1.0 at the center, fading toward 0.
#[inline]
pub fn gaussian_falloff(dist: f32, sigma: f32) -> f32 {
    if sigma <= 0.0 {
        return if dist == 0.0 { 1.0 } else { 0.0 };
    }
    (-(dist * dist) / (2.0 * sigma * sigma)).exp()
}

/// Compute a 2D gaussian falloff factor for a point relative to a center.
#[inline]
pub fn gaussian_falloff_2d(col: u16, line: u16, cx: f32, cy: f32, sigma: f32) -> f32 {
    let dx = col as f32 - cx;
    let dy = line as f32 - cy;
    let dist_sq = dx * dx + dy * dy;
    gaussian_falloff(dist_sq.sqrt(), sigma)
}

/// Apply a brightness factor to (r, g, b) via white-blend (lerp toward 255).
/// `factor` in [0.0, 1.0] — 0 = no change, 1 = full white.
#[inline]
pub fn apply_white_blend(r: u8, g: u8, b: u8, factor: f32) -> (u8, u8, u8) {
    let wf = ((factor.clamp(0.0, 1.0)) * 256.0) as i32;
    let nr = (r as i32 + ((255 - r as i32) * wf + 128) / 256).clamp(0, 255) as u8;
    let ng = (g as i32 + ((255 - g as i32) * wf + 128) / 256).clamp(0, 255) as u8;
    let nb = (b as i32 + ((255 - b as i32) * wf + 128) / 256).clamp(0, 255) as u8;
    (nr, ng, nb)
}

/// Apply a brightness multiplier to (r, g, b).
/// `factor` in [0.0, ∞) — <1 dims, >1 brightens.
#[inline]
pub fn apply_brightness_mult(r: u8, g: u8, b: u8, factor: f32) -> (u8, u8, u8) {
    let fi = (factor.clamp(0.0, 3.0) * 256.0) as i32;
    let nr = ((r as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
    let ng = ((g as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
    let nb = ((b as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
    (nr, ng, nb)
}

/// Check whether a point is within a bounding rectangle.
#[inline]
pub fn point_in_rect(col: u16, line: u16, rx: u16, ry: u16, rw: u16, rh: u16) -> bool {
    col >= rx && col < rx.saturating_add(rw) && line >= ry && line < ry.saturating_add(rh)
}

/// Compute the message bounding box if a message is active.
/// Returns (start_col, start_line, width, height).
pub fn compute_message_bounds(
    msg_cols: u16,
    msg_lines: u16,
    cols: u16,
    lines: u16,
    border: bool,
) -> Option<(u16, u16, u16, u16)> {
    if msg_cols == 0 || msg_lines == 0 {
        return None;
    }
    let b = if border { 1u16 } else { 0 };
    let box_w = msg_cols.saturating_add(2 * b).saturating_add(4); // pad_x=2
    let box_h = msg_lines.saturating_add(2 * b).saturating_add(2); // pad_y=1
    let start_col = (cols / 2).saturating_sub(box_w / 2);
    let start_line = (lines / 2).saturating_sub(box_h / 2);
    Some((start_col, start_line, box_w, box_h))
}

/// Select a bolt character based on the direction of travel.
/// Returns one of 8 glyphs for natural thickness variation:
///   Vertical:   │ ┃ ┆ (thick, medium, thin)
///   Diagonal L: ╲ ╲ (repeated for probability distribution)
///   Diagonal R: ╱ ╱
/// The probability distribution favors ╲/╱ for diagonals and varies
/// thickness for vertical segments to avoid visual monotony.
#[inline]
pub fn bolt_char_for_step(dcol: i16, _dline: i16, rng: &mut impl rand::Rng) -> char {
    let roll: f32 = rand::distr::Uniform::new(0.0, 1.0)
        .expect("[0,1) always valid")
        .sample(rng);
    match dcol {
        -3..=-2 => '╲',
        -1 => {
            if roll < 0.3 {
                '┃'
            } else {
                '╲'
            }
        }
        0 => {
            // Vertical: vary thickness — 50% medium, 30% thick, 20% thin
            if roll < 0.5 {
                '│'
            } else if roll < 0.8 {
                '┃'
            } else {
                '┆'
            }
        }
        1 => {
            if roll < 0.3 {
                '┃'
            } else {
                '╱'
            }
        }
        2..=3 => '╱',
        _ => '│',
    }
}
