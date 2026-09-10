<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-lts-1 stage 5 — the master depth audit: flux + aeolian

Owner directive 2026-09-10 (continuous approval, stages 2-7): stage 5
audits flux + aeolian. This document is the stage-5 report. It also
records the audit's first measured binary-layout regression and the
same-window ablation method that caught it.

## Scope and method

- Flux (`cloud/type_rain/flux/`): the PIC/FLIP two-way fluid style
  (task-19, superseded the rejected ripple style) — the field
  (flux_field.rs: P2G accumulate/normalize, gravity, transfer),
  the particle advance and draw, the spawn pass. ~1 045 LOC.
- Aeolian (`cloud/type_rain/aeolian/`): the instrument style — the
  string field with its hop-clock lattice and resonance laws
  (strings.rs), the falling drops with string-seeking capture
  (drops + core advance/draw). ~1 420 LOC.
- Hot-path dependencies audited: the shared monolith ladder
  (color_for_level / clear_cell / pick_pool_char), DrawCtx, and the
  style_rain tuning constants.
- Method: full read of every advance/draw/spawn hot path; allocation
  scan (the only family with a per-frame heap allocation candidate —
  see the reverted section); transcendental inventory; panic scan
  (zero in production paths); test census (flux and aeolian suites
  under cloud/tests/); baseline 10 s benches for the record.
- The flux fixed 60 Hz PIC/FLIP stepping contract (one step per bench
  frame, ~0.006 ms) is the documented rate-independence regime and is
  not an optimization target.

## The audit verdict: both styles are at peak, with one trap found and disarmed

The per-particle and per-cell work is the irreducible solver physics
(flux: P2G/G2P transfers, coherent motion producing the lowest
dirty-cell count of any style, ~60/frame; aeolian: string conduction
plus drop seeking at ~46 dirty cells/frame). Both styles run at
~62K (flux) and ~159K (aeolian) fps — one to three orders of
magnitude beyond display rate. Two hygiene constants shipped. The
one candidate that looked like a genuine win — removing the aeolian
per-frame row-snapshot `Vec` allocation — measured as a reproducible
REGRESSION on a sibling scene and was reverted; the full story below,
because the method is now part of the audit's toolkit.

## Shipped (value-identical constant promotions, codegen-identical)

1. `AEOLIAN_SEEK_VX_LIMIT` (style_rain.rs): the lateral bend clamp
   `d.vx.clamp(-3.0, 3.0)` promoted to a named constant with the
   "bend, never a slide" contract documented — the value is
   unchanged.
2. `FLUX_WEIGHT_FLOOR` (flux_field.rs): the P2G weight-epsilon
   `1.0e-6` appeared twice (accumulator normalization and the
   gravity pass); both sites now share one named threshold with the
   sharing documented.

## The reverted candidate: allocation removal vs binary layout

The advance and draw passes of aeolian each began with
`self.strings.string_rows().to_vec()` — a genuine per-frame heap
allocation (two per frame total) forced by the split borrow (the
capture pass mutates the field while iterating the rows). The
candidate replaced it with a fixed stack buffer sized to the tier
ladder's bound (4 rows max by construction) — values and iteration
order identical, zero behavior change by inspection.

The A/B then measured, reproducibly across three independent builds:

| state | flux avg fps | note |
|---|---|---|
| baseline (clean binary, first window) | 62 623.4 | spread 0.05 % |
| full-diff binary, build A | 61 508.9 | 4 runs, spread 2.01 % |
| full-diff binary, build B (fresh rebuild) | 61 578.2 | 2 runs, spread 1.00 % |
| clean binary, same window as build B | 62 670.8 | 2 runs, spread 0.01 % |
| consts-only binary (reverted, same day) | 62 920.4 | 2 runs, spread 0.38 % |

The same-window control is the decisive row: the clean binary
measures 62 671 while the full-diff binary measures 61 578 minutes
apart on the same machine — machine drift is excluded. The flux
source delta itself is a value-identical constant rename (no
behavioral channel), so the ~1.6 % penalty is a binary-layout
effect: the aeolian function-size change shifts code placement in
the shared binary, and flux landed on worse alignment. Cargo codegen
is deterministic, so the penalty reproduces on every rebuild of this
exact diff.

