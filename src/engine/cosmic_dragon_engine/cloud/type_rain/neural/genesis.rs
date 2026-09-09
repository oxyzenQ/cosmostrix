// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Neural genesis intro (NIGHT-research-9, law 0): the pure phase
//! math for the network's birth sequence, split the way the DNA
//! helix splits its `genesis.rs` and the quasar its `ignition.rs`
//! (the state machine's mutable half — the genesis clock and the
//! lit flag — lives on `NeuralRain`; this file answers the
//! stateless questions).
//!
//! Owner mandate (the DNA genesis set the bar): a style must be
//! BORN, not popped in. The network's origin story is the
//! training run, in four continuous phases (each continuous in
//! time with the next — no pop at any seam):
//!
//! - Signal: the data falls (the streamers run their genesis
//!   multiplier — the sky is thicker than the steady drizzle
//!   because while the machine is unbuilt the rain IS the scene,
//!   the DNA soup precedent). Nothing is drawn but the data:
//!   no neurons, no wires, no thought.
//! - Layers: the captures build the machine (a landing streamer
//!   materializes the next neuron, layer by layer, the fresh-
//!   write economy), capped at Dim — cold cells, no thought.
//! - Wire: the dendrites reach out (each wire's growth is armed
//!   on the phase's stagger, so the wiring sweeps through the
//!   machine layer-pair by layer-pair — the assembly's most
//!   beautiful second). The phase's arrival completes every
//!   wire (the machine finishes wiring exactly when it first
//!   thinks — the deterministic no-seam handoff's setup).
//! - Thought: the wiring completes, the first cascade fires
//!   (the state machine force-fires a clump of inputs — the
//!   wave crosses the machine for the first time) and the
//!   luminosity ramps to full; at the phase's end the machine
//!   evaluates exactly to the steady law — the last frame of
//!   the genesis and the first frame of the steady state are
//!   identical, no seam.
//!
//! Timeline (sim-seconds since genesis start, constants in
//! style_rain.rs; the clock rides the engine's sim time — the
//! family speed contract scales the birth with the engine, the
//! DNA genesis precedent):
//! - [0, SIGNAL): the data falls.
//! - [SIGNAL, SIGNAL+LAYERS): the neurons materialize (the
//!   capture economy's build arm; this phase gates the node
//!   brightness cap and completes the build at its end).
//! - [+LAYERS, +WIRE): the dendrites extend (the staggered
//!   growth; the phase's arrival forces every wire complete).
//! - [+WIRE, +THOUGHT): the first cascade fires and the
//!   luminosity ramps 0 -> 1 (linear).
//! - after: steady (the lit flag flips; the queries return the
//!   full laws exactly).

/// The genesis phase at sim-time `t` since the sequence began.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GenesisPhase {
    /// The data falls: the thick cold streamers, nothing built.
    Signal,
    /// The layers build: the captures materialize the neurons.
    Layers,
    /// The wiring sweeps: the dendrites reach out and connect.
    Wire,
    /// The first thought: the cascade fires, the luminosity ramps.
    Thought,
    /// Steady state: the full machine under laws 1 through 5.
    Steady,
}

/// Classify the genesis timeline at sim-time `t`.
pub(crate) fn genesis_phase(t: f32) -> GenesisPhase {
    let signal = crate::constants::NEUR_GENESIS_SIGNAL_SECS;
    let layers = signal + crate::constants::NEUR_GENESIS_LAYERS_SECS;
    let wire = layers + crate::constants::NEUR_GENESIS_WIRE_SECS;
    let thought = wire + crate::constants::NEUR_GENESIS_THOUGHT_SECS;
    if t < signal {
        GenesisPhase::Signal
    } else if t < layers {
        GenesisPhase::Layers
    } else if t < wire {
        GenesisPhase::Wire
    } else if t < thought {
        GenesisPhase::Thought
    } else {
        GenesisPhase::Steady
    }
}

/// Total genesis duration (sim-seconds from the first falling
/// streamer to the steady machine).
pub(crate) fn genesis_total_secs() -> f32 {
    crate::constants::NEUR_GENESIS_SIGNAL_SECS
        + crate::constants::NEUR_GENESIS_LAYERS_SECS
        + crate::constants::NEUR_GENESIS_WIRE_SECS
        + crate::constants::NEUR_GENESIS_THOUGHT_SECS
}

/// The luminosity fraction in [0, 1] at sim-time `t` (the Thought
/// phase's ramp): 0 through the signal, the layers and the wire
/// (nothing thinks), a linear ramp over the first-thought window,
/// 1 from the steady state on (the machine runs at full law). The
/// neurons' and wires' brightness caps ride the same fraction —
/// the whole machine's light arrives together.
pub(crate) fn luminosity(t: f32) -> f32 {
    let start = crate::constants::NEUR_GENESIS_SIGNAL_SECS
        + crate::constants::NEUR_GENESIS_LAYERS_SECS
        + crate::constants::NEUR_GENESIS_WIRE_SECS;
    let dur = crate::constants::NEUR_GENESIS_THOUGHT_SECS;
    if t <= start {
        return 0.0;
    }
    if t >= start + dur {
        return 1.0;
    }
    (t - start) / dur
}

// The shipped calibration must stay internally consistent — the
// compile-time checks mirror the engine's other constant
// contracts (the phase classifier's ordering depends on every
// window being positive: a zero window would make a phase
// unreachable, a negative one would mis-order the timeline).
const _: () = assert!(crate::constants::NEUR_GENESIS_SIGNAL_SECS > 0.0);
const _: () = assert!(crate::constants::NEUR_GENESIS_LAYERS_SECS > 0.0);
const _: () = assert!(crate::constants::NEUR_GENESIS_WIRE_SECS > 0.0);
const _: () = assert!(crate::constants::NEUR_GENESIS_THOUGHT_SECS > 0.0);
