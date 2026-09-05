// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! S-master-HUNT-26 regression tests: the phosphor decay pass's post-skip
//! resume must amortize the frozen backlog (thaw), not dump it at once.
//!
//! History: the M1 pressure gate (PHOSPHOR_SKIP_HIGH 0.70 / LOW 0.50)
//! froze the whole decay pass while pressure sat above the band. Droplet
//! tails kept blanking cells, so every cell vacated during the freeze
//! joined a backlog of active cells whose energy never decayed and whose
//! ghost was never written. On resume the pass rendered the entire
//! backlog within one or two frames — thousands of blank cells flashing
//! to afterglow at once (measured 6,151 cells at 200x56 in the PTY
//! content harness), a frame-size burst that re-saturated the pipe and
//! re-armed the skip: the self-exciting "glitch rain shift" the owner
//! still saw during the startup fill-up window and after the first
//! charset/color shortkey. The fix marks every active cell pending on
//! resume and writes at most PHOSPHOR_THAW_MAX_CELLS_PER_FRAME of them
//! per frame; the rest stay frozen with their marks intact.

use std::time::{Duration, Instant};

use crossterm::style::Color;

use super::make_cloud;
use crate::constants::PHOSPHOR_THAW_MAX_CELLS_PER_FRAME;
use crate::frame::Frame;
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};