The trade fails the over-engineering guard in both directions: the
allocation removal bought nothing measurable on its own scene
(aeolian moved +0.26 %, inside its 0.18 % run spread — 160K fps
already amortizes two small allocations to noise) while costing a
reproducible ~1.6 % on an untouched sibling scene. Reverted. The
`to_vec()` snapshot stays, now with the measured justification on
record. The same-window binary ablation (clean-binary control
benched in the same machine window as the candidate binary) joins
the stage-4 .text-md5 control as a standing audit tool: the former
catches layout regressions, the latter proves codegen identity.

## Considered and skipped (the over-engineering guard, quantified)

1. The aeolian row-snapshot stack buffer — reverted, see above.
2. `color_for_level` palette-index ladder per cell — stages 2, 3 and
   4 quantified and skipped it; flux and aeolian call it through the
   same shared signature, and the cross-family blast radius verdict
   stands.
3. trail-depth `.min(N)` literal caps — stage 3's census includes
   aeolian and flux; stage 4 re-derived the output-invariance (the
   brightness ladder saturates at depth >= 2) and this stage's read
   of both styles confirms the same saturation. Skipped.
4. flux P2G/G2P transfer loops — the irreducible solver cost
   (~0.013 ms sim per frame total at 80x24), fixed-stepping contract
   preserved.
5. aeolian string conduction laws (the hop-clock lattice) —
   per-cell channel work with no loop-invariant remainder; the decay
   `exp()` is once per frame, already hoisted at the pass head.
6. drawn_gen u32 wrap asymmetry — shared with vortex/solar (stages
   3-4 verdict: collision unreachable, previous cells always tagged
   gen-1). Skipped.

## Baseline measurements (10 s, this audit's build, 80x24 default)

| scene | avg fps | sim ms | dirty cells/frame | density gini | frame entropy (bits) |
|---|---|---|---|---|---|
| flux | 62 623 | 0.0133 | 59.72 | 0.643 | 5.062 |
| aeolian | 159 390 | 0.0042 | 46.01 | 0.645 | 4.980 |

## A/B (10 s benches, 2 runs per side, run-averaged, shipped state)

| scene | metric | baseline | after | delta |
|---|---|---|---|---|
| flux | avg fps | 62 623.38 | 62 920.38 | +0.47 % |
| flux | sim ms | 0.013342 | 0.013278 | −0.48 % |
| flux | dirty cells | 59.72 | 59.71 | −0.02 % |
| flux | density gini | 0.6426 | 0.6429 | +0.05 % |
| flux | entropy bits | 5.0623 | 5.0613 | −0.02 % |
| aeolian | avg fps | 159 390.18 | 159 211.72 | −0.11 % |
| aeolian | sim ms | 0.004231 | 0.004246 | +0.35 % |
| aeolian | dirty cells | 46.01 | 46.02 | +0.01 % |
| aeolian | density gini | 0.6454 | 0.6449 | −0.08 % |
| aeolian | entropy bits | 4.9795 | 4.9812 | +0.03 % |

Run-level spread: flux 0.05 % (baseline) / 0.38 % (after); aeolian
0.18 % / 0.24 %. Every delta sits inside or near the spread —
performance- and visual-neutral, as expected for two constant
renames whose functions compile to identical code. Visual metrics
match to three decimals on both scenes.

Evidence trail in `benchmark/bench-labs/night_lts1_stage5/`:
`baseline_*` (clean), `baseline_recheck_flux_*` (drift
investigation), `ablation_cleanflux_*` (same-window clean-binary
control), `evidence_prerevert_*` (the full-diff state before the
revert), `after_*` (the shipped state).

## Stage gate

Stage 5 complete: flux and aeolian confirmed at peak with measured
evidence; two codegen-identical constant promotions shipped; one
allocation-removal candidate caught, ablation-tested, and reverted
with the full quantitative record; the gate suite green (fmt, clippy
--all-targets, build.sh check-all -q, gate-keepers.sh 10/10, 2 679
tests passed + 2 ignored in 56.3 s). Proceeding to stage 6
(dna helix + murmuration) under the owner's continuous-approval
directive.
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
