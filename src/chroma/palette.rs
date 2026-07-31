// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Color pipeline for Cosmostrix.
//!
//! Handles palette construction, color quantization across modes (truecolor,
//! 256-color, 16-color, mono), and runtime color blending operations.
//!
//! ## Palette Construction
//!
//! Palettes are built from hand-tuned ANSI 256-color indices or gradient
//! stop points, then quantized to the active color mode at construction time.
//! Each of the 40+ color schemes defines its own aesthetic character through
//! careful gradient design. The brightness floor (Phase 7) is applied at
//! construction time — see `apply_palette_relative_floor` for the rationale.
//!
//! ## Blending Operations
//!
//! Real-time color effects (bloom, fog, glow, flash) are implemented as
//! composable blending functions that convert to RGB, apply the effect, and
//! convert back. The `color_to_rgb()` function handles all crossterm Color
//! variants including named ANSI colors, 256-color indices, and truecolor RGB.

use crossterm::style::Color;

use crate::runtime::{ColorMode, ColorScheme};

#[derive(Clone, Debug)]
pub struct Palette {
    pub colors: Vec<Color>,
    pub bg: Option<Color>,
}

pub(crate) fn from_ansi_list(list: &[u8]) -> Vec<Color> {
    list.iter().map(|&v| Color::AnsiValue(v)).collect()
}

pub(crate) fn from_rgb_list(list: &[(u8, u8, u8)]) -> Vec<Color> {
    list.iter()
        .map(|&(r, g, b)| Color::Rgb { r, g, b })
        .collect()
}

fn dist2(r0: u8, g0: u8, b0: u8, r1: u8, g1: u8, b1: u8) -> i32 {
    let dr = (r0 as i32) - (r1 as i32);
    let dg = (g0 as i32) - (g1 as i32);
    let db = (b0 as i32) - (b1 as i32);
    (dr * dr) + (dg * dg) + (db * db)
}

fn rgb_to_ansi256(r: u8, g: u8, b: u8) -> u8 {
    const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];

    let r6 = ((r as u16 * 5) + 127) / 255;
    let g6 = ((g as u16 * 5) + 127) / 255;
    let b6 = ((b as u16 * 5) + 127) / 255;

    let cr = CUBE_LEVELS[r6 as usize];
    let cg = CUBE_LEVELS[g6 as usize];
    let cb = CUBE_LEVELS[b6 as usize];
    let cube_idx = 16 + (36 * r6 as u8) + (6 * g6 as u8) + (b6 as u8);
    let cube_dist = dist2(r, g, b, cr, cg, cb);

    let avg = ((r as u16 + g as u16 + b as u16) / 3) as u8;
    let gray_idx = if avg < 8 {
        16
    } else if avg > 238 {
        231
    } else {
        232 + ((avg - 8) / 10)
    };
    let (gr, gg, gb) = if gray_idx == 16 {
        (0, 0, 0)
    } else if gray_idx == 231 {
        (255, 255, 255)
    } else {
        let v = 8 + 10 * (gray_idx - 232);
        (v, v, v)
    };
    let gray_dist = dist2(r, g, b, gr, gg, gb);

    if gray_dist < cube_dist {
        gray_idx
    } else {
        cube_idx
    }
}

fn rgb_to_color16(r: u8, g: u8, b: u8) -> Color {
    const TABLE: [(Color, (u8, u8, u8)); 16] = [
        (Color::Black, (0, 0, 0)),
        (Color::DarkGrey, (128, 128, 128)),
        (Color::Grey, (192, 192, 192)),
        (Color::White, (255, 255, 255)),
        (Color::DarkRed, (128, 0, 0)),
        (Color::Red, (255, 0, 0)),
        (Color::DarkGreen, (0, 128, 0)),
        (Color::Green, (0, 255, 0)),
        (Color::DarkBlue, (0, 0, 128)),
        (Color::Blue, (0, 0, 255)),
        (Color::DarkCyan, (0, 128, 128)),
        (Color::Cyan, (0, 255, 255)),
        (Color::DarkMagenta, (128, 0, 128)),
        (Color::Magenta, (255, 0, 255)),
        (Color::DarkYellow, (128, 128, 0)),
        (Color::Yellow, (255, 255, 0)),
    ];

    let mut best = Color::White;
    let mut best_d = i32::MAX;
    for (c, (cr, cg, cb)) in TABLE {
        let d = dist2(r, g, b, cr, cg, cb);
        if d < best_d {
            best_d = d;
            best = c;
        }
    }
    best
}

