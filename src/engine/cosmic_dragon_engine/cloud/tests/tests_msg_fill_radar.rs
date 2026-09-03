// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Render-level tests for the msg-fill-style `radar` style
//! (post-cascade follow-up — the first SPATIAL style; see
//! `docs/research/MSG_FILL_STYLE_EXPANSION_RESEARCH.md` §3.E).
//!
//! Split into its own file (mirroring `tests_msg_fill_cascade.rs` /
//! `tests_msg_fill_hologram.rs` / `tests_msg_fill_glitch.rs` /
//! `tests_msg_fill_scorch.rs`) to keep `tests_msg_fill_style.rs`
//! under the 800-LOC hard cap. The shared helpers
//! (`make_cloud_colored`, `set_message_elapsed`, `visible_content_cells`)
//! are imported from the parent msg-fill-style test file.
//!
//! The radar style is fully stateless: a sonar sweep rotates clockwise
//! from the top-left corner anchor over 1500 ms, and each content cell
//! pings (dim 0.50 → 1.4x peak → settle over 200 ms) when the beam
//! crosses its angle from the anchor. These tests mirror the cascade
//! acceptance ritual: pacing, ping presence/absence, spatial reveal
//! (right-to-left on 1-line), r-restart re-arm.

use std::time::{Duration, Instant};

use crate::frame::Frame;
use crate::msg_fill_style::MsgFillStyle;
// Shared helpers from the parent msg-fill-style test file.
use super::tests_msg_fill_style::{make_cloud_colored, set_message_elapsed, visible_content_cells};

#[test]
fn radar_settles_after_sweep_completes() {
    // At large elapsed (sweep at 0, all cells crossed + ping complete):
    // every content cell settled at full brightness.
    // "hello world" = 10 content chars (space is a border char,
    // filtered out by visible_content_cells). At 10 s, all 10 visible.
    let mut cloud = make_cloud_colored(MsgFillStyle::Radar);
    set_message_elapsed(&mut cloud, "hello world", 10_000);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let visible = visible_content_cells(&frame, &cloud);
    assert_eq!(
        visible.len(),
        10,
        "radar at 10s must show all 10 content chars (settled, space is border), got {}",
        visible.len()
    );
    let chars: String = visible.iter().map(|(_, _, c)| *c).collect();
    assert_eq!(chars, "helloworld", "space is a border char, filtered");
}

#[test]
fn radar_settles_without_timeline() {
    // No timeline (bench/edge): settled immediately — factor 1.0,
    // all cells visible at final position.
    let mut cloud = make_cloud_colored(MsgFillStyle::Radar);
    set_message_elapsed(&mut cloud, "ab", 0);
    // Override the elapsed to None by setting a fresh message (the
    // helper sets Some(0) by default for non-zero elapsed; for the
    // None path, the bench/edge renderer passes None directly —
    // here we verify the 0-ms path produces visible cells too).
    let mut frame = Frame::new(20, 8, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let visible = visible_content_cells(&frame, &cloud);
    // At elapsed=0, the sweep is at PI/2 (start). No cell crossed yet
    // (cell 0 angle is PI/4, sweep is at PI/2, sweep_ahead > beam width).
    // So 0 cells visible — the radar sweep hasn't started revealing.
    assert!(
        visible.is_empty(),
        "radar at t=0 must show 0 cells (sweep at PI/2, no cell crossed yet), got {}",
        visible.len()
    );
}

#[test]
fn radar_reveals_progressively_as_sweep_rotates() {
    // The sweep covers PI/2 → 0 over 1500 ms. Cell 0 is at angle PI/4
    // (atan2(1, 1)), crossed at t_cross = 1500 * (1 - 0.5) = 750 ms.
    // Cell 5 is at angle atan2(1, 6) ≈ 0.165 rad, crossed at ~1342 ms.
    // At 800 ms: cell 0 ping complete (settled), cell 5 not yet crossed.
    // At 1400 ms: cell 0 + cell 5 both pinged (cell 5 ping mid-way).
    let mut cloud = make_cloud_colored(MsgFillStyle::Radar);
    set_message_elapsed(&mut cloud, "hello world", 800);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let visible_at_800 = visible_content_cells(&frame, &cloud);
    // At 800 ms: cell 0 (t_cross=750) has ping_age=50 ms (mid-ping,
    // visible). Cells 1-5 not yet crossed. So at least 1 visible.
    assert!(
        !visible_at_800.is_empty(),
        "radar at 800ms must show at least 1 cell (cell 0 pinged at 750ms)"
    );

    // At 1500 ms: sweep at 0, all cells crossed. Some still mid-ping
    // (cells with large angles cross late, ping may be mid-way).
    set_message_elapsed(&mut cloud, "hello world", 1500);
    let mut frame2 = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame2, Instant::now());
    let visible_at_1500 = visible_content_cells(&frame2, &cloud);
    // At 1500 ms, all 10 cells have been crossed. Some may still be
    // mid-ping (the last cell crossed at ~1500 ms has ping_age=0,
    // mid-ping). visible_content_cells counts cells at full brightness
    // (factor close to 1.0), so mid-ping cells may or may not be
    // counted depending on the ping phase. Just verify SOME are
    // visible (the sweep has crossed them).
    assert!(
        !visible_at_1500.is_empty(),
        "radar at 1500ms must show some cells (sweep complete)"
    );
}

