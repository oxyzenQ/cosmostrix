// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The neural network state machine (NIGHT-research-9, the
//! fourteenth style) — the orchestration pass set: pools, spawn,
//! advance, draw.
//!
//! The style is the training run (see mod.rs for the full
//! derivation): four populations — the static neurons, the
//! synapse wiring, the traveling pulses and the falling data —
//! advanced on the single family clock. The streamers ride the
//! spawn accumulator (the staggered entry, the family contract);
//! the network is not spawned, it is TRAINED from the rain (every
//! absorbed streamer builds a neuron or charges an input, the
//! capture economy); the wires grow at the genesis wire phase and
//! rewire through plasticity forever after; the thoughts fire
//! when the potentials cross the threshold.
//!
//! The genesis (law 0) rides sim-time: a scene entry re-arms the
//! birth (reset + begin_genesis), a pure resize keeps the trained
//! network (the pools rebuild to the steady state when already
//! lit), and the bench fast-forwards past the choreography (the
//! Z-6 critical-path contract — the genesis tests pin the
//! sequence frame by frame instead).

use std::time::{Duration, Instant};

use rand::distr::Distribution;

use crate::constants::{
    NEUR_BURST_CLOCK_MEAN, NEUR_BURST_SURGE, NEUR_BURST_WINDOW, NEUR_FANOUT,
    NEUR_GENESIS_SIGNAL_MULT, NEUR_MAX_STREAMER, NEUR_MIN_STREAMER, NEUR_POT_CAP, NEUR_PULSE_POOL,
    NEUR_REWIRE_CLOCK_MEAN, NEUR_SIM_TIME_PER_CPS, NEUR_SPONT_KICK, NEUR_SPONT_RATE,
    NEUR_STREAMER_SURGE_CAP, NEUR_THRESHOLD,
};

use super::genesis::{genesis_phase, genesis_total_secs, GenesisPhase};
use super::network::{
    growth_step, neuron_step, pulse_step, streamer_step, synapse_step, NeuralGeom, NeuralRandom,
    Neuron, Pulse, Streamer, Synapse,
};

/// One drawn cell (col, line) — the diff-cleanup currency (same
/// shape as the family's cells).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NeurCell {
    pub(crate) col: u16,
    pub(crate) line: u16,
}

/// Spawn inputs (mirrors the family bundles — the accumulator
/// contract's parameter set).
pub(crate) struct NeurSpawnParams {
    pub(crate) cols: u16,
    pub(crate) lines: u16,
    /// The density dial's read for the streamer target (the
    /// data-rate dial; the neuron counts are set at reset).
    pub(crate) density: f32,
    pub(crate) active_palette_slot: u8,
    pub(crate) spawn_scale: f32,
}

/// Per-frame step inputs (mirrors `QuasStep`/`MurmStep`).
pub(crate) struct NeurStep {
    pub(crate) now: Instant,
    /// chars_per_sec already multiplied by the terminal
    /// speed_mult. Scales the whole machine on one clock (dt_sim)
    /// — the family speed contract: the rain, the pulses, the
    /// clocks and the genesis all survive the speed keys
    /// together.
    pub(crate) chars_per_sec: f32,
    pub(crate) cols: u16,
    pub(crate) lines: u16,
    pub(crate) max_sim_delta: Duration,
    pub(crate) resume_blend: f32,
}

