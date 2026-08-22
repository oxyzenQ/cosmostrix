// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Custom color palette definitions from config.toml.
//!
//! One mode: `rain` gradient stops + optional `bg`.
//!
//! ```toml
//! [colors-custom.sunset]
//! bg = "#0a0a12"
//! rain = "#1a0033", "#4d0080", "#9933ff", "#cc66ff", "#e6b3ff", "#f2ccff", "#ffffff"
//! ```
//!
//! Load with `--colors-custom sunset` or use in an ambient phase:
//! ```toml
//! ambient.22-00 = sunset
//! ```

use std::collections::{BTreeMap, HashMap};

use crossterm::style::Color;

use crate::chroma_dragon_engine::palette::colors_from_stops;
use crate::palette::Palette;
use crate::runtime::ColorMode;

/// Number of perceptual samples produced from the user's `rain` stops via
/// the OKLab polar gradient engine. Matches the expansion applied to
/// built-in themes (`catalog.rs:933` routes through `colors_from_stops` with
/// `steps = 9`). Without this, colors-custom palettes would only carry the
/// raw user stops (typically 3-5 entries), producing visible banding on long
/// rain trails — every built-in theme avoids this by expanding to 9
/// perceptually-uniform OKLab samples.
const COLORS_CUSTOM_PALETTE_STEPS: usize = 9;

/// A parsed custom color palette definition.
#[derive(Debug, Clone, Default)]
pub(crate) struct CustomPaletteDef {
    /// Background color (optional).
    pub bg: Option<Color>,
    /// Gradient stops for the rain trail (tail → head order).
    pub rain: Vec<Color>,
}

impl CustomPaletteDef {
    /// Build a cosmostrix `Palette` from this definition.
    ///
    /// The user's `rain` stops are expanded through the same OKLab polar
    /// gradient engine (`chroma::gradient::gradient_from_stops_oklab`) that
    /// built-in themes use. This produces `COLORS_CUSTOM_PALETTE_STEPS`
    /// perceptually-uniform samples from the raw stops, eliminating banding
    /// on long rain trails and bringing colors-custom palettes onto equal
    /// footing with built-in themes like `synthwave`, `cosmos`, etc.
    ///
    /// Before this change, `to_palette()` returned the raw `rain` stops
    /// verbatim — a 3-stop palette stayed 3 entries while every built-in
    /// theme expanded to 9 OKLab-interpolated entries. That asymmetry was
    /// the only place colors-custom diverged from the chroma engine.
    pub(crate) fn to_palette(&self) -> Result<Palette, String> {
        if self.rain.is_empty() {
            return Err("custom palette needs 'rain' field with at least 2 hex colors".to_string());
        }
        if self.rain.len() < 2 {
            return Err("rain needs at least 2 hex colors for a gradient".to_string());
        }
        // Convert the user's Color stops to RGB tuples for the gradient
        // engine. All colors-custom stops are parsed from hex by
        // `parse_hex_color`, so they are always `Color::Rgb`. The
        // `color_to_rgb` helper handles the AnsiValue fallback path too.
        let stops_rgb: Vec<(u8, u8, u8)> = self
            .rain
            .iter()
            .map(|c| crate::chroma_dragon_engine::palette::color_to_rgb(*c))
            .collect();
        // TrueColor is the only mode that makes sense for colors-custom:
        // user-supplied hex colors are by definition 24-bit. Mono mode would
        // collapse everything to white, defeating the purpose.
        let colors = colors_from_stops(
            ColorMode::TrueColor,
            &stops_rgb,
            COLORS_CUSTOM_PALETTE_STEPS,
        );
        Ok(Palette {
            colors,
            bg: self.bg,
        })
    }
}

/// Parse a hex color string to a crossterm Color.
///
/// Accepts: `#rrggbb`, `rrggbb`, `#rgb`, `rgb`, `"#rrggbb"` (quoted).
pub(crate) fn parse_hex_color(s: &str) -> Result<Color, String> {
    let s = s.trim().trim_matches('"').trim();
    let s = s.strip_prefix('#').unwrap_or(s);

    if s.len() == 6 && s.chars().all(|c| c.is_ascii_hexdigit()) {
        let r = u8::from_str_radix(&s[0..2], 16).map_err(|e| e.to_string())?;
        let g = u8::from_str_radix(&s[2..4], 16).map_err(|e| e.to_string())?;
        let b = u8::from_str_radix(&s[4..6], 16).map_err(|e| e.to_string())?;
        Ok(Color::Rgb { r, g, b })
    } else if s.len() == 3 && s.chars().all(|c| c.is_ascii_hexdigit()) {
        let r = u8::from_str_radix(&s[0..1].repeat(2), 16).map_err(|e| e.to_string())?;
        let g = u8::from_str_radix(&s[1..2].repeat(2), 16).map_err(|e| e.to_string())?;
        let b = u8::from_str_radix(&s[2..3].repeat(2), 16).map_err(|e| e.to_string())?;
        Ok(Color::Rgb { r, g, b })
    } else {
        Err(format!(
            "invalid hex color '{s}' (expected #rrggbb or rrggbb)"
        ))
    }
}

