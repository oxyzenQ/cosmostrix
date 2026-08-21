// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Intro Cinematic Color Constants — Chroma Dragon Integration
//!
//! Single source of truth for ALL color data used by the intro cinematic
//! system (Cosmic Burst and Logo intros). Before this module, brand colors
//! were scattered as orphan constants across `interactive/intro_cosmic.rs`
//! and `interactive/intro_logo/mod.rs`, making them invisible to the chroma
//! dragon engine's audit surface.
//!
//! ## Why here and not in the intro modules?
//!
//! Owner mandate: *"every color-processing site must route through the
//! chroma dragon pipeline (primary)"*. The intro's color *processing*
//! (blending, palette extraction) already routes through the engine via
//! `gradient::oklab_blend_rgb` and `palette::color_to_rgb`. But the
//! color *constants* — the actual RGB values that serve as inputs to
//! those operations — lived outside the engine. Moving them here makes
//! the chroma dragon engine the canonical owner of ALL color data in
//! cosmostrix, not just the rain palette.
//!
//! ## Design decisions
//!
//! - Brand colors are palette-independent by design. The Cosmic Burst
//!   always uses gold/purple/cyan regardless of the user's `--color` flag,
//!   so the brand mark stays consistent across all palette themes.
//! - These constants are `pub(crate)` so intro modules in `interactive/`
//!   can import them, but they are NOT part of the public API.
//! - All blending of these colors still routes through
//!   `gradient::oklab_blend_rgb()` at the call site — this module owns
//!   the *data*, not the *processing*.
//!
//! ## Relationship to `catalog.rs`
//!
//! `catalog.rs` owns rain palette theme definitions (40+ themes).
//! This module owns intro cinematic brand colors (3 constants).
//! They serve different purposes and should not be merged.

// ─────────────────────────────────────────────────────────────────────────────
// Cosmic Burst intro colors
// ─────────────────────────────────────────────────────────────────────────────

/// Cosmic Burst particle color palette (RGB).
///
/// Sampled by per-particle random index during Phase 2 (burst) — the
/// explosion alternates gold (energy), brand purple (identity), and
/// cyan (plasma) for visual variety.
///
/// The brand-purple slot (`[1]`) is replaced at runtime by `logo_color`
/// (from `--intro-color` or the default brand purple) when
/// `intro-color` is set in config. The constant here is the fallback
/// used when no override is provided.
///
/// ## Brightness contract
///
/// Every stop must have at least one channel >= 200. Enforced by
/// `intro_cosmic::tests::cosmic_colors_are_valid`.
pub(crate) const COSMIC_COLORS_RGB: [(u8, u8, u8); 3] = [
    (255, 200, 0),  // bright gold — energy, warmth
    (168, 85, 247), // brand purple — cosmostrix identity (#A855F7)
    (0, 255, 255),  // cyan — plasma, cold counterpoint to gold
];

/// Singularity color — pure white-hot at the center of the Cosmic Burst.
///
/// Rendered as a pulsing `@` glyph during Phase 1 (0–1 s) and the
/// early part of Phase 2. Brightness is modulated by a chirped
/// triangle wave (3 Hz → 9 Hz) and then faded out as the burst
/// takes over.
pub(crate) const SINGULARITY_RGB: (u8, u8, u8) = (255, 255, 255);

// ─────────────────────────────────────────────────────────────────────────────
// Logo intro colors
// ─────────────────────────────────────────────────────────────────────────────

/// Brand purple — the Cosmostrix signature color (`#A855F7` / RGB
/// 168,85,247).
///
/// The logo always renders in this color (or the `--intro-color`
/// override), regardless of the user's `--color` flag, so the brand
/// mark stays consistent across all palette themes. During the
/// dissolve/rain phase, droplets interpolate from this purple toward
/// the active rain palette's brightest stop, creating a cinematic
/// "brand → rain" handoff.
///
/// BL-05 (Dragon Hunt v3): removed the `Color` enum form (was
/// test-only + had a tautology test). This RGB tuple is the single
/// canonical form — when a `Color` is needed (rare), construct it
/// inline: `Color::Rgb { r: 168, g: 85, b: 247 }`.
///
/// Default brand purple — kept for reference. Replaced by the
/// `logo_color` parameter at runtime when `--intro-color` is set.
#[allow(dead_code)]
pub(crate) const LOGO_COLOR_RGB: (u8, u8, u8) = (168, 85, 247);

/// Neon green fallback for `palette_target_rgb()` when the palette
/// is empty. Matches the original fallback used in `intro::palette_target_rgb`.
pub(crate) const NEON_GREEN_FALLBACK: (u8, u8, u8) = (57, 255, 20);

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosmic_colors_are_bright() {
        // Every cosmic color must have at least one channel >= 200.
        for &(r, g, b) in &COSMIC_COLORS_RGB {
            let max = r.max(g).max(b);
            assert!(
                max >= 200,
                "cosmic color ({r},{g},{b}) should have max channel >= 200, got {max}"
            );
        }
    }

    #[test]
    fn cosmic_brand_purple_matches_logo_brand() {
        // The brand purple in COSMIC_COLORS_RGB must match LOGO_COLOR_RGB.
        assert_eq!(
            COSMIC_COLORS_RGB[1], LOGO_COLOR_RGB,
            "cosmic burst brand slot must match logo brand color"
        );
    }

    #[test]
    fn singularity_is_pure_white() {
        assert_eq!(SINGULARITY_RGB, (255, 255, 255));
    }

    #[test]
    fn logo_color_is_brand_purple() {
        assert_eq!(LOGO_COLOR_RGB, (168, 85, 247));
    }

    #[test]
    fn neon_green_fallback_is_visible() {
        let (r, g, b) = NEON_GREEN_FALLBACK;
        assert!(g > 200, "neon green fallback must have bright G channel");
        assert!(
            r < 100 && b < 100,
            "neon green fallback should be predominantly green"
        );
    }

    #[test]
    fn cosmic_colors_are_distinct() {
        // All three cosmic colors must differ from each other.
        for (i, ci) in COSMIC_COLORS_RGB.iter().enumerate() {
            for (j, cj) in COSMIC_COLORS_RGB.iter().enumerate().skip(i + 1) {
                assert_ne!(
                    ci, cj,
                    "cosmic colors[{i}] and [{j}] must be distinct"
                );
            }
        }
    }
}