/// The neural network: the four populations and the clocks — one
/// state machine.
#[derive(Debug)]
pub(crate) struct NeuralRain {
    /// Law 1's neurons (built from captures, static after).
    pub(crate) nodes: Vec<Neuron>,
    /// Law 2's wiring (grown at the genesis wire phase, rewired
    /// through plasticity).
    pub(crate) synapses: Vec<Synapse>,
    /// Law 4's pulses (the traveling signals).
    pub(crate) pulses: Vec<Pulse>,
    /// Law 3's streamers (the data — the accumulator's
    /// population).
    pub(crate) streamers: Vec<Streamer>,
    /// The viewport layout (the layer anchors, the counts, the
    /// absorption edge).
    pub(crate) geom: NeuralGeom,
    /// The genesis clock (sim-seconds since the birth began).
    genesis_t: f32,
    /// The steady flag (the genesis has completed).
    lit: bool,
    /// The phase classifier's last read (the transition detector
    /// — the wire sweep and the first thought fire on the seams).
    last_phase: GenesisPhase,
    /// Sim-seconds until the next thought burst.
    burst_clock: f32,
    /// The active burst's age (None while the machine is calm).
    /// `pub(super)`: the draw pass reads it (the surge read and
    /// the wires' flare rung).
    pub(super) burst_age: Option<f32>,
    /// Sim-seconds until the next plasticity rewire.
    rewire_clock: f32,
    /// The retiring synapse's index (None while the wiring is
    /// stable — at most one rewire in flight, the bounded
    /// plasticity). `pub(super)`: the plasticity economy owns it.
    pub(super) retiring: Option<usize>,
    /// Global motion clock (dt = now - last_step, clamped by
    /// max_sim_delta and eased by resume_blend — the family
    /// contract; a fully-paused run simply stops advancing).
    last_step: Option<Instant>,
    pub(super) current_cells: Vec<NeurCell>,
    pub(super) previous_cells: Vec<NeurCell>,
    pub(super) drawn_gen: Vec<u32>,
    pub(super) drawn_gen_counter: u32,
    /// Palette slot of the machine (one network, one slot).
    pub(super) field_palette_slot: u8,
    /// The active-population counters (the starvation-free
    /// bookkeeping — every activation/deactivation flows through
    /// them). `pub(super)`: the economies (plasticity.rs) write
    /// them.
    pub(super) node_active: usize,
    pub(super) pulse_active: usize,
    pub(super) streamer_active: usize,
    /// Rotating scan cursors for amortized O(1) free-slot search
    /// (mirrors the family). `pub(super)`: the economies own the
    /// scans.
    pub(super) pulse_scan_idx: usize,
    pub(super) streamer_scan_idx: usize,
    /// Total absorptions since reset (test-only dial).
    /// `pub(super)`: the capture economy counts it.
    #[cfg(test)]
    pub(super) absorptions_for_test: usize,
    /// Total thought bursts since the machine lit (test-only
    /// dial). `pub(super)`: the firing pass counts it.
    #[cfg(test)]
    pub(super) bursts_for_test: usize,
    /// Total rewires since the machine lit (test-only dial).
    #[cfg(test)]
    rewires_for_test: usize,
    /// Total neuron fires since reset (test-only dial).
    /// `pub(super)`: the firing pass counts it.
    #[cfg(test)]
    pub(super) fires_for_test: usize,
}

impl NeuralRain {
    pub(crate) fn new() -> Self {
        // The machine is born unborn: a plain `--scene neural`
        // startup without a scene transition replays the genesis
        // (the DNA genesis contract).
        Self {
            nodes: Vec::new(),
            synapses: Vec::new(),
            pulses: Vec::new(),
            streamers: Vec::new(),
            geom: NeuralGeom::for_viewport(1, 1),
            genesis_t: 0.0,
            lit: false,
            last_phase: GenesisPhase::Signal,
            burst_clock: NEUR_BURST_CLOCK_MEAN,
            burst_age: None,
            rewire_clock: NEUR_REWIRE_CLOCK_MEAN,
            retiring: None,
            last_step: None,
            current_cells: Vec::new(),
            previous_cells: Vec::new(),
            drawn_gen: Vec::new(),
            drawn_gen_counter: 0,
            field_palette_slot: 0,
            node_active: 0,
            pulse_active: 0,
            streamer_active: 0,
            pulse_scan_idx: 0,
            streamer_scan_idx: 0,
            #[cfg(test)]
            absorptions_for_test: 0,
            #[cfg(test)]
            bursts_for_test: 0,
            #[cfg(test)]
            rewires_for_test: 0,
            #[cfg(test)]
            fires_for_test: 0,
        }
    }

    /// The steady-state streamer population for a viewport + the
    /// density dial (the data rate — the dial reads as the sky's
    /// busyness).
    pub(crate) fn streamer_target_base(cols: u16, density: f32) -> usize {
        let n = cols.max(1) as f32 * (0.045 + density.clamp(0.05, 3.0) * 0.070);
        (n.round() as usize).clamp(NEUR_MIN_STREAMER, NEUR_MAX_STREAMER)
    }

    /// The live streamer target: the dial, the genesis's thick
    /// data cloud and the burst surge (a mid-session density
    /// change re-sizes the pool; shrinking drops the inactive
    /// tail first, then the excess actives). Hard-capped so the
    /// surge never floods the dirty-cell budget. `pub(super)`:
    /// the data entry pass (spawn.rs) reads it.
    pub(super) fn streamer_target(&self, cols: u16, density: f32) -> usize {
        let base = Self::streamer_target_base(cols, density);
        let mut mult = 1.0;
        if !self.lit {
            // The genesis's thick cloud: while the machine is
            // unbuilt the rain IS the scene (the DNA soup
            // precedent).
            mult *= NEUR_GENESIS_SIGNAL_MULT;
        }
        if self.burst_age.is_some() {
            // The clump arrives: the data itself thickens.
            mult *= NEUR_BURST_SURGE;
        }
        ((base as f32 * mult).round() as usize).min(NEUR_STREAMER_SURGE_CAP)
    }

