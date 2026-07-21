// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Runtime color tuning (`--color-tune`).
//!
//! Lets users adjust saturation and brightness of ANY built-in theme at
//! load time, turning the 43 fixed themes into a 43 × ∞ palette space
//! without adding new theme presets.
//!
//! ## Algorithm
//!
//! Both transforms operate in linear RGB space (no HSL round-trip):
//!
//! - **Saturation** (`saturation=1.0` = identity): scales each channel's
//!   distance from its luminance. `sat=0` → grayscale, `sat=2` → doubled
//!   saturation. Formula: `new = gray + (orig - gray) * sat`, where
//!   `gray = 0.299r + 0.587g + 0.114b` (Rec. 601 luminance).
//!
//! - **Brightness** (`brightness=1.0` = identity): multiplies each channel
//!   by the factor. `bright=0.5` → half brightness, `bright=1.5` → +50%.
//!   Clamped to [0, 255].
//!
//! ## CLI syntax
//!
//! ```text
//! --color-tune saturation=1.2,brightness=0.9
//! --color-tune sat=0.0             # grayscale
//! --color-tune bright=1.5          # +50% brightness only
//! --color-tune saturation=1.5,brightness=1.2
//! ```
//!
//! Accepted keys: `saturation` / `sat`, `brightness` / `bright`.
//! Values must be in [0.0, 3.0]. Out-of-range values produce a clean
//! error message; missing keys default to 1.0 (identity).

use crossterm::style::Color;

use crate::palette::{color_to_rgb, Palette};
use crate::runtime::ColorMode;

/// User-supplied color tuning parameters. Defaults (1.0, 1.0) are identity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorTune {
    pub saturation: f32,
    pub brightness: f32,
}

impl ColorTune {
    /// Identity tune — no transformation applied.
    pub const IDENTITY: ColorTune = ColorTune {
        saturation: 1.0,
        brightness: 1.0,
    };

    /// Returns true if this tune would have no effect (both factors = 1.0).
    /// Used to skip the per-color transform loop entirely on the fast path.
    pub fn is_identity(&self) -> bool {
        (self.saturation - 1.0).abs() < 1e-6 && (self.brightness - 1.0).abs() < 1e-6
    }
}

/// Minimum and maximum accepted values for both factors.
const TUNE_MIN: f32 = 0.0;
const TUNE_MAX: f32 = 3.0;

/// Parse a `--color-tune` string like `"saturation=1.2,brightness=0.9"`.
///
/// Accepted keys (case-insensitive): `saturation` / `sat`,
/// `brightness` / `bright`. Missing keys default to 1.0 (identity).
/// Returns an error with a human-readable message on malformed input.
pub fn parse_color_tune(s: &str) -> Result<ColorTune, String> {
    let mut saturation = 1.0_f32;
    let mut brightness = 1.0_f32;

    let s = s.trim();
    if s.is_empty() {
        return Err("error: --color-tune value is empty".to_string());
    }

    let mut found_any = false;
    for (i, part) in s.split(',').enumerate() {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        found_any = true;
        let (key, value) = part.split_once('=').ok_or_else(|| {
            format!(
                "error: --color-tune segment {} '{}' is not key=value",
                i + 1,
                part
            )
        })?;
        let key = key.trim().to_ascii_lowercase();
        let value_str = value.trim();
        // Validate key BEFORE value so unknown keys produce a clear error
        // even when the value is also out of range (e.g. "hue=30").
        let is_sat = matches!(key.as_str(), "saturation" | "sat");
        let is_bright = matches!(key.as_str(), "brightness" | "bright");
        if !is_sat && !is_bright {
            return Err(format!(
                "error: --color-tune unknown key '{}' (accepted: saturation/sat, brightness/bright)",
                key
            ));
        }
        let value: f32 = value_str.parse().map_err(|_| {
            format!(
                "error: --color-tune '{}' value '{}' is not a number",
                key, value_str
            )
        })?;
        if !(TUNE_MIN..=TUNE_MAX).contains(&value) {
            return Err(format!(
                "error: --color-tune '{}' value {} is out of range [{}, {}]",
                key, value, TUNE_MIN, TUNE_MAX
            ));
        }
        if is_sat {
            saturation = value;
        } else {
            brightness = value;
        }
    }

    // If the input was non-empty but contained only commas/whitespace
    // (e.g. ",,,"), no key=value pairs were found. Reject rather than
    // silently returning identity — the user clearly intended to tune
    // something but mistyped.
    if !found_any {
        return Err("error: --color-tune value contains no key=value pairs".to_string());
    }

    Ok(ColorTune {
        saturation,
        brightness,
    })
}

/// Apply a color tune to a palette in place. Decodes each color to RGB,
/// applies the saturation + brightness transforms, then re-encodes to the
/// active color mode. No-op (returns immediately) when `tune.is_identity()`.
///
/// The background color is also tuned for visual consistency — otherwise
/// the foreground shifts but the background stays the same, producing an
/// odd look on themes with non-black backgrounds.
pub fn apply_tune_to_palette(palette: &mut Palette, mode: ColorMode, tune: &ColorTune) {
    if tune.is_identity() {
        return;
    }
    for color in &mut palette.colors {
        *color = apply_tune_to_color(*color, mode, tune);
    }
    if let Some(bg) = palette.bg {
        palette.bg = Some(apply_tune_to_color(bg, mode, tune));
    }
}

