# Benchmarking Guide

<!-- SPDX-License-Identifier: GPL-3.0-only -->

> Independent guide to benchmarking cosmostrix: how to run, interpret,
> compare, and trust the numbers. Covers every benchmark flag, the strict
> `--bench-scene` validation contract, and the v30 4-run reference results
> (peak 102,051 FPS, avg 73,618 FPS).

## Table of Contents

1. [Quick Start](#1-quick-start)
2. [Benchmark Modes](#2-benchmark-modes)
3. [`--bench-scene` Strict Validation](#3-bench-scene-strict-validation)
4. [Reading the Report](#4-reading-the-report)
5. [v30 Reference Results (4-Run Matrix)](#5-v30-reference-results-4-run-matrix)
6. [Interpreting Key Metrics](#6-interpreting-key-metrics)
7. [Component Timing Breakdown](#7-component-timing-breakdown)
8. [Wet I/O vs Dry Benchmarking](#8-wet-io-vs-dry-benchmarking)
9. [Scaling Across Screen Sizes (`--bench-all`)](#9-scaling-across-screen-sizes-bench-all)
10. [Baseline Save & Compare](#10-baseline-save--compare)
11. [Microarchitecture & Energy (Linux only)](#11-microarchitecture--energy-linux-only)
12. [Reproducibility Checklist](#12-reproducibility-checklist)
13. [Honesty Contract](#13-honesty-contract)

---

## 1. Quick Start

```bash
# Default 5s benchmark (dry, no I/O)
target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix --benchmark

# 10s benchmark with wet I/O (writes ANSI to /dev/null)
target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix \
    --benchmark --bench-io --bench-duration 10s

# Measure the production render path (what the terminal actually sees)
target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix \
    --benchmark --bench-io --bench-scene production-draw --bench-duration 10s

# JSON output for CI/scripts
target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix --benchmark --json | jq .performance.avg_fps
```

The default benchmark runs **dry** (no I/O). It measures pure engine
throughput — how many frames the renderer can *compute* per second, not
how many frames the terminal *draws*. Real interactive FPS is bounded by
the terminal emulator, refresh rate, and ANSI output bandwidth. Use `i`
(live HUD) during a real run to see actual interactive FPS.

---

## 2. Benchmark Modes

| Flag | What it does | When to use |
|------|-------------|-------------|
| `--benchmark` | Premium 5s benchmark (2s warmup + 3s measurement). Prints FPS, frame-time percentiles, dirty-cell coverage, throughput, MEMORY (RSS), CPU %, component timing, DRIFT. | Default user-facing benchmark |
| `--bench-frames N` | Legacy CI benchmark. Runs N headless frames, prints compact `BENCH:` output. | CI pipelines, frame-count-based measurement |
| `--bench-duration N` | Override duration (1–600s). Accepts `30s`, `5m`, `1h30m`. | Endurance testing, drift/leak detection |
| `--bench-io` | Wet I/O — writes ANSI to `/dev/null`. Exercises kernel syscall path. | Measure real write bandwidth + latency |
| `--bench-scene NAME` | Select render path (lean or production-draw). Requires `--bench-io`. | Measure specific render path |
| `--bench-all` | Scaling sweep across 6×6 → 200×60. Prints SCALING SUMMARY table. | See how FPS scales with screen size |
| `--screen-size WxH` | Fixed virtual screen size. Min 4×4, max 7680×4320 in bench mode. | Benchmark at exact dimensions |
| `--save-baseline PATH` | Save JSON output to whitelist-enforced path. | Lock in regression baseline |
| `--compare-baseline PATH` | Compare current run against saved baseline. Flags >5% FPS regressions. | CI regression detection |
| `--json` | Machine-readable JSON output. | Scripts, CI, dashboards |

---

## 3. `--bench-scene` Strict Validation

`--bench-scene` is **strict**. Only two values are accepted:

| Value | What it measures |
|-------|------------------|
| `lean` (default) | The `emit_cell_lean` path — per-dirty-cell SGR emission. The fastest path cosmostrix uses in interactive mode. |
| `production-draw` | The full `Terminal::draw` redraw path — `MoveTo` per row + `ColorCache` SGR + BOLT bold escape. Mirrors what the terminal actually receives during interactive rendering. |

**Typos are rejected, not silently fallback'd.** This is part of the
cosmostrix honesty contract: no hidden flags, no hidden behavior.

```bash
# Typo → clean error, not silent fallback
$ cosmostrix --benchmark --bench-io --bench-scene leanax
error: invalid value 'leanax' for '--bench-scene <NAME>'
  [possible values: lean, production-draw]

  tip: a similar value exists: 'lean'

For more information, try '--help'.
```

### Two-Layer Validation

1. **Parse-time (clap `value_parser`)** — rejects invalid values before
   any code runs. Returns the value list + a "did you mean?" tip.
2. **Runtime (`validate_bench_scene`)** — called at the top of all 3
   benchmark entry points (`run_benchmark`, `run_premium_benchmark`,
   `run_premium_benchmark_silent`). Covers the programmatic
   `CliExplicit` path that bypasses clap.

### `production-draw` requires `--bench-io`

`--bench-scene production-draw` requires `--bench-io` to be set, because
the production draw path routes through `BenchIoWriter` (it needs the
writer to measure the full-redraw I/O cost). Without `--bench-io`, you
get a clean error explaining the dependency.

---

## 4. Reading the Report

The `--benchmark` report is organized into sections:

| Section | What it covers |
|---------|---------------|
| **SYSTEM** | CPU model, rustc version, LTO/PGO status, git SHA, target features |
| **RENDERER** | Backend, pacing, frame strategy, color depth, I/O strategy, GPU status |
| **CONFIG** | Scene, palette, charset, FPS, density, speed, terminal dimensions |
| **BENCHMARK ENVIRONMENT** | Kernel, libc, terminal, CPU governor, SMT status |
| **PERFORMANCE** | avg/peak/median FPS, frame-time percentiles (p95/p99/p99.9/max), jitter, stability |
| **THROUGHPUT** | glyphs/sec, dirty glyphs/sec, ANSI bytes/sec, active streams |
| **TIMING** | Elapsed, total frames, drawn frames, frames with changes |
| **MEMORY** | Peak/avg RSS, samples |
| **CPU** | Avg/peak CPU %, samples, basis |
| **RESOURCE** | Page faults, context switches (via `getrusage`) |
| **COMPONENT TIMING** | sim/render/io split (ms + share %) |
| **CELL EFFICIENCY** | ns per cell (render/io/total) |
| **DRIFT** | First-half vs second-half FPS, drift %, interpretation |
| **TERMINAL I/O** | Write bandwidth, latency, backpressure (wet only) |
| **ENERGY** | Total energy, avg power, energy per frame/cell (Linux RAPL) |
| **MICROARCHITECTURE** | Cycles, instructions, IPC, branch misses (Linux perf) |
| **ALLOCATOR** | Alloc/dealloc/realloc calls, bytes, heap retained |
| **VISUAL OBJECTIVE** | Frame entropy, density Gini, color transition delta |

---

## 5. v30 Reference Results (4-Run Matrix)

**Machine**: AMD Ryzen 7 5800HS, Cachyos LTS kernel 6.18.40, schedutil
governor, SMT on.
**Binary**: `v30.0.0-alpha.1`, commit `585bcac`, `pro-linux-v3` profile
(AVX2/BMI2/FMA), fat LTO, rustc 1.97.1.
**Terminal**: 88×32 auto-detected, xterm-direct, 24-bit truecolor.

### Performance Matrix

| Run | Scene | Palette | bench-scene | Duration | avg_fps | peak_fps | p99 (ms) | max (ms) |
|-----|-------|---------|-------------|----------|--------:|---------:|---------:|---------:|
| 1 | monolith | green | production-draw | 10s | 32,478 | 42,043 | 0.040 | 0.080 |
| 2 | monolith | green | lean | 10s | 69,107 | 98,619 | 0.019 | 0.629* |
| 3 | cinematic | green | lean | 10s | 12,777 | 16,018 | 0.107 | 0.155 |
| 4 | monolith | zen (neon-purple) | lean | 60s | 73,618 | 102,051 | 0.018 | 0.056 |

\* Run 2 max_frame_time 0.629ms is a single OS-scheduler outlier
(involuntary_ctxt = 524). The 60s endurance run (Run 4) confirms this
is not a real spike — max stays at 0.056ms over 4.4M frames.

### Full Metrics (Run 4 — 60s Endurance)

| Metric | Value |
|--------|------:|
| avg_fps | 73,618.0 |
| peak_fps | 102,051.2 |
| median_fps | 68,932.2 |
| avg_frame_time | 0.015ms |
| p95_frame_time | 0.017ms |
| p99_frame_time | 0.018ms |
| p99_9_frame_time | 0.023ms |
| max_frame_time | 0.056ms |
| frame_jitter | low |
| frame_time_stability | excellent |
| total_frames | 4,417,081 |
| elapsed | 60.000s |
| peak_rss | 5.4 MiB |
| avg_rss | 5.4 MiB |
| avg_cpu_percent | 99.1% |
| peak_cpu_percent | 105.0% |
| glyphs_per_second | 207,308,307 |
| dirty_glyphs_per_second | 10,654,522 |
| ansi_bytes_per_second | 202,435,918 |
| write_bandwidth | 168.1 MB/s |
| avg_write_latency | 0.3 µs |
| backpressure_events | 0 |
| total_bytes_written | 10.6 GB |
| total_energy | 1,363.53 J |
| avg_power | 22.73 W |
| energy_per_frame | 308.7 µJ |
| energy_per_cell | 2,132.9 nJ |
| cycles | 9.3B |
| instructions | 24.0B |
| IPC | 2.58 |
| branch_instructions | 3.9B |
| branch_misses | 87.57M |
| branch_mispredict_rate | 2.24% |
| alloc_calls | 346 |
| dealloc_calls | 326 |
| realloc_calls | 1,202 |
| heap_retained | 0 |
| heap_virtual | 480 KiB |
| fps_drift_percent | -1.77% (stable) |

---

## 6. Interpreting Key Metrics

### FPS

- **avg_fps**: Mean frames per second over the measurement window. The
  primary throughput number.
- **peak_fps**: Highest instantaneous FPS. Often much higher than avg
  because the diff engine can skip frames with zero dirty cells.
- **median_fps**: 50th percentile FPS. Less sensitive to outliers than
  avg. Use this when comparing across machines.
- **target_fps**: The configured cap (default 60.0). Benchmark mode
  disables the cap — this is the *uncapped* throughput.

**Headroom**: avg_fps / 60 = how many times faster than real-time. v30
lean path: 73,618 / 60 = **1,227× headroom**.

### Frame-Time Percentiles

- **avg_frame_time**: Mean time per frame (ms). Inverse of avg_fps.
- **p95 / p99 / p99.9**: Frame time at the 95th / 99th / 99.9th
  percentile. These catch tail latency — the slowest frames.
- **max_frame_time**: Worst single-frame spike. What users perceive as
  jank. v30 60s run: 0.056ms = 56 microseconds. Imperceptible.

**Stability rating**: `excellent` = p99 within 2× of avg_frame_time,
max within 5×. v30 hits `excellent` on all 4 runs.

### Dirty-Cell Ratio

The % of cells that changed between frames. This is the diff engine's
core metric — lower = more efficient (fewer cells to redraw).

- v30 monolith: ~6.13% avg dirty ratio (172.6 dirty cells / 2,816 total)
- v30 cinematic: higher (more visual activity per frame)

### Drift

`fps_drift_percent` = (first_half_fps - second_half_fps) /
first_half_fps × 100.

- **Positive drift** = FPS dropped over time (possible thermal throttle
  or memory leak).
- **Negative drift** = FPS increased over time (warmup effect, CPU
  boosting).
- **|drift| < 5%** = `stable` (no significant drift detected).
- **|drift| 5–10%** = `minor drift` (investigate).
- **|drift| > 10%** = `significant drift` (regression likely).

v30 60s run: -1.77% = stable. No thermal throttle, no memory leak.

---

## 7. Component Timing Breakdown

The renderer splits frame work into 3 components:

| Component | What it covers | v30 lean share% | v30 prod-draw share% |
|-----------|---------------|----------------:|---------------------:|
| **sim** | Atmosphere events + spawn rate + droplet physics (`cloud.rain_at` pre-render) | 64–68% | 30% |
| **render** | Phosphor decay + anomaly zones + atmospheric FX + message box (frame mutations) | 12–23% | 9% |
| **io** | `BenchIoWriter` write_frame + VisualSampler sampling + clear_dirty + loop bookkeeping | 13–20% | 62% |

**Why production-draw is I/O-heavy**: The production path mirrors
`Terminal::draw` full-redraw — it does `MoveTo` per row + `ColorCache`
SGR + BOLT bold escape for every row, even rows with no changes. This
is what the terminal actually receives in interactive mode. The lean
path only emits dirty cells, so it's I/O-light.

**ns per cell**: size-independent efficiency metric. Lower = better.

| Run | render_ns/cell | io_ns/cell | total_ns/cell |
|-----|---------------:|-----------:|--------------:|
| 1 (prod-draw) | 15.7 | 108.9 | 177.1 |
| 2 (lean) | 13.5 | 17.9 | 87.8 |
| 3 (cinematic, lean) | 31.3 | 17.7 | 132.0 |
| 4 (lean, 60s) | 11.4 | 18.4 | 92.3 |

---

## 8. Wet I/O vs Dry Benchmarking

### Dry (default, no `--bench-io`)

- Computes frames but does not write ANSI to any file descriptor.
- Measures pure engine throughput (sim + render).
- `io_share%` is near-zero (only loop bookkeeping).
- Use for: measuring engine ceiling, comparing algorithmic changes.

### Wet (`--bench-io`)

- Writes ANSI to `/dev/null` (exercises kernel syscall path without
  terminal emulator overhead).
- `io_share%` reflects real write cost.
- Use for: measuring real I/O bandwidth, latency, backpressure.
- Pair with `--bench-scene` to select which render path to exercise.

### TERMINAL I/O Section (wet only)

| Metric | What it means |
|--------|---------------|
| `write_bandwidth` | ANSI bytes/sec written to /dev/null. v30: 168–213 MB/s |
| `avg_write_latency` | Time per write syscall. v30: 0.2–0.6 µs |
| `backpressure_events` | Count of write stalls (kernel pipe full). v30: 0 across all runs |
| `effective_write_fps` | How many full-frame-equivalent writes per second. v30: 152K–165K |
| `total_bytes_written` | Total ANSI bytes over the run. v30 60s: 10.6 GB |

---

## 9. Scaling Across Screen Sizes (`--bench-all`)

```bash
target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix \
    --bench-all --bench-duration 3s --bench-io --bench-scene production-draw
```

Runs the benchmark across a fixed ladder:

| Size | Cells | Typical FPS (v15, 120×40 ref) |
|------|------:|------------------------------:|
| 6×6 | 36 | ~489,000 |
| 20×20 | 400 | ~200,000 |
| 40×20 | 800 | ~100,000 |
| 80×24 | 1,920 | ~50,000 |
| 120×40 | 4,800 | ~31,000 |
| 200×60 | 12,000 | ~12,000 |

FPS scales roughly linearly with cell count reduction (fewer cells =
less work per frame). RSS stays flat (~5 MiB) across all sizes — frame
buffers are heap-allocated once and reused.

---

## 10. Baseline Save & Compare

```bash
# Save a baseline
target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix \
    --benchmark --json --save-baseline /tmp/base.json

# ... make a change ...

# Compare against the baseline
target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix \
    --benchmark --compare-baseline /tmp/base.json
```

The comparison flags >5% FPS regressions with a PASS/FAIL verdict. Use
in CI to catch performance regressions before they ship.

**Path whitelist**: `--save-baseline` only accepts paths under the
current directory, `/tmp/`, or `$XDG_CACHE_HOME/cosmostrix/`. This
prevents arbitrary file writes.

---

## 11. Microarchitecture & Energy (Linux only)

Two additional report sections require elevated privileges:

### MICROARCHITECTURE (`perf_event_open`)

Requires `perf_event_open` syscall access. On most Linux distros:

```bash
# Option 1: run as root (quick test)
sudo target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix --benchmark

# Option 2: set perf_event_paranoid (persistent)
echo 1 | sudo tee /proc/sys/kernel/perf_event_paranoid
```

Reports: cycles, instructions, IPC, branch instructions, branch misses,
branch mispredict rate.

**Interpretation**:
- **IPC > 2.0** = healthy instruction throughput. v30: 2.53–3.14.
- **IPC > 3.0** = excellent (working set fits in L1, branch predictor
  is hot).
- **Branch mispredict < 2%** = BOLT branchless lookup tables working.
  v30: 0.57–2.41%.

### ENERGY (RAPL powercap sysfs)

Requires read access to `/sys/class/powercap/intel-rapl/`. On most
Linux distros:

```bash
# Option 1: run as root
sudo target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix --benchmark

# Option 2: grant read access (persistent)
sudo chmod -R a+r /sys/class/powercap/intel-rapl/
```

Reports: total energy (J), avg power (W), energy per frame (µJ),
energy per cell (nJ).

**Interpretation**:
- **energy_per_frame** = how much energy one frame costs. Lower =
  more efficient. v30 lean: 309–387 µJ.
- **energy_per_cell** = size-independent energy metric. v30 lean:
  2,133–2,388 nJ.

See [docs/BENCHMARK_ADVANCED.md](BENCHMARK_ADVANCED.md) for the full
guide on enabling these sections.

---

## 12. Reproducibility Checklist

Benchmark numbers are **machine-dependent**. For reproducible comparisons:

- [ ] **Same machine** — CPU model, core count, cache size.
- [ ] **Same kernel** — version + scheduler config (governor, SMT).
- [ ] **Same build profile** — `pro-linux-v3`, `pro-linux-v4`, etc.
- [ ] **Same rustc version** — recorded in the SYSTEM section.
- [ ] **Same terminal size** — use `--screen-size WxH` for exact dims.
- [ ] **Same scene + palette** — scene choice is the biggest FPS lever.
- [ ] **Same bench-scene** — lean vs production-draw differ by ~2×.
- [ ] **Same duration** — longer runs are more stable but slower.
- [ ] **Same CPU governor** — `schedutil` vs `performance` affects
  results by 10–20%.
- [ ] **Same SMT state** — SMT on/off affects single-core throughput.
- [ ] **Close other apps** — browser tabs, IDEs, etc. steal CPU cycles.
- [ ] **Wait for warmup** — the 2s warmup phase is automatic; for
  manual runs, discard the first 2s of data.

The SYSTEM + BENCHMARK ENVIRONMENT sections record all of these so
reports are self-documenting for cross-machine comparison.

---

## 13. Honesty Contract

Cosmostrix is honest. No hidden flags, no hidden behavior.

- **All flags are documented** in `--help` and `--help-detail`.
- **All `--bench-scene` values are strict-validated** — typos are
  rejected, not silently fallback'd.
- **`production-draw` requires `--bench-io`** — the dependency is
  enforced with a clean error, not a silent no-op.
- **Benchmark numbers are machine-dependent** — the report records the
  full environment (CPU, kernel, governor, SMT, rustc, LTO/PGO) so you
  can verify reproducibility.
- **No FPS number is a release promise** — the 50k/70k/100k FPS lab
  targets are headless ceilings on specific hardware, not portable
  guarantees. Real interactive FPS is terminal-bounded.
- **GPU usage is `not_applicable`** — cosmostrix is a CPU + stdout
  renderer. No GPU context is ever created. This is by design.

If a benchmark number looks too good to be true, check the SYSTEM and
BENCHMARK ENVIRONMENT sections. The report tells you exactly what
hardware, software, and configuration produced the number.

---

## See Also

- [benchmark/README.md](../benchmark/README.md) — Full reference results
  across versions (v15, v30) and comparison vs other Matrix rain tools
- [docs/BENCHMARK_ADVANCED.md](BENCHMARK_ADVANCED.md) — Enabling
  MICROARCHITECTURE and ENERGY metrics (Linux perf + RAPL)
- [docs/RAIN_DEPTH_AUDIT.md](RAIN_DEPTH_AUDIT.md) — Visual-audit
  methodology using `--bench-scene production-draw`
- [docs/RENDER_ENGINE.md](RENDER_ENGINE.md) — Formal architecture spec
  of the diff-based rendering engine
- [docs/PERFORMANCE_ACROSS_SCALES.md](PERFORMANCE_ACROSS_SCALES.md) —
  How FPS scales with screen size
- [docs/ENDURANCE.md](ENDURANCE.md) — Long-run endurance testing and
  resource monitoring
- [docs/RELEASE_GUARD.md](RELEASE_GUARD.md) — Performance regression
  gates for releases
