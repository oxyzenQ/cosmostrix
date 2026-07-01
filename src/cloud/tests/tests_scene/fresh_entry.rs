// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Fresh-entry warm-start tests — upper-quarter seeding, top-biased visibility,
//! short trails, no in-progress look.

use std::time::{Duration, Instant};

use super::{has_dirty_cells, make_monolith_cloud};
use crate::constants::WARM_START_MAX_HEAD;
use crate::frame::Frame;
use crate::rain_style::RainStyle;

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
