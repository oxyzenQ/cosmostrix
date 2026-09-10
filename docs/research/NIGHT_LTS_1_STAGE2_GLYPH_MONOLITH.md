<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-lts-1 stage 2 — the master depth audit: glyph + monolith

Owner directive 2026-09-10 (stage 2 approval): the staged master depth
audit continues with glyph + monolith, with stages 2 through 7
approved to run continuously to completion. This document is the
stage-2 report.

## Scope and method

- Glyph (`cloud/type_rain/glyph/`): 3 files, 496 LOC — the module
  wiring (mod.rs), the droplet spawn decision + spec construction
  (spawn_logic.rs: `spawn_droplets` + `build_droplet_spec`), the pool
  lifecycle trio (pool_lifecycle.rs: `recalc_droplets_per_sec`,
  `update_droplet_speeds`, `ensure_glyph_pool_and_warm_start`).
- Monolith (`cloud/type_rain/monolith/`): 4 files, 1 200 LOC — the
  state machine (monolith.rs: MonolithRain spawn/advance/draw with the
  three-pass generation-tag diff), the 19 free helpers
  (monolith_helpers.rs: activate/build_segments/draw_spine/
  draw_segments/color_for_level/bold_for_level/clear_cell), the
  charset-aware glyph mapping (monolith_glyphs.rs).
- Hot-path dependencies audited beyond the two directories:
  `cloud/cinematic.rs` (motion/breathing/hero-pulse/spine-cadence
  factors), `cloud/render.rs` (`DrawCtx::get_char`, `edge_fade`,
  `color_uses_previous_palette`), and the Glyph draw dispatch in
  `cloud/rain_at.rs` (the per-droplet loop).
- Method: full read of every spawn/advance/draw hot path in scope;
  panic/unwrap/expect scan (zero hits in production paths of all 7
  files — the only assertions are debug-gated);
  constant-centralization scan; test coverage count (26 monolith
  contract tests across core/depth/residue/transitions/charset, 6
  droplet-pool tests, 43 scene-cycle and entry tests pinning the
  Glyph warm-start/cycle contracts); baseline 10 s benches for the
  record.

## The audit verdict: both styles are at peak

Five dimensions checked, each clean — and unlike stage 1, the
re-audit lens (the pass that found the two latent stage-1 hoists)
found nothing further here. These two modules are the two most
hunted code paths in the project (the historical A/B labs
night_hunter20/21/22/23/25 all bench the cinematic+monolith pair),
and it shows:

1. **Precision / math hygiene.** Spawn budgets clamp against
   SPAWN_REMAINDER_CAP in both families; glyph dt saturates via
   `saturating_duration_since` and clamps against max_sim_delta;
   monolith advance clamps elapsed identically and multiplies by
   resume_blend (the anti-teleport contract). Phase inputs to the
   cinematic factors are clamped at use (`phase.clamp(0.0, 1.0)` in
   the cadence; triangle_wave01 wraps via rem_euclid so f32 phase
   precision is preserved). All span/length rolls are clamped before
   the u8/u16 casts.
2. **Hot-loop economics.** Glyph spawn already carries the three
   historical wins: the v30 Hinnant time hoist (one anchor read, zero
   syscalls), the O(1) free-list pop (hunter-14, with the warm-start
   pop fix keeping the list exact), and the spawn-remainder cap.
   Monolith draw already carries: the three-pass drawn-cell diff with
   generation tags (60-80% of clears skipped), the SpineTone hoist
   (breath + cadence computed once per stream, shared by spine and
   segments), the F8 hero-pulse hoist per segment, the F9 co-sized
   array single bounds check, the egg-13/18 LUT and direct-index
   patterns, and the incremental active_count (hunter-14 removed the
   last redundant O(cols) scan). Per-cell character resolution is a
   power-of-2 bitmask; edge fade is a LUT. Nothing allocates in any
   hot loop.
3. **Stability / LTS.** Zero unwrap/expect/panic in 1 696 LOC. The
   drawn_gen counter has the u32 wrap guard (zero-fill on wrap, 0
   stays the "never drawn" sentinel); degenerate viewports are guarded
   (zero cols/lines return early in every entry point); warm-start
   keeps the free-list contract exact under pool pressure (the
   hunter-14 pop fix); pool capacity persists across style switches
   (reset reuses the existing allocation when the lane count is
   unchanged).
4. **Harmony / family contracts.** Glyph follows the spawn-remainder
   fractional budget family pattern and the parallax ladder
   (speed/length/brightness/density tables all keyed by layer).
   Monolith is the reference implementation of the structured-family
   contracts: the params-struct call signatures, the monolith
   brightness ladder re-exported for lorenz/vortex/dragon/physarum/
   flux, the drawn-cell diff cleanup shared via MonolithCleanup, and
   the centralized tuning constants in central_control_rains.rs.
5. **Test coverage.** 75 tests touching the two styles' contracts
   (26 monolith + 6 droplet pool + 43 scene cycle/entry), including
   the fresh-entry sparse seeding, the free-list exactness, the
   generation-tag wrap guard, and the spine cadence contract.

## The one shipped change: MONOLITH_SPINE_GHOST_TONE

The re-audit found exactly one hygiene item: `draw_spine_cell`'s
inline `0.72` — the ghost-tone discount the spine carries on top of
MONOLITH_SPINE_BRIGHTNESS and the layer ladder — was the last unnamed
magic number in the monolith tone chain. Promoted to
`MONOLITH_SPINE_GHOST_TONE` in central_control_rains.rs (value
unchanged, codegen identical: a const-folded multiply), mirroring the
stage-1 promotion of QUAS_HALO_CIRC_TAU. Zero behavior change; the
full spine tone chain now reads as named factors end to end.