/// Collect all custom color palette definitions from the config HashMap.
#[must_use]
pub(crate) fn collect_colors_custom(
    cfg: &HashMap<String, String>,
) -> BTreeMap<String, CustomPaletteDef> {
    let mut palettes: BTreeMap<String, CustomPaletteDef> = BTreeMap::new();

    for (key, value) in cfg {
        let Some(rest) = key.strip_prefix("colors-custom.") else {
            continue;
        };
        let Some((name, field)) = rest.split_once('.') else {
            continue;
        };
        let name = name.to_ascii_lowercase();
        let palette = palettes.entry(name).or_default();

        match field {
            "bg" => {
                if let Ok(color) = parse_hex_color(value) {
                    palette.bg = Some(color);
                }
            }
            // (bug #8): `stops` is a deprecated alias for `rain`.
            // The validator still accepts it (with a --testconf deprecation
            // warning); the runtime parser treats it identically to `rain`.
            // (CLI-D-2): emit a one-time deprecation warning at runtime
            // too — previously only --testconf warned, so users who never
            // ran --testconf used the deprecated alias indefinitely with no
            // signal.
            // AB-10 (rain-screen cleanliness): buffer the warning to
            // `LIVE_RELOAD_RUNTIME_WARNINGS` instead of eprintln. This
            // function runs on every config save via the live-reload path,
            // and the eprintln fired while the alt screen was active,
            // leaking into the rain matrix. main.rs drains the buffer
            // AFTER Terminal::drop restores the main screen.
            "rain" | "stops" => {
                if field == "stops" {
                    crate::live_config::push_runtime_warning(
                        "colors-custom: '.stops' is deprecated — rename to '.rain' (alias removed in a future release)",
                    );
                }
                // v25 masterclass: support both CSV string and TOML array format.
                // CSV: "#1a0033", "#4d0080", "#9933ff"
                // Array: ["#1a0033", "#4d0080", "#9933ff", ...] (7-stop)
                let stops = if value.trim_start().starts_with('[') {
                    parse_rain_array(value)
                } else {
                    value.split(',').map(|s| s.trim()).collect()
                };
                for stop in &stops {
                    if let Ok(color) = parse_hex_color(stop) {
                        palette.rain.push(color);
                    }
                }
            }
            _ => {}
        }
    }

    palettes
}

/// Parse a TOML array-style rain value: `["#1a0033", "#4d0080", ...]`.
/// Strips brackets, splits by comma, trims whitespace + quotes from each element.
/// Returns the list of stop strings (caller parses hex).
fn parse_rain_array(value: &str) -> Vec<&str> {
    let s = value.trim();
    let s = s.strip_prefix('[').unwrap_or(s);
    let s = s.strip_suffix(']').unwrap_or(s);
    s.split(',')
        .map(|e| e.trim().trim_matches('"').trim())
        .filter(|e| !e.is_empty())
        .collect()
}

/// Look up a custom palette by name and convert it to a cosmostrix Palette.
pub(crate) fn load_custom_palette(
    cfg: &HashMap<String, String>,
    name: &str,
) -> Result<Palette, String> {
    let palettes = collect_colors_custom(cfg);
    let normalized = name.trim().to_ascii_lowercase();
    let def = palettes.get(&normalized).ok_or_else(|| {
        let mut available: Vec<String> = palettes.keys().cloned().collect();
        available.sort();
        let list = if available.is_empty() {
            "<none defined>".to_string()
        } else {
            available.join(", ")
        };
        format!(
            "custom color '{name}' not found in config\nexpected one of: {list}\n\n  Use --list-colors to see built-in and custom palettes."
        )
    })?;
    def.to_palette()
}

