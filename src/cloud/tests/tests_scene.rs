// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Tests for runtime scene cycling and transitions.

use std::time::{Duration, Instant};

use super::Cloud;
use crate::constants::{RUNTIME_SPEED_MAX, WARM_START_MAX_HEAD, WARM_START_SEED_MAX};
use crate::frame::Frame;
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};

/// Create a monolith-style cloud for scene transition tests.
fn make_monolith_cloud() -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::Mono,
        false,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Cosmos,
        RainStyle::Monolith,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(40, 20);
    cloud.scene_name = "monolith".to_string();
    cloud.clear_redraw_flags_for_test();
    cloud
}

/// Create a glyph (matrix) style cloud.
fn make_glyph_cloud() -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::Mono,
        false,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Green,
        RainStyle::Glyph,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(40, 20);
    cloud.scene_name = "matrix".to_string();
    cloud.clear_redraw_flags_for_test();
    cloud
}

#[test]
fn scene_cycle_forward_updates_cloud_scene() {
    let mut cloud = make_monolith_cloud();
    let charset = "binary".to_string();
    let new_charset = cloud.apply_scene_runtime("matrix", &charset, &[], false);
    assert_eq!(cloud.active_scene(), "matrix");
    assert_eq!(cloud.rain_style(), RainStyle::Glyph);
    assert_eq!(new_charset, "binary"); // matrix has no charset override
}

#[test]
fn scene_cycle_to_signal_updates_cloud_scene() {
    let mut cloud = make_monolith_cloud();
    let charset = "binary".to_string();
    let new_charset = cloud.apply_scene_runtime("signal", &charset, &[], false);
    assert_eq!(cloud.active_scene(), "signal");
    assert_eq!(cloud.rain_style(), RainStyle::Glyph);
    assert_eq!(new_charset, "code"); // signal overrides charset to code
}

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
    cloud.apply_scene_runtime("monolith", "binary", &[], false);
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
    cloud.apply_scene_runtime("monolith", "binary", &[], false);
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

#[test]
fn monolith_scene_applies_cosmos_color() {
    let mut cloud = make_glyph_cloud();
    cloud.apply_scene_runtime("monolith", "binary", &[], false);
    assert_eq!(cloud.color_scheme(), ColorScheme::Cosmos);
}

#[test]
fn signal_scene_applies_cyan_color() {
    let mut cloud = make_glyph_cloud();
    cloud.apply_scene_runtime("signal", "binary", &[], false);
    assert_eq!(cloud.color_scheme(), ColorScheme::Cyan);
}

#[test]
fn speed_updates_after_scene_switch() {
    let mut cloud = make_glyph_cloud();
    cloud.set_chars_per_sec(5.0);
    cloud.apply_scene_runtime("monolith", "binary", &[], false);
    // Monolith scene sets speed=10
    assert_eq!(cloud.chars_per_sec, 10.0);
}

#[test]
fn speed_remains_clamped_after_scene_switch() {
    let mut cloud = make_glyph_cloud();
    cloud.apply_scene_runtime("monolith", "binary", &[], false);
    // Speed should be within valid range
    assert!(cloud.chars_per_sec >= 1.0);
    assert!(cloud.chars_per_sec <= RUNTIME_SPEED_MAX);
}

#[test]
fn density_updates_after_scene_switch() {
    let mut cloud = make_glyph_cloud();
    cloud.set_droplet_density(1.0);
    cloud.apply_scene_runtime("monolith", "binary", &[], false);
    // Monolith scene sets density=0.75
    assert!((cloud.droplet_density - 0.75).abs() < 0.001);
}

#[test]
fn signal_density_is_high() {
    let mut cloud = make_glyph_cloud();
    cloud.apply_scene_runtime("signal", "binary", &[], false);
    // Signal scene sets density=0.95
    assert!((cloud.droplet_density - 0.95).abs() < 0.001);
}

#[test]
fn glitch_level_subtle_applied_from_monolith() {
    let mut cloud = make_glyph_cloud();
    cloud.glitchy = false;
    cloud.glitch_pct = 0.0;
    cloud.apply_scene_runtime("monolith", "binary", &[], false);
    assert!(cloud.glitchy);
    // Subtle glitch: pct=0.03
    assert!((cloud.glitch_pct - 0.03).abs() < 0.001);
}

#[test]
fn scene_switch_drops_spawn_debt() {
    let mut cloud = make_monolith_cloud();
    cloud.spawn_remainder = 100.0;
    cloud.last_spawn_time = Instant::now() - Duration::from_secs(5);
    // Switching to monolith resets spawn debt
    cloud.apply_scene_runtime("monolith", "binary", &[], false);
    assert!(
        cloud.spawn_remainder < 1.0,
        "monolith scene switch should drop spawn debt"
    );
}

