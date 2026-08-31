// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Render-level tests for the v51 msg-fill-style `cascade` style
//! (post-scorch follow-up — see
//! `docs/research/MSG_FILL_STYLE_EXPANSION_RESEARCH.md` §3.D).
//!
//! Split into its own file (mirroring `tests_msg_fill_hologram.rs` /
//! `tests_msg_fill_glitch.rs` / `tests_msg_fill_scorch.rs`) to keep
//! `tests_msg_fill_style.rs` under the 800-LOC hard cap. The shared
//! helpers (`make_cloud_colored`, `set_message_elapsed`,
//! `visible_content_cells`) are imported from the parent
//! msg-fill-style test file.
//!
//! The cascade style is fully stateless: column-paced reveal
//! (60 ms/column) + drop-from-above (240 ms fall, 3 rows above →
//! landed). The drop uses the signed `slide_rows` field (negative =
//! above), so the existing slide deferred-second-pass mechanism
//! handles the rendering. These tests mirror the engrave acceptance
//! ritual: pacing, drop presence/absence, column-paced reveal,
//! Space-restart re-arm.

use std::time::{Duration, Instant};

use crate::frame::Frame;
use crate::msg_fill_style::MsgFillStyle;
// Shared helpers from the parent msg-fill-style test file.
use super::tests_msg_fill_style::{make_cloud_colored, set_message_elapsed, visible_content_cells};

/// Count content cells that are drawn at their FINAL position (cell.ch
/// == mc.val, NOT mid-drop). The cascade drop paints the glyph at an
/// ABOVE row during the fall, so visible_content_cells (which checks
/// the final position) returns 0 for mid-drop cells and the final char
/// only after the drop completes.
fn content_cells_at_final(frame: &Frame, cloud: &super::super::Cloud) -> Vec<(u16, u16, char)> {
    visible_content_cells(frame, cloud)
}

#[test]
fn cascade_reveals_progressively_at_60ms_per_column() {
    // "hello world" = 10 content chars. At 180 ms: 180/60 = 3 columns
    // revealed (max(1) floor). Some may still be mid-drop (240 ms fall).
    let mut cloud = make_cloud_colored(MsgFillStyle::Cascade);
    set_message_elapsed(&mut cloud, "hello world", 180);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    // At 180 ms, 3 columns are eligible. Column 0 (reveal_at=0) has
    // age 180 ms < 240 ms (mid-drop) → painted above, NOT at final.
    // Column 1 (reveal_at=60) has age 120 ms < 240 → mid-drop.
    // Column 2 (reveal_at=120) has age 60 ms < 240 → mid-drop.
    // So visible_content_cells (which checks final position) may
    // return 0 — all 3 are mid-drop. That's the cascade signature:
    // chars fall INTO place, they don't fade in AT place.
    let at_final = content_cells_at_final(&frame, &cloud);
    assert!(
        at_final.len() <= 3,
        "cascade at 180ms must have at most 3 cells at final position (3 columns revealed), got {}",
        at_final.len()
    );
}

#[test]
fn cascade_settles_after_drop_window() {
    // At large elapsed, all cells are landed (drop complete).
    // "hello world" = 10 content chars (space is a border char,
    // filtered out by visible_content_cells). At 10 s, all 10
    // visible at final.
    let mut cloud = make_cloud_colored(MsgFillStyle::Cascade);
    set_message_elapsed(&mut cloud, "hello world", 10_000);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let visible = visible_content_cells(&frame, &cloud);
    assert_eq!(
        visible.len(),
        10,
        "cascade at 10s must show all 10 content chars (settled, space is border), got {}",
        visible.len()
    );
    let chars: String = visible.iter().map(|(_, _, c)| *c).collect();
    assert_eq!(chars, "helloworld", "space is a border char, filtered");
}

#[test]
fn cascade_settles_without_timeline() {
    // No timeline (bench/edge): settled immediately — slide_rows = 0,
    // factor 1.0.
    let mut cloud = make_cloud_colored(MsgFillStyle::Cascade);
    set_message_elapsed(&mut cloud, "hi", 0);
    // Override to None timeline (set_message_elapsed sets a real
    // timeline; we want the None path).
    cloud.message_start_time = None;
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let visible = visible_content_cells(&frame, &cloud);
    assert_eq!(
        visible.len(),
        2,
        "cascade with no timeline must show both chars immediately"
    );
}

