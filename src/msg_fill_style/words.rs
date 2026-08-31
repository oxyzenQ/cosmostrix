// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! msg-fill-style `words` — word-by-word reveal.
//!
//! Word ordinal 1 reveals at t=0, word N at `(N-1) * WORDS_PER_WORD_MS`
//! (200 ms), each word fading in over `WORDS_FADE_MS` (150 ms) from
//! 30% to 100% brightness. Padding spaces (ordinal 0) are always
//! visible immediately — they are invisible cells and must never
//! delay the reveal.
//!
//! Word ordinals are hoisted once per message in `reset_message`
//! (`message_word_ordinals`, Z-5 zero-alloc) — this module only reads
//! them.
//!
//! Border: lags word progress with the shared `t^1.5` ease-out curve.

use super::{char_fade_in, lagged_border, CellReveal};

/// Per-word reveal stagger.
pub(crate) const WORDS_PER_WORD_MS: usize = 200;
/// Per-word fade-in duration.
pub(crate) const WORDS_FADE_MS: usize = 150;

/// Per-cell reveal from the cell's 1-based word ordinal (0 = padding
/// space before the first word — always visible immediately).
pub(super) fn reveal(word_ord: u32, elapsed_ms: Option<usize>) -> CellReveal {
    let word_reveal_at = word_ord.saturating_sub(1) as usize * WORDS_PER_WORD_MS;
    match elapsed_ms {
        None => CellReveal::settled(),
        Some(ms) => {
            if word_ord == 0 || ms >= word_reveal_at {
                CellReveal {
                    visible: true,
                    factor: char_fade_in(elapsed_ms, word_reveal_at, WORDS_FADE_MS),
                    slide_rows: 0,
                    glyph_override: None,
                    tint: None,
                }
            } else {
                CellReveal::hidden()
            }
        }
    }
}

/// Index budget: word-ordinal visibility decides per-cell — the index
/// budget is effectively "all revealed" (dead value for this style).
pub(super) fn reveal_budget(elapsed_ms: Option<usize>, total_text: usize) -> usize {
    match elapsed_ms {
        None => usize::MAX,
        Some(_) => total_text.max(1),
    }
}

/// Border lags word progress (t^1.5) — same curve as the index-paced
/// styles, driven by the word fraction instead.
pub(super) fn border_progress(text_progress: f32) -> f32 {
    lagged_border(text_progress)
}

/// Text progress: revealed-word fraction, derived from elapsed time
/// (word ordinals only tell which word a cell belongs to, not how far
/// the reveal has progressed).
pub(super) fn text_progress(total_words: usize, elapsed_ms: Option<usize>) -> f32 {
    let total_words = total_words.max(1);
    match elapsed_ms {
        None => 1.0,
        Some(ms) => {
            let revealed_words = (ms / WORDS_PER_WORD_MS + 1).min(total_words);
            revealed_words as f32 / total_words as f32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{content_reveal, text_progress, MsgFillStyle};
    use super::*;

    #[test]
    fn words_reveal_word_by_word_at_200ms() {
        // Word 1 (ord=1) reveals at t=0; word 2 (ord=2) at t=200.
        let word1 = content_reveal(MsgFillStyle::Words, 0, 1, Some(0), 0, 1.0);
        assert!(word1.visible);
        let word2_early = content_reveal(MsgFillStyle::Words, 3, 2, Some(199), 0, 1.0);
        assert!(!word2_early.visible);
        let word2_on_time = content_reveal(MsgFillStyle::Words, 3, 2, Some(200), 0, 1.0);
        assert!(word2_on_time.visible);
        // Word fade-in completes after WORDS_FADE_MS.
        let word1_settled = content_reveal(MsgFillStyle::Words, 0, 1, Some(WORDS_FADE_MS), 0, 1.0);
        assert!((word1_settled.factor - 1.0).abs() < 1e-6);
    }

    #[test]
    fn words_padding_spaces_reveal_immediately() {
        // ord=0 (padding before the first word) must never delay the
        // (invisible) padding cells.
        let r = content_reveal(MsgFillStyle::Words, 0, 0, Some(0), 0, 1.0);
        assert!(r.visible);
    }

    #[test]
    fn text_progress_words_uses_word_fraction() {
        // 3 words: at t=0 one word is (about to be) visible → 1/3.
        let tp = text_progress(MsgFillStyle::Words, 0, 9, 3, Some(0));
        assert!((tp - 1.0 / 3.0).abs() < 1e-6);
        // At t=199ms still 1/3; at t=200ms 2/3.
        let tp = text_progress(MsgFillStyle::Words, 0, 9, 3, Some(199));
        assert!((tp - 1.0 / 3.0).abs() < 1e-6);
        let tp = text_progress(MsgFillStyle::Words, 0, 9, 3, Some(200));
        assert!((tp - 2.0 / 3.0).abs() < 1e-6);
        // No timeline → complete.
        let tp = text_progress(MsgFillStyle::Words, 0, 9, 3, None);
        assert!((tp - 1.0).abs() < 1e-6);
    }
}
