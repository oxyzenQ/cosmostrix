<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-lts-1 stage 7 — the master depth audit: dragon + neural network (final stage)

Owner directive 2026-09-10 (continuous approval, stages 2-7): stage
7 audits dragon + neural network, the final pair. This document is
the stage-7 report and closes the staged master depth audit.

## Scope and method

- Dragon (`cloud/type_rain/dragon/`): ~965 LOC — the orchestrator
  (dragon.rs, 858 gross lines with the NIGHT-enhanced-4 LOC_EXEMPT
  marker verified: the entry-reveal animation's documented
  encapsulation trade) and the free helpers (dragon_helpers.rs).
- Neural network (`cloud/type_rain/neural/`): ~2 569 LOC across 8
  files — the state machine (neural.rs), the network geometry and
  physics steps (network.rs), the firing/pulse delivery (firing.rs),
  plasticity, spawn, genesis, draw.
- Method: grep-driven transcendental inventory across both
  directories, then full read of every hit's context plus the
  advance/draw hot paths; allocation scan (all `Vec` sites verified
  as construction-path or event-scale); panic scan (zero
  unwrap/expect/panic in production paths of both modules); test
  census (29 contract tests: dragon 13, neural 16 across
  core/genesis); baseline 10 s benches; the flux canary pair (the
  stage-5 layout control) to exonerate any sibling scene effect.

## The audit verdict: both styles at peak; two explicit-semantics items shipped

Both scenes run two orders of magnitude beyond display rate
(cosmic_dragon ~150K fps with ~67 dirty cells per frame — the
fastest structured scene in the project; neural ~120K fps with
~115 dirty cells). The dragon advance is the FABRIK-chain state
machine with pace entering only through dt (the hunter-10 fix);
the neural machine's wiring is deterministic by contract (no RNG —
the bench determinism guarantee) with the Poisson input kicks as
the only stochastic element. The audit ships the two
stage-precedent items it found and skips the rest with
quantification — the honest peak verdict.

## Shipped (bit-identical by construction)

1. **NeurFactors** (network.rs + neural.rs) — the stage-1 quasar
   StepFactors precedent applied to the neural physics: the
   membrane leak, fired-flash and signal-glow decay factors are
   pure functions of the frame's sim dt, but were re-evaluated as
   `exp()` inside `neuron_step` (twice per active neuron) and
   `synapse_step` (once per active synapse) — ~150 exponential
   evaluations per frame at bench geometry (~30 neurons, ~90
   synapses). The advance pass now evaluates the three factors
   once per frame and threads the `NeurFactors` snapshot (which
   also carries the dt for the linear terms — the refractory
   window, the retire fade, the ages). The spontaneous-input
   `kick_p` exponential was already hoisted at its loop head (the
   audit verified rather than duplicated it). Bit-identical: the
   same three products, same values, evaluated once.
2. **Dragon heading fusion** (dragon.rs advance loop) — the head
   translation's separate `heading.cos()` / `heading.sin()` pair
   reads as one `sin_cos()` call (the stage-1 RollFrame / stage-6
   precedent), per dragon per frame.

## Considered and skipped (the over-engineering guard, quantified)

1. Dragon spawn-path `entry_heading.cos()/sin()` — once per dragon
   entry, a cold path; nanoseconds per spawn. Skipped.
2. `node_pos`'s golden-angle y-jitter `sin()` (network.rs) — the
   function's own contract is "a node's static position": computed
   at construction/genesis, never per frame. Skipped.
3. The neuron drift `sin()` (network.rs, `self.x + (drift + age *
   1.7).sin() * amp`) — per-neuron per-frame with an age-varying
   argument; irreducible. Skipped.
4. The wire bow `sin(PI * t)` (network.rs) — per wire cell with
   t varying; irreducible. Skipped.
5. `wire_path`'s `Vec::with_capacity` — event-scale (pulse spawn),
   capacity-pre-allocated. Skipped.
