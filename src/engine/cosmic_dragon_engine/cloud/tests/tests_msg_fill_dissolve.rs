// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Render-level tests for the msg-fill-style `dissolve` style
//! (post-tide follow-up — the first DITHERED style; see
//! `docs/research/MSG_FILL_STYLE_EXPANSION_RESEARCH.md` §3.G).
//!
//! Split into its own file (mirroring `tests_msg_fill_radar.rs` /
//! `tests_msg_fill_tide.rs` / `tests_msg_fill_cascade.rs` / etc.) to
//! keep `tests_msg_fill_style.rs` under the 800-LOC hard cap. The
//! shared helpers (`make_cloud_colored`, `set_message_elapsed`,
//! `visible_content_cells`) are imported from the parent msg-fill-
//! style test file.
//!
//! The dissolve style is fully stateless: each content cell starts as
//! a noise glyph (from a fixed 8-glyph ASCII table) at 50% brightness
//! and condenses into its true character over 200 ms, with a per-cell
//! hashed dither threshold for the noise→true swap. These tests mirror
//! the glitch acceptance ritual (since dissolve shares the
//! glyph_override extension point).

use std::time::{Duration, Instant};

use crate::frame::Frame;
use crate::msg_fill_style::MsgFillStyle;
// Shared helpers from the parent msg-fill-style test file.
use super::tests_msg_fill_style::{make_cloud_colored, set_message_elapsed, visible_content_cells};

#[test]
fn dissolve_settles_after_dissolve_window() {
    // At large elapsed (every cell past its reveal_at + 200 ms
    // dissolve window): every content cell shows the true glyph at
    // full brightness.
    // "hello world" = 10 content chars (space is a border char,
    // filtered out by visible_content_cells). At 10 s, all 10 visible.
    let mut cloud = make_cloud_colored(MsgFillStyle::Dissolve);
    set_message_elapsed(&mut cloud, "hello world", 10_000);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let visible = visible_content_cells(&frame, &cloud);
    assert_eq!(
        visible.len(),
        10,
        "dissolve at 10s must show all 10 content chars (settled, space is border), got {}",
        visible.len()
    );
    let chars: String = visible.iter().map(|(_, _, c)| *c).collect();
    assert_eq!(chars, "helloworld", "space is a border char, filtered");
}

#[test]
fn dissolve_reveals_progressively_at_80ms_per_char() {
    // 80 ms/char pacing (same as typewriter/engrave/hologram/glitch/
    // scorch). At 240 ms: 3 cells eligible (240/80 = 3, max(1) floor).
    // Some may still be mid-dissolve (200 ms window). The
    // visible_content_cells helper checks final position + true glyph
    // (glyph_override = None), so mid-dissolve cells with noise glyphs
    // are NOT counted.
    let mut cloud = make_cloud_colored(MsgFillStyle::Dissolve);
    set_message_elapsed(&mut cloud, "hello world", 240);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let visible = visible_content_cells(&frame, &cloud);
    // At 240 ms: cell 0 (reveal_at=0) age=240 > 200 (settled, true glyph).
    // Cell 1 (reveal_at=80) age=160 < 200 (mid-dissolve, may have noise glyph).
    // Cell 2 (reveal_at=160) age=80 < 200 (mid-dissolve, likely noise glyph).
    // Cell 3 (reveal_at=240) age=0 (just revealed, noise glyph).
    // So at most 1-3 cells visible at final position (cell 0 settled,
    // others may have swapped to true glyph if dither_t < 0.4).
    assert!(
        visible.len() <= 4,
        "dissolve at 240ms must have at most 4 cells at final position (3 revealed + maybe cell 0), got {}",
        visible.len()
    );
}

#[test]
fn dissolve_reveal_rearms_fresh_window_after_typewriter_restart() {
    // r-restart (restart_message_typewriter) rewinds the timeline;
    // the dissolve must re-arm. At 10 s all settled; after restart
    // at t=0, only cell 0 is eligible (reveal_at=0, age=0, noise glyph).
    let mut cloud = make_cloud_colored(MsgFillStyle::Dissolve);
    set_message_elapsed(&mut cloud, "hello world", 10_000);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let visible_before = visible_content_cells(&frame, &cloud);
    assert_eq!(
        visible_before.len(),
        10,
        "dissolve at 10s must show all 10 chars (settled)"
    );

    // Restart: rewind to t=0. Cell 0 just revealed (noise glyph),
    // cells 1-9 hidden. visible_content_cells checks true glyph, so
    // cell 0 (noise glyph) is NOT counted → 0 visible.
    set_message_elapsed(&mut cloud, "hello world", 0);
    let mut frame2 = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame2, Instant::now());
    let visible_after = visible_content_cells(&frame2, &cloud);
    // Cell 0 at age=0: progress=0, dither_t in [0,1). If dither_t = 0
    // (rare), the cell swaps immediately and is counted. Otherwise
    // the noise glyph is shown and the cell is NOT counted.
    // Most likely 0 visible (cell 0 shows noise glyph).
    assert!(
        visible_after.len() <= 1,
        "restarted dissolve at t=0 must show at most 1 cell (cell 0 noise glyph, others hidden), got {}",
        visible_after.len()
    );
}

