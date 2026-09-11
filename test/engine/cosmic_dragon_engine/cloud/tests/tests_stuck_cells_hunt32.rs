// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-hunt-32 regression tests: the stuck-cell sweep vs the
//! structured families (the black hole crown blink).
//!
//! Owner report (2026-09-11, the hunt-31 follow-up round): the three
//! disks above the black hole — the halo triple crown — intermittently
//! glitch/blink for a few seconds to minutes. NOT the startup intro
//! (the genesis visual is intended); the blink hits the steady state.
//!
//! Root cause: the stuck-cell sweep (phosphor_anomaly.rs) fired on
//! structured-style cells every STUCK_CELL_SWEEP_INTERVAL_FRAMES (600
//! frames, ~10 s at 60 FPS). The stuck signature (visible glyph +
//! zero phosphor energy + no droplet coverage) describes a droplet
//! rain dirty-tracking gap, but on a structured style all three
//! clauses point at LIVE cells: `droplets` is empty by contract, the
//! NIGHT-hunter-29 ownership rule zeroes phosphor on every drawn
//! cell, and `Frame::set`'s equality skip keeps persistently-drawn
//! cells (settled crown riders, the stable annulus rungs) out of the
//! dirty list so phosphor decay Pass 1 never re-arms their energy.
//! The sweep then force-cleared up to STUCK_CELL_MAX_PER_SWEEP live
//! cells per pass, row-major from the top of the screen — the budget
//! landing on the black hole's upper crowns, the densest structure
//! above the ball — one blink every ~10 s.
//!
//! Fix (NIGHT-hunt-32): the sweep is droplet-family-only
//! (`rain_style.is_droplet_family()`). The thirteen structured styles
//! own their vacated cells through the monolith-style diff cleanup
//! contract, so the sweep adds nothing for them — and with the gate,
//! it can no longer eat their live cells.

use crossterm::style::Color;
use std::time::Duration;

use super::tests_black_hole::{run_frames, run_frames_to_steady};
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

/// The e2e pipelines need a COLORED cloud: `color_for_level` returns
/// `fg: None` under ColorMode::Mono, so a Mono harness never plants
/// the fg the sweep's candidacy reads — the real binary runs colored
/// and every drawn cell carries a Some(fg). Mirrors the shared
/// black-hole harness except for the color mode.
fn make_color_black_hole_cloud(cols: u16, lines: u16) -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::Color256,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::EnergyZen,
        RainStyle::BlackHole,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.set_droplet_density(0.55);
    cloud.set_chars_per_sec(12.0);
    cloud.reset(cols, lines);
    cloud.set_max_sim_delta(Duration::from_millis(16));
    cloud.clear_redraw_flags_for_test();
    cloud
}

/// Plant a live structured-family cell: a visible glyph with zero
/// phosphor energy and no droplet covering it — the exact signature
/// the sweep used to mistake for a stuck droplet cell. The cell must
/// survive the sweep on a structured style.
fn plant_live_cell(frame: &mut Frame, cloud: &mut Cloud, col: u16, line: u16, ch: char) {
    let cell = Cell {
        ch,
        fg: Some(Color::Green),
        bg: cloud.palette.bg,
        bold: false,
    };
    frame.set(col, line, cell);
    if !cloud.phosphor.is_empty() {
        let pidx = col as usize * cloud.lines as usize + line as usize;
        cloud.phosphor[pidx] = 0;
    }
}

