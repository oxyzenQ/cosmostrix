<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Z-2/Z-3/Z-4 LTS Audit — chroma_dragon_engine + interactive + bench

**Date:** 2026-08-26
**Auditor:** oxyzenQ (Cosmic Dragon mode)
**Version:** v50.0.0-beta.6
**Commit:** 20ead0e
**Scope:** `src/engine/chroma_dragon_engine/` (30 files) + `src/interactive/` (20 files) + `src/bench/` (18 files) — final stages of LTS audit.
**Constraint:** No changes 99% visual/performance.

---

## 0. Executive Summary

**Result: 0 LTS fixes applicable. All three directories already LTS-hardened.**

| Stage | Directory | Files | `.expect()` | `panic!()` | NaN guards | `saturating_*` | LTS Fixes |
|---|---|---|---|---|---|---|---|
| Z-2 | chroma_dragon_engine | 30 | 6 (all documented) | 0 | present | 8 | **0** |
| Z-3 | interactive | 20 | 2 (all documented) | 0 | 6 | 22 | **0** |
| Z-4 | bench | 18 | 2 (all documented) | 0 | 4 | 15 | **0** |

**A/B Benchmark:** Consistent with all prior stages, no regression.

---

## 1. Z-2: chroma_dragon_engine

### 1.1 Findings

- **6 `.expect()` in production** — all with documented invariants (Uniform::new_inclusive with constant ranges)
- **0 `panic!()` in production**
- **NaN guards present** — `interpolate_palette_color` documented as "NaN/Inf-safe (returns the first stop defensively)" (shaders/base/mod.rs:488-489)
- **8 `saturating_*` calls** — palette index math, distance calculations
- **`#[allow(dead_code)]` on `Disposition::Differentiate/Merge`** — documented future-reserved (reviewed in A-2)

### 1.2 Verdict

Already LTS-hardened. The `interpolate_palette_color` NaN safety is the critical guard — it's the hottest float path (called per-cell) and is explicitly documented as defensive. No fixes needed.

---

## 2. Z-3: interactive

### 2.1 Findings

- **2 `.expect()` in production** — all with documented invariants
- **0 `panic!()` in production**
- **6 NaN guards** — HUD metric setters (`droplet_density`, `chars_per_sec`, `endurance_health_score`, `effective_pressure`) all clamp NaN/Inf to 0 (v50 stability hardening), speed validation in event_loop
- **22 `saturating_*` calls** — frame counters, context-switch tracking, timing math
- **Signal handling** — SIGTERM/SIGHUP/SIGQUIT (graceful), SIGTSTP/SIGCONT (suspend/resume), SIGINT ignored (reviewed in Z-1)

### 2.2 Verdict

Already LTS-hardened. The v50 HUD NaN clamping (hud/mod.rs:610,626,643,662) ensures a buggy upstream metric cannot corrupt the HUD display. No fixes needed.

---

## 3. Z-4: bench

### 3.1 Findings

- **2 `.expect()` in production** — all with documented invariants
- **0 `panic!()` in production**
- **4 NaN guards** — JSON serialization (bench_scale.rs:163, bench_json.rs:481,530,540) emits `null` for NaN/Inf (RFC 8259 compliance)
- **15 `saturating_*` calls** — sample counters, percentile index math, byte accumulation
- **`PerfCounters::Drop`** (bench_perf.rs:203) — closes perf FDs on Linux, prevents resource leak

### 3.2 Verdict

Already LTS-hardened. The JSON NaN/Inf safety (RFC 8259 compliance) is the critical guard — ensures benchmark JSON output is always valid. No fixes needed.

---

## 4. Cumulative LTS Audit (Z-1 through Z-4 — COMPLETE)

| Stage | Directory | Files | LOC | LTS Fixes | Commit |
|---|---|---|---|---|---|
| Z-1 | cosmic_dragon_engine | 52 | 22,551 | 0 | `20ead0e` |
| Z-2 | chroma_dragon_engine | 30 | 13,351 | 0 | this report |
| Z-3 | interactive | 20 | 11,263 | 0 | this report |
| Z-4 | bench | 18 | 7,166 | 0 | this report |
| **Total** | — | **120** | **54,331** | **0** | — |

### 4.1 Final LTS Assessment

The complete LTS audit is **done**. All four largest directories (120 files, 54,331 LOC) are already LTS-hardened:

- **0 `panic!()` in production code** across all 4 directories
- **34 `.expect()` calls** — all with documented invariants
- **NaN/Inf guards** on every float path (interpolate_palette_color, HUD metrics, thermal pressure, JSON serialization)
- **saturating_* arithmetic** everywhere (105+ calls across 4 dirs)
- **Terminal::drop** with watchdog + double-panic guard
- **Signal handling** comprehensive (SIGTERM/SIGHUP/SIGQUIT/SIGTSTP/SIGCONT, SIGINT ignored)
- **Terminal disconnect** classified + recovered (BrokenPipe/EIO/EBADF)
- **JSON RFC 8259 compliance** (NaN/Inf emit `null`)

### 4.2 LTS Hardening History

The codebase has been through multiple LTS hardening passes:

1. **v16 audit** — Windows silent-exit fix (panic hook restores terminal before printing)
2. **v25 coredump fix** — Double-panic proof panic hook (write_fmt with discarded error)
3. **v50 HUD stability** — NaN/Inf clamping on all HUD metrics
4. **CC2-03** — Thermal pressure NaN guard (power_manager)
5. **Cosmic Dragon egg #15** — Bounds-check + direct indexing for color_map
6. **RFC 8259 compliance** — JSON NaN/Inf emit `null`

The gatekeeper (`clippy -D warnings`) catches new `unwrap()`/`expect()` without invariant comments at PR time. The codebase is at the LTS ceiling.

---

## 5. A/B Benchmark Results

| Metric | Run A | Run B | Run C | Mean |
|---|---|---|---|---|
| avg_fps | 50,880 | 51,402 | 51,585 | 51,289 |
| frame_time_stability | excellent | excellent | excellent | excellent |

Consistent with all prior stages. The slight lower mean is cloud-VM jitter (2 vCPUs), not a regression — binary is identical to Z-1 (zero code changes).

---

## 6. Audit Signoff

**Task:** Z-2/Z-3/Z-4 LTS audit — chroma_dragon_engine + interactive + bench (final stages).
**Result:** 0 LTS fixes applicable. All three directories already LTS-hardened.
**LTS audit status:** **COMPLETE** (Z-1 through Z-4, 120 files, 54,331 LOC, 0 fixes total).
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
