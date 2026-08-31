// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Render-level tests for the v51 msg-fill-style `glitch` style
//! (post-hologram follow-up — see
//! `docs/research/MSG_FILL_STYLE_EXPANSION_RESEARCH.md` §3.B).
//!
//! Split into its own file (mirroring `tests_msg_fill_hologram.rs`)
//! to keep `tests_msg_fill_style.rs` under the 800-LOC hard cap
//! (see `src/RULES_LOC.md`). The shared helpers
//! (`make_cloud_colored`, `set_message_elapsed`, `visible_content_cells`)
//! are imported from the parent msg-fill-style test file.
//!
//! The glitch style is fully stateless but extends `CellReveal`
//! with a `glyph_override: Option<char>` field — the ONE structural
//! extension point shared by every future glyph-substituting style.
//! The renderer unwraps to `mc.val` when `None`, so existing styles
//! are bit-identical to before. These tests mirror the engrave
//! acceptance ritual: pacing budget, settle window, glyph
//! substitution presence/absence, r-restart re-arm, and the
//! `--no-effects` no-op (glitch has no particle sidecar).

use std::time::Instant;

use crate::frame::Frame;
use crate::msg_fill_style::MsgFillStyle;
// Shared helpers from the parent msg-fill-style test file.
use super::tests_msg_fill_style::{make_cloud_colored, set_message_elapsed, visible_content_cells};

/// Count content cells that are DRAWN (non-space, non-border glyphs)
/// — strictly weaker than `visible_content_cells`, which requires
/// `cell.ch == mc.val`. For glitch, during the settle window the
/// cell is drawn with a WRONG glyph (substituted from
/// `GLITCH_WRONG_GLYPHS`), so `visible_content_cells` does NOT
/// count it. This helper counts ANY drawn content cell — used to
/// verify a cell is "visible" in the frame even when its glyph is
/// still in the wrong-glyph settle phase.
fn drawn_content_cells(frame: &Frame, cloud: &super::super::Cloud) -> Vec<(u16, u16, char)> {
    let mut cells = Vec::new();
    for mc in &cloud.message {
        if crate::cloud::border::is_border_char(mc.val) {
            continue;
        }
        if let Some(cell) = frame.get(mc.col, mc.line) {
            // A content cell that has been blanked to ' ' (hidden)
            // reads as a space — anything else means the cell is
            // drawn (either the true glyph or a wrong-glyph
            // substitute during the glitch settle window).
            if cell.ch != ' ' {
                cells.push((mc.col, mc.line, cell.ch));
            }
        }
    }
    cells
}

/// Count cells drawn with one of the glitch wrong-glyph table
/// entries (the settle window is the only place these glyphs
/// appear in the message overlay region).
fn wrong_glyph_cells(frame: &Frame, cloud: &super::super::Cloud) -> Vec<(u16, u16, char)> {
    let wrong: &[char] = &['0', '1', '#', '%', '&', '$', '@', '?'];
    drawn_content_cells(frame, cloud)
        .into_iter()
        .filter(|(_, _, ch)| wrong.contains(ch))
        .collect()
}

#[test]
fn glitch_reveals_within_budget_at_80ms_per_char() {
    // Same index budget as typewriter/engrave/hologram: "hello world"
    // = 10 content chars; reveal_count = elapsed/80. At 160 ms
    // elapsed, reveal_count = 2 — at most 2 cells can be drawn
    // (the budget gate). Some may still be hidden if their
    // scramble offset pushes the reveal into the future.
    let mut cloud = make_cloud_colored(MsgFillStyle::Glitch);
    set_message_elapsed(&mut cloud, "hello world", 160);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let drawn = drawn_content_cells(&frame, &cloud);
    assert!(
        drawn.len() <= 2,
        "glitch at 160ms must draw at most 2 cells (reveal_count budget), got {}",
        drawn.len()
    );
    assert!(
        !drawn.is_empty(),
        "glitch at 160ms must draw at least 1 cell (max(1) floor on reveal_count)"
    );
}

#[test]
fn glitch_settles_to_correct_glyphs_at_large_elapsed() {
    // At 10 s elapsed, every cell of "hello world" is revealed,
    // past the 90 ms settle window — the visible_content_cells
    // helper (which requires cell.ch == mc.val) must match all
    // 10 content cells.
    let mut cloud = make_cloud_colored(MsgFillStyle::Glitch);
    set_message_elapsed(&mut cloud, "hello world", 10_000);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let visible = visible_content_cells(&frame, &cloud);
    assert_eq!(
        visible.len(),
        10,
        "glitch at 10s must show all 10 chars with correct glyphs (post-settle), got {}",
        visible.len()
    );
    // No wrong-glyph substitutes should remain at this elapsed.
    let wrong = wrong_glyph_cells(&frame, &cloud);
    assert!(
        wrong.is_empty(),
        "glitch at 10s must have no wrong-glyph substitutes (got {})",
        wrong.len()
    );
}

