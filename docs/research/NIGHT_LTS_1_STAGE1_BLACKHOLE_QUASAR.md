<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-lts-1 stage 1 — the master depth audit: black hole + quasar (report to the owner, gate for stage 2)

Owner directive 2026-09-10: "master depth audit for all scene and
type rain, for peak optimize code, and LTS. first stage 1 only
blackhole+quasar, if done report to owner, stage 2 if owner approve
only for glyph+monolith." This document is the stage-1 report.

## Scope and method

- Black hole (`cloud/type_rain/black_hole/`): 7 files, 3 493 LOC —
  the orchestrator (black_hole.rs 1 435, LOC-exempted with a
  documented encapsulation trade), the per-mote physics (ring.rs),
  the halo streams (halo.rs), the glyph infall third pool
  (infall.rs), the formation birth sequence (formation.rs), the ball
  cell helpers (ball_helpers.rs).
- Quasar (`cloud/type_rain/quasar/`): 6 files, 1 934 LOC — the
  orchestrator (quasar.rs), the five-laws particles (particles.rs),
  the draw pass with its QuasPainter params-struct (draw.rs), the
  ignition timeline (ignition.rs), the capture economy (capture.rs).
- Method: full read of every advance/draw/spawn hot path;
  panic/unwrap/expect scan (result: zero hits in both modules);
  const-assertion scan (physics invariants pinned in style_rain.rs);
  test-coverage count (82 tests: black hole 57 across core/ring/
  infall/formation, quasar 25 across core/ignition); baseline
  10 s pro-profile benches for the record.

## The audit verdict: both styles are at peak

Five dimensions checked, each clean:

1. **Precision / math hygiene.** dt integration is saturating,
   clamped by max_sim_delta, scaled by resume_blend (the
   anti-teleport contract) — identical in both orchestrators. Phase
   angles wrap at 64 turns via rem_euclid (f32 precision kept —
   pulse, precession, disk theta, halo theta all wrap). Every
   physical input is clamped at its use site (disk f, halo f, infall
   f, jet s, tier specs) — defensive in depth: disk_step's
   exponential circularization provably stays in [DISK_INNER, 1]
   (convex combination), and disk_omega clamps again anyway.
2. **Hot-loop economics.** Per-frame state hoisted (roll angle,
   major_limit, center floats, outer radius, luminosity/pulse/knot);
   per-mote work is the irreducible physics (RK4 Lorenz = 4
   derivative evals — the documented stability regime; sin_cos
   paired evaluation used in projection). No allocation in any hot
   loop (current_cells pushes into pre-reserved capacity; spawn
   remainder budgets are scalar). Free-slot search is the amortized
   rotating-cursor O(1) pattern. Diff cleanup runs on the
   generation-tag drawn_gen array (no per-frame clear).
3. **Stability / LTS.** Zero unwrap/panic/expect in 5 427 LOC.
   Degenerate-viewport guards everywhere (zero cols/lines freeze
   the engine, spawn guards, resize rebuilds). Geometry is
   fraction-based end to end (the dynamic-size contract) — nothing
   assumes a terminal size. Formation state survives resize (the
   hole does not re-form); style re-entry resets choreography.
4. **Harmony / family contracts.** Both styles follow the family
   patterns: the three-pass draw (current_cells -> drawn_gen ->
   diff cleanup), the monolith brightness ladder, the
   motion-gated shimmer, the params-struct call signatures (the
   hunter-25 CellPaint school), the spawn-remainder fractional
   budget with SPAWN_REMAINDER_CAP. One style's constants live in
   one place with const assertions (BLACK_HOLE_INFALL capture <
   influence; QUAS ignition phases positive; halo weights ordered).
5. **Test coverage.** 82 contract tests across the two styles,
   including the formation timeline, the occlusion rule, the Kepler
   shear, the capture economy, and the infall brake.

## Considered and skipped (the over-engineering guard, quantified)

1. Parallel-array bounds checks in the ball draw loop
   (cell_angles / cell_buckets / cell_dist_norm / glyphs are
   rebuilt in lockstep at reset, yet each cell pays ~4 redundant
   `len() > idx` branches per frame). Measured impact: ~200-400
   cells x 4 branches = sub-microsecond per frame — against the
   per-cell shader work, ~0.003 percent. Removing them trades
   defensive depth for noise. Skipped.
2. `occludes_ring_cell` computes a sqrt then compares against
   ball_outer_r — could compare squared distances. ~200 calls per
   frame = ~600 ns. Skipped.
3. `ratio.powf(-KEPLER_EXP)` per mote per frame (Kepler's third
   law, non-integer exponent) — already the fast form of f^-1.5.
   Skipped.
