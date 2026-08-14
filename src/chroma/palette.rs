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
// moved to `chroma::gradient`, which uses OKLab interpolation (perceptually
// uniform, no muddy mid-tones on hue crossings). Phase 9-A made polar the
// sole production path; the legacy sRGB-linear path and the Cartesian
// OKLab variant were removed in v30.
//
// `gradient_from_stops()` below is a one-line delegator.

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

/// RGB-tuple version of `blend_toward_white`. Avoids the `color_to_rgb()`
/// decode + `Color::Reset` check when the caller already has the
/// pre-decoded (r, g, b) values. Returns the blended (r, g, b) triple.
///
/// (chroma audit, A2): added for the mouse-click flash wave hot
/// path. The equation is identical to `blend_toward_white` -- the
/// difference is the input/output shape (tuple vs Color). Used by
/// droplet.rs::CellShader::shade when the chroma pipeline is active;
/// the legacy fallback uses `chroma::legacy::blend_toward_white`.
#[inline]
#[must_use]
pub(crate) fn blend_toward_white_rgb(r: u8, g: u8, b: u8, factor: f32) -> (u8, u8, u8) {
    let f = factor.clamp(0.0, 1.0);
    let wf = (f * 256.0) as i32;
    (
        (r as i32 + ((255 - r as i32) * wf + 128) / 256).clamp(0, 255) as u8,
        (g as i32 + ((255 - g as i32) * wf + 128) / 256).clamp(0, 255) as u8,
        (b as i32 + ((255 - b as i32) * wf + 128) / 256).clamp(0, 255) as u8,
    )
}

/// RGB-tuple version of `blend_toward_bg`. Blends `(r, g, b)` toward the
/// target `(tr, tg, tb)` by `factor` (0.0 = no change, 1.0 = full target).
///
/// (chroma audit, A1): added for the quantum ripple render path.
/// The particle carries its snapshot body color as `(r, g, b)` and blends
/// the cell's current color toward that snapshot by `brightness`. The
/// chroma path uses this helper; the legacy fallback uses
/// `chroma::legacy::blend_toward_rgb`. Both produce bit-identical
/// output (same equation as `lerp_u8`).
#[inline]
#[must_use]
pub(crate) fn blend_toward_bg_rgb(
    r: u8,
    g: u8,
    b: u8,
    tr: u8,
    tg: u8,
    tb: u8,
    factor: f32,
) -> (u8, u8, u8) {
    let f = factor.clamp(0.0, 1.0);
    let wf = (f * 256.0) as i32;
    (
        (r as i32 + ((tr as i32 - r as i32) * wf + 128) / 256).clamp(0, 255) as u8,
        (g as i32 + ((tg as i32 - g as i32) * wf + 128) / 256).clamp(0, 255) as u8,
        (b as i32 + ((tb as i32 - b as i32) * wf + 128) / 256).clamp(0, 255) as u8,
    )
}

