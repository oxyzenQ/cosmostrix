<!-- SPDX-License-Identifier: GPL-3.0-only -->

# S-master-6-v2 — Chroma Dragon Visual-Impact Peak Audit + Lock (LTS)

**Date:** 2026-09-01
**Scope:** `src/engine/chroma_dragon_engine/` (visual-impact axis)
**Author:** oxyzenQ (cosmic dragon mode, master audit pass v2)
**Task:** The masterclass-most-valuable audit — chroma dragon at peak
high-quality visual impact, specialized/efficient code, strong LTS
foundation. No visual/performance downgrade. Hidden vulnerabilities
if found. Improve potential gain if possible. 10s A/B benchmark. Lock
with signature. Skip if already peak (no over-engineering).

## Audit axes (all staged, chroma-focused)

### 1. Visual tuning constants — 12/12 at sweep-audit-verified sweet spots

Every amplitude/frequency constant in `tuning.rs` (329 LOC, the
engine's visual control panel) carries a documented empirical sweep
that justifies its value and names the rejected alternatives:

| Constant | Value | Sweet-spot evidence |
|---|---|---|
| COLUMN_COHERENCE_FREQ | 0.105 rad/s (~60 s period) | atmospheric-not-animated band |
| SUBPIXEL_JITTER_AMPLITUDE | 3 | film-grain vs static(6-8)/invisible(1-2) |
| HEAD_HALO_FACTOR | 0.15 | soft-glow edge; 0.3-0.5 washes out dark bg |
| TRANSITION_L_SMOOTHING_WINDOW | 3.0 lines | 7-frame dissolve band, cascade preserved |
| ANOMALY_HALO_CYCLE_RATE | 4.0 stops/s | ~6/9 stops per lifetime, no strobe |
| PALETTE_FLOOR_RATIO | 0.20 | `phase7_print_ratio_sweep_audit` — 0.25 caps 4 themes, 0.30 = full v17 washout regression |
| ABSOLUTE_MIN_FLOOR | 30 | true-invisibility catch |
| BODY_TAIL_MAX_GAP_RATIO | 2.0 | `phase7b_print_gap_ratio_sweep_audit` — 2.5 horizontal-line illusion, 1.5 loses trail-fade |
| GLOBAL_MAX_FLOOR | 180 | v17 upper bound preserved |
| BORDER_TOUCH_PULSE_MAX/LIFETIME | 1.0 / 1500 ms | owner spec ("black to white, fades after a few seconds") |
| BORDER_TOUCH_HALO_MAX/LIFETIME | 0.3 / 400 ms | splash-cue cue, doesn't compete with message |

Owner-verdict anchor: the shipped visual identity (Deep Focus,
preset battle round 2, 2026-08-23, docs/VISUAL_IDENTITY.md) was
chosen by owner terminal A/B and locked — retuning any constant
would violate the "no visual downgrade" mandate.

### 2. Shading-path differentiation — design intent, not a gap

The default cinematic shading (DistanceFromHead) intentionally runs
its own length-aware gradient with Bayer ordered dithering + smooth
`t_param` interpolation + subpixel jitter, and SKIPS hue_drift /
column_coherence (in-shader `!shading_distance` guard, documented:
"stacking a hue shift would muddy the brightness-decay signal").
The random-shading path runs hue_drift + column coherence. Both
paths are v2-feature-complete for their visual language. All six
innovations remain always-`Some` in the production `DrawCtx`.

### 3. Resource efficiency — verified at peak (S-master-2 + this pass)

- `ColorCache`: single flat allocation + offset table of
  pre-formatted SGR byte sequences; hot-path style changes are
  memcpy splices (eliminates ~300-400 `write_sgr` calls/frame).
- `resolve_cell_color`: borrow-view `ShaderCtx`, direct array
  indexing (defensive bounds), integer hot path, pre-computed
  per-frame LUTs (hue_drift offset, column_coherence_lut).
- OKLab math lives at palette-build (cold path); per-cell work is
  index + lerp.
- Steady-state zero-alloc (565 allocs total in a 10s truecolor run,
  all construction-time; bit-stable across runs).

### 4. Security sweep of the chroma surface — clean

- `parse_color_tune`: strict key=value grammar, allowlisted keys,
  numeric parse, range [0.0, 3.0], applied values clamped [0, 255].
- `colors_custom`: LTS bounds from `dd87239` —
  COLORS_CUSTOM_MAX_BLOCKS=100, MAX_RAIN_STOPS=64, MAX_NAME_LEN=64,
  enforced with skip semantics.
- `color_map` per-cell indexing: defensive `idx < len` guard in the
  shader (out-of-range maps to stop 0).
- Zero `unsafe` in the chroma engine (v1 S6 sweep; re-verified).

### 5. A/B benchmark (10s, monolith 80x24 dry, TRUECOLOR chroma path)

Baseline A = the S-master-5-v2 truecolor evidence run (22b9417
binary); B = control after the S6 audit (e93aca5 binary, zero
source changes between — docs only):

| Metric | A | B (control) | Delta |
|---|---|---|---|
| avg_fps | 62084.37 | 62055.86 | -0.05% |
| frame_entropy_bits | 4.2120 | 4.2123 | +0.01% |
| density_gini | 0.8122 | 0.8122 | -0.00% |
| color_transition_delta_avg | 94.71 | 95.14 | +0.45% |
| dirty_cells_per_frame | 154.29 | 154.30 | +0.00% |
| alloc_calls | 565 | 565 | 0.00% (bit-stable) |
| total_ns_per_cell | 104.39 | 104.44 | +0.04% |
| frame_time_stability | excellent | excellent | stable |

All deltas inside the noise band on the CHROMA-ACTIVE path (the
mono path control was established in S-master-4-v2). Raw JSON:
benchmark/bench-labs/S_master_v2_v2/{S5_truecolor_chroma_evidence,S6_truecolor_control}.json.

## Verdict — ALREADY AT PEAK, LOCKED

The chroma dragon is at peak visual impact: every amplitude is
sweep-audit-pinned, the visual identity is owner-locked, the six v2
innovations are live, the code is specialized and efficient (LUTs,
flat caches, zero steady-state alloc), the security surface is
closed, and 289 chroma tests + 36 lock-suite tests + 1995 full
binary tests pass. Any further "gain" would either change output
bits (visual regression, forbidden) or add churn for an
unmeasurable delta (over-engineering, forbidden). The dragon stays
locked.

## Files Changed

- `src/engine/chroma_dragon_engine/KEY.md` — S-master-6-v2 lock
  signature entry.
- `docs/archive/audits/S6_V2_CHROMA_VISUAL_PEAK.md` — this doc.
- `CHANGELOG.md` — Unreleased entry.
- `benchmark/bench-labs/S_master_v2_v2/S6_truecolor_control.json` —
  raw control benchmark JSON.
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
