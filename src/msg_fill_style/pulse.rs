// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! msg-fill-style `pulse` — typewriter plus a scanner cursor.
//!
//! The reveal IS the typewriter reveal (80 ms/char, 100 ms fade-in —
//! constants imported from `typewriter.rs`, because pulse is
//! literally "typewriter + scanner"): every cell's fade factor is
//! multiplied by a scanner boost that peaks at `(1 + PULSE_BOOST)`
//! (1.5x) the moment the scanner head reaches the cell and decays
//! back to 1.0 over `PULSE_DECAY_MS` (200 ms). The boost factor goes
//! through the renderer's unclamped brightness path (factors > 1.0
//! are allowed; per-channel clamp at 255 downstream).
//!
//! Border: lags text with the shared `t^1.5` ease-out curve.

use super::typewriter;
use super::{index_fraction, index_pacing, lagged_border, CellReveal};

/// Scanner-cursor boost applied to recently revealed chars.
pub(crate) const PULSE_BOOST: f32 = 0.50;
/// Scanner-cursor decay window.
pub(crate) const PULSE_DECAY_MS: usize = 200;

/// Per-cell reveal: typewriter base × scanner boost.
pub(super) fn reveal(
    content_idx: usize,
    elapsed_ms: Option<usize>,
    reveal_count: usize,
) -> CellReveal {
    let base = typewriter::reveal(content_idx, elapsed_ms, reveal_count);
    if !base.visible {
        return CellReveal::hidden();
    }
    let reveal_at = content_idx * typewriter::TYPEWRITER_CHAR_MS;
    // Scanner head: recently revealed chars glow up to
    // (1 + PULSE_BOOST) and decay to 1.0 over PULSE_DECAY_MS.
    let boost = match elapsed_ms {
        None => 1.0,
        Some(ms) => {
            let age = ms.saturating_sub(reveal_at);
            if age >= PULSE_DECAY_MS {
                1.0
            } else {
                let decay = 1.0 - age as f32 / PULSE_DECAY_MS as f32;
                1.0 + PULSE_BOOST * decay
            }
        }
    };
    CellReveal {
        visible: true,
        factor: base.factor * boost,
        slide_rows: 0,
        glyph_override: None,
        tint: None,
    }
}

/// Index budget: identical to typewriter (same pacing constant).
pub(super) fn reveal_budget(elapsed_ms: Option<usize>, total_text: usize) -> usize {
    index_pacing(typewriter::TYPEWRITER_CHAR_MS, elapsed_ms, total_text)
}

/// Border lags text (t^1.5) — the pre-v51 cinematic curve.
pub(super) fn border_progress(text_progress: f32) -> f32 {
    lagged_border(text_progress)
}

/// Text progress: revealed-cell fraction (same as typewriter).
pub(super) fn text_progress(reveal_count: usize, total_text: usize) -> f32 {
    index_fraction(reveal_count, total_text)
}

#[cfg(test)]
mod tests {
    use super::super::{content_reveal, MsgFillStyle};
    use super::*;

    #[test]
    fn pulse_scanner_boosts_recent_chars_and_decays() {
        // Cell 0 at age 0: fade factor 0.30 * (1 + 0.5) = 0.45.
        let head = content_reveal(MsgFillStyle::Pulse, 0, 1, Some(0), 10, 1.0);
        assert!((head.factor - super::super::FADE_IN_START * (1.0 + PULSE_BOOST)).abs() < 1e-6);
        // After the decay window: back to the plain typewriter curve.
        let decayed = content_reveal(
            MsgFillStyle::Pulse,
            0,
            1,
            Some(PULSE_DECAY_MS + typewriter::TYPEWRITER_FADE_MS),
            10,
            1.0,
        );
        assert!((decayed.factor - 1.0).abs() < 1e-6);
    }

    #[test]
    fn pulse_hidden_cells_stay_hidden() {
        // The scanner boost must never resurrect an unrevealed cell.
        let r = content_reveal(MsgFillStyle::Pulse, 9, 1, Some(0), 3, 1.0);
        assert!(!r.visible);
    }
}
