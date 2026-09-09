// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The network's cells and wiring — the physics half of the
//! neural style (NIGHT-research-9, the fourteenth style).
//!
//! This module owns the four cell types (the neuron, the synapse
//! with its precomputed wire path, the traveling pulse, the
//! falling streamer), the viewport layout (the layer anchors and
//! the per-layer node counts), the dendrite path builder, and the
//! per-kind physics steps (laws 1-5 executable form). The full
//! derivation essay lives in `type_rain/neural/mod.rs`; the state
//! machine and the per-frame orchestration live in `neural.rs`.
//!
//! The populations (one `Vec` each on `NeuralRain`, the quasar's
//! per-subsystem precedent — each population spawns, steps and
//! reads independently):
//! - Neurons: static integrate-and-fire cells on the layer
//!   anchors (the potential integrates, leaks, fires into a
//!   refractory window; the layout is born at reset and never
//!   moves — the machine's bones).
//! - Synapses: the wiring — one fanout block per source node,
//!   each wire a precomputed cell path bowed like a dendrite,
//!   grown at genesis or through plasticity, retired the same
//!   way.
//! - Pulses: the signals — glyph carriers riding a wire's path
//!   at their rolled speed, delivering the wire's weight to the
//!   target on arrival.
//! - Streamers: the data — glyph rain falling onto the input
//!   band, absorbed by the nearest input neuron (or, while the
//!   network is unbuilt, consumed as the next neuron's birth).

use crate::constants::{
    NEUR_FALL_MIN, NEUR_FALL_SPAN, NEUR_FLASH_TAU, NEUR_HIDDEN_FLOOR, NEUR_INPUT_MAX,
    NEUR_INPUT_MIN, NEUR_INPUT_Y_FRAC, NEUR_LEAK_TAU, NEUR_OUTPUT_Y_FRAC, NEUR_PULSE_SPREAD,
    NEUR_PULSE_V, NEUR_WEIGHT_MIN, NEUR_WEIGHT_SPAN, NEUR_X_MARGIN_FRAC, NEUR_Y_JITTER,
};

use rand::{distr::Uniform, rngs::StdRng};

/// RNG bundle (the advance pass is a stochastic pass in the family
/// sense: the spont kicks, the burst picks and the rewire targets
/// ride along).
pub(crate) struct NeuralRandom<'a> {
    pub(crate) rng: &'a mut StdRng,
    pub(crate) rand_chance: &'a Uniform<f32>,
}

/// One neuron: a static integrate-and-fire cell. The position is
/// born at reset and never moves (the machine's bones — the
/// motion lives in the signals, not the substrate); the potential
/// integrates kicks, leaks exponentially, and fires at the
/// threshold into a refractory window. Unused fields read zero
/// (the family's flat-struct precedent).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Neuron {
    pub(crate) active: bool,
    /// The layer index (0 = the input band ... L-1 = the output).
    pub(crate) layer: u8,
    /// The node's column (static).
    pub(crate) x: u16,
    /// The node's line (static, carrying the golden-angle jitter).
    pub(crate) y: u16,
    /// The membrane potential in [0, NEUR_POT_CAP].
    pub(crate) potential: f32,
    /// The refractory countdown in sim-seconds (no fire while
    /// positive — a saturated neuron sheds load instead of
    /// locking up).
    pub(crate) refract: f32,
    /// The fired flash in [0, 1] (set to 1 on fire, decays — the
    /// spike's afterglow).
    pub(crate) flash: f32,
    /// The first synapse index of this node's fanout block (the
    /// wiring is contiguous per source node — O(fanout) firing).
    /// The output layer's nodes carry the end sentinel (no
    /// outgoing wires).
    pub(crate) first_syn: usize,
    /// True for the output layer (the answer reads one rung
    /// hotter than the question).
    pub(crate) is_output: bool,
    /// Glyph carried by the cell; re-rolled on fire (event-gated
    /// mutation, the family contract — a neuron that has never
    /// fired keeps its glyph).
    pub(crate) ch: char,
    /// Palette slot adopted at build / palette transition.
    pub(crate) palette_slot: u8,
    /// Sim-seconds since the cell materialized.
    pub(crate) age: f32,
}

impl Neuron {
    pub(crate) const fn vacant() -> Self {
        Self {
            active: false,
            layer: 0,
            x: 0,
            y: 0,
            potential: 0.0,
            refract: 0.0,
            flash: 0.0,
            first_syn: 0,
            is_output: false,
            ch: '\0',
            palette_slot: 0,
            age: 0.0,
        }
    }