#[test]
fn cascade_drop_glyphs_visible_above_final_position_during_fall() {
    // During the drop (age < 240 ms), the glyph is painted at an
    // ABOVE row (slide_rows negative). The final-position cell is
    // blanked (space). So at small elapsed for column 0 (age 60 ms,
    // mid-drop), the final cell of 'h' must be a space, and the
    // glyph 'h' must appear at an above row.
    let mut cloud = make_cloud_colored(MsgFillStyle::Cascade);
    set_message_elapsed(&mut cloud, "hi", 60);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());

    // Find the first content cell (char 'h').
    let mc = cloud
        .message
        .iter()
        .find(|mc| !crate::cloud::border::is_border_char(mc.val))
        .expect("message must have content cells");
    // At 60 ms, column 0 (reveal_at=0) has age 60 ms < 240 → mid-drop.
    // Final cell must be blanked (space) — the glyph is painted above.
    let final_cell = frame.get(mc.col, mc.line).expect("final cell exists");
    assert_eq!(
        final_cell.ch, ' ',
        "mid-drop final cell must be blanked (space), got {:?}",
        final_cell.ch
    );
    // The glyph 'h' must appear at an above row (line < mc.line).
    let mut found_above = false;
    for y in 0..mc.line {
        if let Some(cell) = frame.get(mc.col, y) {
            if cell.ch == mc.val {
                found_above = true;
                break;
            }
        }
    }
    assert!(
        found_above,
        "mid-drop glyph 'h' must be painted at an above row during fall"
    );
}

#[test]
fn cascade_column_paced_reveal_not_per_char() {
    // Cascade is column-paced (60 ms/column), NOT per-char (80 ms/char
    // for typewriter). At 120 ms, cascade reveals 2 columns (120/60),
    // typewriter reveals 1 char (120/80 = 1). Distinct pacing — verified
    // via index_reveal_count, not the rendered frame (mid-drop glyphs
    // make the rendered comparison fragile).
    use crate::msg_fill_style::index_reveal_count;
    let cascade_budget = index_reveal_count(MsgFillStyle::Cascade, Some(120), 10);
    let type_budget = index_reveal_count(MsgFillStyle::Typewriter, Some(120), 10);
    assert_eq!(
        cascade_budget, 2,
        "cascade at 120ms must reveal 2 columns (120/60)"
    );
    assert_eq!(
        type_budget, 1,
        "typewriter at 120ms must reveal 1 char (120/80 floor 1)"
    );
    assert!(
        cascade_budget > type_budget,
        "cascade must reveal more columns than typewriter chars at 120ms (faster pacing)"
    );
}

#[test]
fn cascade_reveal_rearms_fresh_drop_after_typewriter_restart() {
    // Space-restart (restart_message_typewriter) rewinds the timeline;
    // the drop animation must re-arm. At 10 s all settled; after
    // restart at small elapsed, mid-drop glyphs reappear above.
    let mut cloud = make_cloud_colored(MsgFillStyle::Cascade);
    set_message_elapsed(&mut cloud, "hi", 10_000);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    // 10 s: both chars landed.
    assert_eq!(
        visible_content_cells(&frame, &cloud).len(),
        2,
        "cascade at 10s must show both chars (settled)"
    );

    cloud.restart_message_typewriter();
    // After restart, elapsed ~60 ms (mid-drop for column 0).
    cloud.message_start_time = Some(Instant::now() - Duration::from_millis(60));
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    // Column 0 (reveal_at=0) mid-drop: final cell blanked.
    let mc = cloud
        .message
        .iter()
        .find(|mc| !crate::cloud::border::is_border_char(mc.val))
        .expect("message must have content");
    let final_cell = frame.get(mc.col, mc.line).expect("final cell exists");
    assert_eq!(
        final_cell.ch, ' ',
        "restarted cascade at 60ms must blank the final cell (mid-drop re-armed)"
    );
}
