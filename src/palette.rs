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
//! careful gradient design.
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
// Human eyes perceive brightness non-linearly. Linear RGB interpolation
// (lerp_u8) produces muddy mid-tones and uneven brightness steps — the
// middle of a black→white gradient looks gray instead of 50% brightness.
//
// Gamma-correct interpolation converts sRGB → linear light, interpolates
// in linear space, then converts back. This produces perceptually uniform
// gradients that match how eyes actually see color transitions.
//
// Cost: ~5 multiplies + 2 pow() per channel. Called ONLY at palette build
// time (gradient_from_stops), NOT in the hot render path. Negligible.

/// Convert an sRGB byte (0-255) to linear light (0.0-1.0).
/// Uses the exact sRGB transfer function (IEC 61966-2-1).
#[inline]
fn srgb_to_linear(c: u8) -> f32 {
    let cs = c as f32 / 255.0;
    if cs <= 0.04045 {
        cs / 12.92
    } else {
        ((cs + 0.055) / 1.055).powf(2.4)
    }
}

/// Convert linear light (0.0-1.0) to an sRGB byte (0-255).
/// Uses the exact sRGB transfer function (IEC 61966-2-1).
#[inline]
fn linear_to_srgb(c: f32) -> u8 {
    let cs = if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (cs * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Gamma-correct linear interpolation between two sRGB bytes.
/// Converts to linear light, interpolates, converts back to sRGB.
/// Produces perceptually uniform gradients (no muddy mid-tones).
#[inline]
fn lerp_u8_gamma(a: u8, b: u8, t: f32) -> u8 {
    let la = srgb_to_linear(a);
    let lb = srgb_to_linear(b);
    let lerped = la + (lb - la) * t;
    linear_to_srgb(lerped)
}

/// Blend a color toward white by the given factor (0.0 = no change, 1.0 = pure white).
/// Works with all color types (Rgb, AnsiValue, Ansi16).
#[must_use]
pub fn blend_toward_white(color: Color, factor: f32) -> Color {
    if factor <= 0.0 || matches!(color, Color::Reset) {
        return color;
    }
    let f = factor.clamp(0.0, 1.0);
    let (r, g, b) = color_to_rgb(color);
    Color::Rgb {
        r: lerp_u8(r, 255, f),
        g: lerp_u8(g, 255, f),
        b: lerp_u8(b, 255, f),
    }
}

/// Darken a color by the given factor (1.0 = no change, 0.0 = black).
/// Works with all color types (Rgb, AnsiValue, Ansi16).
#[must_use]
#[allow(dead_code)] // PERF(v10): inlined into atmospheric hot path; kept for API stability
pub fn apply_brightness(color: Color, factor: f32) -> Color {
    if factor >= 1.0 || matches!(color, Color::Reset) {
        return color;
    }
    let f = factor.clamp(0.0, 1.0);
    let (r, g, b) = color_to_rgb(color);
    Color::Rgb {
        r: (r as f32 * f).round().clamp(0.0, 255.0) as u8,
        g: (g as f32 * f).round().clamp(0.0, 255.0) as u8,
        b: (b as f32 * f).round().clamp(0.0, 255.0) as u8,
    }
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

/// Reduce saturation of a color by the given factor (1.0 = no change, 0.0 = grayscale).
#[must_use]
#[allow(dead_code)] // PERF(v10): inlined into atmospheric hot path; kept for API stability
pub fn apply_saturation(color: Color, factor: f32) -> Color {
    if factor >= 1.0 || matches!(color, Color::Reset) {
        return color;
    }
    let f = factor.clamp(0.0, 1.0);
    let (r, g, b) = color_to_rgb(color);
    let gray = ((r as u16 + g as u16 + b as u16) / 3) as u8;
    Color::Rgb {
        r: lerp_u8(gray, r, f),
        g: lerp_u8(gray, g, f),
        b: lerp_u8(gray, b, f),
    }
}

pub(crate) fn gradient_from_stops(stops: &[(u8, u8, u8)], steps: usize) -> Vec<(u8, u8, u8)> {
    if steps == 0 || stops.is_empty() {
        return Vec::new();
    }
    if stops.len() == 1 {
        return vec![stops[0]; steps];
    }
    if steps == 1 {
        return vec![stops[0]];
    }

    let segs = stops.len().saturating_sub(1);
    let mut out = Vec::with_capacity(steps);
    for i in 0..steps {
        let t = (i as f32) / ((steps - 1) as f32);
        let pos = t * (segs as f32);
        let mut seg = pos.floor() as usize;
        if seg >= segs {
            seg = segs.saturating_sub(1);
        }
        let lt = pos - (seg as f32);
        let (r0, g0, b0) = stops[seg];
        let (r1, g1, b1) = stops[seg + 1];
        // v17 mastery: gamma-correct interpolation for perceptually uniform
        // gradients. Linear interpolation (lerp_u8) produces muddy mid-tones;
        // lerp_u8_gamma converts to linear light, interpolates, converts back.
        out.push((
            lerp_u8_gamma(r0, r1, lt),
            lerp_u8_gamma(g0, g1, lt),
            lerp_u8_gamma(b0, b1, lt),
        ));
    }
    out
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
    // v17 mastery: global brightness floor. Any palette color with RGB sum
    // below 180 gets boosted so it's visible (not invisible dark). This fixes
    // the 'dim/dark' complaint across ALL themes — the darkest trail colors
    // are now clearly visible instead of blending into the background.
    // The floor preserves the head→body→tail hierarchy: head is still much
    // brighter (RGB sum 500+) than the floored trail (RGB sum 180+).
    const MIN_RGB_SUM: u16 = 180;
    for (r, g, b) in &mut rgb {
        let sum = *r as u16 + *g as u16 + *b as u16;
        if sum < MIN_RGB_SUM {
            let scale = MIN_RGB_SUM as f32 / sum.max(1) as f32;
            *r = ((*r as f32) * scale).min(255.0) as u8;
            *g = ((*g as f32) * scale).min(255.0) as u8;
            *b = ((*b as f32) * scale).min(255.0) as u8;
        }
    }
    colors_from_rgb(mode, &rgb)
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

    // v18: All color data lives in central_colors.rs — the single source of
    // truth. build_colors() returns greyscale [White] if the scheme is not
    // in the registry (graceful degradation when THEMES is empty).
    let colors: Vec<Color> = crate::central_colors::build_colors(scheme, mode);

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
            Comet,
            Galaxy,
            Supernova,
            BlackHole,
            Andromeda,
            Stardust,
            Meteor,
            Eclipse,
            DeepSpace,
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
