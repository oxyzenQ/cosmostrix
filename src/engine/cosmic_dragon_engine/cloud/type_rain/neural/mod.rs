// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

#![allow(clippy::module_inception)]

//! Neural network rain (NIGHT-research-9, the fourteenth style):
//! the rain trains the network.
//!
//! The scene reads as a mind assembling itself under a data
//! stream: glyph signals fall out of the sky onto an input band
//! of neurons, charge them, and fire pulses down dendritic wires
//! that bow from layer to layer — the pulse arrives, the target
//! charges, and when enough signals land inside the leak window
//! the target fires in turn. The matrix rain is reborn as data:
//! every falling glyph is a training sample, and the machine
//! below eats it, thinks in waves, and slowly rewrites its own
//! wiring. The domain set the style registry could not cover
//! until now — nature (physarum, murmuration), life (the DNA
//! helix), the cosmos (the black hole, the quasar) — gains its
//! fourth kingdom: the machine mind.
//!
//! ## Origin
//!
//! NIGHT-research-9 (the owner's pick, the neural-network
//! proposal finally seated after the quasar round chose the
//! engine first): the artificial neural network is a canonical
//! object — McCulloch-Pitts 1943 gave the threshold neuron,
//! Rosenblatt 1958 the perceptron, and the integrate-and-fire
//! compartmental model carries a century of electrophysiology
//! (the leak, the refractory window, the all-or-nothing spike).
//! Like the DNA helix, the murmuration and the quasar, this
//! style borrows a canonical reference; what IS derived here is
//! the terminal mapping: the network laid out as horizontal
//! layers on the cell grid with the signal flowing DOWNWARD (the
//! rain's own direction — the data literally streams through the
//! machine), the integrate-and-fire dynamics collapsed to a
//! leak-decay potential with a threshold and a refractory gate,
//! the synapses as precomputed bowed cell paths (deterministic
//! dendrites, drawn dotted so the idle wiring reads as a faint
//! loom), the pulses as glyph carriers riding the paths with a
//! rolled speed spread, and the learning economy that turns the
//! rain itself into the machine's birth and its food.
//!
//! ## The engine equations (the five laws)
//!
//! The engine is one static architecture and three moving
//! populations through it — every moving cell a glyph carrier
//! bound to the machine by construction, none of them free.
//!
//! ### Law 0 — the genesis (the birth)
//!
//! The style does not enter with the machine already thinking:
//! the entry replays the training run in four continuous phases
//! (the DNA genesis contract — the ignition precedent's heir).
//! The data falls first (the thick signal cloud, nothing built —
//! while the machine is unbuilt the rain IS the scene), then the
//! layers materialize as the captures land (a streamer births
//! the next neuron, the input band first), then the dendrites
//! reach out (the wires grow on a staggered sweep, layer-pair by
//! layer-pair — the assembly's most beautiful second), then the
//! wiring completes and the first thought fires: a cascade
//! crosses the machine while the luminosity ramps to the full
//! law. A pure resize keeps the trained network; only a scene
//! entry re-arms the birth. The clock rides sim-time (the family
//! speed contract scales the birth with the engine).
//!
//! ### Law 1 — the layers (the architecture)
//!
//! The machine's bones: three or four horizontal layers (the
//! viewport's height decides), the input band widest under the
//! sky, tapering through the hidden layers to the sparse output
//! band near the floor — the flow reads downward, the rain's own
//! direction. The positions are deterministic (the golden-angle
//! y-jitter keeps the lattice from reading as a machine grid)
//! and static after birth: the motion lives in the signals, not
//! the substrate. The output band renders one rung hotter — the
//! answer reads brighter than the question.
//!
//! ### Law 2 — the neurons (the integrate-and-fire)
//!
//! Each cell integrates: kicks raise a bounded potential, the
//! membrane leaks exponentially (the forgetting — a cell that
//! never reaches the threshold inside the leak window forgets
//! the meal), and at the threshold the cell FIRES — the
//! potential resets, a refractory window opens (a saturated cell
//! sheds load instead of locking up, exactly like the real
//! substrate), a flash decays (the spike's afterglow), and the
//! glyph re-rolls (event-gated mutation, the family contract).
//! Between meals the input band idles alive on spontaneous kicks
//! (the Poisson read) so the machine never fully sleeps.
//!
//! ### Law 3 — the rain is the data (the capture economy)
//!
//! The streamers: glyph signals falling onto the input band
//! (their columns biased toward the built input neurons — the
//! data aims at the machine; the rest is noise the machine has
//! not learned yet). A streamer that lands is absorbed: while
//! the machine is below its layout population the capture BIRTHS
//! the next neuron (flashed once — the fresh-write economy, the
//! DNA rung charge's heir); at population the capture kicks the
//! nearest active input neuron's potential. This is the matrix
//! rain's third life: the falling glyph is no longer the scene,
//! it is the scene's training data — the machine is literally
//! built from it and kept thinking by it.
//!
//! ### Law 4 — the signals (the pulses)
//!
//! A fire launches pulses down the cell's fanout (the wires that
//! conduct — grown and not retiring); each pulse rides its
//! wire's precomputed path at a rolled speed (the spread that
//! keeps a wave from reading as a grid — the quasar jet
//! energy-share's heir), and on arrival delivers the wire's
//! weight to the target and lights the wire's glow (the light
//! shows where signals have recently passed). One signal is
//! usually sub-threshold (a single meal is not a thought —
//! realistic sparseness); the thought-burst clock is the drama
//! event: a clump of inputs force-fires together and the wave
//! crosses the whole machine, the output flaring as it lands
//! (the feed-flare's heir, the money shot). The burst also
//! thickens the rain — the data itself surges while the machine
//! thinks.
//!
//! ### Law 5 — the plasticity (the rewiring)
//!
//! The learning: the rewire clock retires one healthy wire at a
//! time (the fade — a slow dim-out while its riding pulses
//! finish their trips) and grows the successor in the SAME slot
//! (the wire count is constant by construction — nothing
//! accumulates), the same source reaching to a NEW target, the
//! path rebuilt and re-grown cell by cell (the dendrite
//! reach-out replayed in miniature). The machine's topology
//! rewrites itself forever — the rain's long memory: the network
//! is never frozen, and never floods.
//!
//! ## Stability
//!
//! Every state variable is bounded by direct construction: the
//! neuron pool is layout-derived (viewport-capped, floored); the
//! potential clamps at NEUR_POT_CAP; the refractory counts to
//! zero; the flash, glow, grown and fade live in [0, 1] by
//! construction (the fade only decays, the growth only
//! accumulates, both clamped); the pulse pool is fixed with a
//! rotating free-slot scan (a saturated volley sheds load —
//! bounded by the pool, never by demand); the streamer pool is
//! viewport-derived with a hard surge cap; the wire count is
//! constant (the successor reuses the retired slot); the
//! positions are static and clamped inside the frame at reset
//! (the degenerate-terminal contract: a 1x1 viewport produces a
//! collapsed but valid machine, never a panic).
//!
//! ## FPS invariance
//!
//! Every rate (the fall speeds, the pulse speeds, the leak, the
//! refractory, the flash and glow decays, the growth, the burst
//! and rewire clocks, the spontaneous kick rate, the genesis
//! timeline) is expressed per SIM-second and integrated with the
//! clamped dt on the single family clock (dt_sim = dt_wall x
//! chars_per_sec x NEUR_SIM_TIME_PER_CPS, eased by resume_blend).
//! The physics is first-order drift per population (y += v dt,
//! pos += v dt, decay *= exp(-dt/tau)) — stable at any dt the
//! cap admits, and the bench's uniform stepping exercises one
//! advance + one draw per frame exactly as the interactive loop
//! does.
//!
//! ## Module map
//!
//! - `network.rs` — the four cell types (the neuron, the synapse
//!   and its path, the pulse, the streamer), the layout queries
//!   (the layer anchors, the node counts, the wiring spread),
//!   and the per-kind physics steps (laws 1-5 executable form).
//! - `genesis.rs` — the birth sequence's pure phase math (law 0).
//! - `neural.rs` — the machine: the four pools, the advance pass
//!   (the clocks, the seams, the physics, the firing), the test
//!   diagnostics.
//! - `spawn.rs` — the data entry pass (the staggered streamer
//!   spawn through the family's accumulator contract — law 3's
//!   accumulator half; split from `neural.rs` for the 800-line
//!   source cap).
//! - `firing.rs` — the machine's drama: the genesis seams'
//!   choreography, the neuron fire and the thought burst (split
//!   from `neural.rs` for the 800-line source cap).
//! - `plasticity.rs` — the learning economy: the capture
//!   handler (law 3's accounting) and the rewire pass (law 5's
//!   retire-and-regrow).
//! - `draw.rs` — the draw pass (wires, pulses, neurons,
//!   streamers; the potential and glow ladders; drawn-cell diff
//!   cleanup).
//! - `mod.rs` — this derivation.
//!
//! The draw pass stays with the orchestration family split
//! convention: `network.rs` carries the physics, `neural.rs`
//! the state, `draw.rs` the render contract.

pub(crate) mod draw;
pub(crate) mod firing;
pub(crate) mod genesis;
pub(crate) mod network;
pub(crate) mod neural;
pub(crate) mod plasticity;
pub(crate) mod spawn;

pub(crate) use network::NeuralRandom;
pub(crate) use neural::{NeurSpawnParams, NeurStep, NeuralRain};
