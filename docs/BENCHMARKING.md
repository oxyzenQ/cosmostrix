# Benchmarking Guide
<!-- SPDX-License-Identifier: GPL-3.0-only -->

> Independent guide to benchmarking cosmostrix: how to run, interpret, compare, and trust the numbers. For exhaustive metric definitions, see `--help` and the `--benchmark` output itself.

## Quick Start

```bash
# Default 5s benchmark (dry, no I/O — pure engine throughput)
target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix --benchmark

# 10s benchmark with wet I/O (writes ANSI to /dev/null)
cosmostrix --benchmark --bench-io --bench-duration 10s

# Measure the production render path (what the terminal actually sees)
cosmostrix --benchmark --bench-io --bench-scene production-draw --bench-duration 10s

# JSON output for CI/scripts
cosmostrix --benchmark --json | jq .performance.avg_fps
```

The default benchmark runs **dry** (no I/O) — it measures pure engine throughput, not how many frames the terminal *draws*. Real interactive FPS is bounded by the terminal emulator, refresh rate, and ANSI output bandwidth. Use `i` (live HUD, lowercase only — uppercase `I` is a no-op) during a real run to see actual interactive FPS.

## Benchmark Modes

| Flag | What it does | When to use |
|------|-------------|-------------|
| `--benchmark` | Premium 5s benchmark (2s warmup + 3s measurement). Prints FPS, frame-time percentiles, dirty-cell coverage, throughput, MEMORY (RSS), CPU %, component timing, DRIFT. | Default user-facing benchmark |
| `--bench-frames N` | Legacy CI benchmark. Runs N headless frames, prints compact `BENCH:` output. | CI pipelines, frame-count-based measurement |
| `--bench-duration N` | Override duration (1s minimum, no maximum). Accepts `30s`, `5m`, `1h30m`. | Endurance testing, drift/leak detection |
| `--bench-io` | Wet I/O — writes ANSI to `/dev/null`. Exercises kernel syscall path. | Measure real write bandwidth + latency |
| `--bench-scene NAME` | Select render path (`lean` or `production-draw`). Requires `--bench-io`. | Measure specific render path |
| `--bench-all` | Scaling sweep across 6×6 → 200×60. Prints SCALING SUMMARY table. | See how FPS scales with screen size |
| `--screen-size WxH` | Fixed virtual screen size. Min 4×4, max 7680×4320 in bench mode. | Benchmark at exact dimensions |
| `--save-baseline PATH` | Save JSON output to whitelist-enforced path. | Lock in regression baseline |
| `--compare-baseline PATH` | Compare current run against saved baseline. Flags >5% FPS regressions. | CI regression detection |
| `--json` | Machine-readable JSON output. | Scripts, CI, dashboards |

## `--bench-scene` Strict Validation + Reading the Report

