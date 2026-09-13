// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-hunt-43 regression tests: the P4 idle-resync MADV_DONTNEED
//! reclaim must re-normalize the zeroed frame cells.
//!
//! Owner report (glitch shift rain, third round — hunt-17 fixed the
//! sweep gates, hunt-36 removed the last sweep gates + the phosphor
//! transposition, but stuck glyph cells still appeared on the Glyph
//! type after ~1:43 of an unattended screensaver run): rain glyph
//! cells stay stuck until another droplet happens to pass over them.
//!
//! Root cause: `run_adaptive_throttle` (the P4 idle resync path)
//! called `hint_reclaim_pages` directly — WITHOUT the
//! `normalize_reclaimed_cells` fix HUNT-26 had added at the P2
//! self-heal site. Its SAFETY comment still claimed "the next
//! rain_at() bumps the content generation before any cell is read",
//! which stopped being true for the Glyph droplet family when HUNT-25
//! moved its force path to `Frame::force_repaint` (no gen bump; only
//! the thirteen structured styles still run `clear_with_bg`).
//!
//! A zero-filled interior cell is therefore gen-matched and:
//! 1. emitted as a RAW NUL byte (terminals drop it) — the terminal
//!    keeps the pre-reclaim glyph while the model says blank;
//! 2. skipped by the stuck-cell sweep (`fg.is_none()`);
//! 3. never armed by phosphor (Pass 1 only arms cells written this
//!    frame — nothing rewrites a cell no droplet covers);
//! 4. re-emitted as the same dropped NUL by every later full repaint.
//!
//! Fix: both event-loop reclaim sites route through
//! `reclaim_frame_cells` (madvise + normalize + cooldown mark
//! travel together), so a future call site cannot forget the
//! normalize step.

use std::time::{Duration, Instant};

use crossterm::style::Color;

use super::make_cloud;
use crate::cell::Cell;
use crate::cloud::Cloud;
use crate::constants::{reclaim_frame_cells, ReclaimState, STUCK_CELL_SWEEP_INTERVAL_FRAMES};
use crate::frame::Frame;
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};

/// The MADV_DONTNEED zero-fill signature: a Cell whose every byte is
/// zero. Planted directly into `frame.cells` (bypassing `set`, exactly
/// like the real page reclaim — `set()` would trip the width debug
/// guard on '\0').
fn zeroed() -> Cell {
    Cell {
        ch: '\0',
        fg: None,
        bg: None,
        bold: false,
    }
}

/// The headline regression: `reclaim_frame_cells` re-blanks exactly the
/// zeroed cells and records the cooldown, so the post-reclaim full
/// repaint emits proper blanks (ch ' ') instead of raw NUL bytes that
/// terminals silently drop.
#[test]
fn hunt43_reclaim_frame_cells_normalizes_zeroed_cells() {
    let mut frame = Frame::new(
        20,
        10,
        Some(Color::Rgb {
            r: 10,
            g: 10,
            b: 10,
        }),
    );
    // A live glyph (must survive) and two zeroed cells (must re-blank).
    frame.set(
        4,
        4,
        Cell {
            ch: 'Z',
            fg: Some(Color::Green),
            bg: None,
            bold: false,
        },
    );
    let z1 = frame.index(2, 3).expect("in bounds");
    let z2 = frame.index(17, 8).expect("in bounds");
    frame.cells[z1] = zeroed();
    frame.cells[z2] = zeroed();

    let mut reclaim_state = ReclaimState::new();
    let now = Instant::now();
    assert!(reclaim_state.should_reclaim(now), "fresh state reclaims");

    reclaim_frame_cells(&mut frame, &mut reclaim_state, now);

    assert_eq!(
        frame.cell_at_index(z1).ch,
        ' ',
        "zeroed cell must read as a proper blank after the reclaim"
    );
    assert!(
        frame.cell_at_index(z1).fg.is_none(),
        "re-blanked cell must carry no foreground"
    );
    assert_eq!(
        frame.cell_at_index(z2).ch,
        ' ',
        "every zeroed cell must be re-blanked"
    );
    assert_eq!(
        frame
            .cell_at_index(frame.index(4, 4).expect("in bounds"))
            .ch,
        'Z',
        "legitimate glyphs must be untouched by normalization"
    );
    assert!(
        !reclaim_state.should_reclaim(now + Duration::from_secs(10)),
        "cooldown must be recorded by the shared helper"
    );
}

