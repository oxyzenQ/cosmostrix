// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Custom color palette definitions from config.toml.
//!
//! Users can define named custom palettes in config.toml using
//! Alacritty-style notation (with cosmostrix-specific extensions):
//!
//! ```toml
//! colors-custom.mytheme.bg = "#0a0a12"
//! colors-custom.mytheme.head = "#ffffff"
//! colors-custom.mytheme.stops = "#1a0033", "#4d0080", "#9933ff", "#cc66ff", "#ffffff"
//! colors-custom.mytheme.normal.red = "#fe0100"
//! colors-custom.mytheme.normal.green = "#33ff00"
//! colors-custom.mytheme.normal.blue = "#0066ff"
//! colors-custom.mytheme.bright.red = "#ff4444"
//! colors-custom.mytheme.bright.green = "#66ff66"
//! colors-custom.mytheme.bright.blue = "#4499ff"
//! ```
//!
//! Load with `--color-custom mytheme` or use in `adaptive-custom`:
//! ```toml
//! adaptive-custom.22-00 = mytheme, monolith, speed=10
//! ```
//!
//! ## Palette construction
//!
//! Custom palettes are converted to cosmostrix's `Palette` struct
//! (`{ colors: Vec<Color>, bg: Option<Color> }`). Two construction modes:
//!
//! 1. **Stops mode** (cosmostrix-specific): `stops = "#color1", "#color2", ...`
//!    — the colors are used directly as the gradient (trail → head order).
//!
//! 2. **Alacritty mode**: `normal.*` + `bright.*` + optional `head` + `bg`
//!    — the 16 ANSI colors are interpolated into a gradient from darkest
//!    (normal.black) to brightest (bright.white or head).

// Production callers: main.rs (--color-custom), live_config.rs (live reload),
// event_loop.rs (adaptive-custom integration — step 7).

use std::collections::{BTreeMap, HashMap};

use crossterm::style::Color;

use crate::palette::Palette;

/// A parsed custom color palette definition.
#[derive(Debug, Clone, Default)]
pub struct CustomPaletteDef {
    /// Background color (optional).
    pub bg: Option<Color>,
    /// Head (brightest) color — the leading character of each rain stream.
    pub head: Option<Color>,
    /// Body color — mid-gradient color for the rain trail body.
    pub body: Option<Color>,
    /// Tail color — dimmest color at the end of the rain trail.
    pub tail: Option<Color>,
    /// Gradient stops for the full rain trail (replaces stops, v16).
    /// Comma-separated hex colors from tail (darkest) to head (brightest).
    /// If set, takes priority over head/body/tail interpolation.
    pub rain: Vec<Color>,
    /// Legacy gradient stops (alias for rain, kept for backward compat).
    pub stops: Vec<Color>,
    /// Normal ANSI colors (Alacritty-style, advanced/optional).
    pub normal: AnsiColors,
    /// Bright ANSI colors (Alacritty-style, advanced/optional).
    pub bright: AnsiColors,
}

/// The 8 standard ANSI colors (normal or bright variant).
#[derive(Debug, Clone, Default)]
pub struct AnsiColors {
    pub black: Option<Color>,
    pub red: Option<Color>,
    pub green: Option<Color>,
    pub yellow: Option<Color>,
    pub blue: Option<Color>,
    pub magenta: Option<Color>,
    pub cyan: Option<Color>,
    pub white: Option<Color>,
}

impl CustomPaletteDef {
    /// Check if this definition has any color data.
    pub fn is_empty(&self) -> bool {
        self.bg.is_none()
            && self.head.is_none()
            && self.body.is_none()
            && self.tail.is_none()
            && self.rain.is_empty()
            && self.stops.is_empty()
            && self.normal.black.is_none()
            && self.normal.red.is_none()
            && self.normal.green.is_none()
            && self.normal.yellow.is_none()
            && self.normal.blue.is_none()
            && self.normal.magenta.is_none()
            && self.normal.cyan.is_none()
            && self.normal.white.is_none()
            && self.bright.black.is_none()
            && self.bright.red.is_none()
            && self.bright.green.is_none()
            && self.bright.yellow.is_none()
            && self.bright.blue.is_none()
            && self.bright.magenta.is_none()
            && self.bright.cyan.is_none()
            && self.bright.white.is_none()
    }

