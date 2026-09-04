// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! msg-fill-style `cascade` — per-column waterfall reveal with
//! drop-from-above.
//!
//! The fourth (and final) candidate from the post-engrave expansion
//! family (see `docs/research/MSG_FILL_STYLE_EXPANSION_RESEARCH.md`
//! §3.D). The research doc originally flagged cascade as "defer
//! until multi-line overlays are common" because on a 1-line
//! overlay (the default), a per-column "drop top-to-bottom"
//! degenerates into a fast left-to-right wipe nearly
//! indistinguishable from typewriter. This implementation solves
//! that by adding a drop-from-above animation that is visible
//! even on a 1-line overlay: each char appears N rows ABOVE its
//! final position and drops down to land, column-paced (not
//! per-char-paced). The visual reads as "water falling from above,
//! column by column" — distinct from typewriter (no drop), slide
//! (drops from BELOW, per-char-paced), and every other shipped
//! style.
//!
//! ## Reveal math (stateless)
//!
//! Each content cell has a column-based reveal time:
//! `reveal_at = col_offset * CASCADE_COL_MS`. The `col_offset` is
//! the cell's horizontal column index relative to the box's left
//! edge (so column 0 reveals first, then column 1, etc.). With
//! `CASCADE_COL_MS` = 60 ms, a 20-char overlay takes 1.2 s to
//! fully reveal (vs typewriter's 80 ms × 20 = 1.6 s — cascade is
//! slightly faster, matching the "waterfall" feel).
//!
//! After `reveal_at`, the cell enters a drop phase over
//! `CASCADE_DROP_MS` (240 ms): the glyph starts `CASCADE_DROP_ROWS`
//! (3) rows above the final position with a dim factor
//! (`CASCADE_DROP_DIM` = 0.40), then slides down to the final
//! position while fading in to full brightness. The drop uses the
//! shared `slide_rows` field (negative = above, see `mod.rs`) so the
//! existing slide deferred-second-pass mechanism handles the
//! rendering — zero renderer churn beyond the signed-`slide_rows`
//! support added in this round.
//!
//! Without a timeline (`elapsed_ms = None`), every cell settles
//! instantly (`slide_rows = 0`, factor = 1.0) — same `usize::MAX`
//! reveal_count semantics every stateless style uses for bench and
//! edge paths.
//!
//! ## Why column-paced, not per-char-paced
//!
//! The research doc spec is "columns light up left-to-right". On a
//! 1-line overlay, each column IS one char, so column-paced = per-
//! char-paced — but the drop-from-above animation is what
//! makes cascade distinct from typewriter (which just fades in
//! each char in place). The drop is visible on any overlay height:
//! the glyph is painted 3 rows above, then the next frame paints it
//! 2 rows above, then 1, then landed. On a 1-line overlay, the
//! glyph appears to "fall" from outside the box (above the top
//! border) into its final position.
//!
//! ## --no-effects contract
//!
//! Cascade has NO particle sidecar — the drop animation IS the
//! reveal math, not a cosmetic overlay. So `--no-effects` does NOT
//! gate anything in this style (same contract as glitch).
//!
//! Border: lags text with the shared `t^1.5` ease-out curve.

use super::{index_fraction, index_pacing, lagged_border, CellReveal};

// ── Reveal math constants (stateless) ───────────────────────────────────────

/// Per-column reveal stagger. 60 ms = slightly faster than
/// typewriter's 80 ms/char, matching the "waterfall" feel (water
/// falls fast). On a 1-line overlay, column = char, so this IS the
/// per-char pacing. On multi-line, each column reveals all its rows
/// at the same instant (the drop animation is per-cell, see below).
pub(crate) const CASCADE_COL_MS: usize = 60;

/// Drop animation duration. 240 ms = ~4 frames at 60 FPS, enough
/// to read as a "fall" without being choppy. The glyph starts
/// `CASCADE_DROP_ROWS` rows above and slides down over this window.
pub(crate) const CASCADE_DROP_MS: usize = 240;

