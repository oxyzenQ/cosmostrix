<!-- SPDX-License-Identifier: GPL-3.0-only -->

# A-3 Zombie Kill Stage — interactive Deep Audit

**Date:** 2026-08-26
**Auditor:** oxyzenQ (Cosmic Dragon mode)
**Version:** v50.0.0-beta.6
**Commit:** e73bb21
**Scope:** `src/interactive/` (20 files, 11,263 LOC) — third-largest source directory, audited third per per-stage strategy.
**Constraint:** No changes 99% visual/performance.
**Methodology:** `scripts/stale-hunt.py` + targeted `rg` sweeps + `cargo clippy` + mod-tree wiring verification + 10s A/B benchmark.

---

## 0. Executive Summary

**Result: 0 zombies found. 0 code changes required.**

The `interactive` directory is already zombie-free, consistent with
A-1 and A-2. The gatekeeper continues to hold. A/B benchmark confirms
no regression.

| Metric | Value |
|---|---|
| Files audited | 20 (all `.rs` files in `interactive/`) |
| Total LOC | 11,263 |
| Zombie files (not wired into mod tree) | **0** |
| Stale references (stale-hunt.py) | **0** |
| `TODO`/`FIXME`/`XXX`/`HACK` markers | **0** |
| `todo!()`/`unimplemented!()`/`unreachable!()` | **0** |
| `#[allow(dead_code)]` in production code | **0** |
| Clippy warnings | **0** |
| Feature-gated dead code | **0** |

**A/B Benchmark (10s, 120x40 monolith, pro profile):**

| Metric | Run A | Run B | Assessment |
|---|---|---|---|
| avg_fps | 51,895.06 | 51,194.35 | -1.35% (noise — 2-vCPU cloud VM jitter) |
| p99_frame_time | 0.0246 ms | 0.0331 ms | within "excellent" stability |
| frame_time_stability | excellent | excellent | same |
| peak_rss | 4.57 MiB | 4.47 MiB | noise |

---

## 1. Per-Stage Progress

| Rank | Directory | Files | LOC | Audited | Result |
|---|---|---|---|---|---|
| 1 | `cosmic_dragon_engine/` | 52 | 22,551 | A-1 (done) | 0 zombies |
| 2 | `chroma_dragon_engine/` | 30 | 13,351 | A-2 (done) | 0 zombies |
| 3 | `interactive/` | 20 | 11,263 | **A-3 (this report)** | **0 zombies** |
| 4 | `bench/` | 18 | — | Pending | — |

---

## 2. Findings

### 2.1 No Zombie Files

All 20 `.rs` files in `interactive/` are properly wired into the
module tree. `tests_v50_first_reload.rs` is loaded via `#[path]`
attribute in `tests.rs:812` (verified in A-1 §2.1).

### 2.2 No Stale References, No Tech-Debt Markers

`scripts/stale-hunt.py` reported 0 stale references. Zero
`TODO`/`FIXME`/`XXX`/`HACK`, zero `todo!()`/`unimplemented!()`/
`unreachable!()` in 11,263 LOC.

### 2.3 Zero `#[allow(dead_code)]` in Production

Unlike A-1 (1 spot) and A-2 (2 spots), `interactive/` has **zero**
`#[allow(dead_code)]` markers in production code. This is the cleanest
result so far.

### 2.4 Suspicious Items Investigated + Cleared

#### 2.4.1 `event_loop.rs` vs `event_loop_finalize.rs` — Not Redundant

**Initial flag:** Two files with "event_loop" prefix.

**Investigation:**
- `event_loop.rs` (1,500 LOC) — main interactive event loop (`run_interactive()`). Drives signal handling, frame pacing, input dispatch, simulation stepping, rendering.
- `event_loop_finalize.rs` (415 LOC) — post-loop session finalization. Owns shutdown signal, terminal stats capture, `--perf-stats` report, terminal drop, final-state handoff.

**Verdict:** `event_loop_finalize.rs` was extracted from `event_loop.rs`
to keep the latter under the 1,500-LOC file cap. Both files are
actively used, distinct in purpose. Not redundant.

#### 2.4.2 Three Intro Files — Not Redundant

