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

## Baseline measurements (pro profile, 10 s, this audit's build)

| scene | avg fps | peak fps | dirty cells/frame | density gini | frame entropy (bits) |
|---|---|---|---|---|---|
| sorgonemous_intrascals | 25 914 | 30 046 | 330.3 | 0.554 | 5.483 |
| quasar | 96 525 | 113 225 | 84.8 | 0.711 | 4.789 |

Both styles run one to two orders of magnitude beyond any
terminal's display rate — the frame budget is nowhere near
pressure, which is the quantitative form of the peak verdict: any
micro-optimization here buys nothing the user could perceive.

## Stage gate

Stage 1 complete. No code changes warranted — the two styles are
at peak (the audit's product is this report). Awaiting the owner's
approval to open stage 2: glyph + monolith.
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
