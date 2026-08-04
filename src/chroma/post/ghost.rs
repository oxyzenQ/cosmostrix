// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Palette-aware Ghost Color
//!
//! Chroma Dragon Innovation I — replaces the hardcoded `GHOST_BASE_COLOR`
//! in `cloud::events::ghost` with a color derived from the current palette's
//! darkest stop.
//!
//! ## Problem (pre-Phase-3-I)
//!
//! Ghost events (kanji characters that fade in/out on dim rain cells) used
//! a hardcoded base color `(18, 22, 18)` — a dark green that matched the
//! default Green palette but clashed with every other theme. On a Red
//! palette, ghosts were still green. On a Blue palette, ghosts were still
//! green. The ghost effect broke the scene's color coherence.
//!
//! ## Solution
//!
//! `ghost_base_color(palette_colors)` derives the ghost color from
//! `palette_colors[0]` (the darkest stop, used for tail cells). The
//! function:
//!
//! 1. Reads the darkest stop from the palette.
//! 2. Decodes it to `(r, g, b)` via `chroma::palette::color_to_rgb`.
//! 3. Scales each channel by `GHOST_DIM_FACTOR` (0.2) — producing a color
//!    that preserves the palette's hue ratio but is dim enough to read as
//!    a "ghost" (≈20% of the darkest stop's brightness).
//! 4. Falls back to `(18, 22, 18)` if the palette is empty or the darkest
//!    stop is `Color::Reset` (no RGB to derive from).
//!
//! ## Effect
//!
//! Ghosts now match the scene's color scheme:
//!
//! - Green palette → dark green ghosts (matches pre-Phase-3-I behavior)
//! - Red palette → dark red ghosts
//! - Blue palette → dark blue ghosts
//! - Cyan palette → dark cyan ghosts
//!
//! The hue is preserved from the palette, so ghosts feel like a natural
//! extension of the rain rather than an unrelated overlay.

use crossterm::style::Color;

use crate::chroma::palette::color_to_rgb;

/// Scaling factor for the ghost base color. The darkest palette stop is
/// multiplied by this factor to produce a dim, ghostly color.
///
/// 0.2 = 20% brightness. This matches the pre-Phase-3-I perceived
/// brightness of the hardcoded `(18, 22, 18)` against a typical Green
/// palette whose darkest stop is around `(0, 100, 0)` —
/// `100 * 0.2 = 20 ≈ 18`. For brighter palettes (e.g., Rainbow with a
/// darker dark stop), the ghost stays proportional.
const GHOST_DIM_FACTOR: f32 = 0.2;

/// Fallback color when the palette is empty or the darkest stop has no
/// RGB (e.g., `Color::Reset`). Matches the pre-Phase-3-I hardcoded
/// `GHOST_BASE_COLOR` so the ghost effect still works in edge cases.
const GHOST_FALLBACK_COLOR: (u8, u8, u8) = (18, 22, 18);