    /// Materialize the cell at its static position (the capture
    /// economy's fresh write — the network is BUILT from the
    /// rain; or the genesis phase-fill).
    pub(crate) fn activate(
        &mut self,
        layer: u8,
        x: u16,
        y: u16,
        first_syn: usize,
        is_output: bool,
        palette_slot: u8,
    ) {
        self.active = true;
        self.layer = layer;
        self.x = x;
        self.y = y;
        self.potential = 0.0;
        self.refract = 0.0;
        self.flash = 0.0;
        self.first_syn = first_syn;
        self.is_output = is_output;
        // The unrolled-glyph sentinel: the draw pass picks the
        // cell's glyph at first light (the lorenz contract —
        // never draw the sentinel itself).
        self.ch = '\0';
        self.palette_slot = palette_slot;
        self.age = 0.0;
    }

    /// The fired state (set by the state machine at the fire
    /// decision — the potential resets, the window opens, the
    /// flash lights; the glyph re-roll is the caller's, the
    /// event-gated shimmer).
    pub(crate) fn mark_fired(&mut self, refractory: f32) {
        self.potential = 0.0;
        self.refract = refractory;
        self.flash = 1.0;
    }
}

/// One synapse: a wire from a source node to a target node — the
/// path precomputed at layout (bowed like a dendrite), grown at
/// the genesis wire phase or through plasticity, and retired the
/// same way. The path's endpoints are the neurons' own cells
/// (excluded here — the node pass owns them).
#[derive(Clone, Debug)]
pub(crate) struct Synapse {
    pub(crate) active: bool,
    /// The source node index (the fanout block's owner).
    pub(crate) src: usize,
    /// The destination node index.
    pub(crate) dst: usize,
    /// The wire's cell path (endpoints excluded, one cell per
    /// line between the layers).
    pub(crate) path: Vec<(u16, u16)>,
    /// Growth progress in [0, 1] (the dendrite extends cell by
    /// cell — the genesis sweep and the plasticity reach-out).
    pub(crate) grown: f32,
    /// The growth countdown in sim-seconds (the genesis stagger
    /// arms it; zero grows immediately).
    pub(crate) grow_delay: f32,
    /// The delivery weight in [NEUR_WEIGHT_MIN, +SPAN] (what a
    /// pulse's arrival adds to the target's potential).
    pub(crate) weight: f32,
    /// The retire fade in [0, 1] (1 = healthy; the rewire economy
    /// decays it to zero, then the wire deactivates and a
    /// successor grows).
    pub(crate) fade: f32,
    /// The signal glow in [0, 1] (set to 1 on a delivery, decays
    /// — the light shows where signals have recently passed).
    pub(crate) glow: f32,
    /// The wire's glyph (one per wire — the dotted cells all
    /// carry it; re-rolled when the successor grows).
    pub(crate) ch: char,
    /// Palette slot adopted at wiring / palette transition.
    pub(crate) palette_slot: u8,
    /// Sim-seconds since the wire grew.
    pub(crate) age: f32,
}

impl Synapse {
    pub(crate) fn vacant() -> Self {
        Self {
            active: false,
            src: 0,
            dst: 0,
            path: Vec::new(),
            grown: 0.0,
            grow_delay: 0.0,
            weight: NEUR_WEIGHT_MIN,
            fade: 1.0,
            glow: 0.0,
            ch: '0',
            palette_slot: 0,
            age: 0.0,
        }
    }

    /// Wire (or rewire) from `src` to `dst` over the precomputed
    /// path (the plasticity economy builds the successor here;
    /// the genesis builds the whole machine the same way). The
    /// glyph starts on the unrolled sentinel (the draw pass picks
    /// it at first light).
    pub(crate) fn activate(
        &mut self,
        src: usize,
        dst: usize,
        path: Vec<(u16, u16)>,
        weight: f32,
        grow_delay: f32,
        palette_slot: u8,
    ) {
        self.active = true;
        self.src = src;
        self.dst = dst;
        self.path = path;
        self.grown = 0.0;
        self.grow_delay = grow_delay.max(0.0);
        self.weight = weight;
        self.fade = 1.0;
        self.glow = 0.0;
        self.ch = '\0';
        self.palette_slot = palette_slot;
        self.age = 0.0;
    }

    /// The number of cells the growth has uncovered (the drawn
    /// prefix length — the rest of the path is not yet wire).
    pub(crate) fn grown_cells(&self) -> usize {
        (self.grown.clamp(0.0, 1.0) * self.path.len() as f32).ceil() as usize
    }

    /// Begin the retire fade (the rewire economy's first arm —
    /// the wire dims out over NEUR_REWIRE_FADE_SECS while its
    /// riding pulses finish their trips).
    pub(crate) fn begin_retire(&mut self) {
        self.fade = 0.999;
    }