pub(crate) fn colors_from_rgb(mode: ColorMode, list: &[(u8, u8, u8)]) -> Vec<Color> {
    match mode {
        ColorMode::Mono => vec![Color::White],
        ColorMode::TrueColor => from_rgb_list(list),
        ColorMode::Color256 => list
            .iter()
            .map(|&(r, g, b)| Color::AnsiValue(rgb_to_ansi256(r, g, b)))
            .collect(),
        ColorMode::Color16 => list
            .iter()
            .map(|&(r, g, b)| rgb_to_color16(r, g, b))
            .collect(),
    }
}

/// Phase 7: apply the palette-relative brightness floor to a raw RGB list,
/// then quantize to the active color mode.
///
/// This is the floored equivalent of `colors_from_rgb`. Use this for themes
/// that supply raw RGB values (e.g. `ThemeColors::RgbWithC16`'s TrueColor
/// path) so they get the same brightness floor that `colors_from_stops`
/// applies to gradient stops.
///
/// `colors_from_stops` applies the floor itself before calling
/// `colors_from_rgb` (no double-application), so it does NOT call this
/// helper. Callers that have raw RGB and want the floor should call this
/// instead of `colors_from_rgb` directly.
pub(crate) fn colors_from_rgb_floored(mode: ColorMode, list: &[(u8, u8, u8)]) -> Vec<Color> {
    if matches!(mode, ColorMode::Mono) {
        return vec![Color::White];
    }
    let mut rgb: Vec<(u8, u8, u8)> = list.to_vec();
    apply_palette_relative_floor(&mut rgb);
    colors_from_rgb(mode, &rgb)
}

/// Convert any crossterm Color to approximate (r, g, b).
/// Returns (0, 0, 0) for Reset.
///
/// When the color is already `Color::Rgb`, this is a zero-cost destructure.
/// For other variants, it decodes the ANSI/named representation.
///
/// Hot-path callers should prefer `apply_brightness_rgb`
/// which accepts pre-decoded `(u8, u8, u8)` tuples to avoid repeated decoding.
#[must_use]
#[allow(unreachable_patterns)] // Catch-all guards against future crossterm Color variants
pub(crate) fn color_to_rgb(color: Color) -> (u8, u8, u8) {
    match color {
        // Fast path: most common in TrueColor mode — zero branching for the
        // dominant case in production rendering.
        Color::Rgb { r, g, b } => (r, g, b),
        Color::AnsiValue(v) => {
            // Decode 256-color: 0-7 = standard, 8-15 = bright, 16-231 = 6x6x6 cube, 232-255 = grayscale
            if v < 16 {
                const ANSI16_RGB: [(u8, u8, u8); 16] = [
                    (0, 0, 0),       // 0  Black
                    (128, 0, 0),     // 1  DarkRed
                    (0, 128, 0),     // 2  DarkGreen
                    (128, 128, 0),   // 3  DarkYellow
                    (0, 0, 128),     // 4  DarkBlue
                    (128, 0, 128),   // 5  DarkMagenta
                    (0, 128, 128),   // 6  DarkCyan
                    (192, 192, 192), // 7  Grey
                    (128, 128, 128), // 8  DarkGrey
                    (255, 0, 0),     // 9  Red
                    (0, 255, 0),     // 10 Green
                    (255, 255, 0),   // 11 Yellow
                    (0, 0, 255),     // 12 Blue
                    (255, 0, 255),   // 13 Magenta
                    (0, 255, 255),   // 14 Cyan
                    (255, 255, 255), // 15 White
                ];
                ANSI16_RGB[v as usize]
            } else if v < 232 {
                // 6x6x6 color cube: index = 16 + 36*r + 6*g + b
                let v = v - 16;
                let r_idx = v / 36;
                let g_idx = (v % 36) / 6;
                let b_idx = v % 6;
                // Standard cube levels
                const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
                (
                    LEVELS[r_idx as usize],
                    LEVELS[g_idx as usize],
                    LEVELS[b_idx as usize],
                )
            } else {
                // Grayscale ramp: 232-255
                let v = 8 + 10 * (v - 232);
                (v, v, v)
            }
        }
        // Named 8/16 colors — handle Reset early alongside Black (both → (0,0,0))
        // to avoid iterating through all named variants before reaching Reset.
        Color::Reset | Color::Black => (0, 0, 0),
        Color::DarkGrey => (128, 128, 128),
        Color::Red => (255, 0, 0),
        Color::DarkRed => (128, 0, 0),
        Color::Green => (0, 255, 0),
        Color::DarkGreen => (0, 128, 0),
        Color::Yellow => (255, 255, 0),
        Color::DarkYellow => (128, 128, 0),
        Color::Blue => (0, 0, 255),
        Color::DarkBlue => (0, 0, 128),
        Color::Magenta => (255, 0, 255),
        Color::DarkMagenta => (128, 0, 128),
        Color::Cyan => (0, 255, 255),
        Color::DarkCyan => (0, 128, 128),
        Color::White => (255, 255, 255),
        Color::Grey => (192, 192, 192),
        // Catch-all for any future crossterm Color variants
        _ => (0, 0, 0),
    }
}

