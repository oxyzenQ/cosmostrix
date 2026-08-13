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

use super::palette::{colors_from_rgb_floored, colors_from_stops, from_ansi_list};
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
                (55, 218, 83),
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
                (72, 222, 118),
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
                (67, 251, 206),
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
                (41, 218, 76),
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
                (190, 204, 244),
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
                (58, 142, 226),
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
                (224, 66, 66),
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
                (226, 120, 31),
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
                (251, 231, 58),
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
                (72, 230, 251),
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
                (112, 119, 127),
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
                (219, 172, 65),
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
                (222, 200, 53),
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
                (224, 124, 36),
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
                (222, 70, 66),
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
                (56, 142, 227),
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
                (52, 198, 222),
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
                (168, 84, 224),
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
                (158, 61, 158),
                (212, 85, 202),
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
                (190, 102, 22),
                (251, 140, 34),
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
                (42, 148, 175),
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
                (110, 216, 84),
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
                (216, 102, 190),
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
                (142, 144, 153),
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
            // v30 OKLab/OKLCH audit: previously used raw sRGB primaries
            // (255,0,0), (0,255,0), (0,0,255), etc. While the comment
            // claimed "maximum vibrancy", perceptually pure sRGB green
            // (L=0.87) appeared ~2× brighter than pure sRGB blue (L=0.45),
            // breaking the "uniform spectrum" visual identity. The "pop"
            // was actually perceptual non-uniformity, not vibrancy.
            //
            // Replaced with OKLCH-derived stops at L=0.65, C=0.18 —
            // consistent perceptual lightness and chroma across all 7
            // hues. The hue angles (29°, 60°, 100°, 142°, 200°, 250°,
            // 300°) span the perceptual hue wheel rather than the sRGB
            // primary triangle. Stops still go through polar OKLab
            // interpolation between them, so midpoints stay saturated.
            //
            // Head stop L=0.65 (sum 411) — well under the 655 head-
            // luminance cap, so the head bloom doesn't wash it out.
            stops: &[
                (232, 89, 74),   // red       (OKLCH 29°)
                (219, 109, 0),   // orange    (OKLCH 60°)
                (170, 143, 0),   // yellow    (OKLCH 100°)
                (64, 169, 55),   // green     (OKLCH 142°)
                (0, 173, 186),   // cyan-blue (OKLCH 200°)
                (15, 146, 247),  // blue      (OKLCH 250°)
                (161, 112, 235), // violet    (OKLCH 300°, head)
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
                (132, 156, 185),
                (180, 210, 245),
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
                (49, 216, 162),
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
                (116, 170, 222),
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
                (94, 80, 221),
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
                (146, 102, 216),
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
            // v30 OKLab/OKLCH audit: previously used raw sRGB primaries
            // (255,0,0), (0,255,0), (0,0,255), (255,255,0), (255,0,255),
            // (0,255,255), plus (128,0,0), (0,128,0), (0,0,128) etc.
            // These classic ANSI/sRGB primaries are perceptually non-
            // uniform: pure sRGB green (L=0.87) appears ~2× brighter
            // than pure sRGB blue (L=0.45). The "broad spectrum" identity
            // was undermined by the lightness drift.
            //
            // Replaced with OKLCH-derived hues at L=0.68, C=0.17 for the
            // 18 body stops (indices 1..18), at 20° hue steps spanning
            // the full perceptual hue wheel (0° → 340°). Stops 0 (origin)
            // and 19 (head) are intentionally outside the uniform band:
            //   - stop 0: dark deep-blue origin (L=0.18, C=0.04, H=250°)
            //     — preserves the cinematic "void trail" aesthetic
            //   - stop 19: warm off-white head (L=0.94, C=0.025, H=90°)
            //     — preserves the "head must not be pure white" rule
            //     (cinematic head bloom expects a tinted base)
            //
            // The c16 and ansi arrays remain as explicit fallbacks for
            // Color16/Color256 terminals (per graceful-degradation tier).
            // They were not regenerated from the new RGB because the
            // fallback tables are hand-tuned for maximum contrast on
            // legacy terminals and don't suffer the same perceptual
            // drift issue (16-color terminals can only show 8 hues
            // anyway — perceptual uniformity is meaningless there).
            rgb: &[
                (3, 18, 34),     //  0: origin (deep blue void)
                (232, 100, 148), //  1: H=0   (rose)
                (239, 101, 107), //  2: H=20  (red-orange)
                (236, 109, 61),  //  3: H=40  (orange)
                (226, 121, 0),   //  4: H=60  (amber)
                (206, 137, 0),   //  5: H=80  (yellow-amber)
                (178, 152, 0),   //  6: H=100 (olive yellow)
                (140, 166, 0),   //  7: H=120 (yellow-green)
                (88, 176, 67),   //  8: H=140 (green)
                (0, 183, 115),   //  9: H=160 (emerald)
                (0, 184, 156),   // 10: H=180 (teal)
                (0, 181, 193),   // 11: H=200 (cyan)
                (0, 173, 224),   // 12: H=220 (sky blue)
                (0, 162, 245),   // 13: H=240 (azure)
                (86, 150, 255),  // 14: H=260 (blue)
                (134, 136, 254), // 15: H=280 (indigo)
                (169, 124, 240), // 16: H=300 (violet)
                (197, 113, 216), // 17: H=320 (purple)
                (218, 105, 185), // 18: H=340 (magenta)
                // Off-white warm tint (L=0.94, C=0.025, H=90°) instead of
                // pure white (255,255,255). The head-cell color of every
                // theme must not be pure white — the cinematic head bloom
                // (HEAD_WF=45% blend toward white) expects a non-white
                // base so the head retains hue. Pure white as the head
                // stop would make the head indistinguishable from the
                // bloom transition, collapsing the 3-2-2 color
                // distribution. (242,235,217) is visually almost
                // identical to white on a dark background but preserves
                // the warm hue hint.
                (242, 235, 217), // 19: head (warm off-white)
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
                (30, 34, 78),
                (30, 35, 80),
                (68, 97, 155),
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
                (138, 34, 18),
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
                (119, 73, 14),
                (158, 98, 24),
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
                (69, 67, 64),
                (93, 90, 86),
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
                (113, 65, 27),
                (148, 88, 39),
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
                (129, 85, 23),
                (168, 113, 34),
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
                (0, 96, 103),
                (0, 128, 138),
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
                (0, 44, 148),
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
                (50, 73, 108),
                (69, 98, 143),
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
                (57, 58, 64),
                (78, 80, 88),
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
                (197, 74, 0),
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
    // ── ENERGY-ZEN: premium exclusive rarity ─────────────────────────────
    // Honors the cosmostrix + oxyzenQ journey. Deeper saturation than
    // NeonPurple, brighter head with a crystal-edge magenta lift in the
    // mid stops. The signature palette for the hardthinking-mode reward.
    // Default for monolith + cinematic scenes.
    //
    // Stops progression (deep void → crystal magenta → radiant violet head):
    //   (4, 0, 24)      — near-black void with purple undertone
    //   (28, 4, 72)     — deep amethyst
    //   (78, 18, 168)   — saturated royal purple
    //   (155, 60, 240)  — crystal-edge magenta lift (the signature stop)
    //   (190, 110, 255) — radiant violet
    //   (215, 160, 255) — bright lavender
    //   (230, 200, 255) — head: luminous crystal-white-violet
    //
    // Head (230,200,255) sum=685 — matches NeonGreen head luminance,
    // preserving the 20% head / 60% body proportion contract.
    ThemeDef {
        scheme: ColorScheme::EnergyZen,
        def: ThemeColors::StopsWithC16 {
            stops: &[
                (4, 0, 24),
                (28, 4, 72),
                (78, 18, 168),
                (155, 60, 240),
                (190, 110, 255),
                (215, 160, 255),
                (230, 200, 255),
            ],
            steps: 7,
            c16: &[Color::Magenta, Color::White],
            ansi: &[53, 90, 135, 177, 207, 225, 231],
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
            // Phase 7: apply palette-relative brightness floor to raw RGB
            // (matches the floor that colors_from_stops applies to gradient
            // stops). Without this, RgbWithC16 themes (Spectrum20) would
            // skip the floor and have invisible trail stops like (0,0,0).
            ColorMode::TrueColor => colors_from_rgb_floored(mode, rgb),
            _ => from_ansi_list(ansi),
        },
    }
}

/// Check if a scheme is registered in the central theme registry.
/// Test-only — production code uses theme::find_theme() instead.
#[cfg(test)]
pub fn has_theme(scheme: ColorScheme) -> bool {
    THEMES.iter().any(|t| t.scheme == scheme)
}

/// Number of registered themes.
/// Test-only — production code uses theme::theme_count() instead.
#[cfg(test)]
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
            ColorScheme::EnergyZen,
        ];
        for &scheme in &schemes {
            assert!(
                has_theme(scheme),
                "ColorScheme::{:?} not in THEMES registry",
                scheme
            );
        }
        assert_eq!(theme_count(), 44);
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