    /// True while the wire accepts new pulses (fully grown and
    /// not retiring — the fire pass's fanout filter).
    pub(crate) fn conducts(&self) -> bool {
        self.active && self.grown >= 1.0 && self.fade >= 1.0
    }
}

/// One pulse: a glyph signal riding a wire from its source. The
/// rolled speed keeps waves from reading as a grid (the quasar
/// jet energy-share precedent — same-law riders collapse into
/// lockstep; the spread is what makes a wave read as a stream).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Pulse {
    pub(crate) active: bool,
    /// The wire the pulse rides.
    pub(crate) syn: usize,
    /// The position along the path (in cells, from the source
    /// end).
    pub(crate) pos: f32,
    /// The travel speed in cells per sim-second.
    pub(crate) speed: f32,
    pub(crate) ch: char,
    pub(crate) palette_slot: u8,
    pub(crate) age: f32,
    /// The last-drawn head cell (the wake's memory; u16::MAX
    /// means "never drawn").
    pub(crate) trail_col: u16,
    pub(crate) trail_line: u16,
}

impl Pulse {
    pub(crate) const fn vacant() -> Self {
        Self {
            active: false,
            syn: 0,
            pos: 0.0,
            speed: NEUR_PULSE_V,
            ch: '0',
            palette_slot: 0,
            age: 0.0,
            trail_col: u16::MAX,
            trail_line: 0,
        }
    }

    /// Launch onto wire `syn` (the fire pass's spawn — the speed
    /// rolled with the family spread).
    pub(crate) fn launch(&mut self, syn: usize, speed: f32, palette_slot: u8) {
        self.active = true;
        self.syn = syn;
        self.pos = 0.0;
        self.speed = speed.max(0.5);
        self.palette_slot = palette_slot;
        self.age = 0.0;
        self.trail_col = u16::MAX;
    }

    /// The pulse's previous cell, if any (the wake's memory).
    pub(crate) fn trail_cell(&self) -> Option<(u16, u16)> {
        if self.trail_col == u16::MAX {
            None
        } else {
            Some((self.trail_col, self.trail_line))
        }
    }

    /// Push a new head cell into the one-deep trail.
    pub(crate) fn set_trail(&mut self, col: u16, line: u16) {
        self.trail_col = col;
        self.trail_line = line;
    }
}

/// One streamer: the data — a glyph falling onto the input band.
/// The display sways around the base column (each streamer rides
/// its own drift phase); the absorption edge is the input band's
/// line.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Streamer {
    pub(crate) active: bool,
    /// The base column (the display sways around it).
    pub(crate) x: f32,
    /// The falling position (line).
    pub(crate) y: f32,
    /// The fall speed in cells per sim-second (rolled at spawn —
    /// the rain does not fall in lockstep).
    pub(crate) speed: f32,
    /// The sway phase (radians — each streamer its own).
    pub(crate) drift: f32,
    pub(crate) ch: char,
    pub(crate) palette_slot: u8,
    pub(crate) age: f32,
    /// The last-drawn head cell (the shimmer gate's memory;
    /// u16::MAX means "never drawn").
    pub(crate) trail_col: u16,
    pub(crate) trail_line: u16,
}

impl Streamer {
    pub(crate) const fn vacant() -> Self {
        Self {
            active: false,
            x: 0.0,
            y: 0.0,
            speed: NEUR_FALL_MIN,
            drift: 0.0,
            ch: '0',
            palette_slot: 0,
            age: 0.0,
            trail_col: u16::MAX,
            trail_line: 0,
        }
    }

    /// Drop into the sky at column `x` (the accumulator's
    /// activation — the glyph is picked at the first draw pass,
    /// the lorenz contract).
    pub(crate) fn activate(&mut self, x: f32, speed: f32, drift: f32, palette_slot: u8) {
        self.active = true;
        self.x = x;
        self.y = 0.0;
        self.speed = speed;
        self.drift = drift;
        self.palette_slot = palette_slot;
        self.age = 0.0;
        self.trail_col = u16::MAX;
    }

    /// The display column at the streamer's own drift (the gentle
    /// sway — the rain falls, it does not march).
    pub(crate) fn display_x(&self) -> f32 {
        self.x + (self.drift + self.age * 1.7).sin() * crate::constants::NEUR_DRIFT_AMP
    }

    /// The previous head cell, if any (the wake's memory).
    pub(crate) fn trail_cell(&self) -> Option<(u16, u16)> {
        if self.trail_col == u16::MAX {
            None
        } else {
            Some((self.trail_col, self.trail_line))
        }
    }

