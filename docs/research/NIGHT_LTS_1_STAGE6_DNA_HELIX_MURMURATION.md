<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-lts-1 stage 6 — the master depth audit: dna helix + murmuration

Owner directive 2026-09-10 (continuous approval, stages 2-7): stage 6
audits the dna helix + murmuration pair. This document is the
stage-6 report. It records the audit's first MEASURED performance
win (stages 1-5 shipped only optimizer-neutral explicit semantics).

## Scope and method

- DNA helix (`cloud/type_rain/dna_helix/`): ~2 037 LOC — the
  orchestrator/draw pass (draw.rs), the genome with strand/rung
  geometry and the charge/replication laws (helix.rs), the genesis
  timeline (genesis.rs).
- Murmuration (`cloud/type_rain/murmuration/`): ~1 326 LOC — the
  Reynolds-boids flock (boids.rs: spatial-hash buckets, neighbor
  scan, flock_forces triad) and the orchestrator (murmuration.rs:
  clocks, anchor, predator startle, draw).
- Hot-path dependencies: the shared monolith ladder, DrawCtx,
  style_rain constants.
- Method: full read of the draw/advance hot paths; transcendental
  inventory; allocation scan; panic scan (zero in production
  paths); test census; baseline 10 s benches; a flux canary pair
  (the stage-5 layout-regression lesson: any code-size change can
  perturb sibling scenes through binary layout, so an untouched
  high-sensitivity scene is benched as a control).

## The audit verdict: one real win shipped, both styles otherwise at peak

The dna helix rung draw pass carried a genuine per-cell
redundancy: `rung_depth(idx, t)` re-evaluated the FULL strand
projection (angle, sin/cos, effective radius, the Option branch)
for every span cell, although the projection is a per-rung
invariant — only the blend fraction t varies per cell. Unlike
stages 1-5, where the A/B proved LLVM had already lifted the
invariant, here it had not: the method chain reads nested state
through `self.genome` and interleaves with `frame.set` side
effects in the span loop, so the optimizer never hoisted it. The
measured result of making the invariance explicit in the source:

| metric | baseline (2 runs) | after (4 runs) | delta |
|---|---|---|---|
| avg fps | 88 470.62 | 96 062.68 | **+8.58 %** |
| avg sim ms | 0.007290 | 0.006417 | −11.98 % |
| dirty cells/frame | 107.89 | 107.90 | +0.01 % |
| density gini | 0.6720 | 0.6720 | 0.00 % |
| frame entropy (bits) | 4.9697 | 4.9701 | +0.01 % |

The fps separation is decisive: baseline runs 88 325-88 616, after
runs 95 336-96 414 — no overlap. The visual metrics match to three
decimals across all six runs (dirty 107.87-107.96, gini 0.6720
throughout, entropy 4.9696-4.9709), the direct bit-identical
evidence. The murmuration side (one value-identical constant
shipped) is neutral: 25 117 -> 25 105 fps (−0.05 %), sim +0.12 %,
with gini/entropy inside the scene's own stochastic run-to-run
band (the flock evolves a different trajectory per run; the
baseline pair itself spans gini 0.6875-0.6921 and entropy
4.8090-4.8322). The flux canary measured 62 753 fps — within 0.4 %
of both stage-5 reference points — so the dna helix code-size
change caused no sibling layout regression.

## Shipped (dna helix: the projection economy; bit-identical values)

1. `strand_pair` (helix.rs + draw.rs): the strand pass reads strand
   A's projection once per line and derives strand B by the mirror
   arithmetic (x mirrored about the axis, depth negated) — the old
   `strand_b(line)` internally re-evaluated `strand_a(line)`, so the
   strand pass paid the trig twice per line. Bit-identical: the old
   strand_b WAS this mirror.
2. `rung_geometry` (helix.rs + draw.rs): one projection per rung
   returning (left x, right x, strand A depth) — the per-rung
   invariant the span-cell loop needs. `rung_span` now delegates to
   it (other callers and tests unchanged).
3. `rung_depth_blend` (helix.rs): the per-cell blend alone
   (`ad * (1 - t) - ad * t`, the former `rung_depth` body verbatim)
   reading the hoisted depth — the per-cell projection is gone.
   This is the +8.6 % item.
4. `strand_a` sin/cos pair fused to `sin_cos()` (the stage-4
   precedent; LLVM emits the fused call either way).
5. `STRAND_DEPTH_HOT` (0.55) and `RUNG_DEPTH_BLEND_BAND` (0.30)
   (draw.rs): law 1's front-deep Hot threshold and law 2's
   depth-blend band promoted to named constants, value-identical,
   both comparison sites sharing the band constant.
6. Murmuration: `MURM_BREATH_WRAP_LIMIT` (= TAU x 64) — the
   breathing-oscillator phase wrap promoted from an inline literal,
   mirroring the DNA/vortex wrap-limit family precedent.
   Value-identical.

Test updates: the strand mirror test reads through `strand_pair`
(the same values the separate strand_a + strand_b reads returned).

## Considered and skipped (the over-engineering guard, quantified)

1. The boids neighbor scan — already spatially bucketed (MurmHash
   grid, 3x3 bucket neighborhood, bucket-local force pass); the
   amortized family pattern, not O(n^2). Peak confirmed by
   construction; 25K fps at 80x24 is far beyond display rate.
2. `color_for_level` palette-index ladder — stages 2-5 quantified
   and skipped it; both styles call it through the shared
   cross-family signature. Cited, not re-measured.
3. trail-depth `.min(N)` literal caps — stage 3's 9-site census
   includes dna_helix; stages 3-4 re-derived the output-invariance
   (the brightness ladder saturates at depth >= 2). Confirmed.
4. Law 3's charge decay `exp()` inside the per-rung loop — the
   disassembly control (objdump of the baseline binary) shows LLVM
   already evaluates `expf` once before the multiply loop and
   vectorizes it; the source keeps the family's per-rung shape
   (documented in place). Skipped with instruction-level evidence.
5. `drawn_gen` u32 wrap asymmetry — stages 3-5 verdict (collision
   unreachable; previous cells always tagged gen-1). Cited.
6. The genesis timeline and fork mechanics — event-driven, cold
   paths; no per-frame work to hoist.

## Baseline measurements (10 s, this audit's build, 80x24 default)

| scene | avg fps | sim ms | dirty cells/frame | density gini | frame entropy (bits) |
|---|---|---|---|---|---|
| dna_helix | 88 471 | 0.0073 | 107.89 | 0.672 | 4.970 |
| murmuration | 25 117 | 0.0346 | 144.47 | 0.690 | 4.821 |

## A/B evidence trail

`benchmark/bench-labs/night_lts1_stage6/`: `baseline_*` (2 runs per
scene, pre-edit), `after_dna_helix_run1..4` (the win, four runs for
a decisive separation), `after_murmuration_run1..2`, and
`canary_flux_run1..2` (the stage-5 layout control, clean).

## Stage gate

Stage 6 complete: the dna helix projection economy ships with a
measured +8.58 % fps and −11.98 % sim ms at bit-identical visual
output; murmuration confirmed at peak with one value-identical
constant promoted; the flux canary exonerates the change of any
sibling layout cost; the full gate suite green (fmt, clippy
--all-targets, build.sh check-all -q, gate-keepers.sh 10/10, 2 679
tests passed + 2 ignored in 56.4 s; LOC draw.rs 393 / helix.rs 691 /
murmuration.rs 496, all under the 800 cap). Proceeding to stage 7
(dragon + neural network) under the owner's continuous-approval
directive — the final stage.
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