#[test]
fn glitch_wrong_glyphs_visible_during_settle_window() {
    // Some cells of "wake up, neo" (12 chars) are within the 90 ms
    // settle window at SOME elapsed during the reveal — they must
    // show a wrong glyph from the table. The exact count depends on
    // the deterministic scramble pattern, so the test scans elapsed
    // values 100..3000 step 50 (58 sample points) and asserts at
    // least one shows a wrong-glyph substitute. Probability of all
    // 58 missing ≈ (1-0.5)^58 ≈ 4e-18 — effectively guaranteed.
    let mut cloud = make_cloud_colored(MsgFillStyle::Glitch);
    let mut found_wrong = None;
    for ms in (100..3000).step_by(50) {
        set_message_elapsed(&mut cloud, "wake up, neo", ms);
        let mut frame = Frame::new(30, 12, cloud.palette.bg);
        cloud.draw_message(&mut frame, Instant::now());
        let wrong = wrong_glyph_cells(&frame, &cloud);
        if !wrong.is_empty() {
            found_wrong = Some(wrong);
            break;
        }
    }
    let wrong = found_wrong.expect(
        "at least one elapsed in 100..3000 step 50 must show a wrong-glyph substitute during settle"
    );
    // Every wrong glyph must come from the fixed ASCII table.
    for (_, _, ch) in &wrong {
        assert!(
            matches!(ch, '0' | '1' | '#' | '%' | '&' | '$' | '@' | '?'),
            "wrong glyph {ch:?} must come from GLITCH_WRONG_GLYPHS table"
        );
    }
}

#[test]
fn glitch_no_effects_does_not_suppress_reveal_math() {
    // PERF-4: --no-effects gates PARTICLE sidecars (engrave sparks,
    // hologram scanline). Glitch has no particle sidecar — the
    // wrong-glyph substitution IS the reveal math. So --no-effects
    // must NOT suppress the glitch reveal: cells still draw with
    // wrong glyphs during the settle window, then settle on the
    // true glyph. This test asserts the same drawn cell count
    // with and without --no-effects at a moderate elapsed.
    let mut cloud_on = make_cloud_colored(MsgFillStyle::Glitch);
    set_message_elapsed(&mut cloud_on, "wake up, neo", 1500);
    let mut frame_on = Frame::new(30, 12, cloud_on.palette.bg);
    cloud_on.draw_message(&mut frame_on, Instant::now());
    let drawn_on = drawn_content_cells(&frame_on, &cloud_on);

    let mut cloud_off = make_cloud_colored(MsgFillStyle::Glitch);
    cloud_off.set_effects_enabled(false);
    set_message_elapsed(&mut cloud_off, "wake up, neo", 1500);
    let mut frame_off = Frame::new(30, 12, cloud_off.palette.bg);
    cloud_off.draw_message(&mut frame_off, Instant::now());
    let drawn_off = drawn_content_cells(&frame_off, &cloud_off);

    assert_eq!(
        drawn_on.len(),
        drawn_off.len(),
        "glitch draw count must be identical with/without --no-effects (no particle sidecar to gate)"
    );
}

#[test]
fn glitch_reveal_rearms_after_typewriter_restart() {
    // r-restart (restart_message_typewriter) rewinds the
    // timeline; the glitch reveal must re-arm so the fresh reveal
    // shows the wrong-glyph settle window again, then settles
    // back to the correct glyphs at large elapsed. The scan
    // approach matches `glitch_wrong_glyphs_visible_during_settle_window`
    // — at least one elapsed in 100..3000 step 50 must produce a
    // wrong glyph (probabilistically guaranteed).
    let mut cloud = make_cloud_colored(MsgFillStyle::Glitch);
    set_message_elapsed(&mut cloud, "wake up, neo", 10_000);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    // 10 s elapsed: all settled, no wrong glyphs.
    assert!(
        wrong_glyph_cells(&frame, &cloud).is_empty(),
        "no wrong glyphs expected at 10 s elapsed"
    );

    cloud.restart_message_typewriter();
    // The restart rewinds the timeline. Scan to find an elapsed where
    // the wrong-glyph settle window is active — at least one cell
    // must re-enter settle.
    let mut found = false;
    for ms in (100..3000).step_by(50) {
        set_message_elapsed(&mut cloud, "wake up, neo", ms);
        let mut f = Frame::new(30, 12, cloud.palette.bg);
        cloud.draw_message(&mut f, Instant::now());
        if !wrong_glyph_cells(&f, &cloud).is_empty() {
            found = true;
            break;
        }
    }
    assert!(
        found,
        "restarted reveal must re-arm the wrong-glyph settle window at some elapsed in 100..3000"
    );

    // And then settles again at large elapsed.
    set_message_elapsed(&mut cloud, "wake up, neo", 10_000);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    assert!(
        wrong_glyph_cells(&frame, &cloud).is_empty(),
        "restarted reveal must settle back to correct glyphs at large elapsed"
    );
}
