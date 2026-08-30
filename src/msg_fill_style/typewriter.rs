// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! msg-fill-style `typewriter` — the classic default.
//!
//! Per-character reveal at `TYPEWRITER_CHAR_MS` (80 ms) with a
//! `TYPEWRITER_FADE_MS` (100 ms) per-char fade-in from 30% to 100%
//! brightness. This is the DEFAULT style and is bit-identical to the
//! pre-v51 renderer — the reveal pacing (`(elapsed / 80).max(1)`) and
//! the fade curve are the original formulas, kept verbatim as the LTS
//! guarantee (upgrading changes nothing unless the user opts in).
//!
//! Border: lags text with the shared `t^1.5` ease-out curve.

use super::{char_fade_in, index_fraction, index_pacing, lagged_border, CellReveal};

/// Per-character reveal stagger.
pub(crate) const TYPEWRITER_CHAR_MS: usize = 80;
/// Per-character fade-in duration.
pub(crate) const TYPEWRITER_FADE_MS: usize = 100;

/// Per-cell reveal: fade-in ramp indexed by the cell's reveal time,
/// gated by the index budget.
pub(super) fn reveal(
    content_idx: usize,
    elapsed_ms: Option<usize>,
    reveal_count: usize,
) -> CellReveal {
    if content_idx < reveal_count {
        let reveal_at = content_idx * TYPEWRITER_CHAR_MS;
        CellReveal {
            visible: true,
            factor: char_fade_in(elapsed_ms, reveal_at, TYPEWRITER_FADE_MS),
            slide_rows: 0,
        }
    } else {
        CellReveal::hidden()
    }
}

/// Index budget: 80 ms/char with the pre-v51 `.max(1)` floor.
pub(super) fn reveal_budget(elapsed_ms: Option<usize>, total_text: usize) -> usize {
    index_pacing(TYPEWRITER_CHAR_MS, elapsed_ms, total_text)
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
    use super::super::{content_reveal, index_reveal_count, MsgFillStyle, FADE_IN_START};
    use super::*;

    #[test]
    fn typewriter_reveals_at_80ms_per_char() {
        // 40-char message: cell 0 visible at t=0 (max(1) rule), cell 4
        // only after 320 ms.
        let total = 40;
        let count = index_reveal_count(MsgFillStyle::Typewriter, Some(319), total);
        assert_eq!(count, 3); // 319/80 = 3
        let count = index_reveal_count(MsgFillStyle::Typewriter, Some(320), total);
        assert_eq!(count, 4); // 320/80 = 4, .max(1) floor
        let first_frame = index_reveal_count(MsgFillStyle::Typewriter, Some(0), total);
        assert_eq!(first_frame, 1);
    }

    #[test]
    fn typewriter_fade_in_ramps_from_30_to_100_percent() {
        let reveal = content_reveal(MsgFillStyle::Typewriter, 0, 1, Some(0), 10, 1.0);
        assert!((reveal.factor - FADE_IN_START).abs() < 1e-6);
        let settled = content_reveal(
            MsgFillStyle::Typewriter,
            0,
            1,
            Some(TYPEWRITER_FADE_MS),
            10,
            1.0,
        );
        assert!((settled.factor - 1.0).abs() < 1e-6);
    }

    #[test]
    fn hidden_cells_stay_hidden_until_reveal_count_reaches_them() {
        let r = content_reveal(MsgFillStyle::Typewriter, 5, 1, Some(100), 3, 1.0);
        assert!(!r.visible);
    }

    #[test]
    fn border_lags_text_with_power_15_for_paced_styles() {
        // Pre-v51 cinematic behavior preserved for typewriter-style
        // pacing: border_progress = text_progress^1.5.
        let bp = super::super::border_progress(MsgFillStyle::Typewriter, 0.5, Some(10_000));
        assert!((bp - 0.353_553_39).abs() < 1e-5);
    }
}