/// Integer-based linear interpolation for u8 values.
/// Uses fixed-point arithmetic (8.8) to avoid float conversion overhead.
/// Equivalent to `a + (b - a) * t` where t is in [0.0, 1.0].
#[inline]
fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let a = a as i32;
    let b = b as i32;
    let ti = (t * 256.0) as i32; // 0..256 range
    let result = a + ((b - a) * ti + 128) / 256;
    result.clamp(0, 255) as u8
}

// ── v17 mastery: gamma-correct color interpolation ──────────────────────────
//
// Historically this file held `srgb_to_linear`, `linear_to_srgb`, and
// `lerp_u8_gamma` — the gamma-correct sRGB interpolator used by
// `gradient_from_stops`. In Phase 3-A (Chroma Dragon) the gradient logic
// moved to `chroma::gradient`, which now defaults to OKLab interpolation
// (perceptually uniform, no muddy mid-tones on hue crossings). The legacy
// sRGB-linear path survives as `chroma::gradient::gradient_from_stops_srgb`.
//
// `gradient_from_stops()` below is now a one-line delegator. The
// hand-tuned sRGB conversion functions are no longer duplicated here.

/// Blend a color toward an arbitrary target color by the given factor
/// (0.0 = no change, 1.0 = pure target).
///
/// Phase 3-D (Chroma Dragon Innovation D): generalizes `blend_toward_white`
/// to blend toward any target color, including the actual scene background.
/// This is the foundation for halo effects that respect the background —
/// e.g. a head halo on a dark-cosmos background blends toward near-black,
/// not toward white, so the halo "dissolves into the scene" rather than
/// producing a bright white smear.
///
/// Both inputs accept all color types (Rgb, AnsiValue, Ansi16, Reset).
/// `Color::Reset` on either input returns the original color unchanged
/// (Reset has no meaningful RGB to blend toward).
///
/// Output is always `Color::Rgb` (normalized via `color_to_rgb`).
#[must_use]
pub fn blend_toward_bg(color: Color, bg: Color, factor: f32) -> Color {
    if factor <= 0.0 || matches!(color, Color::Reset) || matches!(bg, Color::Reset) {
        return color;
    }
    let f = factor.clamp(0.0, 1.0);
    let (r, g, b) = color_to_rgb(color);
    let (br, bgc, bb) = color_to_rgb(bg);
    Color::Rgb {
        r: lerp_u8(r, br, f),
        g: lerp_u8(g, bgc, f),
        b: lerp_u8(b, bb, f),
    }
}

/// Blend a color toward white by the given factor (0.0 = no change, 1.0 = pure white).
/// Works with all color types (Rgb, AnsiValue, Ansi16).
///
/// Phase 3-D: now delegates to `blend_toward_bg` with a white target.
/// Behavior is identical to the pre-Phase-3-D inlined implementation.
#[must_use]
pub fn blend_toward_white(color: Color, factor: f32) -> Color {
    blend_toward_bg(
        color,
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        },
        factor,
    )
}

/// RGB-tuple version of `apply_brightness`. Avoids `color_to_rgb()` decode
/// when the caller already has the pre-decoded (r, g, b) values.
/// Uses integer math to avoid f32->f32->u8 round-trip overhead.
/// This is the primary hot-path variant used by the rendering pipeline.
#[inline]
#[must_use]
pub(crate) fn apply_brightness_rgb(r: u8, g: u8, b: u8, factor: f32) -> Color {
    let f = factor.clamp(0.0, 1.0);
    let fi = (f * 256.0) as i32; // 0..256
    Color::Rgb {
        r: ((r as i32 * fi + 128) >> 8).clamp(0, 255) as u8,
        g: ((g as i32 * fi + 128) >> 8).clamp(0, 255) as u8,
        b: ((b as i32 * fi + 128) >> 8).clamp(0, 255) as u8,
    }
}

