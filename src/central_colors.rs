// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Central color theme definitions — plug-and-play modular system.
//!
//! To add a new theme: add one entry to `THEMES` below. Zero code changes
//! needed elsewhere. The theme is auto-discovered by --list-colors and
//! available via `--color <name>`.
//!
//! ## Theme definition formats
//!
//! Each theme can use one of:
//! - `Stops`: RGB gradient stops + step count (gamma-correct interpolation)
//! - `Ansi`: ANSI 256-color indices (legacy, for themes that predate TrueColor)
//! - `Rgb`: Direct RGB values (exact colors, no interpolation)
//!
//! Color16 fallbacks are auto-generated from the TrueColor/Ansi palette
//! via nearest-color matching in `palette::rgb_to_color16()`.
//!
//! ## Background
//!
//! `bg: None` means use the terminal's default background.
//! `bg: Some((r, g, b))` means solid black or the specified color.

use crossterm::style::Color;

use crate::palette::{colors_from_rgb, colors_from_stops, from_ansi_list};
use crate::runtime::{ColorMode, ColorScheme};

/// A single theme definition. Add entries to `THEMES` to register new themes.
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub struct ThemeDef {
    pub scheme: ColorScheme,
    pub name: &'static str,
    pub def: ThemeColors,
}

/// Color definition variants for different palette construction methods.
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum ThemeColors {
    /// RGB gradient stops + step count. Gamma-correct interpolation.
    /// Used by most space themes (Stars, Mars, Neptune, etc.)
    Stops {
        stops: &'static [(u8, u8, u8)],
        steps: usize,
    },
    /// ANSI 256-color indices. Used by legacy themes (Green, Red, Blue, etc.)
    Ansi(&'static [u8]),
    /// Direct RGB values. Used by Spectrum20 (exact color list, no interpolation).
    Rgb(&'static [(u8, u8, u8)]),
    /// Stops with custom Color16 fallback.
    StopsWithC16 {
        stops: &'static [(u8, u8, u8)],
        steps: usize,
        c16: &'static [Color],
    },
    /// ANSI with custom Color16 fallback.
    AnsiWithC16 {
        ansi: &'static [u8],
        c16: &'static [Color],
    },
}

/// All built-in themes. To add a new theme, add one entry here.
#[allow(dead_code)]
pub static THEMES: &[ThemeDef] = &[
    // ── Classic themes (ANSI 256 with Color16 fallback) ──
    ThemeDef {
        scheme: ColorScheme::Green,
        name: "green",
        def: ThemeColors::StopsWithC16 {
            stops: &[
                (0, 180, 0),
                (0, 255, 0),
                (0, 145, 30),
                (20, 200, 60),
                (75, 235, 95),
                (135, 255, 150),
                (185, 255, 210),
            ],
            steps: 7,
            c16: &[Color::DarkGreen, Color::Green],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Green2,
        name: "green2",
        def: ThemeColors::StopsWithC16 {
            stops: &[
                (0, 90, 10),
                (0, 160, 40),
                (60, 220, 100),
                (120, 255, 160),
                (200, 255, 230),
                (240, 255, 250),
            ],
            steps: 7,
            c16: &[
                Color::DarkGrey,
                Color::DarkGreen,
                Color::Green,
                Color::White,
            ],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Green3,
        name: "green3",
        def: ThemeColors::StopsWithC16 {
            stops: &[
                (0, 60, 50),
                (5, 110, 80),
                (10, 165, 110),
                (40, 220, 150),
                (120, 255, 200),
                (180, 255, 230),
            ],
            steps: 7,
            c16: &[Color::DarkGreen, Color::White],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Yellow,
        name: "yellow",
        def: ThemeColors::AnsiWithC16 {
            ansi: &[100, 142, 184, 226, 227, 229, 230],
            c16: &[Color::DarkGrey, Color::Yellow, Color::White],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Orange,
        name: "orange",
        def: ThemeColors::AnsiWithC16 {
            ansi: &[52, 94, 130, 166, 202, 208, 231],
            c16: &[Color::Red, Color::Grey],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Red,
        name: "red",
        def: ThemeColors::AnsiWithC16 {
            ansi: &[234, 52, 88, 124, 160, 196, 217],
            c16: &[Color::DarkRed, Color::Red, Color::White],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Blue,
        name: "blue",
        def: ThemeColors::AnsiWithC16 {
            ansi: &[234, 17, 18, 19, 20, 21, 75, 159],
            c16: &[Color::DarkBlue, Color::Blue, Color::White],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Cyan,
        name: "cyan",
        def: ThemeColors::AnsiWithC16 {
            ansi: &[24, 25, 31, 32, 38, 45, 159],
            c16: &[Color::DarkCyan, Color::Cyan, Color::White],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Gold,
        name: "gold",
        def: ThemeColors::AnsiWithC16 {
            ansi: &[58, 94, 172, 178, 228, 230, 231],
            c16: &[
                Color::DarkGrey,
                Color::DarkYellow,
                Color::Yellow,
                Color::White,
            ],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Purple,
        name: "purple",
        def: ThemeColors::AnsiWithC16 {
            ansi: &[60, 61, 62, 63, 69, 111, 225],
            c16: &[Color::Magenta, Color::Grey],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Neon,
        name: "neon",
        def: ThemeColors::AnsiWithC16 {
            ansi: &[17, 18, 19, 54, 93, 129, 201, 51, 231],
            c16: &[Color::Blue, Color::Magenta, Color::Cyan, Color::White],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Fire,
        name: "fire",
        def: ThemeColors::AnsiWithC16 {
            ansi: &[52, 88, 124, 160, 196, 202, 208, 214, 226, 231],
            c16: &[
                Color::DarkRed,
                Color::Red,
                Color::DarkYellow,
                Color::Yellow,
                Color::White,
            ],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Ocean,
        name: "ocean",
        def: ThemeColors::AnsiWithC16 {
            ansi: &[17, 18, 19, 24, 30, 37, 44, 51, 87, 159, 231],
            c16: &[
                Color::DarkBlue,
                Color::Blue,
                Color::DarkCyan,
                Color::Cyan,
                Color::White,
            ],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Forest,
        name: "forest",
        def: ThemeColors::AnsiWithC16 {
            ansi: &[22, 28, 34, 40, 46, 82, 118, 154, 190, 229, 231],
            c16: &[Color::DarkGreen, Color::Green, Color::Yellow, Color::White],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Vaporwave,
        name: "vaporwave",
        def: ThemeColors::AnsiWithC16 {
            ansi: &[
                53, 54, 55, 134, 177, 219, 214, 220, 227, 229, 87, 123, 159, 195, 231,
            ],
            c16: &[
                Color::Magenta,
                Color::Magenta,
                Color::Yellow,
                Color::Cyan,
                Color::White,
            ],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Gray,
        name: "gray",
        def: ThemeColors::AnsiWithC16 {
            ansi: &[234, 237, 240, 243, 246, 249, 251, 252, 231],
            c16: &[Color::DarkGrey, Color::Grey, Color::White],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Rainbow,
        name: "rainbow",
        def: ThemeColors::AnsiWithC16 {
            ansi: &[196, 208, 226, 46, 21, 93, 201],
            c16: &[
                Color::Red,
                Color::Blue,
                Color::Yellow,
                Color::Green,
                Color::Cyan,
                Color::Magenta,
            ],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Snow,
        name: "snow",
        def: ThemeColors::AnsiWithC16 {
            ansi: &[234, 240, 250, 252, 231, 117, 159],
            c16: &[Color::DarkGrey, Color::Grey, Color::White, Color::Cyan],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Aurora,
        name: "aurora",
        def: ThemeColors::AnsiWithC16 {
            ansi: &[22, 28, 34, 40, 45, 51, 93, 129, 159],
            c16: &[Color::DarkGreen, Color::Green, Color::Cyan, Color::Magenta],
        },
    },
    ThemeDef {
        scheme: ColorScheme::FancyDiamond,
        name: "fancy-diamond",
        def: ThemeColors::AnsiWithC16 {
            ansi: &[45, 51, 87, 123, 159, 195, 231, 225],
            c16: &[Color::Cyan, Color::White, Color::Magenta],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Cosmos,
        name: "cosmos",
        def: ThemeColors::AnsiWithC16 {
            ansi: &[20, 27, 33, 57, 63, 93, 99, 129, 141, 189, 225],
            c16: &[Color::DarkBlue, Color::Blue, Color::Magenta, Color::White],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Nebula,
        name: "nebula",
        def: ThemeColors::AnsiWithC16 {
            ansi: &[53, 54, 90, 126, 162, 198, 201, 207, 213, 219, 225],
            c16: &[Color::Magenta, Color::Red, Color::Blue, Color::White],
        },
    },
    // ── Space themes (RGB stops, auto Color16 fallback) ──
    ThemeDef {
        scheme: ColorScheme::Stars,
        name: "stars",
        def: ThemeColors::Stops {
            stops: &[(0, 0, 0), (10, 10, 40), (80, 160, 255), (255, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Mars,
        name: "mars",
        def: ThemeColors::Stops {
            stops: &[(20, 0, 0), (120, 10, 10), (220, 60, 20), (255, 235, 220)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Venus,
        name: "venus",
        def: ThemeColors::Stops {
            stops: &[(10, 10, 0), (120, 90, 30), (255, 220, 120), (255, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Mercury,
        name: "mercury",
        def: ThemeColors::Stops {
            stops: &[(0, 0, 0), (64, 64, 64), (160, 160, 160), (255, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Jupiter,
        name: "jupiter",
        def: ThemeColors::Stops {
            stops: &[(20, 10, 0), (120, 60, 20), (200, 140, 90), (255, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Saturn,
        name: "saturn",
        def: ThemeColors::Stops {
            stops: &[(30, 15, 0), (160, 100, 20), (250, 200, 60), (255, 250, 200)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Uranus,
        name: "uranus",
        def: ThemeColors::Stops {
            stops: &[(0, 10, 10), (0, 120, 130), (120, 255, 255), (255, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Neptune,
        name: "neptune",
        def: ThemeColors::Stops {
            stops: &[(0, 0, 20), (0, 40, 140), (0, 140, 255), (240, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Pluto,
        name: "pluto",
        def: ThemeColors::Stops {
            stops: &[(5, 10, 20), (40, 60, 100), (120, 170, 230), (230, 245, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Moon,
        name: "moon",
        def: ThemeColors::Stops {
            stops: &[(0, 0, 0), (90, 100, 120), (200, 210, 220), (255, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Sun,
        name: "sun",
        def: ThemeColors::Stops {
            stops: &[(40, 0, 0), (200, 60, 0), (255, 200, 0), (255, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Comet,
        name: "comet",
        def: ThemeColors::Stops {
            stops: &[(0, 0, 40), (0, 30, 120), (80, 180, 255), (255, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Galaxy,
        name: "galaxy",
        def: ThemeColors::Stops {
            stops: &[(10, 0, 20), (60, 0, 120), (180, 60, 255), (255, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Supernova,
        name: "supernova",
        def: ThemeColors::Stops {
            stops: &[(20, 0, 40), (180, 0, 60), (255, 120, 0), (255, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::BlackHole,
        name: "blackhole",
        def: ThemeColors::Stops {
            stops: &[(0, 0, 0), (20, 0, 40), (40, 0, 80), (200, 120, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Andromeda,
        name: "andromeda",
        def: ThemeColors::Stops {
            stops: &[(0, 0, 20), (50, 0, 120), (255, 80, 200), (255, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Stardust,
        name: "stardust",
        def: ThemeColors::Stops {
            stops: &[(10, 0, 20), (120, 60, 200), (80, 200, 255), (255, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Meteor,
        name: "meteor",
        def: ThemeColors::Stops {
            stops: &[(20, 10, 0), (180, 60, 0), (255, 200, 80), (180, 220, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Eclipse,
        name: "eclipse",
        def: ThemeColors::Stops {
            stops: &[(0, 0, 0), (40, 0, 60), (255, 120, 0), (255, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::DeepSpace,
        name: "deepspace",
        def: ThemeColors::Stops {
            stops: &[(0, 0, 0), (0, 10, 40), (0, 80, 160), (200, 120, 255)],
            steps: 9,
        },
    },
    // ── Special themes ──
    ThemeDef {
        scheme: ColorScheme::Spectrum20,
        name: "spectrum20",
        def: ThemeColors::Rgb(&[
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
    },
];

/// Look up a theme by ColorScheme enum and build its color list for the given mode.
/// This replaces the massive match statement in build_palette().
#[allow(dead_code)]
pub fn build_colors(scheme: ColorScheme, mode: ColorMode) -> Vec<Color> {
    // Find the theme definition
    let def = THEMES.iter().find(|t| t.scheme == scheme);
    let Some(theme) = def else {
        // Unknown scheme — return white-only as fallback
        return vec![Color::White];
    };

    if matches!(mode, ColorMode::Mono) {
        return vec![Color::White];
    }

    match &theme.def {
        ThemeColors::Stops { stops, steps } => colors_from_stops(mode, stops, *steps),
        ThemeColors::Ansi(ansi) => {
            if matches!(mode, ColorMode::Color16) {
                auto_c16_from_ansi(ansi)
            } else {
                from_ansi_list(ansi)
            }
        }
        ThemeColors::Rgb(rgb) => {
            if matches!(mode, ColorMode::Color16) {
                auto_c16_from_rgb(rgb)
            } else {
                colors_from_rgb(mode, rgb)
            }
        }
        ThemeColors::StopsWithC16 { stops, steps, c16 } => {
            if matches!(mode, ColorMode::Color16) {
                c16.to_vec()
            } else {
                colors_from_stops(mode, stops, *steps)
            }
        }
        ThemeColors::AnsiWithC16 { ansi, c16 } => {
            if matches!(mode, ColorMode::Color16) {
                c16.to_vec()
            } else {
                from_ansi_list(ansi)
            }
        }
    }
}

/// Auto-generate Color16 fallback from ANSI 256 list by picking the
/// most distinct colors. This is a simple heuristic — themes with
/// custom C16 fallbacks use StopsWithC16/AnsiWithC16 instead.
#[allow(dead_code)]
fn auto_c16_from_ansi(ansi: &[u8]) -> Vec<Color> {
    // Pick first and last as the two most distinct colors
    if ansi.is_empty() {
        return vec![Color::White];
    }
    if ansi.len() == 1 {
        return vec![Color::White];
    }
    let first = ansi[0];
    let last = ansi[ansi.len() - 1];
    // Map ANSI 256 to approximate Color16
    let map = |v: u8| -> Color {
        if v < 8 {
            Color::DarkGrey
        }
        // dark
        else if v < 16 {
            Color::Grey
        }
        // medium
        else if v > 231 {
            Color::White
        }
        // bright
        else {
            Color::Grey
        } // mid-range
    };
    vec![map(first), map(last), Color::White]
}

/// Auto-generate Color16 fallback from RGB list.
#[allow(dead_code)]
fn auto_c16_from_rgb(rgb: &[(u8, u8, u8)]) -> Vec<Color> {
    if rgb.is_empty() {
        return vec![Color::White];
    }
    let (r, g, b) = rgb[rgb.len() / 2];
    let mid = Color::Rgb { r, g, b };
    vec![Color::DarkGrey, mid, Color::White]
}
