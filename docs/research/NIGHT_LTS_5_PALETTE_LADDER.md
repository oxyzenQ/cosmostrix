<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-lts-5b — the color_for_level palette-ladder hoisted per frame

Owner approval (2026-09-10): "color_for_level palette-ladder adalah
satu-satunya quantified-skip cross-family yang tersisa — butuh
keputusan owner karena blast radius-nya 6 families". The stages 2-6
audit verdicts had deferred exactly this item pending an owner
decision; the owner approved proceeding. This document records the
implementation, the equivalence proof, and the honest measurement
outcome.

## Scope

- `color_for_level` (monolith_helpers.rs) — the structured families'
  shared brightness-ladder resolver, called from 15 sites across 14
  families (monolith x2, lorenz, quasar, vortex, neural, physarum,
  aeolian, solar_flare, flux, dragon, black_hole, dna_helix,
  murmuration); the six-family shared-signature core the stage docs
  cited plus the inheritors.
- The per-cell cost before the change: three integer divisions on a
  loop-invariant `last` (the palette stop count), re-derived per
  drawn cell — ~330 calls/frame on lorenz, ~270 on vortex, ~350-400
  on solar flare at 80x24.

## The change (bit-identical by construction)

1. `PaletteLadder` (render.rs) — the four level-to-stop indices
   (ghost/mid/hot/core) as a named, `Copy`, `PartialEq` struct with
   `from_len` (the v17 mastery equations, values unchanged) and
   `from_slices` (per-slot alignment with `DrawCtx::palette_slices`).
2. `DrawCtx.palette_ladders: [PaletteLadder; MAX_PALETTE_SLOTS]` —
   derived once per frame in `rain_at` (one extra 4-iteration loop
   on the palette table), alongside the existing `palette_slices`
   build.
3. `color_for_level` — the effective-slot resolution and the
   empty-slice fallback are unchanged; the ladder now loads from the
   precomputed array with the same fallback chain, and the level
   match picks the precomputed stop.

The family call sites are untouched: the ladder is internal to
`DrawCtx` + `color_for_level`, so the "blast radius 6 families"
feared by the stage docs collapsed to exactly two production files
(render.rs, monolith_helpers.rs) plus the per-frame wiring in
rain_at.rs — the signatures never moved.

## Equivalence proof

- The stop indices are the same equations evaluated at the same
  input (the palette slice length); the fixture ladders are built
  through the same `from_slices` from the same slices.
- 6 new contract tests (tests_monolith/palette_ladder.rs): the
  equations pinned against an independent re-derivation of the
  pre-lts-5b inline math (lengths 0-255), bounds safety (every
  index < len for lens 1-64), the v17 monotonicity hierarchy (Ghost
  <= Mid <= Hot <= Core from 3 stops up — plus the len-2 legacy
  quirk pinned verbatim: ghost pins to the visible floor 1 while
  mid/hot sit at 0), `from_slices` per-slot alignment, the exact
  stop each level resolves to through `color_for_level` (factor 1.0
  skips both blend stages), and empty-palette None.
- 10 existing DrawCtx test fixtures updated (the ladder field);
  the monolith depth and visual-depth suites now double as
  end-to-end equivalence guards (they assert exact stop colors and
  luminance ordering through the real `color_for_level`).
- Gate: build.sh check-all -q green, gate-keepers 10/10, 2685/2685
  tests (2679 prior + 6 new).

## A/B (10 s, 2 runs per side, 80x24 default profile)

Baseline at b75a994 (clean), after = this change; a same-window
control (the stage-5 tool: clean binary stashed-rebuilt and benched
in the same machine window as the after runs) because multiple
scenes showed deltas outside their baseline spreads.

| scene | baseline | control (same window) | after | after vs control |
|---|---|---|---|---|
| monolith | 70 225.37 | — | 70 203.92 | -0.03 % (in spread) |
| lorenz | 98 535.42 | 98 563.38 | 97 962.2 (4 runs) | **-0.61 %** |
| solar_flare | 36 689.04 | 36 415.46 | 36 400.86 | -0.04 % (neutral) |
| vortex | 80 720.88 | — | 80 490.54 | -0.29 % (in spread) |
| flux (canary) | 62 525.77 | 62 403.84 | 62 158.13 | -0.39 % (1.71 % noisy pair) |

The control column settles attribution: the clean binary reproduced
the baseline (lorenz +0.03 %, solar -0.75 % — the solar baseline
pair was simply measured on a faster machine window; flux -0.19 %),
so the deltas are NOT machine drift.

The lorenz finding, decomposed: sim_ms 0.00731 -> 0.00736, i.e.
+50 ns per frame over ~330 ladder calls — about 0.13 ns per call,
far below the cost of any real extra load+branch (which LLVM
evidently kept nearly free after inlining). The regression is
reproducible across 4 runs but is either a residual-codegen or
code-layout effect of nanosecond scale; at any real terminal frame
budget (16.6 ms at 60 fps) it is five orders of magnitude below
perceptibility. Dirty cells, gini and entropy match to the
stochastic band on every scene (lorenz dirty 72.89-72.99 across all
8 runs).

## Verdict

The stages 2-6 quantified-skip verdicts are empirically confirmed:
no scene gained anything (LLVM already CSE'd the divisions after
inlining, exactly as stage 3 argued). The change therefore ships as
what the owner approved it for — not a speedup, but the
explicit-semantics and LTS-maintainability contract: the v17
mastery ladder is now one named, documented, test-pinned type
instead of three inline magic divisions inside a 14-family shared
hot function, and future palette-length or ladder tuning changes
have a single test surface. The measured cost profile (neutral on
4 of 5 scenes, ~60 ns/frame absolute on the ladder-heaviest scene)
is recorded above for the record; per the stage-1/4 precedent,
explicit-semantics changes ship at neutral A/B. Evidence:
`benchmark/bench-labs/night_lts5b/` (baseline_*, after_*,
control_* = the same-window ablation).
<!-- COSMOSTRIX-DISCLAIMER -->