/// Decode a color to RGB once, returning both the original Color and the (r, g, b) tuple.
/// Used by hot-path callers that need to chain multiple blend operations
/// without re-decoding the color each time.
/// Returns `None` for `Color::Reset` (no visual contribution).
#[inline]
#[must_use]
pub(crate) fn decode_color(color: Color) -> Option<(u8, u8, u8)> {
    if matches!(color, Color::Reset) {
        return None;
    }
    let (r, g, b) = color_to_rgb(color);
    Some((r, g, b))
}

/// Format an `Option<Color>` as a human-readable hex string.
///
/// - `None` → `"none"`
/// - `Some(Color::Rgb { r, g, b })` → `"#rrggbb"`
/// - `Some(AnsiValue/Named)` → decoded to hex via `color_to_rgb`
///
/// Shared between `--verbose` output and benchmark CONFIG section so both
/// report the identical on-screen background hex for a custom palette's bg.
#[must_use]
pub(crate) fn format_color_hex(bg: Option<Color>) -> String {
    match bg {
        None => "none".to_string(),
        Some(c) => {
            let (r, g, b) = color_to_rgb(c);
            format!("#{r:02x}{g:02x}{b:02x}")
        }
    }
}

pub(crate) fn gradient_from_stops(stops: &[(u8, u8, u8)], steps: usize) -> Vec<(u8, u8, u8)> {
    // Phase 3-A (Chroma Dragon): now delegates to OKLab interpolation by
    // default. OKLab rotates hue smoothly through the chroma ring and keeps
    // saturation high at midpoints, eliminating the muddy brown/gray mid-tones
    // that sRGB-linear interpolation produces on hue-crossing gradients
    // (red→green, blue→yellow, etc.).
    //
    // The legacy sRGB-linear implementation is preserved as
    // `chroma::gradient::gradient_from_stops_srgb` for any future theme that
    // explicitly wants the old look.
    //
    // Endpoints are preserved exactly (same as before); only intermediate
    // colors change. Build-time cost is negligible (~12 mul + 3 cbrt per
    // segment transition, called only at palette build, not in the hot path).
    super::gradient::gradient_from_stops_oklab(stops, steps)
}

pub(crate) fn colors_from_stops(
    mode: ColorMode,
    stops: &[(u8, u8, u8)],
    steps: usize,
) -> Vec<Color> {
    if matches!(mode, ColorMode::Mono) {
        return vec![Color::White];
    }
    let mut rgb = gradient_from_stops(stops, steps);
    apply_palette_relative_floor(&mut rgb);
    colors_from_rgb(mode, &rgb)
}

