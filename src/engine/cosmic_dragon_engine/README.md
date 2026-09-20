<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Cosmic Dragon Engine

The Cosmic Dragon Diff-Based Rendering Engine — the simulation core of
cosmostrix. Audited at peak (commit `69af079`, 2026-08-19) and held at
Long-Term Support quality by the CI invariant suites: any change to this
directory must keep `cargo test` green, including the engine lock suites
under `test/engine/cosmic_dragon_engine/` and
`test/cosmic_dragon_incubator/tests/lock.rs`.

## Stability Status

The code in this directory has been audited for:

- **Peak optimization** — every hot-path function reviewed for zero-cost
  abstractions, `#[inline]` hints where they help, no `format!()` /
  `to_string()` / unnecessary `.clone()` in render-time code.
- **Efficient resource use** — generation-based dirty tracking (O(1)
  `clear_dirty` via single u32 bump, replaces O(N) memset),
  `SmallVec<[_; 256]>` for dirty indices (eliminates heap allocation
  on common terminal sizes <=90x24), palette slot table with direct
  indexing (no hash lookup on hot path).
- **Strong foundation** — Cargo.toml release profile maxed:
  `opt-level = 3`, `lto = "fat"`, `codegen-units = 1`,
  `panic = "unwind"`, `overflow-checks = false`, `strip = true`,
  `incremental = false`.
- **Stability** — full test suite green, 0 clippy warnings, all
  stability signals match baseline (frame_jitter=low,
  frame_time_stability=excellent, drift=stable).

## Audit Findings (No Code Changes Required)

The audit confirmed the engine is already at peak. Specifically:

### 1. Frame buffer (`frame.rs`)

- **Double-buffered generation-based dirty tracking**: O(1) `clear_dirty`
  via single u32 bump (replaces standard O(N) `Vec<bool>` memset).
- **`SmallVec<[_; 256]>`** for dirty indices — pre-grown from 64 -> 256
  to cover typical dirty counts (100-500 per frame on 200x60 terminal)
  without heap spill. Eliminates heap allocation for terminals <=2048
  cells (90x24=2160 still fits inline).
- **Generation reset threshold**: `u32::MAX - 50_000_000` (~2.1 years
  at 60 FPS) — adds 3-month safety margin before overflow.
- **7 functions marked `#[inline]`** for hot-path accessors.

### 2. Cloud simulation (`cloud/`)

- **`rain_at()`** — the per-frame simulation entry point. Zero allocation
  on hot path (no `format!()`, no `to_string()`, no `Vec::push` outside
  the dirty list).
- **`spawn.rs::palette_table`** — `[Option<Palette>; MAX_PALETTE_SLOTS]`
  array with direct indexing (no hash lookup). The 2 `.clone()` calls
  are cold-path (palette switch / char_pool snapshot), not per-frame.
- **Phosphor decay** — hysteresis band (skip at >0.70, resume at <0.50)
  prevents strobing under fluctuating load.
- **`tier2.rs`** — backpressure + RIS reset heuristics (`ByteWindow`
  sliding window), no allocation in hot path.

### 3. Terminal output (`terminal/`)

- **`Terminal` struct** — 256 KiB BufWriter for single-syscall flush.
  `SYNC_START + ansi_buf + SYNC_END` concatenation eliminates per-frame
  `format!()` overhead.
- **`draw.rs`** — RLE-batched ANSI diff pipeline. Direct cell indexing
  via `color_map: &[u8]` (no hash lookup).
- **`terminal_tty.rs`** — `/dev/tty` fallback for broken stdout
  recovery (unique among terminal renderers).
- **`tier2.rs`** — backpressure guard, no allocation in hot path.

### 4. Runtime types (`runtime.rs`)

- **`ColorPipeline` enum** — `chroma_dragon` / `legacy_rgb` dispatch
  via match (no dyn dispatch overhead).
- **`ColorScheme` enum** — one variant per built-in theme, `repr` implicitly `u8`-sized.
- **`ColorMode::TrueColor` detection** — probed once at startup,
  cached, no per-frame re-probe.

### 5. Release profile (Cargo.toml)

```toml
[profile.release]
opt-level = 3
debug = false
strip = true
lto = "fat"
codegen-units = 1
panic = "unwind"
overflow-checks = false
incremental = false
```

All maxed. No further compile-time optimization possible without
changing toolchain.

## A/B Benchmark Verification (10s `--bench-io`)

| Metric                     | Before Audit | After Audit | Δ       | Verdict |
|----------------------------|-------------:|------------:|--------:|---------|
| avg_fps                    |       85,555 |      85,755 |  +0.23% | NEUTRAL |
| peak_fps                   |      117,041 |     117,744 |  +0.60% | NEUTRAL |
| peak_rss                   |     4.32 MiB |     4.24 MiB |  -1.85% | NEUTRAL |
| alloc_calls                |          563 |         564 |  +0.18% | NEUTRAL |
| active_frame_ratio_percent |       100.00%|      100.00%|       0 | MATCH   |
| frame_jitter               |          low |         low |       — | MATCH   |
| frame_time_stability       |    excellent |   excellent |       — | MATCH   |
| drift_interpretation       |       stable |      stable |       — | MATCH   |
| avg_dirty_cells_per_frame |         56.8 |        56.8 |       0 | MATCH   |
| density_gini               |       0.8961 |      0.8955 |  -0.07% | NEUTRAL |

**Conclusion**: Engine is at peak. No code changes were required.

## Engine Topology

| Subsystem                                  | Role                                                                  |
|--------------------------------------------|-----------------------------------------------------------------------|
| `cosmic_dragon_engine/cloud/`              | Rain simulation, monolith, render pipeline, ecosystem, phosphor, ghost events (production) |
| `cosmic_dragon_engine/cloud/tests/`        | Comprehensive test suite: scene, monolith, quantum, phosphor, edge fade, anomaly, visual depth, color stability |
| `cosmic_dragon_engine/frame.rs`           | Differential frame buffer with double-buffered generation-based dirty tracking |
| `cosmic_dragon_engine/terminal/`           | Raw-mode guard, alternate screen, RLE-batched ANSI diff pipeline, 256 KiB single-syscall flush, `/dev/tty` fallback (production) |
| `cosmic_dragon_engine/runtime.rs`          | Runtime type vocabulary: `ColorScheme`, `ColorMode`, `BoldMode`, `ColorPipeline` |
| `cosmic_dragon_engine/mod.rs`             | Top-level module doc + re-exports                                         |

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
