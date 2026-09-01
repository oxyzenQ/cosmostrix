// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! msg-fill-style `glitch` — cyberpunk distortion settle.
//!
//! The second candidate from the post-engrave expansion family
//! (see `docs/research/MSG_FILL_STYLE_EXPANSION_RESEARCH.md` §3.B):
//! characters do NOT appear left-to-right. Each char's reveal time
//! is a deterministic scramble (hash of its index), and each newly
//! revealed char flickers between 2-3 wrong glyphs for ~90 ms
//! before settling on the true one (Matrix-decode feel).
//!
//! Fully stateless — no sidecar, no per-frame bookkeeping. The ONE
//! structural extension this style needs is `CellReveal.glyph_override`,
//! a `Option<char>` field added in this round as the permanent API
//! surface shared by every future glyph-substituting style (see
//! the research doc §2 ground rule). Every existing style leaves
//! the field `None`, so they are bit-identical to before.
//!
//! ## Reveal math (stateless)
//!
//! Each content cell has TWO gates:
//!
//! 1. **Budget gate** (`reveal_count` from `index_reveal_count`):
//!    `content_idx < reveal_count`. This keeps the reveal pacing
//!    roughly typewriter-speed (one cell eligible every 80 ms) and
//!    matches the pre-v80.0.0-beta.1 `usize::MAX` semantics for `None` timeline.
//! 2. **Scramble gate** (per-cell reveal time): `elapsed_ms >=
//!    reveal_at(content_idx)` where
//!    `reveal_at = content_idx * GLITCH_CHAR_MS + scramble_offset(content_idx) * GLITCH_SCRAMBLE_MS`.
//!    The scramble offset is a deterministic hash of the content
//!    index, picked from `0..GLITCH_SCRAMBLE_SPREAD`. With spread 8
//!    and step 80 ms, the scramble window spans up to 560 ms — wide
//!    enough to break the strict left-to-right order, narrow enough
//!    that the reveal still feels paced (not all-at-once).
//!
//! Within the budget but before the scramble gate, the cell is
//! hidden. After the scramble gate, the cell enters the **settle
//! window** (90 ms): the glyph is one of `GLITCH_WRONG_GLYPHS`
//! (picked deterministically by `hash(content_idx, bucket)`), and
//! the brightness modulates in `1.0 ± GLITCH_FLICKER_AMPLITUDE`
//! (±20%). After the settle window, the cell shows the true glyph
//! (`glyph_override = None`) at factor 1.0.
//!
//! Without a timeline (`elapsed_ms = None`), every cell settles
//! instantly (factor 1.0, correct glyph) — same `usize::MAX`
//! reveal_count semantics every stateless style uses for bench
//! and edge paths.
//!
//! ## Wrong-glyph table
//!
//! 8 ASCII printable graphic chars (`['0', '1', '#', '%', '&',
//! '$', '@', '?']`) — all single-width, all in the safe ASCII
//! printable range. The substitution never breaks cell alignment
//! (Bug #11) and never introduces wide CJK chars.
//!
//! ## --no-effects contract
//!
//! Glitch has NO particle sidecar — the glyph substitution IS the
//! reveal math, not a cosmetic overlay. So `--no-effects` does NOT
//! gate anything in this style. (The hologram scanline pass, by
//! contrast, is a cosmetic overlay on top of the reveal math, so
//! it self-gates on `effects_enabled` — same contract as every
//! particle subsystem.)
//!
//! Border: lags text with the shared `t^1.5` ease-out curve.

use super::{index_fraction, index_pacing, lagged_border, CellReveal};

// ── Reveal math constants ───────────────────────────────────────────────────

/// Per-character base reveal pacing (same 80 ms as typewriter/engrave/
/// hologram). Cells are eligible for reveal every 80 ms, but each
/// cell's actual reveal time is scrambled within a ±SPREAD window
/// (see GLITCH_SCRAMBLE_SPREAD / GLITCH_SCRAMBLE_MS).
pub(crate) const GLITCH_CHAR_MS: usize = 80;