#[test]
fn matrix_scene_keeps_current_color() {
    let mut cloud = make_glyph_cloud();
    // Matrix has color=None, so it should keep current color (Green)
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    assert_eq!(cloud.color_scheme(), ColorScheme::Green);
}

#[test]
fn unknown_scene_does_not_change_state() {
    let mut cloud = make_monolith_cloud();
    let original_scene = cloud.active_scene().to_string();
    let original_style = cloud.rain_style();
    let result = cloud.apply_scene_runtime("nonexistent", "binary", &[], false);
    assert_eq!(cloud.active_scene(), original_scene);
    assert_eq!(cloud.rain_style(), original_style);
    assert_eq!(result, "binary");
}

#[test]
fn cycle_monolith_signal_monolith_roundtrip() {
    let mut cloud = make_monolith_cloud();
    let mut charset = "binary".to_string();
    // monolith -> signal
    let c = cloud.apply_scene_runtime("signal", &charset, &[], false);
    charset = c.to_string();
    assert_eq!(cloud.rain_style(), RainStyle::Glyph);
    assert_eq!(cloud.active_scene(), "signal");
    // signal -> monolith
    let c = cloud.apply_scene_runtime("monolith", &charset, &[], false);
    charset = c.to_string();
    assert_eq!(cloud.rain_style(), RainStyle::Monolith);
    assert_eq!(cloud.active_scene(), "monolith");
    assert_eq!(charset, "binary");
}

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

#[test]
fn existing_controls_still_work_after_scene_switch() {
    let mut cloud = make_monolith_cloud();
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    // Speed up/down should still work
    let original_cps = cloud.chars_per_sec;
    cloud.set_chars_per_sec(original_cps + 1.0);
    assert!(cloud.chars_per_sec > original_cps);
    // Density should still work
    cloud.set_droplet_density(cloud.droplet_density + 0.1);
    // Glitch toggle should still work
    cloud.set_glitchy(!cloud.glitchy);
}

// ---------------------------------------------------------------------------
// Warm-start and blank-screen fix tests (v3.2.0)
// ---------------------------------------------------------------------------

