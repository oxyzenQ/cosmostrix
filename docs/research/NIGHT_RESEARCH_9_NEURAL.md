<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-research-9: the neural network — the fourteenth rain style

The owner's question (the quasar rating round): a neural network
as the next rain style, or another masterpiece option? The
registry answered for itself: three cosmic styles (the black
hole, the solar flare, the quasar), one biology style (the DNA
helix), two emergent-nature styles (the physarum, the
murmuration) — and no machine-mind. The neural network was the
quasar round's runner-up, recorded in the code's own comments
("the owner's pick over the neural-network proposal") as a debt
waiting to be paid. This round pays it: the collection completes
its four kingdoms — nature, life, cosmos, mind. The rain trains the network: the falling
glyph is no longer the scene, it is the scene's training data.

## The research

The artificial neural network is the canonical object: the
threshold neuron (McCulloch-Pitts 1943), the perceptron
(Rosenblatt 1958), and the integrate-and-fire compartmental
model carrying a century of electrophysiology — the leak, the
refractory window, the all-or-nothing spike. The anatomy the
style borrows:

- The architecture: layered neurons, the input band widest,
  tapering to a sparse output — signals flow one way, layer to
  layer.
- The neuron: a membrane potential integrating kicks, leaking
  exponentially (forgetting), firing at a threshold into a
  refractory window — a saturated cell sheds load instead of
  locking up.
- The signal: a spike traveling the axon, its arrival raising
  the target's potential by the connection's weight — one
  signal is sub-threshold; thoughts are sums.
- The learning: plasticity — connections weaken and die, new
  ones grow; the topology rewrites itself from the data.

What is derived here (the terminal mapping, the style's original
math): the network laid out as horizontal layers with the signal
flowing DOWNWARD — the rain's own direction, so the data
literally streams through the machine; the synapses as
precomputed bowed cell paths (deterministic dendrites, drawn
dotted so the idle wiring reads as a faint loom — the wire
budget is constant by design, events raise the rung, never the
cell count); the pulses as glyph carriers with a rolled speed
spread (the quasar jet energy-share's heir — same-law riders
collapse into lockstep); and the two economies that turn the
rain into the machine: the capture (the network is BUILT from
the streamers) and the plasticity (the topology learns forever
at constant wire count).

## The five laws of the engine

- Law 0, the genesis: the entry replays the training run — the
  data falls (nothing built), the layers materialize from the
  captures, the dendrites reach out in a staggered sweep, the
  wiring completes and the first thought fires. A pure resize
  keeps the trained network; the bench fast-forwards.
- Law 1, the layers: static deterministic positions (the
  golden-angle y-jitter keeps the lattice organic), the output
  band one rung hotter — the answer reads brighter than the
  question.
- Law 2, the neurons: integrate, leak, fire, refract — the
  fired flash decays, the glyph re-rolls on fire (event-gated
  mutation), and the input band idles alive on spontaneous
  Poisson kicks between meals.
- Law 3, the rain is the data: streamers fall onto the input
  band (columns biased toward the built inputs — the data aims
  at the machine); a landing streamer either births the next
  neuron (the fresh-write economy) or kicks the nearest input.
- Law 4, the signals: a fire launches pulses down the fanout at
  rolled speeds; a delivery charges the target and lights the
  wire's glow. The thought-burst clock is the drama event: a
  clump of inputs force-fires, the wave crosses the machine,
  the output flares, the rain surges.
- Law 5, the plasticity: the rewire clock retires one healthy
  wire at a time (a slow fade — its riding pulses land first)
  and grows the successor in the SAME slot to a NEW target: the
  wire count is constant, the machine never floods, never
  freezes.

## The scene

`cosmostrix --scene neural` — cycle position 15, grouped with
the style flagships. Cyan palette (the electric-signal read of a
live circuit), binary charset (bits, not letters — the data
reads as data), 60 fps, speed 16 (the measured signal cadence,
a touch under the family reference), density 0.55 (the sky's
data rate), glitch none (the machine is precise; the burst is
the drama).

## Implementation map

- `src/engine/cosmic_dragon_engine/cloud/type_rain/neural/` —
  the style module, split per the family convention:
  `mod.rs` (the derivation), `network.rs` (the cells, the
  layout, the path builder, the per-kind steps), `genesis.rs`
  (the pure phase math), `neural.rs` (the state machine),
  `spawn.rs` (the staggered data entry), `firing.rs` (the
  seams, the fire, the burst), `plasticity.rs` (the capture and
  rewire economies), `draw.rs` (the render contract).