    /// Build a cosmostrix `Palette` from this definition.
    ///
    /// Construction priority:
    /// 1. If `rain` is non-empty, use rain stops directly as the gradient.
    /// 2. If `stops` is non-empty (legacy alias), use stops directly.
    /// 3. If head/body/tail are set, build a 3-stop gradient from them.
    /// 4. Otherwise, interpolate from normal/bright ANSI colors.
    /// 5. Background comes from `bg` (or `background` alias).
    pub fn to_palette(&self) -> Result<Palette, String> {
        if self.is_empty() {
            return Err("custom palette has no color definitions".to_string());
        }

        let colors = if !self.rain.is_empty() {
            // rain mode (v16 primary): use directly (tail → head order).
            self.rain.clone()
        } else if !self.stops.is_empty() {
            // Legacy stops mode (backward compat): same as rain.
            self.stops.clone()
        } else if self.head.is_some() || self.body.is_some() || self.tail.is_some() {
            // head/body/tail mode: build a simple gradient.
            self.build_hbt_gradient()?
        } else {
            // Alacritty mode: interpolate from ANSI colors.
            self.interpolate_gradient()?
        };

        // Defensive: ensure colors is never empty — this would cause
        // downstream panics in fill_color_map (gen_range on empty range)
        // and get_attr (index into empty slice). If all parsing silently
        // failed, we must error here rather than crash later.
        if colors.is_empty() {
            return Err("custom palette produced no valid colors — check hex values".to_string());
        }

        Ok(Palette {
            colors,
            bg: self.bg,
        })
    }

    /// Build a gradient from head/body/tail colors.
    ///
    /// If all three are set: [tail → body → head] interpolated to ~24 stops.
    /// If only two are set: interpolate between them.
    /// If only one is set: use it as a single-color palette.
    fn build_hbt_gradient(&self) -> Result<Vec<Color>, String> {
        let mut stops: Vec<Color> = Vec::new();
        if let Some(tail) = self.tail {
            stops.push(tail);
        }
        if let Some(body) = self.body {
            stops.push(body);
        }
        if let Some(head) = self.head {
            stops.push(head);
        }

        if stops.is_empty() {
            return Err("no head/body/tail colors set".to_string());
        }

        if stops.len() <= 2 {
            return Ok(stops);
        }

        // Interpolate to ~24 entries for a smooth rain trail.
        let target_len = 24usize;
        let mut gradient: Vec<Color> = Vec::with_capacity(target_len);
        for i in 0..stops.len().saturating_sub(1) {
            let a = stops[i];
            let b = stops[i + 1];
            let steps = target_len / (stops.len() - 1).max(1);
            for s in 0..steps {
                let t = s as f32 / steps as f32;
                gradient.push(lerp_color(a, b, t));
            }
        }
        gradient.push(*stops.last().unwrap());
        Ok(gradient)
    }

    /// Interpolate a gradient from normal/bright ANSI colors.
    ///
    /// The gradient goes from darkest (normal.black) through the normal
    /// colors, then through the bright colors, to the brightest (bright.white
    /// or head). Colors that are None are skipped.
    fn interpolate_gradient(&self) -> Result<Vec<Color>, String> {
        let mut stops: Vec<Color> = Vec::new();

        // Collect in order: normal.black → normal.red → ... → normal.white
        //                  → bright.black → bright.red → ... → bright.white
        // This gives a natural dark-to-bright progression.
        let normal_order = [
            self.normal.black,
            self.normal.red,
            self.normal.green,
            self.normal.yellow,
            self.normal.blue,
            self.normal.magenta,
            self.normal.cyan,
            self.normal.white,
        ];
        let bright_order = [
            self.bright.black,
            self.bright.red,
            self.bright.green,
            self.bright.yellow,
            self.bright.blue,
            self.bright.magenta,
            self.bright.cyan,
            self.bright.white,
        ];

        for &c in normal_order.iter().chain(bright_order.iter()).flatten() {
            stops.push(c);
        }

        // Add head as the brightest stop if specified.
        if let Some(head) = self.head {
            stops.push(head);
        }

        if stops.is_empty() {
            return Err(
                "custom palette has no valid color stops (need stops or normal/bright fields)"
                    .to_string(),
            );
        }

        // If only 1-2 stops, return as-is (cosmostrix handles short gradients).
        if stops.len() <= 2 {
            return Ok(stops);
        }

        // Interpolate between stops to create a smooth gradient.
        // cosmostrix palettes typically have 16-32 entries; we interpolate
        // to fill ~24 entries for a smooth rain trail.
        let target_len = 24usize;
        let mut gradient: Vec<Color> = Vec::with_capacity(target_len);

        for i in 0..stops.len().saturating_sub(1) {
            let a = stops[i];
            let b = stops[i + 1];
            let steps = target_len / (stops.len() - 1).max(1);
            for s in 0..steps {
                let t = s as f32 / steps as f32;
                gradient.push(lerp_color(a, b, t));
            }
        }
        // Add the final stop.
        gradient.push(*stops.last().unwrap());

        Ok(gradient)
    }
}

