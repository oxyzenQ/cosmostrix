// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Render-level tests for the pulse scanner cursor (post-cascade
//! improvement — see `msg_fill_style/pulse.rs`). The cursor is a
//! visible `▌` (U+258C) glyph painted ON TOP of the most recently
//! revealed content cell, traveling left-to-right as the text
//! types. `--no-effects` suppresses the cursor (PERF-4).

use std::time::Instant;

use crate::frame::Frame;
use crate::msg_fill_style::MsgFillStyle;
// Shared helpers from the parent msg-fill-style test file.
use super::tests_msg_fill_style::{make_cloud_colored, set_message_elapsed};

/// Count `▌` (U+258C) cursor glyphs anywhere in the frame. The
/// overlay itself never emits this glyph outside the pulse cursor
/// pass, so any hit is the cursor.
fn pulse_cursor_glyphs(frame: &Frame) -> Vec<(u16, u16)> {
    let mut out = Vec::new();
    for y in 0..frame.height {
        for x in 0..frame.width {
            if let Some(cell) = frame.get(x, y) {
                if cell.ch == '▌' {
                    out.push((x, y));
                }
            }
        }
    }
    out
}

#[test]
fn pulse_cursor_visible_during_reveal() {
    // During the reveal (elapsed < total reveal time), the cursor
    // must be painted at the most recently revealed content cell.
    let mut cloud = make_cloud_colored(MsgFillStyle::Pulse);
    set_message_elapsed(&mut cloud, "wake up, neo", 500);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let cursors = pulse_cursor_glyphs(&frame);
    assert_eq!(
        cursors.len(),
        1,
        "pulse must paint exactly one cursor glyph at the head, got {}",
        cursors.len()
    );
}

#[test]
fn pulse_cursor_at_most_recently_revealed_char() {
    // At elapsed 500 ms, reveal_count = 500/80 = 6 (max(1)). The
    // head is index 5 (6th content char). The cursor must be at the
    // same position as the 6th content cell.
    let mut cloud = make_cloud_colored(MsgFillStyle::Pulse);
    set_message_elapsed(&mut cloud, "wake up, neo", 500);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());

    // Find the 6th content cell (index 5, 0-based).
    let head_mc = cloud
        .message
        .iter()
        .filter(|mc| !crate::cloud::border::is_border_char(mc.val))
        .nth(5)
        .expect("message must have at least 6 content cells");
    let cursors = pulse_cursor_glyphs(&frame);
    assert_eq!(cursors.len(), 1);
    assert_eq!(
        cursors[0],
        (head_mc.col, head_mc.line),
        "cursor must be at the 6th content cell (head_idx = 5)"
    );
}

#[test]
fn pulse_cursor_respects_no_effects() {
    // PERF-4: --no-effects must suppress the cursor pass. The
    // brightness boost (part of the reveal math) is NOT gated.
    let mut cloud = make_cloud_colored(MsgFillStyle::Pulse);
    cloud.set_effects_enabled(false);
    set_message_elapsed(&mut cloud, "wake up, neo", 500);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let cursors = pulse_cursor_glyphs(&frame);
    assert!(
        cursors.is_empty(),
        "--no-effects must suppress the pulse cursor (got {} '▌' glyphs)",
        cursors.len()
    );
}

#[test]
fn pulse_cursor_no_cursor_without_timeline() {
    // No timeline (bench/edge): the cursor pass early-returns.
    let mut cloud = make_cloud_colored(MsgFillStyle::Pulse);
    set_message_elapsed(&mut cloud, "wake up, neo", 0);
    // Override to None timeline.
    cloud.message_start_time = None;
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let cursors = pulse_cursor_glyphs(&frame);
    assert!(
        cursors.is_empty(),
        "no timeline must skip the cursor pass (got {} '▌' glyphs)",
        cursors.len()
    );
}
