<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-lts-1 stage 4 — the master depth audit: physarum + lorenz

Owner directive 2026-09-10 (stage 2 approval, continuous through
stage 7): the staged master depth audit proceeds family by family;
stage 4 owns the stigmergic slime mold and the strange attractor.
This document is the stage-4 report.

## Scope and method

- Physarum (`cloud/type_rain/physarum/`): 3 files, 916 LOC — the
  stigmergic core (physarum.rs 789: particle pool, spawn, the
  sense/decide/move/deposit advance, the toroidal trail field, the
  head draw with the three-pass diff cleanup, the test hooks) and
  the extracted helpers (physarum_helpers.rs: `sample_trail` with
  the wraparound modulo, `level_for_trail`, `sample_random`,
  `draw_physarum_cell`).
- Lorenz (`cloud/type_rain/lorenz/`): 2 files, 700 LOC — the
  single-file strange-attractor system (lorenz.rs 692: mote pool,
  lobe-alternating spawn near the equilibria, the RK4 integration
  core, the head-plus-comet-trail draw, the diff cleanup).
- Hot-path dependencies audited beyond the two directories: the
  rain_at.rs dispatch arms (spawn/advance/draw + MonolithCleanup
  wiring for both styles), the monolith_helpers shared surface
  (`color_for_level`, `bold_for_level`, `clear_cell`,
  `pick_pool_char`), and the style_rain.rs dial blocks (all 37
  PHYSARUM_*/LORENZ_* tuning constants are named — no inline dials).
- Method: full read of every spawn/advance/draw hot path in scope;
  panic/unwrap/expect scan (zero hits in all 5 files); a full
  transcendental inventory (rg over sin/cos/exp/powf/sqrt: physarum
  six sites — three of them the shipped fusions below, one
  per-frame powf decay that is the documented contract, one
  tie-break-only sin; lorenz zero sites — the RK4 body is pure
  arithmetic and `level_for_z` is a compare ladder); test coverage
  count (25 style-contract tests — physarum 15, lorenz 10 — plus
  the scene suite pinning both cycle positions); baseline 10 s
  benches for the record; and a machine-code-level A/B proof
  (see the A/B section) unique to this stage.

## The audit verdict: both styles are at peak

Five dimensions checked, each clean. The re-audit lens (the pass
that found stage 1's hoists and stage 3's two explicit-semantics
items) found the same class of item here — three separate
sin/cos pairs at one angle — and nothing beyond them:

1. **Precision / math hygiene.** Both styles run the family clock
   contract: dt saturates via `saturating_duration_since`, clamps
   against max_sim_delta, and eases by resume_blend on the single
   global step clock. The RK4 integration dt is
   chars_per_sec × LORENZ_DT_PER_CPS × dt_wall — 3.2e-4 at the
   bench config (speed 24, 60 fps), two orders below the
   literature stability bound (dt < 0.01), pinned by
   `lorenz_rk4_step_bounded`. Physarum carries two LTS contracts
   of its own: the amortized heading wrap past 64 turns (rem_euclid
   — trig-equivalent, branch almost never taken, pinned by test)
   and the rate-independent trail decay
   (`PHYSARUM_TRAIL_DECAY.powf(dt × 60)` — the equilibrium grades
   identically at 30 and 144 Hz, pinned by
   `physarum_trail_decay_is_frame_rate_independent`). Positions
   wrap toroidally with an explicit in-range fast path; trail
   sampling uses the identical wrap so sensing and motion agree
   across edges.
2. **Hot-loop economics.** Per-frame invariants are already hoisted
   on both sides: lorenz lifts sigma/rho/beta and
   dt_lorenz_base before the mote loop and the whole projection
   (col_half/line_half/cx/cy/x_scale/y_scale) before the draw
   loop; physarum lifts step_dist, the sensor-angle trig, and the
   viewport floats before the particle loop (NIGHT-hunter-10's
   angle-addition ladder already replaced six per-particle trig
   calls with two). The per-particle/per-mote remainder is the
   irreducible physics: four RK4 derivative evaluations per mote
   (the documented stability regime — untouchable by charter),
   three trail-field sensor samples plus deposit per particle,
   the rate-independent decay sweep (O(cells), the contract
   itself). Nothing allocates in any steady-state loop — the
   three diff arrays and the trail field are clear/reuse only,
   and the trail field reallocates solely on a dimension change.
