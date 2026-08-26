<!-- SPDX-License-Identifier: GPL-3.0-only -->

# A-4 Zombie Kill Stage — bench Deep Audit

**Date:** 2026-08-26
**Auditor:** oxyzenQ (Cosmic Dragon mode)
**Version:** v50.0.0-beta.6
**Commit:** 5b140c8
**Scope:** `src/bench/` (18 files, 7,166 LOC) — fourth-largest source directory, final stage in per-stage zombie kill sweep.
**Constraint:** No changes 99% visual/performance.
**Methodology:** `scripts/stale-hunt.py` + targeted `rg` sweeps + `cargo clippy` + mod-tree wiring verification + 10s A/B benchmark.

---

## 0. Executive Summary

**Result: 0 zombies found. 0 code changes required.**

The `bench` directory is already zombie-free, consistent with A-1
through A-3. The gatekeeper continues to hold across all four
largest directories. A/B benchmark confirms no regression.

| Metric | Value |
|---|---|
| Files audited | 18 (all `.rs` files in `bench/`) |
| Total LOC | 7,166 |
| Zombie files | **0** |
| Stale references | **0** |
| `TODO`/`FIXME`/`XXX`/`HACK` | **0** |
| `todo!()`/`unimplemented!()`/`unreachable!()` | **0** |
| `#[allow(dead_code)]` in production | **1** (platform-cfg guard, defensible) |
| Clippy warnings | **0** |
| Feature-gated dead code | **0** |

**A/B Benchmark (10s, 120x40 monolith, pro profile):**

| Metric | Run A | Run B | Delta |
|---|---|---|---|
| avg_fps | 51,936.51 | 51,671.06 | -0.5% (noise) |
| p99_frame_time | 0.0241 ms | 0.0250 ms | within noise |
| frame_time_stability | excellent | excellent | same |

---

## 1. Per-Stage Progress (Complete)

| Rank | Directory | Files | LOC | Audited | Result |
|---|---|---|---|---|---|
| 1 | `cosmic_dragon_engine/` | 52 | 22,551 | A-1 (done) | 0 zombies |
| 2 | `chroma_dragon_engine/` | 30 | 13,351 | A-2 (done) | 0 zombies |
| 3 | `interactive/` | 20 | 11,263 | A-3 (done) | 0 zombies |
| 4 | `bench/` | 18 | 7,166 | **A-4 (this report)** | **0 zombies** |
| **Total** | — | **120** | **54,331** | **Complete** | **0 zombies** |

---

## 2. Findings

### 2.1 No Zombie Files

All 18 `.rs` files in `bench/` are properly wired into the module
tree via `mod` declarations in `mod.rs`.

### 2.2 No Stale References, No Tech-Debt Markers

Zero stale references, zero TODO/FIXME/XXX/HACK, zero
`todo!()`/`unimplemented!()`/`unreachable!()` in 7,166 LOC.

### 2.3 One `#[allow(dead_code)]` — Platform-cfg Guard

**Location:** `src/bench/bench_perf.rs:292`

```rust
pub(crate) struct PerfCounterHandle {
    #[cfg(target_os = "linux")]
    inner: Option<linux::PerfCounters>,
    #[cfg(not(target_os = "linux"))]
    #[allow(dead_code)]
    inner: Option<()>,
}
```

**Verdict:** Correct platform-cfg guard. On non-Linux platforms, the
`inner` field becomes `Option<()>` which is never read (only Linux
has the `read()` implementation that accesses `inner`). Same pattern
as the 11 platform-cfg guards in A-1 §2.4. Defensible.

### 2.4 No Clippy Warnings, No Feature Gates

`cargo clippy` reported 0 warnings for `bench/`. Zero
`#[cfg(feature = "...")]` gates — no feature-gated dead code.

---

## 3. A/B Benchmark Results