    /// Push a new head cell into the one-deep trail.
    pub(crate) fn set_trail(&mut self, col: u16, line: u16) {
        self.trail_col = col;
        self.trail_line = line;
    }
}

/// The network layout for one viewport — the layer anchors and
/// the per-layer node counts, every cap keeping the cells inside
/// the frame by construction (the degenerate-terminal contract: a
/// 1x1 viewport produces a collapsed but valid machine, never a
/// panic).
#[derive(Clone, Debug)]
pub(crate) struct NeuralGeom {
    /// The layer line anchors (fractional, one per layer).
    pub(crate) layer_ys: Vec<f32>,
    /// The nodes per layer (same length as `layer_ys`).
    pub(crate) nodes_per_layer: Vec<usize>,
    /// The input band's line (the streamers' absorption edge).
    pub(crate) input_y: f32,
}

/// The per-layer population taper (the input count is derived
/// from the viewport directly; hidden layers shrink on the
/// ratios, floored at NEUR_HIDDEN_FLOOR).
const LAYER_RATIOS: [f32; 4] = [1.0, 0.72, 0.50, 0.34];

impl NeuralGeom {
    /// Derive the layout for a viewport (deterministic — no RNG,
    /// the bench determinism contract).
    pub(crate) fn for_viewport(cols: u16, lines: u16) -> Self {
        let w = cols.max(1) as f32;
        let h = lines.max(1) as f32;
        // Four layers on tall frames, three on short ones (the
        // layer band needs room for the wires to read).
        let n_layers = if lines >= 26 { 4 } else { 3 };
        let first = h * NEUR_INPUT_Y_FRAC;
        let last = h * NEUR_OUTPUT_Y_FRAC;
        let layer_ys = (0..n_layers)
            .map(|l| {
                if n_layers == 1 {
                    first
                } else {
                    first + (l as f32 / (n_layers - 1) as f32) * (last - first)
                }
            })
            .collect();
        // The input band: the widest layer, viewport-derived and
        // hard-capped (the data needs bandwidth).
        let input_n = ((w * 0.11).round() as usize).clamp(NEUR_INPUT_MIN, NEUR_INPUT_MAX);
        let nodes_per_layer = (0..n_layers)
            .map(|l| {
                let ratio = LAYER_RATIOS[l.min(LAYER_RATIOS.len() - 1)];
                ((input_n as f32 * ratio).round() as usize).max(NEUR_HIDDEN_FLOOR)
            })
            .collect();
        Self {
            layer_ys,
            nodes_per_layer,
            input_y: first,
        }
    }

    /// A node's static position on layer `layer`, index `idx`
    /// within the layer (the golden-angle y jitter keeps the
    /// lattice from reading as a machine grid — the organic
    /// cortex read; deterministic, no RNG).
    pub(crate) fn node_pos(
        &self,
        layer: usize,
        idx: usize,
        n_layer: usize,
        cols: u16,
        lines: u16,
    ) -> (u16, u16) {
        let w = cols.max(1) as f32;
        let margin = w * NEUR_X_MARGIN_FRAC;
        let usable = (w - 2.0 * margin).max(0.0);
        let x = if n_layer <= 1 {
            w * 0.5
        } else {
            margin + (idx as f32 / (n_layer - 1) as f32) * usable
        };
        let y_anchor = self.layer_ys.get(layer).copied().unwrap_or(self.input_y);
        let jitter = ((idx as f32) * GOLDEN_ANGLE + (layer as f32) * 0.7).sin() * NEUR_Y_JITTER;
        let y = y_anchor + jitter;
        (
            (x.round().max(0.0) as u16).min(cols.saturating_sub(1)),
            (y.round().max(0.0) as u16).min(lines.saturating_sub(1)),
        )
    }

    /// The destination node (within the next layer) for source
    /// index `idx`, fanout slot `k` — the deterministic spread
    /// wiring (every source reaches across the whole target
    /// layer; no RNG — the bench determinism contract).
    pub(crate) fn wire_dst(idx: usize, k: usize, layer: usize, n_next: usize) -> usize {
        if n_next == 0 {
            return 0;
        }
        (idx * 7 + k * 3 + layer) % n_next
    }

    /// A wire's weight (deterministic per (src, dst) — the
    /// connection strengths vary without RNG).
    pub(crate) fn wire_weight(src: usize, dst: usize) -> f32 {
        let hash = (src * 13 + dst * 29) % 8;
        NEUR_WEIGHT_MIN + (hash as f32 / 7.0) * NEUR_WEIGHT_SPAN
    }