3. **Stability / LTS.** Zero unwrap/expect/panic in 1 616 LOC.
   Degenerate viewports return early in every entry point; the
   trail field lazily (re)sizes when the advance viewport changes
   and re-syncs defensively at draw; a mote pool rebuilt on
   resize resets the draw history so the diff never sees stale
   geometry; style transitions reset state both ways (pinned by
   tests).
4. **Harmony / family contracts.** Both styles are textbook family
   members: params-struct call signatures (LorenzSpawnParams /
   PhysarumSpawnParams mirror VortexSpawnParams — the hunter-25
   school), the spawn-remainder fractional budget with
   SPAWN_REMAINDER_CAP, the rotating-cursor amortized O(1)
   free-slot scan, the monolith brightness ladder reused through
   color_for_level / bold_for_level, the MonolithCleanup
   three-pass generation-tag diff, and the lane pool (one
   mote/particle per column).
5. **Test coverage.** 25 contract tests across the two styles
   (physarum: density target, trail accumulation, viewport bounds,
   lifetime absorption, drawn-cell bounds, sensor-distance band,
   count routing, style transitions, live-frame smoke, palette
   adoption, network emergence, rate-independent decay, sensor
   steering signs through the angle-addition ladder, heading wrap;
   lorenz: density target, attractor bounds, absorption,
   drawn-cell bounds, RK4 dt bound, canonical parameters
   (compile-time), count routing, style transitions, live-frame
   smoke, palette adoption), plus the scene suite pinning both
   cycle positions.

## The shipped changes: three sin_cos fusions (physarum only)

The advance pass evaluated the heading trig as separate libm
calls at three sites, each a pair at one angle — the exact
stage-1 RollFrame / stage-3 vortex pattern:

1. **Sensor-angle pair (per frame):** `sensor_angle.cos()` +
   `sensor_angle.sin()` — one pair per advance call — now one
   `sensor_angle.sin_cos()` evaluation.
2. **Sensor pair (per particle):** `p.heading.cos()` +
   `p.heading.sin()` feeding the angle-addition ladder and the
   front sensor — now one `p.heading.sin_cos()` evaluation.
3. **Move pair (per particle, post-turn):** the move step's
   `p.heading.cos()` + `p.heading.sin()` — a different angle from
   the sensor pair because the heading was just steered — now one
   `p.heading.sin_cos()` evaluation.

Source-level call count per frame at the bench config
(42 active particles): 4N + 2 = 170 separate trig evaluations →
2N + 1 = 85 fused evaluations. Values are bit-identical —
`sin_cos` returns the same (sin, cos) pair the separate calls
return — and no signature changed, so all 25 style tests and
their call sites are untouched.

The decisive follow-up (new to this stage): a binary-level
control. The baseline source and the fused source were each
release-built (fat LTO, same toolchain) and their `.text`
sections compared: byte-identical (2 344 208 bytes, md5
3d92aad3ea318cf23c91bf52323fcfdb on both sides; whole-file
hashes differ only in non-code metadata). The call-site census
agrees: both binaries import and call sincosf at 29 sites, sinf
at 16, cosf at 2 — LLVM's libcall simplifier already fused the
separate sin/cos pairs at both physarum runtime sites in the
baseline (the per-frame sensor-angle pair is const-folded away
entirely — PHYSARUM_SENSOR_ANGLE is a compile-time constant).
So the fusion ships as explicit, optimizer-independent semantics
— the machine code was already optimal, and the source now
states that fact instead of depending on the compiler to
discover it (the stage-1 lesson, this time proven at the
instruction level rather than inferred from timing). Lorenz
ships nothing: its hot path has no trig at all.

## Considered and skipped (the over-engineering guard, quantified)

