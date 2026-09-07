// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-special-1 stage 2.2 tests: the formation intro — the hole's
//! birth sequence (singularity seed -> collapse flare -> horizon
//! bloom -> accretion). Covers the phase timeline's drawn-cell
//! contract, the mote spawn gate, resize preserving the steady state
//! (no re-formation), and style re-entry replaying the sequence.

use super::*;

#[test]
fn formation_plays_the_birth_sequence() {
    // The owner's question made a contract: the hole must NOT pop in.
    // Early frames draw only the singularity seed (one glyph at the
    // viewport center, plus the collapse cross at most five cells);
    // mid-bloom the annulus is partially drawn (growing from the
    // inside out); past the sequence the full ball is on screen.
    let (cols, lines) = (120, 40);
    let cx = (cols - 1) / 2;
    let cy = (lines - 1) / 2;

    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);

    // Seed phase (~0.5 s in): the dot only.
    run_frames(&mut cloud, &mut frame, 30, 16);
    let drawn = cloud.black_hole_rain.drawn_cells_for_test();
    assert!(
        !drawn.is_empty() && drawn.len() <= 5,
        "the seed phase must draw at most the dot + cross (got {} cells)",
        drawn.len()
    );
    assert!(
        drawn.iter().any(|c| c.col == cx && c.line == cy),
        "the seed must sit at the viewport center"
    );

    // Mid horizon bloom (~2.3 s in): the annulus is partially drawn —
    // more than the dot, less than the full ball.
    run_frames(&mut cloud, &mut frame, 115, 16);
    let full = cloud.black_hole_rain.ring_cells_for_test().len();
    let drawn = cloud.black_hole_rain.drawn_cells_for_test();
    assert!(
        drawn.len() > 5 && drawn.len() < full,
        "the bloom must be partial ({} drawn of {} ball cells)",
        drawn.len(),
        full
    );

    // Steady state: the full annulus (plus any accreted motes).
    run_frames_to_steady(&mut cloud, &mut frame);
    let drawn = cloud.black_hole_rain.drawn_cells_for_test();
    assert!(
        drawn.len() >= full,
        "the steady state must draw the full ball ({} of {})",
        drawn.len(),
        full
    );
}

#[test]
fn formation_gates_the_mote_spawn() {
    // Accretion begins only when the hole is whole: no motes while
    // the birth sequence runs, the formed flag flips at the sequence
    // end, and the ring then populates.
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);

    run_frames(&mut cloud, &mut frame, 100, 16);
    assert!(
        !cloud.black_hole_rain.formed_for_test(),
        "the hole must still be forming at 1.6 s"
    );
    assert_eq!(
        cloud.black_hole_rain.active_motes_for_test(),
        0,
        "no motes may orbit a half-born horizon"
    );

    run_frames_to_steady(&mut cloud, &mut frame);
    run_frames(&mut cloud, &mut frame, 60, 16);
    assert!(
        cloud.black_hole_rain.formed_for_test(),
        "the formed flag must flip once the sequence completes"
    );
    assert!(
        cloud.black_hole_rain.active_motes_for_test() > 0,
        "accretion must begin once the hole is whole"
    );
}

#[test]
fn formation_resize_keeps_the_steady_state() {
    // A pure resize rebuilds the geometry for the new viewport but
    // does NOT replay the birth: the very first frame after the reset
    // draws the full annulus (the hole re-forms only on style entry).
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames_to_steady(&mut cloud, &mut frame);

    cloud.reset(100, 30);
    let mut frame = Frame::new(100, 30, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 3, 16);

    assert!(
        cloud.black_hole_rain.formed_for_test(),
        "resize must not rewind the formation clock"
    );
    let full = cloud.black_hole_rain.ring_cells_for_test().len();
    let drawn = cloud.black_hole_rain.drawn_cells_for_test();
    assert!(
        drawn.len() >= full,
        "the first post-resize frames must draw the full ball ({} of {})",
        drawn.len(),
        full
    );
}

#[test]
fn formation_style_reentry_replays_the_sequence() {
    // Style re-entry rewinds the formation clock: the hole re-forms
    // from the singularity dot (the scene switch is the replay
    // trigger, distinct from the resize path above).
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames_to_steady(&mut cloud, &mut frame);
    assert!(cloud.black_hole_rain.formed_for_test());

    cloud.transition_rain_style(RainStyle::Vortex);
    cloud.transition_rain_style(RainStyle::BlackHole);
    assert!(
        !cloud.black_hole_rain.formed_for_test(),
        "re-entry must re-arm the formation"
    );
    assert!(
        (cloud.black_hole_rain.formation_t_for_test()).abs() < f32::EPSILON,
        "re-entry must rewind the formation clock to zero"
    );

    run_frames(&mut cloud, &mut frame, 5, 16);
    let drawn = cloud.black_hole_rain.drawn_cells_for_test();
    assert!(
        !drawn.is_empty() && drawn.len() <= 5,
        "the replay must begin with the seed dot (got {} cells)",
        drawn.len()
    );
}