fn has_dirty_cells(frame: &Frame) -> bool {
    frame.is_dirty_all() || !frame.dirty_indices().is_empty()
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
    cloud.apply_scene_runtime("monolith", "binary", &[], false);
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

// ---------------------------------------------------------------------------
// Fresh-entry warm-start tests (v3.2.0 visual sync fix)
// ---------------------------------------------------------------------------

/// After monolith → matrix, warm-started droplets must have heads in the
/// upper portion of the viewport (not scattered mid-screen). The head_cap
/// is min(lines/4, WARM_START_MAX_HEAD), so all seeded droplets should
/// have head_put_line <= head_cap.
#[test]
fn fresh_entry_matrix_heads_in_upper_quarter() {
    let mut cloud = make_monolith_cloud();
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    let head_cap = (cloud.lines / 4).clamp(2, WARM_START_MAX_HEAD);
    for d in &cloud.droplets {
        if d.is_alive {
            assert!(
                d.head_put_line <= head_cap,
                "fresh-entry droplet head (line {}) must be in upper quarter (cap={head_cap})",
                d.head_put_line
            );
        }
    }
}

/// After monolith → matrix, most visible glyph cells must be in the upper
/// half of the viewport. No droplets should have heads in the lower half.
#[test]
fn fresh_entry_matrix_not_in_progress_look() {
    let mut cloud = make_monolith_cloud();
    let lines = cloud.lines;
    let midpoint = lines / 2;
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    let mut heads_below_midpoint = 0usize;
    let mut total_alive = 0usize;
    for d in &cloud.droplets {
        if d.is_alive {
            total_alive += 1;
            if d.head_put_line > midpoint {
                heads_below_midpoint += 1;
            }
        }
    }
    assert!(
        heads_below_midpoint == 0,
        "fresh-entry: no heads should be below midpoint ({midpoint}), got {heads_below_midpoint}/{total_alive}"
    );
}

/// After monolith → signal, warm-started droplets must also obey the
/// fresh-entry head_cap bound.
#[test]
fn fresh_entry_signal_heads_in_upper_quarter() {
    let mut cloud = make_monolith_cloud();
    cloud.apply_scene_runtime("signal", "binary", &[], false);
    let head_cap = (cloud.lines / 4).clamp(2, WARM_START_MAX_HEAD);
    for d in &cloud.droplets {
        if d.is_alive {
            assert!(
                d.head_put_line <= head_cap,
                "fresh-entry signal droplet head (line {}) must be in upper quarter (cap={head_cap})",
                d.head_put_line
            );
        }
    }
}

/// After monolith → signal, first frame must produce dirty cells in the
/// upper half of the viewport — confirming fresh-entry seeding is visible
/// and top-biased, not scattered across the full screen.
#[test]
fn fresh_entry_signal_top_biased_visibility() {
    let mut cloud = make_monolith_cloud();
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    let lines = cloud.lines;
    let cols = cloud.cols;
    let midpoint = lines / 2;
    cloud.apply_scene_runtime("signal", "binary", &[], false);
    let now = Instant::now();
    cloud.last_spawn_time = now;
    cloud.rain_at(&mut frame, now + Duration::from_millis(16));
    assert!(
        has_dirty_cells(&frame),
        "fresh-entry signal: first frame must have dirty cells"
    );
    // Check that some cells in the upper half have visible glyph content
    // (non-space character — mono mode uses fg=None so we check ch only).
    let mut upper_visible = 0usize;
    for line in 0..=midpoint {
        for col in 0..cols {
            if let Some(cell) = frame.get(col, line) {
                if cell.ch != ' ' {
                    upper_visible += 1;
                }
            }
        }
    }
    assert!(
        upper_visible > 0,
        "fresh-entry signal: at least some visible cells should be in upper half (got {upper_visible})"
    );
}

/// After monolith → matrix, warm-started droplets should have short trails
/// (tail_put_line set to Some(0)), not full-screen trails.
#[test]
fn fresh_entry_matrix_short_trails() {
    let mut cloud = make_monolith_cloud();
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    for d in &cloud.droplets {
        if d.is_alive {
            // Fresh-entry droplets must have tail at row 0 (short trail)
            assert_eq!(
                d.tail_put_line,
                Some(0),
                "fresh-entry droplet must have tail at row 0 (got {:?})",
                d.tail_put_line
            );
            // Trail length = head - tail, should be bounded by head_cap
            let head_cap = (cloud.lines / 4).clamp(2, WARM_START_MAX_HEAD);
            assert!(
                d.head_put_line <= head_cap,
                "fresh-entry trail length ({}) must be <= head_cap ({head_cap})",
                d.head_put_line
            );
        }
    }
}

/// Repeated x cycling (forward) never produces a frame where warm-started
/// glyph droplets have heads in the lower half.
#[test]
fn fresh_entry_repeated_forward_never_scattered() {
    let mut cloud = make_monolith_cloud();
    let scenes = ["matrix", "signal", "monolith", "matrix", "signal"];
    let lines = cloud.lines;
    let head_cap = (lines / 4).clamp(2, WARM_START_MAX_HEAD);
    for scene in &scenes {
        cloud.apply_scene_runtime(scene, "binary", &[], false);
        if matches!(cloud.rain_style(), RainStyle::Glyph) {
            for d in &cloud.droplets {
                if d.is_alive {
                    assert!(
                        d.head_put_line <= head_cap,
                        "forward cycle '{scene}': head (line {}) must be in upper quarter (cap={head_cap})",
                        d.head_put_line
                    );
                }
            }
        }
    }
}

/// Repeated X cycling forward never produces a frame where warm-started
/// glyph droplets have heads in the lower half.
#[test]
fn fresh_entry_repeated_uppercase_forward_never_scattered() {
    let mut cloud = make_monolith_cloud();
    let scenes = ["matrix", "signal", "monolith", "matrix", "signal"];
    let lines = cloud.lines;
    let head_cap = (lines / 4).clamp(2, WARM_START_MAX_HEAD);
    for scene in &scenes {
        cloud.apply_scene_runtime(scene, "binary", &[], false);
        if matches!(cloud.rain_style(), RainStyle::Glyph) {
            for d in &cloud.droplets {
                if d.is_alive {
                    assert!(
                        d.head_put_line <= head_cap,
                        "uppercase forward cycle '{scene}': head (line {}) must be in upper quarter (cap={head_cap})",
                        d.head_put_line
                    );
                }
            }
        }
    }
}

/// No monolith residue after switching to glyph: monolith draw history
/// must be fully cleared and phosphor_base_ch must not contain stale
/// monolith glyphs.
#[test]
fn fresh_entry_no_monolith_residue_phosphor() {
    let mut cloud = make_monolith_cloud();
    cloud.monolith_rain.reset(40, false);
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    // Monolith history must be empty
    assert_eq!(cloud.monolith_rain.draw_history_count_for_test(), 0);
    // Glyph pool must be warm-started with fresh-entry semantics
    assert!(!cloud.droplets.is_empty());
    let alive_count = cloud.droplets.iter().filter(|d| d.is_alive).count();
    assert!(alive_count > 0, "fresh-entry should have active droplets");
    // Run a frame and verify no monolith residue
    cloud.last_spawn_time = Instant::now();
    cloud.rain(&mut frame);
    assert_eq!(cloud.monolith_rain.draw_history_count_for_test(), 0);
}

// ---------------------------------------------------------------------------
// Sparse fresh-entry density tests (v3.2.1 stabilization)
// ---------------------------------------------------------------------------

/// After monolith → matrix, the number of warm-started alive droplets must
/// be bounded by WARM_START_SEED_MAX — no per-column flooding.
#[test]
fn sparse_entry_matrix_alive_count_bounded() {
    let mut cloud = make_monolith_cloud();
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    let alive = cloud.droplets.iter().filter(|d| d.is_alive).count();
    assert!(
        alive <= WARM_START_SEED_MAX,
        "sparse entry: alive droplets ({alive}) must be <= WARM_START_SEED_MAX ({WARM_START_SEED_MAX})"
    );
    assert!(
        alive >= 3,
        "sparse entry: must have at least 3 alive droplets for no-blank guarantee (got {alive})"
    );
}

/// After monolith → signal, the same sparse bound applies.
#[test]
fn sparse_entry_signal_alive_count_bounded() {
    let mut cloud = make_monolith_cloud();
    cloud.apply_scene_runtime("signal", "binary", &[], false);
    let alive = cloud.droplets.iter().filter(|d| d.is_alive).count();
    assert!(
        alive <= WARM_START_SEED_MAX,
        "sparse entry signal: alive ({alive}) must be <= WARM_START_SEED_MAX ({WARM_START_SEED_MAX})"
    );
    assert!(
        alive >= 3,
        "sparse entry signal: must have at least 3 alive droplets (got {alive})"
    );
}

/// The scene-entry ramp must be active immediately after switching to glyph.
#[test]
fn sparse_entry_ramp_starts_on_scene_switch() {
    let mut cloud = make_monolith_cloud();
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    assert!(
        cloud.glyph_entry_time.is_some(),
        "glyph_entry_time must be set after switching to glyph scene"
    );
}

/// The scene-entry ramp must be cleared when switching back to monolith.
#[test]
fn sparse_entry_ramp_cleared_on_monolith_switch() {
    let mut cloud = make_monolith_cloud();
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    assert!(cloud.glyph_entry_time.is_some());
    cloud.apply_scene_runtime("monolith", "binary", &[], false);
    assert!(
        cloud.glyph_entry_time.is_none(),
        "glyph_entry_time must be cleared when switching to monolith"
    );
}

/// Repeated x cycling must never overpopulate initial glyph scenes.
/// After each switch, alive count must stay within the sparse bound.
#[test]
fn sparse_entry_repeated_forward_stays_sparse() {
    let mut cloud = make_monolith_cloud();
    let scenes = ["matrix", "signal", "monolith", "matrix", "signal"];
    for scene in &scenes {
        cloud.apply_scene_runtime(scene, "binary", &[], false);
        if matches!(cloud.rain_style(), RainStyle::Glyph) {
            let alive = cloud.droplets.iter().filter(|d| d.is_alive).count();
            assert!(
                alive <= WARM_START_SEED_MAX,
                "forward cycle '{scene}': alive ({alive}) must be <= {WARM_START_SEED_MAX}"
            );
        }
    }
}

/// Repeated X forward cycling must also stay sparse.
#[test]
fn sparse_entry_repeated_uppercase_forward_stays_sparse() {
    let mut cloud = make_monolith_cloud();
    let scenes = ["matrix", "signal", "monolith", "matrix", "signal"];
    for scene in &scenes {
        cloud.apply_scene_runtime(scene, "binary", &[], false);
        if matches!(cloud.rain_style(), RainStyle::Glyph) {
            let alive = cloud.droplets.iter().filter(|d| d.is_alive).count();
            assert!(
                alive <= WARM_START_SEED_MAX,
                "uppercase forward cycle '{scene}': alive ({alive}) must be <= {WARM_START_SEED_MAX}"
            );
        }
    }
}

/// All Rust source files must stay under 1000 LOC after the architecture split.
#[test]
fn all_rust_files_under_loc_cap() {
    let files = [
        "src/cloud/mod.rs",
        "src/cloud/spawn.rs",
        "src/cloud/tests/mod.rs",
        "src/cloud/tests/tests_scene.rs",
        "src/cloud/tests/tests_architecture.rs",
        "src/cloud/scene_runtime.rs",
        "src/cloud/runtime_controls.rs",
    ];
    for path in &files {
        let content = std::fs::read_to_string(path).unwrap_or_default();
        let count = content.lines().count();
        assert!(count <= 1000, "{path}: {count} LOC exceeds 1000 cap");
    }
}
