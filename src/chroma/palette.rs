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
/// v30.3 (chroma audit, A2): added for the mouse-click flash wave hot
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
mod audit_tests {
    use super::*;

    /// A pair of schemes + their average RGB distance. Used by the audit
    /// test to keep clippy's type_complexity lint happy.
    type SchemePair = (ColorScheme, ColorScheme, f64);

    /// A scheme + its TrueColor RGB stops. Factored out to satisfy
    /// clippy's type_complexity lint on the Vec<(Scheme, Vec<...>)> type.
    type SchemeStops = (ColorScheme, Vec<(u8, u8, u8)>);

    /// Disposition of a known near-duplicate theme pair.
    ///
    /// The audit test (`audit_near_duplicate_themes_act`) fails when a
    /// near-duplicate pair (avg RGB distance < 30) is discovered that is
    /// NOT listed in `KNOWN_NEAR_DUPLICATES`. Each listed pair must have
    /// an explicit disposition + reason, so accidental near-duplicates
    /// from newly added themes are caught at PR time while intentional
    /// ones remain documented.
    ///
    /// `Differentiate` and `Merge` are not currently used by any entry
    /// in `KNOWN_NEAR_DUPLICATES` (all 13 pairs are `Intentional` as of
    /// v25). They exist for future use — when a developer adds a new
    /// theme that's too close to an existing one, they can mark the pair
    /// as `Differentiate` or `Merge` to flag technical debt without
    /// blocking the PR.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[allow(dead_code)] // Differentiate/Merge variants reserved for future use
    enum Disposition {
        /// The two themes are intentionally similar — they belong to the
        /// same aesthetic family (e.g. "planets", "synthwave") and the
        /// subtle difference is a deliberate user-facing choice.
        Intentional,
        /// The two themes are too close and should be made more distinct.
        /// The test will still pass (the pair is allowlisted), but a
        /// follow-up issue should be filed to differentiate them.
        Differentiate,
        /// One of the two themes should be removed (merged into the
        /// other). The test will still pass, but a follow-up issue
        /// should be filed to deprecate the redundant theme.
        Merge,
    }

    /// A known near-duplicate pair + its disposition + a human-readable
    /// reason. The reason is printed by the audit test so reviewers
    /// understand why the pair was allowlisted.
    struct NearDupDisposition {
        a: ColorScheme,
        b: ColorScheme,
        disposition: Disposition,
        reason: &'static str,
    }