/// Linearly interpolate between two colors.
fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let (ar, ag, ab) = color_to_rgb(a);
    let (br, bg, bb) = color_to_rgb(b);
    let r = (ar as f32 + (br as f32 - ar as f32) * t).round() as u8;
    let g = (ag as f32 + (bg as f32 - ag as f32) * t).round() as u8;
    let blue = (ab as f32 + (bb as f32 - ab as f32) * t).round() as u8;
    Color::Rgb { r, g, b: blue }
}

/// Convert a crossterm Color to (r, g, b) tuple.
fn color_to_rgb(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb { r, g, b } => (r, g, b),
        Color::AnsiValue(n) => {
            // Approximate ANSI 256 → RGB (simplified).
            if n < 16 {
                ansi16_to_rgb(n)
            } else if n < 232 {
                let n = n - 16;
                let r = (n / 36) * 51;
                let g = ((n / 6) % 6) * 51;
                let b = (n % 6) * 51;
                (r, g, b)
            } else {
                let v = ((n - 232) * 10) + 8;
                (v, v, v)
            }
        }
        _ => (128, 128, 128), // fallback for named colors
    }
}

/// Convert ANSI 16-color index to RGB.
fn ansi16_to_rgb(n: u8) -> (u8, u8, u8) {
    match n {
        0 => (0, 0, 0),        // black
        1 => (128, 0, 0),      // red
        2 => (0, 128, 0),      // green
        3 => (128, 128, 0),    // yellow
        4 => (0, 0, 128),      // blue
        5 => (128, 0, 128),    // magenta
        6 => (0, 128, 128),    // cyan
        7 => (192, 192, 192),  // white
        8 => (128, 128, 128),  // bright black
        9 => (255, 0, 0),      // bright red
        10 => (0, 255, 0),     // bright green
        11 => (255, 255, 0),   // bright yellow
        12 => (0, 0, 255),     // bright blue
        13 => (255, 0, 255),   // bright magenta
        14 => (0, 255, 255),   // bright cyan
        15 => (255, 255, 255), // bright white
        _ => (128, 128, 128),
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
        // Short form #rgb → #rrggbb
        let r = u8::from_str_radix(&s[0..1].repeat(2), 16).map_err(|e| e.to_string())?;
        let g = u8::from_str_radix(&s[1..2].repeat(2), 16).map_err(|e| e.to_string())?;
        let b = u8::from_str_radix(&s[2..3].repeat(2), 16).map_err(|e| e.to_string())?;
        Ok(Color::Rgb { r, g, b })
    } else {
        Err(format!(
            "invalid hex color '{s}' (expected #rrggbb or rrggbb, got {s_len} chars)",
            s_len = s.len()
        ))
    }
}

/// Collect all custom color palette definitions from the config HashMap.
///
/// Returns a BTreeMap keyed by palette name (lowercased).
/// Each value is a `CustomPaletteDef` that can be converted to a `Palette`
/// via `to_palette()`.
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
            "head" => {
                if let Ok(color) = parse_hex_color(value) {
                    palette.head = Some(color);
                }
            }
            "body" => {
                if let Ok(color) = parse_hex_color(value) {
                    palette.body = Some(color);
                }
            }
            "tail" => {
                if let Ok(color) = parse_hex_color(value) {
                    palette.tail = Some(color);
                }
            }
            "rain" => {
                // v16: Comma-separated gradient stops (primary format).
                for stop in value.split(',') {
                    if let Ok(color) = parse_hex_color(stop) {
                        palette.rain.push(color);
                    }
                }
            }
            "stops" => {
                // Legacy alias for rain (backward compat).
                for stop in value.split(',') {
                    if let Ok(color) = parse_hex_color(stop) {
                        palette.stops.push(color);
                    }
                }
            }
            "normal.red" => palette.normal.red = parse_hex_color(value).ok(),
            "normal.green" => palette.normal.green = parse_hex_color(value).ok(),
            "normal.blue" => palette.normal.blue = parse_hex_color(value).ok(),
            "normal.yellow" => palette.normal.yellow = parse_hex_color(value).ok(),
            "normal.cyan" => palette.normal.cyan = parse_hex_color(value).ok(),
            "normal.magenta" => palette.normal.magenta = parse_hex_color(value).ok(),
            "normal.white" => palette.normal.white = parse_hex_color(value).ok(),
            "normal.black" => palette.normal.black = parse_hex_color(value).ok(),
            "bright.red" => palette.bright.red = parse_hex_color(value).ok(),
            "bright.green" => palette.bright.green = parse_hex_color(value).ok(),
            "bright.blue" => palette.bright.blue = parse_hex_color(value).ok(),
            "bright.yellow" => palette.bright.yellow = parse_hex_color(value).ok(),
            "bright.cyan" => palette.bright.cyan = parse_hex_color(value).ok(),
            "bright.magenta" => palette.bright.magenta = parse_hex_color(value).ok(),
            "bright.white" => palette.bright.white = parse_hex_color(value).ok(),
            "bright.black" => palette.bright.black = parse_hex_color(value).ok(),
            _ => {}
        }
    }

    palettes
}

