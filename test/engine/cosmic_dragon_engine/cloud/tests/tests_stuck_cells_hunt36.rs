// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-hunt-36 regression tests: the stuck-cell sweep must actually RUN
//! on default interactive sessions and consult the CORRECT phosphor slot.
//!
//! Two bugs were fixed (owner report: micro glitch shift rain — some rain
//! glyph cells stay stuck near the top/bottom of the screen until another
//! droplet happens to pass over them, or for a very long time):
//!
//! 1. Whole-function message gate: `!self.message.is_empty() -> return`
//!    disabled the sweep entirely whenever an overlay was active — and the
//!    default interactive config ALWAYS carries the built-in fallback
//!    message ("Experience a masterpiece with cosmostrix vX", wired in
//!    `build_cloud_cfg`), so the sweep NEVER ran on a default run. Fix:
//!    per-cell rectangle exemption (`message_sweep_top/bottom/left/right`,
//!    cached in `relayout_message`, bordered AND borderless) — the sweep
//!    now runs with overlays visible and spares only the overlay box.
//!
//! 2. Phosphor transposition: the sweep read `self.phosphor[i]` with the
//!    frame's ROW-major index, but the phosphor arrays are COLUMN-major
//!    (pidx = col * lines + line, see phosphor.rs Pass 1). It consulted a
//!    transposed cell's energy: live decaying ghosts were force-cleared
//!    and real orphans were skipped whenever the transposed slot's energy
//!    disagreed. Fix: index the cell's own slot.

use std::time::{Duration, Instant};

use crossterm::style::Color;

use super::make_cloud;
use crate::cell::Cell;
use crate::cloud::Cloud;
use crate::constants::STUCK_CELL_SWEEP_INTERVAL_FRAMES;
use crate::frame::Frame;

fn stuck(ch: char) -> Cell {
    Cell {
        ch,
        fg: Some(Color::Green),
        bg: None,
        bold: false,
    }
}

/// Zero the phosphor energy of a cell's OWN column-major slot.
fn zero_phosphor(cloud: &mut Cloud, col: u16, line: u16) {
    let pidx = col as usize * cloud.lines as usize + line as usize;
    assert!(pidx < cloud.phosphor.len(), "phosphor slot out of bounds");
    cloud.phosphor[pidx] = 0;
}

/// The headline regression: the sweep must fire while a message overlay is
/// active. The pre-hunt-36 whole-function gate returned early whenever
/// `message` was non-empty — which on a default interactive run (fallback
/// message always present) meant the sweep never ran at all, leaving
/// orphan glyph cells stuck until a random droplet overwrote them.
#[test]
fn hunt36_sweep_runs_with_message_active() {
    let mut cloud = make_cloud();
    // Simulate the default interactive condition: overlay present. Use the
    // exact built-in fallback text to mirror build_cloud_cfg's wiring.
    cloud.set_message(&crate::constants::default_message_text());
    assert!(!cloud.message.is_empty());

    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // Orphan outside the overlay box (top edge, far left).
    frame.set(0, 0, stuck('X'));
    zero_phosphor(&mut cloud, 0, 0);
    assert!(frame.get(0, 0).unwrap().fg.is_some());

    cloud.frames_since_stuck_sweep = STUCK_CELL_SWEEP_INTERVAL_FRAMES;
    cloud.stuck_cell_sweep(&mut frame);

    assert!(
        frame.get(0, 0).unwrap().fg.is_none(),
        "sweep must run and clear orphans while a message overlay is active"
    );
    assert!(
        cloud.stuck_cells_cleared_total >= 1,
        "sweep stats must record the clear"
    );
}