    /// The allowlist of known near-duplicate theme pairs.
    ///
    /// Every pair in this list has avg RGB distance < 30 (the audit
    /// threshold). Pairs NOT in this list that fall below the threshold
    /// will cause `audit_near_duplicate_themes_act` to FAIL — the
    /// developer must either:
    ///   - differentiate the new theme so it's no longer a near-dup, or
    ///   - add an entry here with an explicit disposition + reason.
    ///
    /// Dispositions as of v25 (all 13 known pairs):
    ///   - All marked `Intentional` — they belong to deliberate aesthetic
    ///     families (planets, synthwave, neon variants, grayscale variants).
    ///
    /// If a pair is later marked `Differentiate` or `Merge`, the test
    /// still passes (the pair is allowlisted), but the disposition flags
    /// the need for a follow-up issue.
    #[rustfmt::skip]
    const KNOWN_NEAR_DUPLICATES: &[NearDupDisposition] = &[
        NearDupDisposition {
            a: ColorScheme::Venus, b: ColorScheme::Saturn,
            disposition: Disposition::Intentional,
            reason: "Both are warm-amber planet palettes (Venus yellow-cream, \
                     Saturn gold-amber). Part of the planets family; the subtle \
                     hue shift is the user-facing distinction.",
        },
        NearDupDisposition {
            a: ColorScheme::Neon, b: ColorScheme::Vaporwave,
            disposition: Disposition::Intentional,
            reason: "Both synthwave-inspired. Neon is magenta+cyan, Vaporwave \
                     is pink+cyan with a slightly different hue balance. Same \
                     aesthetic family, distinct enough that users prefer one.",
        },
        NearDupDisposition {
            a: ColorScheme::Mercury, b: ColorScheme::Moon,
            disposition: Disposition::Intentional,
            reason: "Both grayscale planets. Mercury is warm gray (sun-baked), \
                     Moon is cool gray (cold). Reflects actual color-temperature \
                     difference between the two bodies.",
        },
        NearDupDisposition {
            a: ColorScheme::Green, b: ColorScheme::NeonGreen,
            disposition: Disposition::Intentional,
            reason: "Both green. NeonGreen has a more saturated/neon body. \
                     Intentional variant — users requested a 'punchier' green.",
        },
        NearDupDisposition {
            a: ColorScheme::Carbon, b: ColorScheme::Gray,
            disposition: Disposition::Intentional,
            reason: "Both grayscale. Carbon has a cool blue tint (tech/industrial \
                     aesthetic), Gray is more neutral. Different aesthetic identity.",
        },
        NearDupDisposition {
            a: ColorScheme::Venus, b: ColorScheme::Jupiter,
            disposition: Disposition::Intentional,
            reason: "Both warm planet palettes. Venus is yellow-cream, Jupiter \
                     is tan-brown. Part of the planets family.",
        },
        NearDupDisposition {
            a: ColorScheme::Orange, b: ColorScheme::Fire,
            disposition: Disposition::Intentional,
            reason: "Both warm orange-red. Orange is pure orange, Fire has \
                     more red at the trail. Different aesthetic intent.",
        },
        NearDupDisposition {
            a: ColorScheme::NeonPurple, b: ColorScheme::Purple,
            disposition: Disposition::Intentional,
            reason: "Both purple. NeonPurple is more saturated/neon, Purple \
                     is more royal/lavender. Same pattern as Green/NeonGreen.",
        },
        NearDupDisposition {
            a: ColorScheme::Yellow, b: ColorScheme::Gold,
            disposition: Disposition::Intentional,
            reason: "Both yellow-gold. Yellow is pure signal yellow, Gold has \
                     a brown tint (polished metal aesthetic).",
        },
        NearDupDisposition {
            a: ColorScheme::Jupiter, b: ColorScheme::Saturn,
            disposition: Disposition::Intentional,
            reason: "Both warm planet palettes. Jupiter is tan-brown, Saturn \
                     is gold-amber. Part of the planets family.",
        },
        NearDupDisposition {
            a: ColorScheme::Purple, b: ColorScheme::Nebula,
            disposition: Disposition::Intentional,
            reason: "Both purple-ish. Purple is saturated royal, Nebula has \
                     more blue-violet (nebula gas aesthetic).",
        },
        NearDupDisposition {
            a: ColorScheme::Green, b: ColorScheme::Green2,
            disposition: Disposition::Intentional,
            reason: "Both green. Green is the original, Green2 is a slightly \
                     brighter variant added as a user-requested alternative.",
        },
        NearDupDisposition {
            a: ColorScheme::Snow, b: ColorScheme::FancyDiamond,
            disposition: Disposition::Intentional,
            reason: "Both cool-cyan-white. Snow is pure white-blue, \
                     FancyDiamond has iridescent cyan-magenta (prismatic \
                     diamond aesthetic).",
        },
        NearDupDisposition {
            a: ColorScheme::Blue, b: ColorScheme::Ocean,
            disposition: Disposition::Intentional,
            reason: "Both blue-family. Blue is pure royal blue, Ocean is \
                     blue-cyan (sea-water aesthetic). Polar gradient (sole \
                     path since v30) shifted intermediate colors so the \
                     avg RGB distance dropped to 29.9 (just below the 30 \
                     threshold). The themes are visually distinct — Blue \
                     stays royal throughout, Ocean has a visible cyan \
                     body/tail. Different aesthetic intent.",
        },
    ];

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