    /// A wire's bow (deterministic per (src, dst) — each dendrite
    /// bends its own way, in [-2, 2] cells).
    pub(crate) fn wire_bow(src: usize, dst: usize) -> f32 {
        ((src * 31 + dst * 17) % 5) as f32 - 2.0
    }
}

/// The golden angle (radians) — the deterministic spread constant.
pub(crate) const GOLDEN_ANGLE: f32 = 2.399_963_2;

/// Build a wire's cell path from node cell `a` down to node cell
/// `b` (endpoints excluded): one cell per line, x eased along the
/// chord with a half-sine bow — the dendrite's bend. Degenerate
/// spans (adjacent or inverted layers) yield an empty path (the
/// pulse delivers at once; nothing to walk).
pub(crate) fn wire_path(a: (u16, u16), b: (u16, u16), bow: f32) -> Vec<(u16, u16)> {
    let dy = b.1 as i32 - a.1 as i32;
    if dy <= 1 {
        return Vec::new();
    }
    let mut path = Vec::with_capacity((dy - 1) as usize);
    let ax = a.0 as f32;
    let span = (b.0 as i32 - a.0 as i32) as f32;
    for step in 1..dy {
        let t = step as f32 / dy as f32;
        let x = ax + span * t + bow * (std::f32::consts::PI * t).sin();
        path.push((x.round().max(0.0) as u16, (a.1 as i32 + step) as u16));
    }
    path
}

/// The pulse launch speed (rolled with the family spread —
/// NEUR_PULSE_V scaled by [1 - SPREAD, 1 + SPREAD]).
pub(crate) fn pulse_speed(roll: f32) -> f32 {
    NEUR_PULSE_V * (1.0 + (roll * 2.0 - 1.0) * NEUR_PULSE_SPREAD)
}

/// The streamer fall speed (rolled at spawn — the rain does not
/// fall in lockstep).
pub(crate) fn fall_speed(roll: f32) -> f32 {
    NEUR_FALL_MIN + roll * NEUR_FALL_SPAN
}

/// Law 1's neuron step: the membrane leaks (exponential
/// forgetting toward zero), the refractory window counts down,
/// the fired flash decays. The potential never leaves
/// [0, NEUR_POT_CAP] (the callers' kicks clamp; the leak only
/// shrinks).
pub(crate) fn neuron_step(p: &mut Neuron, dt: f32) {
    p.potential *= (-dt / NEUR_LEAK_TAU).exp();
    p.refract = (p.refract - dt).max(0.0);
    p.flash *= (-dt / NEUR_FLASH_TAU).exp();
    p.age += dt;
}

/// Law 5's synapse step: the retire fade decays (the rewire
/// economy's dim-out — returns true when it completes, the caller
/// grows the successor), and the signal glow decays (the light
/// shows where signals have RECENTLY passed).
pub(crate) fn synapse_step(p: &mut Synapse, dt: f32) -> bool {
    if p.fade < 1.0 {
        p.fade -= dt / crate::constants::NEUR_REWIRE_FADE_SECS;
        if p.fade <= 0.0 {
            p.fade = 0.0;
            p.active = false;
            return true;
        }
    }
    p.glow *= (-dt / crate::constants::NEUR_GLOW_TAU).exp();
    p.age += dt;
    false
}

/// The growth step: while armed, the delay counts down, then the
/// wire extends (grown 0 -> 1 over NEUR_GROW_SECS — the dendrite
/// reach-out, cell by cell). Bounded by construction.
pub(crate) fn growth_step(p: &mut Synapse, dt: f32) {
    if p.grown >= 1.0 {
        return;
    }
    if p.grow_delay > 0.0 {
        p.grow_delay -= dt;
        return;
    }
    p.grown = (p.grown + dt / crate::constants::NEUR_GROW_SECS).min(1.0);
}

/// Law 3's streamer step: the fall (constant speed, the sway is
/// a draw-side projection). Returns true when the streamer
/// reaches the input band (the caller runs the capture economy;
/// the particle deactivates).
pub(crate) fn streamer_step(p: &mut Streamer, dt: f32, input_y: f32) -> bool {
    p.y += p.speed * dt;
    p.age += dt;
    if p.y >= input_y {
        p.active = false;
        return true;
    }
    false
}

/// Law 4's pulse step: the ride. Returns true on arrival (the
/// caller delivers the wire's weight to the target and sets the
/// wire's glow; the particle deactivates).
pub(crate) fn pulse_step(p: &mut Pulse, dt: f32, path_len: usize) -> bool {
    p.pos += p.speed * dt;
    p.age += dt;
    p.pos >= path_len as f32
}