- `src/types/rain_style.rs` — the `Neural` variant, the label
  surface (`neural` / `neural_network` / `neuralnet` / `nn`).
- `src/central_control_rains/style_rain.rs` — the NEUR_*
  calibration section with its compile-time contracts.
- `src/scene/catalog.rs` + `src/scene/mod.rs` — the scene entry
  and the cycle position (31 built-in scenes; the count
  detectors moved 30 -> 31 as designed).
- Tests: `test/.../tests_neural/` — the genesis timeline, the
  capture economy and bookkeeping, the firing and pulse rides,
  the burst volley, the plasticity's constant wire count, the
  boundedness integration, the drawn bounds, the degenerate
  viewport, the resize and speed contracts.

## Verification record

- Local gates (this session, per the owner's light-verification
  mandate): cargo fmt --check clean; cargo clippy --all-targets
  --all-features -D warnings clean; check-rs-loc.sh OK (all
  eight module files under the 800 cap — neural.rs itself split
  into spawn.rs + firing.rs to honor it); check-comment-style
  clean; check-headers.sh OK (732 files). The test tree
  compiles under --all-targets.
- CI (per the owner's mandate: the suite runs there, not here):
  the full cargo test round owns the neural contracts; the
  10 s A/B bench (benchmark/bench-labs/) and the PTY shape
  smoke (scripts/neural_smoke.py, mirroring quasar_smoke.py)
  run in the release pipeline — the expected shape: the signal
  phase leaves the machine's band empty (only data), the
  steady machine draws its wiring + neurons + pulses + data.

## Design notes (the honest trade-offs)

- The wire budget is deliberately constant: the idle wire draws
  every third path cell (the dashed-line read) and the events
  raise the brightness RUNG, never the cell count — the burst
  lights the whole wiring one step brighter instead of painting
  more cells, so the dirty-cell budget survives the drama by
  construction.
- Single deliveries are sub-threshold by calibration
  (NEUR_WEIGHT_MIN + SPAN < THRESHOLD): one meal is not a
  thought. The idle machine shows sparse input fires; only the
  burst clock produces whole waves. This is the sparseness real
  networks have — and it keeps the pulse pool honest.
- The force-fires (the Thought seam's first cascade, the burst
  clock's volley) are direct fire_node calls, never
  potential-priming. The physics pass leaks every potential
  before the firing gate reads it, so a primed node sat just
  under the threshold and only fired when a capture or spont
  kick happened to land on it in the window — a rescue dice
  roll that platform libm ulp differences (the shared RNG
  stream shift) lost on the MSRV CI runner: the armed burst
  produced zero fires and the neur_burst_clock_fires_volleys
  contract went red. Direct calls make the volley and the
  money-shot cascade deterministic by construction.
- The synapse pool is precomputed at reset (deterministic
  spread wiring, no RNG — the bench determinism contract) and
  the plasticity rewrites it in place: the rewire is a topology
  edit, not a pool edit. The cost: a degenerate viewport's
  wires may collapse to zero-length paths (the pulse delivers
  at once) — valid, never a panic.
- The wiring (like the quasar's halo) is excluded from
  `active_count`: it is the machine's anatomy, not activity.
  The exit contract reads zero when the machine disarms.
- The genesis's Wire phase completes the build at its seam
  (deterministic fill) rather than trusting the capture rate:
  a sparse data cloud cannot strand a half-built machine at
  the moment the wiring needs the full layer set.
- No benchmark was run locally this round (the owner's
  light-verification mandate — the release-build bench belongs
  to CI): the perf profile is bounded by construction (static
  positions, precomputed paths, fixed pools) and pinned by the
  boundedness contracts; the A/B evidence lands with CI's run.
<!-- COSMOSTRIX-DISCLAIMER -->
<!--
  Documentation Disclaimer — read before relying on any data point.

  This document may contain stale data, hardcoded counts, or outdated
  file paths and symbol names. Maintainers update source code but may
  forget to sync every doc — the project ships 80+ .md files and
  perfect sync is a known maintenance burden with diminishing returns.

  Source code (`src/**/*.rs`) is the single source of truth.
  Always cross-check against the actual `.rs` files before relying on
  any specific number (test count, LOC, FPS, ms timeout), file path,
  function name, or config key.

  If you find a discrepancy, please open a PR — the doc is wrong, not
  the source.
-->