    /// Rebuild the pools for a new viewport (or style entry). The
    /// genesis state survives a pure resize (the trained network
    /// keeps thinking — the pools refill to the steady state when
    /// already lit; the DNA resize contract); `begin_genesis`
    /// re-arms the birth.
    pub(crate) fn reset(&mut self, cols: u16, lines: u16) {
        self.geom = NeuralGeom::for_viewport(cols, lines);
        // The neuron pool: sized to the layout, built by captures
        // (the deterministic positions are born here — the
        // machine's bones).
        let total: usize = self.geom.nodes_per_layer.iter().sum();
        self.nodes.clear();
        self.nodes.resize_with(total, Neuron::vacant);
        let mut node_idx = 0;
        // The fanout blocks: contiguous per source node (the
        // O(fanout) fire pass).
        let mut first_syn = 0;
        let n_layers = self.geom.nodes_per_layer.len();
        for (layer, &n_layer) in self.geom.nodes_per_layer.iter().enumerate() {
            let is_output = layer + 1 == n_layers;
            for idx in 0..n_layer {
                let (x, y) = self.geom.node_pos(layer, idx, n_layer, cols, lines);
                let node = &mut self.nodes[node_idx];
                node.layer = layer as u8;
                node.x = x;
                node.y = y;
                node.first_syn = first_syn;
                node.is_output = is_output;
                if !is_output {
                    first_syn += NEUR_FANOUT;
                }
                node_idx += 1;
            }
        }
        self.node_active = 0;
        // The synapse pool: the whole wiring precomputed (paths,
        // weights, bows — deterministic, no RNG in the reset; the
        // bench determinism contract), dormant until the wire
        // phase arms the growth.
        let total_syn = first_syn;
        self.synapses.clear();
        self.synapses
            .resize_with(total_syn, super::network::Synapse::vacant);
        self.wire_layout();
        // The pulses: the fixed signal pool.
        self.pulses.clear();
        self.pulses.resize_with(NEUR_PULSE_POOL, Pulse::vacant);
        self.pulse_active = 0;
        self.pulse_scan_idx = 0;
        // The streamers: the accumulator fills them.
        self.streamers.clear();
        self.streamer_active = 0;
        self.streamer_scan_idx = 0;
        // The clocks re-arm.
        self.burst_clock = NEUR_BURST_CLOCK_MEAN;
        self.burst_age = None;
        self.rewire_clock = NEUR_REWIRE_CLOCK_MEAN;
        self.retiring = None;
        self.last_step = None;
        #[cfg(test)]
        {
            self.absorptions_for_test = 0;
            self.bursts_for_test = 0;
            self.rewires_for_test = 0;
            self.fires_for_test = 0;
        }
        if self.lit {
            // A resize on the trained machine: the steady state
            // rebuilds immediately (no second genesis).
            self.fill_steady();
        }
        self.clear_draw_history();
    }

    /// Precompute the wiring (the deterministic spread topology:
    /// every source node reaches across the whole target layer —
    /// the paths bowed per (src, dst), the weights hashed per
    /// (src, dst), no RNG).
    fn wire_layout(&mut self) {
        let n_layers = self.geom.nodes_per_layer.len();
        // The node index ranges per layer (the flat pool's layer
        // segments).
        let mut layer_starts = Vec::with_capacity(n_layers);
        let mut acc = 0;
        for &n in &self.geom.nodes_per_layer {
            layer_starts.push(acc);
            acc += n;
        }
        let mut syn_idx = 0;
        for layer in 0..n_layers.saturating_sub(1) {
            let n_layer = self.geom.nodes_per_layer[layer];
            let n_next = self.geom.nodes_per_layer[layer + 1];
            let next_start = layer_starts[layer + 1];
            for idx in 0..n_layer {
                for k in 0..NEUR_FANOUT {
                    let src = layer_starts[layer] + idx;
                    let dst_in = NeuralGeom::wire_dst(idx, k, layer, n_next);
                    let dst = next_start + dst_in;
                    let a = (self.nodes[src].x, self.nodes[src].y);
                    let b = (self.nodes[dst].x, self.nodes[dst].y);
                    let path = super::network::wire_path(a, b, NeuralGeom::wire_bow(src, dst));
                    let weight = NeuralGeom::wire_weight(src, dst);
                    // Dormant until the wire phase arms it: the
                    // infinite delay is the "not yet wired"
                    // sentinel (growth_step never starts).
                    self.synapses[syn_idx].activate(
                        src,
                        dst,
                        path,
                        weight,
                        f32::INFINITY,
                        self.field_palette_slot,
                    );
                    self.synapses[syn_idx].grown = 0.0;
                    syn_idx += 1;
                }
            }
        }
    }