/// Phase 5 closure (P1-#5): check whether `name` refers to a defined
/// `[colors-custom.<name>]` block in `cfg`. Used by profile/scene-custom
/// layers to resolve custom color names (matching top-level config_apply
/// behavior which resolves via `parse_color_scheme || colors-custom lookup`).
#[must_use]
pub(crate) fn is_colors_custom_name(cfg: &HashMap<String, String>, name: &str) -> bool {
    let palettes = collect_colors_custom(cfg);
    palettes.contains_key(&name.trim().to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_color_full_with_hash() {
        let c = parse_hex_color("#ff0000").unwrap();
        assert_eq!(c, Color::Rgb { r: 255, g: 0, b: 0 });
    }

    #[test]
    fn parse_hex_color_full_without_hash() {
        let c = parse_hex_color("00ff00").unwrap();
        assert_eq!(c, Color::Rgb { r: 0, g: 255, b: 0 });
    }

    #[test]
    fn parse_hex_color_short_with_hash() {
        let c = parse_hex_color("#0f0").unwrap();
        assert_eq!(c, Color::Rgb { r: 0, g: 255, b: 0 });
    }

    #[test]
    fn parse_hex_color_quoted() {
        let c = parse_hex_color("\"#4488ff\"").unwrap();
        assert_eq!(
            c,
            Color::Rgb {
                r: 68,
                g: 136,
                b: 255
            }
        );
    }

    #[test]
    fn parse_hex_color_invalid() {
        assert!(parse_hex_color("#gg0000").is_err());
        assert!(parse_hex_color("xyz").is_err());
        assert!(parse_hex_color("").is_err());
    }

    #[test]
    fn collect_colors_custom_rain_mode() {
        let mut cfg = HashMap::new();
        cfg.insert(
            "colors-custom.mytheme.rain".to_string(),
            "#1a0033, #4d0080, #9933ff, #cc66ff, #ffffff".to_string(),
        );
        cfg.insert(
            "colors-custom.mytheme.bg".to_string(),
            "#0a0a12".to_string(),
        );

        let palettes = collect_colors_custom(&cfg);
        assert!(palettes.contains_key("mytheme"));
        let def = &palettes["mytheme"];
        assert_eq!(def.rain.len(), 5);
        assert_eq!(
            def.bg,
            Some(Color::Rgb {
                r: 10,
                g: 10,
                b: 18
            })
        );
    }

    #[test]
    fn to_palette_rain_mode() {
        let def = CustomPaletteDef {
            rain: vec![
                Color::Rgb { r: 0, g: 0, b: 0 },
                Color::Rgb {
                    r: 255,
                    g: 255,
                    b: 255,
                },
            ],
            bg: Some(Color::Rgb {
                r: 10,
                g: 10,
                b: 18,
            }),
        };
        let palette = def.to_palette().unwrap();
        // masterclass: 2 raw stops expand to 9 OKLab-polar samples.
        assert_eq!(palette.colors.len(), COLORS_CUSTOM_PALETTE_STEPS);
        assert_eq!(
            palette.bg,
            Some(Color::Rgb {
                r: 10,
                g: 10,
                b: 18
            })
        );
    }

    #[test]
    fn to_palette_empty_fails() {
        let def = CustomPaletteDef::default();
        assert!(def.to_palette().is_err());
    }

    #[test]
    fn to_palette_single_color_fails() {
        let def = CustomPaletteDef {
            rain: vec![Color::Rgb { r: 0, g: 0, b: 0 }],
            ..Default::default()
        };
        assert!(def.to_palette().is_err());
    }

    #[test]
    fn load_custom_palette_not_found() {
        let cfg = HashMap::new();
        assert!(load_custom_palette(&cfg, "nonexistent").is_err());
    }

    #[test]
    fn load_custom_palette_found() {
        let mut cfg = HashMap::new();
        cfg.insert(
            "colors-custom.mytheme.rain".to_string(),
            "#000000, #ffffff".to_string(),
        );
        let palette = load_custom_palette(&cfg, "mytheme").unwrap();
        // masterclass: 2 CSV stops expand to 9 OKLab-polar samples.
        assert_eq!(palette.colors.len(), COLORS_CUSTOM_PALETTE_STEPS);
    }

    #[test]
    fn load_custom_palette_case_insensitive() {
        let mut cfg = HashMap::new();
        cfg.insert(
            "colors-custom.MyTheme.rain".to_string(),
            "#000000, #ffffff".to_string(),
        );
        let palette = load_custom_palette(&cfg, "mytheme").unwrap();
        // masterclass: 2 CSV stops expand to 9 OKLab-polar samples.
        assert_eq!(palette.colors.len(), COLORS_CUSTOM_PALETTE_STEPS);
    }

    /// v25 masterclass: TOML array format for rain field.
    #[test]
    fn rain_array_format_parses_7_stops() {
        let mut cfg = HashMap::new();
        cfg.insert(
            "colors-custom.mythme.rain".to_string(),
            "[\"#1a0033\", \"#4d0080\", \"#9933ff\", \"#cc66ff\", \"#e6b3ff\", \"#f2ccff\", \"#ffffff\"]"
                .to_string(),
        );
        // Verify the 7 stops were parsed correctly by inspecting the raw
        // CustomPaletteDef before to_palette() expands them.
        let collected = collect_colors_custom(&cfg);
        let raw = collected.get("mythme").expect("palette must be collected");
        assert_eq!(raw.rain.len(), 7, "array format must parse 7 raw stops");
        // After to_palette(), the 7 stops expand to COLORS_CUSTOM_PALETTE_STEPS
        // via the OKLab polar gradient engine (same as built-in themes).
        let palette = load_custom_palette(&cfg, "mythme").unwrap();
        assert_eq!(
            palette.colors.len(),
            COLORS_CUSTOM_PALETTE_STEPS,
            "expanded palette must have COLORS_CUSTOM_PALETTE_STEPS entries"
        );
    }

    /// v25 masterclass: CSV format still works (backward compat).
    #[test]
    fn rain_csv_format_still_works() {
        let mut cfg = HashMap::new();
        cfg.insert(
            "colors-custom.oldstyle.rain".to_string(),
            "#000000, #ffffff".to_string(),
        );
        let palette = load_custom_palette(&cfg, "oldstyle").unwrap();
        // masterclass: 2 CSV stops expand to 9 OKLab-polar samples.
        assert_eq!(
            palette.colors.len(),
            COLORS_CUSTOM_PALETTE_STEPS,
            "CSV format must still work"
        );
    }

    /// masterclass: colors-custom must flow through the same OKLab
    /// polar gradient engine as built-in themes. This integration test
    /// asserts the two properties that prove the bypass is fixed:
    ///
    /// 1. **Expansion**: 2 raw stops produce COLORS_CUSTOM_PALETTE_STEPS
    ///    palette entries (not the raw 2).
    /// 2. **Midpoint saturation**: the middle palette entry is distinct
    ///    from both endpoints (after the palette-relative floor is
    ///    applied). This proves the polar OKLab engine is producing
    ///    interpolated colors, not just clamping to one endpoint.
    ///
    /// Note: exact endpoint preservation is NOT asserted here. The
    /// palette-relative floor (`apply_palette_relative_floor` in
    /// `palette.rs:383`) intentionally boosts dark endpoints to prevent
    /// total black crush on long rain trails. Every built-in theme
    /// undergoes the same floor — so endpoint equality is not a property
    /// of the chroma pipeline. The `to_palette_matches_builtin_gradient_path`
    /// test below is the authoritative byte-match proof.
    #[test]
    fn to_palette_routes_through_oklab_polar_engine() {
        let black = Color::Rgb { r: 0, g: 0, b: 0 };
        let white = Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        };
        let def = CustomPaletteDef {
            rain: vec![black, white],
            bg: None,
        };
        let palette = def.to_palette().expect("palette must build");

        // (1) Expansion: 2 raw stops -> 9 OKLab-polar samples.
        assert_eq!(
            palette.colors.len(),
            COLORS_CUSTOM_PALETTE_STEPS,
            "2 raw stops must expand to COLORS_CUSTOM_PALETTE_STEPS samples"
        );

        // (2) Midpoint saturation: the middle entry must not be equal to
        // either endpoint (after floor). A black-to-white gradient should
        // produce a mid-gray at the midpoint, distinct from both endpoints
        // even after the floor boosts black.
        let mid = palette
            .colors
            .get(COLORS_CUSTOM_PALETTE_STEPS / 2)
            .expect("midpoint must exist");
        let floor_first = palette.colors.first().expect("first entry must exist");
        let floor_last = palette.colors.last().expect("last entry must exist");
        assert_ne!(
            mid, floor_first,
            "midpoint must be interpolated, not clamped to first endpoint"
        );
        assert_ne!(
            mid, floor_last,
            "midpoint must be interpolated, not clamped to last endpoint"
        );
    }

    /// masterclass: colors-custom must produce the SAME output as a
    /// built-in theme that uses the same raw stops. This is the strongest
    /// possible proof that the bypass is fixed — colors-custom and built-in
    /// themes now share the identical code path.
    #[test]
    fn to_palette_matches_builtin_gradient_path() {
        use crate::chroma_dragon_engine::palette::{color_to_rgb, colors_from_stops};
        use crate::runtime::ColorMode;

        let stops = vec![
            Color::Rgb { r: 26, g: 0, b: 51 }, // #1a0033
            Color::Rgb {
                r: 77,
                g: 0,
                b: 128,
            }, // #4d0080
            Color::Rgb {
                r: 153,
                g: 51,
                b: 255,
            }, // #9933ff
        ];
        let def = CustomPaletteDef {
            rain: stops.clone(),
            bg: None,
        };
        let palette = def.to_palette().expect("palette must build");

        // Build the same palette directly through the chroma engine, exactly
        // like catalog.rs:933 does for built-in themes.
        let stops_rgb: Vec<(u8, u8, u8)> = stops.iter().map(|c| color_to_rgb(*c)).collect();
        let expected = colors_from_stops(
            ColorMode::TrueColor,
            &stops_rgb,
            COLORS_CUSTOM_PALETTE_STEPS,
        );

        assert_eq!(
            palette.colors, expected,
            "colors-custom palette must byte-match the chroma engine output for the same stops"
        );
    }
}
