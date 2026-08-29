// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! P4 sweep tests for the cloud module's phosphor orphan-glyph sweeper.
//!
//! Extracted from `tests_phosphor.rs` to keep that source file under the
//! 800-LOC cap. Pure code motion — no behavior change.

use std::time::{Duration, Instant};

use crossterm::style::Color;

use super::make_cloud;
use crate::cell::Cell;
use crate::cloud::Cloud;
use crate::constants::{STUCK_CELL_MAX_PER_SWEEP, STUCK_CELL_SWEEP_INTERVAL_FRAMES};
use crate::frame::Frame;

fn make_cloud_with_timing() -> Cloud {
    let mut cloud = make_cloud();
    cloud.enable_component_timing = true;
    cloud
}

/// Helper: count cells in the frame that have a visible glyph (fg.is_some()).
fn count_glyph_cells(frame: &Frame, cols: u16, lines: u16) -> usize {
    let mut count = 0;
    for line in 0..lines {
        for col in 0..cols {
            if frame
                .get(col, line)
                .map(|c| c.fg.is_some())
                .unwrap_or(false)
            {
                count += 1;
            }
        }
    }
    count
}

#[test]
fn p4_sweep_skips_when_component_timing_disabled() {
    // Production interactive mode (no --perf-stats) → sweep is a no-op.
    let mut cloud = make_cloud(); // timing disabled by default
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // Plant a stuck cell manually.
    let stuck_cell = Cell {
        ch: 'X',
        fg: Some(Color::Green),
        bg: cloud.palette.bg,
        bold: false,
    };
    frame.set(0, 0, stuck_cell);
    // Ensure phosphor at (0,0) is 0 so the cell qualifies as stuck.
    if !cloud.phosphor.is_empty() {
        cloud.phosphor[0] = 0;
    }
    // Pre-condition: the cell has a glyph.
    assert!(frame.get(0, 0).unwrap().fg.is_some());

    // Bump the counter past the threshold to confirm the gate is on
    // enable_component_timing, not just the counter.
    cloud.frames_since_stuck_sweep = STUCK_CELL_SWEEP_INTERVAL_FRAMES + 1;
    cloud.stuck_cell_sweep(&mut frame);

    // The cell should still be there — sweep didn't run.
    assert!(
        frame.get(0, 0).unwrap().fg.is_some(),
        "sweep must be a no-op when enable_component_timing is false"
    );
}

#[test]
fn p4_sweep_skips_when_message_active() {
    // When a message box is visible, overlay cells would be false positives.
    let mut cloud = make_cloud_with_timing();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // Plant a stuck cell.
    let stuck_cell = Cell {
        ch: 'X',
        fg: Some(Color::Green),
        bg: cloud.palette.bg,
        bold: false,
    };
    frame.set(0, 0, stuck_cell);
    if !cloud.phosphor.is_empty() {
        cloud.phosphor[0] = 0;
    }

    // Activate a message box.
    cloud.set_message("hello");
    cloud.frames_since_stuck_sweep = STUCK_CELL_SWEEP_INTERVAL_FRAMES + 1;
    cloud.stuck_cell_sweep(&mut frame);

    // The cell should still be there — sweep skipped due to message.
    assert!(
        frame.get(0, 0).unwrap().fg.is_some(),
        "sweep must skip when a message box is active"
    );
}

#[test]
fn p4_sweep_skips_when_interval_not_elapsed() {
    // Counter gate: sweep only fires after STUCK_CELL_SWEEP_INTERVAL_FRAMES.
    let mut cloud = make_cloud_with_timing();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    let stuck_cell = Cell {
        ch: 'X',
        fg: Some(Color::Green),
        bg: cloud.palette.bg,
        bold: false,
    };
    frame.set(0, 0, stuck_cell);
    if !cloud.phosphor.is_empty() {
        cloud.phosphor[0] = 0;
    }

    // Two frames shy of the threshold: counter will bump to THRESHOLD - 1
    // inside the sweep call, which is still < THRESHOLD → sweep does not fire.
    cloud.frames_since_stuck_sweep = STUCK_CELL_SWEEP_INTERVAL_FRAMES - 2;
    cloud.stuck_cell_sweep(&mut frame);

    assert!(
        frame.get(0, 0).unwrap().fg.is_some(),
        "sweep must not fire before STUCK_CELL_SWEEP_INTERVAL_FRAMES"
    );
    // Counter should have advanced by 1 (still below threshold).
    assert_eq!(
        cloud.frames_since_stuck_sweep,
        STUCK_CELL_SWEEP_INTERVAL_FRAMES - 1,
        "counter should advance by 1 each call when below threshold"
    );
}