/// Apply the tune to a single color. Decodes to RGB, transforms, re-encodes
/// to the active color mode.
fn apply_tune_to_color(color: Color, mode: ColorMode, tune: &ColorTune) -> Color {
    if tune.is_identity() {
        return color;
    }
    // Mono mode has no color to tune.
    if matches!(mode, ColorMode::Mono) {
        return color;
    }
    let (r, g, b) = color_to_rgb(color);
    let (nr, ng, nb) = apply_tune_rgb(r, g, b, tune);
    reencode_color(nr, ng, nb, mode)
}

/// Apply saturation + brightness to an RGB triple.
///
/// v17 mastery: brightness is applied in LINEAR light space (gamma-correct),
/// not sRGB space. Human eyes perceive brightness non-linearly — applying
/// `*= 1.5` in sRGB produces uneven perceptual steps and clips too early.
/// Converting to linear, scaling, and converting back matches how eyes
/// actually perceive brightness changes. Saturation stays in sRGB (Rec. 709
/// luminance weights) since saturation is a color-difference operation, not
/// a brightness operation.
fn apply_tune_rgb(r: u8, g: u8, b: u8, tune: &ColorTune) -> (u8, u8, u8) {
    let r = f32::from(r);
    let g = f32::from(g);
    let b = f32::from(b);

    // Saturation: scale distance from luminance (Rec. 709 HDTV weights —
    // more accurate than Rec. 601 for modern displays).
    let gray = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let mut nr = gray + (r - gray) * tune.saturation;
    let mut ng = gray + (g - gray) * tune.saturation;
    let mut nb = gray + (b - gray) * tune.saturation;

    // Brightness: apply in LINEAR light space (gamma-correct).
    // Convert sRGB → linear, scale, convert back. This produces perceptually
    // uniform brightness changes — `brightness=2.0` looks like "twice as
    // bright" to the eye, not "clipped to white".
    if (tune.brightness - 1.0).abs() > 1e-6 {
        nr = srgb_to_linear_f32(nr) * tune.brightness;
        ng = srgb_to_linear_f32(ng) * tune.brightness;
        nb = srgb_to_linear_f32(nb) * tune.brightness;
        nr = linear_to_srgb_f32(nr);
        ng = linear_to_srgb_f32(ng);
        nb = linear_to_srgb_f32(nb);
    }

    // Clamp to [0, 255] and round.
    (
        nr.round().clamp(0.0, 255.0) as u8,
        ng.round().clamp(0.0, 255.0) as u8,
        nb.round().clamp(0.0, 255.0) as u8,
    )
}

