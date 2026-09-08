<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-research-7: the DNA helix — the eleventh rain style

Task: the owner's DeepSeek-researched shortlist of new rain types
(the first pick approved for implementation now, the murmuration
second — each lands as its own commit so the owner can rate them
independently, the corona arcade commit ce7b6f4 the 10/10
reference). This document is the DNA helix half: the research, the
derivation, the implementation map, and the verification record.

## The research

DeepSeek's shortlist ranked the DNA helix 9/10 ("ikonik dan
nerdy... cocok untuk project cosmic dragon") with the reuse note:
"vortex polar orbit + lorenz z-depth — cukup dengan mapping posisi
partikel ke helix 3D". The final design honors the icon but does
NOT borrow the vortex orbit integration: the helix is not a
particle system on orbits, it is a FIELD — a standing structure
whose geometry is closed-form in one rotation phase. What the
vortex actually contributed is the single-body-on-one-clock
composition and the phase-wrap LTS precedent; what the lorenz
contributed is the z-as-depth brightness read (depth grades the
ladder rungs, front bright / back dim). The canonical reference
(Watson-Crick-Franklin 1953) is the icon; the terminal mapping
(the projection of a rotating ribbon onto a discrete cell grid,
the rung recency economy, the fork's traveling-wave envelope) is
derived here, in the house derivation style.

## The five laws of the ladder

(Complete derivation essay: `type_rain/dna_helix/mod.rs` — the
commit mirrors the solar flare's law-structured documentation.)

1. **The turn** — theta(y) = phi + y k; k = 2pi / 22 rad/line
   (one full turn every 22 lines; with a rung every 2 lines that
   is 11 base pairs per turn — B-DNA's 10.5 honored at terminal
   legibility). The molecule rotates uniformly (phi at 0.5
   rad/sim-s); strand A at (cx + R sin(theta), cos(theta)),
   strand B at theta + pi. The projection gives the X crossings
   for free: where sin(theta) = 0 both strands sit at cx, one
   front one back — the signature DNA ladder crossings, drifting
   as the molecule turns.
2. **The pairing** — a rung every 2 lines carries a Watson-Crick
   pair state; the rung's projected span is 2 R_eff |sin(theta)|
   (the projection IS the rung geometry: wide at the lateral
   swing, collapsed at the crossings). The rung END cells show the
   bases (semantic identity glyphs — A/T/G/C survive charset
   switches, the dragon-head precedent); dashed bond glyphs span
   between (stride 4 — a solid bar would read as a wall of text;
   the dashes read as hydrogen bonds). Every rung cell interpolates
   the strand depths: the front half steps one rung brighter.
3. **The recency** — each rung carries a synthesis charge in
   [0, 2.6], hard-clamped, exponentially decaying (0.35/sim-s).
   The charge ladder: Ghost the ancient archive, Mid transcribed,
   Hot freshly written, Core the replication window. The light
   shows where the genome has been recently written — the corona's
   deposition economy, transplanted onto a ladder.
4. **The replication** — a clocked fork (mean 12 sim-s between
   sweeps) enters above the screen and travels down at 8
   lines/sim-s. Rungs within the Gaussian window (sigma =
   min(7, 35% of height)) are dissolved; the strands bow apart
   through the window (+85% local radius at the peak — the Y) and
   peel upward slightly (half a rung spacing of lift). As the fork
   center crosses each rung: re-synthesis — the charge stamps to
   max and the pair RE-ROLLS (the visible mutation). The stamped
   charge decays while the wave travels on, so a brightness
   gradient trails the fork down the molecule — the replication's
   wake. The fork exits at the floor and the clock re-arms.
5. **The soup** — free nucleotides (the rain) fall from the top
   inside the capture band (cx +- 1.6 R): terminal velocity (8
   lines/sim-s +- 25%), a clamped brownian lateral drift. A drop
   crossing a rung line inside the rung's projected span (+-1.5
   cells) is absorbed: the charge deposits and, at 35% chance, the
   pair re-rolls — the rain visibly edits the genome it lands on.
   Misses fall to the floor and expire; the lifetime backstop
   (16 sim-s +- 15%) sweeps stragglers. The calm-sky dial (6% +
   density x 5%, cap 16%) keeps the soup a sparse minority — the
   molecule is the hero.

## Stability

Bounded by construction throughout: the phase wraps at 128 turns
(the vortex arm-phase precedent — f32 ulp far below visual
resolution on multi-day sessions); the radius clamps to
[3.5, 16.0]; the bow is a bounded multiplier with R_eff clamped
inside the viewport margins; the charge decays strictly and clamps
at the max; the fork is a linear position with a clocked re-entry;
the drop pool is lane-bounded (one per column) with wall clamps,
velocity clamps, floor expiry, and the lifetime backstop; the
absorption test is a crossing test on a strictly increasing y
(terminal velocity > 0) — no tunneling past a rung line at any dt.
Every drawn cell is bounds-checked at the frame.

## The scene

`dna_helix` at cycle position 12 (after the solar flare, grouped
with the style flagships): **neptune** palette (the iconic deep
azure — the classic DNA-illustration blue, previously unclaimed by
any scene) + **dna** charset (the shipped ACGT preset — the scene
finally gives the charset its flagship: the soup renders real
bases, the rung ends real complementary pairs). Speed 14 (the
majestic turn — a full rotation every ~11 s at scene speed),
density 0.50 (the calm-sky dial), glitch none (the genome is
clean, the mutation is the drama — the solar-flare's
composition-inversion logic).

## Implementation map

- `src/engine/cosmic_dragon_engine/cloud/type_rain/dna_helix/mod.rs`
  — the derivation essay + module map.
- `helix.rs` — the genome: rung table (pair, charge), the
  rotation phase, the fork state machine, the closed-form geometry
  queries (strand positions/depths, rung spans, the bow envelope,
  the dissolve window), laws 1-4 executable form.
- `drops.rs` — the nucleotide drop (position, terminal velocity,
  brownian drift, trail, the age ladder).
- `dna_helix.rs` — the orchestration: lane pool, accumulator
  spawn (capture-band rolls), advance (genome first, then the drop
  physics with the collect-then-apply absorption — the solar
  landings pattern), test diagnostics.
- `draw.rs` — the draw pass: strands (back-then-front over the
  frame, the bow lift), rungs (dashed bonds, base ends,
  depth-stepped ladder), soup (heads + 2-cell comet trails), the
  generation-tagged diff cleanup. Split from the orchestration at
  the 800-LOC cap (the monolith family's split pattern).
- `central_control_rains/style_rain.rs` — the 30 shipped
  constants and their compile-time calibration contracts.
- Integration: `RainStyle::DnaHelix` (labels `dna_helix` /
  `dnahelix` / `dna`), the Cloud field + the six dispatch chains
  (rain_at: adopt/spawn/semantic/force-draw/advance/draw; spawn:
  charset + palette; runtime_controls: palette + shading;
  scene_runtime: exit + entry; spawn_reset: the full reset), the
  scene catalog entry, SCENE_ORDER position 12, and the ten
  label-list comment surfaces (help_detail, scene_custom family,
  configfile_dump, scene_apply, the HUD trio).

## Verification record

- **Tests**: 25 new behavior contracts (tests_dna_helix: 13
  genome-level and 12 orchestration) — the rung registry,
  rotation uniformity,
  strand mirror + crossings, span breathing, Watson-Crick
  complementarity, bounded charge, the fork travel/dissolve/
  re-synthesis/mutation/bow/re-arm cycle, spawn dial, monotone
  fall, absorption + mutation through the full pipeline, drawn
  bounds, repaint without residue, pause freeze, style round-trip,
  speed scaling, sustained boundedness (3600 frames), degenerate
  narrow terminals (20x6). Full suite: 2586 passed / 0 failed
  (was 2561 — +25).
- **A/B 10 s** (`benchmark/bench-labs/night_research7_dna/`):
  cinematic 460.9->457.3 dirty / 5.174->5.165 entropy / 0.637->
  0.639 gini; aeolian 45.9->46.0 / 4.979->4.979 / 0.646->0.646;
  solar_flare 247.5->249.5 / 6.037->6.037 / 0.325->0.325 — zero
  visual regression (third-decimal identical); fps deltas
  (+2.3/+1.5/-1.0%) inside the interleaved-run noise band. The new
  scene: 96,591 fps / 102.0 dirty / 4.859 entropy / 0.692 gini
  (the concentrated single-body composition — see AB_REPORT.md).
- **PTY smoke**: the 100x40 visual reconstruction shows the
  alternating strand-rung ladder, the dashed bonds, the X
  crossings at the ~11-line half-turn, and the sparse soup
  (scripts/dna_visual_check.py, the ansi_screen reconstructor); a
  16 s direct run exits cleanly at the duration cap with a full
  ANSI stream.
- **Quality gates**: cargo fmt clean; clippy -D warnings clean
  (two argument-bundles folded to stay under the 7-arg threshold);
  check-all -q exit 0; gate-keepers 10/10.

## Design notes (the honest trade-offs)

- **Canonical, not original-math**: the double helix is a borrowed
  reference (like Lorenz 1963 / Jones 2010 / PIC-FLIP before it) —
  the original-math line stays with aeolian and the corona. The
  derived contribution is the terminal mapping: the projection
  economy (the rung geometry IS the projection), the recency
  economy, and the fork's traveling-wave envelope.
- **One helix, centered** (not a lane of helices on wide
  terminals): the single-body flagship aesthetic the black hole
  established — the icon stays an icon; the soup fills the sky.
- **The dashed bond** (stride 4): a full-span rung bar measured
  30+ cells wide and read as a wall of text in the PTY
  reconstruction; the dashes read as hydrogen bonds and cut the
  drawn population to a lean 102 cells/frame.
- **The charset question**: DeepSeek suggested charset binary;
  the shipped `dna` preset (ACGT) is the honest pick — the soup
  renders real bases and the rung ends real pairs.
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
