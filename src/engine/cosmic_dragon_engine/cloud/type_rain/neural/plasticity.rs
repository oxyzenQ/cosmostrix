// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The learning economy (NIGHT-research-9): the free-slot scans,
//! the capture handler (law 3's accounting — the rain that
//! builds and feeds the machine) and the plasticity pass (law 5 —
//! the retire-and-regrow rewiring).
//!
//! Split from `neural.rs` for the 800-line source cap (the
//! family's per-concern file split): the state machine owns the
//! pools and the clocks, this file owns what happens when the
//! data lands and when a connection retires. The economy's two
//! arms mirror the quasar capture economy's fresh-write/steady-
//! feed split:
//! - Below population: the capture BIRTHS a neuron (the streamer
//!   materializes the next cell in the build order, flashed —
//!   the fresh write; the network is BUILT from the rain).
//! - At population: the capture kicks the nearest input neuron's
//!   potential (the data feeds the machine until it thinks).
//!
//! The plasticity (law 5): the rewire clock retires one healthy
//! wire (the fade — its riding pulses finish their trips first)
//! and grows the successor in the SAME slot (the wire count is
//! constant by construction — a connection retires only into a
//! replacement, nothing accumulates). The successor reaches from
//! the same source to a NEW target: the dendrite re-grows, the
//! machine's topology slowly rewrites itself — the rain's long
//! memory.
//!
//! The bookkeeping contract (the DNA starvation lesson, pinned by
//! tests): every deactivation decrements its counter and every
//! activation increments it — the spawn gate reads the true
//! population, the sky never starves.

use rand::distr::Distribution;

use crate::constants::{NEUR_CAPTURE_KICK, NEUR_POT_CAP};

use super::network::{wire_path, NeuralGeom, NeuralRandom};
use super::neural::NeuralRain;

impl NeuralRain {
    /// Amortized free-slot scan for the pulse pool (the fire
    /// pass's launch path).
    pub(super) fn find_inactive_pulse(&mut self) -> Option<usize> {
        let len = self.pulses.len();
        if len == 0 {
            return None;
        }
        for step in 0..len {
            let idx = (self.pulse_scan_idx + step) % len;
            if !self.pulses[idx].active {
                self.pulse_scan_idx = (idx + 1) % len;
                return Some(idx);
            }
        }
        None
    }

    /// The capture economy (law 3): an absorbed streamer either
    /// births a neuron (while the machine is below its layout
    /// population — the fresh write, flashed once) or kicks the
    /// nearest active input neuron's potential (the steady feed —
    /// the data charges the machine until it thinks).
    pub(super) fn run_capture(&mut self, x: f32) {
        #[cfg(test)]
        {
            self.absorptions_for_test += 1;
        }
        // Arm 1 — the build: the streamer materializes the next
        // cell in the deterministic build order (the input band
        // first, then the hidden layers, then the output — the
        // machine assembles top to bottom).
        if self.node_active < self.nodes.len() {
            if let Some(idx) = (0..self.nodes.len()).find(|&i| !self.nodes[i].active) {
                let (layer, nx, ny, first_syn, is_output) = {
                    let n = &self.nodes[idx];
                    (n.layer, n.x, n.y, n.first_syn, n.is_output)
                };
                self.nodes[idx].activate(
                    layer,
                    nx,
                    ny,
                    first_syn,
                    is_output,
                    self.field_palette_slot,
                );
                // The birth flash: the fresh cell lights once (the
                // capture's receipt — the light shows where the
                // rain became the machine).
                self.nodes[idx].flash = 0.8;
                self.node_active += 1;
                return;
            }
        }
        // Arm 2 — the steady feed: the nearest active input neuron
        // by column distance takes the kick.
        let input_n = self.geom.nodes_per_layer.first().copied().unwrap_or(0);
        let mut best: Option<usize> = None;
        let mut best_d = f32::MAX;
        for i in 0..input_n.min(self.nodes.len()) {
            if !self.nodes[i].active {
                continue;
            }
            let d = (self.nodes[i].x as f32 - x).abs();
            if d < best_d {
                best_d = d;
                best = Some(i);
            }
        }
        if let Some(i) = best {
            let n = &mut self.nodes[i];
            n.potential = (n.potential + NEUR_CAPTURE_KICK).min(NEUR_POT_CAP);
        }
    }

    /// Begin a rewire (law 5's first arm): pick a healthy wire
    /// (grown, conducting, carrying cells) and begin its retire
    /// fade — the bounded scan keeps the pick cheap on saturated
    /// machines (no wire conducts: no rewire, the machine is
    /// learning nothing worth rewiring).
    pub(super) fn begin_rewire(&mut self, random: &mut NeuralRandom<'_>) {
        let total = self.synapses.len();
        if total == 0 {
            return;
        }
        for _ in 0..8 {
            let roll = random.rand_chance.sample(random.rng);
            let idx = (roll * total as f32) as usize % total;
            let healthy = {
                let s = &self.synapses[idx];
                s.conducts() && !s.path.is_empty()
            };
            if healthy {
                self.synapses[idx].begin_retire();
                self.retiring = Some(idx);
                return;
            }
        }
    }

    /// Grow the retiring wire's successor (law 5's second arm —
    /// the fade completed, the riding pulses have landed): the
    /// SAME source node reaches to a NEW target in the next
    /// layer, the path and weight rebuilt, the growth starting
    /// immediately (the dendrite reach-out replayed in
    /// miniature). The successor reuses the retired slot — the
    /// wire count is constant by construction.
    pub(super) fn grow_successor(&mut self, random: &mut NeuralRandom<'_>) {
        let Some(idx) = self.retiring else {
            return;
        };
        self.retiring = None;
        let (src, old_dst) = match self.synapses.get(idx) {
            Some(s) => (s.src, s.dst),
            None => return,
        };
        let next = match self.nodes.get(src) {
            Some(n) => n.layer as usize + 1,
            None => return,
        };
        let n_layers = self.geom.nodes_per_layer.len();
        if next >= n_layers {
            // The source is an output cell (no outgoing layer) —
            // nothing to rewire (the pool never wires these; the
            // guard is defense in depth).
            return;
        }
        let n_next = self.geom.nodes_per_layer[next];
        if n_next == 0 {
            return;
        }
        let next_start: usize = self.geom.nodes_per_layer[..next].iter().sum();
        let roll = random.rand_chance.sample(random.rng);
        let mut dst_in = (roll * n_next as f32) as usize % n_next;
        // A different target when there is a choice (the same
        // destination would be a no-op rewire).
        if n_next > 1 && next_start + dst_in == old_dst {
            dst_in = (dst_in + 1) % n_next;
        }
        let dst = next_start + dst_in;
        let a = (self.nodes[src].x, self.nodes[src].y);
        let b = (self.nodes[dst].x, self.nodes[dst].y);
        let path = wire_path(a, b, NeuralGeom::wire_bow(src, dst));
        let weight = NeuralGeom::wire_weight(src, dst);
        if let Some(s) = self.synapses.get_mut(idx) {
            s.activate(src, dst, path, weight, 0.0, self.field_palette_slot);
        }
    }
}