/// Scramble spread: each cell's reveal time is offset by
/// `(hash(content_idx) % GLITCH_SCRAMBLE_SPREAD) * GLITCH_SCRAMBLE_MS`.
/// 8 = the cell can reveal anywhere within an 8-step window around
/// its base pacing slot — wide enough to break the strict
/// left-to-right order, narrow enough that the reveal still feels
/// paced (not all-at-once).
pub(crate) const GLITCH_SCRAMBLE_SPREAD: usize = 8;

/// Per scramble-offset step. 80 ms = same as GLITCH_CHAR_MS, so the
/// scramble window spans up to `(SPREAD-1) * 80 = 560 ms` — a cell
/// can reveal up to 560 ms AFTER its base pacing slot, or right on
/// it. The budget gate (`reveal_count`) still caps eligibility at
/// `elapsed / 80`, so the scramble only reshuffles cells within
/// the budget window.
pub(crate) const GLITCH_SCRAMBLE_MS: usize = 80;

/// Settle window after reveal: the cell flickers between wrong
/// glyphs for 90 ms before settling on the true one. 90 ms =
/// ~5-6 frames at 60 FPS, enough to read as "matrix decode"
/// without being choppy.
pub(crate) const GLITCH_SETTLE_MS: usize = 90;

/// Settle flicker bucket size: one distinct wrong-glyph pick per
/// 30 ms. 3 buckets during the 90 ms settle = 2-3 distinct wrong
/// glyphs before settling.
pub(crate) const GLITCH_SETTLE_BUCKET_MS: usize = 30;

/// Settle flicker brightness amplitude: factor modulates in
/// `1.0 ± 0.20` (80%..120%) — subtler than hologram's ±30% since
/// the wrong-glyph substitution is already the dominant visual.
pub(crate) const GLITCH_FLICKER_AMPLITUDE: f32 = 0.20;

/// Wrong-glyph table: 8 ASCII graphic glyphs that read as "matrix
/// decode" noise. All single-width, all in the safe ASCII printable
/// range, so the substitution never breaks cell alignment (Bug #11)
/// and never introduces wide CJK chars.
pub(crate) const GLITCH_WRONG_GLYPHS: [char; 8] = ['0', '1', '#', '%', '&', '$', '@', '?'];

// ── Reveal math (stateless) ────────────────────────────────────────────────

/// Per-cell reveal: budget gate + scramble gate + settle window with
/// wrong-glyph substitution.
///
/// Pure function of `(content_idx, elapsed_ms, reveal_count)` — no
/// per-frame state, no per-cell bookkeeping in `Cloud`. The
/// `CellReveal.slide_rows` field is always 0 (glitch cells do not
/// move — the slide style owns that channel).
pub(super) fn reveal(
    content_idx: usize,
    elapsed_ms: Option<usize>,
    reveal_count: usize,
) -> CellReveal {
    // Budget gate: the reveal_count budget (from index_pacing)
    // caps how many cells COULD be revealed. Within that budget,
    // each cell's own scramble offset decides its actual reveal
    // time. This keeps the reveal pacing roughly typewriter-speed
    // (one cell every 80 ms) while breaking the strict left-to-right
    // order — the cyberpunk "characters appear out of order" feel.
    if content_idx >= reveal_count {
        return CellReveal::hidden();
    }
    let reveal_at =
        content_idx * GLITCH_CHAR_MS + (scramble_offset(content_idx) as usize) * GLITCH_SCRAMBLE_MS;
    let Some(ms) = elapsed_ms else {
        // No timeline (bench/edge): settled immediately.
        return CellReveal::settled();
    };
    if ms < reveal_at {
        // Cell is within the reveal_count budget but its scramble
        // offset puts the reveal in the future — still hidden. The
        // scramble gate reshuffles the reveal order within the
        // budget window.
        return CellReveal::hidden();
    }
    let age = ms - reveal_at;
    if age < GLITCH_SETTLE_MS {
        // Settle phase: wrong glyph + flicker brightness.
        let bucket = age / GLITCH_SETTLE_BUCKET_MS;
        let wrong = wrong_glyph(content_idx, bucket);
        let noise = flicker_noise(content_idx, bucket);
        CellReveal {
            visible: true,
            factor: 1.0 + noise * GLITCH_FLICKER_AMPLITUDE,
            slide_rows: 0,
            glyph_override: Some(wrong),
            tint: None,
        }
    } else {
        // Settled: correct glyph, factor 1.0.
        CellReveal::settled()
    }
}