## Considered and skipped (the over-engineering guard, quantified)

1. `color_for_level` recomputes the level-to-palette-index ladder per
   drawn cell (`last / 3`, `(last * 3) / 5`, `(last * 17) / 20` with
   `last` loop-invariant in the steady state). ~140 drawn cells per
   frame at 80x24 pay one strength-reduced integer division each —
   sub-microsecond, and LLVM lifts the ladder after inlining when the
   palette slice is invariant (the same LICM argument the stage-1 A/B
   proved empirically). A per-frame ladder snapshot would change the
   signature shared by five other families (lorenz, vortex, dragon,
   physarum, flux) — blast radius far beyond the gain. Skipped here;
   re-evaluated per-family as those stages open (stage 3 owns the
   first of them).
2. `draw_spine_cell` re-applies `tone.cadence.max(MONOLITH_SPINE_
   PERIOD)` per spine cell although the max is stream-invariant.
   ~40-60 spine cells per frame x one u16 max = nanoseconds. LLVM
   hoists it trivially after inlining. Skipped.
3. The glyph density gate rolls its own layer (matching distribution)
   before `build_droplet_spec` rolls the droplet's actual layer — two
   samples from the same distribution per successful spawn, so the
   gate's layer and the droplet's layer can differ. This is the
   documented v30 design (the gate is a stochastic pre-filter, not a
   per-sample constraint); "unifying" them would change per-layer
   spawn density and therefore the visual. Skipped as intentional.
4. A manual per-droplet hoist of the DrawCtx view chain in the
   cinematic draw dispatch: hunter-25 part 2 already measured manual
   hoists there as noise-scale with contradictory deltas across
   scenes and left the explicit verdict "do not hoist by hand — the
   compiler already folds the chain". Not re-attempted; that verdict
   stands as this stage's glyph-render answer.
5. `build_droplet_spec` samples ~6-8 randoms per spawn — irreducible
   stochastic spec construction, and spawns arrive at well under one
   per frame at display rates. Skipped.
6. monolith `find_inactive_lane` random probe (up to 16) + rotating
   linear fallback — the amortized O(1) family pattern. Skipped.

## Baseline measurements (10 s, this audit's build, 80x24 default)

| scene | avg fps | sim ms | dirty cells/frame | density gini | frame entropy (bits) |
|---|---|---|---|---|---|
| cinematic (Glyph) | 28 216 | 0.0191 | 461.8 | 0.636 | 5.178 |
| monolith | 70 022 | 0.0086 | 140.9 | 0.817 | 4.176 |

Glyph renders one to two orders of magnitude beyond display rate;
monolith sits at ~70K fps with the diff engine emitting ~141 cells per
frame. Both are nowhere near frame-budget pressure — the quantitative
form of the peak verdict.

## A/B (10 s benches, this sandbox, 2 runs per side, run-averaged)

| scene | metric | baseline | after | delta |
|---|---|---|---|---|
| cinematic (Glyph) | avg fps | 28 216.46 | 28 935.30 | +2.55 % |
| cinematic (Glyph) | sim ms | 0.019065 | 0.018542 | −2.74 % |
| cinematic (Glyph) | dirty cells | 461.80 | 454.78 | −1.52 % |
| cinematic (Glyph) | density gini | 0.6363 | 0.6403 | +0.63 % |
| cinematic (Glyph) | entropy bits | 5.1777 | 5.1597 | −0.35 % |
| monolith | avg fps | 70 022.39 | 70 435.15 | +0.59 % |
| monolith | sim ms | 0.008604 | 0.008560 | −0.51 % |
| monolith | dirty cells | 140.88 | 140.91 | +0.02 % |
| monolith | density gini | 0.8174 | 0.8175 | +0.01 % |
| monolith | entropy bits | 4.1755 | 4.1752 | −0.01 % |

Run-level spread (the noise yardstick): 1.87 % (cinematic baseline)
/ 0.42 % (cinematic after) / 0.50 % (monolith baseline) / 0.21 %
(monolith after).

The monolith deltas — the only code the diff touches — sit inside the
run spread, as expected for a const promotion with identical
codegen.

The cinematic deltas deserve the honest reading: the diff changes
zero lines of the Glyph code path (the only edit is a monolith
constant), so the +2.55 % fps cannot be a code effect. It is sandbox
machine-state drift between the baseline block (benched immediately
after a cold rebuild plus a 2 679-test gate run) and the after block
(benched on a settled cache). The dirty-cell count moves inversely
with fps by construction — droplets advance less per frame at higher
frame rates, so fewer cells cross row boundaries per frame; the
normalized throughput (fps x dirty cells) is 13.03M vs 13.16M cells
per second, ~1 % apart, inside the same noise band. Gini and entropy
drift with the stochastic sampling of different droplet states per
run. Verdict: performance- and visual-neutral.

## Stage gate

Stage 2 complete: the audit verdict is peak for both styles with
measured evidence, the one hygiene promotion is shipped and
A/B-verified neutral, and the full gate suite is green (fmt, clippy
--all-targets, build.sh check-all -q, gate-keepers.sh 10/10, 2 679
tests passed + 2 ignored in 56.5 s). Full A/B JSONs in
`benchmark/bench-labs/night_lts1_stage2/`.

Proceeding to stage 3 (vortex + solar flare) under the owner's
continuous-approval directive.
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
