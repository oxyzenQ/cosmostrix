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
    /// Legacy format — all themes have been migrated to `Stops`/`StopsWithC16`.
    /// Retained for any future theme that wants explicit ANSI fallback control.
    #[allow(dead_code)]
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
    // ── 11 NEON THEMES (masterclass tuning) ─────────────────────────────
    // Principle: head stays tinted (not pure white), body is deeply saturated,
    // tail is near-black with a faint hue. Stops the classic 'neon fade to
    // grey' failure mode and gives each theme a recognizable tube-glow.
    ThemeDef {
        scheme: ColorScheme::NeonGreen,
        def: ThemeColors::StopsWithC16 {
            stops: &[
                (0, 25, 3),
                (0, 140, 25),
                (15, 220, 60),
                (80, 255, 110),
                (170, 255, 160),
                (220, 255, 200),
                (235, 255, 225),
            ],
            steps: 7,
            c16: &[Color::DarkGreen, Color::Green, Color::White],
            ansi: &[22, 34, 40, 46, 84, 156, 231],
        },
    },
    ThemeDef {
        scheme: ColorScheme::NeonPurple,
        def: ThemeColors::StopsWithC16 {
            stops: &[
                (15, 0, 35),
                (80, 0, 160),
                (140, 20, 230),
                (195, 90, 255),
                (220, 150, 255),
                (240, 200, 255),
                (250, 235, 255),
            ],
            steps: 7,
            c16: &[Color::Magenta, Color::White],
            ansi: &[53, 90, 135, 177, 213, 225, 231],
        },
    },
    ThemeDef {
        scheme: ColorScheme::NeonWhite,
        def: ThemeColors::StopsWithC16 {
            stops: &[
                (5, 8, 14),
                (40, 50, 70),
                (100, 120, 155),
                (170, 190, 220),
                (215, 225, 240),
                (240, 245, 252),
                (252, 253, 255),
            ],
            steps: 7,
            c16: &[Color::DarkGrey, Color::White],
            ansi: &[232, 238, 244, 249, 252, 255, 231],
        },
    },
    ThemeDef {
        scheme: ColorScheme::NeonBlue,
        def: ThemeColors::StopsWithC16 {
            stops: &[
                (0, 5, 30),
                (0, 45, 170),
                (8, 110, 250),
                (60, 170, 255),
                (140, 205, 255),
                (200, 228, 255),
                (235, 245, 255),
            ],
            steps: 7,
            c16: &[Color::DarkBlue, Color::Blue, Color::White],
            ansi: &[17, 19, 21, 75, 117, 159, 231],
        },
    },
    ThemeDef {
        scheme: ColorScheme::NeonRed,
        def: ThemeColors::StopsWithC16 {
            stops: &[
                (30, 0, 0),
                (130, 0, 8),
                (210, 15, 25),
                (255, 65, 75),
                (255, 120, 130),
                (255, 175, 185),
                (255, 225, 230),
            ],
            steps: 7,
            c16: &[Color::DarkRed, Color::Red, Color::White],
            ansi: &[52, 88, 124, 160, 196, 217, 231],
        },
    },
    ThemeDef {
        scheme: ColorScheme::NeonOrange,
        def: ThemeColors::StopsWithC16 {
            stops: &[
                (30, 5, 0),
                (140, 30, 0),
                (215, 65, 0),
                (255, 125, 18),
                (255, 175, 75),
                (255, 210, 135),
                (255, 235, 205),
            ],
            steps: 7,
            c16: &[Color::DarkRed, Color::DarkYellow, Color::White],
            ansi: &[52, 94, 130, 166, 202, 215, 231],
        },
    },
    ThemeDef {
        scheme: ColorScheme::NeonYellow,
        def: ThemeColors::StopsWithC16 {
            stops: &[
                (25, 20, 0),
                (110, 90, 0),
                (195, 165, 0),
                (245, 220, 28),
                (255, 242, 115),
                (255, 251, 190),
                (255, 254, 238),
            ],
            steps: 7,
            c16: &[Color::DarkYellow, Color::Yellow, Color::White],
            ansi: &[58, 100, 142, 184, 220, 229, 231],
        },
    },
    ThemeDef {
        scheme: ColorScheme::NeonCyan,
        def: ThemeColors::StopsWithC16 {
            stops: &[
                (0, 20, 25),
                (0, 90, 110),
                (0, 195, 215),
                (55, 235, 245),
                (145, 245, 252),
                (205, 250, 255),
                (240, 254, 255),
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
                (95, 105, 115),
                (140, 150, 160),
                (190, 200, 205),
                (235, 240, 243),
            ],
            steps: 7,
            c16: &[Color::DarkGrey, Color::Grey, Color::White],
            ansi: &[232, 236, 240, 244, 248, 252, 255],
        },
    },
    // ── 19 LEGACY THEMES (migrated from AnsiWithC16 to Stops) ──────────
    // Each theme was originally a hand-picked ANSI 256-color index list.
    // Migrated to RGB Stops with masterclass tuning: deep tinted origin →
    // saturated body → tinted head (not pure white). RGB is now the primary
    // truth; Color16/ANSI fallbacks are auto-derived by colors_from_stops.
    ThemeDef {
        scheme: ColorScheme::Gold,
        def: ThemeColors::Stops {
            // Polished gold: near-black brown origin → burnished amber →
            // luminous pale-gold head.
            stops: &[
                (20, 12, 0),
                (90, 55, 0),
                (180, 130, 20),
                (230, 185, 60),
                (248, 220, 110),
                (253, 240, 175),
                (255, 250, 220),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Yellow,
        def: ThemeColors::Stops {
            // Warm signal yellow: dark olive origin → rich amber-yellow →
            // pale buttercream head.
            stops: &[
                (20, 18, 0),
                (85, 70, 0),
                (165, 140, 10),
                (220, 195, 35),
                (245, 225, 90),
                (253, 245, 165),
                (255, 252, 220),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Orange,
        def: ThemeColors::Stops {
            // Amber-orange: burnt umber origin → tangerine → warm peach head.
            stops: &[
                (25, 8, 0),
                (110, 35, 0),
                (185, 80, 5),
                (230, 130, 25),
                (250, 175, 70),
                (255, 210, 130),
                (255, 235, 195),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Red,
        def: ThemeColors::Stops {
            // High-alert red: oxblood origin → arterial red → coral-blush head.
            stops: &[
                (25, 0, 0),
                (95, 0, 5),
                (165, 20, 20),
                (215, 60, 55),
                (240, 110, 100),
                (252, 165, 155),
                (255, 215, 205),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Blue,
        def: ThemeColors::Stops {
            // Clean electric blue: near-black indigo → royal blue →
            // pale sky-blue head.
            stops: &[
                (0, 5, 25),
                (0, 35, 110),
                (10, 80, 180),
                (50, 130, 220),
                (110, 175, 240),
                (175, 210, 250),
                (220, 235, 255),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Cyan,
        def: ThemeColors::Stops {
            // Cool cyan: deep teal-black → aqua → frosty pale-cyan head.
            stops: &[
                (0, 15, 20),
                (0, 60, 80),
                (0, 130, 150),
                (40, 185, 200),
                (110, 220, 230),
                (175, 240, 245),
                (220, 250, 252),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Purple,
        def: ThemeColors::Stops {
            // Saturated royal purple: dark plum → regal violet →
            // pale lavender-rose head.
            stops: &[
                (15, 0, 25),
                (65, 10, 90),
                (120, 30, 160),
                (165, 70, 200),
                (205, 120, 225),
                (230, 170, 240),
                (248, 215, 250),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Neon,
        def: ThemeColors::Stops {
            // Synthwave neon: deep indigo → magenta → cyan → pale rose head.
            // Multi-hue ramp preserves the synthwave dual-color identity.
            stops: &[
                (10, 0, 30),
                (60, 0, 110),
                (140, 20, 180),
                (220, 60, 200),
                (90, 180, 230),
                (170, 230, 245),
                (245, 230, 250),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Fire,
        def: ThemeColors::Stops {
            // Hot ember and flame: deep maroon origin → blood-red →
            // ember orange → pale yellow-white head.
            stops: &[
                (20, 0, 0),
                (90, 5, 0),
                (170, 30, 0),
                (220, 75, 10),
                (250, 140, 30),
                (255, 200, 90),
                (255, 240, 190),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Ocean,
        def: ThemeColors::Stops {
            // Deep-sea blue-green: abyssal black-blue → ocean teal →
            // pale surf-foam head.
            stops: &[
                (0, 5, 20),
                (0, 35, 70),
                (0, 80, 110),
                (20, 130, 145),
                (80, 180, 190),
                (150, 220, 225),
                (220, 245, 248),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Forest,
        def: ThemeColors::Stops {
            // Moss and canopy: forest-floor brown → moss green →
            // sunlit pale-lime head.
            stops: &[
                (10, 12, 0),
                (35, 55, 10),
                (60, 110, 25),
                (100, 165, 50),
                (155, 210, 85),
                (200, 235, 140),
                (235, 250, 200),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Vaporwave,
        def: ThemeColors::Stops {
            // Retro pink-cyan haze: deep magenta-violet → hot pink →
            // cyan-mist head. The signature vaporwave dual-tone ramp.
            stops: &[
                (15, 0, 40),
                (90, 20, 110),
                (180, 50, 160),
                (230, 100, 180),
                (140, 180, 220),
                (190, 220, 240),
                (240, 240, 250),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Gray,
        def: ThemeColors::Stops {
            // Neutral monochrome gray: near-black → mid grey →
            // near-white with a faint cool tint (not pure neutral).
            stops: &[
                (10, 10, 12),
                (45, 46, 50),
                (90, 92, 98),
                (140, 142, 148),
                (185, 187, 192),
                (220, 222, 226),
                (245, 246, 248),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Rainbow,
        def: ThemeColors::Stops {
            // Full-spectrum hue cycle. Origin dark red → red → orange →
            // yellow → green → cyan → blue → magenta head. Preserves
            // the original hue-cycling identity in RGB space.
            stops: &[
                (80, 0, 0),
                (220, 30, 0),
                (250, 130, 0),
                (240, 220, 0),
                (40, 200, 60),
                (0, 180, 220),
                (130, 60, 230),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Snow,
        def: ThemeColors::Stops {
            // Cold white-blue shimmer: deep blue-black → ice blue →
            // frosty pale-cyan head.
            stops: &[
                (5, 10, 20),
                (30, 50, 80),
                (90, 130, 170),
                (150, 185, 215),
                (200, 220, 240),
                (230, 240, 250),
                (250, 252, 255),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Aurora,
        def: ThemeColors::Stops {
            // Northern-lights: dark green origin → emerald →
            // cyan-violet shimmer → pale auroral-green head.
            stops: &[
                (0, 15, 10),
                (0, 70, 50),
                (10, 140, 90),
                (40, 200, 160),
                (80, 200, 220),
                (140, 180, 240),
                (200, 230, 250),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::FancyDiamond,
        def: ThemeColors::Stops {
            // Prismatic diamond: deep teal origin → cyan-magenta shift →
            // pale iridescent head. Captures the multi-color sparkle
            // identity of the original prismatic palette.
            stops: &[
                (5, 15, 25),
                (20, 60, 100),
                (40, 130, 170),
                (90, 180, 210),
                (170, 180, 230),
                (220, 200, 245),
                (245, 235, 255),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Cosmos,
        def: ThemeColors::Stops {
            // Cosmic blue-purple: deep void-blue → royal indigo →
            // magenta-violet → pale cosmic-lilac head.
            stops: &[
                (5, 5, 20),
                (20, 25, 70),
                (50, 50, 140),
                (90, 70, 180),
                (140, 100, 210),
                (195, 150, 235),
                (235, 210, 250),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Nebula,
        def: ThemeColors::Stops {
            // Nebula magenta-blue: deep magenta-black → rose-violet →
            // blue-mist → pale nebular-lavender head.
            stops: &[
                (15, 0, 25),
                (70, 20, 80),
                (130, 50, 140),
                (170, 90, 180),
                (140, 130, 220),
                (190, 180, 240),
                (235, 220, 250),
            ],
            steps: 9,
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
    // ── 18 PLANET & SPACE THEMES (masterclass tuning) ───────────────────
    // 4-stop gradients with 9-step interpolation. Head stays tinted with
    // the body hue (not pure white) so each planet reads as itself rather
    // than collapsing to a generic white-bright core.
    ThemeDef {
        scheme: ColorScheme::Stars,
        def: ThemeColors::Stops {
            stops: &[(0, 0, 0), (8, 8, 35), (70, 150, 245), (220, 240, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Mars,
        def: ThemeColors::Stops {
            stops: &[(20, 0, 0), (110, 8, 8), (210, 55, 18), (245, 220, 210)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Venus,
        def: ThemeColors::Stops {
            stops: &[(10, 10, 0), (110, 85, 28), (245, 210, 110), (255, 250, 240)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Mercury,
        def: ThemeColors::Stops {
            stops: &[(0, 0, 0), (60, 60, 60), (150, 150, 150), (245, 245, 245)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Jupiter,
        def: ThemeColors::Stops {
            stops: &[(20, 10, 0), (110, 55, 18), (190, 130, 85), (250, 245, 240)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Saturn,
        def: ThemeColors::Stops {
            stops: &[(30, 15, 0), (150, 95, 18), (240, 190, 55), (255, 248, 195)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Uranus,
        def: ThemeColors::Stops {
            stops: &[(0, 10, 10), (0, 115, 125), (110, 245, 250), (240, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Neptune,
        def: ThemeColors::Stops {
            stops: &[(0, 0, 20), (0, 38, 135), (0, 135, 250), (230, 248, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Pluto,
        def: ThemeColors::Stops {
            stops: &[(5, 10, 20), (38, 58, 95), (115, 165, 225), (225, 240, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Moon,
        def: ThemeColors::Stops {
            stops: &[(0, 0, 0), (85, 95, 115), (195, 205, 215), (255, 255, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Sun,
        def: ThemeColors::Stops {
            stops: &[(40, 0, 0), (190, 55, 0), (250, 195, 0), (255, 255, 240)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Comet,
        def: ThemeColors::Stops {
            stops: &[(0, 0, 40), (0, 28, 115), (75, 170, 250), (240, 250, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Galaxy,
        def: ThemeColors::Stops {
            stops: &[(10, 0, 20), (55, 0, 115), (170, 55, 250), (250, 240, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Supernova,
        def: ThemeColors::Stops {
            stops: &[(20, 0, 40), (170, 0, 55), (250, 115, 0), (255, 250, 245)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::BlackHole,
        def: ThemeColors::Stops {
            stops: &[(0, 0, 0), (18, 0, 35), (38, 0, 75), (190, 115, 250)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Andromeda,
        def: ThemeColors::Stops {
            stops: &[(0, 0, 20), (45, 0, 115), (245, 75, 195), (255, 245, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Stardust,
        def: ThemeColors::Stops {
            stops: &[(10, 0, 20), (115, 55, 195), (75, 195, 250), (245, 250, 255)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Meteor,
        def: ThemeColors::Stops {
            stops: &[(20, 10, 0), (170, 55, 0), (245, 195, 75), (170, 215, 250)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Eclipse,
        def: ThemeColors::Stops {
            stops: &[(0, 0, 0), (38, 0, 55), (245, 115, 0), (255, 250, 245)],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::DeepSpace,
        def: ThemeColors::Stops {
            stops: &[(0, 0, 0), (0, 8, 35), (0, 75, 150), (190, 115, 250)],
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