/// Derive the ghost base color from a palette's darkest stop.
///
/// Takes the first color in `palette_colors` (the darkest stop, used for
/// tail cells in the rain shader), decodes it to RGB, and scales each
/// channel by `GHOST_DIM_FACTOR` (0.2). The result is a dim, palette-hued
/// color suitable for ghost events.
///
/// Falls back to `GHOST_FALLBACK_COLOR` (the pre-Phase-3-I hardcoded
/// `(18, 22, 18)`) when:
/// - `palette_colors` is empty
/// - The darkest stop is `Color::Reset` (no RGB to derive from)
///
/// Returns `(r, g, b)` ready for use as the ghost base color (before the
/// opacity fade is applied by the ghost renderer).
#[inline]
pub(crate) fn ghost_base_color(palette_colors: &[Color]) -> (u8, u8, u8) {
    let Some(darkest) = palette_colors.first().copied() else {
        return GHOST_FALLBACK_COLOR;
    };
    if matches!(darkest, Color::Reset) {
        return GHOST_FALLBACK_COLOR;
    }
    let (r, g, b) = color_to_rgb(darkest);
    (
        (r as f32 * GHOST_DIM_FACTOR).round().clamp(0.0, 255.0) as u8,
        (g as f32 * GHOST_DIM_FACTOR).round().clamp(0.0, 255.0) as u8,
        (b as f32 * GHOST_DIM_FACTOR).round().clamp(0.0, 255.0) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Empty palette → fallback color.
    #[test]
    fn empty_palette_uses_fallback() {
        assert_eq!(ghost_base_color(&[]), GHOST_FALLBACK_COLOR);
    }

    /// Palette with Color::Reset as darkest → fallback color.
    #[test]
    fn reset_darkest_uses_fallback() {
        let palette = [
            Color::Reset,
            Color::Rgb {
                r: 100,
                g: 100,
                b: 100,
            },
        ];
        assert_eq!(ghost_base_color(&palette), GHOST_FALLBACK_COLOR);
    }

    /// Green palette → dark green ghost (preserves hue, dims brightness).
    /// Pre-Phase-3-I hardcoded value was (18, 22, 18); a green palette
    /// with darkest stop (0, 100, 0) produces (0, 20, 0) — same hue,
    /// slightly different brightness but in the same ballpark.
    #[test]
    fn green_palette_produces_dark_green_ghost() {
        let palette = [
            Color::Rgb { r: 0, g: 100, b: 0 },
            Color::Rgb { r: 0, g: 200, b: 0 },
            Color::Rgb {
                r: 100,
                g: 255,
                b: 100,
            },
        ];
        let (r, g, b) = ghost_base_color(&palette);
        // 0 * 0.2 = 0, 100 * 0.2 = 20, 0 * 0.2 = 0
        assert_eq!((r, g, b), (0, 20, 0));
        // Hue is preserved: green channel dominant.
        assert!(g >= r && g >= b, "green should be the dominant channel");
    }

    /// Red palette → dark red ghost.
    #[test]
    fn red_palette_produces_dark_red_ghost() {
        let palette = [
            Color::Rgb { r: 100, g: 0, b: 0 },
            Color::Rgb { r: 200, g: 0, b: 0 },
            Color::Rgb {
                r: 255,
                g: 100,
                b: 100,
            },
        ];
        let (r, g, b) = ghost_base_color(&palette);
        assert_eq!((r, g, b), (20, 0, 0));
        assert!(r >= g && r >= b, "red should be the dominant channel");
    }

    /// Blue palette → dark blue ghost.
    #[test]
    fn blue_palette_produces_dark_blue_ghost() {
        let palette = [
            Color::Rgb { r: 0, g: 0, b: 100 },
            Color::Rgb { r: 0, g: 0, b: 200 },
            Color::Rgb {
                r: 100,
                g: 100,
                b: 255,
            },
        ];
        let (r, g, b) = ghost_base_color(&palette);
        assert_eq!((r, g, b), (0, 0, 20));
        assert!(b >= r && b >= g, "blue should be the dominant channel");
    }

    /// Cyan palette → dark cyan ghost (green + blue channels).
    #[test]
    fn cyan_palette_produces_dark_cyan_ghost() {
        let palette = [
            Color::Rgb {
                r: 0,
                g: 100,
                b: 100,
            },
            Color::Rgb {
                r: 0,
                g: 200,
                b: 200,
            },
            Color::Rgb {
                r: 100,
                g: 255,
                b: 255,
            },
        ];
        let (r, g, b) = ghost_base_color(&palette);
        assert_eq!((r, g, b), (0, 20, 20));
        // Cyan = green + blue, both channels equal.
        assert_eq!(g, b, "cyan should have equal green and blue");
        assert!(g > r, "red should be the smallest channel");
    }

    /// White palette → light gray ghost (all channels scaled equally).
    #[test]
    fn white_palette_produces_light_gray_ghost() {
        let palette = [
            Color::Rgb {
                r: 255,
                g: 255,
                b: 255,
            },
            Color::Rgb {
                r: 200,
                g: 200,
                b: 200,
            },
        ];
        let (r, g, b) = ghost_base_color(&palette);
        // 255 * 0.2 = 51
        assert_eq!((r, g, b), (51, 51, 51));
    }

    /// Black palette → black ghost (all channels zero). This is an edge
    /// case — the ghost would be invisible. The ghost renderer's opacity
    /// fade handles this by skipping render when all channels are zero.
    #[test]
    fn black_palette_produces_black_ghost() {
        let palette = [Color::Rgb { r: 0, g: 0, b: 0 }];
        let (r, g, b) = ghost_base_color(&palette);
        assert_eq!((r, g, b), (0, 0, 0));
    }

    /// Only the FIRST palette color is used — other colors in the palette
    /// don't affect the ghost base color.
    #[test]
    fn only_first_palette_color_used() {
        let palette_a = [
            Color::Rgb { r: 100, g: 0, b: 0 },
            Color::Rgb { r: 0, g: 255, b: 0 },
            Color::Rgb { r: 0, g: 0, b: 255 },
        ];
        let palette_b = [
            Color::Rgb { r: 100, g: 0, b: 0 },
            Color::Rgb {
                r: 255,
                g: 255,
                b: 255,
            },
        ];
        assert_eq!(
            ghost_base_color(&palette_a),
            ghost_base_color(&palette_b),
            "same darkest stop must produce same ghost color"
        );
    }

    /// Non-RGB color types (AnsiValue, Ansi16) are decoded via
    /// color_to_rgb and scaled. Verify the function doesn't panic on
    /// these types.
    #[test]
    fn ansi_color_types_handled_gracefully() {
        let palette = [Color::AnsiValue(2)]; // ANSI green
        let (r, g, b) = ghost_base_color(&palette);
        // Just verify it returns something (the actual RGB depends on the
        // terminal's ANSI palette, which color_to_rgb approximates).
        let _ = (r, g, b);
    }

    /// Deterministic: same palette → same ghost color.
    #[test]
    fn ghost_base_color_is_deterministic() {
        let palette = [Color::Rgb {
            r: 100,
            g: 50,
            b: 200,
        }];
        let a = ghost_base_color(&palette);
        let b = ghost_base_color(&palette);
        assert_eq!(a, b);
    }
}
