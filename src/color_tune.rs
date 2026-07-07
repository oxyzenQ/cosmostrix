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

    for (i, part) in s.split(',').enumerate() {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
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
fn apply_tune_rgb(r: u8, g: u8, b: u8, tune: &ColorTune) -> (u8, u8, u8) {
    let r = f32::from(r);
    let g = f32::from(g);
    let b = f32::from(b);

    // Saturation: scale distance from luminance (Rec. 601 weights).
    let gray = 0.299 * r + 0.587 * g + 0.114 * b;
    let mut nr = gray + (r - gray) * tune.saturation;
    let mut ng = gray + (g - gray) * tune.saturation;
    let mut nb = gray + (b - gray) * tune.saturation;

    // Brightness: scale each channel.
    nr *= tune.brightness;
    ng *= tune.brightness;
    nb *= tune.brightness;

    // Clamp to [0, 255] and round.
    (
        nr.round().clamp(0.0, 255.0) as u8,
        ng.round().clamp(0.0, 255.0) as u8,
        nb.round().clamp(0.0, 255.0) as u8,
    )
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
        let tune = ColorTune {
            saturation: 1.0,
            brightness: 0.5,
        };
        let (r, g, b) = apply_tune_rgb(200, 100, 50, &tune);
        assert_eq!(r, 100, "brightness 0.5 must halve R");
        assert_eq!(g, 50, "brightness 0.5 must halve G");
        assert_eq!(b, 25, "brightness 0.5 must halve B");
    }

    #[test]
    fn apply_tune_rgb_clamps_to_255() {
        let tune = ColorTune {
            saturation: 1.0,
            brightness: 3.0,
        };
        let (r, g, b) = apply_tune_rgb(200, 100, 50, &tune);
        assert_eq!(r, 255, "clamped to 255");
        assert_eq!(g, 255, "clamped to 255");
        assert_eq!(b, 150, "50 * 3 = 150, no clamp needed");
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
