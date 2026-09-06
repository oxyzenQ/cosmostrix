// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-hunter-17 regression tests: stuck rain cells on Glyph rain.
//!
//! Owner report (2026-09-06, post-HUNT-27 a94c6021): after the HUNT-27
//! cell-level skip fix eliminated the "glitch rain shift" mass-glyph
//! flash, a new bug appeared — rain cells would get "stuck" on screen
//! for a long time (several seconds to minutes), in small clusters of
//! 3-15 cells, until another rain droplet happened to pass through
//! those exact cells and overwrote them.
//!
//! Root cause: the stuck-cell sweep (phosphor_anomaly.rs) — the
//! mechanism designed to clear cells whose content persists in
//! frame.cells without being tracked by phosphor or covered by an
//! active droplet — was gated on `enable_component_timing`, which
//! defaults to `false` and is only set to `true` by `--perf-stats`.
//! The owner ran `-v -s` (verbose + screensaver), NOT `--perf-stats`,
//! so the sweep NEVER ran. Stuck cells accumulated and were never
//! cleared.
//!
//! Fix (NIGHT-hunter-17):
//! 1. Removed the `enable_component_timing` gate from stuck_cell_sweep.
//!    The sweep now runs on every interactive session (only disabled
//!    in benchmark mode via enable_stuck_cell_sweep=false).
//! 2. The sweep now checks ALL cells (not just cells written this
//!    frame) via `frame.cells[i]` directly — a stale cell with a real
//!    glyph is the stuck-cell signature.
//! 3. Reduced STUCK_CELL_SWEEP_INTERVAL_FRAMES from 3600 (60s) to
//!    600 (10s) so stuck cells are cleared within 10s instead of 60s.

use crossterm::style::Color;

use crate::cell::Cell;
use crate::cloud::Cloud;
use crate::frame::Frame;
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};
use crate::STUCK_CELL_SWEEP_INTERVAL_FRAMES;

fn make_cloud(style: RainStyle) -> Cloud {
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
    cloud.reset(20, 10);
    cloud
}

/// The sweep must run even when enable_component_timing is false (the
/// default for interactive runs without --perf-stats). Before the fix,
/// the sweep was gated on enable_component_timing and never ran on
/// interactive sessions, leaving stuck cells permanently on screen.
#[test]
fn hunt17_sweep_runs_without_component_timing() {
    let mut cloud = make_cloud(RainStyle::Glyph);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // enable_component_timing is false by default (make_cloud doesn't
    // call set_component_timing(true)).
    assert!(!cloud.enable_component_timing, "precondition: timing off");

    // Plant a stuck cell: a glyph with no phosphor tracking and no
    // active droplet covering it.
    let stuck_cell = Cell {
        ch: 'X',
        fg: Some(Color::Green),
        bg: cloud.palette.bg,
        bold: false,
    };
    frame.set(5, 5, stuck_cell);
    if !cloud.phosphor.is_empty() {
        let pidx = 5usize * cloud.lines as usize + 5usize;
        cloud.phosphor[pidx] = 0;
    }

    // Pre-condition: the cell has a glyph.
    assert!(frame.get(5, 5).unwrap().fg.is_some());

    // Bump the counter past the threshold.
    cloud.frames_since_stuck_sweep = STUCK_CELL_SWEEP_INTERVAL_FRAMES + 1;
    cloud.stuck_cell_sweep(&mut frame);

    // The cell must be CLEARED — the sweep ran despite
    // enable_component_timing being false.
    assert!(
        frame.get(5, 5).unwrap().fg.is_none(),
        "sweep must clear stuck cells even when enable_component_timing is false"
    );
}

/// The sweep must check ALL cells, not just cells written this frame.
/// A stuck cell is a cell whose content (a visible glyph) persists in
/// frame.cells[i] but is NOT tracked by phosphor and NOT covered by an
/// active droplet. The sweep reads frame.cells[i] directly (the actual
/// stored content) so it catches cells regardless of when they were
/// last written.
#[test]
fn hunt17_sweep_checks_all_cells() {
    let mut cloud = make_cloud(RainStyle::Glyph);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // Plant a stuck cell: a glyph with no phosphor tracking and no
    // active droplet covering it. This simulates a cell that was
    // written by a droplet that has since died and cleared its trail,
    // but the cell content was not cleared (e.g., a phosphor ghost
    // that reached zero energy without being properly removed, or a
    // cell that fell through the phosphor tracking gap).
    let stuck_cell = Cell {
        ch: 'Y',
        fg: Some(Color::Red),
        bg: cloud.palette.bg,
        bold: false,
    };
    frame.set(3, 3, stuck_cell);
    // Ensure phosphor at (3,3) is 0 so the cell qualifies as stuck.
    if !cloud.phosphor.is_empty() {
        let pidx = 3usize * cloud.lines as usize + 3usize;
        cloud.phosphor[pidx] = 0;
    }

    // Pre-condition: the cell has a glyph in frame.cells.
    let idx = 3usize * cloud.cols as usize + 3usize;
    assert_eq!(
        frame.cells[idx].ch, 'Y',
        "precondition: frame.cells holds the stuck glyph"
    );

    // Bump the counter past the threshold.
    cloud.frames_since_stuck_sweep = STUCK_CELL_SWEEP_INTERVAL_FRAMES + 1;
    cloud.stuck_cell_sweep(&mut frame);

    // The cell must be CLEARED — the sweep checked frame.cells[i]
    // directly and found the stuck glyph.
    assert!(
        frame.get(3, 3).unwrap().fg.is_none(),
        "sweep must clear stuck cells (checked frame.cells[i] directly)"
    );
}

/// The sweep interval must be 600 frames (10s at 60fps), not 3600 (60s).
/// Before the fix, the 60s interval was too long — stuck cells persisted
/// for a full minute before being cleared. 10s is short enough that the
/// owner won't perceive stuck cells as "permanent".
#[test]
fn hunt17_sweep_interval_is_10s_not_60s() {
    assert_eq!(
        STUCK_CELL_SWEEP_INTERVAL_FRAMES, 600,
        "NIGHT-hunter-17: sweep interval must be 600 frames (10s at 60fps), not 3600 (60s)"
    );
}
