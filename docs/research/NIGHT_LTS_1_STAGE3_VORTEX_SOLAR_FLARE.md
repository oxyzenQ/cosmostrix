<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-lts-1 stage 3 — the master depth audit: vortex + solar flare

Owner directive 2026-09-10 (stage 2 approval, continuous through
stage 7): the staged master depth audit proceeds family by family;
stage 3 owns the polar-orbit drain and the coronal arcade. This
document is the stage-3 report.

## Scope and method

- Vortex (`cloud/type_rain/vortex/`): 2 files, 579 LOC — the module
  wiring (mod.rs) and the single-file style system (vortex.rs 571:
  mote pool, arm-biased rim spawn, the Keplerian advance, the
  head-plus-comet-trail draw, the monolith three-pass diff cleanup).
- Solar flare (`cloud/type_rain/solar_flare/`): 5 files, 1 956 LOC —
  the derivation essay and module map (mod.rs, the five laws), the
  arcade physics (loops.rs: the magnetic carpet, the flux ladder,
  the flare gate, the granulation surface), the drop state
  (drops.rs: riding/ejecta modes, the kinetic ladder), the
  orchestration (solar_flare.rs: pool, spawn tournament, the law 2-4
  advance), the draw pass (draw.rs: surface, arcs, rain, ejecta,
  diff cleanup).
- Hot-path dependencies audited beyond the two directories: the
  rain_at.rs dispatch arms (spawn/advance/draw + MonolithCleanup
  wiring), the monolith_helpers shared surface (`color_for_level`,
  `bold_for_level`, `clear_cell`, `pick_pool_char`), the style_rain
  constants (the VORTEX_*/SOLAR_* dial blocks), and DrawCtx.
- Method: full read of every spawn/advance/draw hot path in scope;
  panic/unwrap/expect scan (zero hits in all 7 files); a full
  transcendental inventory (rg over sin/cos/exp/sqrt: five sites —
  two shipped below, three irreducible by design); test coverage
  count (32 style-contract tests — vortex 10, solar flare 22 across
  core+loops — plus the 26 scene tests pinning both scenes' cycle
  positions); baseline 10 s benches for the record.

## The audit verdict: both styles are at peak