/// Look up a custom palette by name and convert it to a cosmostrix Palette.
///
/// Returns `Ok(Palette)` if found and valid, `Err(msg)` if not found or
/// invalid.
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
    fn collect_colors_custom_stops_mode() {
        let mut cfg = HashMap::new();
        cfg.insert(
            "colors-custom.mytheme.stops".to_string(),
            "#1a0033, #4d0080, #9933ff, #cc66ff, #ffffff".to_string(),
        );
        cfg.insert(
            "colors-custom.mytheme.bg".to_string(),
            "#0a0a12".to_string(),
        );

        let palettes = collect_colors_custom(&cfg);
        assert!(palettes.contains_key("mytheme"));
        let def = &palettes["mytheme"];
        assert_eq!(def.stops.len(), 5);
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
    fn collect_colors_custom_alacritty_mode() {
        let mut cfg = HashMap::new();
        cfg.insert(
            "colors-custom.acme.normal.red".to_string(),
            "#fe0100".to_string(),
        );
        cfg.insert(
            "colors-custom.acme.normal.green".to_string(),
            "#33ff00".to_string(),
        );
        cfg.insert(
            "colors-custom.acme.bright.red".to_string(),
            "#ff4444".to_string(),
        );

        let palettes = collect_colors_custom(&cfg);
        let def = &palettes["acme"];
        assert_eq!(def.normal.red, Some(Color::Rgb { r: 254, g: 1, b: 0 }));
        assert_eq!(
            def.normal.green,
            Some(Color::Rgb {
                r: 51,
                g: 255,
                b: 0
            })
        );
        assert_eq!(
            def.bright.red,
            Some(Color::Rgb {
                r: 255,
                g: 68,
                b: 68
            })
        );
    }

    #[test]
    fn to_palette_stops_mode() {
        let def = CustomPaletteDef {
            stops: vec![
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
            ..Default::default()
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
    fn to_palette_alacritty_mode_interpolates() {
        let def = CustomPaletteDef {
            normal: AnsiColors {
                black: Some(Color::Rgb { r: 0, g: 0, b: 0 }),
                white: Some(Color::Rgb {
                    r: 200,
                    g: 200,
                    b: 200,
                }),
                ..Default::default()
            },
            bright: AnsiColors {
                white: Some(Color::Rgb {
                    r: 255,
                    g: 255,
                    b: 255,
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        let palette = def.to_palette().unwrap();
        // Should have interpolated gradient (more than 3 entries).
        assert!(palette.colors.len() > 3);
        // First color should be near black, last near white.
        let first = palette.colors.first().unwrap();
        let last = palette.colors.last().unwrap();
        if let (Color::Rgb { r: fr, .. }, Color::Rgb { r: lr, .. }) = (first, last) {
            assert!(*fr < *lr, "gradient should go dark to bright");
        }
    }

    #[test]
    fn to_palette_empty_fails() {
        let def = CustomPaletteDef::default();
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
            "colors-custom.mytheme.stops".to_string(),
            "#000000, #ffffff".to_string(),
        );
        let palette = load_custom_palette(&cfg, "mytheme").unwrap();
        assert_eq!(palette.colors.len(), 2);
    }

    #[test]
    fn load_custom_palette_case_insensitive() {
        let mut cfg = HashMap::new();
        cfg.insert(
            "colors-custom.MyTheme.stops".to_string(),
            "#000000, #ffffff".to_string(),
        );
        // Lookup with different case should find it.
        let palette = load_custom_palette(&cfg, "mytheme").unwrap();
        assert_eq!(palette.colors.len(), 2);
    }
}