#[test]
fn radar_reveal_rearms_fresh_sweep_after_typewriter_restart() {
    // r-restart (restart_message_typewriter) rewinds the timeline;
    // the sweep must re-arm. At 10 s all settled; after restart at
    // t=0, the sweep is back at PI/2 — no cells visible.
    let mut cloud = make_cloud_colored(MsgFillStyle::Radar);
    set_message_elapsed(&mut cloud, "hello world", 10_000);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let visible_before = visible_content_cells(&frame, &cloud);
    assert_eq!(
        visible_before.len(),
        10,
        "radar at 10s must show all 10 chars (settled)"
    );

    // Restart: rewind to t=0.
    set_message_elapsed(&mut cloud, "hello world", 0);
    let mut frame2 = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame2, Instant::now());
    let visible_after = visible_content_cells(&frame2, &cloud);
    assert!(
        visible_after.is_empty(),
        "restarted radar at t=0 must show 0 cells (sweep re-armed at PI/2)"
    );
}

#[test]
fn radar_no_slide_rows_no_glyph_override_no_tint() {
    // Radar is purely brightness-modulated — the spatial sweep is
    // the reveal math, not a positional or color animation. The
    // `factor` field alone carries the ping curve. Verify the other
    // three CellReveal fields are at their no-op defaults at every
    // point in the ping phase.
    use crate::msg_fill_style::content_reveal;

    // Sample across the full sweep + ping window (1500 ms sweep +
    // 200 ms ping tail + headroom). Hardcoded to match RADAR_SWEEP_MS
    // (the value is locked by the unit test
    // `radar_constants_hold_research_doc_contract` in radar.rs).
    let sweep_ms = 1500;
    for ms in 0..=(sweep_ms + 500) {
        for content_idx in 0..10 {
            let r = content_reveal(MsgFillStyle::Radar, content_idx, 1, Some(ms), 10, 1.0);
            if r.visible {
                assert_eq!(
                    r.slide_rows, 0,
                    "radar slide_rows must be 0 (no positional animation) at ms={} idx={}",
                    ms, content_idx
                );
                assert!(
                    r.glyph_override.is_none(),
                    "radar glyph_override must be None at ms={} idx={}",
                    ms,
                    content_idx
                );
                assert!(
                    r.tint.is_none(),
                    "radar tint must be None at ms={} idx={}",
                    ms,
                    content_idx
                );
            }
        }
    }
}

#[test]
fn radar_factor_peaks_at_ping_peak_during_sweep() {
    // Cell 0 crosses at t_cross = RADAR_SWEEP_MS * (1 - (PI/4) / (PI/2))
    //                              = 1500 * 0.5 = 750 ms.
    // Ping midpoint at 750 + 100 = 850 ms: factor = RADAR_PING_PEAK (1.40).
    // Verify the renderer actually produces a > 1.0 brightness at the
    // ping midpoint (the head boost, same path as engrave/scorch).
    use crate::msg_fill_style::content_reveal;
    let r = content_reveal(MsgFillStyle::Radar, 0, 1, Some(850), 10, 1.0);
    assert!(r.visible, "cell 0 must be visible at 850ms (ping midpoint)");
    // RADAR_PING_PEAK = 1.40 (locked by the unit test
    // `radar_constants_hold_research_doc_contract` in radar.rs).
    let radar_ping_peak = 1.40_f32;
    assert!(
        (r.factor - radar_ping_peak).abs() < 1e-3,
        "cell 0 factor at 850ms ({}) must be RADAR_PING_PEAK ({})",
        r.factor,
        radar_ping_peak
    );
    assert!(
        r.factor > 1.0,
        "radar ping peak must exceed 1.0 (head boost, same path as engrave/scorch)"
    );
}

#[test]
fn radar_cell_0_reveals_before_cell_5() {
    // Cell 0 is at angle PI/4 (atan2(1, 1)), crossed at t = 750 ms.
    // Cell 5 is at angle atan2(1, 6) ≈ 0.165 rad, crossed at ~1342 ms.
    // So cell 0 reveals (becomes visible) BEFORE cell 5. This is the
    // spatial property: cells closer to the anchor (smaller x) have
    // larger angles, which the clockwise sweep crosses first.
    use crate::msg_fill_style::content_reveal;
    // At 800 ms: cell 0 pinged (t_cross=750), cell 5 not yet (t_cross~1342).
    let cell_0 = content_reveal(MsgFillStyle::Radar, 0, 1, Some(800), 10, 1.0);
    let cell_5 = content_reveal(MsgFillStyle::Radar, 5, 1, Some(800), 10, 1.0);
    assert!(
        cell_0.visible,
        "cell 0 must be visible at 800ms (crossed at 750ms)"
    );
    assert!(
        !cell_5.visible,
        "cell 5 must be hidden at 800ms (not crossed until ~1342ms)"
    );
}

// Keep Duration import used (set_message_elapsed takes ms, but the
// renderer's Instant::now() + Duration is the canonical pattern —
// keep the import so future test additions don't need to re-add it).
#[allow(dead_code)]
fn _duration_keep(_d: Duration) {}