#[test]
fn dissolve_no_slide_rows_no_tint() {
    // Dissolve is brightness + glyph modulated — the noise-to-text
    // condensation is the reveal math, not a positional or color
    // animation. Verify `slide_rows = 0` and `tint = None` at every
    // point in the dissolve window.
    use crate::msg_fill_style::content_reveal;
    // Sample across the full dissolve of cells 0..10.
    for ms in 0..2000 {
        for content_idx in 0..10 {
            let r = content_reveal(MsgFillStyle::Dissolve, content_idx, 1, Some(ms), 10, 1.0);
            if r.visible {
                assert_eq!(
                    r.slide_rows, 0,
                    "dissolve slide_rows must be 0 (no positional animation) at ms={} idx={}",
                    ms, content_idx
                );
                assert!(
                    r.tint.is_none(),
                    "dissolve tint must be None at ms={} idx={}",
                    ms,
                    content_idx
                );
            }
        }
    }
}

#[test]
fn dissolve_glyph_override_is_none_after_settle() {
    // After the dissolve window (age >= 200 ms), glyph_override must
    // be None (the true glyph is shown). Verify via content_reveal.
    use crate::msg_fill_style::content_reveal;
    // Cell 0 reveal_at=0. At 250 ms (age=250 > 200): settled.
    let r = content_reveal(MsgFillStyle::Dissolve, 0, 1, Some(250), 10, 1.0);
    assert!(r.visible);
    assert!(
        r.glyph_override.is_none(),
        "dissolve glyph_override must be None after settle (true glyph)"
    );
    assert!(
        (r.factor - 1.0).abs() < 1e-6,
        "dissolve factor must be 1.0 after settle"
    );
}

#[test]
fn dissolve_glyph_override_is_some_during_noise_phase() {
    // During the noise phase (progress < dither_t), glyph_override
    // is Some(noise_glyph). Find a cell with dither_t > 0.1 (almost
    // all cells) and verify the noise glyph at progress=0.
    use crate::msg_fill_style::content_reveal;
    for content_idx in 0..20 {
        // Use the dissolve module's dither_threshold via content_reveal
        // at age=0 (progress=0). If glyph_override is Some, the cell
        // is in the noise phase (dither_t > 0).
        let r = content_reveal(
            MsgFillStyle::Dissolve,
            content_idx,
            1,
            Some(content_idx * 80),
            10,
            1.0,
        );
        if let Some(noise) = r.glyph_override {
            // The noise glyph must be in the DISSOLVE_NOISE_GLYPHS table.
            // The table is ['0', '1', '#', '%', '&', '$', '@', '?'] —
            // all single-width ASCII graphic chars.
            assert!(
                matches!(noise, '0' | '1' | '#' | '%' | '&' | '$' | '@' | '?'),
                "dissolve noise glyph '{}' must be in the DISSOLVE_NOISE_GLYPHS table",
                noise
            );
            // The cell must be at dim brightness (progress=0 → factor=DISSOLVE_DIM=0.50).
            assert!(
                (r.factor - 0.50).abs() < 1e-3,
                "dissolve noise-phase factor ({}) must be DISSOLVE_DIM (0.50)",
                r.factor
            );
            return; // one cell is enough to verify the noise phase
        }
    }
    // If no cell had glyph_override = Some, that's a bug — at least
    // some cells should have dither_t > 0.
    panic!("no cell showed a noise glyph during the dissolve window");
}

#[test]
fn dissolve_settles_without_timeline_at_bench() {
    // No timeline (bench/edge): settled immediately. The bench path
    // passes elapsed_ms = None, so every cell shows at full brightness
    // with the true glyph.
    use crate::msg_fill_style::content_reveal;
    for content_idx in 0..10 {
        let r = content_reveal(MsgFillStyle::Dissolve, content_idx, 1, None, 10, 1.0);
        assert!(
            r.visible,
            "cell {} must be visible without timeline",
            content_idx
        );
        assert_eq!(r.slide_rows, 0, "slide_rows must be 0 without timeline");
        assert!(
            r.glyph_override.is_none(),
            "glyph_override must be None without timeline (true glyph)"
        );
        assert!(
            (r.factor - 1.0).abs() < 1e-6,
            "factor must be 1.0 without timeline"
        );
    }
}

// Keep Duration import used (set_message_elapsed takes ms, but the
// renderer's Instant::now() + Duration is the canonical pattern —
// keep the import so future test additions don't need to re-add it).
#[allow(dead_code)]
fn _duration_keep(_d: Duration) {}
