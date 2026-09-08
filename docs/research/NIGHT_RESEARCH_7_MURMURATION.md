<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-research-7: the murmuration — the twelfth rain style

Task: the owner's DeepSeek-researched shortlist of new rain types,
second pick (the first, the DNA helix, landed in the previous
commit — each style ships as its own commit for an independent
owner rating, the corona arcade commit ce7b6f4 the 10/10
reference). This document is the murmuration half: the research,
the derivation, the implementation map, and the verification
record.

## The research

DeepSeek's shortlist ranked the murmuration 9/10 ("paling organik
dan hidup... visual paling wow di terminal") with the honest cost
note: ~500-600 LOC, the most complex candidate, needing a boids
system with per-particle state. The final design keeps the
canonical core (Reynolds 1987 boids) and adds the terminal-mapping
derivations: the spatial hash that turns O(n^2) neighbor scans
into O(n) (the reason 100-220 birds cost the same per frame as a
30-bird naive scan), the roaming anchor (the flock's macro
intent), the breathing cohesion (the signature shape cycles), and
the clocked startle (the predator drama). The starling-field
literature is honored in the calibration: the neighbor window is
sized so the average bird scans the topological interaction number
(~7, Ballerini 2008) at the shipped population dial.

## The five laws of the flock

(Complete derivation essay: `type_rain/murmuration/mod.rs` — the
commit mirrors the house law-structured documentation.)

1. **The three forces** (Reynolds 1987): separation (the
   inverse-distance push inside the 3-cell tight radius — the
   minimum spacing IS the visual bird density), alignment (the
   steer toward the neighbors' mean heading — what makes a
   hundred strokes read as ONE body), cohesion (the weak spring
   toward the local centroid). Semi-implicit Euler integration
   with the speed clamps [7, 26] cells/sim-s: a starling never
   hovers, never teleports. A per-bird clamped jitter keeps the
   system organic (no dead-locked symmetric configurations).
2. **The neighbor window**: the alignment/cohesion radius (8
   cells) IS the hash bucket size; each bird reads its 3x3 bucket
   scan — O(n) pair checks per frame. The radius is sized so the
   average bird scans ~7-10 neighbors (the topological number).
3. **The thought** (the roaming anchor): a point that
   random-walks across the sky (clamped speed, re-rolled targets,
   wall reflection); every bird carries a WEAK attraction to it
   (0.05 vs cohesion 1.6 — the macro intent, never the collapse).
   The wall banking: a soft inward force inside the 9-cell margin
   — birds curve along the screen edges, never hit them (plus a
   hard re-project backstop that never fires in practice).
4. **The breathing**: the cohesion weight cycles on a slow sine
   (base 1.0 +- 0.75, period ~14 sim-s) — the flock periodically
   tightens into a dense ball and loosens into a wide cloud, THE
   signature murmuration shape-shift, emergent from one scalar.
