<!-- SPDX-License-Identifier: GPL-3.0-only -->

# B-3/B-4 Optimize Code — interactive + bench Audit

**Date:** 2026-08-26
**Auditor:** oxyzenQ (Cosmic Dragon mode)
**Version:** v50.0.0-beta.6
**Commit:** 6ac5a95
**Scope:** `src/interactive/` (20 files, 11,263 LOC) + `src/bench/` (18 files, 7,166 LOC) — final stages of optimize sweep.
**Constraint:** No changes 99% visual/performance.

---

## 0. Executive Summary

**Result: 0 optimizations applicable. Both directories already optimized.**

| Stage | Directory | Files | LOC | Optimizations | Verdict |
|---|---|---|---|---|---|
| B-3 | interactive | 20 | 11,263 | 0 | Per-frame path clean (allocs only at startup/init) |
| B-4 | bench | 18 | 7,166 | 0 | Hot fns called once-per-bench, not per-frame |

**A/B Benchmark (10s, 120x40 monolith, pro profile):** Consistent with all prior stages, no regression.

---

## 1. B-3: interactive Audit

### 1.1 event_loop.rs Hot Path — Clean

The per-frame event loop body (`run_interactive`) was checked for:
- **Vec/alloc in hot path:** 0 (no `Vec::new()`, `vec![]`, `.collect()`, `format!`, `to_string()` in the per-frame loop body)
- **`.clone()` in hot path:** 0 (all `.clone()` calls are at startup/init — lines 45, 218, 219, 221, 230, 235, 241, 247, 249, 265 — these are config setup before the loop starts)

### 1.2 Why No Optimizations

The event loop follows the established pattern:
- Frame pacing uses `Instant::now()` + `Duration` math (no allocs)
- Input dispatch matches on `crossterm::event::Event` (Copy type, no allocs)
- Simulation step delegates to `Cloud::update()` (already optimized in B-1)
- Render delegates to `Terminal::draw()` (already optimized in Cosmic Dragon egg experiments)
- Idle-frame fast path skips render entirely when no cells changed (documented in draw.rs:97-104)

The `.clone()` calls at startup (config setup) are correct — they clone `CloudConfig` for the live-reload watcher thread, which needs an owned copy. These run once at startup, not per-frame.

---

## 2. B-4: bench Audit

### 2.1 Bench Hot Path — Clean

`bench/mod.rs` was checked for:
- **Vec/alloc in benchmark loop:** 0 (no `Vec::new()`, `vec![]`, `.collect()` in the benchmark iteration)
- **`#[inline]` on bench fns:** Not needed — `run_benchmark`, `run_premium_benchmark`, `compute_peak_fps` are called once per benchmark run (not per-frame). Inlining would bloat the binary without benefit.

### 2.2 Why No Optimizations

The benchmark path is inherently not perf-critical in the same way as the render path:
- `run_benchmark()` sets up the benchmark, runs N frames, collects metrics, prints report
- The per-frame work inside the benchmark loop is the same `Cloud::update()` + `Terminal::draw()` path (already optimized in B-1)
- `compute_peak_fps()` sorts frame times — called once at report time, not per-frame
- The benchmark is a measurement tool, not a production hot path

---

## 3. Cumulative Optimize Sweep (B-1 through B-4 — COMPLETE)

| Stage | Directory | Files | LOC | Optimizations | Commit |
|---|---|---|---|---|---|
| B-1 | cosmic_dragon_engine | 52 | 22,551 | 1 (hoisted buffer) | `89ca7b3` |
| B-2 | chroma_dragon_engine | 30 | 13,351 | 0 (already optimal) | `6ac5a95` |
| B-3 | interactive | 20 | 11,263 | 0 (per-frame clean) | this report |
| B-4 | bench | 18 | 7,166 | 0 (not perf-critical) | this report |
| **Total** | — | **120** | **54,331** | **1** | — |

### 3.1 Final Assessment

The optimize sweep is **complete**. Across all four largest directories (120 files, 54,331 LOC):

- **1 optimization applied** (B-1: hoisted `border_cross_candidates` buffer, following the `crt_vignette_candidates` precedent)
- **0 visual changes** (all optimizations produce identical render output)
- **0 perf regressions** (A/B benchmark consistent across all stages)
- **Codebase is at the optimization ceiling** for the current architecture

The project has already been through multiple optimization passes:
1. Cosmic Dragon egg experiments (eliminated per-frame allocs in render path)
2. T1.1-real (hoisted `crt_vignette_candidates` scratch buffer)
3. Chroma Dragon phases 2-5 (LUT-based shader math, pre-computed tables)
4. TRAIL_EXP_LUT (replaced per-cell `exp()`)
5. color_cache (pre-formatted SGR bytes)
6. Split sim loops (eliminated dead branches in bench mode)
7. B-1 (this sweep — hoisted border-cross candidates buffer)

### 3.2 SIMD Note

`docs/SIMD_FEASIBILITY.md` already investigated and rejected SIMD for the shader hot path (too branchy). The LUT-based approach is faster than SIMD for this workload. No action needed.

### 3.3 Remaining Directories

The remaining 26 small directories (~39K LOC) are unlikely to contain optimization opportunities — they are config parsing, CLI dispatch, platform glue, and test infrastructure, none of which are per-frame hot paths. The gatekeeper (`clippy -D warnings`) catches any new performance issues at PR time.

---

## 4. Audit Signoff

**Task:** B-3/B-4 optimize code — interactive + bench audit (final stages).
**Result:** 0 optimizations applicable. Both directories already optimized.
**Optimize sweep status:** **COMPLETE** (B-1 through B-4, 120 files, 54,331 LOC, 1 optimization applied total).
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
