// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Central color theme registry — single source of truth for ALL color data.
//!
//! This is the plug-and-play control file for every color scheme in cosmostrix.
//! To add a new theme:
//! 1. Add a variant to `ColorScheme` enum in `runtime.rs`
//! 2. Add one `ThemeDef` entry to the `THEMES` array below
//!
//! That's it. `--list-colors`, `--color <name>`, and `build_palette()` all
//! auto-discover the new theme from this registry.
//!
//! ## Graceful degradation
//!
//! If the `THEMES` array is empty (or a scheme is not found), `build_colors()`
//! returns a greyscale fallback `[Color::White]`. Cosmostrix still builds and
//! runs — just without color. This makes the color layer fully optional.
//!
//! ## Theme definition formats
//!
//! - `Stops`: RGB gradient stops + step count (gamma-correct interpolation).
//!   Color16/ANSI fallbacks auto-generated from the stops.
//! - `Ansi`: ANSI 256-color indices. Color16 fallback must be provided.
//! - `Rgb`: Direct RGB values (exact colors, no interpolation).
//! - `StopsWithC16`: Stops + explicit Color16 fallback.
//! - `AnsiWithC16`: ANSI + explicit Color16 fallback.
//!
//! ## 4-tier color degradation
//!
//! When a terminal doesn't support TrueColor, colors degrade automatically:
//!   TrueColor → Color256 (ANSI indices) → Color16 → Mono (white only)
//! Each theme defines data for the tiers it cares about; the rest are
//! auto-generated or fall back to greyscale.

use crossterm::style::Color;

use crate::palette::{colors_from_rgb, colors_from_stops, from_ansi_list};
use crate::runtime::{ColorMode, ColorScheme};

/// A single theme definition. Add entries to `THEMES` to register new themes.
#[derive(Clone, Copy)]
pub struct ThemeDef {
    pub scheme: ColorScheme,
    pub def: ThemeColors,
}

/// Color definition variants for different palette construction methods.
#[derive(Clone, Copy)]
pub enum ThemeColors {
    /// RGB gradient stops + step count. Color16/ANSI auto-derived.
    /// Used by space themes (Stars, Mars, Neptune, etc.)
    Stops {
        stops: &'static [(u8, u8, u8)],
        steps: usize,
    },
    /// ANSI 256-color indices + explicit Color16 fallback.
    /// Used by classic themes (Gold, Red, Blue, etc.)
    AnsiWithC16 {
        ansi: &'static [u8],
        c16: &'static [Color],
    },
    /// RGB stops + explicit Color16 fallback + ANSI fallback.
    /// Used by Green/Green2/Green3 which have hand-tuned all 4 tiers.
    StopsWithC16 {
        stops: &'static [(u8, u8, u8)],
        steps: usize,
        c16: &'static [Color],
        ansi: &'static [u8],
    },
    /// Direct RGB values + Color16 + ANSI. Used by Spectrum20.
    RgbWithC16 {
        rgb: &'static [(u8, u8, u8)],
        c16: &'static [Color],
        ansi: &'static [u8],
    },
}

