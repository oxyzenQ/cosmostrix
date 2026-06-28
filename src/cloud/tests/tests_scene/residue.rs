// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Residue cleanup tests — monolith residue after glyph switch, cycle
//! residue guards, depth lab scene switch residue, glyph→monolith clean render.

use std::time::{Duration, Instant};

use super::{has_dirty_cells, make_glyph_cloud, make_monolith_cloud};
use crate::frame::Frame;
use crate::rain_style::RainStyle;

#[test]
fn no_monolith_residue_after_switching_to_glyph() {
    let mut cloud = make_monolith_cloud();
    // Activate some monolith streams
    cloud.monolith_rain.reset(40, false);
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    // Switch to matrix
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    // Verify draw history is fully cleared
    assert_eq!(cloud.monolith_rain.draw_history_count_for_test(), 0);
    assert_eq!(cloud.monolith_rain.drawn_cells_for_test().len(), 0);
    // Droplets should be warm-started (non-empty) for immediate glyph visibility
    assert!(
        !cloud.droplets.is_empty(),
        "glyph pool should be warm-started after switch"
    );
    assert!(
        cloud.droplets.iter().any(|d| d.is_alive),
        "warm-started pool should have active droplets"
    );
    // Run a rain frame to verify no monolith residue
    cloud.last_spawn_time = Instant::now() - Duration::from_secs(1);
    cloud.rain(&mut frame);
    // Should still have no monolith drawn cells
    assert_eq!(cloud.monolith_rain.draw_history_count_for_test(), 0);
}

/// No monolith draw residue persists after switching to any glyph scene.
#[test]
fn no_monolith_residue_glyph_to_glyph_to_monolith_cycle() {
    let mut cloud = make_monolith_cloud();
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    // monolith → matrix
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    assert_eq!(cloud.monolith_rain.draw_history_count_for_test(), 0);
    // matrix → signal (glyph→glyph transition)
    cloud.apply_scene_runtime("signal", "code", &[], false);
    assert_eq!(cloud.monolith_rain.draw_history_count_for_test(), 0);
    // signal → monolith
    cloud.apply_scene_runtime("monolith", "code", &[], false);
    assert_eq!(cloud.rain_style(), RainStyle::Monolith);
    // Rain a few frames and verify monolith works cleanly
    let now = Instant::now();
    for i in 0..3 {
        cloud.rain_at(&mut frame, now + Duration::from_millis(i * 16));
    }
    assert!(
        has_dirty_cells(&frame),
        "monolith should render after full cycle"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// v4.5.0 Phase 3 — Scene Switch Depth Regression Lab
//
// These tests verify that scene transitions preserve visual identity:
// monolith → glyph clears all monolith-specific residue, and glyph →
// monolith reinitializes cleanly. No stale artifacts, no blank screens.
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn depth_lab_scene_switch_monolith_to_matrix_clears_phosphor() {
    // After running monolith rain, switching to matrix must clear monolith
    // phosphor state so no segmented monolith residue bleeds into glyph mode.
    let mut cloud = make_monolith_cloud();
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    let start = Instant::now();
    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;

    // Run monolith for 30 frames
    for i in 0..30 {
        cloud.rain_at(&mut frame, start + Duration::from_millis(16 * i));
        frame.clear_dirty();
    }

    // Switch to matrix — must clear monolith state
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    assert_eq!(cloud.monolith_rain.draw_history_count_for_test(), 0);
    assert_eq!(cloud.rain_style(), RainStyle::Glyph);

    // Run 10 more frames in matrix mode
    cloud.last_spawn_time = Instant::now();
    for i in 0..10 {
        cloud.rain_at(&mut frame, Instant::now() + Duration::from_millis(16 * i));
        frame.clear_dirty();
    }

    // Monolith draw history must remain empty
    assert_eq!(cloud.monolith_rain.draw_history_count_for_test(), 0);
}

#[test]
fn depth_lab_scene_switch_monolith_to_signal_clears_drawn_cells() {
    // Switching monolith → signal must clear all drawn cells immediately.
    let mut cloud = make_monolith_cloud();
    cloud.monolith_rain.reset(40, false);
    let _frame = Frame::new(40, 20, cloud.palette.bg);

    cloud.apply_scene_runtime("signal", "binary", &[], false);
    assert_eq!(cloud.monolith_rain.drawn_cells_for_test().len(), 0);
}

#[test]
fn depth_lab_scene_switch_glyph_to_monolith_renders_clean() {
    // Switching from glyph to monolith must render a clean monolith frame
    // without any leftover glyph droplet residue in the monolith draw path.
    let mut cloud = make_glyph_cloud();
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    let start = Instant::now();

    // Run glyph rain
    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.rain_at(&mut frame, start);

    // Switch to monolith
    cloud.apply_scene_runtime("monolith", "binary", &[], false);
    assert_eq!(cloud.rain_style(), RainStyle::Monolith);
    frame.clear_dirty();

    // Run monolith rain
    cloud.last_spawn_time = Instant::now();
    cloud.rain_at(&mut frame, Instant::now() + Duration::from_millis(16));
    assert!(has_dirty_cells(&frame), "monolith must render after switch");
}

#[test]
fn depth_lab_repeated_cycle_never_accumulates_residue() {
    // Cycle through all scenes 5 times and verify no accumulated residue.
    let mut cloud = make_monolith_cloud();
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    let scenes = ["matrix", "signal", "monolith"];
    let start = Instant::now();

    for round in 0..5 {
        for scene in &scenes {
            cloud.apply_scene_runtime(scene, "binary", &[], false);
            frame.clear_dirty();
            cloud.last_spawn_time = start + Duration::from_millis(round * 500);
            cloud.rain(&mut frame);

            if *scene == "monolith" {
                // After monolith, history should be populated during rain
                // but cleared when switching away
            } else {
                // Glyph modes: monolith history must be empty
                assert_eq!(
                    cloud.monolith_rain.draw_history_count_for_test(),
                    0,
                    "round {round}, scene '{scene}': monolith history should be empty"
                );
            }
        }
    }
}
