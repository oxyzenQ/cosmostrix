<!-- SPDX-License-Identifier: GPL-3.0-only -->

# S-master-5-v2 — Integrated Chroma Dragon Engine Verification (LTS, deeper)

**Date:** 2026-09-01
**Scope:** `src/engine/chroma_dragon_engine/` + integration points
**Author:** oxyzenQ (cosmic dragon mode, master audit pass v2)
**Task:** Deeper audit that all integrated chroma dragon engine is
works/real — 99% (not perfect) but stable and strong, already stable
production LTS. Update all docs/reference to avoid stale data.
**Predecessor:** v1 verification at `dd34821`
(docs/archive/audits/S5_CHROMA_INTEGRATED_VERIFY.md).

## What v2 adds over v1

v1 verified: static `ColorPipeline` routing, 289 chroma test count,
19 lock invariants, and `--doctor` disclosure — but the doctor run
executed on `legacy_rgb` (bench container has no truecolor TTY), so
v1's dynamic evidence exercised only the FALLBACK pipeline. v2 closes
that gap with forced-truecolor dynamic proof, a production-only module
census, and per-feature wiring verification of the six chroma v2
innovations from dragon-engine upgrade v2 (`d55442d`).

## 1. Dynamic runtime proof — chroma executes in the hot loop

10s monolith 80x24 dry benchmark, `--color-mode 24` (TrueColor forced):

| Signal | TrueColor (chroma active) | Mono (legacy baseline) |
|---|---|---|
| renderer.color_depth | `truecolor` | `mono` |
| color_transition_delta_avg | **94.71** | 0 |
| frame_entropy_bits | 4.2120 | 3.2953 |
| density_gini | 0.8122 | 0.8961 |
| dirty_cells_per_frame | 154.29 | 56.76 |
| avg_fps | 62084 | 93069 |
| frame_time_stability | excellent | excellent |
| fps_drift_percent | -0.24 | -0.37 |
| alloc_calls | 565 | 563 |

Interpretation: the nonzero color-transition stream and the higher
entropy/gini profile are chroma-dragon output flowing through the
production render loop — OKLab gradient interpolation, palette
routing, and post-FX all executing per frame. The ~33% fps cost vs
mono is the documented, accepted price of perceptual color math
(stability still excellent, drift flat). Raw JSON:
`benchmark/bench-labs/S_master_v2_v2/S5_truecolor_chroma_evidence.json`.

## 2. Pipeline disclosure on the chroma path (v1 gap closed)

`--doctor --color-mode 24` with `COLORTERM=truecolor` now shows the
chroma disclosure end to end:

```
color_pipeline: chroma_dragon
color_pipeline_detail: oklab gradient, perceptual blend, climate post-fx,
                       head halo, l-smoothing
```

Both disclosure branches (chroma_dragon + legacy_rgb with reason) are
now verified live; v1 could only demonstrate the legacy branch.

## 3. Production-only module census — 19/19 files integrated

Symbol-level rg census over every production `.rs` file in the engine
(test files, testconf, and docs_tests excluded), counting non-test
callers:

| Module | Production callers |
|---|---|
| catalog.rs | 139 |
| catalog/themes.rs | 80 |
| color_cache.rs | 99 |
| color_tune.rs | 178 |
| colors_custom.rs | 193 |
| gradient/mod.rs | 7 (+4 internal) |
| intro_colors.rs | 97 |
| legacy.rs | 16 (incl. droplet/draw.rs hot path) |
| mod.rs | 163 |
| palette/mod.rs | 161 |
| post/anomaly/mod.rs | 49 |
| post/climate/mod.rs | 103 |
| post/ghost.rs | 7 (+2 internal) |
| post/mod.rs | 46 |
| shaders/base/helpers.rs | 12 (+4 internal) |
| shaders/base/mod.rs | 190 |
| shaders/mod.rs | 85 |
| shaders/transition/mod.rs | 153 |
| tuning.rs | 97 |

**ZERO production zombies** — every module has non-test callers in
the render, config, validation, or diagnostics paths.

## 4. Six chroma v2 innovations — all wired live in production

Verified at the single `DrawCtx` construction site
(`cloud/rain_at.rs`), the per-frame entry into the shader:

| # | Innovation (from d55442d) | Production evidence |
|---|---|---|
| 1 | Temporal column hue coherence | `column_coherence_lut: Some(&self.column_coherence_lut)` — always Some; LUT built per frame, ~60s period |
| 2 | hue_drift activation | `hue_drift_offset: Some(hue_drift_offset(ecosystem.hue_drift))` — always Some; ecosystem accumulates `COLOR_HUE_DRIFT_RATE`, clamped to [-pi, pi] |
| 3 | Subpixel hue jitter | `subpixel_jitter_amplitude: Some(SUBPIXEL_JITTER_AMPLITUDE)` — always Some, deterministic per (line, col) |
| 4 | Head halo | `head_halo_factor: Some(HEAD_HALO_FACTOR)` — always Some; shader no-ops when bg is None/Reset |
| 5 | Bayer ordered dithering | `bayer_threshold(line, col)` (4x4 matrix) active in `resolve_cell_color` shading-distance path |
| 6 | Palette-aware ghost color | `rain_at.rs:619` calls `ghost::ghost_base_color(&self.palette.colors)` — derives ghost base from darkest palette stop |

All six consumed inside `resolve_cell_color` — the per-cell shader
that v1 already traced as the production hot path. Additional
always-on phases confirmed in the same sweep: Phase 5 perceptual
L-smoothing (transition window), Phase 6 palette-aware anomaly
halos, Phase 7 palette-relative brightness floor.

## 5. Fresh test counts (current tree, fe571b3)

| Suite | v1 (dd34821) | v2 (fe571b3) |
|---|---|---|
| chroma-filtered tests | 289 | **289** |
| lock suite (lock_inv01-19 + engine locks) | 36 | **36** |
| full binary suite | 1945 | **1995** |

Chroma contract surface unchanged since v1 (+50 tests elsewhere:
dragon-engine-v2 regression tests, config-cap tests).

## 6. Stale-data sweep

- Chroma comments audited for dormant/NOT-YET claims: all three hits
  (`tuning.rs` module doc, `ShaderCtx::head_halo_factor` doc,
  `tuning.rs:113`) accurately describe HISTORICAL pre-Phase-4 state
  and explicitly state the current always-on status — accurate, kept.
- README.md "~1649 tests" and lock-table entries are lock-time
  historical records (dated sections) — accurate as written, kept.
- No stale file-path pointers found in the engine (S-master-1 fixed
  the last 10; re-swept clean this pass).

## Verdict

The integrated chroma dragon engine is REAL, WORKING, and STABLE
PRODUCTION LTS — now verified DYNAMICALLY on the truecolor path,
not just statically and via tests. 99% (not perfect — nothing is —
but stable and strong). No code changes; the engine stays locked.

## Files Changed

- `src/engine/chroma_dragon_engine/KEY.md` — appended S-master-5-v2
  LTS verification entry (no code change, lock intact).
- `docs/archive/audits/S5_CHROMA_INTEGRATED_VERIFY_V2.md` — this doc.
- `CHANGELOG.md` — Unreleased entry.
- `benchmark/bench-labs/S_master_v2_v2/S5_truecolor_chroma_evidence.json`
  — raw truecolor benchmark JSON.
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