`--bench-scene` is **strict** — only two values accepted, typos are rejected (not silently fallback'd). This is part of the cosmostrix honesty contract: no hidden flags, no hidden behavior.

| Value | What it measures |
|-------|------------------|
| `lean` (default) | The `emit_cell_lean` path — per-dirty-cell SGR emission. Fastest path cosmostrix uses in interactive mode. |
| `production-draw` | The full `Terminal::draw` redraw path — `MoveTo` per row + `ColorCache` SGR + BOLT bold escape. Mirrors what the terminal actually receives during interactive rendering. |

Two-layer validation: parse-time (clap `value_parser`, rejects invalid values with "did you mean?" tip) + runtime (`validate_bench_scene`, called at the top of all 3 benchmark entry points). `production-draw` requires `--bench-io` (the production draw path routes through `BenchIoWriter`).

The `--benchmark` report is organized into sections: `BENCHMARK ENVIRONMENT` (system info, git SHA, Rust version, profile), `RENDERER` (engine config, `gpu_usage: not_applicable`), `CONFIG` (CLI flags + config file), `PERFORMANCE` (FPS, frame-time percentiles, jitter, stability), `MEMORY` (RSS), `CPU` (process CPU %), `COMPONENT TIMING` (per-subsystem frame budget), `DRIFT` (first-half vs second-half FPS delta), `RESOURCE` (energy/power on Linux), `THROUGHPUT` (glyphs/sec, ANSI bytes/sec).

## Key Metrics

| Metric | Unit | What it tells you |
|--------|------|-------------------|
| `avg_fps` | FPS | Mean frames per second. Primary throughput number. |
| `peak_fps` | FPS | Highest instantaneous FPS. Often much higher than avg (diff engine skips frames with zero dirty cells). |
| `p99_frame_time` | ms | 99th-percentile frame time — slowest 1% of frames. Catches spikes avg hides. |
| `frame_time_stability` | label | `excellent` = p99 within 2× avg, max within 5×. |
| `fps_drift_percent` | % | (first_half − second_half) / first_half × 100. Negative = warmup; positive = throttle/leak. \|drift\| < 5% = stable. |
| `glyphs_per_second_theoretical` | glyphs/sec | Theoretical upper bound: full-frame cell count × active-frame rate. NOT actual throughput — use `dirty_glyphs_per_second` for actual rendered work. |
| `dirty_glyphs_per_second` | glyphs/sec | Changed cells per second — the work the diff engine actually emits. |
| `peak_rss` | MiB | Peak resident set size. Steady growth across runs = possible leak. |
| `avg_cpu_percent` | % | Process CPU%. ~99% = single-threaded, fully utilized. |
| `alloc_calls_per_frame` | count | Fresh allocations per frame. Higher = leaking heap. v30 baseline: 3.00. |
| `heap_retained` | bytes | Bytes allocated and never freed. Non-zero = investigate. |
| `energy_per_frame` | µJ | Energy per frame (Linux + RAPL only). Lower = more efficient. |
| `IPC` | ratio | Instructions per cycle (Linux + perf_event_open). >2.0 = healthy; >3.0 = excellent. |
| `frame_entropy` | bits | Information entropy of frame content. Higher = more visual variety. |
| `density_gini` | 0..1 | Gini coefficient of cell density. 0 = uniform; 1 = maximally concentrated. |

Wet (`--bench-io`) = writes ANSI to `/dev/null`; dry = no I/O (pure engine throughput). `lean` = dirty-cell-only emission (fastest); `production-draw` = full `Terminal::draw` path.

## Component Timing + Wet I/O + Scaling + Baseline

**Component Timing**: the `COMPONENT TIMING` section breaks the frame budget into per-subsystem costs — rain simulation, phosphor decay, color resolution (Chroma Dragon), BOLT formatting, ANSI emission, I/O write. Use this to identify which subsystem dominates frame time when profiling.

**Wet I/O vs Dry**: dry benchmarks measure pure compute throughput. Wet benchmarks (`--bench-io`) additionally exercise the kernel syscall path by writing ANSI bytes to `/dev/null`, surfacing `write_bandwidth` (MB/s), `avg_write_latency` (µs), `backpressure_events` (write stalls — non-zero = terminal can't keep up), `effective_write_fps`, `total_bytes_written`.

**Scaling (`--bench-all`)**: runs the benchmark across a sweep of screen sizes (6×6 → 200×60) and prints a SCALING SUMMARY table showing how FPS, dirty-cell ratio, and throughput scale with cell count. Use this to verify the diff engine's O(dirty_cells) claim holds at scale — dirty-cell ratio should drop as screen size grows (most cells unchanged per frame).

**Baseline Save & Compare**: `--save-baseline PATH` writes the JSON output to a whitelist-enforced path (`~/.config/cosmostrix/` or `/etc/cosmostrix/`). `--compare-baseline PATH` compares the current run against the saved baseline and flags any metric that regressed by more than 5%. Use in CI to catch performance regressions before they ship.

## Microarchitecture & Energy (Linux only)

Two additional sections require elevated privileges: `MICROARCHITECTURE` (Linux `perf_event_open` syscall — CPU cycles, retired instructions, branch instructions, branch misses, IPC, branch mispredict rate) and `ENERGY` (Linux RAPL powercap sysfs — total energy, avg power, energy per frame, energy per cell). Both are entirely opt-in — cosmostrix never silently probes privileged interfaces. See [BENCHMARK_ADVANCED.md](BENCHMARK_ADVANCED.md) for setup instructions.

## Reproducibility + Honesty Contract

**Reproducibility checklist**: same commit (`git rev-parse HEAD`), same profile (`pro-linux-v3` / `pro-linux-v4` / `nitro-pgo`), same `--bench-duration`, `--screen-size`, `--bench-scene`. Pin CPU governor (`cpupower frequency-set -g performance`), disable turbo if comparing across machines. Close other CPU-bound processes. Run twice — discard the first (warmup fills caches); use the second as the reported number. For wet benchmarks, ensure `/dev/null` is on tmpfs (default on Linux). For energy benchmarks, unplug laptop charger (battery gives cleaner RAPL) or pin to a desktop CPU with stable power.

**Honesty contract**: benchmark FPS is **synthetic uncapped throughput** measured in a headless simulation. It is NOT a release promise. The actual runtime target is the configured FPS (dynamic default: 60 on standard terminals, 144 on high-refresh; override with `--fps`). The terminal emulator's ANSI parse speed is the ceiling — no amount of SIMD, GPU, or C supercharger can fix a slow terminal. Do not chase raw FPS; frame-time stability and p99 latency matter more. The `RENDERER` section always reports `gpu_usage: not_applicable` — cosmostrix is CPU-only by design (see [PHILOSOPHY.md](PHILOSOPHY.md)). `--doctor` carries the same field for consistency.

## Diagnostic Recipes

- **"FPS is lower than expected"**: check `frame_time_stability` — if `medium`/`high`, look at `max_frame_time` for spikes. Check `fps_drift_percent` — positive drift = throttle/leak. Verify CPU governor is `performance`.
- **"RSS grows over time"**: check `alloc_calls_per_frame` and `heap_retained`. Steady growth in `peak_rss` across multiple `--bench-duration 60s` runs = leak. Use `--bench-duration 5m` to confirm.
- **"Wet bandwidth is low"**: check `backpressure_events` — non-zero = kernel pipe full. Check `avg_write_latency` — >1ms suggests `/dev/null` is not on tmpfs.
- **"IPC is below 2.0"**: verify the binary is built with `pro-linux-v3` or `pro-linux-v4` profile (AVX2/AVX-512). `cargo build --release` without a profile gives baseline SIMD.
- **"Energy per frame is high"**: check CPU governor — `powersave` inflates energy-per-frame. Verify RAPL is reading the right socket (multi-socket systems).
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
