// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only
// LOC_EXEMPT: pure data file — 44-theme registry (ThemeDef entries, no logic). Exempt per src/RULES_LOC.md 'When NOT to Split' (generated-like data).

//! Theme registry — the full `THEMES` static array of all 44 built-in
//! color schemes. Extracted from `catalog.rs` to keep that file under
//! the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Each `ThemeDef` defines a color scheme's full color pipeline:
//! - `stops`: RGB gradient stops (head → tail brightness curve)
//! - `steps`: interpolation step count between stops
//! - `c16`: 16-color ANSI fallbacks
//! - `ansi`: 256-color ANSI fallbacks
//!
//! Re-exported from `catalog.rs` via `pub(crate) use` so all existing
//! `crate::chroma_dragon_engine::catalog::THEMES` call sites resolve
//! unchanged.

use crossterm::style::Color;

use super::{ThemeColors, ThemeDef};
use crate::runtime::ColorScheme;

pub static THEMES: &[ThemeDef] = &[
    ThemeDef {
        scheme: ColorScheme::Green,
        def: ThemeColors::StopsWithC16 {
            stops: &[
                (0, 12, 1),
                (0, 45, 6),
                (23, 126, 42),
                (55, 218, 83),
                (80, 255, 110),
                (125, 255, 150),
                (170, 255, 190),
                (201, 244, 210),
            ],
            steps: 7,
            // NIGHT-hunter-11a c16 quality pass: extended from the
            // 2-anchor [DarkGreen, Green] to the family's 3-anchor
            // convention (matches green2's quantized head and green3's
            // White head) — the classic matrix look: dark-green trail,
            // bright green body, white head bloom.
            c16: &[Color::DarkGreen, Color::Green, Color::White],
            ansi: &[234, 22, 28, 35, 78, 84, 159],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Green2,
        def: ThemeColors::StopsWithC16 {
            stops: &[
                (0, 20, 5),
                (5, 70, 18),
                (37, 142, 66),
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
                (30, 150, 120),
                (67, 251, 206),
                (70, 255, 210),
                (115, 255, 218),
                (160, 255, 225),
                (190, 242, 223),
            ],
            steps: 7,
            // NIGHT-hunter-11a c16 quality pass: the missing mid anchor
            // [DarkGreen, White] → [DarkGreen, Green, White] — the
            // spring-green body (70,255,210) maps to the bright Green
            // slot, keeping the 3-anchor ladder convention and a
            // graduated body instead of a two-step jump.
            c16: &[Color::DarkGreen, Color::Green, Color::White],
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
                (15, 126, 38),
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
                (134, 58, 208),
                (180, 90, 255),
                (198, 120, 255),
                (215, 150, 255),
                (235, 195, 255),
                (220, 180, 255),
                (220, 180, 255),
                (220, 180, 255),
            ],
            steps: 7,
            // NIGHT-hunter-11a c16 quality pass: extended from the
            // 2-anchor [Magenta, White] to the neon family's 3-anchor
            // convention (matches NeonRed/NeonBlue/NeonCyan): deep
            // DarkMagenta trail, saturated Magenta body, White head.
            c16: &[Color::DarkMagenta, Color::Magenta, Color::White],
            ansi: &[53, 90, 135, 177, 213, 225, 231],
        },
    },
    ThemeDef {
        scheme: ColorScheme::NeonWhite,
        def: ThemeColors::StopsWithC16 {
            stops: &[
                (5, 6, 8),
                (28, 32, 40),
                (103, 112, 135),
                (190, 204, 244),
                (190, 205, 245),
                (205, 220, 250),
                (220, 235, 255),
                (212, 219, 224),
            ],
            steps: 7,
            // NIGHT-hunter-11a c16 quality pass: extended from the
            // 2-anchor [DarkGrey, White] to the 3-anchor grayscale
            // ladder [DarkGrey, Grey, White] — a graduated mid step
            // instead of a two-step jump.
            c16: &[Color::DarkGrey, Color::Grey, Color::White],
            ansi: &[232, 238, 244, 249, 252, 255, 231],
        },
    },
    ThemeDef {
        scheme: ColorScheme::NeonBlue,
        def: ThemeColors::StopsWithC16 {
            stops: &[
                (0, 5, 20),
                (0, 30, 90),
                (24, 84, 156),
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
                (149, 35, 34),
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
                (153, 68, 12),
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
                (49, 39, 0),
                (80, 65, 0),
                (161, 144, 26),
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
                (33, 137, 156),
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
                (68, 72, 78),
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
                (130, 95, 28),
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
        def: ThemeColors::StopsWithC16 {
            // Warm signal yellow: dark olive origin → rich amber-yellow →
            // pale buttercream head.
            //
            // NIGHT-hunter-11a c16 quality pass: quantization produced an
            // INVERTED head hierarchy — the saturated body stops landed on
            // Yellow (slot L 0.97) while the pale buttercream head landed
            // on Grey (0.92), so the head was the dimmest slot of the top
            // three. The hand-tuned ladder DarkYellow → Yellow → White
            // restores a monotone ramp with a bright white head. The ansi
            // ladder is the exact Color256 output of the pre-change
            // gradient, so 256-color rendering is byte-identical.
            stops: &[
                (18, 15, 0),
                (65, 55, 0),
                (139, 124, 23),
                (222, 200, 53),
                (255, 235, 75),
                (255, 240, 108),
                (255, 245, 140),
                (237, 233, 185),
            ],
            steps: 9,
            c16: &[Color::DarkYellow, Color::Yellow, Color::White],
            ansi: &[237, 237, 94, 142, 220, 227, 228, 228, 223],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Orange,
        def: ThemeColors::Stops {
            // Amber-orange: burnt umber origin → tangerine → warm peach head.
            stops: &[
                (20, 5, 0),
                (75, 20, 0),
                (146, 70, 14),
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
                (142, 36, 34),
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
                (21, 83, 159),
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
                (22, 120, 145),
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
                (103, 45, 146),
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
                (95, 36, 121),
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
                (131, 54, 7),
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
                (13, 85, 118),
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
            // Moss and canopy (v80.0.0 real-color tune): forest-floor
            // brown -> moss green -> real sunlit foliage -> pale
            // sage-lime head. The old upper body pinned the green
            // channel at 255 for three consecutive stops
            // (140/168/195, 255, ~) — neon-lime, not foliage. Real
            // sunlit leaves never max G while R climbs to 195:
            // chlorophyll reflectance keeps every channel in motion,
            // and pale foliage desaturates rather than saturating.
            // The plateau is replaced with true chartreuse foliage
            // steps; tail, canopy and head were already faithful and
            // are unchanged (head keeps the 655 family luminance sum).
            stops: &[
                (8, 10, 0),
                (30, 50, 8),
                (69, 128, 44),
                (110, 216, 84),
                (150, 235, 120),
                (175, 240, 155),
                (195, 240, 185),
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
                (130, 58, 142),
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
                (86, 87, 93),
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
        def: ThemeColors::StopsWithC16 {
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
            //
            // NIGHT-hunter-11a c16 quality pass: OKLab-nearest
            // quantization broke the rainbow in two ways — the orange
            // band collapsed to the NEUTRAL DarkGrey slot (an L-gap
            // artifact: orange L 0.65 sits between DarkYellow 0.82 and
            // DarkRed 0.53, and the neutral slot at 0.60 won the
            // distance contest), and the head (violet) quantized to
            // Blue (L 0.58), DIMMER than the trail Red (L 0.63) — an
            // inverted hierarchy. The hand-tuned ladder walks the
            // saturated hue slots in stop order and closes on Magenta
            // (the c16 violet, L 0.70 > trail 0.63) so the head keeps
            // the anchor role. Rainbow is a hue-cycle theme: slot-L
            // dips inside the ladder are inherent and documented as
            // the hue-cycle exemption in the c16 anchor invariants.
            // The ansi ladder is the exact Color256 output of the
            // pre-change gradient, so 256-color rendering is
            // byte-identical.
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
            c16: &[
                Color::Red,
                Color::Red,
                Color::DarkYellow,
                Color::DarkYellow,
                Color::DarkGreen,
                Color::DarkCyan,
                Color::DarkCyan,
                Color::Blue,
                Color::Magenta,
            ],
            ansi: &[167, 166, 172, 136, 35, 37, 38, 69, 134],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Snow,
        def: ThemeColors::Stops {
            // Cold white-blue shimmer (v80.0.0 real-color tune): deep
            // blue-black -> ice blue -> frosty pale-cyan head. Snow is
            // never neutral gray in nature — shadowed snow carries a
            // strong blue cast from Rayleigh sky-light, and the head
            // must keep that ice tint. The old head (214,218,223) was
            // near-neutral gray (B-R = 9), dropping the body hue in
            // the final stop and contradicting this ramp's own
            // "pale-cyan head" documentation. The new head (192,222,
            // 241) restores the frosty cyan cast at the same 655
            // family luminance sum.
            stops: &[
                (3, 8, 18),
                (20, 38, 65),
                (73, 94, 123),
                (132, 156, 185),
                (180, 210, 245),
                (185, 215, 250),
                (205, 228, 252),
                (225, 240, 255),
                (192, 222, 241),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Aurora,
        def: ThemeColors::Stops {
            // Northern-lights (v80.0.0 real-color tune): the dominant
            // auroral emission is the oxygen 557.7nm line — the
            // iconic curtain green — with cyan shimmer fringes and a
            // green-white glow at the brightest core. The old body was
            // teal-shifted (B up to 78% of G) and the head (188,222,
            // 245) was blue-dominant, contradicting both the real
            // structure and this ramp's own "pale auroral-green head"
            // documentation. The body now tracks the true 557.7nm
            // green; the cyan shimmer stop keeps the curtain-fringe
            // character; the head stays at the 655 family sum, now
            // pale auroral-green as documented.
            stops: &[
                (0, 12, 8),
                (0, 55, 30),
                (15, 140, 70),
                (45, 225, 120),
                (70, 255, 150),
                (110, 245, 200),
                (140, 230, 235),
                (160, 255, 240),
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
                (63, 107, 154),
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
                (52, 47, 136),
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
                (96, 57, 139),
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
    // ── 11 PLANET & SPACE THEMES (v80.0.0 real-color masterclass) ───────
    // 10-12 stop gradient ramps with 9-step interpolation. Every head
    // stop sums to 655 (the planet-family head-luminance convention) and
    // stays tinted with the body hue (not pure white) so each planet
    // reads as itself rather than collapsing to a generic white-bright
    // core.
    //
    // v80.0.0 real-color audit (owner directive 2026-09-02): six palettes
    // retuned to each body's true-color appearance — Mars dusty
    // butterscotch-rust, Venus pale sulfur-cream, Jupiter banded
    // sienna/cream, Saturn hazy butterscotch-gold, Uranus serene pale
    // cyan, Pluto New-Horizons buff-tan (was icy blue-gray — an
    // astronomical misread). Mercury and Moon (grayscale), Sun (warm
    // gold) and Neptune (iconic deep azure) were already faithful and
    // are unchanged.
    ThemeDef {
        scheme: ColorScheme::Stars,
        def: ThemeColors::StopsWithC16 {
            stops: &[
                (0, 0, 0),
                (2, 2, 12),
                (30, 34, 78),
                (30, 35, 80),
                (48, 65, 116),
                (68, 97, 155),
                (90, 130, 200),
                (129, 165, 225),
                (170, 200, 250),
                (205, 224, 252),
                (240, 248, 255),
                (217, 219, 219),
            ],
            steps: 9,
            // NIGHT-hunter-11a c16 quality pass: quantization produced a
            // trail-start defect — the floored black origin rendered as
            // neutral DarkGrey (L 0.60) BRIGHTER than the deep-blue slots
            // that followed (0.43), an inverted dip at the trail anchor,
            // and the near-white stops landed on Grey instead of White.
            // The hand-tuned ladder keeps the starry identity: deep-blue
            // trail → icy cyan body → star-white head. The ansi ladder is
            // the exact Color256 output of the pre-change gradient, so
            // 256-color rendering is byte-identical.
            c16: &[
                Color::DarkBlue,
                Color::DarkBlue,
                Color::DarkCyan,
                Color::Grey,
                Color::White,
            ],
            ansi: &[236, 17, 235, 24, 61, 110, 153, 255, 253],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Mars,
        def: ThemeColors::Stops {
            // v80.0.0 real-color tune: Mars is the butterscotch planet,
            // not a fire. NASA true-color imagery shows a dusty
            // rust-brown surface with pale salmon-tan bright regions;
            // the old neon stops (220,75,30)/(255,130,60) read as
            // embers. Trail keeps the dark-rust character; head stays
            // at the family 655 sum.
            stops: &[
                (14, 4, 2),
                (38, 14, 6),
                (72, 28, 12),
                (110, 45, 20),
                (148, 63, 28),
                (150, 65, 30),
                (190, 100, 55),
                (215, 135, 90),
                (235, 170, 130),
                (236, 220, 199),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Venus,
        def: ThemeColors::StopsWithC16 {
            // v80.0.0 real-color tune: Venus is a pale sulfuric
            // yellow-cream haze, nearly featureless — not saturated
            // amber-gold. Body shifted toward muted old-gold with a
            // brighter cloud-cream ramp; head unchanged at 655.
            //
            // NIGHT-hunter-11a c16 quality pass: same inverted-head defect
            // as yellow — the pale cream head quantized to Grey (L 0.92)
            // under the Yellow body slots (0.97). The hand-tuned ladder
            // DarkYellow → Yellow → White restores the hierarchy. The
            // ansi ladder is the exact Color256 output of the pre-change
            // gradient, so 256-color rendering is byte-identical.
            stops: &[
                (28, 22, 4),
                (58, 45, 10),
                (92, 72, 18),
                (126, 100, 28),
                (162, 136, 44),
                (165, 139, 46),
                (198, 172, 68),
                (226, 202, 102),
                (244, 224, 148),
                (252, 240, 196),
                (255, 249, 205),
                (229, 225, 201),
            ],
            steps: 9,
            c16: &[Color::DarkYellow, Color::Yellow, Color::White],
            ansi: &[237, 237, 94, 136, 143, 185, 222, 230, 254],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Mercury,
        def: ThemeColors::Stops {
            // Real-color verified v80.0.0: Mercury is gray (a
            // sun-baked warm-gray ramp) — unchanged.
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
            // v80.0.0 real-color tune: Jupiter reads as cream-tan
            // zones banded with sienna belts (JunoCam), paler than
            // the old saturated-orange ramp. Belt browns keep the
            // banded identity; head unchanged at 655.
            stops: &[
                (18, 10, 2),
                (50, 24, 6),
                (105, 62, 28),
                (140, 90, 42),
                (143, 93, 45),
                (178, 118, 62),
                (208, 150, 98),
                (232, 184, 132),
                (244, 214, 166),
                (251, 233, 196),
                (231, 221, 203),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Saturn,
        def: ThemeColors::Stops {
            // v80.0.0 real-color tune: Saturn is a hazy pale
            // butterscotch-gold — visibly paler and more muted than
            // Jupiter (as it is astronomically). The old vivid gold
            // stops (240,175,60)/(255,210,95) over-saturated the
            // ringed giant; head unchanged at 655.
            stops: &[
                (22, 14, 2),
                (55, 34, 8),
                (88, 58, 16),
                (125, 88, 28),
                (162, 105, 38),
                (164, 107, 40),
                (202, 148, 62),
                (230, 182, 96),
                (246, 212, 138),
                (252, 232, 178),
                (229, 225, 201),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Uranus,
        def: ThemeColors::Stops {
            // v80.0.0 real-color tune: Uranus is a serene, nearly
            // featureless pale cyan — the calmest planet in the
            // system, not a neon cyan flare. Saturation dialed down
            // across the body; head unchanged at 655.
            stops: &[
                (0, 14, 14),
                (0, 38, 40),
                (0, 66, 70),
                (0, 95, 100),
                (2, 120, 126),
                (4, 122, 128),
                (35, 150, 158),
                (70, 178, 186),
                (115, 205, 212),
                (160, 228, 232),
                (205, 245, 247),
                (207, 224, 224),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Neptune,
        def: ThemeColors::Stops {
            // Real-color verified v80.0.0: Neptune keeps its iconic
            // deep azure (the Voyager-2-recognized identity) —
            // unchanged.
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
            // v80.0.0 real-color fix: Pluto is NOT icy blue-gray —
            // New Horizons (2015) imaged a warm buff-tan dwarf with
            // dark red-brown maculae and a pale cream heart. The
            // whole ramp moves from cold steel-blue to dusty tan,
            // with a macula-brown trail and camel-to-cream body;
            // head stays at the family 655 sum.
            stops: &[
                (14, 8, 6),
                (32, 18, 12),
                (55, 32, 22),
                (85, 52, 36),
                (105, 68, 48),
                (108, 70, 50),
                (138, 98, 72),
                (170, 130, 100),
                (198, 164, 138),
                (222, 196, 172),
                (230, 222, 203),
            ],
            steps: 9,
        },
    },
    ThemeDef {
        scheme: ColorScheme::Moon,
        def: ThemeColors::StopsWithC16 {
            // Real-color verified v80.0.0: the Moon is a neutral
            // cool gray with a faint blue tint — unchanged.
            //
            // NIGHT-hunter-11a c16 quality pass: quantizing the 11-stop
            // grayscale ramp collapsed all 9 palette entries onto just
            // DarkGrey + Grey (the whitish stops L 0.87-0.96 all landed
            // on Grey 0.92, never White). The hand-tuned 3-anchor
            // ladder DarkGrey → Grey → White restores a full perceptual
            // grayscale gradient with a bright head. The ansi ladder is
            // the exact Color256 output of the pre-change gradient, so
            // 256-color rendering is byte-identical.
            stops: &[
                (0, 0, 0),
                (15, 15, 18),
                (57, 58, 64),
                (78, 80, 88),
                (80, 82, 90),
                (109, 113, 122),
                (140, 145, 155),
                (195, 200, 210),
                (220, 224, 231),
                (245, 248, 252),
                (217, 218, 220),
            ],
            steps: 9,
            c16: &[Color::DarkGrey, Color::Grey, Color::White],
            ansi: &[236, 236, 238, 239, 242, 247, 188, 255, 253],
        },
    },
    ThemeDef {
        scheme: ColorScheme::Sun,
        def: ThemeColors::Stops {
            // Real-color verified v80.0.0: the Sun keeps its warm
            // golden-orange ramp (the perceived solar color) —
            // unchanged.
            stops: &[
                (30, 5, 0),
                (75, 18, 0),
                (133, 45, 0),
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
            // NIGHT-hunter-11a c16 quality pass: extended from the
            // 2-anchor [Magenta, White] to the 3-anchor ladder
            // [DarkMagenta, Magenta, White] — the deep purple void
            // trail (4,0,24) now maps to DarkMagenta, the crystal-edge
            // magenta mid stops map to Magenta, and the pale lilac head
            // keeps its White bloom. Full 3-anchor graduation for the
            // signature palette.
            c16: &[Color::DarkMagenta, Color::Magenta, Color::White],
            ansi: &[53, 90, 135, 177, 207, 225, 231],
        },
    },
];
