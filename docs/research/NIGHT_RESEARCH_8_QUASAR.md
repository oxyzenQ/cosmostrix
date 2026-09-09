<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-research-8: the quasar — the thirteenth rain style

The owner's question (the DNA genesis round): a quasar effect as
the next rain style, or a neural network? The answer shipped as
the quasar — the pick with the canonical physics, the
photographic reference every telescope owner has already seen
(M87, 2019), and the perfect narrative slot: the black hole rain
style (NIGHT-special-1) is the SAME engine silent; the quasar is
it running at full power. The neural network stays on the
style shortlist for a future round — its motion DNA is a graph,
not a field, and the graph layouts that read well (layered,
spring-embedded) fight the terminal's grid; the quasar's
central-force fields ARE the grid's geometry. The rain feeds the engine: the falling glyph
is no longer the scene, it is the scene's food.

## The research

The quasar is the canonical object: a supermassive black hole
eating ferociously — the most luminous persistent thing in the
universe (Schmidt's 1963 redshift; Shakura-Sunyaev disk
thermodynamics; Blandford-Znajek jets; the Event Horizon
Telescope photographs, M87 2019 and Sgr A* 2022). The anatomy the
style borrows:

- The engine: a core that outshines its host galaxy, breathing on
  accretion-rate flicker.
- The disk: Keplerian orbits (omega ~ r^-1.5 — the inner edge
  laps the outer ~6x, the shear IS the rotation read), a tilted
  thin ellipse in projection, hotter inside (the radial
  temperature ladder), doppler-beamed on the approaching limb
  (relativistic beaming — the M87 photograph's one-side-brighter
  signature).
- The jets: collimated polar outflows, precessing on a slow cone,
  knotted where shocks travel the beam.
- The fuel: gas falling in from the host — the accretion IS the
  rain, and where it lands the disk carries the light of having
  been recently fed.

What is derived here (the terminal mapping, the style's original
math): the doppler beaming collapsed to a brightness-rung
asymmetry (mono-safe — the limb shift survives colorless
terminals), the jets as recycling particle streams whose
per-particle energy share keeps the beam from riding in lockstep
(a same-law stream is a blob, not a beam), the capture economy
that builds the disk from the rain (the DNA rung-charge economy's
heir), and the ignition birth sequence (the owner's DNA-genesis
mandate: a style must be born, not popped in).

## The five laws of the engine

Derived in `src/engine/cosmic_dragon_engine/cloud/type_rain/quasar/mod.rs`
(the full essay); the constants live in
`src/central_control_rains/style_rain.rs` (the QUAS block, with
compile-time contracts).

- Law 0 — the ignition: the entry replays the birth in four
  continuous phases — the dark cloud (the thick cold infall,
  nothing lit), the disk condensing (captures circularize, capped
  at Dim), first light (the core ignites, the luminosity and the
  doppler asymmetry ramp), the jets pushing out (the front
  travels to the full beam, no seam at the handoff). A pure
  resize keeps the burning engine; the bench fast-forwards (Z-6).
- Law 1 — the engine: the core cell breathes on a bounded sine,
  wrapped in a pulse-following glow ring; on the feed-flare clock
  a clump arrives — the pulse locks above its peak, the infall
  surges, a knot climbs each beam, and the core's glyph re-rolls
  (event-gated mutation).
- Law 2 — the disk: Kepler's third law on the orbit fraction
  (omega = K / f^1.5, the inner lapping the outer ~6x), the
  tilted-ellipse projection, the radial temperature ladder, and
  the doppler rung swing — the approaching limb steps up a rung
  and brightens its factor, the receding limb dims.
- Law 3 — the fuel: the infall spirals inward on the accelerating
  plunge (the free-fall 1/f^2 read), and a streamer absorbed at
  the disk's rim either births an orbit (below target, charged to
  full) or re-charges the nearest-angle orbit (at target). The
  charge decays exponentially — the light shows where the engine
  has been recently fed. The bookkeeping is pinned by tests: the
  starvation lesson from the DNA helix (an absorbed particle must
  free its budget) is a contract, not a hope.
- Law 4 — the jets: per beam a recycling stream riding s in
  [0, 1] on the accelerating law v(s) = V0(1 + ACC s), each
  particle carrying a rolled energy share (the spread that makes
  the beam read as a stream — same-law particles collapse into a
  transiting blob), the whole beam precessing on a slow cone and
  riding a helix; the flare's knot travels the beam as a
  brightness pulse.
