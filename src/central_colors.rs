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
                (0, 12, 1),
                (0, 45, 6),
                (30, 182, 56),
                (80, 255, 110),
                (125, 255, 150),
                (170, 255, 190),
                (201, 244, 210),
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
                (0, 20, 5),
                (5, 70, 18),
                (43, 190, 86),
                (100, 255, 150),
                (140, 255, 175),
                (180, 255, 200),
                (206, 238, 211),
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
                (0, 18, 12),
                (0, 60, 45),
                (24, 187, 141),
                (70, 255, 210),
                (115, 255, 218),
                (160, 255, 225),
                (190, 242, 223),
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
                (0, 12, 1),
                (0, 45, 6),
                (22, 182, 53),
                (60, 255, 100),
                (105, 255, 138),
                (150, 255, 175),
                (195, 255, 205),
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
                (8, 0, 20),
                (35, 5, 70),
                (92, 22, 162),
                (180, 90, 255),
                (198, 120, 255),
                (215, 150, 255),
                // v25 calibration: head stop toned down from (235, 195, 255)
                // to (220, 180, 255). The previous value's high luminance
                // (sum 685) made the head dominate the body in the cinematic
                // scene (which uses neon-purple), creating the "head too
                // long/white" symptom. (220, 180, 255) (sum 655) matches
                // NeonGreen's head luminance, preserving the 20% head / 60%
                // body proportion contract.
                (220, 180, 255),
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
                (5, 6, 8),
                (28, 32, 40),
                (121, 132, 163),
                (190, 205, 245),
                (205, 220, 250),
                (220, 235, 255),
                (212, 219, 224),
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
                (0, 5, 20),
                (0, 30, 90),
                (32, 108, 197),
                (85, 175, 255),
                (125, 192, 255),
                (165, 210, 255),
                (196, 220, 239),
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
                (20, 0, 0),
                (80, 5, 5),
                (194, 42, 42),
                (255, 90, 90),
                (255, 115, 118),
                (255, 140, 145),
                (255, 190, 195),
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
                (20, 5, 0),
                (85, 20, 0),
                (196, 90, 17),
                (255, 150, 45),
                (255, 165, 68),
                (255, 180, 90),
                (255, 210, 140),
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
                (20, 15, 0),
                (80, 65, 0),
                (194, 168, 22),
                (255, 235, 60),
                (255, 240, 95),
                (255, 245, 130),
                (238, 235, 182),
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
                (0, 15, 18),
                (0, 55, 70),
                (28, 163, 187),
                (75, 235, 255),
                (112, 240, 255),
                (150, 245, 255),
                (182, 234, 239),
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
                (8, 9, 10),
                (28, 30, 33),
                (85, 90, 96),
                (140, 148, 158),
                (165, 174, 184),
                (190, 200, 210),
                (209, 218, 228),
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
                (15, 8, 0),
                (50, 28, 0),
                (183, 134, 40),
                (255, 210, 90),
                (255, 220, 120),
                (255, 230, 150),
                (239, 229, 187),
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
                (18, 15, 0),
                (65, 55, 0),
                (188, 165, 31),
                (255, 235, 75),
                (255, 240, 108),
                (255, 245, 140),
                (237, 233, 185),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Orange,
        def: ThemeColors::Stops {
            // Amber-orange: burnt umber origin → tangerine → warm peach head.
            stops: &[
                (20, 5, 0),
                (75, 20, 0),
                (192, 92, 21),
                (255, 155, 50),
                (255, 172, 75),
                (255, 190, 100),
                (255, 220, 155),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Red,
        def: ThemeColors::Stops {
            // High-alert red: oxblood origin → arterial red → coral-blush head.
            stops: &[
                (18, 0, 0),
                (70, 5, 5),
                (190, 46, 42),
                (255, 95, 90),
                (255, 120, 115),
                (255, 145, 140),
                (255, 195, 190),
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
                (0, 5, 22),
                (0, 28, 95),
                (31, 108, 199),
                (80, 175, 255),
                (118, 192, 255),
                (155, 210, 255),
                (190, 223, 242),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Cyan,
        def: ThemeColors::Stops {
            // Cool cyan: deep teal-black → aqua → frosty pale-cyan head.
            stops: &[
                (0, 12, 18),
                (0, 50, 75),
                (28, 162, 188),
                (75, 235, 255),
                (112, 240, 255),
                (150, 245, 255),
                (182, 234, 239),
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
                (12, 0, 22),
                (45, 8, 75),
                (136, 57, 192),
                (200, 110, 255),
                (212, 138, 255),
                (225, 165, 255),
                (226, 194, 235),
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
                (8, 0, 28),
                (40, 5, 85),
                (168, 60, 175),
                (255, 110, 230),
                (218, 165, 242),
                (180, 220, 255),
                (202, 220, 233),
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
                (18, 0, 0),
                (75, 5, 0),
                (192, 73, 14),
                (255, 145, 35),
                (255, 172, 62),
                (255, 200, 90),
                (253, 233, 169),
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
                (0, 5, 18),
                (0, 28, 65),
                (24, 111, 140),
                (60, 185, 210),
                (98, 205, 228),
                (135, 225, 245),
                (180, 233, 242),
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
                (8, 10, 0),
                (30, 50, 8),
                (80, 176, 57),
                (140, 255, 110),
                (168, 255, 140),
                (195, 255, 170),
                (214, 242, 199),
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
                (12, 0, 35),
                (55, 10, 95),
                (176, 73, 166),
                (255, 130, 215),
                (218, 172, 232),
                (180, 215, 250),
                (202, 220, 233),
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
                (8, 8, 10),
                (35, 36, 40),
                (108, 111, 118),
                (175, 178, 188),
                (195, 199, 210),
                (215, 220, 232),
                (213, 218, 224),
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
                (40, 0, 0),
                (140, 10, 0),
                (154, 146, 24),
                (50, 240, 80),
                (25, 220, 168),
                (0, 200, 255),
                (170, 100, 255),
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
                (3, 8, 18),
                (20, 38, 65),
                (110, 139, 174),
                (185, 215, 250),
                (205, 228, 252),
                (225, 240, 255),
                (214, 218, 223),
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
                (0, 12, 8),
                (0, 55, 35),
                (28, 178, 124),
                (70, 255, 200),
                (105, 238, 225),
                (140, 220, 250),
                (188, 222, 245),
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
                (3, 12, 22),
                (15, 50, 90),
                (78, 146, 190),
                (155, 195, 255),
                (182, 208, 255),
                (210, 220, 255),
                (209, 218, 228),
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
                (3, 3, 18),
                (15, 18, 60),
                (68, 59, 187),
                (120, 100, 255),
                (150, 125, 255),
                (180, 150, 255),
                (213, 194, 248),
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
                (10, 0, 22),
                (50, 15, 70),
                (132, 73, 178),
                (160, 130, 255),
                (180, 155, 255),
                (200, 180, 255),
                (217, 203, 235),
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
                // Off-white (255,255,230) instead of pure white (255,255,255).
                // The head-cell color of every theme must not be pure white —
                // the cinematic head bloom (HEAD_WF=45% blend toward white)
                // expects a non-white base so the head retains hue. Pure
                // white as the head stop would make the head indistinguishable
                // from the bloom transition, collapsing the 3-2-2 color
                // distribution. (255,255,230) is visually almost identical
                // to white on a dark background but preserves the hue hint.
                (255, 255, 230),
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
            stops: &[
                (0, 0, 0),
                (2, 2, 12),
                (20, 23, 56),
                (30, 35, 80),
                (90, 130, 200),
                (170, 200, 250),
                (205, 224, 252),
                (240, 248, 255),
                (217, 219, 219),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Mars,
        def: ThemeColors::Stops {
            stops: &[
                (15, 0, 0),
                (40, 8, 5),
                (105, 26, 14),
                (140, 35, 18),
                (220, 75, 30),
                (255, 130, 60),
                (255, 172, 115),
                (255, 215, 170),
                (241, 221, 193),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Venus,
        def: ThemeColors::Stops {
            stops: &[
                (15, 8, 0),
                (45, 25, 0),
                (119, 73, 16),
                (160, 100, 25),
                (230, 165, 50),
                (255, 210, 90),
                (255, 228, 142),
                (255, 245, 195),
                (229, 225, 201),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Mercury,
        def: ThemeColors::Stops {
            stops: &[
                (5, 5, 5),
                (25, 24, 23),
                (70, 68, 66),
                (95, 92, 88),
                (150, 145, 140),
                (195, 190, 185),
                (220, 215, 210),
                (245, 240, 235),
                (220, 218, 217),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Jupiter,
        def: ThemeColors::Stops {
            stops: &[
                (15, 8, 0),
                (50, 22, 5),
                (115, 66, 27),
                (150, 90, 40),
                (220, 150, 80),
                (255, 195, 120),
                (255, 215, 160),
                (255, 235, 200),
                (231, 221, 203),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Saturn,
        def: ThemeColors::Stops {
            stops: &[
                (20, 12, 0),
                (60, 35, 5),
                (132, 87, 24),
                (170, 115, 35),
                (240, 175, 60),
                (255, 210, 95),
                (255, 228, 145),
                (255, 245, 195),
                (229, 225, 201),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Uranus,
        def: ThemeColors::Stops {
            stops: &[
                (0, 12, 12),
                (0, 35, 38),
                (0, 96, 104),
                (0, 130, 140),
                (40, 200, 215),
                (110, 235, 245),
                (160, 244, 250),
                (210, 252, 255),
                (207, 224, 224),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Neptune,
        def: ThemeColors::Stops {
            stops: &[
                (0, 0, 18),
                (0, 8, 50),
                (0, 32, 115),
                (0, 45, 150),
                (0, 100, 215),
                (20, 150, 255),
                (90, 188, 255),
                (160, 225, 255),
                (195, 223, 237),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Pluto,
        def: ThemeColors::Stops {
            stops: &[
                (5, 10, 18),
                (15, 25, 45),
                (50, 73, 110),
                (70, 100, 145),
                (130, 165, 210),
                (180, 210, 245),
                (208, 228, 250),
                (235, 245, 255),
                (215, 219, 221),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Moon,
        def: ThemeColors::Stops {
            stops: &[
                (0, 0, 0),
                (15, 15, 18),
                (58, 58, 65),
                (80, 82, 90),
                (140, 145, 155),
                (195, 200, 210),
                (220, 224, 231),
                (245, 248, 252),
                (217, 218, 220),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Sun,
        def: ThemeColors::Stops {
            stops: &[
                (30, 5, 0),
                (75, 18, 0),
                (157, 55, 0),
                (200, 75, 0),
                (255, 140, 15),
                (255, 190, 50),
                (255, 215, 112),
                (255, 240, 175),
                (232, 227, 196),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Comet,
        def: ThemeColors::Stops {
            stops: &[
                (0, 0, 30),
                (0, 10, 60),
                (0, 42, 135),
                (0, 60, 175),
                (40, 130, 230),
                (110, 190, 255),
                (165, 215, 255),
                (220, 240, 255),
                (211, 220, 224),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Galaxy,
        def: ThemeColors::Stops {
            stops: &[
                (8, 0, 18),
                (25, 0, 55),
                (73, 16, 124),
                (100, 25, 160),
                (180, 60, 230),
                (220, 110, 255),
                (235, 160, 255),
                (250, 210, 255),
                (223, 207, 225),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Supernova,
        def: ThemeColors::Stops {
            stops: &[
                (18, 0, 30),
                (60, 0, 35),
                (151, 26, 18),
                (200, 40, 10),
                (255, 95, 15),
                (255, 155, 35),
                (255, 192, 90),
                (255, 230, 145),
                (239, 229, 187),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::BlackHole,
        def: ThemeColors::Stops {
            stops: &[
                (0, 0, 0),
                (5, 0, 18),
                (24, 3, 55),
                (35, 5, 75),
                (110, 40, 180),
                (170, 80, 235),
                (205, 130, 245),
                (240, 180, 255),
                (226, 199, 230),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Andromeda,
        def: ThemeColors::Stops {
            stops: &[
                (0, 0, 18),
                (15, 0, 55),
                (71, 16, 133),
                (100, 25, 175),
                (190, 60, 240),
                (230, 110, 255),
                (241, 155, 255),
                (252, 200, 255),
                (225, 204, 226),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Stardust,
        def: ThemeColors::Stops {
            stops: &[
                (8, 0, 18),
                (20, 5, 50),
                (78, 35, 131),
                (110, 50, 175),
                (95, 145, 235),
                (140, 200, 255),
                (180, 222, 255),
                (220, 245, 255),
                (210, 221, 224),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Meteor,
        def: ThemeColors::Stops {
            stops: &[
                (15, 5, 0),
                (50, 18, 0),
                (134, 52, 6),
                (180, 70, 10),
                (245, 130, 30),
                (255, 180, 60),
                (228, 200, 148),
                (200, 220, 235),
                (206, 220, 229),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Eclipse,
        def: ThemeColors::Stops {
            stops: &[
                (0, 0, 0),
                (15, 0, 30),
                (76, 13, 20),
                (110, 20, 15),
                (220, 60, 0),
                (255, 130, 25),
                (255, 175, 88),
                (255, 220, 150),
                (237, 223, 195),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::DeepSpace,
        def: ThemeColors::Stops {
            stops: &[
                (0, 0, 0),
                (0, 5, 25),
                (0, 27, 81),
                (0, 40, 110),
                (0, 90, 190),
                (60, 120, 235),
                (120, 160, 245),
                (180, 200, 255),
                (206, 215, 234),
            ],
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
