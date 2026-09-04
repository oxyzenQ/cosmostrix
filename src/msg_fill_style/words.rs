// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! msg-fill-style `words` — word-by-word reveal with land impact.
//!
//! Word ordinal 1 reveals at t=0, word N at `(N-1) * WORDS_PER_WORD_MS`
//! (200 ms), each word fading in over `WORDS_FADE_MS` (150 ms) from
//! 30% to 100% brightness. Padding spaces (ordinal 0) are always
//! visible immediately — they are invisible cells and must never
//! delay the reveal.
//!
//! ## Word land impact (post-cascade improvement)
//!
//! The original words style was just a per-word fade-in — visually
//! nearly indistinguishable from `fade` (owner feedback: "mirip
//! mirip/duplicate"). This improvement adds a land impact flash:
//! the moment a word's fade-in completes (age >= `WORDS_FADE_MS`),
//! the word "lands" with a brightness boost to `1.0 + WORDS_LAND_BOOST`
//! (1.3x), decaying back to 1.0 over `WORDS_LAND_MS` (80 ms). This
//! reads as "word appears, then punches in" — distinct from plain
//! `fade` (no punch) and from `pulse` (per-word, not per-char). The
//! boost goes through the renderer's unclamped brightness path
//! (factors > 1.0 allowed; per-channel clamp at 255 downstream).
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
/// Word land impact boost — factor peaks at `1.0 + WORDS_LAND_BOOST`
/// (1.3x) the moment the fade-in completes, decays to 1.0 over
/// `WORDS_LAND_MS`. Makes words "punch in" — distinct from plain
/// `fade`.
pub(crate) const WORDS_LAND_BOOST: f32 = 0.30;
/// Word land impact decay window.
pub(crate) const WORDS_LAND_MS: usize = 80;

/// Per-cell reveal from the cell's 1-based word ordinal (0 = padding
/// space before the first word — always visible immediately).
pub(super) fn reveal(word_ord: u32, elapsed_ms: Option<usize>) -> CellReveal {
    let word_reveal_at = word_ord.saturating_sub(1) as usize * WORDS_PER_WORD_MS;
    match elapsed_ms {
        None => CellReveal::settled(),
        Some(ms) => {
            if word_ord == 0 || ms >= word_reveal_at {
                let fade = char_fade_in(elapsed_ms, word_reveal_at, WORDS_FADE_MS);
                // Word land impact: after the fade-in completes, boost
                // to (1 + WORDS_LAND_BOOST) and decay to 1.0 over
                // WORDS_LAND_MS. The boost multiplies the settled fade
                // (1.0 at age >= WORDS_FADE_MS), so during the fade-in
                // the boost is 1.0 (no double-boost).
                let land_boost = if fade >= 1.0 {
                    let land_age = ms.saturating_sub(word_reveal_at + WORDS_FADE_MS);
                    if land_age >= WORDS_LAND_MS {
                        1.0
                    } else {
                        let decay = 1.0 - land_age as f32 / WORDS_LAND_MS as f32;
                        1.0 + WORDS_LAND_BOOST * decay
                    }
                } else {
                    1.0
                };
                CellReveal {
                    visible: true,
                    factor: fade * land_boost,
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
        // Word fade-in completes after WORDS_FADE_MS — but the land
        // impact kicks in at that moment (factor 1.3). The word is
        // fully settled (factor 1.0) only after the land impact decays
        // (WORDS_FADE_MS + WORDS_LAND_MS).
        let word1_settled = content_reveal(
            MsgFillStyle::Words,
            0,
            1,
            Some(WORDS_FADE_MS + WORDS_LAND_MS),
            0,
            1.0,
        );
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

    #[test]
    fn words_land_impact_boosts_after_fade_in() {
        // Word 1 (ord=1) reveals at t=0. At age = WORDS_FADE_MS
        // (fade-in complete), the land impact kicks in: factor
        // = 1.0 * (1 + WORDS_LAND_BOOST) = 1.3.
        let landed = content_reveal(MsgFillStyle::Words, 0, 1, Some(WORDS_FADE_MS), 0, 1.0);
        assert!(landed.visible);
        assert!(
            (landed.factor - (1.0 + WORDS_LAND_BOOST)).abs() < 1e-6,
            "factor at land moment must be 1.0 + WORDS_LAND_BOOST (1.3), got {}",
            landed.factor
        );
        // Mid-land (age WORDS_FADE_MS + WORDS_LAND_MS/2): factor
        // decays toward 1.0.
        let mid = content_reveal(
            MsgFillStyle::Words,
            0,
            1,
            Some(WORDS_FADE_MS + WORDS_LAND_MS / 2),
            0,
            1.0,
        );
        assert!(mid.visible);
        assert!(
            mid.factor > 1.0 && mid.factor < 1.0 + WORDS_LAND_BOOST,
            "mid-land factor {} must be between 1.0 and {} (decaying)",
            mid.factor,
            1.0 + WORDS_LAND_BOOST
        );
    }

    #[test]
    fn words_settles_after_land_impact() {
        // At age >= WORDS_FADE_MS + WORDS_LAND_MS: factor = 1.0 (settled).
        let settled = content_reveal(
            MsgFillStyle::Words,
            0,
            1,
            Some(WORDS_FADE_MS + WORDS_LAND_MS),
            0,
            1.0,
        );
        assert!(settled.visible);
        assert!(
            (settled.factor - 1.0).abs() < 1e-6,
            "factor must be 1.0 after land impact decays"
        );
    }

    #[test]
    fn words_no_land_boost_during_fade_in() {
        // During fade-in (age < WORDS_FADE_MS), the land boost must
        // NOT fire — only the fade factor applies. This prevents a
        // double-boost (fade is dim, land is bright — both at once
        // would be muddy).
        let mid_fade = content_reveal(MsgFillStyle::Words, 0, 1, Some(WORDS_FADE_MS / 2), 0, 1.0);
        assert!(mid_fade.visible);
        // Fade-in at 50%: factor = 0.30 + (1.0 - 0.30) * 0.5 = 0.65.
        // No land boost (fade < 1.0).
        assert!(
            (mid_fade.factor - 0.65).abs() < 1e-6,
            "mid-fade factor must be 0.65 (no land boost during fade-in), got {}",
            mid_fade.factor
        );
    }

    #[test]
    fn words_constants_hold_research_doc_contract() {
        // Lock the values so a future tuning round can't drift them.
        assert_eq!(WORDS_PER_WORD_MS, 200);
        assert_eq!(WORDS_FADE_MS, 150);
        assert!((WORDS_LAND_BOOST - 0.30).abs() < 1e-6);
        assert_eq!(WORDS_LAND_MS, 80);
    }
}