    /// Re-arm the genesis (the scene-entry contract: the entry
    /// replays the birth — reset first, then this; the pools the
    /// reset rebuilt wipe back to the dark start).
    pub(crate) fn begin_genesis(&mut self) {
        self.genesis_t = 0.0;
        self.lit = false;
        self.last_phase = GenesisPhase::Signal;
        for n in &mut self.nodes {
            n.active = false;
            n.potential = 0.0;
            n.refract = 0.0;
            n.flash = 0.0;
        }
        self.node_active = 0;
        for s in &mut self.synapses {
            s.grown = 0.0;
            s.grow_delay = f32::INFINITY;
            s.fade = 1.0;
            s.glow = 0.0;
        }
        for p in &mut self.pulses {
            p.active = false;
        }
        self.pulse_active = 0;
        for d in &mut self.streamers {
            d.active = false;
        }
        self.streamer_active = 0;
        self.burst_clock = NEUR_BURST_CLOCK_MEAN;
        self.burst_age = None;
        self.rewire_clock = NEUR_REWIRE_CLOCK_MEAN;
        self.retiring = None;
        self.clear_draw_history();
    }

    /// Fill the machine to the steady state (the bench
    /// fast-forward, and the resize path on an already-lit
    /// network): the genesis completes, every neuron
    /// materializes at its deterministic position, every wire
    /// stands complete. Deterministic — no RNG (the bench
    /// determinism contract).
    pub(crate) fn fast_forward_genesis(&mut self) {
        self.fill_steady();
    }

    fn fill_steady(&mut self) {
        self.genesis_t = genesis_total_secs() + 1.0;
        self.lit = true;
        self.last_phase = GenesisPhase::Steady;
        for n in &mut self.nodes {
            n.active = true;
            n.potential = 0.0;
            n.refract = 0.0;
            n.flash = 0.0;
        }
        self.node_active = self.nodes.len();
        for s in &mut self.synapses {
            s.grown = 1.0;
            s.grow_delay = 0.0;
            s.fade = 1.0;
        }
    }

    /// The machine's population: the neurons, the signals and the
    /// data. The wiring is deliberately NOT counted — it is the
    /// machine's anatomy (present while the style is mounted,
    /// like the quasar's host halo, not activity): the exit
    /// contract reads zero when the machine disarms, and the
    /// neurons rebuild on the next entry.
    pub(crate) fn active_count(&self) -> usize {
        self.node_active + self.pulse_active + self.streamer_active
    }

    /// The genesis phase at the machine's current clock (Steady
    /// once lit — the draw pass's cap queries read this).
    pub(crate) fn phase(&self) -> GenesisPhase {
        if self.lit {
            GenesisPhase::Steady
        } else {
            genesis_phase(self.genesis_t)
        }
    }

    /// The luminosity fraction in [0, 1] (law 0's ramp — the
    /// caps and the brightness factors ride it).
    pub(crate) fn luminosity(&self) -> f32 {
        if self.lit {
            1.0
        } else {
            super::genesis::luminosity(self.genesis_t)
        }
    }

    /// Palette transition completion: every active cell adopts
    /// the new slot (the family contract; the wiring follows the
    /// field slot).
    pub(crate) fn adopt_palette_slot(&mut self, palette_slot: u8) {
        self.field_palette_slot = palette_slot;
        for n in &mut self.nodes {
            if n.active {
                n.palette_slot = palette_slot;
            }
        }
        for s in &mut self.synapses {
            if s.active {
                s.palette_slot = palette_slot;
            }
        }
        for p in &mut self.pulses {
            if p.active {
                p.palette_slot = palette_slot;
            }
        }
        for d in &mut self.streamers {
            if d.active {
                d.palette_slot = palette_slot;
            }
        }
    }