/// Drop starting height (rows above the final position). 3 = the
/// glyph appears 3 rows above its final cell, then drops down. On
/// a 1-line overlay (box height 3: 1 border + 1 content + 1
/// border), 3 rows above the content row is ABOVE the top border —
/// the glyph appears to fall from outside the box.
pub(crate) const CASCADE_DROP_ROWS: i16 = 3;

/// Drop starting brightness (dim). 0.40 = the glyph starts at 40%
/// brightness and ramps to 100% as it lands. Lower than slide's
/// 0.70 because the drop is from FURTHER away (3 rows vs 1) — the
/// "far away, dim" depth cue.
pub(crate) const CASCADE_DROP_DIM: f32 = 0.40;

// ── Reveal math (stateless) ────────────────────────────────────────────────

/// Per-cell reveal: column-paced reveal + drop-from-above.
///
/// Pure function of `(content_idx, elapsed_ms, reveal_count)` — no
/// per-frame state, no per-cell bookkeeping in `Cloud`. The
/// `content_idx` IS the column offset on a 1-line overlay (the
/// research doc's "columns light up left-to-right" maps to "chars
/// reveal left-to-right" on 1-line, but the drop animation makes
/// it distinct from typewriter).
pub(super) fn reveal(
    content_idx: usize,
    elapsed_ms: Option<usize>,
    reveal_count: usize,
) -> CellReveal {
    if content_idx >= reveal_count {
        return CellReveal::hidden();
    }
    let reveal_at = content_idx * CASCADE_COL_MS;
    let Some(ms) = elapsed_ms else {
        // No timeline (bench/edge): settled immediately.
        return CellReveal::settled();
    };
    if ms < reveal_at {
        // Before this column's reveal time: hidden.
        return CellReveal::hidden();
    }
    let age = ms - reveal_at;
    if age >= CASCADE_DROP_MS {
        // Drop complete: landed at full brightness.
        CellReveal::settled()
    } else {
        // Drop phase: glyph is above the final position, sliding
        // down + fading in. `slide_rows` is negative (above) and
        // ramps from -CASCADE_DROP_ROWS (start) to 0 (landed).
        let progress = age as f32 / CASCADE_DROP_MS as f32;
        let rows_above = CASCADE_DROP_ROWS as f32 * (1.0 - progress);
        // Round to nearest i16: at progress=0, rows_above=3 → -3;
        // at progress=1, rows_above=0 → 0 (landed, handled above).
        let slide_rows = -(rows_above.round() as i16);
        let factor = CASCADE_DROP_DIM + (1.0 - CASCADE_DROP_DIM) * progress;
        CellReveal {
            visible: true,
            factor,
            slide_rows,
            glyph_override: None,
            tint: None,
        }
    }
}