Five dimensions checked, each clean. The re-audit lens (the pass
that found stage 1's two latent hoists) found two explicit-semantics
items this time — both shipped below — and nothing beyond them:

1. **Precision / math hygiene.** Both styles run the family clock
   contract: dt saturates via `saturating_duration_since`, clamps
   against max_sim_delta, and eases by resume_blend (vortex on the
   global last_step; solar additionally scales to sim-time by
   chars_per_sec × SOLAR_SIM_TIME_PER_CPS so trajectory shapes
   survive the speed keys). Vortex's arm phase wraps at 128 turns
   via rem_euclid (f32 precision kept); every solar state variable
   is bounded by direct construction — flux decays and hard-clamps,
   spans/heights/drift clamp inside their bands, the along-arc speed
   is a closed form bounded by sqrt(v0² + 2·LEG_G·H_CAP), and the
   landing test is a state test (s motion strictly monotone — no
   tunneling at any dt, any frame rate).
2. **Hot-loop economics.** Viewport geometry is hoisted per frame on
   both sides (vortex draw: cx/cy/max_rx/max_ry; solar advance:
   surface_top/cols_f; solar draw: per-arc fade/base/steps/shimmer
   before the filament loop). Per-mote/per-drop work is the
   irreducible physics: Kepler omega with the divisor floor, the
   law-2 closed-form speed plus metric, the arc parabola reads. The
   granulation walk's one RNG sample per column per frame is the
   visual itself, not overhead. No allocation in any steady-state
   loop (the landings/splashes collectors are lazily allocated only
   on event frames). Free-slot search is the rotating-cursor O(1)
   family pattern; both draws end in the three-pass generation-tag
   diff cleanup.
3. **Stability / LTS.** Zero unwrap/expect/panic in 2 535 LOC.
   Degenerate viewports return early everywhere; resize rebuilds
   pool and arcade; a straggler rider whose loop vanished in a
   viewport rebuild is retired through `loops.get()` (no panic on
   the race); style transitions reset state both ways (pinned by
   tests).
4. **Harmony / family contracts.** Both styles follow the family
   patterns: params-struct call signatures (VortexSpawnParams /
   VortexStep / SolarCellPaint — the hunter-25 school), the
   spawn-remainder fractional budget with SPAWN_REMAINDER_CAP, the
   monolith brightness ladder reused through color_for_level /
   bold_for_level, the MonolithCleanup diff engine, the lane pool
   (one mote/drop per column), and the centralized tuning constants
   in style_rain.rs.
5. **Test coverage.** 32 contract tests across the two styles
   (vortex: density target, monotonic inward convergence, core
   absorption, drawn-cell bounds, omega bound, style transitions,
   palette adoption; solar: law 1/3/4/5 boundedness, rider
   acceleration and speed bound, landings charging the flux, the
   flare cycle end-to-end, diff-cleanup residue, pause freeze,
   speed keys, style round-trip, sustained boundedness), plus the
   scene suite pinning both cycle positions.

## The shipped changes: two explicit-semantics hoists and two named ladders

1. **Vortex — sin_cos fusion (vortex.rs, draw):** the head
   projection evaluated `m.angle.cos()` and `m.angle.sin()` as two
   separate libm calls per mote per frame; they now fuse into one
   `m.angle.sin_cos()` evaluation (the stage-1 RollFrame precedent —
   same values, half the trig call count per mote).
2. **Solar flare — the flux-decay hoist (loops.rs, advance):** law
   3a's decay factor `(-SOLAR_FLUX_DECAY * dt).exp()` was evaluated
   once per loop per frame although dt is frame-invariant; it is now
   evaluated once per frame and shared by every loop (the stage-1
   StepFactors precedent — value identical, ≤ 8 → 1 exp() calls per
   frame).
3. **Vortex — the luminance-zone ladder named:** `level_for_radius`
   read its Ghost/Mid boundaries as inline 0.66 / 0.33 while the
   third boundary (VORTEX_CORE_R) was already a named constant;
   VORTEX_ZONE_RIM (0.66) and VORTEX_ZONE_MID (0.33) now live in the
   style_rain vortex block, completing the named ladder. Values
   unchanged, codegen identical (const-folded compares).
4. **Solar flare — the granulation ladder named:** `granule_level`
   read its rungs as inline 0.80 / 0.50 while its sibling ladder
   `loop_level` reads fully named SOLAR_FLUX_LEVEL_* rungs;
   SOLAR_GRANULE_LEVEL_MID (0.50) and SOLAR_GRANULE_LEVEL_HOT
   (0.80) now sit beside SOLAR_GRANULE_MIN/MAX/STEP in style_rain.
   Values unchanged, codegen identical.

## Considered and skipped (the over-engineering guard, quantified)

1. `color_for_level` recomputes the level-to-palette-index ladder
   per drawn cell (stage 2's cross-family finding, re-verified from
   this stage's perspective): vortex draws ~54 active motes × up to
   5 head+trail cells ≈ up to ~270 calls per frame at 80x24; solar
   flare draws ~2×cols surface cells + ~2×loop-count footpoints +
   ~8 loops × ~20 arc filament cells + the rain ≈ ~350-400 calls
   per frame. Each pays three strength-reduced integer divisions on
   a loop-invariant `last` — sub-microsecond, and LLVM CSEs them
   after inlining (the LICM argument stage 1's A/B proved
   empirically). The signature is shared by six families (monolith,
   lorenz, vortex, dragon, physarum, flux — solar inherits through
   the same helper), so a ladder hoist is a cross-family blast
   radius far beyond the gain. Skipped, per the stage-2 verdict.
2. `width_band()` / `height_band()` are recomputed inside
   `CoronaArcade::advance` although viewport-invariant — but they
   are called once per frame (~10 float ops ≈ 5 ns) against the
   per-column granulation walk (~cols RNG samples). Hoisting needs
   four new struct fields synced in reset. Skipped.
3. `draw_vortex_cell` / `draw_solar_cell` re-check bounds their
   callers already proved (head cells checked in the dispatch, trail
   cells checked, arc/surface cells clamped) — 2 compares per drawn
   cell × ~250-400 cells = sub-microsecond; the family's
   defensive-depth pattern (monolith's draw does the same). Skipped.
4. Pass A's `surface_top + 1 < ctx.lines` is loop-invariant across
   the column loop — ~80 compares ≈ 20 ns; hoisting it means
   restructuring the second surface line out of the loop. Skipped.
5. The trail-depth `.min(N)` literal caps (vortex `.min(4)`, solar
   `.min(3)`) are a 9-site cross-family idiom (lorenz, dna_helix,
   black_hole ×2, aeolian, murmuration, flux share it), and both
   caps are output-invariant by construction: `step_down_level`
   saturates at depth ≥ 2, vortex depth ≤ 4 = its cap, solar depth
   ≤ SOLAR_TRAIL_LEN (2) < 3. Promoting only this stage's sites
   would diverge from the family idiom stage 1 left in place in
   black_hole. Skipped as family harmony.
6. The `landings` / `splashes` collectors construct fresh `Vec::new()`
   per advance — `Vec::new()` allocates nothing until the first
   push, and pushes arrive only on landing/splash event frames (a
   trickle against the drop population). No steady-state
   allocation. Skipped.
7. Vortex `current_cells` has no `reserve()` before the draw loop —
   capacity persists after the first frames (stage 1's quasar item
   5). Skipped.
8. The Kepler omega division per mote (K / r_safe) is irreducible
   per-mote physics (r varies); rewriting as a reciprocal multiply
   would change f32 rounding. Same verdict for the arc metric's
   per-drop sqrt and the law-2 speed's per-drop sqrt (both depend
   on s). Skipped.
9. Both styles' `drawn_gen_counter` uses plain `wrapping_add`
   without monolith's zero-fill-on-wrap guard — a wrap collision
   needs a tag written exactly 2³² frames earlier to equal the
   fresh generation AND sit in the one-frame-deep previous_cells,
   whose tags are always gen−1 ≠ gen by u32 arithmetic; the horizon
   is 2.3 years at 60 fps. Skipped (stage 1 item 4's reasoning,
   re-applied).
10. Draw Pass B's per-step `i as f32 / steps` (numerator-varying)
    and `2.0 / steps` (arc-invariant, LLVM-hoisted) divisions — the
    varying one is irreducible without reciprocal-multiply, which
    changes f32 values. Skipped.
11. The spawn-path inline roll bands (vortex `0.85 + roll×0.30`
    spin / `0.80 + roll×0.45` fall; solar ejecta life
    `0.75 + roll×0.5`) — cold spawn-rate paths with the ranges
    documented on the struct fields they seed. Skipped.

## Baseline measurements (10 s, this audit's build, 80x24 default)

| scene | avg fps | sim ms | dirty cells/frame | density gini | frame entropy (bits) |
|---|---|---|---|---|---|
| vortex | 80 710 | 0.0075 | 137.2 | 0.497 | 5.624 |
| solar_flare | 36 597 | 0.0169 | 248.8 | 0.323 | 6.042 |

Both styles run one to two orders of magnitude beyond any terminal's
display rate (vortex ~80K fps, solar flare ~37K fps with the diff
engine emitting ~249 cells per frame) — the frame budget is nowhere
near pressure, the quantitative form of the peak verdict.

## A/B (10 s benches, this sandbox, 2 runs per side, run-averaged)

| scene | metric | baseline | after | delta |
|---|---|---|---|---|
| vortex | avg fps | 80 710.33 | 80 826.34 | +0.14 % |
| vortex | sim ms | 0.007506 | 0.007483 | −0.31 % |
| vortex | dirty cells | 137.18 | 136.94 | −0.17 % |
| vortex | density gini | 0.4968 | 0.4968 | 0.00 % |
| vortex | entropy bits | 5.6238 | 5.6232 | −0.01 % |
| solar_flare | avg fps | 36 597.12 | 36 511.49 | −0.23 % |
| solar_flare | sim ms | 0.016853 | 0.016844 | −0.05 % |
| solar_flare | dirty cells | 248.76 | 248.95 | +0.08 % |
| solar_flare | density gini | 0.3234 | 0.3248 | +0.43 % |
| solar_flare | entropy bits | 6.0418 | 6.0364 | −0.09 % |

Run-level spread (the noise yardstick): 0.66 % (vortex baseline) /
0.43 % (vortex after) / 0.01 % (solar baseline) / 0.95 % (solar
after).

The vortex deltas — the scene carrying the sin_cos fusion — sit
inside the run spread, with the density gini identical to four
decimals (0.4968): the drawn-cell distribution is unchanged, which
is the direct visual-neutrality evidence for the fusion. The solar
flare deltas deserve the honest reading: the baseline pair was
anomalously tight (0.01 % spread — a lucky cache-warm pair), while
the after pair spans 0.95 % (36 338.92 vs 36 684.06 fps); the
−0.23 % run-averaged fps delta and the +0.43 % gini delta sit inside
the after block's own run-to-run variance (its two gini readings
span 0.3221 to 0.3276). The bench process instances sample
different stochastic trajectories per run (wind re-rolls, flare
clock, granulation walk, spawn tournament timing), the same
run-to-run variance stage 2 documented — and the only solar code
change multiplies every loop's flux by a bit-identical factor, so no
behavioral channel exists for a real delta. Verdict: performance-
and visual-neutral.

## Stage gate

Stage 3 complete: the audit verdict is peak for both styles with
measured evidence, the two explicit-semantics hoists and two named
ladders ship zero-behavior-change and A/B-verify neutral, and the
full gate suite is green (fmt, clippy --all-targets, build.sh
check-all -q, gate-keepers.sh 10/10, 2 679 tests passed + 2 ignored
in 56.1 s; edited files: vortex.rs 570 LOC, loops.rs 565 LOC, both
under the 800 cap; style_rain.rs 2 749 LOC is the LOC_EXEMPT
tuning-constants file, +15 lines within its charter). Full A/B JSONs
in `benchmark/bench-labs/night_lts1_stage3/`.

Proceeding to stage 4 under the owner's continuous-approval
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