/// The black hole's live cells must survive the sweep: a structured
/// family owns its drawn cells, and the sweep has no droplet
/// machinery to second-guess it. Before the fix, this exact setup
/// cleared the cell (the crown-blink mechanism).
#[test]
fn hunt32_sweep_leaves_black_hole_live_cells_alone() {
    let mut cloud = make_cloud(RainStyle::BlackHole);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    plant_live_cell(&mut frame, &mut cloud, 5, 5, 'X');
    assert!(frame.get(5, 5).unwrap().fg.is_some());

    cloud.frames_since_stuck_sweep = STUCK_CELL_SWEEP_INTERVAL_FRAMES + 1;
    cloud.stuck_cell_sweep(&mut frame);

    assert!(
        frame.get(5, 5).unwrap().fg.is_some(),
        "the sweep must not clear a structured family's live drawn cell (the black hole crown blink)"
    );
    assert_eq!(
        cloud.stuck_cell_stats(),
        (0, 0),
        "the sweep must book zero clears on a structured style"
    );
}

/// The same immunity covers every structured style — Monolith stands
/// in for the twelve non-black-hole structured families (the sweep's
/// droplet-coverage predicate was never meaningful for any of them).
#[test]
fn hunt32_sweep_leaves_monolith_live_cells_alone() {
    let mut cloud = make_cloud(RainStyle::Monolith);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    plant_live_cell(&mut frame, &mut cloud, 3, 3, 'Y');
    assert!(frame.get(3, 3).unwrap().fg.is_some());

    cloud.frames_since_stuck_sweep = STUCK_CELL_SWEEP_INTERVAL_FRAMES + 1;
    cloud.stuck_cell_sweep(&mut frame);

    assert!(
        frame.get(3, 3).unwrap().fg.is_some(),
        "the sweep must not clear a structured family's live drawn cell (Monolith stands in for all twelve)"
    );
}

/// Contrast contract: the sweep still runs for the droplet family —
/// the NIGHT-hunter-17 stuck-cell fix must stay intact. A genuinely
/// stuck glyph-rain cell (droplet gone, phosphor zero) is still
/// cleared.
#[test]
fn hunt32_sweep_still_clears_glyph_stuck_cells() {
    let mut cloud = make_cloud(RainStyle::Glyph);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    plant_live_cell(&mut frame, &mut cloud, 5, 5, 'X');
    assert!(frame.get(5, 5).unwrap().fg.is_some());

    cloud.frames_since_stuck_sweep = STUCK_CELL_SWEEP_INTERVAL_FRAMES + 1;
    cloud.stuck_cell_sweep(&mut frame);

    assert!(
        frame.get(5, 5).unwrap().fg.is_none(),
        "the sweep must keep clearing genuinely stuck droplet-family cells (hunter-17 contract)"
    );
}

/// End-to-end through the real pipeline: a steady-state black hole
/// driven past the 600-frame sweep boundary must book ZERO stuck
/// clears and keep a full drawn population — before the fix, the
/// sweep frame force-cleared up to 256 live cells off the crowns and
/// the annulus.
#[test]
fn hunt32_black_hole_pipeline_sweep_clears_nothing() {
    let (cols, lines) = (120, 40);
    let mut cloud = make_color_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);

    // Past the formation intro, into the steady state (crowns
    // populated, annulus drawn). A colored run plants real fg on
    // every drawn cell — the sweep's candidacy input.
    run_frames_to_steady(&mut cloud, &mut frame);
    let drawn_before = cloud.black_hole_rain.drawn_cells_for_test().len();
    assert!(
        drawn_before > 100,
        "precondition: a substantial steady-state population (got {} cells)",
        drawn_before
    );
    let fg_before = frame.cells.iter().filter(|c| c.fg.is_some()).count();
    assert!(
        fg_before > 100,
        "precondition: the frame carries the drawn glyphs (got {} fg cells)",
        fg_before
    );

    // 700 more frames at 60 FPS — crosses the 600-frame sweep
    // boundary at least once with the crowns on screen.
    run_frames(&mut cloud, &mut frame, 700, 16);

    assert_eq!(
        cloud.stuck_cell_stats(),
        (0, 0),
        "the sweep must never book a clear on the black hole pipeline (the crown blink)"
    );
    let drawn_after = cloud.black_hole_rain.drawn_cells_for_test().len();
    assert!(
        drawn_after > 100,
        "the steady-state population must survive the sweep boundary frames (got {} cells)",
        drawn_after
    );
}

