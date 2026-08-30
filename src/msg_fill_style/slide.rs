// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! msg-fill-style `slide` — chars rise from one row below.
//!
//! Each character reveals at `SLIDE_CHAR_MS` (60 ms) stagger and then
//! travels over `SLIDE_TRAVEL_MS` (240 ms) in two phases:
//!
//! - Phase 1 (first half of travel): the glyph is drawn ONE ROW BELOW
//!   its final position, dim (max `SLIDE_BELOW_MAX` = 70%) while it
//!   fades in. The renderer blanks the final cell and defers the
//!   moving glyph to a second pass (the row below is itself a message
//!   cell that would otherwise overwrite the sliding glyph).
//! - Phase 2 (second half): the glyph has landed — the fade completes
//!   at the final position (30% → 100%).
//!
//! Border: lags text with the shared `t^1.5` ease-out curve.

use super::{index_fraction, index_pacing, lagged_border, CellReveal, FADE_IN_START};

/// Per-character reveal stagger.
pub(crate) const SLIDE_CHAR_MS: usize = 60;
/// Travel time (fade-in below, then land at the final row).
pub(crate) const SLIDE_TRAVEL_MS: usize = 240;
/// Peak brightness while the char is still one row below.
pub(crate) const SLIDE_BELOW_MAX: f32 = 0.70;

/// Per-cell reveal: two-phase rise (below → landed).
pub(super) fn reveal(content_idx: usize, elapsed_ms: Option<usize>) -> CellReveal {
    let reveal_at = content_idx * SLIDE_CHAR_MS;
    match elapsed_ms {
        None => CellReveal::settled(),
        Some(ms) => {
            if ms < reveal_at {
                CellReveal::hidden()
            } else {
                let age = ms - reveal_at;
                let progress = (age as f32 / SLIDE_TRAVEL_MS as f32).min(1.0);
                if progress < 0.5 {
                    // Phase 1: fading in one row below the final
                    // position (dim — max SLIDE_BELOW_MAX).
                    CellReveal {
                        visible: true,
                        factor: progress * 2.0 * SLIDE_BELOW_MAX,
                        slide_rows: 1,
                    }
                } else {
                    // Phase 2: landed. Complete the fade at the
                    // final position (30% → 100%). Indexed by travel
                    // phase, not cell age — the shared char_fade_in
                    // ramp shape with a phase-derived input.
                    let p2 = (progress - 0.5) * 2.0;
                    CellReveal {
                        visible: true,
                        factor: FADE_IN_START + (1.0 - FADE_IN_START) * p2,
                        slide_rows: 0,
                    }
                }
            }
        }
    }
}

/// Index budget: 60 ms/char with the pre-v51 `.max(1)` floor.
pub(super) fn reveal_budget(elapsed_ms: Option<usize>, total_text: usize) -> usize {
    index_pacing(SLIDE_CHAR_MS, elapsed_ms, total_text)
}

/// Border lags text (t^1.5) — the pre-v51 cinematic curve.
pub(super) fn border_progress(text_progress: f32) -> f32 {
    lagged_border(text_progress)
}

/// Text progress: revealed-cell fraction.
pub(super) fn text_progress(reveal_count: usize, total_text: usize) -> f32 {
    index_fraction(reveal_count, total_text)
}

#[cfg(test)]
mod tests {
    use super::super::content_reveal;
    use super::*;

    #[test]
    fn slide_char_rises_from_below_then_lands() {
        // Cell 0 reveal at t=0. Phase 1 (age < 120 ms): one row below,
        // dim. Phase 2 (120..240 ms): landed, ramping 30%→100%.
        let below = content_reveal(super::super::MsgFillStyle::Slide, 0, 1, Some(60), 10, 1.0);
        assert!(below.visible);
        assert_eq!(below.slide_rows, 1);
        assert!(below.factor < SLIDE_BELOW_MAX + 1e-6);

        let landed = content_reveal(super::super::MsgFillStyle::Slide, 0, 1, Some(180), 10, 1.0);
        assert!(landed.visible);
        assert_eq!(landed.slide_rows, 0);
        assert!(landed.factor > FADE_IN_START);
        assert!(landed.factor < 1.0);

        let settled = content_reveal(
            super::super::MsgFillStyle::Slide,
            0,
            1,
            Some(SLIDE_TRAVEL_MS),
            10,
            1.0,
        );
        assert!(settled.visible);
        assert_eq!(settled.slide_rows, 0);
        assert!((settled.factor - 1.0).abs() < 1e-6);

        // Cell 3 does not start until 3 * 60 = 180 ms.
        let not_yet = content_reveal(super::super::MsgFillStyle::Slide, 3, 1, Some(179), 10, 1.0);
        assert!(!not_yet.visible);
    }
}