/// Phosphor lookup must be column-major: a cell with LIVE decaying energy
/// in its own slot must NOT be force-cleared. The pre-hunt-36 code indexed
/// `phosphor[frame_row_major_i]` — a transposed slot — so a live ghost
/// whose transposed slot happened to be zero was destroyed by the sweep
/// (visible as phosphor trails glitch-flickering), while the sweep's real
/// targets were missed whenever the transposed slot held energy.
#[test]
fn hunt36_sweep_phosphor_lookup_is_column_major() {
    let mut cloud = make_cloud();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // Plant a LIVE decaying ghost: glyph present, phosphor energy armed in
    // the cell's own column-major slot (col=6, line=2).
    let (col, line) = (6u16, 2u16);
    frame.set(col, line, stuck('G'));
    let pidx = col as usize * cloud.lines as usize + line as usize;
    cloud.phosphor[pidx] = 200;
    cloud.phosphor_base_ch[pidx] = 'G';
    cloud.phosphor_base_fg[pidx] = Some(Color::Green);

    cloud.frames_since_stuck_sweep = STUCK_CELL_SWEEP_INTERVAL_FRAMES;
    cloud.stuck_cell_sweep(&mut frame);

    assert!(
        frame.get(col, line).unwrap().fg.is_some(),
        "sweep must not clear a cell whose own phosphor slot has live energy"
    );
    assert_eq!(
        cloud.stuck_cells_cleared_total, 0,
        "no cells should be classified as stuck"
    );
}

/// The borderless overlay exemption rectangle must cover exactly the
/// overlay box (border + padding + content). Cells inside survive the
/// sweep; cells one step outside each edge do not.
#[test]
fn hunt36_exemption_rect_tracks_borderless_box() {
    let mut cloud = make_cloud();
    // Borderless "hello" on 20x10: box 9x3 at col [5,14), line [3,6).
    cloud.set_message("hello");

    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // White-box: the cached rectangle must match the laid-out grid's
    // bounding box (computed from the message cells themselves).
    let min_line = cloud.message.iter().map(|mc| mc.line).min().unwrap();
    let max_line = cloud.message.iter().map(|mc| mc.line).max().unwrap();
    let min_col = cloud.message.iter().map(|mc| mc.col).min().unwrap();
    let max_col = cloud.message.iter().map(|mc| mc.col).max().unwrap();
    assert_eq!(cloud.message_sweep_top, min_line);
    assert_eq!(
        cloud.message_sweep_bottom,
        max_line.saturating_add(1),
        "half-open bottom bound"
    );
    assert_eq!(cloud.message_sweep_left, min_col);
    assert_eq!(
        cloud.message_sweep_right,
        max_col.saturating_add(1),
        "half-open right bound"
    );

    // Behavioral: just outside every edge of the box is sweepable.
    let outside: [(u16, u16); 4] = [
        (min_col, min_line.saturating_sub(1)), // above
        (min_col, max_line + 1),               // below
        (min_col.saturating_sub(1), min_line), // left of
        (max_col + 1, min_line),               // right of
    ];
    for &(c, l) in &outside {
        frame.set(c, l, stuck('O'));
        zero_phosphor(&mut cloud, c, l);
    }

    cloud.frames_since_stuck_sweep = STUCK_CELL_SWEEP_INTERVAL_FRAMES;
    cloud.stuck_cell_sweep(&mut frame);

    for &(c, l) in &outside {
        assert!(
            frame.get(c, l).unwrap().fg.is_none(),
            "cell just outside the borderless box ({c},{l}) must be swept"
        );
    }
}

/// The bordered overlay exemption must cover the border ring too: a
/// bordered message is 2 wider and 2 taller than the borderless one, and
/// every cell of the ring (corners included) is overlay-owned.
#[test]
fn hunt36_exemption_rect_covers_bordered_ring() {
    let mut cloud = make_cloud();
    cloud.set_message("hello");
    cloud.set_message_border(true);

    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // Bordered "hello" on 20x10: box 11x5. Border cells exist (the layout
    // stamps them — positional classification, S-master-HUNT-3).
    assert!(
        cloud.message.iter().any(|mc| mc.is_border),
        "bordered layout must stamp border cells"
    );
    let min_line = cloud.message.iter().map(|mc| mc.line).min().unwrap();
    let max_line = cloud.message.iter().map(|mc| mc.line).max().unwrap();
    let min_col = cloud.message.iter().map(|mc| mc.col).min().unwrap();
    let max_col = cloud.message.iter().map(|mc| mc.col).max().unwrap();

    // Plant a glyph on each corner of the ring — the tightest overlay cells.
    let corners: [(u16, u16); 4] = [
        (min_col, min_line),
        (max_col, min_line),
        (min_col, max_line),
        (max_col, max_line),
    ];
    for &(c, l) in &corners {
        frame.set(c, l, stuck('B'));
        zero_phosphor(&mut cloud, c, l);
    }

    cloud.frames_since_stuck_sweep = STUCK_CELL_SWEEP_INTERVAL_FRAMES;
    cloud.stuck_cell_sweep(&mut frame);

    for &(c, l) in &corners {
        assert!(
            frame.get(c, l).unwrap().fg.is_some(),
            "border ring corner ({c},{l}) must be spared by the sweep"
        );
    }
    assert_eq!(
        cloud.stuck_cells_cleared_total, 0,
        "no overlay cell may be classified as stuck"
    );
}