/// Multiplicative RGB boost. `out = (r, g, b) * (1.0 + factor)`, clamped
/// to `[0, 255]`. Used by the head self-bloom effect (the head glyph gets
/// a multiplicative brightness boost scaled by the parallax layer's
/// self-bloom multiplier).
///
/// (chroma audit, A4): added for the head self-bloom hot path.
/// The equation is bit-identical to `chroma::legacy::boost_rgb` -- both
/// use `(c as f32 * (1.0 + factor)).round().clamp(0.0, 255.0) as u8`.
/// The audit proposed a future "perceptual OKLab L lift" variant that
/// would preserve hue+chroma more accurately, but that is a behavior
/// change requiring a separate owner approval. This commit lands the
/// safe migration (same equation, auditability refactor only).
#[inline]
#[must_use]
pub(crate) fn boost_rgb(r: u8, g: u8, b: u8, factor: f32) -> (u8, u8, u8) {
    let scale = 1.0 + factor;
    (
        (r as f32 * scale).round().clamp(0.0, 255.0) as u8,
        (g as f32 * scale).round().clamp(0.0, 255.0) as u8,
        (b as f32 * scale).round().clamp(0.0, 255.0) as u8,
    )
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

/// Unclamped variant of [`blend_toward_bg_rgb`]. Identical equation, but
/// `factor` is NOT clamped to `[0.0, 1.0]`.
///
/// (chroma audit, A11): added for the parallax saturation
/// modulation in `droplet.rs::Droplet::draw`. The saturation effect
/// uses `factor = 1.0 - saturation_mult`, and `PARALLAX_SATURATION_MULT`
/// has values both below 1.0 (back layers desaturate, factor > 0) AND
/// above 1.0 (front layer oversaturates, factor < 0). The standard
/// `blend_toward_bg_rgb` clamps factor to `[0, 1]`, which would
/// silently turn the front-layer oversaturation case into a no-op
/// and regress the saturation fix.
///
/// Negative factors push the channel AWAY from the target (extrapolation
/// beyond the source). Positive factors > 1.0 push beyond the target.
/// Both are well-defined: the equation `(c + ((t - c) * wf + 128) / 256)`
/// works for any `wf` value, the per-channel `.clamp(0, 255)` keeps the
/// final output in u8 range.
///
/// # Parity
/// Bit-identical to `chroma::legacy::blend_toward_rgb` for any factor.
/// The legacy helper is also unclamped -- the only difference between
/// the two is module ownership (chroma engine vs. legacy fallback).
#[inline]
#[must_use]
pub(crate) fn blend_toward_bg_rgb_unclamped(
    r: u8,
    g: u8,
    b: u8,
    tr: u8,
    tg: u8,
    tb: u8,
    factor: f32,
) -> (u8, u8, u8) {
    let wf = (factor * 256.0) as i32;
    (
        (r as i32 + ((tr as i32 - r as i32) * wf + 128) / 256).clamp(0, 255) as u8,
        (g as i32 + ((tg as i32 - g as i32) * wf + 128) / 256).clamp(0, 255) as u8,
        (b as i32 + ((tb as i32 - b as i32) * wf + 128) / 256).clamp(0, 255) as u8,
    )
}

/// Unclamped variant of `apply_brightness_rgb`. Same equation, but the
/// factor is NOT clamped to `[0, 1]`. Required for callers that need to
/// BOOST a channel beyond 1.0 (e.g. front-layer parallax brightness
/// 1.10).
///
/// The standard `apply_brightness_rgb` clamps factor to `[0, 1]`, which
/// would silently turn a 1.10 boost into a 1.0 no-op and regress the
/// .0 fix that enabled front-layer brightness boost.
///
/// Factor > 1.0 scales the channel upward (boost). Factor < 0 inverts
/// the channel (rarely meaningful but mathematically defined). The
/// per-channel `.clamp(0, 255)` keeps the final output in u8 range.
///
/// # Parity
/// Bit-identical to `chroma::legacy::scale_rgb` for any factor. The
/// legacy helper is also unclamped -- the only difference between the
/// two is module ownership (chroma engine vs. legacy fallback).
///
/// # Caller status ( A16 migration)
/// Wired into `droplet::CellShader::shade` for the parallax brightness
/// + glyph dim multiplicative scale. The chroma path uses this helper;
///
/// the legacy fallback uses `chroma::legacy::scale_rgb`.
#[inline]
#[must_use]
pub(crate) fn apply_brightness_rgb_unclamped(r: u8, g: u8, b: u8, factor: f32) -> (u8, u8, u8) {
    let fi = (factor * 256.0) as i32;
    (
        ((r as i32 * fi + 128) >> 8).clamp(0, 255) as u8,
        ((g as i32 * fi + 128) >> 8).clamp(0, 255) as u8,
        ((b as i32 * fi + 128) >> 8).clamp(0, 255) as u8,
    )
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
    // Phase 3-A (Chroma Dragon): OKLab interpolation. Phase 9-A made polar
    // the sole production path (Cartesian variant removed in v30 — polar
    // never regresses against Cartesian on any cosmostrix theme, and aligns
    // cosmostrix with the W3C CSS Color 4 spec which defaults `oklch`
    // interpolation to shortest-arc hue rotation).
    //
    // Polar rotates hue through the shortest arc on the OKLab chroma ring,
    // keeping midpoint chroma magnitude high on opposing-hue gradients
    // (red↔cyan, blue↔yellow). On analogous-hue gradients polar and
    // Cartesian produce identical output — polar is strictly better.
    //
    // Endpoints are preserved exactly; only intermediate colors change.
    // Build-time cost is negligible (~12 mul + 3 cbrt + 2 trig per segment
    // transition, called only at palette build, not in the hot path).
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
/// trails must be at least `PALETTE_FLOOR_RATIO` (20%) as bright as the
/// head, with an absolute minimum of `ABSOLUTE_MIN_FLOOR` (30) and a cap
/// of `GLOBAL_MAX_FLOOR` (180, matching v17's upper bound).
///
/// ## Effect on built-in themes
///
/// - Green (head sum 655): floor = 131. Trail `(0, 12, 1)` → `(0, 121, 3)`.
///   Clearly visible dark green, less aggressive than v17's `(0, 165, 14)`.
/// - Cosmos (head sum 655): floor = 131. Trail `(3, 3, 18)` → `(16, 16, 99)`.
///   Visible void blue, much less aggressive than v17's `(22, 22, 135)`.
/// - Mercury (head sum 720): floor = 144. Trail `(5, 5, 5)` → `(48, 48, 48)`.
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
    apply_palette_relative_floor_with(
        rgb,
        super::tuning::PALETTE_FLOOR_RATIO,
        super::tuning::ABSOLUTE_MIN_FLOOR,
        super::tuning::GLOBAL_MAX_FLOOR,
    );
}

/// Phase 7 parameterized variant for tuning audits. Production callers should
/// use [`apply_palette_relative_floor`], which uses the constants from
/// `tuning`. This variant exists so audit tests can sweep candidate values
/// without recompiling, and so the wrapper stays a one-line forwarder.
///
/// The math is identical to `apply_palette_relative_floor`; see that function
/// for the full rationale. Continuity (`apply_body_tail_continuity`) is still
/// applied afterward using the production `BODY_TAIL_MAX_GAP_RATIO`.
///
/// Visibility: `pub(super)` so the engine lock suite at `chroma::lock_tests`
/// can exercise it directly. Still private outside the `chroma` module.
pub(super) fn apply_palette_relative_floor_with(
    rgb: &mut [(u8, u8, u8)],
    ratio: f32,
    abs_min: u16,
    global_max: u16,
) {
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

    // Derive the floor: clamp(max * ratio, abs_min, global_max).
    // Using std::clamp instead of max().min() — clippy::manual_clamp.
    let derived = (max_sum as f32 * ratio) as u16;
    let floor = derived.clamp(abs_min, global_max);

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
/// a trail stop above 180 if needed to maintain the `BODY_TAIL_MAX_GAP_RATIO`
/// (currently 2.0x, lowered from 2.5x in Phase 7-d) gap contract.
/// This is safe because the continuity target is always
/// `next_stop_sum / BODY_TAIL_MAX_GAP_RATIO`, which is always less than
/// `next_stop_sum`, which is always less than the head brightness. So
/// continuity cannot push a trail stop brighter than the head — hierarchy
/// is preserved.
///
/// The 4 themes that hit the uncapped path (NeonWhite, NeonCyan,
/// NeonYellow, Green3) have very bright bodies (sum > 520) where the
/// 180 cap would leave a residual 2.5x+ gap. Uncapping continuity lets
/// the trail reach up to ~220-255 to maintain the 2.0x contract.
///
/// See `BODY_TAIL_MAX_GAP_RATIO` for the rationale and tuning guidance.
fn apply_body_tail_continuity(rgb: &mut [(u8, u8, u8)]) {
    apply_body_tail_continuity_with(rgb, super::tuning::BODY_TAIL_MAX_GAP_RATIO);
}

/// Phase 7-b parameterized variant for tuning audits. Production callers should
/// use [`apply_body_tail_continuity`], which uses the constant from `tuning`.
///
/// Visibility: `pub(super)` so the engine lock suite at `chroma::lock_tests`
/// can exercise it directly. Still private outside the `chroma` module.
pub(super) fn apply_body_tail_continuity_with(rgb: &mut [(u8, u8, u8)], max_gap: f32) {
    let n = rgb.len();
    if n < 2 {
        return;
    }

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
#[path = "palette_audit_tests.rs"]
mod palette_audit_tests;

#[cfg(test)]
#[path = "palette_blend_tests.rs"]
mod palette_blend_tests;

#[cfg(test)]
#[path = "palette_floor_tests.rs"]
mod palette_floor_tests;
