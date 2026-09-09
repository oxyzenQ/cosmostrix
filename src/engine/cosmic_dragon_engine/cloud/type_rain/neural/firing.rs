// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The firing passes (NIGHT-research-9) — the machine's drama:
//! the genesis seams' choreography, the neuron fire and the
//! thought burst.
//!
//! Split from `neural.rs` for the 800-line source cap (the
//! family's per-concern file split): the state machine owns the
//! pools and the clocks, this file owns the moments the machine
//! ACTS — the phase seams that complete the build and arm the
//! wire sweep, the wiring completion and the first cascade, the
//! per-neuron fire that launches the pulses, and the burst that
//! force-fires a clump of inputs so a whole wave crosses the
//! machine (the feed-flare's heir, the murmuration startle's
//! heir).

use rand::distr::Distribution;

use crate::constants::{
    NEUR_BURST_INPUTS, NEUR_FANOUT, NEUR_REFRACTORY, NEUR_THRESHOLD, NEUR_WIRE_STAGGER_WINDOW,
};

use super::genesis::GenesisPhase;
use super::network::{pulse_speed, NeuralRandom, Synapse};
use super::neural::NeuralRain;

impl NeuralRain {
    /// The genesis seams: the Wire entry completes the build and
    /// arms the wire sweep (the staggered growth), the Thought
    /// entry completes the wiring (the deterministic no-seam
    /// handoff) and fires the first cascade.
    pub(super) fn run_phase_seam(&mut self, phase: GenesisPhase) {
        match phase {
            GenesisPhase::Wire => {
                // The build completes: any neurons the rain has
                // not yet materialized pop in cold (deterministic
                // — the wiring needs the full layer set).
                for n in &mut self.nodes {
                    if !n.active {
                        n.activate(
                            n.layer,
                            n.x,
                            n.y,
                            n.first_syn,
                            n.is_output,
                            self.field_palette_slot,
                        );
                        self.node_active += 1;
                    }
                }
                // The wire sweep: the stagger arms every wire (the
                // growth sweeps the machine layer-pair by
                // layer-pair).
                let total = self.synapses.len();
                for (i, s) in self.synapses.iter_mut().enumerate() {
                    let stagger = if total > 0 {
                        (i as f32 / total as f32) * NEUR_WIRE_STAGGER_WINDOW
                    } else {
                        0.0
                    };
                    s.grow_delay = stagger;
                }
            }
            GenesisPhase::Thought => {
                // The wiring completes (the machine finishes
                // wiring exactly when it first thinks) and the
                // first cascade fires: the birth's money shot.
                for s in &mut self.synapses {
                    s.grown = 1.0;
                    s.grow_delay = 0.0;
                }
                let input_n = self.geom.nodes_per_layer.first().copied().unwrap_or(0);
                let clamped = input_n.clamp(1, NEUR_BURST_INPUTS);
                for i in 0..clamped {
                    if let Some(n) = self.nodes.get_mut(i) {
                        if n.active {
                            n.potential = NEUR_THRESHOLD;
                            n.refract = 0.0;
                        }
                    }
                }
            }
            GenesisPhase::Signal | GenesisPhase::Layers | GenesisPhase::Steady => {}
        }
    }

    /// Fire one neuron: the state resets, the window opens, the
    /// flash lights (the glyph re-rolls at the flash's peak in
    /// the draw pass — the event-gated shimmer), and the pulses
    /// launch down the fanout (only on conducting wires — grown
    /// and not retiring; the pool's free slots bound the volley,
    /// a saturated machine sheds load like a real refractory
    /// substrate).
    pub(super) fn fire_node(&mut self, node_idx: usize, random: &mut NeuralRandom<'_>) {
        #[cfg(test)]
        {
            self.fires_for_test += 1;
        }
        let (first_syn, slot) = {
            let n = &mut self.nodes[node_idx];
            n.mark_fired(NEUR_REFRACTORY);
            (n.first_syn, n.palette_slot)
        };
        for k in 0..NEUR_FANOUT {
            let syn = first_syn + k;
            let conducts = self.synapses.get(syn).is_some_and(Synapse::conducts);
            if !conducts {
                continue;
            }
            let Some(free) = self.find_inactive_pulse() else {
                break;
            };
            let roll = random.rand_chance.sample(random.rng);
            let speed = pulse_speed(roll);
            self.pulses[free].launch(syn, speed, slot);
            self.pulse_active += 1;
        }
    }

    /// The thought burst (the drama event): a clump of inputs
    /// force-fires together — the wave crosses the machine (the
    /// murmuration startle's heir).
    pub(super) fn fire_burst(&mut self, random: &mut NeuralRandom<'_>) {
        let input_n = self.geom.nodes_per_layer.first().copied().unwrap_or(0);
        if input_n == 0 {
            return;
        }
        let count = input_n.clamp(1, NEUR_BURST_INPUTS);
        for _ in 0..count {
            let roll = random.rand_chance.sample(random.rng);
            let idx = (roll * input_n as f32) as usize % input_n;
            if let Some(n) = self.nodes.get_mut(idx) {
                if n.active {
                    n.potential = NEUR_THRESHOLD;
                    n.refract = 0.0;
                }
            }
        }
    }
}