/// Index budget: 60 ms/column with the pre-v80.0.0-beta.1 `.max(1)` floor.
pub(super) fn reveal_budget(elapsed_ms: Option<usize>, total_text: usize) -> usize {
    index_pacing(CASCADE_COL_MS, elapsed_ms, total_text)
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
    fn cascade_reveals_at_60ms_per_column() {
        // 60 ms/column: 179 ms → 2 columns, 180 ms → 3, 0 ms → 1
        // (max(1) floor). Faster than typewriter's 80 ms/char.
        let total = 40;
        let count = index_reveal_count(MsgFillStyle::Cascade, Some(179), total);
        assert_eq!(count, 2);
        let count = index_reveal_count(MsgFillStyle::Cascade, Some(180), total);
        assert_eq!(count, 3);
        let count = index_reveal_count(MsgFillStyle::Cascade, Some(0), total);
        assert_eq!(count, 1, "max(1) floor: first column at t=0");
    }

    #[test]
    fn cascade_settles_after_drop_window() {
        // At age >= CASCADE_DROP_MS (240 ms): slide_rows = 0,
        // factor = 1.0 (settled).
        let r = content_reveal(MsgFillStyle::Cascade, 0, 1, Some(CASCADE_DROP_MS), 10, 1.0);
        assert!(r.visible);
        assert_eq!(r.slide_rows, 0, "slide_rows must be 0 after drop");
        assert!(
            (r.factor - 1.0).abs() < 1e-6,
            "factor must be 1.0 after drop"
        );
        assert!(r.tint.is_none());
        assert!(r.glyph_override.is_none());
    }

    #[test]
    fn cascade_settles_without_timeline() {
        // No timeline (bench/edge): settled immediately.
        let r = content_reveal(MsgFillStyle::Cascade, 0, 1, None, 10, 1.0);
        assert!(r.visible);
        assert_eq!(r.slide_rows, 0);
        assert!((r.factor - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cascade_drop_starts_above_at_dim_brightness() {
        // At age 0 (just revealed): slide_rows = -CASCADE_DROP_ROWS
        // (3 rows above), factor = CASCADE_DROP_DIM (0.40).
        let r = content_reveal(MsgFillStyle::Cascade, 0, 1, Some(0), 10, 1.0);
        assert!(r.visible);
        assert_eq!(
            r.slide_rows, -CASCADE_DROP_ROWS,
            "slide_rows must be -CASCADE_DROP_ROWS at drop start"
        );
        assert!(
            (r.factor - CASCADE_DROP_DIM).abs() < 1e-6,
            "factor must be CASCADE_DROP_DIM at drop start"
        );
    }

    #[test]
    fn cascade_drop_progresses_downward_and_brightens() {
        // At age CASCADE_DROP_MS/2 (120 ms, mid-drop): slide_rows
        // should be between -CASCADE_DROP_ROWS and 0 (partway),
        // factor between CASCADE_DROP_DIM and 1.0.
        let mid = content_reveal(
            MsgFillStyle::Cascade,
            0,
            1,
            Some(CASCADE_DROP_MS / 2),
            10,
            1.0,
        );
        assert!(mid.visible);
        assert!(
            mid.slide_rows > -CASCADE_DROP_ROWS && mid.slide_rows <= 0,
            "mid-drop slide_rows {} must be between -{} and 0",
            mid.slide_rows,
            CASCADE_DROP_ROWS
        );
        assert!(
            mid.factor > CASCADE_DROP_DIM && mid.factor < 1.0,
            "mid-drop factor {} must be between {} and 1.0",
            mid.factor,
            CASCADE_DROP_DIM
        );
    }

    #[test]
    fn cascade_hidden_until_reveal_time() {
        // Column 5 reveals at 5 * 60 = 300 ms. At 299 ms, hidden.
        // At 300 ms, visible (drop start).
        let r = content_reveal(MsgFillStyle::Cascade, 5, 1, Some(299), 10, 1.0);
        assert!(!r.visible, "column 5 must be hidden at 299ms");
        let r = content_reveal(MsgFillStyle::Cascade, 5, 1, Some(300), 10, 1.0);
        assert!(r.visible, "column 5 must be visible at 300ms (reveal_at)");
    }

    #[test]
    fn cascade_hidden_outside_reveal_budget() {
        // Cells beyond reveal_count are always hidden.
        let r = content_reveal(MsgFillStyle::Cascade, 10, 1, Some(60_000), 5, 1.0);
        assert!(!r.visible, "cell 10 must be hidden when reveal_count == 5");
    }

    #[test]
    fn cascade_slide_rows_always_non_positive() {
        // The drop direction is always "from above" — slide_rows
        // must never be positive (that's the slide style's
        // "from below" direction).
        for ms in 0..CASCADE_DROP_MS {
            let r = content_reveal(MsgFillStyle::Cascade, 0, 1, Some(ms), 10, 1.0);
            assert!(r.visible);
            assert!(
                r.slide_rows <= 0,
                "cascade slide_rows {} at ms={} must be <= 0 (drop from above)",
                r.slide_rows,
                ms
            );
        }
    }

    #[test]
    fn cascade_constants_hold_research_doc_contract() {
        // Lock the values called out in
        // MSG_FILL_STYLE_EXPANSION_RESEARCH.md §3.D + the
        // drop-from-above tuning so a future round can't drift
        // them silently.
        assert_eq!(CASCADE_COL_MS, 60);
        assert_eq!(CASCADE_DROP_MS, 240);
        assert_eq!(CASCADE_DROP_ROWS, 3);
        assert!((CASCADE_DROP_DIM - 0.40).abs() < 1e-6);
    }
}