/// When the overlay cannot fit the terminal, `relayout_message` drops the
/// grid and the exemption rectangle must collapse — the sweep then covers
/// the full frame again (no stale rectangle immunity lingering).
#[test]
fn hunt36_relayout_collapses_rect_when_box_does_not_fit() {
    let mut cloud = make_cloud();
    cloud.set_message("hello");
    cloud.set_message_border(true);
    assert!(cloud.message_sweep_bottom > cloud.message_sweep_top);

    // Shrink the terminal below the bordered box minimum (border=1 needs
    // cols >= 2 + 2 pad_x = 6). reset_with_bounds relayouts.
    cloud.reset(4, 3);
    assert!(
        cloud.message.is_empty() || cloud.message_sweep_bottom <= cloud.message_sweep_top,
        "no overlay may keep a sweep exemption after the box stops fitting"
    );

    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    frame.set(0, 0, stuck('X'));
    zero_phosphor(&mut cloud, 0, 0);

    cloud.frames_since_stuck_sweep = STUCK_CELL_SWEEP_INTERVAL_FRAMES;
    cloud.stuck_cell_sweep(&mut frame);

    assert!(
        frame.get(0, 0).unwrap().fg.is_none(),
        "sweep must cover the full frame when no overlay fits"
    );
}

/// Full end-to-end repro of the owner's report through the real frame
/// pipeline (rain_at, not a direct sweep call): a stale orphan glyph is
/// cleared by the periodic sweep even with the default fallback message
/// active, without any droplet needing to pass over it first.
#[test]
fn hunt36_end_to_end_orphan_cleared_with_fallback_message() {
    let mut cloud = make_cloud();
    cloud.set_message(&crate::constants::default_message_text());

    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let now = Instant::now();

    // Natural frame: droplets spawn (message active, as on default runs).
    cloud.last_spawn_time = now - Duration::from_secs(1);
    cloud.rain_at(&mut frame, now);

    // Pick a droplet-free column outside the overlay box.
    let mut free_col = 0u16;
    'outer: for col in 0..cloud.cols {
        for d in &cloud.droplets {
            if d.is_alive && d.bound_col == col {
                continue 'outer;
            }
        }
        let in_overlay = col >= cloud.message_sweep_left && col < cloud.message_sweep_right;
        if in_overlay {
            continue;
        }
        free_col = col;
        break;
    }

    // Plant the orphan, stale its write stamp, lose its phosphor slot.
    frame.set(free_col, 1, stuck('X'));
    frame.clear_dirty();
    zero_phosphor(&mut cloud, free_col, 1);

    // No spawn during the audit frame (fresh last_spawn_time) so the
    // column stays droplet-free; force the sweep interval.
    cloud.last_spawn_time = now;
    cloud.frames_since_stuck_sweep = STUCK_CELL_SWEEP_INTERVAL_FRAMES;
    cloud.rain_at(&mut frame, now + Duration::from_millis(50));

    assert!(
        frame
            .get(free_col, 1)
            .map(|c| c.fg.is_none())
            .unwrap_or(true),
        "end-to-end: orphan glyph must be swept with the fallback message active"
    );
}
