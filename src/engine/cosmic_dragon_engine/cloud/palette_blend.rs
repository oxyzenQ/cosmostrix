// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Palette color interpolation helper — extracted from `cloud/mod.rs`
//! to keep that file under the 800-LOC cap.
//!
//! Owns the `interpolate_palette_color()` function: a pure, stateless
//! helper that smoothly blends between adjacent chroma-dragon palette
//! stops at a parametric position `t ∈ [0.0, 1.0]`.
//!
//! Used by:
//! - `cloud::draw_message()` — border message chroma gradient sweep
//! - `cloud::rain_post` — droplet lifecycle chroma sweep
//! - `chroma::shaders::base` — gradient transitions
//! - `hud::mod` — HUD chroma gradient lines
//!
//! Re-exported from `cloud/mod.rs` via `pub(crate) use` so all existing
//! `crate::cloud::interpolate_palette_color(...)` call sites continue to
//! resolve without changes.

use crossterm::style::Color;

/// Smoothly interpolate a color from a chroma dragon palette at parametric
/// position `t ∈ [0.0, 1.0]`. `t = 0.0` returns the first palette stop;
/// `t = 1.0` returns the last; intermediate values linearly blend between
/// the two surrounding stops using `chroma::legacy::blend_toward_rgb`
/// (linear sRGB per-channel lerp).
///
/// # Why this exists
///
/// The owner reported that the border message (`--message` overlay) chroma
/// dragon sweep had visible "gaps" between palette stops — e.g. a
/// white→red sweep showed a white block then a red block with no
/// in-between, instead of the smooth white → semi-red → red gradient the
/// rain color already produced. Root cause: the previous implementation
/// rounded `t * (n-1)` to the nearest integer index and picked that
/// discrete palette stop. This helper replaces the discrete sampling with
/// linear interpolation, so adjacent border cells get smoothly-varying
/// interpolated colors matching the rain color's per-cell chroma dragon
/// sweep.
///
/// # Edge cases
///
/// - Empty palette → `None` (caller should fall back to `content_fg`).
/// - Single-stop palette → returns that stop for any `t`.
/// - `t <= 0.0` → returns the first stop (clamped).
/// - `t >= 1.0` → returns the last stop (clamped).
/// - `t` exactly at an integer stop boundary → returns that stop exactly
///   (no interpolation), so palette-identity stops are preserved.
/// - NaN / Inf `t` → returns the first stop (defensive, no panic).
///
/// # Performance
///
/// One `decode_color` + one `blend_toward_rgb` per call (≈3 ns each on a
/// warm cache). Called per visible border cell per frame; for a typical
/// 60-cell border at 60 FPS this is ≈11 µs/s — well under 0.1% CPU.
///
/// `pub(crate)` so the `cloud::tests` submodule can access it for
/// regression testing (child modules can access parent's private items,
/// but explicit `pub(crate)` makes the test surface intentional).
pub(crate) fn interpolate_palette_color(palette: &[Color], t: f32) -> Option<Color> {
    let n = palette.len();
    if n == 0 {
        return None;
    }
    if n == 1 {
        return Some(palette[0]);
    }
    // Defensive: NaN or Inf t falls back to the first stop rather than
    // producing a panic on `.floor()` or `.min()` later.
    if !t.is_finite() {
        return Some(palette[0]);
    }
    let palette_last = (n - 1) as f32;
    let scaled_t = (t.clamp(0.0, 1.0)) * palette_last;
    let pos = scaled_t.floor() as usize;
    let frac = scaled_t - pos as f32;
    let color_a = palette.get(pos).copied();
    let color_b = palette.get((pos + 1).min(n - 1)).copied();
    match (color_a, color_b) {
        // Interior stops with a non-zero fraction: interpolate linearly
        // between palette[pos] and palette[pos+1] using `frac` as the
        // blend factor (0.0 = pure palette[pos], 1.0 = pure palette[pos+1]).
        (Some(a), Some(b)) if pos + 1 < n && frac > 0.0 => {
            let (ar, ag, ab) = crate::palette::decode_color(a).unwrap_or((0, 0, 0));
            let (br, bg_c, bb) = crate::palette::decode_color(b).unwrap_or((ar, ag, ab));
            let (r, g, b) = crate::chroma_dragon_engine::legacy::blend_toward_rgb(
                ar, ag, ab, br, bg_c, bb, frac,
            );
            Some(Color::Rgb { r, g, b })
        }
        // Boundary stops (pos == palette_last) or frac == 0.0: use the
        // exact palette stop, no interpolation needed.
        (Some(a), _) => Some(a),
        _ => None,
    }
}
