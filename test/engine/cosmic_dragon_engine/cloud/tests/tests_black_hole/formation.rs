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

// ── NIGHT-hunt-31: the intro-handover dwell skip ──

#[test]
fn skip_formation_dwell_lands_on_the_horizon_bloom() {
    // The cinematic intro hands over with a one-frame full-screen
    // wipe; the dwell skip must land the formation clock exactly at
    // the horizon-bloom start (SEED + COLLAPSE) so the first
    // post-intro frame draws growing annulus content — never the
    // 1.9 s near-empty seed/collapse dwell (the owner's flash
    // report: 1.62 s measured dead window at 120x40).
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);

    // Fresh entry: clock at zero, unborn.
    assert!((cloud.black_hole_rain.formation_t_for_test()).abs() < f32::EPSILON);
    assert!(!cloud.black_hole_rain.formed_for_test());

    cloud.advance_birth_for_intro_handover();

    let dwell = crate::constants::BLACK_HOLE_FORM_SEED_SECS
        + crate::constants::BLACK_HOLE_FORM_COLLAPSE_SECS;
    let t = cloud.black_hole_rain.formation_t_for_test();
    assert!(
        (t - dwell).abs() < f32::EPSILON,
        "the dwell skip must land exactly at the bloom start ({dwell}), got {t}"
    );
    assert!(
        !cloud.black_hole_rain.formed_for_test(),
        "the skip must NOT mark the hole formed — the bloom + accretion still play"
    );

    // The very first frames after the handover draw bloom content:
    // strictly more than the five-cell seed/cross ceiling, strictly
    // less than the full ball (the annulus grows from the inside out).
    run_frames(&mut cloud, &mut frame, 8, 16);
    let full = cloud.black_hole_rain.ring_cells_for_test().len();
    let drawn = cloud.black_hole_rain.drawn_cells_for_test();
    assert!(
        !drawn.is_empty() && drawn.len() > 5,
        "the first post-handover frames must draw bloom content, not the seed dot (got {} cells)",
        drawn.len()
    );
    assert!(
        drawn.len() < full,
        "the bloom must still be partial on the first frames ({} of {})",
        drawn.len(),
        full
    );
}

#[test]
fn skip_formation_dwell_is_monotone_never_rewinds() {
    // A clock already at or past the dwell (e.g. a mid-bloom handover
    // from a future caller) must be left alone: the skip is a
    // fast-forward past the INVISIBLE dwell, never a rewind of
    // visible progress.
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);

    // Advance into the bloom by running frames (~2.1 s in).
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 131, 16);
    let before = cloud.black_hole_rain.formation_t_for_test();
    assert!(
        before
            > crate::constants::BLACK_HOLE_FORM_SEED_SECS
                + crate::constants::BLACK_HOLE_FORM_COLLAPSE_SECS
    );

    cloud.advance_birth_for_intro_handover();
    let after = cloud.black_hole_rain.formation_t_for_test();
    assert!(
        (after - before).abs() < f32::EPSILON,
        "a clock past the dwell must not move (before {before}, after {after})"
    );
}

#[test]
fn skip_formation_dwell_still_reaches_steady_state() {
    // The fast-forwarded birth must still complete: the bloom plays
    // out, the formed flag arms, and the steady state draws the full
    // ball (the accretion contract is untouched by the skip).
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);

    cloud.advance_birth_for_intro_handover();
    // Run past the remaining bloom window (1.2 s + margin).
    run_frames(&mut cloud, &mut frame, 120, 16);

    assert!(
        cloud.black_hole_rain.formed_for_test(),
        "the skip must not block the formed flag from arming"
    );
    let full = cloud.black_hole_rain.ring_cells_for_test().len();
    let drawn = cloud.black_hole_rain.drawn_cells_for_test();
    assert!(
        drawn.len() >= full,
        "the steady state must still draw the full ball ({} of {})",
        drawn.len(),
        full
    );
}

#[test]
fn skip_formation_dwell_leaves_other_styles_untouched() {
    // The Cloud dispatch is black-hole-only today (the only style
    // whose post-intro dead window measured outside the natural
    // first-spawn band): a glyph cloud must run through the handover
    // call with no panic and no state corruption — the call is a
    // no-op for every non-black-hole style.
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    cloud.transition_rain_style(RainStyle::Glyph);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);

    cloud.advance_birth_for_intro_handover();
    // The glyph family keeps spawning through natural fill — five
    // frames of rain with no interference from the handover call.
    run_frames(&mut cloud, &mut frame, 5, 16);
    assert!(
        cloud.rain_style == RainStyle::Glyph,
        "the handover call must not change the active style"
    );
}