/// Deterministic scramble offset for a content cell (0..SPREAD-1).
#[inline]
fn scramble_offset(content_idx: usize) -> u32 {
    glitch_hash(content_idx, 0x1234_5678) % GLITCH_SCRAMBLE_SPREAD as u32
}

/// Deterministic wrong-glyph pick during the settle window.
#[inline]
fn wrong_glyph(content_idx: usize, bucket: usize) -> char {
    let idx = glitch_hash(content_idx, bucket as u32 + 1) % GLITCH_WRONG_GLYPHS.len() as u32;
    GLITCH_WRONG_GLYPHS[idx as usize]
}

/// Deterministic flicker noise in `[-1.0, 1.0)` for the settle
/// brightness modulation. Same shape as hologram's flicker noise
/// but with a different seed so the two styles never correlate.
#[inline]
fn flicker_noise(content_idx: usize, bucket: usize) -> f32 {
    let h = glitch_hash(content_idx, bucket as u32 + 0xBEEF_FACE);
    // Map u32 → [0, 1) using the high 24 bits (mantissa-sized —
    // lower bits of a multiply-xorshift hash are lower quality).
    let bits = h >> 8;
    let unit = (bits as f32) / ((1u32 << 24) as f32);
    // Center to [-1.0, 1.0).
    unit * 2.0 - 1.0
}

/// 32-bit multiply-xorshift hash with a per-call seed. Same shape
/// as hologram's hash but the seed param ensures the scramble
/// offset, wrong-glyph pick, and flicker noise never correlate
/// (a cell whose scramble offset is 0 is no more likely to land
/// on wrong-glyph index 0 or flicker value 0 than any other cell).
#[inline]
fn glitch_hash(content_idx: usize, seed: u32) -> u32 {
    let mut h: u32 = (content_idx as u32).wrapping_mul(0x9E37_79B1);
    h = h.wrapping_add(seed);
    h = h.wrapping_mul(0x27D4_EB2F).rotate_left(11);
    h ^= h.rotate_right(7);
    h
}

/// Index budget: 80 ms/char with the pre-v80.0.0-beta.1 `.max(1)` floor.
pub(super) fn reveal_budget(elapsed_ms: Option<usize>, total_text: usize) -> usize {
    index_pacing(GLITCH_CHAR_MS, elapsed_ms, total_text)
}

/// Border lags text (t^1.5) — the pre-v80.0.0-beta.1 cinematic curve.
pub(super) fn border_progress(text_progress: f32) -> f32 {
    lagged_border(text_progress)
}

/// Text progress: revealed-cell fraction.
pub(super) fn text_progress(reveal_count: usize, total_text: usize) -> f32 {
    index_fraction(reveal_count, total_text)
}

#[cfg(test)]
mod tests {
    use super::super::{content_reveal, index_reveal_count, MsgFillStyle};
    use super::*;

    #[test]
    fn glitch_reveals_at_80ms_per_char_budget() {
        // Same index budget as typewriter/engrave/hologram: 319 ms → 3,
        // 320 ms → 4. The scramble offset only affects per-cell
        // reveal_at, not the budget.
        let total = 40;
        let count = index_reveal_count(MsgFillStyle::Glitch, Some(319), total);
        assert_eq!(count, 3);
        let count = index_reveal_count(MsgFillStyle::Glitch, Some(320), total);
        assert_eq!(count, 4);
        let count = index_reveal_count(MsgFillStyle::Glitch, Some(0), total);
        assert_eq!(count, 1, "max(1) floor: first char at t=0");
    }