    /// Drop the diff-cleanup history (semantic invalidation /
    /// forced redraw). The next draw pass rebuilds it from an
    /// empty baseline. The machine state itself is simulation
    /// state and is preserved.
    pub(crate) fn clear_draw_history(&mut self) {
        self.current_cells.clear();
        self.previous_cells.clear();
        self.drawn_gen.clear();
        self.drawn_gen_counter = 0;
    }

    /// Motion pass — the machine's physics core.
    ///
    /// 1. The genesis clock advances (the birth rides sim-time —
    ///    the family speed contract; the phase seams run the wire
    ///    sweep's arming, the build completion and the first
    ///    thought; on completion the lit flag flips).
    /// 2. The event clocks: the burst window's age and the
    ///    thought-burst clock (a fire re-arms it and force-fires
    ///    a clump of inputs — the drama event, the feed flare's
    ///    heir), the rewire clock (picks a healthy wire to retire
    ///    — the plasticity economy grows the successor).
    /// 3. The physics: the neurons' leak and refractory, the
    ///    spontaneous input kicks, the wires' fade and glow, the
    ///    growth, the pulses' rides (deliveries charge the
    ///    targets and light the wires), the streamers' falls —
    ///    and every absorption runs the capture economy (the
    ///    machine builds from the rain, or the nearest input
    ///    charges).
    /// 4. The firing pass: every neuron past the threshold and
    ///    out of its refractory window fires (the glyph re-roll
    ///    flag set — the event-gated shimmer), launching pulses
    ///    down its fanout.
    pub(crate) fn advance(&mut self, step: &NeurStep, random: &mut NeuralRandom<'_>) {
        // The degenerate-viewport freeze: a zero-size viewport
        // holds the machine (nothing to think, nothing to draw —
        // the spawn guard's advance-side sibling).
        if step.cols == 0 || step.lines == 0 {
            self.last_step = Some(step.now);
            return;
        }
        if self.nodes.is_empty()
            && self.synapses.is_empty()
            && self.pulses.is_empty()
            && self.streamers.is_empty()
        {
            self.last_step = Some(step.now);
            return;
        }
        let dt_wall = match self.last_step {
            Some(last) => {
                step.now
                    .saturating_duration_since(last)
                    .as_secs_f32()
                    .min(step.max_sim_delta.as_secs_f32())
                    .max(0.0)
                    * step.resume_blend.clamp(0.0, 1.0)
            }
            None => 0.0,
        };
        self.last_step = Some(step.now);
        if dt_wall <= 0.0 {
            return;
        }

        // The family speed contract: one sim clock for the rain,
        // the signals, the clocks and the genesis alike.
        let dt_sim = dt_wall * step.chars_per_sec.max(0.0) * NEUR_SIM_TIME_PER_CPS;

        // 1. The genesis clock and its seams.
        if !self.lit {
            self.genesis_t += dt_sim;
            let phase = self.phase();
            if phase != self.last_phase {
                self.run_phase_seam(phase);
                self.last_phase = phase;
            }
            if self.genesis_t >= genesis_total_secs() {
                self.lit = true;
                self.last_phase = GenesisPhase::Steady;
            }
        }

        // 2. The event clocks.
        if let Some(age) = &mut self.burst_age {
            *age += dt_sim;
            if *age >= NEUR_BURST_WINDOW {
                self.burst_age = None;
            }
        }
        if self.lit {
            // The thought-burst clock (the drama event — only a
            // lit machine thinks in volleys).
            self.burst_clock -= dt_sim;
            if self.burst_clock <= 0.0 {
                self.burst_clock =
                    NEUR_BURST_CLOCK_MEAN * (0.6 + random.rand_chance.sample(random.rng) * 0.8);
                self.burst_age = Some(0.0);
                self.fire_burst(random);
                #[cfg(test)]
                {
                    self.bursts_for_test += 1;
                }
            }
            // The plasticity clock (at most one rewire in flight
            // — the bounded learning).
            self.rewire_clock -= dt_sim;
            if self.rewire_clock <= 0.0 {
                self.rewire_clock =
                    NEUR_REWIRE_CLOCK_MEAN * (0.6 + random.rand_chance.sample(random.rng) * 0.8);
                if self.retiring.is_none() {
                    self.begin_rewire(random);
                    #[cfg(test)]
                    {
                        self.rewires_for_test += 1;
                    }
                }
            }
        }

        // 3. The physics.
        for n in &mut self.nodes {
            if n.active {
                neuron_step(n, dt_sim);
            }
        }
        // The spontaneous input kicks (a lit machine idles alive
        // between meals — the Poisson read, one roll per input).
        if self.lit {
            let input_n = self.geom.nodes_per_layer.first().copied().unwrap_or(0);
            let kick_p = 1.0 - (-NEUR_SPONT_RATE * dt_sim).exp();
            for i in 0..input_n.min(self.nodes.len()) {
                if !self.nodes[i].active {
                    continue;
                }
                if random.rand_chance.sample(random.rng) < kick_p {
                    let n = &mut self.nodes[i];
                    n.potential = (n.potential + NEUR_SPONT_KICK).min(NEUR_POT_CAP);
                }
            }
        }
        let mut retired = false;
        for s in &mut self.synapses {
            if synapse_step(s, dt_sim) {
                retired = true;
            }
            growth_step(s, dt_sim);
        }
        if retired {
            // The fade completed: the successor grows in the same
            // slot (the plasticity economy — constant wire count;
            // it clears the retiring marker itself).
            self.grow_successor(random);
        }
        for i in 0..self.pulses.len() {
            if !self.pulses[i].active {
                continue;
            }
            let syn = self.pulses[i].syn;
            let path_len = self.synapses.get(syn).map(|s| s.path.len()).unwrap_or(0);
            if pulse_step(&mut self.pulses[i], dt_sim, path_len) {
                self.pulse_active = self.pulse_active.saturating_sub(1);
                self.pulses[i].active = false;
                // The delivery: the target charges, the wire
                // lights.
                if let Some(s) = self.synapses.get_mut(syn) {
                    s.glow = 1.0;
                    let dst = s.dst;
                    let w = s.weight;
                    if let Some(n) = self.nodes.get_mut(dst) {
                        if n.active {
                            n.potential = (n.potential + w).min(NEUR_POT_CAP);
                        }
                    }
                }
            }
        }
        for i in 0..self.streamers.len() {
            if !self.streamers[i].active {
                continue;
            }
            if streamer_step(&mut self.streamers[i], dt_sim, self.geom.input_y) {
                self.streamer_active = self.streamer_active.saturating_sub(1);
                let x = self.streamers[i].x;
                self.run_capture(x);
            }
        }

        // 4. The firing pass (the threshold + refractory gate —
        // the fire economy lives in firing.rs).
        for i in 0..self.nodes.len() {
            let fire = {
                let n = &self.nodes[i];
                n.active && n.refract <= 0.0 && n.potential >= NEUR_THRESHOLD
            };
            if fire {
                self.fire_node(i, random);
            }
        }
    }