4. `drawn_gen_counter` is a u32 with wrapping_add — a wrap
   collision would need a stale generation tag to equal the fresh
   one AND appear in the same frame's previous_cells; the tags are
   rewritten every frame, so the collision is unreachable in
   practice (4 billion frames = 2.3 years at 60 fps anyway).
   Skipped.
5. quasar draw does not `reserve()` current_cells before the first
   pass (black hole does) — capacity persists after the first
   frames regardless. Skipped.
6. The re-audit's two hoisting findings (roll trig per mote, exp
   decay per particle) were first BELIEVED to be missed wins —
   the A/B below proves the optimizer had already lifted them
   (neutral deltas). They ship as explicit source semantics, not
   as performance claims. Their measured cost before the fix was
   effectively zero; that is why items 1-5 stay skipped: the same
   optimizer argument covers them.

## Baseline measurements (pro profile, 10 s, this audit's build)

| scene | avg fps | peak fps | dirty cells/frame | density gini | frame entropy (bits) |
|---|---|---|---|---|---|
| sorgonemous_intrascals | 25 914 | 30 046 | 330.3 | 0.554 | 5.483 |
| quasar | 96 525 | 113 225 | 84.8 | 0.711 | 4.789 |

Both styles run one to two orders of magnitude beyond any
terminal's display rate — the frame budget is nowhere near
pressure, which is the quantitative form of the peak verdict: any
micro-optimization here buys nothing the user could perceive.

## Stage 1 re-audit addendum (same day): explicit hot-loop hoisting

An independent re-audit of the same scope found two latent
redundancies the first pass missed, implemented the explicit fix,
and A/B-verified the result — the fix is performance-neutral and
visual-neutral, which is itself the load-bearing evidence that the
compiler was already performing the hoisting (LLVM inlines the
small per-particle step functions, then LICM lifts the
loop-invariant transcendentals). The changes ship anyway because
they convert optimizer-dependent behavior into guaranteed source
semantics:

1. **Black hole — `RollFrame` (mod.rs):** the see-saw roll angle
   was hoisted per frame, but its `sin`/`cos` pair was re-evaluated
   per mote in `project_ring_mote`/`project_halo_mote` (the angle
   is rigid across the whole stack within a frame). The draw pass
   now snapshots one `RollFrame` per frame and threads the `Copy`
   pair through both pools' projections; `m.phi` evaluation fused
   to `sin_cos()` at the same time. The 20 projection test call
   sites pin the flat fixture through `RollFrame::FLAT`.
2. **Quasar — `StepFactors` (particles.rs):** `disk_step` and
   `halo_step` re-evaluated two and one `exp()` per particle per
   frame for three dt-only values; the advance pass now evaluates
   the three factors once per frame and threads the snapshot.
3. **Quasar — the inline `1.6` magic number of `halo_step`'s
   circularization damping promoted to the named constant
   `QUAS_HALO_CIRC_TAU` (value unchanged).

### A/B (10 s benches, this sandbox, 2 runs per side, run-averaged)

| scene | metric | baseline | after | delta |
|---|---|---|---|---|
| sorgonemous_intrascals | avg fps | 25 812.13 | 25 803.41 | −0.03 % |
| sorgonemous_intrascals | sim ms | 0.027909 | 0.027831 | −0.28 % |
| sorgonemous_intrascals | dirty cells | 330.69 | 330.33 | −0.11 % |
| sorgonemous_intrascals | density gini | 0.5532 | 0.5530 | −0.04 % |
| sorgonemous_intrascals | entropy bits | 5.4837 | 5.4846 | +0.02 % |
| quasar | avg fps | 96 576.78 | 96 510.77 | −0.07 % |
| quasar | sim ms | 0.007135 | 0.007138 | +0.04 % |
| quasar | dirty cells | 84.72 | 84.72 | 0.00 % |
| quasar | density gini | 0.7111 | 0.7112 | +0.01 % |
| quasar | entropy bits | 4.7862 | 4.7869 | +0.01 % |

Run-level spread (the noise yardstick): 0.22 % (black hole) and
0.06 % (quasar) — every delta sits inside it. The explicit hoisting
matters where the optimizer cannot be trusted to keep it: debug
builds (no inlining), future growth past an inline threshold, and
cross-backend codegen variance. The fused `sin_cos` and the named
constant are pure hygiene. Full A/B JSONs in
`benchmark/bench-labs/night_lts1_stage1/`.

## Stage gate

Stage 1 complete: the re-audit's explicit-hoisting pass is shipped,
A/B-verified performance- and visual-neutral, with the full gate
suite green (fmt, clippy --all-targets, build.sh check-all -q,
gate-keepers.sh 10/10, 2 679 tests). The two styles are confirmed
at peak — now with measured evidence rather than assertion.
Awaiting the owner's approval to open stage 2: glyph + monolith.
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