    #[test]
    fn glitch_chars_settle_to_correct_glyph_after_settle_window() {
        // At large elapsed, all cells are settled: correct glyph
        // (glyph_override = None) and factor = 1.0. This is the
        // "Matrix decode complete" state.
        for idx in 0..10 {
            let r = content_reveal(MsgFillStyle::Glitch, idx, 1, Some(60_000), 10, 1.0);
            assert!(r.visible, "cell {idx} must be visible at 60s");
            assert!(
                r.glyph_override.is_none(),
                "cell {idx} must have no glyph override after settle"
            );
            assert!(
                (r.factor - 1.0).abs() < 1e-6,
                "cell {idx} factor must be 1.0 after settle"
            );
        }
    }

    #[test]
    fn glitch_settles_without_timeline() {
        // No timeline (bench/edge): every cell settles immediately
        // via the settled() helper (visible, factor 1.0, no override).
        let r = content_reveal(MsgFillStyle::Glitch, 0, 1, None, 10, 1.0);
        assert!(r.visible);
        assert!(r.glyph_override.is_none());
        assert!((r.factor - 1.0).abs() < 1e-6);
    }

    #[test]
    fn glitch_glyph_override_some_during_settle_none_after() {
        // Pick a cell whose scramble_offset == 0 so its reveal_at
        // is exactly `content_idx * 80` (no extra scramble delay).
        // At age 30 ms (within settle), glyph_override must be
        // Some(wrong_glyph); at age >= GLITCH_SETTLE_MS, it must
        // be None. With 1/8 probability per cell, the search range
        // 0..200 is effectively guaranteed to find at least one
        // (probability of none ≈ (7/8)^200 ≈ 1.6e-12).
        let mut cell_with_zero_reveal = None;
        for idx in 0..200 {
            if scramble_offset(idx) == 0 {
                cell_with_zero_reveal = Some(idx);
                break;
            }
        }
        let idx = cell_with_zero_reveal
            .expect("at least one cell in 0..200 must have scramble_offset == 0");

        // Within settle window: wrong glyph + flicker. Use an
        // elapsed that puts this cell at age 30 ms (reveal_at +
        // 30). With scramble_offset == 0, reveal_at = idx * 80.
        let elapsed = idx * GLITCH_CHAR_MS + 30;
        let r = content_reveal(MsgFillStyle::Glitch, idx, 1, Some(elapsed), idx + 1, 1.0);
        assert!(r.visible, "cell {idx} must be visible at age 30ms");
        let wrong = r
            .glyph_override
            .expect("cell {idx} must have a wrong glyph during settle");
        assert!(
            GLITCH_WRONG_GLYPHS.contains(&wrong),
            "wrong glyph {wrong:?} must come from the GLITCH_WRONG_GLYPHS table"
        );
        // Factor must be within [1-AMP, 1+AMP].
        let lo = 1.0 - GLITCH_FLICKER_AMPLITUDE;
        let hi = 1.0 + GLITCH_FLICKER_AMPLITUDE;
        assert!(
            r.factor >= lo - 1e-6 && r.factor <= hi + 1e-6,
            "factor {} must stay within [{lo}, {hi}]",
            r.factor
        );

        // After settle: correct glyph + factor 1.0. Use an elapsed
        // that puts this cell at age > SETTLE_MS (reveal_at + SETTLE
        // + 1).
        let elapsed = idx * GLITCH_CHAR_MS + GLITCH_SETTLE_MS + 1;
        let r = content_reveal(MsgFillStyle::Glitch, idx, 1, Some(elapsed), idx + 1, 1.0);
        assert!(r.visible);
        assert!(
            r.glyph_override.is_none(),
            "cell {idx} must have no glyph override after settle"
        );
        assert!((r.factor - 1.0).abs() < 1e-6);
    }