#[test]
fn p4_sweep_clears_orphan_glyph() {
    // Synthesize a stuck cell: has fg, current_gen, no phosphor, no droplet
    // covers it. The sweep should clear it.
    let mut cloud = make_cloud_with_timing();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // Pick a column with no active droplet. Run rain first to populate
    // droplets, then find a free column.
    cloud.last_spawn_time = Instant::now() - Duration::from_secs(1);
    cloud.rain_at(&mut frame, Instant::now());
    frame.clear_dirty();

    // Find a column not used by any living droplet.
    let mut free_col = 0u16;
    'outer: for col in 0..cloud.cols {
        for d in &cloud.droplets {
            if d.is_alive && d.bound_col == col {
                continue 'outer;
            }
        }
        free_col = col;
        break;
    }

    // Plant a stuck cell at (free_col, 0).
    let stuck_cell = Cell {
        ch: 'Z',
        fg: Some(Color::Red),
        bg: cloud.palette.bg,
        bold: true,
    };
    frame.set(free_col, 0, stuck_cell);

    // Ensure phosphor at (free_col, 0) is 0.
    let lines = cloud.lines as usize;
    let pidx = free_col as usize * lines;
    if pidx < cloud.phosphor.len() {
        cloud.phosphor[pidx] = 0;
    }

    // Pre-condition: the cell has a glyph.
    assert!(frame.get(free_col, 0).unwrap().fg.is_some());

    // Force the sweep to fire.
    cloud.frames_since_stuck_sweep = STUCK_CELL_SWEEP_INTERVAL_FRAMES;
    cloud.stuck_cell_sweep(&mut frame);

    // The stuck cell should be cleared.
    assert!(
        frame.get(free_col, 0).unwrap().fg.is_none(),
        "sweep should clear the orphan glyph at ({}, 0)",
        free_col
    );
    // And force_draw_everything should be set so the cleared cell is emitted.
    assert!(
        cloud.force_draw_everything,
        "sweep should set force_draw_everything after clearing stuck cells"
    );
}

#[test]
fn p4_sweep_preserves_droplet_covered_cells() {
    // Cells within a droplet's visible trail must NOT be cleared.
    let mut cloud = make_cloud_with_timing();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // Run rain to spawn droplets that write cells.
    cloud.last_spawn_time = Instant::now() - Duration::from_secs(1);
    cloud.rain_at(&mut frame, Instant::now());

    // Find a living droplet and a cell it covers.
    let living: Vec<_> = cloud.droplets.iter().filter(|d| d.is_alive).collect();
    if living.is_empty() {
        return; // No droplets spawned — nothing to verify.
    }
    let d = living[0];
    let covered_line = d.head_put_line;
    let covered_col = d.bound_col;

    // Confirm the cell has a glyph (was written by the droplet).
    let cell = frame.get(covered_col, covered_line).unwrap();
    if cell.fg.is_none() {
        return; // Edge case: cell wasn't written (e.g., head at 0 with no advance).
    }

    // Force the sweep to fire.
    cloud.frames_since_stuck_sweep = STUCK_CELL_SWEEP_INTERVAL_FRAMES;
    cloud.stuck_cell_sweep(&mut frame);

    // The droplet-covered cell should still be there.
    let cell_after = frame.get(covered_col, covered_line).unwrap();
    assert!(
        cell_after.fg.is_some(),
        "sweep must not clear cells covered by an active droplet at ({}, {})",
        covered_col,
        covered_line
    );
}

