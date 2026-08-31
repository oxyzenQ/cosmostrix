// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! msg-fill-style `fade` — whole-block fade-in.
//!
//! The text appears INSTANTLY (every cell settled); the whole block
//! alpha ramps 0% → 100% over `FADE_BLOCK_MS` (800 ms). At alpha 0.0
//! nothing is drawn ("fade from 0%").
//!
//! Border: fades together with the text block (follows the same
//! block-alpha curve instead of the text-lag curve) — the border
//! brightness in the renderer is also scaled by the block alpha so
//! the frame fades in as one unit.

use super::CellReveal;

/// Whole-block alpha ramp duration.
pub(crate) const FADE_BLOCK_MS: usize = 800;

/// Per-cell reveal: visible at the current block alpha (the block
/// alpha is computed once per frame by the renderer and handed in).
pub(super) fn reveal(block_alpha: f32) -> CellReveal {
    // Whole block appears instantly, then the shared alpha ramps.
    // At alpha 0.0 nothing is drawn (matches "fade from 0%").
    if block_alpha > 0.0 {
        CellReveal {
            visible: true,
            factor: block_alpha,
            slide_rows: 0,
            glyph_override: None,
            tint: None,
        }
    } else {
        CellReveal::hidden()
    }
}

/// Whole-block alpha from elapsed time (0.0 → 1.0 over FADE_BLOCK_MS;
/// 1.0 with no timeline). Also exposed crate-wide as
/// `super::fade_block_alpha` for the renderer's border scaling.
#[inline]
pub(super) fn block_alpha(elapsed_ms: Option<usize>) -> f32 {
    match elapsed_ms {
        None => 1.0,
        Some(ms) => (ms as f32 / FADE_BLOCK_MS as f32).min(1.0),
    }
}

/// Index budget: block style — everything is a candidate; the block
/// alpha decides per frame (`None` timeline still yields `usize::MAX`).
pub(super) fn reveal_budget(elapsed_ms: Option<usize>, total_text: usize) -> usize {
    match elapsed_ms {
        None => usize::MAX,
        Some(_) => total_text.max(1),
    }
}

/// Border follows the block alpha, not the text progress.
pub(super) fn border_progress(elapsed_ms: Option<usize>) -> f32 {
    block_alpha(elapsed_ms)
}

/// Text is not the pacing element for this style.
pub(super) fn text_progress() -> f32 {
    1.0
}

#[cfg(test)]
mod tests {
    use super::super::{border_progress, content_reveal, fade_block_alpha, MsgFillStyle};

    #[test]
    fn fade_block_alpha_ramps_over_800ms() {
        assert!((fade_block_alpha(Some(0)) - 0.0).abs() < 1e-6);
        assert!((fade_block_alpha(Some(400)) - 0.5).abs() < 1e-6);
        assert!((fade_block_alpha(Some(800)) - 1.0).abs() < 1e-6);
        assert!((fade_block_alpha(Some(10_000)) - 1.0).abs() < 1e-6);
        // No timeline (None) → fully visible.
        assert!((fade_block_alpha(None) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn fade_style_hides_block_at_alpha_zero() {
        let r = content_reveal(MsgFillStyle::Fade, 0, 1, Some(0), 0, 0.0);
        assert!(!r.visible);
        let r = content_reveal(MsgFillStyle::Fade, 0, 1, Some(1), 0, 0.001);
        assert!(r.visible);
        assert!((r.factor - 0.001).abs() < 1e-6);
    }

    #[test]
    fn fade_border_follows_block_alpha() {
        // Border follows the block alpha, not the text progress.
        let bp = border_progress(MsgFillStyle::Fade, 1.0, Some(400));
        assert!((bp - 0.5).abs() < 1e-6);
    }
}
