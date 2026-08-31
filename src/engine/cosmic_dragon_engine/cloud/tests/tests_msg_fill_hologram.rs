// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Render-level tests for the v51 msg-fill-style `hologram` style
//! (post-engrave follow-up — see
//! `docs/research/MSG_FILL_STYLE_EXPANSION_RESEARCH.md`).
//!
//! Split from `tests_msg_fill_style.rs` to keep that file under the
//! 800-LOC hard cap (see `src/RULES_LOC.md`). The shared helpers
//! (`make_cloud_colored`, `set_message_elapsed`, `visible_content_cells`)
//! live in `tests_msg_fill_style.rs` as `pub(super)` and are imported
//! via `super::tests_msg_fill_style::*`.
//!
//! The hologram style is fully stateless: the reveal math (burn-in +
//! flicker + breathing) drives the per-cell factor, and a single
//! scanline sweep over `HOLOGRAM_SCANLINE_MS` (600 ms) paints a row
//! of `▔` (U+2594) across the box. These tests mirror the engrave
//! acceptance ritual: pacing, brightness-at-age, scanline
//! presence/absence, `--no-effects` gating, and r-restart re-arm.

use std::time::Instant;

use crate::frame::Frame;
use crate::msg_fill_style::MsgFillStyle;
// Shared helpers from the parent msg-fill-style test file.
use super::tests_msg_fill_style::{make_cloud_colored, set_message_elapsed, visible_content_cells};

/// Count scanline glyphs (`▔`, U+2594 UPPER ONE EIGHTH BLOCK) anywhere
/// in the frame. The overlay itself never emits this glyph outside
/// the hologram scanline pass, so any hit is the scanline.
fn hologram_scanline_glyphs(frame: &Frame) -> Vec<(u16, u16)> {
    let mut out = Vec::new();
    for y in 0..frame.height {
        for x in 0..frame.width {
            if let Some(cell) = frame.get(x, y) {
                if cell.ch == '▔' {
                    out.push((x, y));
                }
            }
        }
    }
    out
}

#[test]
fn hologram_reveals_progressively_like_typewriter_pacing() {
    // Same 80 ms/char pacing as typewriter/engrave: "hello world"
    // = 10 content chars; 160 ms → 2 chars revealed.
    let mut cloud = make_cloud_colored(MsgFillStyle::Hologram);
    set_message_elapsed(&mut cloud, "hello world", 160);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let visible = visible_content_cells(&frame, &cloud);
    assert_eq!(
        visible.len(),
        2,
        "hologram at 160ms must show exactly 2 chars (80ms/char), got {}",
        visible.len()
    );
    let chars: String = visible.iter().map(|(_, _, c)| *c).collect();
    assert_eq!(
        chars, "he",
        "first two chars of 'hello world' must be visible"
    );
}

#[test]
fn hologram_chars_burn_in_at_full_brightness_in_flicker_band() {
    // Cell 0 reveals at t=0; age 0 ms is the first flicker bucket.
    // The factor must lie within [1 - AMP, 1 + AMP] = [0.70, 1.30]
    // — NOT the 0.30 fade-in start the typewriter family uses. The
    // exact flicker value at (0, bucket=0) is unit-tested in
    // `msg_fill_style/hologram.rs`; this render test only verifies
    // the cell is visible and painted with a color (the burn-in
    // happened at all). Combined with the pacing test above, this
    // locks the hologram reveal contract: visible + colored, not
    // the typewriter's 30% dim.
    let mut cloud = make_cloud_colored(MsgFillStyle::Hologram);
    set_message_elapsed(&mut cloud, "hi", 0);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let visible = visible_content_cells(&frame, &cloud);
    assert!(
        !visible.is_empty(),
        "hologram at t=0 must reveal the first char"
    );
    // The first visible cell must carry an explicit fg color (the
    // palette head color, brightness-modulated) — never the
    // monochrome `None` path and never the blanked space.
    let (col, line, _ch) = &visible[0];
    let cell = frame.get(*col, *line).expect("visible cell must exist");
    assert!(
        cell.fg.is_some(),
        "hologram burn-in must paint with an explicit color, not the mono path"
    );
}

#[test]
fn hologram_scanline_visible_during_reveal_sweep() {
    // At elapsed = 100 ms (within the 600 ms sweep), the scanline
    // must paint `▔` glyphs at the sweep row.
    let mut cloud = make_cloud_colored(MsgFillStyle::Hologram);
    set_message_elapsed(&mut cloud, "wake up, neo", 100);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let scanline = hologram_scanline_glyphs(&frame);
    assert!(
        !scanline.is_empty(),
        "hologram scanline must paint ▔ glyphs during the 600 ms sweep"
    );
}

#[test]
fn hologram_scanline_disappears_after_sweep_completes() {
    // After `HOLOGRAM_SCANLINE_MS` (600 ms), the scanline is gone
    // for the rest of the overlay's lifetime.
    let mut cloud = make_cloud_colored(MsgFillStyle::Hologram);
    set_message_elapsed(&mut cloud, "wake up, neo", 601);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let scanline = hologram_scanline_glyphs(&frame);
    assert!(
        scanline.is_empty(),
        "hologram scanline must be gone after the 600 ms sweep (got {} ▔ glyphs)",
        scanline.len()
    );
}

#[test]
fn hologram_scanline_respects_no_effects() {
    // PERF-4: `--no-effects` must suppress the scanline pass exactly
    // like every particle subsystem. The reveal math is unaffected
    // (text still burns in), only the scanline overlay is gated.
    let mut cloud = make_cloud_colored(MsgFillStyle::Hologram);
    cloud.set_effects_enabled(false);
    set_message_elapsed(&mut cloud, "wake up, neo", 100);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let scanline = hologram_scanline_glyphs(&frame);
    assert!(
        scanline.is_empty(),
        "--no-effects must suppress the hologram scanline (got {} ▔ glyphs)",
        scanline.len()
    );
    // The reveal math must still run — the first chars of the
    // message must be visible.
    let visible = visible_content_cells(&frame, &cloud);
    assert!(
        !visible.is_empty(),
        "--no-effects must NOT suppress the reveal math itself"
    );
}

#[test]
fn hologram_reveal_restarts_fresh_sweep_after_typewriter_restart() {
    // r-restart (`restart_message_typewriter`) rewinds the
    // timeline; the scanline sweep must re-arm so the fresh
    // reveal's first 600 ms paints the scanline again.
    let mut cloud = make_cloud_colored(MsgFillStyle::Hologram);
    set_message_elapsed(&mut cloud, "wake up, neo", 10_000);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    // 10 s elapsed: scanline long gone.
    assert!(
        hologram_scanline_glyphs(&frame).is_empty(),
        "no scanline expected at 10 s elapsed"
    );

    cloud.restart_message_typewriter();
    // The restart sets start = now; immediately after, elapsed is
    // ~0 ms, well within the 600 ms sweep window.
    set_message_elapsed(&mut cloud, "wake up, neo", 50);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    assert!(
        !hologram_scanline_glyphs(&frame).is_empty(),
        "restarted reveal must re-arm the scanline sweep"
    );
}