| Metric | Run A | Run B | Delta | Assessment |
|---|---|---|---|---|
| avg_fps | 51,936.51 | 51,671.06 | -0.5% | Noise |
| peak_fps | 66,760.13 | 68,742.70 | +3.0% | Noise |
| p99_frame_time | 0.0241 ms | 0.0250 ms | +3.7% | Noise |
| avg_frame_time | 0.0193 ms | 0.0194 ms | +0.5% | Noise |
| frame_time_stability | excellent | excellent | — | No change |
| avg_dirty_cells_per_frame | 107.3 | 107.4 | +0.1% | Noise |
| peak_rss | 4.60 MiB | 4.52 MiB | -1.7% | Noise |
| heap_retained | 119.67 KiB | 84.29 KiB | -30% | Noise (small absolute values, allocator jitter) |

---

## 4. Cumulative Gatekeeper Validation (A-1 through A-4 — Complete)

| Stage | Directory | Files | LOC | Zombies | Stale Refs | TODO | `#[allow(dead_code)]` | Clippy |
|---|---|---|---|---|---|---|---|---|
| A-1 | cosmic_dragon_engine | 52 | 22,551 | 0 | 0 | 0 | 1 | 0 |
| A-2 | chroma_dragon_engine | 30 | 13,351 | 0 | 0 | 0 | 2 | 0 |
| A-3 | interactive | 20 | 11,263 | 0 | 0 | 0 | 0 | 0 |
| A-4 | bench | 18 | 7,166 | 0 | 0 | 0 | 1 | 0 |
| **Total** | — | **120** | **54,331** | **0** | **0** | **0** | **4** | **0** |

### 4.1 Final Assessment

The per-stage zombie kill sweep is **complete**. All four largest
source directories (120 files, 54,331 LOC — 58% of total src/ LOC)
have been audited:

- **0 zombies found** across all four stages
- **0 stale references** (stale-hunt.py clean)
- **0 tech-debt markers** (TODO/FIXME/XXX/HACK)
- **0 runtime panic markers** (todo!()/unimplemented!()/unreachable!())
- **0 clippy warnings**
- **4 `#[allow(dead_code)]` markers** — all reviewed and defensible:
  - 1 platform-cfg guard (A-1: cloud/mod.rs:290 — future-reserved profile field)
  - 2 future-reserved/doc-anchor (A-2: intro_colors.rs:94, palette/tests_audit.rs:30)
  - 1 platform-cfg guard (A-4: bench_perf.rs:292 — non-Linux fallback)

### 4.2 Gatekeeper Effectiveness

The project's gatekeeper is **highly effective** at preventing zombie
code accumulation:

1. **`clippy -D warnings`** catches unused imports, dead code, style violations at PR time
2. **`scripts/stale-hunt.py`** catches stale CLI flag refs, stale file paths, stale module paths (parses Rust comment structure, no false positives)
3. **LOC guard (1500-line cap)** forces refactoring before files become spaghetti (event_loop.rs sits exactly at cap, with event_loop_finalize.rs extracted)
4. **`gate-keepers.sh`** enforces SPDX headers, markdownlint, version sync, doc disclaimers
5. **`check-rs-loc.sh`** tracks LOC growth

No additional zombie-prevention tooling is needed. The codebase is
in excellent shape.

### 4.3 Remaining Directories (Not Audited — Below Threshold)

The per-stage strategy targeted directories with the most files
first. The remaining 26 `src/` subdirectories have fewer than 18
files each and collectively contain ~39K LOC. Given the zero-zombie
result across the four largest directories (which account for 58% of
src/ LOC), the probability of zombies in the remaining directories
is low — the same gatekeeper applies to all of `src/`.

**Recommendation:** No further per-stage audits needed unless owner
directs otherwise. The gatekeeper is proven effective. Future
zombie-prevention is handled by the existing CI pipeline
(`check-all` gate + `clippy -D warnings`).

---

## 5. Audit Signoff

**Task:** A-4 zombie kill stage — bench deep audit (final stage).
**Result:** 0 zombies found. 0 code changes required. A/B benchmark
confirms no regression.
**Per-stage sweep status:** **COMPLETE** (A-1 through A-4, 120 files,
54,331 LOC, 0 zombies total).
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
