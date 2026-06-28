// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

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

fn from_ansi_list(list: &[u8]) -> Vec<Color> {
    list.iter().map(|&v| Color::AnsiValue(v)).collect()
}

fn from_rgb_list(list: &[(u8, u8, u8)]) -> Vec<Color> {
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

fn colors_from_rgb(mode: ColorMode, list: &[(u8, u8, u8)]) -> Vec<Color> {
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
/// Hot-path callers should prefer `apply_brightness_rgb` / `blend_toward_white_rgb`
/// which accept pre-decoded `(u8, u8, u8)` tuples to avoid repeated decoding.
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

/// RGB-tuple version of `blend_toward_white`. Avoids `color_to_rgb()` decode
/// when the caller already has the pre-decoded (r, g, b) values.
/// This is the primary hot-path variant used by the rendering pipeline.
#[inline]
#[must_use]
#[allow(dead_code)] // Reserved for future hot-path callers (atmospheric effects)
pub(crate) fn blend_toward_white_rgb(r: u8, g: u8, b: u8, factor: f32) -> Color {
    let f = factor.clamp(0.0, 1.0);
    Color::Rgb {
        r: lerp_u8(r, 255, f),
        g: lerp_u8(g, 255, f),
        b: lerp_u8(b, 255, f),
    }
}

/// Darken a color by the given factor (1.0 = no change, 0.0 = black).
/// Works with all color types (Rgb, AnsiValue, Ansi16).
#[must_use]
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

fn gradient_from_stops(stops: &[(u8, u8, u8)], steps: usize) -> Vec<(u8, u8, u8)> {
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
        out.push((
            lerp_u8(r0, r1, lt),
            lerp_u8(g0, g1, lt),
            lerp_u8(b0, b1, lt),
        ));
    }
    out
}

fn colors_from_stops(mode: ColorMode, stops: &[(u8, u8, u8)], steps: usize) -> Vec<Color> {
    if matches!(mode, ColorMode::Mono) {
        return vec![Color::White];
    }
    let rgb = gradient_from_stops(stops, steps);
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

    let colors: Vec<Color> = match scheme {
        ColorScheme::Green => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::DarkGreen, Color::Green],
            ColorMode::TrueColor => colors_from_stops(
                mode,
                &[
                    (0, 20, 0),      // Trail: deep dark green (not gray)
                    (0, 75, 5),      // Dark green
                    (0, 145, 30),    // Medium green
                    (20, 200, 60),   // Bright green
                    (75, 235, 95),   // Vivid green
                    (135, 255, 150), // Bright mint-green
                    (185, 255, 210), // HEAD: luminous green-white
                ],
                7,
            ),
            _ => from_ansi_list(&[234, 22, 28, 35, 78, 84, 159]),
        },
        ColorScheme::Green2 => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![
                Color::DarkGrey,
                Color::DarkGreen,
                Color::Green,
                Color::White,
            ],
            ColorMode::TrueColor => colors_from_stops(
                mode,
                &[
                    (0, 30, 0),
                    (0, 90, 10),
                    (10, 160, 40),
                    (60, 220, 100),
                    (120, 255, 160),
                    (200, 255, 230),
                    (240, 255, 250),
                ],
                7,
            ),
            _ => from_ansi_list(&[28, 34, 76, 84, 120, 157, 231]),
        },
        ColorScheme::Green3 => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::DarkGreen, Color::White],
            ColorMode::TrueColor => colors_from_stops(
                mode,
                &[
                    (0, 25, 0),
                    (0, 85, 5),
                    (5, 150, 25),
                    (30, 195, 65),
                    (85, 235, 110),
                    (150, 255, 175),
                    (185, 255, 210),
                ],
                7,
            ),
            _ => from_ansi_list(&[22, 28, 34, 70, 76, 82, 157]),
        },
        ColorScheme::Gold => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![
                Color::DarkGrey,
                Color::DarkYellow,
                Color::Yellow,
                Color::White,
            ],
            _ => from_ansi_list(&[58, 94, 172, 178, 228, 230, 231]),
        },
        ColorScheme::Yellow => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::DarkGrey, Color::Yellow, Color::White],
            _ => from_ansi_list(&[100, 142, 184, 226, 227, 229, 230]),
        },
        ColorScheme::Orange => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::Red, Color::Grey],
            _ => from_ansi_list(&[52, 94, 130, 166, 202, 208, 231]),
        },
        ColorScheme::Red => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::DarkRed, Color::Red, Color::White],
            _ => from_ansi_list(&[234, 52, 88, 124, 160, 196, 217]),
        },
        ColorScheme::Blue => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::DarkBlue, Color::Blue, Color::White],
            _ => from_ansi_list(&[234, 17, 18, 19, 20, 21, 75, 159]),
        },
        ColorScheme::Cyan => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::DarkCyan, Color::Cyan, Color::White],
            _ => from_ansi_list(&[24, 25, 31, 32, 38, 45, 159]),
        },
        ColorScheme::Purple => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::Magenta, Color::Grey],
            _ => from_ansi_list(&[60, 61, 62, 63, 69, 111, 225]),
        },
        ColorScheme::Neon => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::Blue, Color::Magenta, Color::Cyan, Color::White],
            _ => from_ansi_list(&[17, 18, 19, 54, 93, 129, 201, 51, 231]),
        },
        ColorScheme::Fire => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![
                Color::DarkRed,
                Color::Red,
                Color::DarkYellow,
                Color::Yellow,
                Color::White,
            ],
            _ => from_ansi_list(&[52, 88, 124, 160, 196, 202, 208, 214, 226, 231]),
        },
        ColorScheme::Ocean => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![
                Color::DarkBlue,
                Color::Blue,
                Color::DarkCyan,
                Color::Cyan,
                Color::White,
            ],
            _ => from_ansi_list(&[17, 18, 19, 24, 30, 37, 44, 51, 87, 159, 231]),
        },
        ColorScheme::Forest => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::DarkGreen, Color::Green, Color::Yellow, Color::White],
            _ => from_ansi_list(&[22, 28, 34, 40, 46, 82, 118, 154, 190, 229, 231]),
        },
        ColorScheme::Vaporwave => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![
                Color::Magenta,
                Color::Magenta,
                Color::Yellow,
                Color::Cyan,
                Color::White,
            ],
            _ => from_ansi_list(&[
                53, 54, 55, 134, 177, 219, 214, 220, 227, 229, 87, 123, 159, 195, 231,
            ]),
        },
        ColorScheme::Gray => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::DarkGrey, Color::Grey, Color::White],
            _ => from_ansi_list(&[234, 237, 240, 243, 246, 249, 251, 252, 231]),
        },
        ColorScheme::Rainbow => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![
                Color::Red,
                Color::Blue,
                Color::Yellow,
                Color::Green,
                Color::Cyan,
                Color::Magenta,
            ],
            _ => from_ansi_list(&[196, 208, 226, 46, 21, 93, 201]),
        },
        ColorScheme::Snow => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::DarkGrey, Color::Grey, Color::White, Color::Cyan],
            _ => from_ansi_list(&[234, 240, 250, 252, 231, 117, 159]),
        },
        ColorScheme::Aurora => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::DarkGreen, Color::Green, Color::Cyan, Color::Magenta],
            _ => from_ansi_list(&[22, 28, 34, 40, 45, 51, 93, 129, 159]),
        },
        ColorScheme::FancyDiamond => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::Cyan, Color::White, Color::Magenta],
            _ => from_ansi_list(&[45, 51, 87, 123, 159, 195, 231, 225]),
        },
        ColorScheme::Cosmos => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::DarkBlue, Color::Blue, Color::Magenta, Color::White],
            _ => from_ansi_list(&[17, 18, 19, 54, 55, 56, 57, 93, 129, 189, 225]),
        },
        ColorScheme::Nebula => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::Magenta, Color::Red, Color::Blue, Color::White],
            _ => from_ansi_list(&[53, 54, 90, 126, 162, 198, 201, 207, 213, 219, 225]),
        },
        ColorScheme::Spectrum20 => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![
                Color::DarkGrey,
                Color::DarkRed,
                Color::Red,
                Color::DarkYellow,
                Color::Yellow,
                Color::DarkGreen,
                Color::Green,
                Color::DarkCyan,
                Color::Cyan,
                Color::DarkBlue,
                Color::Blue,
                Color::DarkMagenta,
                Color::Magenta,
                Color::DarkGrey,
                Color::Grey,
                Color::White,
                Color::Cyan,
                Color::Yellow,
                Color::Magenta,
                Color::White,
            ],
            ColorMode::TrueColor => from_rgb_list(&[
                (0, 0, 0),
                (128, 0, 0),
                (255, 0, 0),
                (255, 64, 0),
                (255, 128, 0),
                (255, 191, 0),
                (255, 255, 0),
                (191, 255, 0),
                (128, 255, 0),
                (0, 255, 0),
                (0, 255, 128),
                (0, 255, 191),
                (0, 255, 255),
                (0, 191, 255),
                (0, 128, 255),
                (0, 0, 255),
                (128, 0, 255),
                (191, 0, 255),
                (255, 0, 255),
                (255, 255, 255),
            ]),
            _ => from_ansi_list(&[
                234, 52, 88, 124, 160, 196, 202, 208, 214, 226, 190, 154, 118, 82, 51, 39, 27, 93,
                201, 231,
            ]),
        },
        ColorScheme::Stars => colors_from_stops(
            mode,
            &[(0, 0, 0), (10, 10, 40), (80, 160, 255), (255, 255, 255)],
            9,
        ),
        ColorScheme::Mars => colors_from_stops(
            mode,
            &[(20, 0, 0), (120, 10, 10), (220, 60, 20), (255, 235, 220)],
            9,
        ),
        ColorScheme::Venus => colors_from_stops(
            mode,
            &[(10, 10, 0), (120, 90, 30), (255, 220, 120), (255, 255, 255)],
            9,
        ),
        ColorScheme::Mercury => colors_from_stops(
            mode,
            &[(0, 0, 0), (64, 64, 64), (160, 160, 160), (255, 255, 255)],
            9,
        ),
        ColorScheme::Jupiter => colors_from_stops(
            mode,
            &[(20, 10, 0), (120, 60, 20), (200, 140, 90), (255, 255, 255)],
            9,
        ),
        ColorScheme::Saturn => colors_from_stops(
            mode,
            &[
                (20, 20, 10),
                (140, 120, 60),
                (230, 210, 150),
                (255, 255, 255),
            ],
            9,
        ),
        ColorScheme::Uranus => colors_from_stops(
            mode,
            &[(0, 10, 10), (0, 120, 130), (120, 255, 255), (255, 255, 255)],
            9,
        ),
        ColorScheme::Neptune => colors_from_stops(
            mode,
            &[(0, 0, 20), (0, 40, 140), (0, 140, 255), (240, 255, 255)],
            9,
        ),
        ColorScheme::Pluto => colors_from_stops(
            mode,
            &[(10, 5, 0), (90, 60, 40), (180, 190, 210), (255, 255, 255)],
            9,
        ),
        ColorScheme::Moon => colors_from_stops(
            mode,
            &[(0, 0, 0), (90, 100, 120), (200, 210, 220), (255, 255, 255)],
            9,
        ),
        ColorScheme::Sun => colors_from_stops(
            mode,
            &[(40, 0, 0), (200, 60, 0), (255, 200, 0), (255, 255, 255)],
            9,
        ),
        ColorScheme::Comet => colors_from_stops(
            mode,
            &[(0, 0, 20), (0, 100, 160), (180, 255, 255), (255, 255, 255)],
            9,
        ),
        ColorScheme::Galaxy => colors_from_stops(
            mode,
            &[(10, 0, 20), (60, 0, 120), (180, 60, 255), (255, 255, 255)],
            9,
        ),
        ColorScheme::Supernova => colors_from_stops(
            mode,
            &[(20, 0, 40), (180, 0, 60), (255, 120, 0), (255, 255, 255)],
            9,
        ),
        ColorScheme::BlackHole => colors_from_stops(
            mode,
            &[(0, 0, 0), (20, 0, 40), (40, 0, 80), (200, 120, 255)],
            9,
        ),
        ColorScheme::Andromeda => colors_from_stops(
            mode,
            &[(0, 0, 20), (50, 0, 120), (255, 80, 200), (255, 255, 255)],
            9,
        ),
        ColorScheme::Stardust => colors_from_stops(
            mode,
            &[(10, 0, 20), (120, 60, 200), (80, 200, 255), (255, 255, 255)],
            9,
        ),
        ColorScheme::Meteor => colors_from_stops(
            mode,
            &[(20, 10, 0), (180, 60, 0), (255, 170, 0), (255, 255, 255)],
            9,
        ),
        ColorScheme::Eclipse => colors_from_stops(
            mode,
            &[(0, 0, 0), (40, 0, 60), (255, 120, 0), (255, 255, 255)],
            9,
        ),
        ColorScheme::DeepSpace => colors_from_stops(
            mode,
            &[(0, 0, 0), (0, 10, 40), (0, 80, 160), (200, 120, 255)],
            9,
        ),
    };

    if default_background {
        bg = None;
    }

    Palette { colors, bg }
}