/// Phase 7: apply the palette-relative brightness floor in-place.
///
/// Replaces the v17 global `MIN_RGB_SUM = 180` rule. The floor is derived
/// from the palette's own brightness profile (max stop sum ×
/// `PALETTE_FLOOR_RATIO`), clamped to `[ABSOLUTE_MIN_FLOOR, GLOBAL_MAX_FLOOR]`.
///
/// ## Why palette-relative?
///
/// The v17 global rule caused washout on dark themes: Cosmos `(3, 3, 18)`
/// (intentional "void" trail, sum 24) was boosted 7.5× to `(22, 22, 135)`
/// (sum 180), destroying the deep-space aesthetic. Mercury `(5, 5, 5)`
/// (sum 15) became `(60, 60, 60)` (sum 180), turning a near-black trail
/// into medium gray.
///
/// Phase 7 derives the floor from the palette's brightest stop (head):
/// trails must be at least `PALETTE_FLOOR_RATIO` (15%) as bright as the
/// head, with an absolute minimum of `ABSOLUTE_MIN_FLOOR` (30) and a cap
/// of `GLOBAL_MAX_FLOOR` (180, matching v17's upper bound).
///
/// ## Effect on built-in themes
///
/// - Green (head sum 655): floor = 98. Trail `(0, 12, 1)` → `(0, 90, 2)`.
///   Clearly visible dark green, less aggressive than v17's `(0, 165, 14)`.
/// - Cosmos (head sum 655): floor = 98. Trail `(3, 3, 18)` → `(12, 12, 73)`.
///   Visible void blue, much less aggressive than v17's `(22, 22, 135)`.
/// - Mercury (head sum 720): floor = 108. Trail `(5, 5, 5)` → `(36, 36, 36)`.
///   Visible dark gray, vs v17's `(60, 60, 60)` medium gray.
/// - Theoretical pure-dark palette (head sum 100): floor = 30. All stops
///   below 30 get boosted to 30; others unchanged. Preserves darkness.
///
/// ## Hue preservation
///
/// The boost scales all three channels by the same factor, preserving the
/// RGB ratio and thus the hue. A blue-tinted trail stays blue-tinted, just
/// brighter. This is the same property as the v17 rule.
///
/// ## Cost
///
/// Called once per palette build (not per frame). Two passes over the
/// stops: one to find max sum, one to apply the floor. On a 9-stop palette
/// this is ~18 additions + 1 max + 9 comparisons — sub-microsecond.
fn apply_palette_relative_floor(rgb: &mut [(u8, u8, u8)]) {
    // Empty palette: nothing to floor.
    if rgb.is_empty() {
        return;
    }

    // Find the palette's brightest stop sum (the head).
    let max_sum: u16 = rgb
        .iter()
        .map(|&(r, g, b)| r as u16 + g as u16 + b as u16)
        .max()
        .unwrap_or(0);

    // Derive the floor: clamp(max * ratio, ABSOLUTE_MIN_FLOOR, GLOBAL_MAX_FLOOR).
    // Using std::clamp instead of max().min() — clippy::manual_clamp.
    let derived = (max_sum as f32 * super::tuning::PALETTE_FLOOR_RATIO) as u16;
    let floor = derived.clamp(
        super::tuning::ABSOLUTE_MIN_FLOOR,
        super::tuning::GLOBAL_MAX_FLOOR,
    );

    // Apply the floor: any stop below `floor` gets scaled up to `floor`,
    // preserving the RGB ratio (hue is preserved).
    //
    // Special case: pure black (0, 0, 0) has sum 0, so scaling is a no-op
    // (0 * anything = 0). For this case, set the stop to a neutral dark
    // gray at the floor brightness. This preserves the "visible" property
    // without introducing a hue (since (0, 0, 0) has no hue to preserve).
    // The v17 rule had the same sum==0 issue but didn't handle it — Phase 7
    // fixes this so pure-black trails (e.g. Stars palette's (0, 0, 0) stop)
    // become visible.
    let floor_per_channel = (floor / 3).min(255) as u8;
    for (r, g, b) in rgb.iter_mut() {
        let sum = *r as u16 + *g as u16 + *b as u16;
        if sum < floor {
            if sum == 0 {
                *r = floor_per_channel;
                *g = floor_per_channel;
                *b = floor_per_channel;
            } else {
                let scale = floor as f32 / sum as f32;
                *r = ((*r as f32) * scale).min(255.0) as u8;
                *g = ((*g as f32) * scale).min(255.0) as u8;
                *b = ((*b as f32) * scale).min(255.0) as u8;
            }
        }
    }

    // Phase 7-b: body-tail continuity. After the basic floor, there may
    // still be a large brightness gap between adjacent stops (e.g. trail
    // sum=98, next body stop sum=356 — gap 3.6x). At high rain speed this
    // gap becomes a perceptual hard step, creating a horizontal-line
    // illusion across all columns.
    //
    // Iterate head→trail: for each adjacent pair, if the brighter stop is
    // more than BODY_TAIL_MAX_GAP_RATIO times the dimmer stop, scale up
    // the dimmer stop to maintain continuity (preserve hue via RGB ratio
    // scaling). Capped at GLOBAL_MAX_FLOOR so continuity cannot push
    // trails above the v17 ceiling.
    //
    // Iterating head→tail (rather than tail→head) is critical: the head
    // stop sets the brightness "anchor" and we propagate the constraint
    // downward. If we iterated trail→head, a single very-dim trail stop
    // would force the body to dim too, destroying the head bloom.
    apply_body_tail_continuity(rgb);
}