    /// Audit test (actionable): every near-duplicate pair (avg RGB
    /// distance < 30) MUST be listed in `KNOWN_NEAR_DUPLICATES` with an
    /// explicit disposition + reason.
    ///
    /// This is the "actionable" successor to `audit_near_duplicate_themes`
    /// (which only prints). It catches accidental near-duplicates from
    /// newly added themes at PR time — the developer must either
    /// differentiate the new theme or add an explicit disposition entry.
    ///
    /// Pairs already in `KNOWN_NEAR_DUPLICATES` are allowed to pass; the
    /// test prints their disposition + reason for reviewer visibility.
    /// Pairs NOT in the allowlist cause the test to FAIL with a helpful
    /// message naming the offending pair and its distance.
    ///
    /// ## Adding a new theme
    ///
    /// If you add a new `ColorScheme` variant and this test fails:
    ///   1. Look at the printed near-dup pair — is the new theme too
    ///      close to an existing one?
    ///   2. If yes, decide:
    ///      - Differentiate the new theme (adjust stops until distance
    ///        >= 30). Re-run the test — it should pass.
    ///      - OR add an entry to `KNOWN_NEAR_DUPLICATES` with
    ///        `Disposition::Intentional` (or `Differentiate`/`Merge` if
    ///        the similarity is a problem to fix later) and a reason.
    ///   3. Commit the change.
    ///
    /// ## Disposition hygiene
    ///
    /// Pairs marked `Differentiate` or `Merge` indicate technical debt —
    /// the test still passes, but a follow-up issue should be filed to
    /// either differentiate or remove the redundant theme. The
    /// disposition serves as the issue's justification.
    #[test]
    fn audit_near_duplicate_themes_act() {
        let schemes = all_schemes();
        let stops: Vec<SchemeStops> = schemes.iter().map(|&s| (s, truecolor_stops(s))).collect();

        // Build the list of currently-near-duplicate pairs.
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

        eprintln!("\n=== Actionable Near-Duplicate Audit ===");
        eprintln!("Threshold: avg RGB dist < 30.0");
        eprintln!("Allowlist size: {} pairs", KNOWN_NEAR_DUPLICATES.len());
        eprintln!();

        let mut unlisted: Vec<SchemePair> = Vec::new();
        for (a, b, dist) in &near_dups {
            // Look up the pair in KNOWN_NEAR_DUPLICATES. The pair may be
            // listed in either order (a,b) or (b,a), so check both.
            let found = KNOWN_NEAR_DUPLICATES
                .iter()
                .find(|d| (d.a == *a && d.b == *b) || (d.a == *b && d.b == *a));

            match found {
                Some(d) => {
                    eprintln!(
                        "  [OK] {:?} <-> {:?} ({:.1}): {:?} — {}",
                        a, b, dist, d.disposition, d.reason
                    );
                }
                None => {
                    eprintln!(
                        "  [MISSING] {:?} <-> {:?} ({:.1}): NOT in KNOWN_NEAR_DUPLICATES",
                        a, b, dist
                    );
                    unlisted.push((*a, *b, *dist));
                }
            }
        }

        // Also check for stale allowlist entries — pairs that ARE in
        // KNOWN_NEAR_DUPLICATES but no longer near-duplicate (distance
        // >= 30). These should be removed from the allowlist.
        let mut stale: Vec<&NearDupDisposition> = Vec::new();
        for d in KNOWN_NEAR_DUPLICATES {
            let still_near = near_dups
                .iter()
                .any(|(a, b, _)| (*a == d.a && *b == d.b) || (*a == d.b && *b == d.a));
            if !still_near {
                stale.push(d);
            }
        }
        if !stale.is_empty() {
            eprintln!("\n=== Stale Allowlist Entries (no longer near-duplicate) ===");
            for d in &stale {
                eprintln!(
                    "  {:?} <-> {:?}: listed but distance >= 30; remove from KNOWN_NEAR_DUPLICATES",
                    d.a, d.b
                );
            }
        }

        // The actionable assertion: every near-dup must be allowlisted.
        assert!(
            unlisted.is_empty(),
            "Found {} near-duplicate pair(s) NOT in KNOWN_NEAR_DUPLICATES.\n\
             Either differentiate the themes (adjust stops until avg RGB dist >= 30)\n\
             or add explicit disposition entries to KNOWN_NEAR_DUPLICATES in\n\
             src/chroma/palette.rs.\n\
             Unlisted pairs:\n{}",
            unlisted.len(),
            unlisted
                .iter()
                .map(|(a, b, d)| format!("  - {:?} <-> {:?} (dist {:.1})", a, b, d))
                .collect::<Vec<_>>()
                .join("\n")
        );

        // Stale entries don't fail the test (they're harmless), but
        // the printed message above flags them for cleanup.
        let _ = stale; // silence unused warning if empty
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
