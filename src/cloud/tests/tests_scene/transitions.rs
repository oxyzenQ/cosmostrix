// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Scene transition tests — monolith↔glyph switches, dirty-frame behavior,
//! semantic invalidation, spawn debt clearing, force draw.

use std::time::{Duration, Instant};

use super::{has_dirty_cells, make_glyph_cloud, make_monolith_cloud};
use crate::frame::Frame;
use crate::rain_style::RainStyle;

#[test]
fn monolith_to_matrix_changes_rain_style_to_glyph() {
    let mut cloud = make_monolith_cloud();
    assert_eq!(cloud.rain_style(), RainStyle::Monolith);
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    assert_eq!(cloud.rain_style(), RainStyle::Glyph);
}

#[test]
fn matrix_to_monolith_changes_rain_style_to_monolith() {
    let mut cloud = make_glyph_cloud();
    assert_eq!(cloud.rain_style(), RainStyle::Glyph);
    cloud.apply_scene_runtime("monolith", "braille", &[], false);
    assert_eq!(cloud.rain_style(), RainStyle::Monolith);
}

#[test]
fn switching_from_monolith_clears_draw_history() {
    let mut cloud = make_monolith_cloud();
    // Simulate some monolith draw activity
    cloud.monolith_rain.reset(40, false);
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    // Draw history should be empty after switching away from monolith
    assert_eq!(cloud.monolith_rain.draw_history_count_for_test(), 0);
}

#[test]
fn switching_into_monolith_initializes_state_cleanly() {
    let mut cloud = make_glyph_cloud();
    cloud.droplets.clear();
    cloud.apply_scene_runtime("monolith", "braille", &[], false);
    // Monolith should be reset and ready
    assert_eq!(cloud.rain_style(), RainStyle::Monolith);
    assert_eq!(cloud.monolith_rain.draw_history_count_for_test(), 0);
    assert_eq!(cloud.active_scene(), "monolith");
}

#[test]
fn scene_switch_requests_semantic_invalidate() {
    let mut cloud = make_monolith_cloud();
    cloud.clear_redraw_flags_for_test();
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    assert!(cloud.is_semantic_invalidate());
}

#[test]
fn scene_switch_triggers_force_draw() {
    let mut cloud = make_monolith_cloud();
    cloud.clear_redraw_flags_for_test();
    cloud.apply_scene_runtime("signal", "binary", &[], false);
    assert!(cloud.is_force_draw_everything());
}

/// Scene switch must request semantic invalidation for safe redraw sync.
#[test]
fn scene_switch_glyph_requests_semantic_sync() {
    let mut cloud = make_monolith_cloud();
    cloud.clear_redraw_flags_for_test();
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    assert!(
        cloud.is_semantic_invalidate(),
        "glyph scene switch must request semantic invalidation"
    );
}

#[test]
fn scene_switch_drops_spawn_debt() {
    let mut cloud = make_monolith_cloud();
    cloud.spawn_remainder = 100.0;
    cloud.last_spawn_time = Instant::now() - Duration::from_secs(5);
    // Switching to monolith resets spawn debt
    cloud.apply_scene_runtime("monolith", "braille", &[], false);
    assert!(
        cloud.spawn_remainder < 1.0,
        "monolith scene switch should drop spawn debt"
    );
}

/// After switching monolith → matrix, the first rain frame must produce
/// visible dirty cells — no blank black intermediate screen.
#[test]
fn monolith_to_matrix_produces_dirty_glyph_frame() {
    let mut cloud = make_monolith_cloud();
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    let now = Instant::now();
    cloud.last_spawn_time = now;
    cloud.rain_at(&mut frame, now + Duration::from_millis(16));
    assert!(
        has_dirty_cells(&frame),
        "monolith→matrix: first frame must have dirty glyph cells"
    );
}

