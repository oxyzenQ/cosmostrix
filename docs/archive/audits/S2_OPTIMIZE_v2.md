<!-- SPDX-License-Identifier: GPL-3.0-only -->

# S-master-2 — Optimize Code (Post-Peak Verification)

**Date:** 2026-09-01
**Scope:** `cosmostrix/*`, `src/*`, `deps` (perstage — hot path dirs only)
**Author:** oxyzenQ (cosmic dragon mode, master audit pass)
**Predecessors:** B1–B4 OPTIMIZE, Z5 PERF_REGRESSION, S1 DRAGON_HUNT_v3

## Context

The codebase had already been through multiple optimization sweeps
(B1–B4 OPTIMIZE, Z5 PERF_REGRESSION). This audit was a **post-peak
verification pass** to confirm whether any remaining optimization
opportunities exist, and to apply safe micro-optimizations.

## Method

Static-only audit (ripgrep + Read) of 14 hot-path dirs:
`cloud/rain_at.rs`, `cloud/render.rs`, `cloud/living_rain.rs`,
`cloud/spawn*.rs`, `cloud/runtime_controls.rs`,
`cloud/brightness_factors.rs`, `cloud/palette_blend.rs`,
`cloud/post_rain.rs`, `cloud/message_draw.rs`, `terminal/draw.rs`,
`terminal/sgr_format.rs`, `chroma/palette/mod.rs`,
`chroma/shaders/base/mod.rs`, `interactive/event_loop*.rs`,
`types/cell.rs`.

## Findings (17 total, all LOW/MED)

### Category 1 — Allocations in hot paths (7 items, all LOW)

- `cloud/message_draw.rs` — 7 per-frame Vec allocs (visible_border,
  pulse_factor, pulse_color, halo_factor, halo_color, alive_pulses,
  slide_cells). **Gated on `!bench_mode && !message.is_empty()`** —
  ZERO impact on S_master bench. Hoist to Cloud scratch fields if
  interactive-message-mode perf matters. **Skipped** (not bench-relevant).
- `bench/bench_io.rs:329` — per-frame Vec in production-draw bench
  variant only. **Skipped** (bench-tooling only).
- `cloud/ghost_events.rs:246` — `Box::new(GhostEvent)` per spawn.
  Rare + gated on `events_enabled && !bench_mode`. **Skipped**.

### Category 2 — Redundant computation (3 items)

- `droplet/draw.rs:328-360` — dead fog block (`FOG_MIN_FACTOR=1.0`
  disables it, but the runtime branch + factor computation still
  executed). **Fixed** — const-gated via `FOG_ENABLED` pattern
  (mirrors the existing `GLOW_ENABLED` gate at line 366).
- `droplet/draw.rs:616-628` — `vignette_lut.get().copied().unwrap_or(...)`
  defensive Option path. **Fixed** — direct index with `debug_assert`
  safety contract, empty-LUT fallback for tests/pre-resize.
- `cloud/phosphor.rs:413,430` — `frame.set` (equality-checked) on
  dead phosphor cells could be `frame.set_force`. **Skipped** —
  behavior change risk (set_force skips equality check; if ghost
  cell is non-blank, would cause visual regression).

### Category 3 — Inefficient data structures (1 item, LOW)

- `cloud/mod.rs:294` — `bottom_corner_scratch: HashSet<usize>` for
  max N=2 entries. Could be `SmallVec<[usize; 2]>`. **Skipped** —
  refactor (touches struct field + all usage sites), not mechanical,
  and LLVM likely optimizes HashSet<2> adequately.

### Category 4 — Branching inefficiency (1 item, LOW)

- Same as Category 2 fog block (dead branch). Fixed.

### Category 5 — Missing SIMD (0 items)

**SKIP** — auto-vectorization active via `target-cpu=native`. Bit-exact
parity tests in `legacy.rs:241-253, 286-301, 333-345` block manual SIMD.

### Category 6 — Inline hints (4 items, LOW/MED, LOCKED)

- `chroma/palette/mod.rs:167` — `color_to_rgb` missing `#[inline]`.
- `chroma/palette/mod.rs:276` — `blend_toward_bg` missing `#[inline]`.
- `chroma/palette/mod.rs:296` — `blend_toward_white` missing `#[inline]`.
- `chroma/shaders/base/helpers.rs:22` — `bayer_threshold` missing `#[inline]`.

**All 4 inside Phase 9-D Locked chroma engine** — UNLOCK protocol +
A/B verification required (see `chroma_dragon_engine/KEY.md`).
Fat LTO usually compensates for missing inline hints. **Skipped** —
deferred to S-master-6 (3-dragon lock task).

### Category 7 — Cache locality (0 items)

**SKIP** — all hot structures verified optimal:
- 16-byte `Cell` with niche-optimized `Option<Color>`
- SoA phosphor arrays
- SmallVec dirty indices with 256 inline slots

### Category 8 — Compiler hints (1 item, MED)

- `Cargo.toml:115` — `panic = "unwind"` → could be `"abort"` for
  ~5-10% perf gain (smaller binary, no landing pads). **Skipped** —
  MED risk: changes process semantics, must verify no `catch_unwind`
  in crossterm/notify deps. Owner mandate: "no performance regression"
  — `panic=abort` could break signal handling/error recovery.
  Deferred pending owner authorization.