/// The load-bearing proof of WHY the normalize must live inside the
/// reclaim path: a zeroed cell is invisible to every model-side cleanup.
/// The stuck-cell sweep skips fg-less cells, and the phosphor decay
/// pass only arms cells written this frame — so a gen-matched '\0'
/// cell that no droplet re-covers is immortal in the model while the
/// terminal keeps showing the pre-reclaim glyph. Only
/// `normalize_reclaimed_cells` (called by the reclaim helper) heals it.
#[test]
fn hunt43_zeroed_cell_is_invisible_to_sweep_and_phosphor() {
    let mut cloud = make_cloud();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // Simulate the MADV zero-fill on a cell that holds nothing else.
    let (col, line) = (5u16, 4u16);
    let fidx = frame.index(col, line).expect("in bounds");
    frame.cells[fidx] = zeroed();

    // 1. The stuck-cell sweep (interval armed) cannot see it: fg is
    //    None, so the sweep's own visibility predicate skips it.
    cloud.frames_since_stuck_sweep = STUCK_CELL_SWEEP_INTERVAL_FRAMES;
    cloud.stuck_cell_sweep(&mut frame);
    assert_eq!(
        frame.cells[fidx].ch, '\0',
        "sweep must skip fg-less cells — the zombie is invisible to it"
    );

    // 2. The phosphor decay pass cannot arm it: Pass 1 only captures
    //    cells written THIS frame (dirty stamp), and nothing rewrote
    //    the zeroed cell.
    cloud.phosphor_decay_pass(&mut frame, 1.0 / 30.0);
    let pidx = col as usize * cloud.lines as usize + line as usize;
    assert_eq!(
        cloud.phosphor[pidx], 0,
        "phosphor must never arm an unwritten zeroed cell"
    );
    assert_eq!(
        frame.cells[fidx].ch, '\0',
        "phosphor decay pass must leave the zeroed cell untouched"
    );

    // 3. The shared reclaim helper heals it — the only path that can.
    let mut reclaim_state = ReclaimState::new();
    reclaim_frame_cells(&mut frame, &mut reclaim_state, Instant::now());
    assert_eq!(
        frame.cell_at_index(fidx).ch,
        ' ',
        "the reclaim helper must re-blank the zombie so the emitter \
         writes a real blank and the screen converges with the model"
    );
}

/// End-to-end Glyph contract: the resync frame itself (force_repaint,
/// full-body droplet redraw, phosphor passes, sweep) does NOT heal a
/// zeroed cell that no live droplet covers — locking in that the
/// normalize must happen at the reclaim site, not anywhere inside
/// rain_at.
#[test]
fn hunt43_glyph_resync_frame_does_not_heal_zeroed_cells() {
    // A larger grid than make_cloud (20x10 fills completely while the
    // pool is mid-fall — every cell covered): 60x30 leaves uncovered
    // cells, matching the real-terminal shape the bug was reported on.
    let mut cloud = Cloud::new(
        ColorMode::Mono,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Green,
        RainStyle::Glyph,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(60, 30);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // Run a few frames so droplets exist and the sweep counter is past
    // any warm-start noise.
    let start = Instant::now();
    for step in 0..6u32 {
        cloud.last_spawn_time = start - Duration::from_secs(1);
        cloud.rain_at(&mut frame, start + Duration::from_millis(40 * step as u64));
        frame.clear_dirty();
    }

    // Pick a cell NOT covered by any live droplet's visible range
    // [tail+1, head] — the resync frame's full-body redraw only
    // rewrites covered cells, so an uncovered cell keeps its zero-fill.
    // On a 60x30 grid with the pool mid-fall there is always at least
    // one uncovered cell (coverage is bounded by the droplet pool).
    let target = (0..cloud.cols)
        .flat_map(|col| (0..cloud.lines).map(move |line| (col, line)))
        .find(|&(col, line)| {
            !cloud.droplets.iter().any(|d| {
                d.is_alive && d.bound_col == col && {
                    let vs = d.tail_put_line.map(|t| t.saturating_add(1)).unwrap_or(0);
                    line >= vs && line <= d.head_put_line
                }
            })
        });
    let (col, line) = target.expect("some cell is uncovered while the pool is mid-fall at 60x30");
    let fidx = frame.index(col, line).expect("in bounds");
    frame.cells[fidx] = zeroed();

    // The idle-resync sequence: force flag set (the event loop also
    // calls the reclaim helper right before this — modeled below).
    cloud.force_draw_everything = true;
    let later = start + Duration::from_millis(400);
    cloud.rain_at(&mut frame, later);

    assert_eq!(
        frame.cells[fidx].ch, '\0',
        "the Glyph resync frame must not be assumed to heal zeroed cells \
         (force_repaint keeps the generation, so the cell stays \
         gen-matched; only the reclaim helper's normalize can re-blank it)"
    );

    // With the helper in the loop (the hunt-43 fix), the cell heals.
    let mut reclaim_state = ReclaimState::new();
    reclaim_frame_cells(&mut frame, &mut reclaim_state, later);
    assert_eq!(
        frame.cell_at_index(fidx).ch,
        ' ',
        "post-fix: the reclaim helper re-blanks the zeroed cell, so the \
         resync repaint emits a real blank and clears the stranded glyph"
    );
}