#[test]
fn p4_sweep_respects_max_per_sweep_cap() {
    // Plant more stuck cells than STUCK_CELL_MAX_PER_SWEEP and verify the
    // sweep stops at the cap.
    let mut cloud = make_cloud_with_timing();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // Run rain to populate droplets, then clear the frame and plant many
    // stuck cells in columns with no droplet.
    cloud.last_spawn_time = Instant::now() - Duration::from_secs(1);
    cloud.rain_at(&mut frame, Instant::now());

    // Identify columns NOT used by any living droplet.
    let busy_cols: std::collections::HashSet<u16> = cloud
        .droplets
        .iter()
        .filter(|d| d.is_alive)
        .map(|d| d.bound_col)
        .collect();
    let free_cols: Vec<u16> = (0..cloud.cols).filter(|c| !busy_cols.contains(c)).collect();

    // Need at least MAX+1 free columns to verify the cap.
    let needed = STUCK_CELL_MAX_PER_SWEEP + 1;
    if free_cols.len() < needed {
        // Resize the cloud to ensure enough free columns.
        cloud.reset(80, 30);
        cloud.last_spawn_time = Instant::now() - Duration::from_secs(1);
        let mut f2 = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        cloud.rain_at(&mut f2, Instant::now());
        let busy: std::collections::HashSet<u16> = cloud
            .droplets
            .iter()
            .filter(|d| d.is_alive)
            .map(|d| d.bound_col)
            .collect();
        let free_cols2: Vec<u16> = (0..cloud.cols).filter(|c| !busy.contains(c)).collect();
        if free_cols2.len() < needed {
            eprintln!(
                "[p4_sweep_respects_max_per_sweep_cap] could not find {} free columns (got {}); skipping",
                needed,
                free_cols2.len()
            );
            return;
        }
        // Plant stuck cells across the free columns at line 0.
        let lines = cloud.lines as usize;
        for &col in &free_cols2[..needed] {
            frame.set(
                col,
                0,
                Cell {
                    ch: 'Z',
                    fg: Some(Color::Red),
                    bg: cloud.palette.bg,
                    bold: false,
                },
            );
            let pidx = col as usize * lines;
            if pidx < cloud.phosphor.len() {
                cloud.phosphor[pidx] = 0;
            }
        }
        cloud.frames_since_stuck_sweep = STUCK_CELL_SWEEP_INTERVAL_FRAMES;
        cloud.stuck_cell_sweep(&mut frame);
        // The sweep clears at most MAX cells. Some stuck cells remain.
        let remaining = free_cols2[..needed]
            .iter()
            .filter(|&&col| frame.get(col, 0).unwrap().fg.is_some())
            .count();
        assert!(
            remaining >= 1,
            "sweep should leave at least 1 stuck cell when cap is hit (got {} cleared of {} planted)",
            needed - remaining,
            needed
        );
        return;
    }

    // Plant stuck cells across the free columns at line 0.
    let lines = cloud.lines as usize;
    for &col in &free_cols[..needed] {
        frame.set(
            col,
            0,
            Cell {
                ch: 'Z',
                fg: Some(Color::Red),
                bg: cloud.palette.bg,
                bold: false,
            },
        );
        let pidx = col as usize * lines;
        if pidx < cloud.phosphor.len() {
            cloud.phosphor[pidx] = 0;
        }
    }

    cloud.frames_since_stuck_sweep = STUCK_CELL_SWEEP_INTERVAL_FRAMES;
    cloud.stuck_cell_sweep(&mut frame);

    // The sweep clears at most MAX cells. Some stuck cells remain.
    let remaining = free_cols[..needed]
        .iter()
        .filter(|&&col| frame.get(col, 0).unwrap().fg.is_some())
        .count();
    assert!(
        remaining >= 1,
        "sweep should leave at least 1 stuck cell when cap is hit (got {} cleared of {} planted)",
        needed - remaining,
        needed
    );
}

#[test]
fn p4_sweep_no_op_when_no_stuck_cells() {
    // When the frame is clean (no stuck cells), the sweep is a no-op and
    // force_draw_everything is NOT set.
    let mut cloud = make_cloud_with_timing();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // Run rain normally — no stuck cells planted.
    cloud.last_spawn_time = Instant::now() - Duration::from_secs(1);
    cloud.rain_at(&mut frame, Instant::now());
    let before = count_glyph_cells(&frame, cloud.cols, cloud.lines);

    cloud.frames_since_stuck_sweep = STUCK_CELL_SWEEP_INTERVAL_FRAMES;
    cloud.force_draw_everything = false;
    cloud.stuck_cell_sweep(&mut frame);

    let after = count_glyph_cells(&frame, cloud.cols, cloud.lines);
    assert_eq!(
        before, after,
        "sweep should not clear any cells when none are stuck"
    );
    assert!(
        !cloud.force_draw_everything,
        "sweep should not set force_draw_everything when no stuck cells were found"
    );
}