/// A larger cloud than `make_cloud` (which is 20x10 = 200 cells): the thaw
/// budget only binds when the backlog exceeds it, so the amortization tests
/// need a grid big enough to plant one.
fn big_cloud(cols: u16, lines: u16) -> super::super::Cloud {
    let mut cloud = super::super::Cloud::new(
        ColorMode::Mono,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Green,
        RainStyle::Glyph,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(cols, lines);
    cloud
}

/// Plant `n` active phosphor trail cells (column-major pidx space) with
/// live energy, a base fg, and a base glyph — the backlog a long skip
/// episode accumulates. Returns the planted pidx list.
fn plant_backlog(cloud: &mut super::super::Cloud, n: usize) -> Vec<usize> {
    let total = cloud.cols as usize * cloud.lines as usize;
    let mut planted = Vec::with_capacity(n);
    for pidx in 0..n.min(total) {
        cloud.phosphor[pidx] = 200;
        cloud.phosphor_base_fg[pidx] = Some(Color::Green);
        cloud.phosphor_base_ch[pidx] = '1';
        if !cloud.phosphor_in_active[pidx] {
            cloud.phosphor_active.push(pidx);
            cloud.phosphor_in_active.set(pidx, true);
        }
        planted.push(pidx);
    }
    planted
}

/// The resume frame writes at most PHOSPHOR_THAW_MAX_CELLS_PER_FRAME of the
/// frozen backlog. Pre-HUNT-26 this frame wrote the whole backlog at once.
#[test]
fn hunt26_thaw_resume_frame_is_budgeted() {
    let mut cloud = big_cloud(80, 24); // 1920 cells
    let backlog = plant_backlog(&mut cloud, 900);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // Enter the skip: pressure above the high threshold freezes the pass.
    cloud.perf_pressure = 0.9;
    cloud.phosphor_decay_pass(&mut frame, 1.0 / 60.0);
    assert!(cloud.phosphor_skipped, "pressure 0.9 must skip the pass");
    frame.clear_dirty();
    assert!(
        frame.dirty_indices().is_empty(),
        "a skipped pass must not write"
    );

    // Resume: pressure below the low threshold.
    cloud.perf_pressure = 0.2;
    cloud.phosphor_decay_pass(&mut frame, 1.0 / 60.0);
    assert!(!cloud.phosphor_skipped, "pressure 0.2 must resume the pass");

    let dirty = frame.dirty_indices().len();
    assert!(
        dirty <= PHOSPHOR_THAW_MAX_CELLS_PER_FRAME,
        "resume frame wrote {dirty} cells — budget is {PHOSPHOR_THAW_MAX_CELLS_PER_FRAME} \
         (pre-HUNT-26 behavior: the whole {backlog_len}-cell backlog dumped at once)",
        backlog_len = backlog.len()
    );
    assert_eq!(
        cloud.phosphor_thaw_pending_count,
        backlog.len() - dirty,
        "pending count must equal backlog minus the cells actually written"
    );
}

/// The backlog drains to zero across ceil(backlog / budget) frames and the
/// pass then returns to full-rate operation.
#[test]
fn hunt26_thaw_drains_and_returns_to_full_rate() {
    let mut cloud = big_cloud(80, 24);
    plant_backlog(&mut cloud, 900);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    cloud.perf_pressure = 0.9;
    cloud.phosphor_decay_pass(&mut frame, 1.0 / 60.0);
    cloud.perf_pressure = 0.2;

    let mut frames = 0;
    while cloud.phosphor_thaw_pending_count > 0 && frames < 8 {
        frame.clear_dirty();
        cloud.phosphor_decay_pass(&mut frame, 1.0 / 60.0);
        assert!(
            frame.dirty_indices().len() <= PHOSPHOR_THAW_MAX_CELLS_PER_FRAME,
            "every thawing frame must respect the budget"
        );
        frames += 1;
    }
    assert_eq!(
        cloud.phosphor_thaw_pending_count, 0,
        "backlog must drain within ceil(900/600) + 1 frames (took {frames})"
    );

    // Thaw complete: full-rate operation restored — the pass may now write
    // every live active cell in a single frame again.
    frame.clear_dirty();
    cloud.phosphor_decay_pass(&mut frame, 1.0 / 60.0);
    let full_rate = frame.dirty_indices().len();
    assert!(
        full_rate > 0,
        "post-thaw frames must still decay surviving cells"
    );
}

/// A pending cell covered by a droplet this frame (fresh) leaves the thaw
/// queue without a write — the re-capture supersedes backlog membership.
#[test]
fn hunt26_fresh_recapture_leaves_thaw_queue_silently() {
    let mut cloud = big_cloud(80, 24);
    let planted = plant_backlog(&mut cloud, 1300);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    cloud.perf_pressure = 0.9;
    cloud.phosphor_decay_pass(&mut frame, 1.0 / 60.0);
    frame.clear_dirty();
    cloud.perf_pressure = 0.2;

    // Resume frame: 600 budgeted, 700 still pending.
    cloud.phosphor_decay_pass(&mut frame, 1.0 / 60.0);
    assert_eq!(
        cloud.phosphor_thaw_pending_count,
        planted.len() - PHOSPHOR_THAW_MAX_CELLS_PER_FRAME
    );

    // Mark 100 of the still-pending cells fresh (a droplet covered them
    // this frame — exactly what Pass 1/2 do to re-captured cells).
    let mut fresh_cells = Vec::new();
    for &pidx in &planted {
        if cloud.phosphor_thaw_pending[pidx] && fresh_cells.len() < 100 {
            cloud.phosphor_fresh.set(pidx, true);
            fresh_cells.push(pidx);
        }
    }
    assert_eq!(fresh_cells.len(), 100);
    frame.clear_dirty();

    cloud.phosphor_decay_pass(&mut frame, 1.0 / 60.0);
    assert_eq!(
        cloud.phosphor_thaw_pending_count, 0,
        "fresh cells leave the queue without a write and the remaining          pending cells fit within one frame's budget"
    );
    let width = frame.width as usize;
    let dirty_set: std::collections::HashSet<usize> =
        frame.dirty_indices().iter().copied().collect();
    for &pidx in &fresh_cells {
        let col = pidx / cloud.lines as usize;
        let line = pidx % cloud.lines as usize;
        let fidx = line * width + col;
        assert!(
            !dirty_set.contains(&fidx),
            "fresh cells must not be written by the decay pass"
        );
    }
}

/// A zero-energy pending cell is removed silently — the Monolith-immunity
/// path: styles whose cleanup zeroes energies never dump a backlog because
/// their cells leave through this branch without a write.
#[test]
fn hunt26_zero_energy_leaves_backlog_silently() {
    let mut cloud = big_cloud(80, 24);
    let planted = plant_backlog(&mut cloud, 3);
    // Kill the middle cell's energy (monolith clear_cell equivalent).
    cloud.phosphor[planted[1]] = 0;
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    cloud.perf_pressure = 0.9;
    cloud.phosphor_decay_pass(&mut frame, 1.0 / 60.0);
    frame.clear_dirty();
    cloud.perf_pressure = 0.2;
    cloud.phosphor_decay_pass(&mut frame, 1.0 / 60.0);

    // All three cells are processed in this single resume frame (3 << the
    // budget) — but the zero-energy one leaves WITHOUT a write: only the
    // two live cells are written, and the pending count reaches zero.
    assert_eq!(cloud.phosphor_thaw_pending_count, 0);
    assert_eq!(
        frame.dirty_indices().len(),
        2,
        "the zero-energy cell must leave the backlog without a write"
    );
    assert!(
        !cloud.phosphor_in_active[planted[1]],
        "the zero-energy cell must be removed from the active list"
    );
}

/// A mid-thaw re-skip keeps the remaining marks; the next resume re-arms
/// conservatively (the still-frozen cells stay pending) and the budget
/// still applies.
#[test]
fn hunt26_reskip_mid_thaw_keeps_remaining_backlog() {
    let mut cloud = big_cloud(80, 24);
    plant_backlog(&mut cloud, 900);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    cloud.perf_pressure = 0.9;
    cloud.phosphor_decay_pass(&mut frame, 1.0 / 60.0);
    cloud.perf_pressure = 0.2;
    cloud.phosphor_decay_pass(&mut frame, 1.0 / 60.0);
    let remaining = cloud.phosphor_thaw_pending_count;
    assert!(remaining > 0, "900-cell backlog cannot drain in one frame");

    // Congestion returns mid-thaw: the pass re-skips.
    cloud.perf_pressure = 0.9;
    cloud.phosphor_decay_pass(&mut frame, 1.0 / 60.0);
    assert!(cloud.phosphor_skipped);

    // Final resume: backlog drains under the budget and reaches zero.
    cloud.perf_pressure = 0.2;
    let mut frames = 0;
    let mut last_pending = cloud.phosphor_thaw_pending_count;
    while cloud.phosphor_thaw_pending_count > 0 && frames < 10 {
        frame.clear_dirty();
        cloud.phosphor_decay_pass(&mut frame, 1.0 / 60.0);
        // The budget caps how many PENDING cells leave per frame (already
        // thawed cells keep decaying normally alongside them).
        assert!(
            last_pending - cloud.phosphor_thaw_pending_count <= PHOSPHOR_THAW_MAX_CELLS_PER_FRAME,
            "pending backlog must drain at most the budget per frame"
        );
        last_pending = cloud.phosphor_thaw_pending_count;
        frames += 1;
    }
    assert_eq!(cloud.phosphor_thaw_pending_count, 0);
}

/// A full phosphor reset (semantic/scene transition) voids the backlog
/// marks — the pending state cannot outlive the state it tracked.
#[test]
fn hunt26_reset_phosphor_state_clears_backlog() {
    let mut cloud = big_cloud(80, 24);
    plant_backlog(&mut cloud, 900);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    cloud.perf_pressure = 0.9;
    cloud.phosphor_decay_pass(&mut frame, 1.0 / 60.0);
    cloud.perf_pressure = 0.2;
    cloud.phosphor_decay_pass(&mut frame, 1.0 / 60.0);
    assert!(cloud.phosphor_thaw_pending_count > 0);

    cloud.reset_phosphor_state();
    assert_eq!(cloud.phosphor_thaw_pending_count, 0);
    assert!(
        !cloud.phosphor_thaw_pending.iter().any(|b| *b),
        "no pending marks may survive a full phosphor reset"
    );
}

/// Steady-state decay (no thaw in progress) is untouched: every live
/// active cell is written on a normal frame, regardless of the budget.
#[test]
fn hunt26_steady_state_unaffected_by_budget() {
    let mut cloud = big_cloud(80, 24);
    let planted = plant_backlog(&mut cloud, 3);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // No skip ever happened: no thaw armed, all cells below the budget.
    // clear_dirty first: a fresh Frame carries dirty_all=true (the initial
    // full-redraw flag), and set() does not push to the dirty list while
    // dirty_all is set — the real lifecycle clears it every frame.
    frame.clear_dirty();
    cloud.perf_pressure = 0.2;
    cloud.phosphor_decay_pass(&mut frame, 1.0 / 60.0);
    assert_eq!(
        cloud.phosphor_thaw_pending_count, 0,
        "no skip episode means no pending marks"
    );
    let dirty = frame.dirty_indices().len();
    assert_eq!(
        dirty,
        planted.len(),
        "every live active cell must decay on a normal frame"
    );
}

/// The pressure freeze itself still works: while skipped, energies are
/// frozen (no decay) — the pass must not mutate phosphor state at all.
#[test]
fn hunt26_skip_freezes_decay_completely() {
    let mut cloud = make_cloud();
    let planted = plant_backlog(&mut cloud, 5);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    cloud.perf_pressure = 0.9;
    let energies: Vec<u8> = planted.iter().map(|&p| cloud.phosphor[p]).collect();
    cloud.phosphor_decay_pass(&mut frame, 1.0);
    assert!(cloud.phosphor_skipped);
    for (i, &p) in planted.iter().enumerate() {
        assert_eq!(
            cloud.phosphor[p], energies[i],
            "a skipped pass must not decay energies"
        );
    }
    let _ = Duration::from_millis(0);
    let _ = Instant::now();
}

/// HUNT-26 park semantics: a cell blanked THIS FRAME parks for exactly one
/// frame (energy held at the tail residual, no ghost write); on the NEXT
/// frame it must enter the decay path and render its afterglow. The
/// pre-HUNT-26 epoch check parked vacated cells forever — this test pins
/// the per-frame boundary.
#[test]
fn hunt26_park_is_one_frame_not_forever() {
    let mut cloud = make_cloud();
    let lines = cloud.lines as usize;
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    let pidx = 3usize * lines + 4usize; // (col=3, line=4)
    let col = (pidx / lines) as u16;
    let line = (pidx % lines) as u16;
    // Give the cell real content first, so the later blank write actually
    // changes content and stamps "written this frame" (set() short-circuits
    // no-op writes, and a fresh frame is already blank).
    frame.set(
        col,
        line,
        crate::cell::Cell {
            ch: '1',
            fg: Some(Color::Green),
            bg: cloud.palette.bg,
            bold: false,
        },
    );
    frame.clear_dirty();

    cloud.phosphor[pidx] = 160;
    cloud.phosphor_base_fg[pidx] = Some(Color::Green);
    cloud.phosphor_base_ch[pidx] = '1';
    if !cloud.phosphor_in_active[pidx] {
        cloud.phosphor_active.push(pidx);
        cloud.phosphor_in_active.set(pidx, true);
    }
    // The droplet tail vacates it THIS frame: content glyph -> blank.
    frame.set(
        col,
        line,
        crate::cell::Cell::blank_with_bg(cloud.palette.bg),
    );

    // Frame 1 (vacation frame): parked — energy held, no ghost write.
    cloud.phosphor_decay_pass(&mut frame, 1.0 / 60.0);
    assert_eq!(cloud.phosphor[pidx], 160, "park holds the residual energy");

    // Frame 2: NOT written this frame anymore — the decay path must run.
    // Pre-HUNT-26 (epoch check): parked again, nothing written, forever.
    frame.clear_dirty();
    cloud.phosphor_decay_pass(&mut frame, 1.0 / 60.0);
    assert!(
        cloud.phosphor[pidx] < 160,
        "the frame after vacation must decay the energy (got {})",
        cloud.phosphor[pidx]
    );
    let fidx = frame.index(col, line).expect("planted cell is in bounds");
    assert!(
        frame.cell_at_index_ref(fidx).fg.is_some(),
        "the afterglow ghost must render on the frame after vacation"
    );
}

/// HUNT-26 resync forces must not flip the droplets into full-body mode:
/// cells the fractional-position logic has not reached yet stay unwritten
/// on a force frame. Pre-HUNT-26, draw_everything was wired to the raw
/// force flag, so every P2/idle-resync/ANSI-drift force drew every
/// not-yet-reached body cell at once — the ~1,700-glyph one-frame flash.
#[test]
fn hunt26_resync_force_keeps_fractional_body_skip() {
    let mut cloud = big_cloud(80, 24);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    let t0 = Instant::now();
    let mut now = t0;
    for _ in 0..600 {
        now += Duration::from_millis(16);
        cloud.rain_at(&mut frame, now);
        frame.clear_dirty();
    }

    // Find a live droplet with fractional headroom and a never-reached cell.
    let mut probe: Option<(u16, u16)> = None;
    for d in &cloud.droplets {
        if !d.is_alive || d.head_cur_line >= d.head_put_line {
            continue;
        }
        for line in (d.head_cur_line + 1)..=d.head_put_line {
            if (line as usize) < cloud.lines as usize {
                let fidx = frame
                    .index(d.bound_col, line)
                    .expect("bounds checked above");
                if frame.cell_at_index_ref(fidx).fg.is_none() {
                    probe = Some((d.bound_col, line));
                    break;
                }
            }
        }
        if probe.is_some() {
            break;
        }
    }
    let Some((col, line)) = probe else {
        return; // no fractional headroom this run — vacuous, not a failure
    };

    // Resync force (P2 self-healer / idle resync / ANSI drift path).
    cloud.force_draw_everything();
    now += Duration::from_millis(16);
    cloud.rain_at(&mut frame, now);
    let fidx = frame.index(col, line).expect("probe is in bounds");
    assert!(
        frame.cell_at_index_ref(fidx).fg.is_none(),
        "resync force must not draw the not-yet-reached body cell at \
         ({col}, {line}) — full-body flip was the P2 30 s flash"
    );
}

/// HUNT-26 full-grid capture on resync frames reads the PER-FRAME write
/// stamp, so cells written on earlier frames are not re-captured (the
/// epoch check re-seeded their phosphor energy to the bright capture
/// value, resetting every live trail's decay clock at once).
#[test]
fn hunt26_resync_force_does_not_reseed_old_writes() {
    let mut cloud = big_cloud(80, 24);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    let t0 = Instant::now();
    let mut now = t0;
    for _ in 0..600 {
        now += Duration::from_millis(16);
        cloud.rain_at(&mut frame, now);
        frame.clear_dirty();
    }

    // Find a decaying (non-fresh) active cell written on an earlier frame.
    let lines = cloud.lines as usize;
    let mut probe = None;
    for &pidx in &cloud.phosphor_active {
        if pidx >= cloud.phosphor.len() || cloud.phosphor_fresh[pidx] {
            continue;
        }
        let e = cloud.phosphor[pidx];
        if (30..150).contains(&e) {
            probe = Some((pidx, e));
            break;
        }
    }
    let Some((pidx, energy_before)) = probe else {
        return; // no mid-decay cell this run — vacuous
    };
    let col = (pidx / lines) as u16;
    let line = (pidx % lines) as u16;
    let fidx = frame.index(col, line).expect("active cell is in bounds");
    assert!(
        !frame.cell_written_this_frame(fidx),
        "probe must be an earlier-frame write"
    );

    cloud.force_draw_everything();
    now += Duration::from_millis(16);
    cloud.rain_at(&mut frame, now);
    assert!(
        cloud.phosphor[pidx] <= energy_before,
        "resync force must not re-seed a decaying cell ({} -> {})",
        energy_before,
        cloud.phosphor[pidx]
    );
}

/// HUNT-26 MADV normalization: cells zeroed by page reclaim read back as
/// ch == '\0' and are re-blanked so they emit as proper spaces instead of
/// raw NUL bytes.
#[test]
fn hunt26_normalize_reclaimed_cells_reblanks_nuls() {
    let mut frame = Frame::new(20, 10, None);
    frame.set(2, 3, crate::cell::Cell::blank_with_bg(None));
    // Simulate the MADV_DONTNEED zero-fill directly (set() would trip the
    // width debug guard on '\0' — the zero-fill bypasses set() by design).
    let fidx = frame.index(2, 3).expect("set above");
    frame.cells[fidx].ch = '\0';
    let other = frame.index(5, 5).expect("in bounds");
    frame.cells[other].ch = 'x';

    frame.normalize_reclaimed_cells();

    assert_eq!(
        frame.cell_at_index(fidx).ch,
        ' ',
        "zeroed cell must read as a proper blank after normalization"
    );
    assert_eq!(
        frame.cell_at_index(other).ch,
        'x',
        "legitimate glyphs must be untouched"
    );
}