/// sRGB byte (0-255) → linear light (0.0-1.0). Exact IEC 61966-2-1 transfer.
fn srgb_to_linear_f32(c: f32) -> f32 {
    let cs = c / 255.0;
    if cs <= 0.04045 {
        cs / 12.92
    } else {
        ((cs + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear light (0.0-1.0) → sRGB byte (0-255). Exact IEC 61966-2-1 transfer.
fn linear_to_srgb_f32(c: f32) -> f32 {
    let cs = if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (cs * 255.0).clamp(0.0, 255.0)
}

/// Re-encode an RGB triple back to the active color mode.
fn reencode_color(r: u8, g: u8, b: u8, mode: ColorMode) -> Color {
    match mode {
        ColorMode::TrueColor => Color::Rgb { r, g, b },
        // For 256/16 modes we let the crossterm quantization happen at
        // draw time by emitting a TrueColor color and letting the
        // renderer's color pipeline handle it. This keeps the tune
        // visible even in lower color modes (the renderer will quantize
        // the tuned RGB, not the original).
        _ => Color::Rgb { r, g, b },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_syntax() {
        let t = parse_color_tune("saturation=1.2,brightness=0.9").unwrap();
        assert!((t.saturation - 1.2).abs() < 1e-6);
        assert!((t.brightness - 0.9).abs() < 1e-6);
    }

    #[test]
    fn parse_short_keys() {
        let t = parse_color_tune("sat=2.0,bright=0.5").unwrap();
        assert!((t.saturation - 2.0).abs() < 1e-6);
        assert!((t.brightness - 0.5).abs() < 1e-6);
    }

    #[test]
    fn parse_partial_defaults_to_identity() {
        let t = parse_color_tune("saturation=1.5").unwrap();
        assert!((t.saturation - 1.5).abs() < 1e-6);
        // brightness should default to 1.0
        assert!((t.brightness - 1.0).abs() < 1e-6);
    }

    #[test]
    fn parse_case_insensitive() {
        let t = parse_color_tune("SATURATION=1.3,BRIGHTNESS=0.8").unwrap();
        assert!((t.saturation - 1.3).abs() < 1e-6);
        assert!((t.brightness - 0.8).abs() < 1e-6);
    }

    #[test]
    fn parse_empty_errors() {
        assert!(parse_color_tune("").is_err());
        assert!(parse_color_tune("   ").is_err());
    }

    #[test]
    fn parse_only_commas_errors() {
        // Input that is non-empty but contains only commas/whitespace
        // should error, not silently return identity.
        let err = parse_color_tune(",,,,").unwrap_err();
        assert!(err.contains("no key=value pairs"), "got: {err}");
        let err = parse_color_tune(" , , , ").unwrap_err();
        assert!(err.contains("no key=value pairs"), "got: {err}");
    }

    #[test]
    fn parse_missing_value_errors() {
        let err = parse_color_tune("saturation").unwrap_err();
        assert!(err.contains("not key=value"), "got: {err}");
    }

    #[test]
    fn parse_non_numeric_errors() {
        let err = parse_color_tune("saturation=high").unwrap_err();
        assert!(err.contains("not a number"), "got: {err}");
    }

    #[test]
    fn parse_out_of_range_errors() {
        let err = parse_color_tune("saturation=4.0").unwrap_err();
        assert!(err.contains("out of range"), "got: {err}");
        let err = parse_color_tune("brightness=-0.1").unwrap_err();
        assert!(err.contains("out of range"), "got: {err}");
    }

    #[test]
    fn parse_unknown_key_errors() {
        let err = parse_color_tune("hue=30").unwrap_err();
        assert!(err.contains("unknown key"), "got: {err}");
        assert!(err.contains("hue"), "got: {err}");
    }

    #[test]
    fn identity_tune_detected() {
        assert!(ColorTune::IDENTITY.is_identity());
        assert!(!ColorTune {
            saturation: 1.1,
            brightness: 1.0
        }
        .is_identity());
        assert!(!ColorTune {
            saturation: 1.0,
            brightness: 0.9
        }
        .is_identity());
    }

    #[test]
    fn apply_tune_rgb_saturation_zero_is_grayscale() {
        let tune = ColorTune {
            saturation: 0.0,
            brightness: 1.0,
        };
        let (r, g, b) = apply_tune_rgb(200, 100, 50, &tune);
        // All three channels should converge to the luminance value.
        assert_eq!(r, g, "grayscale must have equal R/G/B");
        assert_eq!(g, b, "grayscale must have equal R/G/B");
    }

    #[test]
    fn apply_tune_rgb_brightness_half_doubles_check() {
        // v17 mastery: brightness is now gamma-correct (linear light space).
        // brightness=0.5 on R=200 gives ~146 (not 100) because 50% perceptual
        // brightness ≠ 50% sRGB value. The eye perceives non-linearly.
        let tune = ColorTune {
            saturation: 1.0,
            brightness: 0.5,
        };
        let (r, g, b) = apply_tune_rgb(200, 100, 50, &tune);
        // Gamma-correct: values are HIGHER than linear halving because
        // the eye is more sensitive to dark changes than bright changes.
        // R=200 → ~146, G=100 → ~71, B=50 → ~34 (computed via IEC 61966-2-1).
        assert!(
            r > 130 && r < 160,
            "brightness 0.5 on R=200: {r} (expected ~146)"
        );
        assert!(
            g > 60 && g < 80,
            "brightness 0.5 on G=100: {g} (expected ~71)"
        );
        assert!(
            b > 25 && b < 45,
            "brightness 0.5 on B=50: {b} (expected ~34)"
        );
    }

    #[test]
    fn apply_tune_rgb_clamps_to_255() {
        // v17 mastery: brightness=3.0 in linear space. R=200 clamps to 255
        // (linear > 1.0). G=100 does NOT clamp (linear * 3 < 1.0) — the old
        // linear test expected 255, but gamma-correct gives a lower value.
        let tune = ColorTune {
            saturation: 1.0,
            brightness: 3.0,
        };
        let (r, g, b) = apply_tune_rgb(200, 100, 50, &tune);
        assert_eq!(r, 255, "R=200 * 3.0 clamps to 255 in linear space");
        // G=100: linear ≈ 0.127, × 3.0 = 0.382 → sRGB ≈ 168 (not clamped)
        assert!(
            g > 150 && g < 200,
            "G=100 * 3.0: {g} (expected ~168, not clamped)"
        );
        // B=50: linear ≈ 0.031, × 3.0 = 0.094 → sRGB ≈ 100
        assert!(b > 80 && b < 120, "B=50 * 3.0: {b} (expected ~100)");
    }

    #[test]
    fn apply_tune_to_palette_is_noop_for_identity() {
        let mut palette = Palette {
            colors: vec![Color::Rgb {
                r: 100,
                g: 200,
                b: 50,
            }],
            bg: Some(Color::Rgb {
                r: 10,
                g: 20,
                b: 30,
            }),
        };
        let original = palette.colors[0];
        apply_tune_to_palette(&mut palette, ColorMode::TrueColor, &ColorTune::IDENTITY);
        assert_eq!(
            palette.colors[0], original,
            "identity tune must not change colors"
        );
    }
}