| File | LOC | Purpose |
|---|---|---|
| `intro.rs` | 816 | Modular intro system dispatch + Linux process metrics helpers (RSS, context switches) for HUD overlay |
| `intro_cosmic.rs` | 699 | Cosmic Burst intro — 4-phase cinematic (singularity → burst → morph → rain) |
| `intro_logo/mod.rs` | 799 | Logo intro — ASCII logo reveal + dissolve to Matrix rain using 7-stage rain method |

> **v52 path update (2026-08-31)**: all three intro files moved out of
> `interactive/` in the one-file-per-style refactor — the dispatcher +
> shared particle skeleton now live in `src/intro_style/mod.rs`, the
> styles in `src/intro_style/cosmic.rs` + `src/intro_style/logo.rs`
> (+ `logo_tests.rs`), and the Linux process metrics helpers in
> `src/sysstat/procstat.rs`. The LOC counts above are the audit-time
> snapshot, not current.

**Verdict:** Each file implements a distinct intro animation with
different visual sequences and phase timings. The `intro.rs` file
addively houses Linux process metrics helpers (documented as "kept
here because the file already exists; the helpers are tiny"). Not
redundant.

#### 2.4.3 Three Test Files — Not Redundant

| File | LOC | Purpose |
|---|---|---|
| `tests.rs` | 813 | Core interactive tests + `#[path]` loader for `tests_v50_first_reload.rs` |
| `tests_v35.rs` | 866 | v35 LTS regression tests (cloud ambient flags, scene defaults) |
| `tests_v50_first_reload.rs` | 202 | v50 LTS regression: first-reload scene reset crash fix |

**Verdict:** Each test file covers a different version's regression
suite. The split keeps each file under the 1,500-LOC cap. Not
redundant.

---

## 3. A/B Benchmark Results

| Metric | Run A | Run B | Delta | Assessment |
|---|---|---|---|---|
| avg_fps | 51,895.06 | 51,194.35 | -1.35% | Noise (2-vCPU cloud VM jitter) |
| peak_fps | 67,312.87 | 65,206.05 | -3.1% | Noise |
| p99_frame_time | 0.0246 ms | 0.0331 ms | +35% | Within "excellent" stability (single-frame spike, avg unaffected) |
| avg_frame_time | 0.0193 ms | 0.0195 ms | +1% | Noise |
| frame_time_stability | excellent | excellent | — | No change |
| avg_dirty_cells_per_frame | 107.4 | 107.3 | -0.1% | Noise |
| peak_rss | 4.57 MiB | 4.47 MiB | -2.2% | Noise |

**Note on Run B p99:** The 0.0331ms p99 on Run B (vs 0.0246ms on Run
A) is a single-frame spike on a 2-vCPU cloud VM. The avg_frame_time
(0.0195ms) is unaffected, and `frame_time_stability` remains
"excellent". This is normal cloud-VM jitter, not a regression — the
audit required zero code changes, so the binary is identical to A-1
and A-2.

---

## 4. Cumulative Gatekeeper Validation (A-1 + A-2 + A-3)

| Stage | Directory | Files | LOC | Zombies | Stale Refs | TODO | `#[allow(dead_code)]` | Clippy |
|---|---|---|---|---|---|---|---|---|
| A-1 | cosmic_dragon_engine | 52 | 22,551 | 0 | 0 | 0 | 1 | 0 |
| A-2 | chroma_dragon_engine | 30 | 13,351 | 0 | 0 | 0 | 2 | 0 |
| A-3 | interactive | 20 | 11,263 | 0 | 0 | 0 | 0 | 0 |
| **Total** | — | **102** | **47,165** | **0** | **0** | **0** | **3** | **0** |

The 3 `#[allow(dead_code)]` markers across 47K LOC are all documented
future-reservations or doc-anchors (reviewed and cleared in A-1 §2.4
and A-2 §2.4). No actionable zombies found in any of the three
largest directories.

---

## 5. Audit Signoff

**Task:** A-3 zombie kill stage — interactive deep audit.
**Result:** 0 zombies found. 0 code changes required. A/B benchmark
confirms no regression.
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