/// Phase 7-b: enforce body-tail continuity. After the basic floor, scale
/// up any dim stop that has a > BODY_TAIL_MAX_GAP_RATIO brightness gap
/// with its next-brighter neighbor. Iterates head→tail.
///
/// Hue is preserved via RGB-ratio scaling (same as the basic floor).
///
/// Unlike the basic floor (which is capped at `GLOBAL_MAX_FLOOR` to
/// preserve the v17 ceiling), continuity is NOT capped — it can boost
/// a trail stop above 180 if needed to maintain the 2.5x gap contract.
/// This is safe because the continuity target is always
/// `next_stop_sum / 2.5`, which is always less than `next_stop_sum`,
/// which is always less than the head brightness. So continuity cannot
/// push a trail stop brighter than the head — hierarchy is preserved.
///
/// The 4 themes that hit the uncapped path (NeonWhite, NeonCyan,
/// NeonYellow, Green3) have very bright bodies (sum > 520) where the
/// 180 cap would leave a residual 3x+ gap. Uncapping continuity lets
/// the trail reach up to ~220-255 to maintain the 2.5x contract.
///
/// See `BODY_TAIL_MAX_GAP_RATIO` for the rationale and tuning guidance.
fn apply_body_tail_continuity(rgb: &mut [(u8, u8, u8)]) {
    let n = rgb.len();
    if n < 2 {
        return;
    }
    let max_gap = super::tuning::BODY_TAIL_MAX_GAP_RATIO;

    // Iterate from second-to-last down to first.
    // For each i, if rgb[i+1].sum / rgb[i].sum > max_gap, scale up rgb[i]
    // to rgb[i+1].sum / max_gap (NOT capped — see doc comment for why).
    for i in (0..n - 1).rev() {
        let next_sum = rgb[i + 1].0 as u16 + rgb[i + 1].1 as u16 + rgb[i + 1].2 as u16;
        let cur_sum = rgb[i].0 as u16 + rgb[i].1 as u16 + rgb[i].2 as u16;
        if cur_sum == 0 || next_sum == 0 {
            continue;
        }
        let gap = next_sum as f32 / cur_sum as f32;
        if gap > max_gap {
            // Target sum = next_sum / max_gap. NOT capped — see doc comment.
            let target = next_sum as f32 / max_gap;
            // Only scale UP (never dim a stop to enforce continuity).
            if target > cur_sum as f32 {
                let scale = target / cur_sum as f32;
                let (r, g, b) = rgb[i];
                rgb[i] = (
                    ((r as f32) * scale).min(255.0) as u8,
                    ((g as f32) * scale).min(255.0) as u8,
                    ((b as f32) * scale).min(255.0) as u8,
                );
            }
        }
    }
}

#[must_use]
pub fn build_palette(scheme: ColorScheme, mode: ColorMode, default_background: bool) -> Palette {
    let mut bg = if default_background {
        None
    } else {
        Some(match mode {
            ColorMode::Color16 => Color::Black,
            ColorMode::TrueColor => Color::Rgb { r: 0, g: 0, b: 0 },
            _ => Color::AnsiValue(16),
        })
    };

    // v18: All color data lives in chroma/catalog.rs — the single source of
    // truth. build_colors() returns greyscale [White] if the scheme is not
    // in the registry (graceful degradation when THEMES is empty).
    let colors: Vec<Color> = super::catalog::build_colors(scheme, mode);

    if default_background {
        bg = None;
    }

    Palette { colors, bg }
}

#[cfg(test)]
mod audit_tests {
    use super::*;

    /// A pair of schemes + their average RGB distance. Used by the audit
    /// test to keep clippy's type_complexity lint happy.
    type SchemePair = (ColorScheme, ColorScheme, f64);

    /// A scheme + its TrueColor RGB stops. Factored out to satisfy
    /// clippy's type_complexity lint on the Vec<(Scheme, Vec<...>)> type.
    type SchemeStops = (ColorScheme, Vec<(u8, u8, u8)>);

    /// Extract the TrueColor RGB stops for a scheme as a Vec<(u8,u8,u8)>.
    fn truecolor_stops(scheme: ColorScheme) -> Vec<(u8, u8, u8)> {
        let p = build_palette(scheme, ColorMode::TrueColor, true);
        p.colors.iter().map(|c| color_to_rgb(*c)).collect()
    }

    /// Average per-stop RGB Euclidean distance between two palettes.
    fn palette_distance(a: &[(u8, u8, u8)], b: &[(u8, u8, u8)]) -> f64 {
        let n = a.len().min(b.len()).max(1);
        let mut sum = 0.0_f64;
        for i in 0..n {
            let (r1, g1, b1) = a[i];
            let (r2, g2, b2) = b[i];
            let dr = (i32::from(r1) - i32::from(r2)) as f64;
            let dg = (i32::from(g1) - i32::from(g2)) as f64;
            let db = (i32::from(b1) - i32::from(b2)) as f64;
            sum += (dr * dr + dg * dg + db * db).sqrt();
        }
        sum / n as f64
    }