1. `color_for_level` recomputes the level-to-palette-index ladder
   per drawn cell (the stage-2 cross-family finding, re-verified
   from this stage's perspective): lorenz draws ~55 motes × up to
   6 head+trail cells ≈ up to ~330 calls per frame at 80x24;
   physarum draws only head cells ≈ ~42 calls per frame. Each
   pays three strength-reduced integer divisions on a
   loop-invariant `last` — sub-microsecond, and LLVM CSEs the
   ladder after inlining (the stage-1 LICM argument). The
   signature is shared by six families, so a hoist is a
   cross-family blast radius far beyond the gain. Skipped, per
   the stage-2/3 verdict.
2. The trail-depth `.min(4)` literal cap in the lorenz comet
   loop — stage 4 owns lorenz, so the stage-3 output-invariance
   claim was re-derived from the actual code: trail_len ≤
   LORENZ_TRAIL_LEN (5), depth = trail_len − t ∈ [1, 5], the cap
   folds it to [1, 4], and `step_down_level` saturates at depth ≥
   2 (every rung past the second lands on the same level), so
   ANY cap ≥ 2 — including removing the cap entirely — emits
   identical cells. The invariant is exactly as strong as stage 3
   claimed; not weaker. The cap is a 9-site family idiom
   (lorenz, dna_helix, black_hole ×2, aeolian, murmuration, flux,
   vortex, solar), ~275 integer min ops per frame ≈ tens of
   nanoseconds. Promoting only this stage's site would diverge
   from the family. Skipped as family harmony.
3. The RK4 body's repeated subexpressions — `0.5 * dt` inside
   the six k2/k3 argument slots and `dt / 6.0` in the three state
   updates — are per-mote invariants (dt = base × pace) that LLVM
   CSEs within the same basic block, guaranteed (identical
   expressions, no intervening side effects). Binding them to
   locals would be bit-identical but pure churn inside the
   documented stability regime whose textbook coefficient form
   (0.5, 2, 1/6) IS the documentation. Skipped.
4. Eliminating the physarum move-trig by rotating the sensor
   pair (cos(h+t) = cos_h·cos_t − sin_h·sin_t) — the turn varies
   per particle per frame, so the rotation needs its own trig
   anyway, and any identity-based rewrite changes f32 rounding
   (a bit-level trajectory change). Forbidden by the
   zero-behavior-change charter. Skipped.
5. `sample_trail`'s `cols as f32` / `lines as f32` conversions —
   three calls × two conversions per particle ≈ six sitofp per
   particle, loop-invariant across the entire particle loop;
   `#[inline]` plus LLVM CSE hoists them for free. Threading the
   floats through the signature would touch the helper and its
   test-hook callers for zero emitted-code gain. Skipped.
6. `draw_lorenz_cell` / `draw_physarum_cell` re-check bounds
   their callers already proved (heads checked in the dispatch,
   trail cells checked, heads rounded in range) — 2 compares per
   drawn cell × ~42/~330 cells = sub-microsecond; the family's
   defensive-depth pattern (monolith, vortex, solar all do the
   same). Skipped.
7. Physarum's head-brightness read `idx < trail_field.len()` —
   provably true after the top-of-draw dimension re-sync; the
   compare guards only the defensive-failure path, ~42 per
   frame ≈ nanoseconds. Skipped.
8. Both styles' `drawn_gen_counter` uses plain `wrapping_add`
   without monolith's zero-fill-on-wrap guard (the stage-3 item 9
   asymmetry, shared). Every cell in previous_cells was tagged
   gen−1 by pass 2 of the immediately preceding draw, so a wrap
   collision would require gen−1 == gen under u32 arithmetic —
   impossible by construction; the residual horizon is 2³² frames
   ≈ 2.3 years at 60 fps even without that argument. Skipped
   (stage-1 item 4's reasoning, re-applied).
9. The per-particle pace products (`dt_p = dt × pace`,
   `dist = step_dist × pace`) are per-particle by construction
   (pace varies); deriving dist from dt_p re-associates the f32
   multiply chain and changes trajectory bits. Skipped.
10. `push_trail`'s shift-left (four (u16, u16) copies per mote
    per frame, ~440 u32 moves total) — a ring buffer would avoid
    the copies but reverse the iteration order and add index
    math for the same visual; the diff arrays' capacity persists
    either way (stage 1's quasar item 5 precedent). Skipped.
11. Spawn-path inline roll bands (pace and lifetime
    `0.85 + roll × 0.30`, lorenz perturbation
    `±LORENZ_SPAWN_PERTURB`, physarum heading `roll × TAU`) —
    cold paths (well under one spawn per frame at display rate)
    with the ranges documented on the fields they seed (the
    stage-3 item 11 idiom). Skipped.

## Baseline measurements (10 s, this audit's build, 80x24 default)

| scene | avg fps | sim ms | dirty cells/frame | density gini | frame entropy (bits) |
|---|---|---|---|---|---|
| physarum | 106 139 | 0.0071 | 65.0 | 0.626 | 5.137 |
| lorenz | 98 160 | 0.0073 | 73.0 | 0.618 | 5.177 |

Both styles render five to six orders of magnitude beyond any
terminal's display rate — physarum at ~106K fps with the diff
engine emitting ~65 cells per frame, lorenz at ~98K fps emitting
~73 cells per frame. The frame budget is nowhere near pressure —
the quantitative form of the peak verdict.

## A/B (10 s benches, this sandbox, 2 runs per side, run-averaged)

| scene | metric | baseline | after | delta |
|---|---|---|---|---|
| physarum | avg fps | 106 138.74 | 106 016.71 | −0.12 % |
| physarum | sim ms | 0.007075 | 0.007096 | +0.30 % |
| physarum | dirty cells | 64.99 | 64.99 | 0.00 % |
| physarum | density gini | 0.6264 | 0.6272 | +0.13 % |
| physarum | entropy bits | 5.1373 | 5.1345 | −0.05 % |
| lorenz | avg fps | 98 159.93 | 98 347.09 | +0.19 % |
| lorenz | sim ms | 0.007325 | 0.007332 | +0.10 % |
| lorenz | dirty cells | 72.95 | 72.96 | +0.02 % |
| lorenz | density gini | 0.6178 | 0.6181 | +0.05 % |
| lorenz | entropy bits | 5.1766 | 5.1760 | −0.01 % |

Run-level spread (the noise yardstick): 0.40 % (physarum
baseline) / 0.46 % (physarum after) / 0.19 % (lorenz baseline) /
0.00 % (lorenz after — an anomalously tight pair, 98 345.42 vs
98 348.77; the stage-3 lucky-pair caveat applies, and per that
lesson the 0.5-1 % band remains the real yardstick).

The honest attribution is stronger this stage than for any
predecessor: the two release binaries' `.text` sections are
byte-identical (proven above), so there is no code channel for
any delta on either scene — the physarum −0.12 % fps and +0.30 %
sim, and the untouched lorenz path's +0.19 % fps, are machine-
state drift by construction. The per-run readings agree: the
after physarum sim pair (0.007112 / 0.007080) straddles the
baseline pair (0.007069 / 0.007082), and the dirty-cell stream
is deterministic under the fixed bench seed — 64.98-65.00 across
all four physarum runs and 72.89-73.03 across all four lorenz
runs — the drawn-cell distribution is unchanged, which is the
direct visual-neutrality evidence. Gini and entropy wobble at
the third decimal with the stochastic sampling of different
particle states per run (the same run-to-run variance stages 2
and 3 documented). Verdict: performance- and visual-neutral,
with machine-code-level proof.

## Stage gate

Stage 4 complete: the audit verdict is peak for both styles with
measured evidence, the three sin_cos fusions ship zero-behavior-
change and are proven codegen-identical at the instruction level,
and the full gate suite is green (fmt, clippy --all-targets,
build.sh check-all -q under 2 minutes, gate-keepers.sh 10/10,
2 679 tests passed + 2 ignored in 55.9 s; edited file:
physarum.rs 794 LOC, under the 800 cap; lorenz untouched). Full
A/B JSONs in `benchmark/bench-labs/night_lts1_stage4/`.

Proceeding to stage 5 under the owner's continuous-approval
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