5. **The startle**: on a global clock (mean 11 sim-s), a predator
   flashes at a rolled position — one glyph at Core brightness
   for 0.8 sim-s (the raptor's silhouette, the visible cause) —
   and every bird inside the 14-cell panic radius takes an outward
   velocity kick (an impulse, not a force: the scatter is
   instant), floored near max speed for 1.2 sim-s. The flock
   blooms apart and re-gathers under the triad.

## The scene

`murmuration` at cycle position 13 (after the DNA helix, grouped
with the style flagships): **gold** palette (previously unclaimed
— the birds read as starlings catching the last sun on a black
dusk sky) + **minimal** charset (the single nabla glyph: each bird
a small flying-V silhouette, the flock a field of gradient
operators). Speed 18 (the darting flight cadence), density 0.55
(the flock-size dial: ~86 birds at 120 cols — the hero dial, the
flock IS the scene, unlike the calm-sky rain styles), glitch none
(the sky is still, the startle is the drama — the composition
logic the solar flare established).

## Implementation map

- `src/engine/cosmic_dragon_engine/cloud/type_rain/murmuration/mod.rs`
  — the derivation essay + module map.
- `boids.rs` — the bird state (position, velocity, trail, panic
  age), the spatial hash (the bucket store, rebuilt per advance),
  the anchor, the force accumulation (the triad + anchor +
  banking, pure function of state), the Euler integration with
  the speed clamps, and the startle impulse.
- `murmuration.rs` — the flock: the pool (viewport-derived,
  density-dialed, re-sized on live density changes), the
  staggered-entry spawn through the family accumulator, the
  advance pass (clocks, hash rebuild, accumulate-then-integrate
  with split borrows), the test diagnostics.
- `draw.rs` — the draw pass: birds (heads + 2-cell comet trails,
  the kinetic speed ladder — fast edges bright, slow cores dim),
  the predator flash glyph, the generation-tagged diff cleanup.
- `central_control_rains/style_rain.rs` — the 28 shipped
  constants and their compile-time calibration contracts.
- Integration: `RainStyle::Murmuration` (labels `murmuration` /
  `murmur` / `starlings`), the Cloud field + the six dispatch
  chains, the scene catalog entry, SCENE_ORDER position 13, and
  the ten label-list comment surfaces.

## Verification record

- **Tests**: 27 new behavior contracts (tests_murmuration: 13
  physics-level — the speed clamps and heading preservation,
  separation push, cohesion pull, wall banking, the startle kick
  and its V_MAX saturation, the integration bounds, the hash
  window and its inactive skip, the anchor's roam-and-reflect,
  the edge activation headings, the flock target band; 14
  orchestration-level — the staggered entry, flight bounds and
  band, the flock's emergent coherence (radius of gyration <
  24 cells) and separation (closest pair > 0.5 cells), the
  breathing band, the startle cycle + scatter + re-gather, drawn
  bounds, repaint without residue, pause freeze, style
  round-trip, speed scaling, sustained boundedness (3600 frames),
  degenerate narrow terminals, palette adoption). Full suite:
  2613 passed / 0 failed (was 2586 — +27).
- **A/B 10 s** (`benchmark/bench-labs/night_research7_murm/`):
  cinematic 454.5 -> 455.2 dirty / 5.157 -> 5.163 entropy /
  0.641 -> 0.640 gini; aeolian 46.1 -> 46.0 / 4.983 -> 4.978 /
  0.644 -> 0.646; dna_helix 101.9 -> 101.9 / 4.860 -> 4.859 /
  0.692 -> 0.692 — zero visual regression (third-decimal
  identical). The new scene: 24,948 fps / 144.5 dirty / 4.841
  entropy / 0.685 gini (see AB_REPORT.md).
- **PTY smoke**: time-lapse screen reconstructions at 4 s and 8 s
  show the flock's macro motion (right-side loose blob at 4 s,
  traveled to the left as a tight dense ball at 8 s) and the ∇
  bird glyphs; the staggered entry assembles over the first
  seconds.
- **Quality gates**: cargo fmt clean; clippy -D warnings clean;
  check-all -q exit 0; gate-keepers 13/13.

## Design notes (the honest trade-offs)

- **Canonical, not original-math**: Reynolds 1987 is the
  reference, like Lorenz 1963 / Jones 2010 / PIC-FLIP / the
  Watson-Crick helix. The derived contributions: the spatial hash
  mapping, the anchor/breathing/startle extensions, the
  calibration to the starling-field findings.
- **The O(n) hash is the enabling engineering**: naive boids at
  100 birds would cost 10,000 distance checks per frame; the hash
  costs ~10 per bird. The dirty-cell budget caps the visual
  population at 220.
- **24.9K fps is honest physics cost**: unlike the field styles
  (whose per-frame cost is mostly the draw), every murmuration
  frame integrates every bird — the price of a real
  many-body simulation, paid gladly, in the sorgonemous band.
- **The predator is visible for 0.8 s**: the causal read (the
  owner rates the drama) — a single Core-bright pool glyph where
  the scatter began, gone before it becomes furniture.
- **Birds are immortal after entry**: a murmuration does not
  churn members; the family accumulator contract is honored
  through the staggered entry (and the live density dial
  re-sizes the pool through the spawn pass).
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