    #[test]
    fn glitch_hidden_cells_outside_reveal_budget_stay_hidden() {
        // Cells beyond reveal_count are always hidden, regardless
        // of scramble offset or elapsed time.
        let r = content_reveal(MsgFillStyle::Glitch, 10, 1, Some(60_000), 5, 1.0);
        assert!(!r.visible, "cell 10 must be hidden when reveal_count == 5");
    }

    #[test]
    fn glitch_hidden_when_scramble_offset_pushes_reveal_into_future() {
        // Cell 0 with scramble_offset(0) > 0 has reveal_at > 0. At
        // small elapsed, it stays hidden even though reveal_count
        // includes it. This is the "scramble gate" — the visual
        // out-of-order reveal.
        if scramble_offset(0) > 0 {
            let r = content_reveal(MsgFillStyle::Glitch, 0, 1, Some(10), 5, 1.0);
            assert!(
                !r.visible,
                "cell 0 with non-zero scramble offset must be hidden at 10ms (reveal_at > 10)"
            );
        }
        // If scramble_offset(0) == 0, cell 0 reveals at t=0 and is
        // visible at 10ms — not a failure, just a different scramble
        // outcome. The hash should scatter, so most likely this
        // branch fires.
    }

    #[test]
    fn glitch_wrong_glyphs_are_all_single_width_ascii() {
        // The wrong-glyph table must contain only single-width
        // ASCII printable graphic chars to avoid breaking cell
        // alignment (Bug #11) and to stay terminal-safe across
        // all environments.
        for ch in GLITCH_WRONG_GLYPHS {
            assert!((ch as u32) < 0x80, "wrong glyph {ch:?} must be ASCII");
            assert!(
                char::is_ascii_graphic(&ch),
                "wrong glyph {ch:?} must be ASCII graphic"
            );
        }
    }

    #[test]
    fn glitch_hash_is_deterministic_and_seed_sensitive() {
        // Same input → same output (no rand dependency, bit-identical
        // frames at the same elapsed — LTS contract).
        assert_eq!(glitch_hash(3, 5), glitch_hash(3, 5));
        // Different seed → different output (the seed must matter,
        // otherwise scramble/wrong-glyph/flicker would correlate).
        assert_ne!(glitch_hash(3, 5), glitch_hash(3, 6));
        // Different content_idx → different output (hash should
        // scatter across cells, otherwise adjacent cells glitch in
        // lockstep).
        assert_ne!(glitch_hash(3, 5), glitch_hash(4, 5));
    }

    #[test]
    fn glitch_scramble_offset_stays_in_spread_range() {
        // Every content_idx must produce a scramble offset in
        // [0, GLITCH_SCRAMBLE_SPREAD). A wider range would break
        // the budget-vs-scramble invariant (cells beyond the
        // budget would never reveal).
        for idx in 0..100 {
            let off = scramble_offset(idx);
            assert!(
                off < GLITCH_SCRAMBLE_SPREAD as u32,
                "scramble_offset({idx}) = {off} must be < SPREAD ({})",
                GLITCH_SCRAMBLE_SPREAD
            );
        }
    }

    #[test]
    fn glitch_constants_hold_research_doc_contract() {
        // Lock the values called out in
        // MSG_FILL_STYLE_EXPANSION_RESEARCH.md so a future tuning
        // round can't drift them silently.
        assert_eq!(GLITCH_CHAR_MS, 80);
        assert_eq!(GLITCH_SCRAMBLE_SPREAD, 8);
        assert_eq!(GLITCH_SCRAMBLE_MS, 80);
        assert_eq!(GLITCH_SETTLE_MS, 90);
        assert_eq!(GLITCH_SETTLE_BUCKET_MS, 30);
        assert!((GLITCH_FLICKER_AMPLITUDE - 0.20).abs() < 1e-6);
        assert_eq!(GLITCH_WRONG_GLYPHS.len(), 8);
    }
}
