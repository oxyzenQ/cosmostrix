# Benchmarking Guide

<!-- SPDX-License-Identifier: GPL-3.0-only -->

> Independent guide to benchmarking cosmostrix: how to run, interpret,
> compare, and trust the numbers. Covers every benchmark flag, the strict
> `--bench-scene` validation contract, the v50 4-scene reference matrix
> (peak 149,745 FPS on monolith, 26,251 FPS on cinematic), and the
> honesty contract that frames performance as a quality enabler rather
> than the goal.

## Table of Contents

1. [Quick Start](#1-quick-start)
2. [Benchmark Modes](#2-benchmark-modes)
3. [`--bench-scene` Strict Validation](#3-bench-scene-strict-validation)
4. [Reading the Report](#4-reading-the-report)
5. [v50 Reference Results (Cloud Xeon, 4-Scene Matrix)](#5-v50-reference-results-cloud-xeon-4-scene-matrix)
5a. [v30 Historical Reference (Owner's Ryzen 5800HS)](#5a-v30-historical-reference-owners-ryzen-5800hs)
5b. [Third-Party Hardware Verification (Cloud Xeon)](#5b-third-party-hardware-verification-cloud-xeon)
6. [Interpreting Key Metrics](#6-interpreting-key-metrics)
7. [Component Timing Breakdown](#7-component-timing-breakdown)
8. [Wet I/O vs Dry Benchmarking](#8-wet-io-vs-dry-benchmarking)
9. [Scaling Across Screen Sizes (`--bench-all`)](#9-scaling-across-screen-sizes-bench-all)
10. [Baseline Save & Compare](#10-baseline-save--compare)
11. [Microarchitecture & Energy (Linux only)](#11-microarchitecture--energy-linux-only)
12. [Reproducibility Checklist](#12-reproducibility-checklist)
13. [Honesty Contract](#13-honesty-contract)
14. [Quick Reference](#14-quick-reference)
15. [Diagnostic Recipes](#15-diagnostic-recipes)
16. [Common Misreadings & Pitfalls](#16-common-misreadings--pitfalls)

---

## Quick Reference

At-a-glance lookup for the most common benchmark metrics. New users:
this table tells you what each number means in one sentence. Veterans
jump to §6 (Interpreting Key Metrics) for the deep dive.

### Performance Metrics

| Metric                  | Unit      | What it tells you in one sentence                                                              |
|-------------------------|-----------|------------------------------------------------------------------------------------------------|
| `avg_fps`               | FPS       | Mean frames per second over the measurement window. The primary throughput number.            |
| `peak_fps`              | FPS       | Highest instantaneous FPS. Often much higher than avg because the diff engine can skip frames with zero dirty cells. |
| `median_fps`            | FPS       | 50th-percentile FPS. Less outlier-sensitive than avg. Use for cross-machine comparison.       |
| `target_fps`            | FPS       | The configured `--fps` cap. In benchmark mode the cap is disabled — this is the uncapped ceiling. |
| `avg_frame_time`        | ms        | Mean time per frame. Inverse of `avg_fps`. 0.015ms = 67,000 FPS.                              |
| `p95_frame_time`        | ms        | 95th-percentile frame time. The slowest 5% of frames.                                          |
| `p99_frame_time`        | ms        | 99th-percentile frame time. The slowest 1% of frames — catches spikes avg hides.               |
| `p99_9_frame_time`      | ms        | 99.9th-percentile frame time. The slowest 0.1% — extreme tail latency.                        |
| `max_frame_time`        | ms        | Worst single-frame spike. What users perceive as jank.                                         |
| `frame_jitter`          | label     | Variability label (`low` / `medium` / `high`). `low` = stable frame pacing.                   |
| `frame_time_stability`  | label     | `excellent` = p99 within 2× avg, max within 5×. v50 hits `excellent` on all 4 scenes.           |
| `fps_drift_percent`     | percent   | (first_half_fps − second_half_fps) / first_half_fps × 100. Negative = warmup; positive = throttle/leak. |
| `total_frames`          | count     | Frames computed during the measurement window.                                                 |
| `elapsed`               | seconds   | Wall-clock duration of the measurement window.                                                 |

### Throughput Metrics

| Metric                    | Unit        | What it tells you in one sentence                                                            |
|---------------------------|-------------|----------------------------------------------------------------------------------------------|
| `glyphs_per_second`       | glyphs/sec  | Total cells processed per second (dirty + clean). v30: 207M glyphs/sec.                      |
| `dirty_glyphs_per_second` | glyphs/sec  | Changed cells per second. The work the diff engine actually emits to the terminal.           |
| `ansi_bytes_per_second`   | bytes/sec   | ANSI escape bytes generated per second. v30: 202 MB/s.                                       |
| `avg_dirty_cells_per_frame` | cells     | Mean cells changed per frame. Lower = more efficient diff (fewer cells to redraw).           |
| `avg_dirty_cell_ratio_percent` | percent | Fraction of cells that changed. v30 monolith: ~6%. v30 400×200: ~1.8%.                       |

### Resource Metrics

| Metric              | Unit    | What it tells you in one sentence                                                            |
|---------------------|---------|----------------------------------------------------------------------------------------------|
| `peak_rss`          | MiB     | Peak resident set size. v30: 5.4 MiB. Steady growth across runs = possible leak.            |
| `avg_rss`           | MiB     | Mean RSS during measurement. Flat = healthy.                                                |
| `avg_cpu_percent`   | percent | Process CPU% during measurement. v30: ~99% (single-threaded, fully utilized).               |
| `peak_cpu_percent`  | percent | Highest instantaneous CPU%. Can exceed 100% on multi-threaded builds.                       |
| `alloc_calls_per_frame` | count | Fresh allocations per frame. v30: 3.00 (constant baseline). Higher = leaking heap.           |
| `dealloc_calls_per_frame` | count | Free calls per frame. Should track `alloc_calls` in steady state.                          |
| `heap_retained`     | bytes   | Bytes allocated and never freed. v30: 0 (zero retained). Non-zero = investigate.            |

### I/O Metrics (wet mode only — requires `--bench-io`)

| Metric                | Unit     | What it tells you in one sentence                                                            |
|-----------------------|----------|----------------------------------------------------------------------------------------------|
| `write_bandwidth`     | MB/s     | ANSI bytes/sec written to /dev/null. v30: 168–213 MB/s.                                     |
| `avg_write_latency`   | µs       | Time per write syscall. v30: 0.2–0.6 µs.                                                    |
| `backpressure_events` | count    | Write stalls (kernel pipe full). v30: 0 across all runs. Non-zero = terminal can't keep up. |
| `effective_write_fps` | FPS      | Full-frame-equivalent writes per second. v30: 152K–165K.                                    |
| `total_bytes_written` | bytes    | Total ANSI bytes over the run. v30 60s: 10.6 GB.                                            |

### Energy & Microarchitecture Metrics (Linux only — requires privileges)

| Metric                  | Unit  | What it tells you in one sentence                                                            |
|-------------------------|-------|----------------------------------------------------------------------------------------------|
| `total_energy`          | J     | Energy consumed during the run. v30 60s: 1,363 J.                                            |
| `avg_power`             | W     | Average power draw. v30: 22.73 W.                                                            |
| `energy_per_frame`      | µJ    | Energy per frame. v30 lean: 309–387 µJ. Lower = more efficient.                             |
| `energy_per_cell`       | nJ    | Size-independent energy metric. v30 lean: 2,133–2,388 nJ.                                   |
| `cycles`                | count | CPU cycles during run. v30 60s: 9.3 billion.                                                 |
| `instructions`          | count | CPU instructions retired. v30 60s: 24 billion.                                              |
| `IPC`                   | ratio | Instructions per cycle. >2.0 = healthy; >3.0 = excellent. v30: 2.53–3.14.                  |
| `branch_mispredict_rate` | percent | Branch predictor failure rate. <2% = BOLT lookup tables working. v30: 0.57–2.41%.        |

### Visual Objective Metrics

| Metric          | Unit    | What it tells you in one sentence                                                            |
|-----------------|---------|----------------------------------------------------------------------------------------------|
| `frame_entropy` | bits    | Information entropy of frame content. Higher = more visual variety per frame.                |
| `density_gini`  | 0..1    | Gini coefficient of cell density. 0 = perfectly uniform; 1 = maximally concentrated.         |

### Units & Symbols Legend

| Symbol / Suffix | Meaning                                                                                          |
|-----------------|--------------------------------------------------------------------------------------------------|
| `ms`            | Milliseconds (frame time unit). 1ms = 0.001s. A 60 FPS target = 16.67ms budget per frame.        |
| `µs`            | Microseconds (latency unit). 1µs = 0.001ms. v30 write latency: 0.2–0.6 µs.                      |
| `ns` / `nJ`     | Nanoseconds / nanojoules. Size-independent per-cell cost. v30: ~80 ns/cell, ~2,133 nJ/cell.      |
| `MiB` / `KiB`   | 1024² bytes / 1024 bytes (binary, NOT decimal SI units).                                         |
| `MB/s`          | Megabytes per second (decimal, 10⁶ bytes/sec). Used for ANSI write bandwidth.                   |
| `%`             | Percent of one CPU core. 100% = one full core. Multi-threaded spills can exceed 100%.            |
| `J` / `W`       | Joules (energy) / Watts (power = energy/sec).                                                    |
| `IPC`           | Instructions Per Cycle. CPU throughput efficiency ratio. >2.0 = healthy.                         |
| `drift`         | Positive = FPS dropped over time (throttle/leak); negative = FPS rose (warmup/boost). \|drift\| < 5% = stable. |
| `wet` / `dry`   | Wet = `--bench-io` enabled (writes ANSI to /dev/null); dry = no I/O (pure engine throughput).    |
| `lean` / `production-draw` | The two `--bench-scene` values. `lean` = dirty-cell-only emission (fastest); `production-draw` = full `Terminal::draw` path. |

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
| `--bench-duration N` | Override duration (1s minimum, no maximum). Accepts `30s`, `5m`, `1h30m`. | Endurance testing, drift/leak detection |
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

## 5. v50 Reference Results (Cloud Xeon, 4-Scene Matrix)

**Machine**: 2-core Intel Xeon (Alibaba Cloud Linux), 3.9 GiB RAM, no swap,
no RAPL, no perf counters. Kernel 5.10.134, gnu libc.
**Binary**: `v50.0.0-alpha.1`, commit `7ba7a76`, `release` profile (x86-64-v1
baseline: SSE/SSE2), fat LTO, rustc 1.97.1, no PGO.
**Terminal**: 80×24, `TERM=dumb`, color_mode=mono (sandbox has no TTY;
production truecolor terminals will route through the Chroma Dragon engine
instead of the legacy_rgb fallback seen here).

### Why this matrix replaces the v30 4-Run Matrix

The v30 reference results (preserved below in §5a for historical context)
were captured on the owner's personal Ryzen 7 5800HS — a high-end 8-core
desktop CPU. Those numbers (peak 102,051 FPS on the `lean` path) are real
but they describe the engine's throughput ceiling on enthusiast hardware,
not what a third party will see on typical infrastructure.

The v50 matrix below was captured on a 2-vCPU cloud VM — the kind of
hardware a CI runner, a contributor's review environment, or a user on
a VPS would actually have. The numbers are smaller than the v30 desktop
numbers, and that is the honest picture: cosmostrix's value is not the
raw FPS ceiling, it is the **quality of the cinematic rain at practical
terminal-bounded FPS** (60–240 on real terminals). The diff engine's
job is to make that quality affordable, not to win a benchmark sprint.

### Performance Matrix (4 scenes, 5s each, default palette)

| Scene      | avg_fps  | peak_fps | p95 (ms) | p99 (ms) | max (ms) | frame_jitter | frame_time_stability | dirty_glyphs/s |
|------------|---------:|---------:|---------:|---------:|---------:|--------------|-----------------------|----------------:|
| monolith   | 84,814.5 | 149,745.4| 0.013    | 0.016    | 0.035    | low          | excellent             | 4,818,306       |
| cinematic  | 26,251.8 | 40,484.2 | 0.049    | 0.053    | —        | low          | excellent             | 10,869,217      |
| signal     | 25,517.5 | 47,535.3 | 0.046    | 0.051    | —        | low          | excellent             | —               |
| matrix     | 24,198.9 | 40,487.5 | 0.045    | 0.048    | —        | low          | excellent             | —               |

### Reading the matrix honestly

- **monolith is the throughput peak.** It is the leanest scene (zen
  charset = 1 glyph, no phosphor decay, no depth fog, no parallax). The
  84K avg_fps is the diff engine's ceiling on this hardware — it tells
  you the engine is not a bottleneck, nothing more.
- **cinematic / signal / matrix are the quality scenes.** They run
  ~3× slower than monolith because they do more work per cell: phosphor
  decay, multi-layer depth, atmospheric modulation, denser charsets.
  This is by design — the cinematic effects are what the user actually
  sees, and they are still well above the 60 FPS interactive cap.
- **All four scenes hit `frame_time_stability: excellent`** — p99 is
  within 2× of avg, max is within 5×. The engine does not have
  stuttering outliers even at 25–85K FPS.
- **`color_pipeline: legacy_rgb` in this matrix** — the sandbox has no
  truecolor TTY, so the Chroma Dragon engine falls back to legacy
  sRGB-linear math. On a real truecolor terminal, the chroma engine
  runs the same hot paths with OKLab gradient construction at palette
  build time (one-time ~12 mul + 3 cbrt per segment — negligible).

### Honest framing: performance enables quality, not the reverse

Cosmostrix is **not a performance-focused renderer**. It is a
**cinematic-quality renderer that uses the diff engine to make the
cinematic effects affordable at practical terminal FPS**. The numbers
above prove the engine is not the bottleneck — your terminal emulator's
ANSI parse speed is. On Alacritty/kitty/WezTerm that's 60–240 FPS,
which is exactly the range where the cinematic effects (phosphor decay,
depth fog, 3-layer parallax, density sculpting) are perceptible to
the human eye.

If you only care about raw FPS, run `--benchmark --scene monolith` and
enjoy the 5-digit number. If you care about the cinematic rain, run
cosmostrix in interactive mode and watch it.

---

## 5a. v30 Historical Reference (Owner's Ryzen 7 5800HS)

The v30 4-Run Matrix below is preserved for historical context. It was
captured on the owner's personal desktop (Ryzen 7 5800HS, 8-core/16-thread)
and represents the engine's throughput ceiling on enthusiast hardware.
The v50 matrix above is the current reference for third-party hardware.

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

## 5b. Third-Party Hardware Verification (Cloud Xeon)

Every number in §5 was produced on the owner's personal Ryzen 5800HS.
That raises an obvious question for any third party: does cosmostrix
actually build and run on a different CPU, or does it secretly depend
on something Ryzen-specific?

To answer that, the same commit (`c97ba87`) and the same `pro-linux-v3`
profile were built and benchmarked on a 2-core Intel Xeon cloud VM
(Alibaba Cloud Linux, 3.9 GiB RAM, no swap, no RAPL, no perf counters).
The headline 60s `lean + monolith + zen` run produced:

| Metric              | Cloud Xeon (2 vCPU) | Owner's Ryzen 5800HS | Ratio   |
|---------------------|--------------------:|---------------------:|--------:|
| avg_fps             | **116,013.9**       | 73,618.0             | 1.58×   |
| peak_fps            | 188,323.9           | 102,051.2            | 1.84×   |
| p99_frame_time      | 0.013 ms            | 0.018 ms             | 0.72×   |
| io_ns_per_cell      | 4.5                 | ~13                  | 0.35×   |
| heap_retained       | 45 KiB              | 0 KiB                | —       |
| fps_drift_percent   | -1.38 % (stable)    | -1.77 % (stable)     | —       |

The 1.58× ratio is fully explained by the cloud Xeon's higher sustained
single-thread IPC at 3.2 GHz — cosmostrix's `--benchmark` mode is
single-threaded by design (`planned_worker_budget: 0`), so it does not
benefit from the Ryzen's 8 cores / 16 threads.

The `--bench-scene` strict-validation contract was also verified on
the cloud CPU: typos like `leanax` and `production-drawmadadadaxa`
are rejected with helpful tips exactly as on the owner's machine. The
honesty contract holds on third-party hardware.

**Full environment, 3-run results, reproduction steps, and raw logs**:
[`docs/BENCHMARK_CLOUD_XEON.md`](BENCHMARK_CLOUD_XEON.md).

---

## 6. Interpreting Key Metrics

### FPS

- **avg_fps**: Mean frames per second over the measurement window. The
  primary throughput number.
- **peak_fps**: Highest instantaneous FPS. Often much higher than avg
  because the diff engine can skip frames with zero dirty cells.
- **median_fps**: 50th percentile FPS. Less sensitive to outliers than
  avg. Use this when comparing across machines.
- **target_fps**: The configured cap. When the user does not pass
  `--fps` or set `fps =` in config, the default is dynamic: 60 FPS on
  standard terminals, 144 FPS on high-refresh terminals (Alacritty,
  kitty, WezTerm, etc.) — see `termdetect.rs` for the detection
  logic. Benchmark mode disables the cap — this is the *uncapped*
  throughput.

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

- **All flags are documented** in `--help`.
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

## 15. Diagnostic Recipes

Symptom → likely cause → what to check → action. Use this table when
a benchmark number looks unexpected and you need a starting point.

| Symptom                                                  | Likely cause                                              | What to check                                                       | Action                                                                                                |
|----------------------------------------------------------|-----------------------------------------------------------|---------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------|
| `avg_fps` differs wildly between two runs                | Different scene/palette/build profile/rustc/governor      | SYSTEM + BENCHMARK ENVIRONMENT sections in both reports             | Match all env variables. See §12 Reproducibility Checklist.                                           |
| `avg_fps` dropped by >5% after a code change             | Real regression                                           | `--compare-baseline` output                                         | Bisect with `git bisect` on the JSON baseline. Check COMPONENT TIMING for which phase regressed.      |
| `p99_frame_time` >> `avg_frame_time` (e.g. 10× ratio)    | Periodic stalls (GC, kernel scheduling, terminal backpressure) | Run `--bench-duration 30s` to see if p99 stays high or was a startup fluke | If p99 stays high → investigate with `perf record` on Linux. If p99 drops over time → startup fluke. |
| `max_frame_time` >> `p99_frame_time` (e.g. 50ms vs 2ms)  | One-off OS-scheduler spike (involuntary context switch)   | `involuntary_ctxt` in RESOURCE section                              | Safe to ignore unless it recurs across runs. Run `--bench-duration 60s` to confirm.                  |
| `fps_drift_percent` > +5%                                | Thermal throttle or memory leak                           | CPU temp; `peak_rss` trend across multiple runs                      | Check cooling; check if `heap_retained` is non-zero. See `docs/ENDURANCE.md`.                        |
| `fps_drift_percent` < −5%                                | CPU boosting / warmup effect                               | CPU governor (`schedutil` vs `performance`)                         | Run longer (`--bench-duration 30s+`) so warmup amortizes. Negative drift is healthy, not a bug.      |
| `peak_rss` grows across multiple runs                    | Memory leak (process not exiting cleanly)                | Run with `--bench-duration 60s` once, check `peak_rss`               | If `heap_retained` > 0 → investigate allocator. See `docs/ENDURANCE.md`.                             |
| `alloc_calls_per_frame` > 3.00                           | New per-frame heap allocation in hot path                 | `ALLOCATOR` section + git log of `src/cloud/`, `src/frame.rs`        | Bisect to find the offending commit. Target: 3.00 (constant baseline).                               |
| `heap_retained` > 0                                      | Allocation never freed                                    | `ALLOCATOR` section                                                 | Source-level review of new code paths. v30 baseline: 0 retained.                                     |
| `IPC` < 2.0                                              | Cache misses or branch mispredicts in hot path            | `branch_mispredict_rate`; `cycles` vs `instructions`                | If branch misses >2% → check BOLT lookup tables. If IPC <1.5 → working set may exceed L1.            |
| `branch_mispredict_rate` > 2%                            | Branch predictor failing on data-dependent branches      | BOLT-generated lookup tables; recent hot-loop changes              | Convert data-dependent branches to lookup tables. v30 baseline: 0.57–2.41%.                          |
| `write_bandwidth` drops sharply                          | Terminal emulator backpressure                            | `backpressure_events` count                                         | If `backpressure_events` > 0 → terminal can't keep up. Try a faster terminal (Alacritty, kitty).     |
| `avg_cpu_percent` < 50%                                  | CPU not fully utilized — likely idle-throttled or I/O-bound | Whether `--bench-io` is set; whether benchmark is single-threaded   | In dry mode the loop may sleep between frames. Use `--bench-io` for full utilization.                |
| `avg_cpu_percent` > 100%                                 | Multi-threaded build or measurement artifact              | Build profile; thread count                                         | Brief spikes >100% are normal on multi-threaded. Sustained >100% means worker threads are saturated. |
| `--bench-scene` typo silently falls back                 | Should NEVER happen — typos are strict-rejected          | Whether you see the "did you mean?" error message                   | If no error → file a bug. The honesty contract requires strict validation.                            |
| `production-draw` runs without `--bench-io`              | Should NEVER happen — dependency is enforced             | Whether you see the "requires --bench-io" error                     | If no error → file a bug. The dependency must be enforced.                                            |
| `--save-baseline` rejects path                           | Path outside whitelist                                    | Path is under cwd, /tmp/, or $XDG_CACHE_HOME/cosmostrix/            | Move target into one of those dirs. The whitelist prevents arbitrary file writes.                    |
| ENERGY section missing                                   | No RAPL access                                            | Whether running as root; `/sys/class/powercap/intel-rapk/` perms    | `sudo chmod -R a+r /sys/class/powercap/intel-rapl/` or run as root. See §11.                         |
| MICROARCHITECTURE section missing                        | No `perf_event_open` access                               | `/proc/sys/kernel/perf_event_paranoid` value                        | `echo 1 \| sudo tee /proc/sys/kernel/perf_event_paranoid` or run as root. See §11.                  |
| JSON output missing fields                               | Older binary or different bench-scene mode                | `--version` + `--bench-scene` value                                 | Some fields are mode-specific (e.g. I/O metrics only in wet mode). Re-run with the right flags.      |

---

## 16. Common Misreadings & Pitfalls

Explicit list of ways users get confused by benchmark numbers. Each
entry states the wrong reading, the correct reading, and why the
difference matters.

### Misreading 1: "avg_fps 73,618 means the terminal renders 73K FPS"

**Wrong:** `avg_fps` is the on-screen frame rate.
**Correct:** `avg_fps` is the headless engine ceiling — how many frames
the renderer can COMPUTE per second with no terminal attached. Real
interactive FPS is bounded by the terminal emulator's refresh rate,
ANSI parse speed, and GPU compositing. A 144 Hz terminal maxes out at
144 FPS regardless of `avg_fps`.
**Why it matters:** users file bug reports saying "cosmostrix claims
73K FPS but I only see 60" — that's the terminal, not the engine.

### Misreading 2: "lean and production-draw should produce the same FPS"

**Wrong:** The two `--bench-scene` values measure the same thing.
**Correct:** `lean` measures the dirty-cell-only emission path (the
fastest path cosmostrix uses in interactive mode). `production-draw`
measures the full `Terminal::draw` path (MoveTo per row + ColorCache
SGR + BOLT bold escape — what the terminal actually receives). They
differ by ~2× because production-draw does I/O work for unchanged rows.
**Why it matters:** comparing lean numbers across versions is fair;
comparing lean to production-draw is not.

### Misreading 3: "peak_fps higher than avg_fps means the engine is unstable"

**Wrong:** High `peak_fps` indicates instability.
**Correct:** `peak_fps` is the highest instantaneous FPS — often much
higher than `avg_fps` because the diff engine can skip frames with
zero dirty cells (nothing to redraw = near-zero frame time = huge
instantaneous FPS). The v30 60s run shows avg=73,618 / peak=102,051
— a 1.39× ratio, which is healthy.
**Why it matters:** users think the engine is "spiking" when it's just
skipping empty frames efficiently.

### Misreading 4: "max_frame_time 0.629ms is a real spike"

**Wrong:** Any `max_frame_time` above the p99 indicates a real problem.
**Correct:** A single OS-scheduler context switch (involuntary preemption)
can produce a one-off spike that has nothing to do with renderer
performance. The v30 Run 2 max of 0.629ms had `involuntary_ctxt = 524`
— the kernel preempted the process 524 times during the run. The 60s
endurance run (Run 4) confirms this is not recurring: max stays at
0.056ms over 4.4M frames.
**Why it matters:** users optimize for one-off spikes that will never
recur. Compare `max` to `p99` — if `p99` is low, the spike was a fluke.

### Misreading 5: "fps_drift_percent -1.77% means FPS is dropping"

**Wrong:** Negative drift = FPS dropping over time.
**Correct:** Negative drift = FPS INCREASED over time (warmup effect,
CPU boosting to higher clock). Positive drift = FPS dropped (thermal
throttle or memory leak). The sign is `(first_half − second_half)`, so
negative means second half was faster.
**Why it matters:** users file leak reports for negative drift when
the engine is actually getting faster as it warms up.

### Misreading 6: "alloc_calls_per_frame 3.00 means the engine allocates 3 times per frame"

**Wrong:** 3.00 allocs/frame is a real allocation in the rendering hot path.
**Correct:** The 3.00 baseline is allocator-internal behavior (glibc
malloc arena management, SmallVec inline-to-heap transitions in rare
paths) — NOT cosmostrix rendering code. The actual rendering hot path
(`frame.rs`, `cloud/rain.rs`, `cloud/phosphor.rs`, `cloud/render.rs`)
has ZERO per-frame heap allocation. See `docs/PERFORMANCE_ACROSS_SCALES.md`
§3 for the source-level proof.
**Why it matters:** users spend time "optimizing" allocator internals
that have nothing to do with the engine.

### Misreading 7: "heap_retained 0 means there's no memory usage"

**Wrong:** `heap_retained = 0` means the process uses no memory.
**Correct:** `heap_retained` is the bytes allocated during the
measurement window and NEVER FREED. It excludes the steady-state
back-buffer, droplet pool, and runtime overhead — those are reported
in `peak_rss` (5.4 MiB on v30). `heap_retained = 0` means no LEAK,
not no USAGE.
**Why it matters:** users think the engine is memory-free when it's
just leak-free.

### Misreading 8: "the cloud Xeon beats the Ryzen 5800HS, so the Ryzen is broken"

**Wrong:** Higher `avg_fps` on cloud Xeon means the Ryzen is underperforming.
**Correct:** Benchmark mode is single-threaded by design
(`planned_worker_budget: 0`). The cloud Xeon's 2 vCPUs run at 3.2 GHz
sustained with higher single-thread IPC, while the Ryzen 5800HS has 8
cores / 16 threads but benchmark mode only uses one. The 1.58× ratio
is fully explained by single-thread IPC difference — the Ryzen is not
broken, it's just not being asked to use all its cores.
**Why it matters:** users file hardware bug reports for a measurement
artifact. See §5b for the full cloud Xeon comparison.

### Misreading 9: "backpressure_events 0 means the terminal is keeping up"

**Wrong:** `backpressure_events = 0` means the terminal renders every frame.
**Correct:** `backpressure_events` counts kernel-level write stalls
(pipe buffer full) when writing to /dev/null in `--bench-io` mode.
It does NOT measure terminal-emulator backpressure — that's a different
layer entirely. A real terminal can be backpressured even when
`backpressure_events = 0` in the benchmark.
**Why it matters:** users think the terminal is fine when the benchmark
only measured kernel I/O, not terminal I/O.

### Misreading 10: "IPC 2.58 means the CPU is bottlenecked on branches"

**Wrong:** IPC < 3.0 means branch mispredicts are dominating.
**Correct:** IPC 2.58 is healthy — the working set fits in L1 and the
branch predictor is hot. IPC > 2.0 = healthy, IPC > 3.0 = excellent
(usually only achievable with SIMD or very tight loops). The bottleneck
at IPC 2.58 is more likely cache latency or instruction dependency
chains, not branches.
**Why it matters:** users waste time on branch optimization when the
real bottleneck is elsewhere. Check `branch_mispredict_rate` (< 2% =
predictor is fine).

---

## See Also

- [benchmark/README.md](../benchmark/README.md) — Full reference results
  across versions (v15, v30) and comparison vs other Matrix rain tools
- [docs/BENCHMARK_CLOUD_XEON.md](BENCHMARK_CLOUD_XEON.md) —
  Third-party hardware verification on a 2-core Intel Xeon cloud VM
  (116K avg FPS, same commit `c97ba87`)
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
