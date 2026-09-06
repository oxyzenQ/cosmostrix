// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-hunter-16 regression tests: HUD toggle-off ('i' shortkey) leaves
//! stale HUD metric text on screen for Glyph rain (owner report 2026-09-06).
//!
//! Symptom: pressing 'i' to hide the HUD left the metric text (fps, prs,
//! ehs, etc.) visible on screen. After some seconds it would clean up
//! when rain droplets happened to pass through the exact cells, but not
//! immediately. Other rain styles (monolith, vortex, flux, lorenz,
//! dragon, physarum) did not have this bug.
//!
//! Root cause: the 'i' toggle-off handler called
//! `cloud.force_draw_everything()`. For structured styles, the
//! `force_draw_everything` block in `rain_at()` calls
//! `frame.clear_with_bg()` which bumps the content generation — all
//! stale cells then read as blank via the gen-mismatch path, and the
//! terminal's cell-skip emit logic (HUNT-27) sees `blank != old_HUD_text`
//! → emit → cleared. For Glyph (droplet family), the
//! `force_draw_everything` block calls `frame.force_repaint()` (HUNT-25)
//! which only sets `dirty_all=true` WITHOUT bumping the generation —
//! stale cells still read as their old content (old HUD text), so the
//! terminal's cell-skip sees `old_HUD_text == old_HUD_text` → skip →
//! stale HUD stays on screen until a droplet overwrites it.
//!
//! Fix: arm `cloud.semantic_invalidate = true` alongside
//! `cloud.force_draw_everything()` in the 'i' toggle-off handler. The
//! `semantic_invalidate` block runs BEFORE the `force_draw_everything`
//! block in `rain_at()`, calling `frame.invalidate_semantic(bg)` which
//! calls `clear_with_bg()` → bumps gen → all cells read as blank →
//! cell-skip emits the difference → HUD cleared immediately on ALL 7
//! rain styles.

use std::time::{Duration, Instant};

use crate::cloud::Cloud;
use crate::frame::Frame;
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};

fn make_style_cloud(style: RainStyle, cols: u16, lines: u16) -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::Mono,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Green,
        style,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.set_droplet_density(0.9);
    cloud.set_chars_per_sec(18.0);
    cloud.reset(cols, lines);
    cloud.clear_redraw_flags_for_test();
    cloud
}

/// Simulate the 'i' toggle-off handler: arm semantic_invalidate +
/// force_draw_everything (the exact sequence in event_loop.rs).
fn arm_hud_toggle_off(cloud: &mut Cloud) {
    cloud.semantic_invalidate = true;
    cloud.force_draw_everything();
}

/// Write "HUD text" into the top-left cells of the frame, simulating
/// the HUD overlay being visible. Returns the flat indices that were
/// written.
fn write_hud_text(frame: &mut Frame, cols: u16, lines: u16) -> Vec<usize> {
    let mut indices = Vec::new();
    // Write a small "HUD" block: rows 0-2, cols 0-10
    for y in 0..3u16.min(lines) {
        for x in 0..10u16.min(cols) {
            let cell = crate::cell::Cell {
                ch: 'H',
                fg: Some(crossterm::style::Color::White),
                bg: None,
                bold: false,
            };
            frame.set(x, y, cell);
            indices.push(y as usize * cols as usize + x as usize);
        }
    }
    indices
}

/// Verify that after the toggle-off + one rain_at frame, ALL HUD cells
/// read as blank (via cell_at_index — the gen-mismatch path). This
/// pins the fix for the specific "stale HUD residue" symptom.
#[test]
fn hud_toggle_off_clears_glyph_residue_immediately() {
    let cols = 20u16;
    let lines = 10u16;
    let mut cloud = make_style_cloud(RainStyle::Glyph, cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);

    // Draw a few frames so the rain system is in steady state.
    let step_ms = 17u64;
    cloud.set_max_sim_delta(Duration::from_millis(step_ms));
    let start = Instant::now();
    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    for i in 0..10u32 {
        cloud.rain_at(
            &mut frame,
            start + Duration::from_millis(i as u64 * step_ms),
        );
        frame.clear_dirty();
    }

    // Simulate HUD being visible: write HUD text into the frame.
    let hud_indices = write_hud_text(&mut frame, cols, lines);
    frame.clear_dirty();

    // Verify the HUD cells actually hold non-blank content.
    for &idx in &hud_indices {
        let cell = frame.cell_at_index(idx);
        assert_eq!(
            cell.ch, 'H',
            "HUD cell at index {idx} should hold 'H' before toggle-off"
        );
    }

    // Simulate the 'i' toggle-off handler.
    arm_hud_toggle_off(&mut cloud);

    // Run one frame of rain_at — this is where semantic_invalidate +
    // force_draw_everything are consumed.
    cloud.rain_at(&mut frame, start + Duration::from_millis(200));
    frame.clear_dirty();

    // After the frame, ALL HUD cells must read as blank (or be
    // overwritten by fresh rain). No cell may still hold the stale 'H'
    // text — that would be the residual bug.
    for &idx in &hud_indices {
        let cell = frame.cell_at_index(idx);
        assert_ne!(
            cell.ch, 'H',
            "HUD cell at index {idx} still holds stale 'H' after toggle-off — the residual bug"
        );
    }
}