All other profile settings (`opt-level=3`, `lto="fat"`,
`codegen-units=1`, `target-cpu=native` via `.cargo/config.toml`)
already at peak.

## A/B Benchmark (10s, scene=monolith)

| Size | Metric | A (before) | B (after) | Delta | Verdict |
|---|---|---|---|---|---|
| 6x6 | avg_fps | 1,551,024 | 1,554,558 | +0.23% | stable |
| 6x6 | entropy | 0.0000 | 0.0000 | +0.00% | stable |
| 6x6 | gini | 0.8333 | 0.8333 | -0.00% | stable |
| 6x6 | avg_dirty_cells | 0.6675 | 0.6678 | +0.05% | stable |
| 6x6 | total_ns_per_cell | 965.96 | 963.27 | -0.28% | stable |
| 20x20 | avg_fps | 500,713 | 493,394 | -1.46% | stable |
| 20x20 | entropy | 0.7536 | 0.7521 | -0.19% | stable |
| 20x20 | gini | 0.9165 | 0.9166 | +0.01% | stable |
| 20x20 | avg_dirty_cells | 7.9254 | 7.9348 | +0.12% | stable |
| 20x20 | total_ns_per_cell | 251.99 | 255.43 | +1.36% | stable |
| 40x20 | avg_fps | 305,345 | 304,194 | -0.38% | stable |
| 40x20 | entropy | 1.4367 | 1.4351 | -0.11% | stable |
| 40x20 | gini | 0.9358 | 0.9359 | +0.01% | stable |
| 40x20 | avg_dirty_cells | 14.2344 | 14.2319 | -0.02% | stable |
| 40x20 | total_ns_per_cell | 230.08 | 230.99 | +0.40% | stable |
| 80x24 | avg_fps | 93,225 | 93,348 | +0.13% | stable |
| 80x24 | entropy | 3.2943 | 3.2975 | +0.10% | stable |
| 80x24 | gini | 0.8961 | 0.8955 | -0.07% | stable |
| 80x24 | avg_dirty_cells | 56.8090 | 56.8250 | +0.03% | stable |
| 80x24 | total_ns_per_cell | 188.82 | 188.52 | -0.16% | stable |
| 120x40 | avg_fps | 54,084 | 53,429 | -1.21% | stable |
| 120x40 | entropy | 3.9252 | 3.9245 | -0.02% | stable |
| 120x40 | gini | 0.8943 | 0.8943 | +0.01% | stable |
| 120x40 | avg_dirty_cells | 107.2915 | 107.3006 | +0.01% | stable |
| 120x40 | total_ns_per_cell | 172.33 | 174.43 | +1.22% | stable |
| 200x60 | avg_fps | 29,836 | 29,906 | +0.23% | stable |
| 200x60 | entropy | 4.7155 | 4.7143 | -0.03% | stable |
| 200x60 | gini | 0.8903 | 0.8904 | +0.01% | stable |
| 200x60 | avg_dirty_cells | 204.9873 | 205.1464 | +0.08% | stable |
| 200x60 | total_ns_per_cell | 163.50 | 162.99 | -0.31% | stable |

**All 30 metrics within ±1.5% natural variance.** Gains are below
the bench noise floor (~3% run-to-run variance). Visual metrics
(gini, entropy) all <0.2% delta. **Zero visual or performance
regression confirmed.**

Raw JSON: `benchmark/bench-labs/S_master_dragon/S2_baseline_A.json`
and `S2_after_B.json`.

## Verdict

**Codebase confirmed post-peak-optimized.** After B1–B4 + Z5 + S1
sweeps, the bench-mode hot path (`cloud::rain_at` →
`monolith_rain::draw`/`droplet::draw` → `phosphor_decay_pass` →
`frame.clear_dirty`) is effectively allocation-free:
`alloc/frame = 0.0000054` at 80×24 = ~1 alloc per 185K frames
(noise floor).

**Changes applied (2 mechanical micro-opts):**
1. `droplet/draw.rs:327-363` — const-gate dead fog block
   (`FOG_MIN_FACTOR=1.0` disables it; was still computing the
   factor + branching every cell).
2. `droplet/draw.rs:619-645` — direct-index vignette LUT with
   debug_assert safety contract + empty-LUT fallback (was using
   `Option::get().copied().unwrap_or(...)` defensive path).

**Deferred (require owner authorization):**
- `panic = "abort"` (~5-10% gain, MED risk — changes process
  semantics, must audit deps for `catch_unwind`).
- 4 missing `#[inline]` in chroma engine (LOCKED — Phase 9-D,
  deferred to S-master-6 unlock task).

**Skipped (over-engineering for <1% gain):**
- 7 message_draw Vec hoists (gated on `!bench_mode`, 0% bench impact).
- HashSet → SmallVec refactor (not mechanical, LLVM compensates).
- phosphor `set_force` (behavior change risk).
- SIMD (parity tests block; auto-vec active).
- Cache locality (already optimal).

## Files Changed

- `src/droplet/draw.rs` — const-gate fog block + direct-index vignette LUT
- `benchmark/bench-labs/S_master_dragon/S2_*.{json,md}` — A/B data + report
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
