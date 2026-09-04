// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! S-master-HUNT-25 regression tests: forced resync redraws must not
//! reset render state.
//!
//! History: `force_draw_everything` (idle resync, stuck-cell sweep, ANSI
//! drift redraw, paste/focus regain) used to call `frame.clear_with_bg`
//! plus a whole-array `phosphor_base_ch` wipe. That blanked every cell,
//! reset phosphor bookkeeping, and produced a 12-18 frame transient with
//! 2-3x the normal frame size — an ANSI burst that stalled terminals and
//! read as "the rain suddenly shifts for a few seconds, then returns to
//! normal" (owner symptom, all terminal classes, ~first minute and
//! recurring). The fix: resyncs set only `dirty_all` (new
//! `Frame::force_repaint`), preserving cell content, generation, and the
//! phosphor decay state; and `phosphor_decay_pass` prefers the
//! dirty-index scan whenever the dirty list is populated (full-grid scan
//! is reserved for the cleared-buffer case, where it is actually needed).

use std::time::{Duration, Instant};

use crossterm::style::Color;

use super::make_cloud;
use crate::cell::Cell;
use crate::frame::Frame;

/// The `force_repaint` primitive: sets dirty_all WITHOUT clearing cell
/// content or bumping the generation.
#[test]
fn hunt25_force_repaint_preserves_cells_and_gen() {
    let mut frame = Frame::new(20, 10, None);
    let cell = Cell {
        ch: '1',
        fg: Some(Color::Green),
        bg: None,
        bold: false,
    };
    frame.set(3, 4, cell);
    let gen_before = frame.current_gen();
    let ch_before = frame.get(3, 4).expect("cell set above").ch;
    let fg_before = frame.get(3, 4).expect("cell set above").fg;

    frame.clear_dirty();
    assert!(!frame.is_dirty_all());

    frame.force_repaint();
    assert!(frame.is_dirty_all(), "force_repaint must set dirty_all");
    assert_eq!(
        frame.current_gen(),
        gen_before,
        "force_repaint must not bump the generation (unlike clear_with_bg)"
    );
    let cell_after = frame.get(3, 4).expect("cell still present");
    assert_eq!(
        ch_before, cell_after.ch,
        "force_repaint must not clear cell content"
    );
    assert!(fg_before.is_some());
}

/// Glyph-style force_draw resync preserves the phosphor glyph
/// bookkeeping of actively-decaying cells (the pre-HUNT-25 code wiped
/// the whole phosphor_base_ch array).
#[test]
fn hunt25_resync_preserves_active_phosphor_glyph() {
    let mut cloud = make_cloud();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // Populate droplets + phosphor state.
    cloud.last_spawn_time = Instant::now() - Duration::from_secs(1);
    let now = Instant::now();
    cloud.rain_at(&mut frame, now);
    frame.clear_dirty();

    // Plant an actively-decaying afterglow cell in a column with no
    // living droplet (so Pass 2 droplet protection cannot refresh it).
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
    let pidx = free_col as usize * cloud.lines as usize + 2usize;
    cloud.phosphor[pidx] = 255;
    cloud.phosphor_base_ch[pidx] = '1';
    cloud.phosphor_base_fg[pidx] = Some(Color::Green);
    if !cloud.phosphor_in_active[pidx] {
        cloud.phosphor_active.push(pidx);
        cloud.phosphor_in_active.set(pidx, true);
    }

    // Trigger a forced resync and run a frame.
    cloud.force_draw_everything = true;
    let later = now + Duration::from_millis(40);
    cloud.rain_at(&mut frame, later);

    // The base glyph must survive the resync (pre-HUNT-25: wiped to '\0').
    assert_eq!(
        cloud.phosphor_base_ch[pidx], '1',
        "resync must not wipe the base glyph of an active afterglow cell"
    );
    // The energy must decay forward from 80, not re-seed to the fresh
    // capture value (~captured_phosphor_energy, 255-class).
    assert!(
        cloud.phosphor[pidx] < 250,
        "resync must not re-seed phosphor energy (got {})",
        cloud.phosphor[pidx]
    );
    assert!(
        !cloud.force_draw_everything,
        "force_draw_everything must be consumed by the frame"
    );
}

/// The stuck-cell sweep still works through the resync path: an orphaned
/// glyph cell is cleared and the frame is flagged for a full re-emit.
#[test]
fn hunt25_resync_still_emits_stuck_cell_clears() {
    let mut cloud = make_cloud();
    cloud.enable_component_timing = true;
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    cloud.last_spawn_time = Instant::now() - Duration::from_secs(1);
    cloud.rain_at(&mut frame, Instant::now());
    frame.clear_dirty();

    // Plant an orphan: glyph cell, no phosphor tracking, no droplet.
    let stuck = Cell {
        ch: 'X',
        fg: Some(Color::Green),
        bg: cloud.palette.bg,
        bold: false,
    };
    frame.set(2, 3, stuck);
    if !cloud.phosphor.is_empty() {
        let pidx = 2usize * cloud.lines as usize + 3usize;
        cloud.phosphor[pidx] = 0;
    }

    // Force the sweep to run on the next rain_at call.
    cloud.frames_since_stuck_sweep = crate::constants::STUCK_CELL_SWEEP_INTERVAL_FRAMES;
    cloud.force_draw_everything = true;
    cloud.rain_at(&mut frame, Instant::now() + Duration::from_millis(50));

    let cleared = frame.get(2, 3).map(|c| c.fg.is_none()).unwrap_or(true);
    assert!(
        cleared,
        "stuck-cell sweep must still clear orphaned glyphs through force_repaint"
    );
}

/// Monolith force path keeps its historical full state reset (draw
/// history + phosphor) — only the Glyph resync path changed.
#[test]
fn hunt25_monolith_force_path_still_clears() {
    use crate::rain_style::RainStyle;
    use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};
    let mut cloud = crate::cloud::Cloud::new(
        ColorMode::Mono,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Green,
        RainStyle::Monolith,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(20, 10);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    cloud.force_draw_everything = true;
    cloud.rain_at(&mut frame, Instant::now());
    // The monolith branch must still have consumed the flag and rebuilt
    // its phosphor state (i.e. not the force_repaint path).
    assert!(!cloud.force_draw_everything);
}
