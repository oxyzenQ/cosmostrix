// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Custom color palette definitions from config.toml.
//!
//! One mode: `rain` gradient stops + optional `bg`.
//!
//! ```toml
//! [colors-custom.sunset]
//! bg = "#0a0a12"
//! rain = "#1a0033", "#4d0080", "#9933ff", "#cc66ff", "#ffffff"
//! ```
//!
//! Load with `--colors-custom sunset` or use in `adaptive-custom`:
//! ```toml
//! adaptive-custom.22-00 = sunset, monolith, speed=10
//! ```

use std::collections::{BTreeMap, HashMap};

use crossterm::style::Color;

use crate::palette::Palette;

/// A parsed custom color palette definition.
#[derive(Debug, Clone, Default)]
pub struct CustomPaletteDef {
    /// Background color (optional).
    pub bg: Option<Color>,
    /// Gradient stops for the rain trail (tail → head order).
    pub rain: Vec<Color>,
}

impl CustomPaletteDef {
    /// Check if this definition has any color data.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.bg.is_none() && self.rain.is_empty()
    }

    /// Build a cosmostrix `Palette` from this definition.
    pub fn to_palette(&self) -> Result<Palette, String> {
        if self.rain.is_empty() {
            return Err("custom palette needs 'rain' field with at least 2 hex colors".to_string());
        }

        // Defensive: ensure colors is never empty — causes downstream panics.
        if self.rain.len() < 2 {
            return Err("rain needs at least 2 hex colors for a gradient".to_string());
        }

        Ok(Palette {
            colors: self.rain.clone(),
            bg: self.bg,
        })
    }
}

/// Parse a hex color string to a crossterm Color.
///
/// Accepts: `#rrggbb`, `rrggbb`, `#rgb`, `rgb`, `"#rrggbb"` (quoted).
pub fn parse_hex_color(s: &str) -> Result<Color, String> {
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
pub fn collect_colors_custom(cfg: &HashMap<String, String>) -> BTreeMap<String, CustomPaletteDef> {
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
            "bg" | "background" => {
                if let Ok(color) = parse_hex_color(value) {
                    palette.bg = Some(color);
                }
            }
            "rain" => {
                for stop in value.split(',') {
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

/// Look up a custom palette by name and convert it to a cosmostrix Palette.
pub fn load_custom_palette(cfg: &HashMap<String, String>, name: &str) -> Result<Palette, String> {
    let palettes = collect_colors_custom(cfg);
    let normalized = name.trim().to_ascii_lowercase();
    let def = palettes
        .get(&normalized)
        .ok_or_else(|| format!("custom color '{name}' not found in config"))?;
    def.to_palette()
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
        assert_eq!(palette.colors.len(), 2);
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
        assert_eq!(palette.colors.len(), 2);
    }

    #[test]
    fn load_custom_palette_case_insensitive() {
        let mut cfg = HashMap::new();
        cfg.insert(
            "colors-custom.MyTheme.rain".to_string(),
            "#000000, #ffffff".to_string(),
        );
        let palette = load_custom_palette(&cfg, "mytheme").unwrap();
        assert_eq!(palette.colors.len(), 2);
    }
}