    // -- Test-only diagnostics (mirrors the family *_for_test API) --

    #[cfg(test)]
    pub(crate) fn node_states_for_test(&self) -> Vec<(f32, f32, f32)> {
        self.nodes
            .iter()
            .filter(|n| n.active)
            .map(|n| (n.potential, n.refract, n.flash))
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn genesis_t_for_test(&self) -> f32 {
        self.genesis_t
    }

    #[cfg(test)]
    pub(crate) fn lit_for_test(&self) -> bool {
        self.lit
    }

    #[cfg(test)]
    pub(crate) fn bursts_for_test(&self) -> usize {
        self.bursts_for_test
    }

    #[cfg(test)]
    pub(crate) fn rewires_for_test(&self) -> usize {
        self.rewires_for_test
    }

    #[cfg(test)]
    pub(crate) fn fires_for_test(&self) -> usize {
        self.fires_for_test
    }

    #[cfg(test)]
    pub(crate) fn absorptions_for_test(&self) -> usize {
        self.absorptions_for_test
    }

    #[cfg(test)]
    pub(crate) fn node_active_for_test(&self) -> usize {
        self.node_active
    }

    #[cfg(test)]
    pub(crate) fn streamer_active_for_test(&self) -> usize {
        self.streamer_active
    }

    #[cfg(test)]
    pub(crate) fn pulse_active_for_test(&self) -> usize {
        self.pulse_active
    }

    /// Plant the burst clock (the burst tests' deterministic
    /// arm).
    #[cfg(test)]
    pub(crate) fn arm_burst_for_test(&mut self, clock: f32) {
        self.burst_clock = clock;
    }

    /// Plant the rewire clock (the plasticity tests' arm).
    #[cfg(test)]
    pub(crate) fn arm_rewire_for_test(&mut self, clock: f32) {
        self.rewire_clock = clock;
    }

    #[cfg(test)]
    pub(crate) fn drawn_cells_for_test(&self) -> &[NeurCell] {
        &self.current_cells
    }
}
