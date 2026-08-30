// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! msg-fill-style `instant` — everything at t=0, border draws itself.
//!
//! The text appears instantly at full brightness (every cell settled
//! from the first frame); the only animation is the border, which
//! draws itself clockwise over `INSTANT_BORDER_MS` (1 s) on an
//! independent timeline (text progress is a constant 1.0 — the text
//! is not the pacing element).

use super::CellReveal;

/// Independent clockwise border draw duration.
pub(crate) const INSTANT_BORDER_MS: usize = 1000;

/// Per-cell reveal: always settled at full brightness.
pub(super) fn reveal() -> CellReveal {
    CellReveal::settled()
}

/// Index budget: everything revealed from the first frame.
pub(super) fn reveal_budget(elapsed_ms: Option<usize>, total_text: usize) -> usize {
    match elapsed_ms {
        None => usize::MAX,
        Some(_) => total_text.max(1),
    }
}

/// Border draws clockwise on its own 1 s timeline.
pub(super) fn border_progress(elapsed_ms: Option<usize>) -> f32 {
    match elapsed_ms {
        None => 1.0,
        Some(ms) => (ms as f32 / INSTANT_BORDER_MS as f32).min(1.0),
    }
}

/// Text is not the pacing element for this style.
pub(super) fn text_progress() -> f32 {
    1.0
}

#[cfg(test)]
mod tests {
    use super::super::{border_progress, content_reveal, MsgFillStyle};
    use super::*;

    #[test]
    fn instant_style_settles_immediately() {
        let r = content_reveal(MsgFillStyle::Instant, 0, 1, Some(0), 0, 1.0);
        assert!(r.visible);
        assert!((r.factor - 1.0).abs() < 1e-6);
        assert_eq!(r.slide_rows, 0);
    }

    #[test]
    fn instant_border_draws_over_one_second() {
        assert!((border_progress(MsgFillStyle::Instant, 1.0, Some(0)) - 0.0).abs() < 1e-6);
        assert!((border_progress(MsgFillStyle::Instant, 1.0, Some(500)) - 0.5).abs() < 1e-6);
        assert!(
            (border_progress(MsgFillStyle::Instant, 1.0, Some(INSTANT_BORDER_MS)) - 1.0).abs()
                < 1e-6
        );
    }
}
