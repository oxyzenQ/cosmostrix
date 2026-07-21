// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Runtime color tuning (`--color-tune` and `[color.tune]` config).
//!
//! v17: extended with head/body/tail segment multipliers for per-segment
//! brightness control. CLI: `--color-tune sat=1.5,head=1.5,tail=0.5`.
//! Config: `[color.tune]` section with brightness, saturation, head, body, tail.

use crossterm::style::Color;

use crate::palette::{color_to_rgb, Palette};
use crate::runtime::ColorMode;

/// User-supplied color tuning parameters. All default to 1.0 (identity).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorTune {
    pub saturation: f32,
    pub brightness: f32,
    pub head: f32,
    pub body: f32,
    pub tail: f32,
}

impl ColorTune {
    pub const IDENTITY: ColorTune = ColorTune {
        saturation: 1.0,
        brightness: 1.0,
        head: 1.0,
        body: 1.0,
        tail: 1.0,
    };

    pub fn is_identity(&self) -> bool {
        (self.saturation - 1.0).abs() < 1e-6
            && (self.brightness - 1.0).abs() < 1e-6
            && (self.head - 1.0).abs() < 1e-6
            && (self.body - 1.0).abs() < 1e-6
            && (self.tail - 1.0).abs() < 1e-6
    }
}

const TUNE_MIN: f32 = 0.0;
const TUNE_MAX: f32 = 3.0;

/// Parse a `--color-tune` string. Keys: sat, bright, head, body, tail.
pub fn parse_color_tune(s: &str) -> Result<ColorTune, String> {
    let mut saturation = 1.0_f32;
    let mut brightness = 1.0_f32;
    let mut head = 1.0_f32;
    let mut body = 1.0_f32;
    let mut tail = 1.0_f32;

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
        let target = match key.as_str() {
            "saturation" | "sat" => &mut saturation,
            "brightness" | "bright" => &mut brightness,
            "head" => &mut head,
            "body" => &mut body,
            "tail" => &mut tail,
            _ => {
                return Err(format!(
                    "error: --color-tune unknown key '{}' (accepted: sat, bright, head, body, tail)",
                    key
                ));
            }
        };
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
        *target = value;
    }

    if !found_any {
        return Err("error: --color-tune value contains no key=value pairs".to_string());
    }

    Ok(ColorTune {
        saturation,
        brightness,
        head,
        body,
        tail,
    })
}

/// v17: Build ColorTune from [color.tune] config section.
pub fn color_tune_from_config(cfg: &std::collections::HashMap<String, String>) -> ColorTune {
    let get = |key: &str| -> f32 {
        cfg.get(key)
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|&v| (0.0..=3.0).contains(&v))
            .unwrap_or(1.0)
    };
    ColorTune {
        saturation: get("color.tune.saturation"),
        brightness: get("color.tune.brightness"),
        head: get("color.tune.head"),
        body: get("color.tune.body"),
        tail: get("color.tune.tail"),
    }
}

pub fn apply_tune_to_palette(palette: &mut Palette, mode: ColorMode, tune: &ColorTune) {
    if tune.is_identity() {
        return;
    }
    let n = palette.colors.len();
    for (i, color) in palette.colors.iter_mut().enumerate() {
        let segment_mult = if n <= 1 {
            1.0
        } else {
            let t = i as f32 / (n - 1) as f32;
            if t > 0.67 {
                tune.head
            } else if t > 0.33 {
                tune.body
            } else {
                tune.tail
            }
        };
        *color = apply_tune_to_color(*color, mode, tune, segment_mult);
    }
    if let Some(bg) = palette.bg {
        palette.bg = Some(apply_tune_to_color(bg, mode, tune, 1.0));
    }
}

fn apply_tune_to_color(
    color: Color,
    mode: ColorMode,
    tune: &ColorTune,
    segment_mult: f32,
) -> Color {
    if tune.is_identity() {
        return color;
    }
    if matches!(mode, ColorMode::Mono) {
        return color;
    }
    let (r, g, b) = color_to_rgb(color);
    let (nr, ng, nb) = apply_tune_rgb(r, g, b, tune, segment_mult);
    reencode_color(nr, ng, nb, mode)
}