/// After switching monolith → signal, the first rain frame must produce
/// visible dirty cells — no blank black intermediate screen.
#[test]
fn monolith_to_signal_produces_dirty_glyph_frame() {
    let mut cloud = make_monolith_cloud();
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    cloud.apply_scene_runtime("signal", "binary", &[], false);
    let now = Instant::now();
    cloud.last_spawn_time = now;
    cloud.rain_at(&mut frame, now + Duration::from_millis(16));
    assert!(
        has_dirty_cells(&frame),
        "monolith→signal: first frame must have dirty glyph cells"
    );
}

/// After switching signal → monolith, the monolith scene should render
/// correctly (monolith has its own draw path, not glyph droplets).
#[test]
fn signal_to_monolith_produces_visible_frame() {
    let mut cloud = make_glyph_cloud();
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    cloud.apply_scene_runtime("monolith", "braille", &[], false);
    let now = Instant::now();
    cloud.last_spawn_time = now;
    cloud.rain_at(&mut frame, now + Duration::from_millis(16));
    assert!(
        has_dirty_cells(&frame),
        "signal→monolith: first frame must have dirty cells"
    );
}

/// Switching monolith → matrix must clear monolith draw history so no
/// monolith segmented residue persists in the glyph scene.
#[test]
fn monolith_to_matrix_clears_monolith_history_no_blank() {
    let mut cloud = make_monolith_cloud();
    cloud.monolith_rain.reset(40, false);
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    // Monolith history must be empty
    assert_eq!(cloud.monolith_rain.draw_history_count_for_test(), 0);
    // Glyph pool must be populated (not blank)
    assert!(!cloud.droplets.is_empty());
    let alive_count = cloud.droplets.iter().filter(|d| d.is_alive).count();
    assert!(
        alive_count > 0,
        "warm-start should seed at least 1 active droplet (got {alive_count})"
    );
    // First frame must render visible content
    cloud.last_spawn_time = Instant::now();
    cloud.rain(&mut frame);
    assert!(has_dirty_cells(&frame));
}

/// Switching monolith → signal must clear monolith draw history and
/// produce visible glyph content on the first frame.
#[test]
fn monolith_to_signal_clears_monolith_history_no_blank() {
    let mut cloud = make_monolith_cloud();
    cloud.monolith_rain.reset(40, false);
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    cloud.apply_scene_runtime("signal", "binary", &[], false);
    assert_eq!(cloud.monolith_rain.draw_history_count_for_test(), 0);
    assert!(!cloud.droplets.is_empty());
    cloud.last_spawn_time = Instant::now();
    cloud.rain(&mut frame);
    assert!(has_dirty_cells(&frame));
}

/// Repeated forward cycling (x key) through all scenes never yields
/// a blank frame. Each scene transition must produce dirty cells.
#[test]
fn repeated_forward_cycle_never_blank() {
    let mut cloud = make_monolith_cloud();
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    let scenes = ["matrix", "signal", "monolith"];
    for scene in &scenes {
        cloud.apply_scene_runtime(scene, "binary", &[], false);
        frame.clear_dirty();
        cloud.last_spawn_time = Instant::now();
        cloud.rain(&mut frame);
        assert!(
            has_dirty_cells(&frame),
            "forward cycle: scene '{scene}' must produce dirty frame"
        );
    }
}

/// Repeated uppercase X cycling forward through all scenes never yields
/// a blank frame. Each scene transition must produce dirty cells.
#[test]
fn repeated_uppercase_forward_cycle_never_blank() {
    let mut cloud = make_monolith_cloud();
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    let scenes = ["matrix", "signal", "monolith"];
    for scene in &scenes {
        cloud.apply_scene_runtime(scene, "binary", &[], false);
        frame.clear_dirty();
        cloud.last_spawn_time = Instant::now();
        cloud.rain(&mut frame);
        assert!(
            has_dirty_cells(&frame),
            "uppercase forward cycle: scene '{scene}' must produce dirty frame"
        );
    }
}