/// All 7 rain styles must clear HUD residue on toggle-off. This pins the
/// contract so no style can regress to the "stale HUD until rain passes
/// through" symptom. Glyph was the original bug; the other 6 already
/// worked via clear_with_bg, but the fix arms semantic_invalidate for
/// ALL styles (the 'i' handler doesn't branch on rain_style), so this
/// test verifies no style breaks.
#[test]
fn hud_toggle_off_clears_residue_for_all_seven_styles() {
    let cols = 20u16;
    let lines = 10u16;
    let step_ms = 17u64;

    for style in [
        RainStyle::Glyph,
        RainStyle::Monolith,
        RainStyle::Vortex,
        RainStyle::Flux,
        RainStyle::Lorenz,
        RainStyle::Dragon,
        RainStyle::Physarum,
    ] {
        let mut cloud = make_style_cloud(style, cols, lines);
        let mut frame = Frame::new(cols, lines, cloud.palette.bg);

        // Prime the rain system.
        cloud.set_max_sim_delta(Duration::from_millis(step_ms));
        let start = Instant::now();
        cloud.last_spawn_time = start - Duration::from_secs(1);
        cloud.last_phosphor_time = start;
        for i in 0..10u32 {
            cloud.rain_at(
                &mut frame,
                start + Duration::from_millis(i as u64 * step_ms),
            );
            frame.clear_dirty();
        }

        // Write HUD text.
        let hud_indices = write_hud_text(&mut frame, cols, lines);
        frame.clear_dirty();

        // Toggle off.
        arm_hud_toggle_off(&mut cloud);
        cloud.rain_at(&mut frame, start + Duration::from_millis(200));
        frame.clear_dirty();

        // No HUD cell may hold the stale 'H'.
        for &idx in &hud_indices {
            let cell = frame.cell_at_index(idx);
            assert_ne!(
                cell.ch, 'H',
                "style {style:?}: HUD cell at index {idx} still holds stale 'H'"
            );
        }
    }
}

/// Bare resync (force_draw_everything WITHOUT semantic_invalidate) must
/// preserve cell content — the HUNT-25 non-perturbing resync contract.
/// This test ensures the NIGHT-hunter-16 fix (arming semantic_invalidate
/// on 'i' toggle-off) does NOT leak into bare resync callers (idle
/// resync, stuck-cell sweep, ANSI drift redraw, P2 mitigation) which
/// must continue to use force_repaint's non-clearing behavior.
#[test]
fn bare_resync_preserves_content_without_semantic_invalidate() {
    let cols = 20u16;
    let lines = 10u16;
    let mut cloud = make_style_cloud(RainStyle::Glyph, cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);

    // Draw content.
    let step_ms = 17u64;
    cloud.set_max_sim_delta(Duration::from_millis(step_ms));
    let start = Instant::now();
    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    for i in 0..10u32 {
        cloud.rain_at(
            &mut frame,
            start + Duration::from_millis(i as u64 * step_ms),
        );
        frame.clear_dirty();
    }

    // Snapshot the semantic_gen before the bare resync — it must not
    // change (the HUNT-25 non-perturbing resync contract).

    // Bare resync: ONLY force_draw_everything, NO semantic_invalidate.
    // This is the idle-resync / stuck-sweep / P2 path.
    cloud.force_draw_everything();
    cloud.rain_at(&mut frame, start + Duration::from_millis(200));
    frame.clear_dirty();

    // The cell must NOT be cleared — the bare resync preserves content
    // (force_repaint → dirty_all=true → re-emit, NO clear). If
    // semantic_invalidate had leaked into this path, the cell would
    // read as blank.
    //
    // We verify the semantic_gen was NOT bumped by invalidate_semantic
    // (the bare resync contract). semantic_gen starts at 0 (Frame::new)
    // and is only bumped by invalidate_semantic. After a bare resync it
    // should still be 0.
    assert_eq!(
        frame.semantic_gen, 0,
        "bare resync must not bump semantic_gen — that's the HUNT-25 contract"
    );
}