fn apply_tune_rgb(r: u8, g: u8, b: u8, tune: &ColorTune, segment_mult: f32) -> (u8, u8, u8) {
    let r = f32::from(r);
    let g = f32::from(g);
    let b = f32::from(b);

    let gray = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let mut nr = gray + (r - gray) * tune.saturation;
    let mut ng = gray + (g - gray) * tune.saturation;
    let mut nb = gray + (b - gray) * tune.saturation;

    let effective_brightness = tune.brightness * segment_mult;
    if (effective_brightness - 1.0).abs() > 1e-6 {
        nr = srgb_to_linear_f32(nr) * effective_brightness;
        ng = srgb_to_linear_f32(ng) * effective_brightness;
        nb = srgb_to_linear_f32(nb) * effective_brightness;
        nr = linear_to_srgb_f32(nr);
        ng = linear_to_srgb_f32(ng);
        nb = linear_to_srgb_f32(nb);
    }

    (
        nr.round().clamp(0.0, 255.0) as u8,
        ng.round().clamp(0.0, 255.0) as u8,
        nb.round().clamp(0.0, 255.0) as u8,
    )
}

fn srgb_to_linear_f32(c: f32) -> f32 {
    let cs = c / 255.0;
    if cs <= 0.04045 {
        cs / 12.92
    } else {
        ((cs + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb_f32(c: f32) -> f32 {
    let cs = if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (cs * 255.0).clamp(0.0, 255.0)
}

fn reencode_color(r: u8, g: u8, b: u8, _mode: ColorMode) -> Color {
    Color::Rgb { r, g, b }
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
    fn parse_head_body_tail() {
        let t = parse_color_tune("head=1.5,body=1.0,tail=0.5").unwrap();
        assert!((t.head - 1.5).abs() < 1e-6);
        assert!((t.body - 1.0).abs() < 1e-6);
        assert!((t.tail - 0.5).abs() < 1e-6);
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
        assert!((t.brightness - 1.0).abs() < 1e-6);
    }

    #[test]
    fn parse_empty_errors() {
        assert!(parse_color_tune("").is_err());
        assert!(parse_color_tune("   ").is_err());
    }

    #[test]
    fn parse_unknown_key_errors() {
        let err = parse_color_tune("hue=30").unwrap_err();
        assert!(err.contains("unknown key"), "got: {err}");
    }

    #[test]
    fn identity_tune_detected() {
        assert!(ColorTune::IDENTITY.is_identity());
        assert!(!ColorTune {
            saturation: 1.1,
            brightness: 1.0,
            head: 1.0,
            body: 1.0,
            tail: 1.0
        }
        .is_identity());
    }

    #[test]
    fn apply_tune_rgb_saturation_zero_is_grayscale() {
        let tune = ColorTune {
            saturation: 0.0,
            brightness: 1.0,
            head: 1.0,
            body: 1.0,
            tail: 1.0,
        };
        let (r, g, b) = apply_tune_rgb(200, 100, 50, &tune, 1.0);
        assert_eq!(r, g, "grayscale must have equal R/G/B");
        assert_eq!(g, b, "grayscale must have equal R/G/B");
    }

    #[test]
    fn apply_tune_rgb_brightness_half_doubles_check() {
        let tune = ColorTune {
            saturation: 1.0,
            brightness: 0.5,
            head: 1.0,
            body: 1.0,
            tail: 1.0,
        };
        let (r, _g, _b) = apply_tune_rgb(200, 100, 50, &tune, 1.0);
        assert!(
            r > 130 && r < 160,
            "brightness 0.5 on R=200: {r} (expected ~146)"
        );
    }

    #[test]
    fn apply_tune_rgb_clamps_to_255() {
        let tune = ColorTune {
            saturation: 1.0,
            brightness: 3.0,
            head: 1.0,
            body: 1.0,
            tail: 1.0,
        };
        let (r, _g, _b) = apply_tune_rgb(200, 100, 50, &tune, 1.0);
        assert_eq!(r, 255, "R=200 * 3.0 clamps to 255");
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
