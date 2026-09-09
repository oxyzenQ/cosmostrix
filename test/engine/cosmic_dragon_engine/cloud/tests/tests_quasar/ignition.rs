// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Ignition contracts (NIGHT-research-8, law 0): the pure phase
//! math (the timeline ordering, the luminosity and jet fronts,
//! the no-seam handoff) and the orchestration's replay of the
//! birth (the entry re-arms, the fast-forward fills, a lit engine
//! rewinds to the dark start).

use super::*;

use crate::cloud::quasar::ignition::{
    ignition_phase, ignition_total_secs, jet_front, luminosity, IgnitionPhase,
};

#[test]
fn ignition_timeline_is_ordered_and_complete() {
    // The phase classifier: every window positive, the phases
    // strictly ordered, the total the exact sum.
    let dark = crate::constants::QUAS_IGNITION_DARK_SECS;
    let disk = crate::constants::QUAS_IGNITION_DISK_SECS;
    let light = crate::constants::QUAS_IGNITION_LIGHT_SECS;
    let jets = crate::constants::QUAS_IGNITION_JET_SECS;
    assert!(dark > 0.0 && disk > 0.0 && light > 0.0 && jets > 0.0);
    assert_eq!(ignition_total_secs(), dark + disk + light + jets);

    assert_eq!(ignition_phase(0.0), IgnitionPhase::Dark);
    assert_eq!(ignition_phase(dark - 0.001), IgnitionPhase::Dark);
    assert_eq!(ignition_phase(dark), IgnitionPhase::Disk);
    assert_eq!(ignition_phase(dark + disk - 0.001), IgnitionPhase::Disk);
    assert_eq!(ignition_phase(dark + disk), IgnitionPhase::Light);
    assert_eq!(
        ignition_phase(dark + disk + light - 0.001),
        IgnitionPhase::Light
    );
    assert_eq!(ignition_phase(dark + disk + light), IgnitionPhase::Jets);
    assert_eq!(
        ignition_phase(ignition_total_secs() - 0.001),
        IgnitionPhase::Jets
    );
    assert_eq!(ignition_phase(ignition_total_secs()), IgnitionPhase::Steady);
    assert_eq!(
        ignition_phase(ignition_total_secs() + 99.0),
        IgnitionPhase::Steady
    );
}

#[test]
fn ignition_luminosity_ramps_with_no_seam() {
    // The luminosity: 0 through the dark cloud and the disk
    // condensation, a linear ramp through first light, 1 from the
    // jets on — and the handoff is exact (the last ramp frame
    // evaluates to the steady value).
    let light_start =
        crate::constants::QUAS_IGNITION_DARK_SECS + crate::constants::QUAS_IGNITION_DISK_SECS;
    let light_end = light_start + crate::constants::QUAS_IGNITION_LIGHT_SECS;

    assert_eq!(luminosity(0.0), 0.0);
    assert_eq!(luminosity(light_start - 0.001), 0.0);
    assert!(luminosity(light_start + 0.001) > 0.0);
    assert_eq!(luminosity(light_end), 1.0);
    assert_eq!(luminosity(ignition_total_secs()), 1.0);

    // Monotone through the ramp, linear in the middle.
    let mut prev = -1.0;
    let mut t = light_start;
    while t <= light_end {
        let v = luminosity(t);
        assert!(v >= prev, "the luminosity ramp is not monotone at t={t:.3}");
        prev = v;
        t += 0.05;
    }
    let mid = luminosity(light_start + crate::constants::QUAS_IGNITION_LIGHT_SECS / 2.0);
    assert!(
        (mid - 0.5).abs() < 0.01,
        "the ramp is not linear at the midpoint: {mid:.3}"
    );
}

#[test]
fn ignition_jet_front_extends_with_no_seam() {
    // The jet front: 0 through first light, a linear push through
    // the jet window, 1 in the steady state — the same no-seam
    // contract (the final frame of the extension is the full
    // beam).
    let jet_start = crate::constants::QUAS_IGNITION_DARK_SECS
        + crate::constants::QUAS_IGNITION_DISK_SECS
        + crate::constants::QUAS_IGNITION_LIGHT_SECS;
    let jet_end = jet_start + crate::constants::QUAS_IGNITION_JET_SECS;

    assert_eq!(jet_front(0.0), 0.0);
    assert_eq!(jet_front(jet_start - 0.001), 0.0);
    assert!(jet_front(jet_start + 0.001) > 0.0);
    assert_eq!(jet_front(jet_end), 1.0);
    assert_eq!(jet_front(ignition_total_secs()), 1.0);

    let mid = jet_front(jet_start + crate::constants::QUAS_IGNITION_JET_SECS / 2.0);
    assert!(
        (mid - 0.5).abs() < 0.01,
        "the front is not linear at the midpoint: {mid:.3}"
    );
}