- Law 5 — the glow: the halo, a sparse annulus of dim orbits
  rounder than the disk's projection, gliding in from beyond the
  frame at entry (no pop), breathing a rung brighter when the
  core flares.

## The scene

`cosmostrix --scene quasar` — the `stars` palette (the
deep-space blue-black-to-white ramp; the telescope-frame read),
the `braille` charset (the plasma reads as granulated light, not
letters), speed 18 (the family reference), density 0.60 (the
hero dial — the disk's ring count, the engine IS the scene),
fps 60, glitch none (the sky is still; the flare is the drama).
Scene cycle position 14, grouped with the style flagships.

## Implementation map

- `type_rain/quasar/mod.rs` — the derivation essay (the laws).
- `type_rain/quasar/quasar.rs` — the engine state machine: the
  four pools, the staggered infall spawn (the accumulator
  contract), the advance pass (the clocks, the physics, the jet
  fire, the fast-forward), the test diagnostics.
- `type_rain/quasar/particles.rs` — the particle, the viewport
  geometry (every cap keeps projections inside the frame), the
  per-kind physics steps.
- `type_rain/quasar/capture.rs` — the capture economy: the
  free-slot scans and the absorption handler.
- `type_rain/quasar/ignition.rs` — the birth sequence's pure
  phase math (law 0).
- `type_rain/quasar/draw.rs` — the painter and the passes (halo,
  disk, jets, infall, core; the doppler and charge ladders; the
  generation-tagged diff cleanup).
- Wiring: the sixteen integration surfaces every structured style
  owns (rain_at's six dispatch arms, scene entry/exit with the
  ignition re-arm, the resize contract, the palette/charset/
  shading invalidations, the bench fast-forward, the scene
  catalog, the labels, the HUD).
- Tests: `tests_quasar/` (25 contracts — the ignition timeline
  and no-seam handoffs, the assembly, the capture economy and its
  starvation-free bookkeeping, the Kepler shear monotonicity, the
  doppler asymmetry, the jet ride, the flare cycle, the drawn
  bounds, the diff cleanup, pause, transitions, resize on the
  burning engine, speed scaling, sustained boundedness, the
  degenerate terminal).
- `scripts/quasar_smoke.py` — the PTY shape smoke (the dark
  cloud leaves the core's home empty; the steady engine draws
  the core + beams there).

## Verification record

- Full suite: 2655 passed / 0 failed (was 2630 — the 25 quasar
  contracts; the scene-count change-detectors updated 29 -> 30
  as designed).
- cargo fmt clean; cargo clippy --all-targets -D warnings clean.
- PTY shape smoke: dark core-box 0 cells / steady core-box 17
  cells, beam rows 23 (both beams firing through the band).
- 10 s A/B bench (benchmark/bench-labs/night_research8_quasar/):
  the four probes (cinematic, aeolian, solar_flare, murmuration)
  hold their dirty populations, entropy and gini to the third
  decimal with mixed-sign fps deltas inside the session's
  interleaved noise band; the quasar itself measures 96,350 fps,
  84.7 dirty cells/frame, entropy 4.788, gini 0.711.

## Design notes (the honest trade-offs)

- The quasar reuses the `stars` palette (north-stars' honor-sky
  ramp) rather than shipping a new one: the deep-space read is
  the same ramp, and a quasar-specific palette would be a
  near-duplicate. The compositions read completely differently
  (a sparse honor sky vs a central engine).
- The halo is excluded from `active_count` by design: it is the
  host's ambient backdrop (present while the style is mounted,
  like the sky itself), not machinery — the exit contract reads
  zero when the engine disarms.
- The mid-ignition resize restarts the disk's assembly (the
  pools rebuild vacant; the clock survives). The contract that
  matters — a resize on the BURNING engine keeps the steady
  state — is pinned by test; the mid-birth resize is a rare
  window with an honest recovery (the captures refill it).
- The one-vec-per-population shape (four pools) follows the
  black hole's per-subsystem precedent rather than the
  murmuration's single pool: each population spawns, steps and
  resets independently, and the capture economy crosses them by
  intent, not by index arithmetic.
- The drawn-cell generation array keeps the family's
  population-sized convention (particles x lines): at extreme
  bench widths the outermost cells can index past it and take
  the clear-then-repaint path — the family-accepted behavior at
  scale (the murmuration carries the same property), a few
  moving cells of extra dirt, no visual defect at interactive
  sizes.
- The neural network stays on the shortlist: this round's pick
  was the physics. A future neural style wants a layered-graph
  layout with signal pulses traveling edges — a different
  geometry family (the graph-vs-field problem stated honestly
  above).
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
