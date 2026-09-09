// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Genesis contracts (NIGHT-research-9, law 0): the pure phase
//! math (the timeline ordering, the luminosity ramp, the
//! no-seam handoff) and the orchestration's replay of the birth
//! (the entry re-arms, the fast-forward fills, a lit machine
//! rewinds to the dark start).

use super::*;

use crate::cloud::neural::genesis::{genesis_phase, genesis_total_secs, luminosity, GenesisPhase};

#[test]
fn genesis_timeline_is_ordered_and_complete() {
    // The phase classifier: every window positive, the phases
    // strictly ordered, the total the exact sum.
    let signal = crate::constants::NEUR_GENESIS_SIGNAL_SECS;
    let layers = crate::constants::NEUR_GENESIS_LAYERS_SECS;
    let wire = crate::constants::NEUR_GENESIS_WIRE_SECS;
    let thought = crate::constants::NEUR_GENESIS_THOUGHT_SECS;
    assert!(signal > 0.0 && layers > 0.0 && wire > 0.0 && thought > 0.0);
    assert_eq!(genesis_total_secs(), signal + layers + wire + thought);

    assert_eq!(genesis_phase(0.0), GenesisPhase::Signal);
    assert_eq!(genesis_phase(signal - 0.001), GenesisPhase::Signal);
    assert_eq!(genesis_phase(signal), GenesisPhase::Layers);
    assert_eq!(genesis_phase(signal + layers - 0.001), GenesisPhase::Layers);
    assert_eq!(genesis_phase(signal + layers), GenesisPhase::Wire);
    assert_eq!(
        genesis_phase(signal + layers + wire - 0.001),
        GenesisPhase::Wire
    );
    assert_eq!(genesis_phase(signal + layers + wire), GenesisPhase::Thought);
    assert_eq!(
        genesis_phase(genesis_total_secs() - 0.001),
        GenesisPhase::Thought
    );
    assert_eq!(genesis_phase(genesis_total_secs()), GenesisPhase::Steady);
    assert_eq!(
        genesis_phase(genesis_total_secs() + 99.0),
        GenesisPhase::Steady
    );
}

#[test]
fn genesis_luminosity_ramps_with_no_seam() {
    // The luminosity: 0 through the data cloud, the layer build
    // and the wiring (nothing thinks), a linear ramp over the
    // first-thought window, 1 from the steady state on — and the
    // ramp's last frame equals the steady law exactly (no seam).
    let signal = crate::constants::NEUR_GENESIS_SIGNAL_SECS;
    let layers = crate::constants::NEUR_GENESIS_LAYERS_SECS;
    let wire = crate::constants::NEUR_GENESIS_WIRE_SECS;
    let thought = crate::constants::NEUR_GENESIS_THOUGHT_SECS;
    let start = signal + layers + wire;
    assert_eq!(luminosity(0.0), 0.0);
    assert_eq!(luminosity(start - 0.001), 0.0);
    assert_eq!(luminosity(start), 0.0);
    let mid = start + thought * 0.5;
    assert!((luminosity(mid) - 0.5).abs() < 1e-4);
    assert_eq!(luminosity(start + thought), 1.0);
    assert_eq!(luminosity(start + thought + 99.0), 1.0);
    // Continuity: the ramp has no jump (monotone linear).
    let mut prev = luminosity(start);
    for i in 1..=40 {
        let t = start + thought * (i as f32 / 40.0);
        let v = luminosity(t);
        assert!(v >= prev - 1e-6, "the luminosity ramp is not monotone");
        prev = v;
    }
}

#[test]
fn genesis_first_cascade_fires_at_the_thought_seam() {
    // The Thought seam completes the wiring and fires the first
    // cascade: the machine's first visible thought (the birth's
    // money shot).
    let mut cloud = make_neur_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    // Run to just inside the Thought window (the seam ran: the
    // wiring completed, the cascade charged).
    let signal = crate::constants::NEUR_GENESIS_SIGNAL_SECS;
    let layers = crate::constants::NEUR_GENESIS_LAYERS_SECS;
    let wire = crate::constants::NEUR_GENESIS_WIRE_SECS;
    let thought_frames = ((signal + layers + wire) / DT_SIM_PER_FRAME).ceil() as u32 + 4;
    run_frames(&mut cloud, &mut frame, thought_frames, 16);
    // The cascade's force-fires show up as real fires (the
    // threshold + zero refractory gate).
    assert!(
        cloud.neural_rain.fires_for_test() > 0,
        "the first cascade never fired"
    );
    // The wiring completed at the seam (the no-seam handoff).
    assert!(cloud
        .neural_rain
        .synapses
        .iter()
        .all(|s| !s.active || s.grown >= 1.0));
}

#[test]
fn genesis_entry_rewinds_the_machine() {
    // The scene-entry contract: begin_genesis rewinds a lit
    // machine to the dark start (the birth replays on entry, not
    // on resize).
    let mut cloud = make_neur_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    let genesis_frames = (8.2 / DT_SIM_PER_FRAME).ceil() as u32;
    run_frames(&mut cloud, &mut frame, genesis_frames + 60, 16);
    assert!(cloud.neural_rain.lit_for_test());
    cloud.neural_rain.begin_genesis();
    assert!(!cloud.neural_rain.lit_for_test());
    assert_eq!(cloud.neural_rain.genesis_t_for_test(), 0.0);
    // The pools wiped: the dormant machine counts nothing (the
    // family exit contract).
    assert_eq!(cloud.neural_rain.node_active_for_test(), 0);
    assert_eq!(cloud.neural_rain.pulse_active_for_test(), 0);
}

#[test]
fn genesis_fast_forward_fills_the_steady_machine() {
    // The bench fast-forward (the Z-6 critical-path contract): the
    // choreography is skipped deterministically — every neuron
    // materializes, every wire stands complete.
    let mut cloud = make_neur_cloud(120, 40);
    cloud.neural_rain.fast_forward_genesis();
    assert!(cloud.neural_rain.lit_for_test());
    assert_eq!(
        cloud.neural_rain.node_active_for_test(),
        cloud.neural_rain.nodes.len()
    );
    assert!(cloud
        .neural_rain
        .synapses
        .iter()
        .all(|s| !s.active || s.grown >= 1.0));
}