#[test]
fn quas_entry_replays_the_birth() {
    // The scene-entry contract: a fresh cloud starts dark; the
    // dark phase shows nothing lit (the core does not draw, the
    // beams do not exist), and the ignition clock advances with
    // the frames.
    let mut cloud = make_quas_cloud(100, 40);
    let mut frame = Frame::new(100, 40, cloud.palette.bg);
    assert!(
        !cloud.quasar_rain.lit_for_test(),
        "a fresh entry starts already lit"
    );
    assert_eq!(cloud.quasar_rain.jet_states_for_test().len(), 0);
    // Half a second in: still the dark cloud (the ignition is
    // 8.1 sim-s; 30 frames = 0.72 sim-s).
    run_frames(&mut cloud, &mut frame, 30, 16);
    assert_eq!(cloud.quasar_rain.phase(), IgnitionPhase::Dark);
    assert!(cloud.quasar_rain.ignition_t_for_test() > 0.5);
    assert!(!cloud.quasar_rain.lit_for_test());
}

#[test]
fn quas_phase_progression_drives_the_assembly() {
    // The full timeline: dark -> disk -> light -> jets -> steady,
    // each phase entered on the family clock.
    let mut cloud = make_quas_cloud(100, 40);
    let mut frame = Frame::new(100, 40, cloud.palette.bg);
    let frames_for = |sim_s: f32| (sim_s / DT_SIM_PER_FRAME).ceil() as u32;

    run_frames(&mut cloud, &mut frame, frames_for(0.5), 16);
    assert_eq!(cloud.quasar_rain.phase(), IgnitionPhase::Dark);

    // 4.1 sim-s in: still the disk phase (the window runs to 4.6),
    // and the captures have begun (a streamer's transit is
    // ~3 sim-s from its spawn, so the first landings arrive here).
    run_frames(&mut cloud, &mut frame, frames_for(3.6), 16);
    assert_eq!(cloud.quasar_rain.phase(), IgnitionPhase::Disk);
    // The disk is condensing: captures have begun.
    assert!(
        cloud.quasar_rain.disk_active_for_test() > 0,
        "the disk never began condensing"
    );

    run_frames(&mut cloud, &mut frame, frames_for(1.5), 16);
    assert_eq!(cloud.quasar_rain.phase(), IgnitionPhase::Light);

    run_frames(&mut cloud, &mut frame, frames_for(1.0), 16);
    assert_eq!(cloud.quasar_rain.phase(), IgnitionPhase::Jets);
    // The beams fired with the phase.
    assert!(
        !cloud.quasar_rain.jet_states_for_test().is_empty(),
        "the jets did not fire with their phase"
    );

    run_frames(&mut cloud, &mut frame, frames_for(2.0), 16);
    assert!(cloud.quasar_rain.lit_for_test());
    assert_eq!(cloud.quasar_rain.phase(), IgnitionPhase::Steady);
}

#[test]
fn quas_fast_forward_fills_the_steady_engine() {
    // The bench contract (the Z-6 critical-path precedent): the
    // fast-forward completes the ignition and fills every pool to
    // the steady state — deterministic, no RNG.
    let mut cloud = make_quas_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    cloud.quasar_rain.fast_forward_ignition();
    assert!(cloud.quasar_rain.lit_for_test());
    assert_eq!(
        cloud.quasar_rain.disk_active_for_test(),
        cloud.quasar_rain.disk.len()
    );
    assert_eq!(
        cloud.quasar_rain.jet_states_for_test().len(),
        crate::constants::QUAS_JET_PER_BEAM * 2
    );
    // The fast-forwarded engine renders immediately (the bench
    // measures the burning engine, not the birth).
    run_frames(&mut cloud, &mut frame, 30, 16);
    assert!(!cloud.quasar_rain.drawn_cells_for_test().is_empty());
    // Deterministic: a second fast-forward produces the same
    // ignition time (no re-roll).
    let t1 = cloud.quasar_rain.ignition_t_for_test();
    cloud.quasar_rain.fast_forward_ignition();
    assert_eq!(t1, cloud.quasar_rain.ignition_t_for_test());
}

#[test]
fn quas_begin_ignition_rewinds_a_lit_engine() {
    // The re-arm contract: a burning engine rewinds to the dark
    // start (the pools wipe, the clock resets).
    let mut cloud = make_quas_cloud(100, 40);
    let mut frame = Frame::new(100, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 900, 16);
    assert!(cloud.quasar_rain.lit_for_test());

    cloud.quasar_rain.begin_ignition();
    assert!(!cloud.quasar_rain.lit_for_test());
    assert_eq!(cloud.quasar_rain.ignition_t_for_test(), 0.0);
    assert_eq!(cloud.quasar_rain.disk_active_for_test(), 0);
    assert_eq!(cloud.quasar_rain.jet_states_for_test().len(), 0);
    // The birth replays from scratch.
    run_frames(&mut cloud, &mut frame, 900, 16);
    assert!(cloud.quasar_rain.lit_for_test());
}