6. `color_for_level` palette-index ladder — the stages 2-6
   cross-family verdicts stand (signature shared by six families,
   sub-microsecond, LLVM-CSE'd). Cited.
7. Dragon sway/circle/noise coefficients (0.7/0.3, 13.7, 7.3) —
   characteristic formula coefficients documented in place; naming
   them adds no tuning surface. Skipped.
8. `drawn_gen` u32 wrap asymmetry — the stages 3-5 verdict
   (collision unreachable). Cited.
9. The dragon `Vec` sites — construction/reset/test-only (the
   `LOC_EXEMPT` marker and the `cfg(test)` diagnostics block both
   verified). Skipped.

## Baseline measurements (10 s, this audit's build, 80x24 default)

| scene | avg fps | sim ms | dirty cells/frame | density gini | frame entropy (bits) |
|---|---|---|---|---|---|
| cosmic_dragon | 150 229 | 0.0039 | 67.43 | 0.654 | 4.944 |
| neural | 119 937 | 0.0047 | 115.32 | 0.507 | 5.586 |

## A/B (10 s benches, 2 runs per side, run-averaged)

| scene | metric | baseline | after | delta |
|---|---|---|---|---|
| cosmic_dragon | avg fps | 150 228.90 | 151 565.81 | +0.89 % |
| cosmic_dragon | sim ms | 0.003940 | 0.003907 | −0.84 % |
| cosmic_dragon | dirty cells | 67.43 | 67.40 | −0.04 % |
| cosmic_dragon | density gini | 0.6536 | 0.6562 | +0.40 % |
| cosmic_dragon | entropy bits | 4.9437 | 4.9339 | −0.20 % |
| neural | avg fps | 119 936.85 | 119 708.96 | −0.19 % |
| neural | sim ms | 0.004727 | 0.004747 | +0.42 % |
| neural | dirty cells | 115.32 | 115.41 | +0.08 % |
| neural | density gini | 0.5068 | 0.5036 | −0.63 % |
| neural | entropy bits | 5.5856 | 5.5920 | +0.11 % |

Run-level spread: cosmic_dragon 1.10 % (baseline) / 0.37 %
(after); neural 0.02 % (an anomalously tight lucky pair — the
stage-3 yardstick caveat) / 0.19 %. Both scenes are stochastic
(state-machine rolls, Poisson kicks), so gini/entropy drift with
the sampled trajectory; the dirty-cell counts match to within
0.1 %. The flux canary measured 62 904 fps — within 0.45 % of
every stage-5/6 reference point — so no sibling layout regression.
Both shipped changes compute identical values by construction, so
no behavioral channel exists: the deltas are machine-state and
sampling noise. Verdict: performance- and visual-neutral, the
expected outcome for explicit-semantics changes on
already-optimizer-hoisted code (the neural factors take pure dt
arguments — the same class LLVM lifted in stage 1's quasar A/B).

## Stage gate — and the audit's close

Stage 7 complete: 2 679 tests passed + 2 ignored; fmt, clippy
--all-targets, gate-keepers.sh 10/10; build.sh check-all -q green
(the first attempt hit the two-minute kill on the cold Windows
cross-compile — the fallback protocol ran the light checks
individually, and the warm-cache retry completed the full suite;
the owner's timeout rule worked exactly as designed). LOC
network.rs 641 / neural.rs 741 / dragon.rs 863 (exempt marker
intact). Evidence in `benchmark/bench-labs/night_lts1_stage7/`.

NIGHT-lts-1 is now complete across all seven stages. Every
scene family in the project carries a measured peak verdict: 13
stages of A/B evidence (stages 1-7, two scenes each), one real
optimization win (+8.58 % dna helix, stage 6), one
measured-and-reverted layout regression (stage 5), and the
standing toolset this audit contributed: the .text-md5 codegen
control (stage 4) and the same-window binary ablation (stage 5).
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