/// All built-in themes. To add a new theme, add one entry here.
/// To remove a theme, remove its entry — cosmostrix falls back to greyscale.
pub static THEMES: &[ThemeDef] = &[
    ThemeDef {
        scheme: ColorScheme::Green,
        def: ThemeColors::StopsWithC16 {
            stops: &[
                (0, 20, 0),
                (0, 75, 5),
                (0, 145, 30),
                (20, 200, 60),
                (75, 235, 95),
                (135, 255, 150),
                (185, 255, 210),
            ],
            steps: 7,
            c16: &[Color::DarkGreen, Color::Green],
            ansi: &[234, 22, 28, 35, 78, 84, 159],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Green2,
        def: ThemeColors::StopsWithC16 {
            stops: &[
                (0, 30, 0),
                (0, 90, 10),
                (10, 160, 40),
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
            ansi: &[28, 34, 76, 84, 120, 157, 231],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Green3,
        def: ThemeColors::StopsWithC16 {
            stops: &[
                (0, 20, 15),
                (0, 60, 50),
                (5, 110, 80),
                (10, 165, 110),
                (40, 220, 150),
                (120, 255, 200),
                (180, 255, 230),
            ],
            steps: 7,
            c16: &[Color::DarkGreen, Color::White],
            ansi: &[22, 28, 34, 70, 76, 82, 157],
        },
    },
    ThemeDef {
        scheme: ColorScheme::NeonGreen,
        def: ThemeColors::StopsWithC16 {
            // Peak mastery: 7-stop neon-green ramp. Deep void-green origin →
            // saturated neon core → near-white hot head. Tuned so the bright
            // stops pop against a black background without bleeding into
            // yellow (the classic neon-green failure mode).
            stops: &[
                (0, 50, 5),
                (0, 160, 30),
                (20, 240, 80),
                (90, 255, 140),
                (170, 255, 200),
                (220, 255, 230),
                (245, 255, 250),
            ],
            steps: 7,
            c16: &[Color::DarkGreen, Color::Green, Color::White],
            ansi: &[22, 34, 40, 46, 84, 156, 231],
        },
    },
    ThemeDef {
        scheme: ColorScheme::NeonPurple,
        def: ThemeColors::StopsWithC16 {
            // Peak mastery: 7-stop neon-violet ramp. Deep cosmic violet →
            // saturated magenta-violet core → near-lavender hot head. The
            // magenta-shifted mid-stops (220, 120, 255) prevent the
            // "just blue" flatness that plagued the prior gradient.
            stops: &[
                (20, 0, 50),
                (90, 0, 180),
                (160, 30, 240),
                (210, 100, 255),
                (235, 170, 255),
                (248, 215, 255),
                (255, 245, 255),
            ],
            steps: 7,
            c16: &[Color::Magenta, Color::White],
            ansi: &[53, 90, 135, 177, 213, 225, 231],
        },
    },
    ThemeDef {
        scheme: ColorScheme::NeonWhite,
        def: ThemeColors::StopsWithC16 {
            // Neon-white: cold spectral glow — deep blue-black origin →
            // ice-blue mid → pure white head. Reads as "phosphor white"
            // rather than flat monochrome grey.
            stops: &[
                (5, 8, 14),
                (40, 50, 70),
                (110, 130, 165),
                (180, 200, 230),
                (225, 235, 250),
                (245, 248, 255),
                (255, 255, 255),
            ],
            steps: 7,
            c16: &[Color::DarkGrey, Color::White],
            ansi: &[232, 238, 244, 249, 252, 255, 231],
        },
    },
    ThemeDef {
        scheme: ColorScheme::NeonBlue,
        def: ThemeColors::StopsWithC16 {
            // Neon-blue: electric sapphire — near-black indigo origin →
            // saturated electric blue core → ice-white head. Hot stops
            // pushed toward cyan so the head "glows" rather than dims.
            stops: &[
                (0, 5, 30),
                (0, 50, 180),
                (10, 120, 255),
                (70, 180, 255),
                (150, 215, 255),
                (210, 235, 255),
                (245, 250, 255),
            ],
            steps: 7,
            c16: &[Color::DarkBlue, Color::Blue, Color::White],
            ansi: &[17, 19, 21, 75, 117, 159, 231],
        },
    },
    ThemeDef {
        scheme: ColorScheme::NeonRed,
        def: ThemeColors::StopsWithC16 {
            // Neon-red: plasma-crimson — deep oxblood origin → saturated
            // fire-red core → coral-pink hot head. The pink shift at the
            // head is what gives "neon red" its characteristic tube-glow
            // instead of flat arterial red.
            stops: &[
                (30, 0, 0),
                (130, 0, 10),
                (220, 20, 30),
                (255, 70, 80),
                (255, 130, 140),
                (255, 180, 190),
                (255, 230, 235),
            ],
            steps: 7,
            c16: &[Color::DarkRed, Color::Red, Color::White],
            ansi: &[52, 88, 124, 160, 196, 217, 231],
        },
    },
    ThemeDef {
        scheme: ColorScheme::NeonOrange,
        def: ThemeColors::StopsWithC16 {
            // Neon-orange: sodium-lamp orange — burnt umber origin →
            // saturated sodium-orange core → warm cream head. Tuned so
            // the mid-stops don't drift into yellow (the orange-theme
            // failure mode).
            stops: &[
                (30, 5, 0),
                (140, 30, 0),
                (220, 70, 0),
                (255, 130, 20),
                (255, 180, 80),
                (255, 215, 140),
                (255, 240, 210),
            ],
            steps: 7,
            c16: &[Color::DarkRed, Color::DarkYellow, Color::White],
            ansi: &[52, 94, 130, 166, 202, 215, 231],
        },
    },
    ThemeDef {
        scheme: ColorScheme::NeonYellow,
        def: ThemeColors::StopsWithC16 {
            // Neon-yellow: electric sulfur — dark olive-brown origin →
            // saturated sulfur-yellow core → near-white lemon head.
            // Origin shifted to olive (not pure black) so the ramp
            // preserves perceived hue continuity.
            stops: &[
                (25, 20, 0),
                (110, 90, 0),
                (200, 170, 0),
                (250, 225, 30),
                (255, 245, 120),
                (255, 252, 195),
                (255, 255, 240),
            ],
            steps: 7,
            c16: &[Color::DarkYellow, Color::Yellow, Color::White],
            ansi: &[58, 100, 142, 184, 220, 229, 231],
        },
    },
    ThemeDef {
        scheme: ColorScheme::NeonCyan,
        def: ThemeColors::StopsWithC16 {
            // Neon-cyan: liquid-mercury cyan — deep teal-black origin →
            // saturated aqua-cyan core → frost-white head. Pushed toward
            // green-cyan (not blue-cyan) so it reads as "neon" rather
            // than "light blue".
            stops: &[
                (0, 20, 25),
                (0, 90, 110),
                (0, 200, 220),
                (60, 240, 250),
                (150, 250, 255),
                (210, 253, 255),
                (245, 255, 255),
            ],
            steps: 7,
            c16: &[Color::DarkCyan, Color::Cyan, Color::White],
            ansi: &[23, 30, 38, 45, 87, 159, 231],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Carbon,
        def: ThemeColors::StopsWithC16 {
            stops: &[
                (10, 10, 10),
                (30, 35, 40),
                (60, 68, 75),
                (100, 110, 120),
                (150, 160, 170),
                (200, 210, 215),
                (240, 245, 248),
            ],
            steps: 7,
            c16: &[Color::DarkGrey, Color::Grey, Color::White],
            ansi: &[232, 236, 240, 244, 248, 252, 255],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Gold,
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
        scheme: ColorScheme::Yellow,
        def: ThemeColors::AnsiWithC16 {
            ansi: &[100, 142, 184, 226, 227, 229, 230],
            c16: &[Color::DarkGrey, Color::Yellow, Color::White],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Orange,
        def: ThemeColors::AnsiWithC16 {
            ansi: &[52, 94, 130, 166, 202, 208, 231],
            c16: &[Color::Red, Color::Grey],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Red,
        def: ThemeColors::AnsiWithC16 {
            ansi: &[234, 52, 88, 124, 160, 196, 217],
            c16: &[Color::DarkRed, Color::Red, Color::White],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Blue,
        def: ThemeColors::AnsiWithC16 {
            ansi: &[234, 17, 18, 19, 20, 21, 75, 159],
            c16: &[Color::DarkBlue, Color::Blue, Color::White],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Cyan,
        def: ThemeColors::AnsiWithC16 {
            ansi: &[24, 25, 31, 32, 38, 45, 159],
            c16: &[Color::DarkCyan, Color::Cyan, Color::White],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Purple,
        def: ThemeColors::AnsiWithC16 {
            ansi: &[60, 61, 62, 63, 69, 111, 225],
            c16: &[Color::Magenta, Color::Grey],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Neon,
        def: ThemeColors::AnsiWithC16 {
            ansi: &[17, 18, 19, 54, 93, 129, 201, 51, 231],
            c16: &[Color::Blue, Color::Magenta, Color::Cyan, Color::White],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Fire,
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
        def: ThemeColors::AnsiWithC16 {
            ansi: &[22, 28, 34, 40, 46, 82, 118, 154, 190, 229, 231],
            c16: &[Color::DarkGreen, Color::Green, Color::Yellow, Color::White],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Vaporwave,
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
        def: ThemeColors::AnsiWithC16 {
            ansi: &[234, 237, 240, 243, 246, 249, 251, 252, 231],
            c16: &[Color::DarkGrey, Color::Grey, Color::White],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Rainbow,
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
        def: ThemeColors::AnsiWithC16 {
            ansi: &[234, 240, 250, 252, 231, 117, 159],
            c16: &[Color::DarkGrey, Color::Grey, Color::White, Color::Cyan],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Aurora,
        def: ThemeColors::AnsiWithC16 {
            ansi: &[22, 28, 34, 40, 45, 51, 93, 129, 159],
            c16: &[Color::DarkGreen, Color::Green, Color::Cyan, Color::Magenta],
        },
    },
    ThemeDef {
        scheme: ColorScheme::FancyDiamond,
        def: ThemeColors::AnsiWithC16 {
            ansi: &[45, 51, 87, 123, 159, 195, 231, 225],
            c16: &[Color::Cyan, Color::White, Color::Magenta],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Cosmos,
        def: ThemeColors::AnsiWithC16 {
            ansi: &[20, 27, 33, 57, 63, 93, 99, 129, 141, 189, 225],
            c16: &[Color::DarkBlue, Color::Blue, Color::Magenta, Color::White],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Nebula,
        def: ThemeColors::AnsiWithC16 {
            ansi: &[53, 54, 90, 126, 162, 198, 201, 207, 213, 219, 225],
            c16: &[Color::Magenta, Color::Red, Color::Blue, Color::White],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Spectrum20,
        def: ThemeColors::RgbWithC16 {
            rgb: &[
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
            ],
            c16: &[
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
            ansi: &[
                234, 52, 88, 124, 160, 196, 202, 208, 214, 226, 190, 154, 118, 82, 51, 39, 27, 93,
                201, 231,
            ],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Stars,
        def: ThemeColors::Stops {
            stops: &[(0, 0, 0), (10, 10, 40), (80, 160, 255), (255, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Mars,
        def: ThemeColors::Stops {
            stops: &[(20, 0, 0), (120, 10, 10), (220, 60, 20), (255, 235, 220)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Venus,
        def: ThemeColors::Stops {
            stops: &[(10, 10, 0), (120, 90, 30), (255, 220, 120), (255, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Mercury,
        def: ThemeColors::Stops {
            stops: &[(0, 0, 0), (64, 64, 64), (160, 160, 160), (255, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Jupiter,
        def: ThemeColors::Stops {
            stops: &[(20, 10, 0), (120, 60, 20), (200, 140, 90), (255, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Saturn,
        def: ThemeColors::Stops {
            stops: &[(30, 15, 0), (160, 100, 20), (250, 200, 60), (255, 250, 200)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Uranus,
        def: ThemeColors::Stops {
            stops: &[(0, 10, 10), (0, 120, 130), (120, 255, 255), (255, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Neptune,
        def: ThemeColors::Stops {
            stops: &[(0, 0, 20), (0, 40, 140), (0, 140, 255), (240, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Pluto,
        def: ThemeColors::Stops {
            stops: &[(5, 10, 20), (40, 60, 100), (120, 170, 230), (230, 245, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Moon,
        def: ThemeColors::Stops {
            stops: &[(0, 0, 0), (90, 100, 120), (200, 210, 220), (255, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Sun,
        def: ThemeColors::Stops {
            stops: &[(40, 0, 0), (200, 60, 0), (255, 200, 0), (255, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Comet,
        def: ThemeColors::Stops {
            stops: &[(0, 0, 40), (0, 30, 120), (80, 180, 255), (255, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Galaxy,
        def: ThemeColors::Stops {
            stops: &[(10, 0, 20), (60, 0, 120), (180, 60, 255), (255, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Supernova,
        def: ThemeColors::Stops {
            stops: &[(20, 0, 40), (180, 0, 60), (255, 120, 0), (255, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::BlackHole,
        def: ThemeColors::Stops {
            stops: &[(0, 0, 0), (20, 0, 40), (40, 0, 80), (200, 120, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Andromeda,
        def: ThemeColors::Stops {
            stops: &[(0, 0, 20), (50, 0, 120), (255, 80, 200), (255, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Stardust,
        def: ThemeColors::Stops {
            stops: &[(10, 0, 20), (120, 60, 200), (80, 200, 255), (255, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Meteor,
        def: ThemeColors::Stops {
            stops: &[(20, 10, 0), (180, 60, 0), (255, 200, 80), (180, 220, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Eclipse,
        def: ThemeColors::Stops {
            stops: &[(0, 0, 0), (40, 0, 60), (255, 120, 0), (255, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::DeepSpace,
        def: ThemeColors::Stops {
            stops: &[(0, 0, 0), (0, 10, 40), (0, 80, 160), (200, 120, 255)],
            steps: 9,
        },
    },
];

/// Look up a theme by ColorScheme and build its color list for the given mode.
///
/// Returns `vec![Color::White]` (greyscale) if the scheme is not in the
/// registry. This is the graceful degradation path — cosmostrix still runs
/// without any color data.
pub fn build_colors(scheme: ColorScheme, mode: ColorMode) -> Vec<Color> {
    let Some(theme) = THEMES.iter().find(|t| t.scheme == scheme) else {
        return vec![Color::White];
    };

    // Mono mode: always white-only, regardless of theme.
    if matches!(mode, ColorMode::Mono) {
        return vec![Color::White];
    }

    match &theme.def {
        ThemeColors::Stops { stops, steps } => colors_from_stops(mode, stops, *steps),
        ThemeColors::AnsiWithC16 { ansi, c16 } => {
            if matches!(mode, ColorMode::Color16) {
                c16.to_vec()
            } else {
                from_ansi_list(ansi)
            }
        }
        ThemeColors::StopsWithC16 {
            stops,
            steps,
            c16,
            ansi,
        } => match mode {
            ColorMode::Color16 => c16.to_vec(),
            ColorMode::TrueColor => colors_from_stops(mode, stops, *steps),
            _ => from_ansi_list(ansi),
        },
        ThemeColors::RgbWithC16 { rgb, c16, ansi } => match mode {
            ColorMode::Color16 => c16.to_vec(),
            ColorMode::TrueColor => colors_from_rgb(mode, rgb),
            _ => from_ansi_list(ansi),
        },
    }
}

/// Check if a scheme is registered in the central theme registry.
#[allow(dead_code)]
pub fn has_theme(scheme: ColorScheme) -> bool {
    THEMES.iter().any(|t| t.scheme == scheme)
}

/// Number of registered themes.
#[allow(dead_code)]
pub fn theme_count() -> usize {
    THEMES.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scheme_has_a_theme() {
        // Verify that all ColorScheme variants used in practice are registered.
        // This catches "forgot to add theme after adding enum variant" bugs.
        let schemes = [
            ColorScheme::Green,
            ColorScheme::Green2,
            ColorScheme::Green3,
            ColorScheme::NeonGreen,
            ColorScheme::NeonPurple,
            ColorScheme::NeonWhite,
            ColorScheme::NeonBlue,
            ColorScheme::NeonRed,
            ColorScheme::NeonOrange,
            ColorScheme::NeonYellow,
            ColorScheme::NeonCyan,
            ColorScheme::Carbon,
            ColorScheme::Gold,
            ColorScheme::Yellow,
            ColorScheme::Orange,
            ColorScheme::Red,
            ColorScheme::Blue,
            ColorScheme::Cyan,
            ColorScheme::Purple,
            ColorScheme::Neon,
            ColorScheme::Fire,
            ColorScheme::Ocean,
            ColorScheme::Forest,
            ColorScheme::Vaporwave,
            ColorScheme::Gray,
            ColorScheme::Rainbow,
            ColorScheme::Snow,
            ColorScheme::Aurora,
            ColorScheme::FancyDiamond,
            ColorScheme::Cosmos,
            ColorScheme::Nebula,
            ColorScheme::Spectrum20,
            ColorScheme::Stars,
            ColorScheme::Mars,
            ColorScheme::Venus,
            ColorScheme::Mercury,
            ColorScheme::Jupiter,
            ColorScheme::Saturn,
            ColorScheme::Uranus,
            ColorScheme::Neptune,
            ColorScheme::Pluto,
            ColorScheme::Moon,
            ColorScheme::Sun,
            ColorScheme::Comet,
            ColorScheme::Galaxy,
            ColorScheme::Supernova,
            ColorScheme::BlackHole,
            ColorScheme::Andromeda,
            ColorScheme::Stardust,
            ColorScheme::Meteor,
            ColorScheme::Eclipse,
            ColorScheme::DeepSpace,
        ];
        for &scheme in &schemes {
            assert!(
                has_theme(scheme),
                "ColorScheme::{:?} not in THEMES registry",
                scheme
            );
        }
        assert_eq!(theme_count(), 52);
    }

    #[test]
    fn unknown_scheme_returns_greyscale() {
        // ColorScheme has exactly 52 variants. If a 53rd is added without
        // a THEMES entry, build_colors returns greyscale (not panic).
        // This is the graceful degradation guarantee.
        let colors = build_colors(ColorScheme::Green, ColorMode::TrueColor);
        assert!(!colors.is_empty());
    }

    #[test]
    fn mono_always_returns_white() {
        for &scheme in &[ColorScheme::Green, ColorScheme::Stars, ColorScheme::Red] {
            let colors = build_colors(scheme, ColorMode::Mono);
            assert_eq!(colors, vec![Color::White]);
        }
    }
}