    fn all_schemes() -> Vec<ColorScheme> {
        use ColorScheme::*;
        vec![
            Green,
            Green2,
            Green3,
            NeonGreen,
            NeonPurple,
            NeonWhite,
            NeonBlue,
            NeonRed,
            NeonOrange,
            NeonYellow,
            NeonCyan,
            Carbon,
            Yellow,
            Orange,
            Red,
            Blue,
            Cyan,
            Gold,
            Rainbow,
            Purple,
            Neon,
            Fire,
            Ocean,
            Forest,
            Vaporwave,
            Gray,
            Snow,
            Aurora,
            FancyDiamond,
            Cosmos,
            Nebula,
            Spectrum20,
            Stars,
            Mars,
            Venus,
            Mercury,
            Jupiter,
            Saturn,
            Uranus,
            Neptune,
            Pluto,
            Moon,
            Sun,
        ]
    }

    /// Audit test: identify near-duplicate themes (avg RGB distance < 30).
    /// Prints findings to stderr so they're visible during `cargo test`.
    /// Does NOT assert — this is an informational audit, not a pass/fail gate.
    #[test]
    fn audit_near_duplicate_themes() {
        let schemes = all_schemes();
        let stops: Vec<SchemeStops> = schemes.iter().map(|&s| (s, truecolor_stops(s))).collect();

        let mut near_dups: Vec<SchemePair> = Vec::new();
        for i in 0..stops.len() {
            for j in (i + 1)..stops.len() {
                let (s1, p1) = &stops[i];
                let (s2, p2) = &stops[j];
                let dist = palette_distance(p1, p2);
                if dist < 30.0 {
                    near_dups.push((*s1, *s2, dist));
                }
            }
        }
        near_dups.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());

        eprintln!("\n=== Theme Audit: Near-Duplicate Pairs (avg RGB dist < 30) ===");
        if near_dups.is_empty() {
            eprintln!("  None found.");
        } else {
            for (s1, s2, dist) in &near_dups {
                eprintln!("  {:?} <-> {:?}: {:.1}", s1, s2, dist);
            }
        }

        // Also print the 5 closest pairs regardless of threshold, for context.
        eprintln!("\n=== 5 Closest Pairs (for context) ===");
        let mut all_dists: Vec<SchemePair> = Vec::new();
        for i in 0..stops.len() {
            for j in (i + 1)..stops.len() {
                let (s1, p1) = &stops[i];
                let (s2, p2) = &stops[j];
                all_dists.push((*s1, *s2, palette_distance(p1, p2)));
            }
        }
        all_dists.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());
        for (s1, s2, dist) in all_dists.iter().take(5) {
            eprintln!("  {:?} <-> {:?}: {:.1}", s1, s2, dist);
        }
    }
}

#[cfg(test)]
mod blend_tests {
    use super::*;

    /// Factor=0 returns the original color unchanged.
    #[test]
    fn blend_toward_bg_zero_factor_unchanged() {
        let c = Color::Rgb {
            r: 100,
            g: 50,
            b: 200,
        };
        let bg = Color::Rgb {
            r: 10,
            g: 20,
            b: 30,
        };
        assert_eq!(blend_toward_bg(c, bg, 0.0), c);
    }

    /// Factor=1 returns approximately the target color (within ±1 unit per
    /// channel — `lerp_u8` uses integer fixed-point with a +128 rounding
    /// term that biases endpoints by 1 LSB).
    #[test]
    fn blend_toward_bg_full_factor_returns_bg() {
        let c = Color::Rgb {
            r: 100,
            g: 50,
            b: 200,
        };
        let bg = Color::Rgb {
            r: 10,
            g: 20,
            b: 30,
        };
        let result = blend_toward_bg(c, bg, 1.0);
        let Color::Rgb { r, g, b } = result else {
            panic!("expected Rgb");
        };
        assert!((10..=11).contains(&r), "r {r} should be 10 or 11 (±1 LSB)");
        assert!((20..=21).contains(&g), "g {g} should be 20 or 21 (±1 LSB)");
        assert!((30..=31).contains(&b), "b {b} should be 30 or 31 (±1 LSB)");
    }

    /// Factor=0.5 returns the midpoint between color and bg.
    #[test]
    fn blend_toward_bg_half_factor_returns_midpoint() {
        let c = Color::Rgb { r: 0, g: 0, b: 0 };
        let bg = Color::Rgb {
            r: 100,
            g: 200,
            b: 50,
        };
        let result = blend_toward_bg(c, bg, 0.5);
        // lerp_u8 uses integer fixed-point: (0 + (100-0)*128 + 128)/256 ≈ 50
        let Color::Rgb { r, g, b } = result else {
            panic!("expected Rgb");
        };
        assert_eq!(r, 50, "midpoint r");
        assert_eq!(g, 100, "midpoint g");
        assert_eq!(b, 25, "midpoint b");
    }

