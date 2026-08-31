<!-- SPDX-License-Identifier: GPL-3.0-only -->

# B-2 Optimize Code — chroma_dragon_engine Audit

**Date:** 2026-08-26
**Auditor:** oxyzenQ (Cosmic Dragon mode)
**Version:** v50.0.0-beta.6
**Commit:** 89ca7b3
**Scope:** `src/chroma_dragon_engine/` (30 files, 13,351 LOC) — second stage of optimize sweep.
**Constraint:** No changes 99% visual/performance.

---

## 0. Executive Summary

**Result: 0 optimizations applicable. Codebase already fully optimized.**

The chroma_dragon_engine shader hot path is already at the optimization ceiling. The `TRAIL_EXP_LUT` precomputed lookup table replaces per-cell `exp()` calls. The OKLab gradient interpolation runs only at palette build time (not per-frame). The `color_cache` pre-formats SGR escape sequences at startup, eliminating per-frame formatting.

| Metric | Value |
|---|---|
| Hot-path functions audited | `resolve_cell_color`, `interpolate_palette_color`, `color_uses_previous_palette` |
| `#[inline]` coverage | All hot fns already `#[inline]` |
| Per-frame heap allocations in shader path | 0 |
| Per-frame `.clone()` in shader path | 0 (1 clone in cache build path, correct) |
| Float math (sinf/cosf/expf) in per-cell path | 0 (all replaced with LUTs) |
| Redundant color conversions | 0 |
| Optimizations applied | **0** (nothing to optimize) |

**A/B Benchmark (10s, 120x40 monolith, pro profile):**

| Metric | 3-run mean | Assessment |
|---|---|---|
| avg_fps | 51,681 | Consistent with B-1 baseline (51,602) + A-1..A-5 (51,700-52,000) |
| frame_time_stability | excellent | No regression |

---

## 1. Audit Findings

### 1.1 Shader Hot Path — Already Optimal

`resolve_cell_color()` (chroma/shaders/base/mod.rs:423) is the per-cell color decision function. Audit confirmed:

- **`#[inline]`** — already marked, LLVM folds the `DrawCtx → ShaderCtx → resolve_cell_color` chain at the call site.
- **LUT-based brightness** — `TRAIL_EXP_LUT` (256-entry precomputed table) replaces per-cell `exp()` call. Comment at line ~470: "OPTIMIZED: use precomputed LUT instead of exp() per cell".
- **Bayer 4x4 dithering** — threshold matrix eliminates banding without per-cell branches.
- **NaN-safe interpolation** — `interpolate_palette_color` is ~3ns/call, defensive on NaN/Inf.
- **No heap allocations** — `ShaderCtx` is a thin borrow view, no allocs.

### 1.2 Gradient Interpolation — Build-Time Only

`gradient/mod.rs` implements OKLab polar interpolation. The module header explicitly documents:

> ~12 multiplies + 3 cbrt() per stop transition (OKLab conversion) plus ~2 atan2 + 2 sin/cos per segment transition (polar math). Called only at palette build time (not the hot render path), so the cost is negligible.

No optimization needed — the heavy math runs once at startup, not per-frame.

### 1.3 Color Cache — Pre-formatted SGR

`color_cache.rs` pre-computes ANSI SGR escape sequences (`\x1b[38;2;R;G;Bm`) at startup and stores them in a flat byte buffer with an index table. The per-frame render path does a single indexed read instead of calling `write_sgr_colors_buf` (which encodes integer→ASCII digits per call).

The one `.clone()` at color_cache.rs:79 (`palette.colors.clone()`) is in the cache BUILD path, not the per-frame render path. Correct.

### 1.4 Legacy Fallback — Already `#[inline]`

`legacy.rs` houses the sRGB-linear fallback math for non-truecolor terminals. Every function is `#[inline]` and the module header documents: "Every function in this module is `#[inline]` and compiles to the exact same machine code as the inlined version it replaces."

---

## 2. Why No Optimizations Applied

The chroma_dragon_engine has already been through multiple optimization passes:

1. **Chroma Dragon Phase 2** — extracted `resolve_cell_color` from `DrawCtx::get_attr` as a pure function with `#[inline]`.
2. **Phase 3-A** — OKLab polar interpolation replaced sRGB-linear (build-time only, no hot-path cost).
3. **Phase 4-A/B/D** — column-coherence LUT, subpixel jitter amplitude, head halo factor all pre-computed once per frame in `rain.rs`, then passed as single values to the shader.
4. **Phase 5** — perceptual L smoothing at palette transition wave, pre-computed table.
5. **Cosmic Dragon egg #15** — bounds-check + direct indexing for `color_map` (avoids `Option` alloc on hot path).
6. **TRAIL_EXP_LUT** — precomputed exponential decay table replaces per-cell `exp()`.
7. **color_cache** — pre-formatted SGR bytes eliminate per-frame integer→ASCII formatting.

The codebase is at the optimization ceiling for this workload. The `docs/SIMD_FEASIBILITY.md` investigation already concluded that SIMD vectorization is NOT beneficial here because the per-cell shader math is too branchy (palette slot selection, CharLoc matching). The LUT-based approach is faster than SIMD for this workload.

---

## 3. A/B Benchmark Results

| Metric | Run A | Run B | Run C | Mean |
|---|---|---|---|---|
| avg_fps | 51,589 | 51,648 | 51,805 | 51,681 |
| frame_time_stability | excellent | excellent | excellent | excellent |

Consistent with all prior stages (A-1 through B-1). No regression. No code changes = identical binary to B-1.

---

## 4. Cumulative Optimize Progress

| Stage | Directory | Files | LOC | Optimizations | Commit |
|---|---|---|---|---|---|
| B-1 | cosmic_dragon_engine | 52 | 22,551 | 1 (hoisted buffer) | `89ca7b3` |
| B-2 | chroma_dragon_engine | 30 | 13,351 | 0 (already optimal) | this report |
| **Total** | — | **82** | **35,902** | **1** | — |

---

## 5. Audit Signoff

**Task:** B-2 optimize code — chroma_dragon_engine audit.
**Result:** 0 optimizations applicable. Codebase already fully optimized (LUT-based shader math, pre-formatted SGR cache, build-time-only gradient interpolation).
**Artifacts:** This report only.

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
