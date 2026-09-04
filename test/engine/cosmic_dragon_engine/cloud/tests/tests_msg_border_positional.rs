// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! v80.0.0-alpha.1 (S-master-HUNT-3, owner bug: message dash swallowed): positional
//! border-classification tests for the message overlay.
//!
//! The bug: `-mb`'s border-vs-content split was GLYPH-based
//! (`is_border_char(val)` also matched ' ', '+', '-', '|' and the
//! box-drawing set), so user text characters that collided with border
//! glyphs were classified as border cells — never revealed as content
//! (drawn blank), excluded from the reveal budget, and (in `-m` mode)
//! able to fabricate a border order out of user text. The default
//! message "…v80.0.0-alpha.1" rendered as "v80.0.0 alpha.1".
//!
//! The fix: `MsgChr.is_border` — stamped POSITIONALLY by the layout
//! (perimeter of a bordered box), user text is ALWAYS content. These
//! tests pin every leg of that contract.

use std::time::Instant;

use crate::frame::Frame;
use crate::msg_fill_style::MsgFillStyle;
// Shared helpers from the parent msg-fill-style test file.
use super::tests_msg_fill_style::{make_cloud_colored, set_message_elapsed};

/// Collect the visible (settled) message content as a string, in
/// reading order. Assumes elapsed is large enough that the reveal has
/// fully settled for every style (60 s covers the slowest pacing:
/// 80 ms/char * ~60 chars + sidecar decay).
fn settled_message_text(cloud: &mut super::super::Cloud, text: &str, border: bool) -> String {
    cloud.set_message_border(border);
    cloud.set_message(text);
    cloud.message_start_time = Some(Instant::now() - std::time::Duration::from_secs(60));
    let mut frame = Frame::new(60, 20, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let mut out = String::new();
    for mc in &cloud.message {
        if mc.is_border || mc.val == ' ' {
            continue;
        }
        if let Some(cell) = frame.get(mc.col, mc.line) {
            out.push(cell.ch);
        }
    }
    out
}

#[test]
fn message_dash_renders_as_content() {
    // The EXACT owner repro: the default message contains
    // "v80.0.0-alpha.1" — the dash must render, not vanish.
    // (Spaces are not content cells — the reveal budget skips them —
    // so the verbatim compare uses the space-stripped expectation.)
    let mut cloud = make_cloud_colored(MsgFillStyle::Instant);
    let text = "Experience a masterpiece with cosmostrix v80.0.0-alpha.1";
    let rendered = settled_message_text(&mut cloud, text, true);
    assert!(
        rendered.contains("v80.0.0-alpha.1"),
        "bordered -mb message must render the dash: got {rendered:?}"
    );
    let stripped: String = text.chars().filter(|&c| c != ' ').collect();
    assert_eq!(
        rendered, stripped,
        "the full message must render verbatim (dash included, spaces excluded from content)"
    );
}

#[test]
fn message_dash_renders_without_border_too() {
    // `-m` (no border) mode: same content contract — the dash cell is
    // content, never a fabricated border cell.
    let mut cloud = make_cloud_colored(MsgFillStyle::Instant);
    let text = "v80.0.0-alpha.1";
    let rendered = settled_message_text(&mut cloud, text, false);
    assert_eq!(
        rendered, text,
        "-m (no border) message must render the dash verbatim"
    );
}

#[test]
fn message_ascii_border_glyphs_render_as_content() {
    // '+' and '|' share the old is_border_char match list — both must
    // render as content in a bordered box (same defect family).
    let mut cloud = make_cloud_colored(MsgFillStyle::Instant);
    let text = "a+b|c-d";
    let rendered = settled_message_text(&mut cloud, text, true);
    assert_eq!(rendered, text, "'+'/'|'/'-' in user text are content");
}

#[test]
fn message_box_drawing_char_renders_as_content() {
    // Even a box-drawing glyph ('╭', width 1 — passes sanitize) is
    // content when the user typed it. The layout's own border glyphs
    // are content-disjoint by POSITION, so this can't be ambiguous.
    let mut cloud = make_cloud_colored(MsgFillStyle::Instant);
    let text = "x─y";
    let rendered = settled_message_text(&mut cloud, text, true);
    assert_eq!(rendered, "x─y", "box-drawing char in user text is content");
}

#[test]
fn border_order_excludes_user_text_glyphs() {
    // build_border_order must only contain POSITIONAL border cells —
    // a message of only dashes must NOT fabricate a border order
    // (the old glyph test produced one even in -m mode).
    let mut cloud = make_cloud_colored(MsgFillStyle::Instant);
    cloud.set_message_border(false); // -m: no layout border cells at all
    cloud.set_message("----");
    assert!(
        cloud.border_order.is_empty(),
        "-m message of dashes must not fabricate a border order"
    );

    // Bordered mode: the border order is exactly the perimeter ring,
    // unaffected by content dashes.
    let mut cloud2 = make_cloud_colored(MsgFillStyle::Instant);
    cloud2.set_message_border(true);
    cloud2.set_message("--");
    let ring: usize = cloud2.message.iter().filter(|mc| mc.is_border).count();
    assert_eq!(
        cloud2.border_order.len(),
        ring,
        "border order must cover exactly the layout border cells"
    );
}

#[test]
fn word_ordinals_dash_joins_the_word() {
    // words style: a dash INSIDE a word must not split it into two
    // words ("v80.0.0-alpha.1" = ONE word; spaces still split).
    let mut cloud = make_cloud_colored(MsgFillStyle::Words);
    cloud.set_message_border(true);
    cloud.set_message("v80.0.0-alpha.1 end");
    // 16 content chars: "v80.0.0-alpha.1" (15) + "end" (3) = 18.
    let content: Vec<u32> = cloud
        .message
        .iter()
        .zip(cloud.message_word_ordinals.iter())
        .filter(|(mc, _)| !mc.is_border && mc.val != ' ')
        .map(|(_, &ord)| ord)
        .collect();
    assert_eq!(content.len(), 18, "content char count (dash included)");
    assert_eq!(
        content.iter().copied().max(),
        Some(2),
        "exactly two words: 'v80.0.0-alpha.1' + 'end' (the dash must not split word 1)"
    );
    // The first 15 ordinals are all word 1 (dash included).
    assert!(
        content[..15].iter().all(|&o| o == 1),
        "'v80.0.0-alpha.1' must be a single word"
    );
}

#[test]
fn reveal_budget_counts_border_colliding_chars() {
    // total_text must count the dash: the reveal pacing budget for
    // typewriter is 80 ms/char — at 80 ms elapsed exactly 1 char is
    // visible. With the OLD classification the dash was excluded from
    // the budget; now "a-b" has 3 budget cells.
    let mut cloud = make_cloud_colored(MsgFillStyle::Typewriter);
    set_message_elapsed(&mut cloud, "a-b", 80);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let visible: Vec<(u16, u16, char)> = cloud
        .message
        .iter()
        .filter(|mc| !mc.is_border && mc.val != ' ')
        .filter_map(|mc| {
            frame
                .get(mc.col, mc.line)
                .filter(|c| c.ch == mc.val)
                .map(|_| (mc.col, mc.line, mc.val))
        })
        .collect();
    assert_eq!(
        visible.len(),
        1,
        "typewriter at 80ms shows exactly 1 char ('a'); the dash occupies a real budget slot"
    );
}