    /// Color::Reset on either input returns the original color.
    #[test]
    fn blend_toward_bg_reset_returns_original() {
        let c = Color::Rgb {
            r: 100,
            g: 50,
            b: 200,
        };
        assert_eq!(blend_toward_bg(Color::Reset, c, 0.5), Color::Reset);
        assert_eq!(blend_toward_bg(c, Color::Reset, 0.5), c);
    }

    /// Factor > 1.0 is clamped to 1.0 (within ±1 LSB of bg).
    #[test]
    fn blend_toward_bg_factor_above_one_clamps() {
        let c = Color::Rgb {
            r: 100,
            g: 50,
            b: 200,
        };
        let bg = Color::Rgb {
            r: 10,
            g: 20,
            b: 30,
        };
        let result = blend_toward_bg(c, bg, 2.0);
        let Color::Rgb { r, g, b } = result else {
            panic!("expected Rgb");
        };
        assert!((10..=11).contains(&r), "clamped r {r}");
        assert!((20..=21).contains(&g), "clamped g {g}");
        assert!((30..=31).contains(&b), "clamped b {b}");
    }

    /// Factor < 0.0 is treated as 0.0 (no blend).
    #[test]
    fn blend_toward_bg_negative_factor_unchanged() {
        let c = Color::Rgb {
            r: 100,
            g: 50,
            b: 200,
        };
        let bg = Color::Rgb {
            r: 10,
            g: 20,
            b: 30,
        };
        assert_eq!(blend_toward_bg(c, bg, -0.5), c);
    }

    /// blend_toward_white is equivalent to blend_toward_bg with white target.
    #[test]
    fn blend_toward_white_delegates_to_blend_toward_bg() {
        let c = Color::Rgb {
            r: 100,
            g: 50,
            b: 200,
        };
        let via_white = blend_toward_white(c, 0.3);
        let via_bg = blend_toward_bg(
            c,
            Color::Rgb {
                r: 255,
                g: 255,
                b: 255,
            },
            0.3,
        );
        assert_eq!(via_white, via_bg);
    }

    /// Halos on a dark background: blending a bright head color toward a
    /// dark BG produces a darker halo (atmospheric dissolve into scene).
    /// Each channel lands between the head value and the bg value.
    #[test]
    fn blend_toward_bg_dark_bg_darkens_halo() {
        // Bright green head on near-black cosmos background
        let head_r = 80u8;
        let head_g = 255u8;
        let head_b = 110u8;
        let head = Color::Rgb {
            r: head_r,
            g: head_g,
            b: head_b,
        };
        let bg_r = 8u8;
        let bg_g = 12u8;
        let bg_b = 20u8;
        let cosmos_bg = Color::Rgb {
            r: bg_r,
            g: bg_g,
            b: bg_b,
        };
        let halo = blend_toward_bg(head, cosmos_bg, 0.4);
        let Color::Rgb { r, g, b } = halo else {
            panic!("expected Rgb");
        };
        // Halo r must be between bg_r (8) and head_r (80). At factor=0.4
        // it's closer to head than bg: lerp(80, 8, 0.4) ≈ 80 - 28.8 ≈ 51.
        assert!(
            (bg_r..=head_r).contains(&r),
            "halo r {r} must be in [{bg_r}, {head_r}]"
        );
        assert!(
            (bg_g..=head_g).contains(&g),
            "halo g {g} must be in [{bg_g}, {head_g}]"
        );
        assert!(
            (bg_b..=head_b).contains(&b),
            "halo b {b} must be in [{bg_b}, {head_b}]"
        );
        // Sanity: halo is darker than head on all channels (blending toward
        // a darker BG must reduce each channel).
        assert!(r < head_r, "halo r {r} must be < head r {head_r}");
        assert!(g < head_g, "halo g {g} must be < head g {head_g}");
        assert!(b < head_b, "halo b {b} must be < head b {head_b}");
        // And the dominant channel (green) must still be brighter than bg
        // so the head silhouette remains visible.
        assert!(g > bg_g, "halo g {g} must be > bg g {bg_g} (head visible)");
    }
}

// Phase 7 + Phase 7-b test suite. Extracted to keep this file under the
// 1500-LOC cap. The `#[path]` attribute preserves `use super::*` access
// to palette's private helpers (apply_palette_relative_floor, etc.).
#[cfg(test)]
#[path = "palette_floor_tests.rs"]
mod palette_floor_tests;
