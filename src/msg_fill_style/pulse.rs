// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! msg-fill-style `pulse` — typewriter plus a visible scanner cursor.
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
//! ## Visible scanner cursor (post-cascade improvement)
//!
//! The original pulse was just a brightness boost — visually nearly
//! indistinguishable from typewriter (owner feedback: "mirip
//! mirip/duplicate"). This improvement adds a visible scanner
//! cursor glyph `▌` (U+258C LEFT ONE QUARTER BLOCK) painted ON TOP
//! of the most recently revealed char (the "scanner head"). The
//! cursor travels left-to-right along the text as it types, then
//! disappears when the reveal completes. This makes pulse read as
//! "a cursor scanning the text" — distinct from typewriter (no
//! cursor) and every other shipped style.
//!
//! The cursor is implemented as a stateless sidecar pass
//! `Cloud::pulse_cursor_pass` invoked at the END of `draw_message`
//! (alongside `engrave_spark_pass`, `scorch_smoke_pass`,
//! `hologram_scanline_pass` — only one is wired per style). The
//! cursor position is derived from `reveal_count - 1` (the most
//! recently revealed content cell) — no per-frame bookkeeping.
//! `--no-effects` (PERF-4) gates the cursor pass (same contract as
//! every particle/cosmetic subsystem).
//!
//! Border: lags text with the shared `t^1.5` ease-out curve.

use super::typewriter;
use super::{index_fraction, index_pacing, lagged_border, CellReveal};
use crate::cell::Cell;
use crate::frame::Frame;
use crate::runtime::ColorMode;

/// Scanner-cursor boost applied to recently revealed chars.
pub(crate) const PULSE_BOOST: f32 = 0.50;
/// Scanner-cursor decay window.
pub(crate) const PULSE_DECAY_MS: usize = 200;

/// Scanner cursor glyph — U+258C LEFT ONE QUARTER BLOCK. Painted
/// ON TOP of the most recently revealed char so the cursor reads
/// as a vertical bar scanning the text. Single-width ASCII-safe
/// (Unicode block element, single cell width — no alignment break).
pub(crate) const PULSE_CURSOR_GLYPH: char = '▌';

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

// ── Scanner cursor pass (stateless, called from draw_message) ────────────────

impl crate::cloud::Cloud {
    /// Pulse scanner cursor pass — one entry point, called at the
    /// END of `draw_message` (after the halo row, alongside the
    /// other style-specific passes — only one is wired per style).
    /// Stateless: the cursor position is derived from `reveal_count`
    /// and `head_pos` (both captured in the main draw loop).
    ///
    /// Paints `PULSE_CURSOR_GLYPH` (`▌`, U+258C LEFT ONE QUARTER
    /// BLOCK) ON TOP of the most recently revealed content cell —
    /// the cursor reads as a vertical bar scanning the text. The
    /// cursor is in the palette head color (bright on most schemes).
    /// It disappears when the reveal completes (head parks on the
    /// last char, but the cursor still shows there — the "scanner
    /// parked at end" look).
    ///
    /// `head_pos = None` or `elapsed_ms = None` (no animation
    /// timeline — bench/edge paths) skips the pass entirely.
    /// `--no-effects` suppresses the cursor (PERF-4 — same contract
    /// as every cosmetic subsystem).
    pub(crate) fn pulse_cursor_pass(
        &mut self,
        frame: &mut Frame,
        head_pos: Option<(u16, u16)>,
        elapsed_ms: Option<usize>,
    ) {
        let Some((col, line)) = head_pos else {
            return;
        };
        if elapsed_ms.is_none() {
            return;
        }
        if !self.effects_enabled {
            return;
        }
        let bg = self.palette.bg;
        let cursor_fg = if self.color_mode == ColorMode::Mono {
            None
        } else {
            // Palette head color (near-white on most schemes):
            // the cursor follows the active theme like the border
            // spark, engrave head, and hologram scanline do.
            self.palette.colors.last().copied()
        };
        frame.set_force(
            col,
            line,
            Cell {
                ch: PULSE_CURSOR_GLYPH,
                fg: cursor_fg,
                bg,
                bold: false,
            },
        );
    }
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

    #[test]
    fn pulse_cursor_glyph_is_single_width_block_element() {
        // PULSE_CURSOR_GLYPH must be U+258C (LEFT ONE QUARTER BLOCK)
        // — a single-cell-width block element that reads as a
        // vertical bar. Lock the exact codepoint so a future tuning
        // round can't drift it to a wide char (which would break
        // cell alignment, Bug #11).
        assert_eq!(PULSE_CURSOR_GLYPH, '▌');
        assert_eq!(PULSE_CURSOR_GLYPH as u32, 0x258C);
    }
}