/// End-to-end, the owner's worst case: the PHOSPHOR PRESSURE WINDOW.
/// A pressure episode above the phosphor skip threshold stops the
/// phosphor pass from re-arming cell energy, so the ownership rule's
/// zeroed phosphor state stays zero on EVERY drawn cell — under the
/// old sweep, the next 600-frame boundary force-cleared the top 256
/// live cells (the crowns) in one frame: a full disk blink for the
/// episode's duration. The structured-family gate must keep the
/// pipeline clean even inside the pressure window.
#[test]
fn hunt32_black_hole_pressure_window_sweep_clears_nothing() {
    let (cols, lines) = (120, 40);
    let mut cloud = make_color_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);

    // Steady state, then arm the pressure episode: above the phosphor
    // pass's skip threshold (PHOSPHOR_SKIP_HIGH, 0.70) — the state the
    // output-congestion episodes drive on the owner's terminal.
    run_frames_to_steady(&mut cloud, &mut frame);
    cloud.set_perf_pressure(0.9);

    // 700 frames at 60 FPS inside the pressure window — at least one
    // sweep boundary fires with every drawn cell at phosphor == 0.
    run_frames(&mut cloud, &mut frame, 700, 16);

    assert_eq!(
        cloud.stuck_cell_stats(),
        (0, 0),
        "the sweep must stay inert inside the phosphor pressure window (the full-crown blink path)"
    );
    assert!(
        cloud.black_hole_rain.drawn_cells_for_test().len() > 100,
        "the drawn population must survive the pressure-window sweep boundary"
    );
}

/// The fourteen-style audit table (the owner's audit ask: "make sure
/// the glitch is gone on the other rain types too"): every structured
/// style's live cells survive the sweep boundary — the bug was never
/// black-hole-specific, all thirteen non-droplet styles wore it — and
/// the glyph family (the sole droplet style) keeps the hunter-17
/// stuck-cell contract.
#[test]
fn hunt32_fourteen_style_sweep_audit_table() {
    let structured = [
        RainStyle::Monolith,
        RainStyle::Vortex,
        RainStyle::Flux,
        RainStyle::Lorenz,
        RainStyle::Dragon,
        RainStyle::Physarum,
        RainStyle::BlackHole,
        RainStyle::Aeolian,
        RainStyle::SolarFlare,
        RainStyle::DnaHelix,
        RainStyle::Murmuration,
        RainStyle::Quasar,
        RainStyle::Neural,
    ];
    for &style in &structured {
        let mut cloud = make_cloud(style);
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        plant_live_cell(&mut frame, &mut cloud, 4, 4, 'A');
        assert!(frame.get(4, 4).unwrap().fg.is_some());

        cloud.frames_since_stuck_sweep = STUCK_CELL_SWEEP_INTERVAL_FRAMES + 1;
        cloud.stuck_cell_sweep(&mut frame);

        assert!(
            frame.get(4, 4).unwrap().fg.is_some(),
            "{}: a structured style's live cell must survive the sweep (the crown-blink class)",
            style.as_str()
        );
        assert_eq!(
            cloud.stuck_cell_stats(),
            (0, 0),
            "{}: the sweep must book zero clears on a structured style",
            style.as_str()
        );
    }

    // The droplet family keeps the hunter-17 contract: a genuinely
    // stuck glyph-rain cell is still cleared by the same sweep.
    let mut cloud = make_cloud(RainStyle::Glyph);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    plant_live_cell(&mut frame, &mut cloud, 4, 4, 'A');
    cloud.frames_since_stuck_sweep = STUCK_CELL_SWEEP_INTERVAL_FRAMES + 1;
    cloud.stuck_cell_sweep(&mut frame);
    assert!(
        frame.get(4, 4).unwrap().fg.is_none(),
        "glyph: the sweep must keep clearing stuck droplet-family cells"
    );
}
