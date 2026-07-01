# Benchmark
<!-- SPDX-License-Identifier: GPL-3.0-only -->

This folder contains the benchmark script and interpretation notes for
Cosmostrix performance measurements.

Benchmark numbers are **machine-dependent**. They depend on CPU, kernel
scheduler behavior, build profile, terminal dimensions, density, color mode,
and whether the test is measuring headless simulation or real terminal I/O.
Use benchmark output to compare builds on the same machine, not as a portable
promise.

## Current Benchmark Model

Cosmostrix exposes two benchmark paths:

- `--benchmark`: recommended human-readable benchmark. It runs a 2-second
  warmup, then measures for 5 seconds and prints FPS, frame-time percentiles,
  dirty-cell coverage, and throughput estimates.
- `--bench-frames N`: legacy CI/regression benchmark. It runs a fixed number
  of headless frames and prints compact `BENCH:` output for scripts.

The benchmark is a headless simulation/draw-computation benchmark. It is useful
for tracking renderer regressions, but interactive rendering can still be
terminal/compositor-bound.

**Important**: benchmark FPS is synthetic uncapped throughput. It measures how
many frames the renderer can compute per second in a tight loop, not the FPS
the user will see at runtime. The actual runtime target is the configured FPS
(normally 60). A lower synthetic FPS in a newer build can be perfectly normal
when diagnostics, reporting, or visual complexity have increased. Do not chase
raw FPS; p95/p99 frame time and `frame_time_stability` matter far more than
peak FPS.

## Example Local Results

The following values are example local measurements from the v4.0.0 AUR
release artifact (`cosmostrix-bin v4.0.0 linux-x86_64-v3`, SHA512 verified).
Treat them as a shape of output and interpretation guide, not as guaranteed
numbers.

| Size | Avg FPS | Median FPS | P95 frame time | P99 frame time | Stability | Avg dirty-cell coverage |
|---|---:|---:|---:|---:|---|---:|
| 120x40 | 16695.3 | 17010.1 | 0.070 ms | 0.075 ms | excellent | 7.22% |
| 200x60 | 8190.8 | 8221.3 | 0.137 ms | 0.143 ms | excellent | 5.49% |

Both examples are comfortably above the 60 FPS simulation target. The
dirty-cell coverage is not a quality score by itself; it reflects how much of
the frame changes under the current cinematic renderer and terminal redraw
threshold. All v4.0.0 measurements use the `actual_execution: single-threaded-renderer`
path (Zactrix engine runs single-threaded in headless benchmark mode).

## v10.0.0 — Peak Performance & Stability

Release benchmark from `pro-linux-v3` binary (commit `93ed607`,
2026-07-01). Default 120×40 terminal size. Three optimization phases
plus pre-release audit + I/O bottleneck research + final bottleneck hunt.

- Binary version: `v10.0.0`
- Commit: `93ed607`
- Profile: `pro-linux-v3` (linux-x86_64-v3)
- CPU: x86-64-v3 baseline (AVX/AVX2/BMI1/BMI2/FMA)

### Optimization Phases

| Phase | Description | Gain |
|-------|-------------|------|
| Phase A | Hot-path: phosphor O(1) dedup, head_brightness hoist, glitch cache, edge_fade LUT, incremental phosphor_fresh clear, monolith dedup, #[inline] | +73.8% FPS |
| Phase 2 | Structural: spawn free-list (O(1)), flat terminal dirty pairs (single sort) | +1.6% FPS |
| Audit | Panic hook race fix, SIGQUIT, overflow guards, memory ordering, dead code removal | Stability |
| I/O | Direct ANSI byte buffer (bypass crossterm .queue()), combined fg+bg SGR, no-heap integer formatting | I/O path |
| Final hunt | Hoist syscalls (now.elapsed, flash_time.elapsed), loop-invariant float hoisting, direct indexing | Waste elimination |

### Before/After Comparison (same machine, same profile)

| Metric | v5.0.3 (old) | v10.0.0 (new) | Δ |
|--------|-------------:|--------------:|------:|
| avg_fps | 27,869 | **39,147** | **+40.5%** |
| peak_fps | 42,801 | **55,451** | **+29.6%** |
| avg_frame_time | 0.035 ms | **0.025 ms** | **-28.6%** |
| p99_frame_time | 0.046 ms | **0.030 ms** | **-34.8%** |
| p95_frame_time | 0.042 ms | **0.028 ms** | **-33.3%** |
| median_fps | — | **40,378** | — |
| total_frames (5s) | 139,344 | **195,736** | **+40.5%** |
| dirty_glyphs/sec | 9.6M | **13.5M** | **+40.6%** |
| ansi_bytes/sec | — | **257M** | — |
| frame_time_stability | excellent | excellent | — |
| avg_dirty_cell_ratio | 7.21% | 7.21% | identical |
| active_streams_avg | 41 | 41 | identical |

### Invariants

| Field | Value |
|-------|-------|
| `actual_execution` | `single-threaded-renderer` |
| `terminal_writer` | `single-owner` |
| `frame_time_stability` | `excellent` |
| `avg_dirty_cell_ratio` | 7.21% |
| `active_streams_avg` | 41 |
| `io_strategy` | `crossterm-queue-batch` (runtime) / direct-ANSI-buffer (optimized) |

### Notes

- **+40.5% avg FPS, +29.6% peak FPS** cumulative from v5.0.3 to v10.0.0.
- Total gain from original v5.0.1 baseline: **+83.3% avg FPS** (21,359 → 39,147).
- p99 frame time dropped 34.8% — critical for smoothness at 60fps target.
- Dirty-cell ratio and active streams identical — zero visual impact.
- I/O optimization (direct ANSI byte buffer) bypasses crossterm `.queue()`
  overhead: eliminates ~170 trait dispatch + heap String alloc calls/frame.
- Combined fg+bg SGR saves ~3 bytes per color change.
- Single `write_all` flush per frame replaces ~170 individual queue calls.
- Headless benchmark doesn't exercise Terminal::draw — real terminal I/O
  gain is estimated 30-50% on the draw path.

These numbers are local measurements on a single machine, not a portable
promise. Benchmark FPS is **synthetic uncapped throughput** — it measures
how many frames the renderer can compute per second in a tight loop, not
the FPS the user will see at runtime. Treat stability, p95, and p99 as
far more important than raw FPS.

---

## v5.0.3 — Phosphor Optimization + Trail LUT + Dirty-Scan

Release benchmark from `pro-linux-v3` binary
(commit `2941aca`, 2026-06-29). Default 120x40 terminal size.
Performance optimizations: phosphor dirty-index scan, active-cell tracking,
trail exp LUT (eliminates ~3K exp() calls/frame), glitch multiply-by-inverse,
glyph set_force (skips 24-byte Cell compare), char-pool bitmask.

- Binary version: `v5.0.3`
- Commit: `2941aca`
- Profile: `pro-linux-v3` (linux-x86_64-v3)

### Before/After Comparison (same machine, same profile)

| Metric | v5.0.1 (old) | v5.0.3 (new) | Δ |
|--------|-------------:|-------------:|------:|
| avg_fps | 21,359 | **27,869** | **+30.5%** |
| peak_fps | 28,283 | **42,801** | **+51.3%** |
| avg_frame_time | 0.046 ms | **0.035 ms** | **-23.9%** |
| p99_frame_time | 0.058 ms | **0.046 ms** | **-20.7%** |
| p95_frame_time | 0.053 ms | **0.042 ms** | **-20.8%** |
| total_frames (5s) | 106,794 | **139,344** | **+30.5%** |
| dirty_glyphs/sec | 7.4M | **9.6M** | **+30.3%** |
| frame_time_stability | excellent | excellent | — |
| avg_dirty_cell_ratio | 7.22% | 7.21% | identical |
| active_streams_avg | 41 | 41 | identical |

### Invariants

| Field | Value |
|-------|-------|
| `actual_execution` | `single-threaded-renderer` |
| `terminal_writer` | `single-owner` |
| `compute_parallelism` | `disabled` |
| `frame_time_stability` | `excellent` |
| `avg_dirty_cell_ratio` | 7.21% |
| `active_streams_avg` | 41 |

### Notes

- **+30.5% avg FPS, +51.3% peak FPS** from pure computation optimization
  with zero visual change — identical cinematic output.
- The 50k FPS lab target remains **not reached** but peak is now within 15%.
- `terminal_writer` remains `single-owner`.
- `compute_parallelism` remains `disabled`.
- `actual_execution` remains `single-threaded-renderer`.
- Dirty-cell ratio and active streams are **identical** to v5.0.0/v5.0.1 —
  confirming zero visual impact.
- These optimizations benefit ALL terminal sizes proportionally.

These numbers are local measurements on a single machine, not a portable
promise.  Benchmark FPS is **synthetic uncapped throughput** — it measures how
many frames the renderer can compute per second in a tight loop, not the FPS
the user will see at runtime.  Treat stability, p95, and p99 as far more
important than raw FPS.

## v5.0.0 — Nightfall: Cinematic UX + Product Identity Release

Release benchmark from `pro-linux-v3` binary
(commit `20552f1`, 2026-06-13). Default 120x40 terminal size.

- Binary version: `Version: v5.0.0`
- Commit: `20552f1`
- Profile: `pro-linux-v3` (linux-x86_64-v3)
- Run count: 5

### 5-Run Table

| Run | Avg FPS | Median FPS | P95 frame time | P99 frame time | Stability | Dirty ratio | Active streams |
|-----|--------:|-----------:|---------------:|---------------:|-----------|------------:|---------------:|
| 1 | 28700.2 | 29078.2 | 0.037 ms | 0.039 ms | excellent | 7.21% | 41 |
| 2 | 28780.7 | 29039.4 | 0.038 ms | 0.039 ms | excellent | 7.21% | 41 |
| 3 | 28690.8 | 29001.5 | 0.038 ms | 0.041 ms | excellent | 7.21% | 41 |
| 4 | 28798.9 | 29071.5 | 0.038 ms | 0.040 ms | excellent | 7.21% | 41 |
| 5 | 28628.7 | 28931.4 | 0.038 ms | 0.040 ms | excellent | 7.21% | 41 |

- **Mean avg_fps**: 28720.0
- **P95 range**: 0.037–0.038 ms
- **P99 range**: 0.039–0.041 ms

### Invariants

| Field | Value |
|-------|-------|
| `actual_execution` | `single-threaded-renderer` |
| `terminal_writer` | `single-owner` |
| `compute_parallelism` | `disabled` |
| `frame_time_stability` | `excellent` (all 5 runs) |
| `avg_dirty_cell_ratio` | 7.21% (all 5 runs) |
| `active_streams_avg` | 41 (all 5 runs) |

### Notes

- This benchmark measures the **default renderer workload** (cosmic rain
  animation at 120x40).  Heavy message or matrix-mode workloads are not
  comparable to the default benchmark and will yield different FPS numbers.
- The 50k FPS lab target was **not reached** and is **not promised**.
- `terminal_writer` remains `single-owner`: terminal writes are never
  parallelized.
- `compute_parallelism` remains `disabled`: no parallel frame computation.
- `actual_execution` remains `single-threaded-renderer`: the renderer executes
  on a single thread in benchmark mode.

These numbers are local measurements on a single machine, not a portable
promise.  Benchmark FPS is **synthetic uncapped throughput** — it measures how
many frames the renderer can compute per second in a tight loop, not the FPS
the user will see at runtime.  Treat stability, p95, and p99 as far more
important than raw FPS.

## v4.9.0 — The Wolf: Release Guard + Terminal Runtime Contract

Release benchmark from `pro-linux-v3` binary
(commit `43e3dc9`, 2026-06-13). Default 120x40 terminal size.

- Binary version: `Version: v4.9.0`
- Commit: `43e3dc9`
- Profile: `pro-linux-v3` (linux-x86_64-v3)
- Run count: 5

### 5-Run Table

| Run | Avg FPS | Median FPS | P95 frame time | P99 frame time | Stability | Dirty ratio | Active streams |
|-----|--------:|-----------:|---------------:|---------------:|-----------|------------:|---------------:|
| 1 | 28324.7 | 28571.8 | 0.039 ms | 0.041 ms | excellent | 7.21% | 41 |
| 2 | 28287.3 | 28670.1 | 0.039 ms | 0.040 ms | excellent | 7.21% | 41 |
| 3 | 28290.1 | 28710.9 | 0.038 ms | 0.040 ms | excellent | 7.21% | 41 |
| 4 | 28380.4 | 28748.9 | 0.038 ms | 0.040 ms | excellent | 7.21% | 41 |
| 5 | 28305.8 | 28565.3 | 0.039 ms | 0.042 ms | excellent | 7.21% | 41 |

- **Mean avg_fps**: 28317.6
- **P95 range**: 0.038–0.039 ms
- **P99 range**: 0.040–0.042 ms

### Invariants

| Field | Value |
|-------|-------|
| `actual_execution` | `single-threaded-renderer` |
| `terminal_writer` | `single-owner` |
| `compute_parallelism` | `disabled` |
| `frame_time_stability` | `excellent` (all 5 runs) |
| `avg_dirty_cell_ratio` | 7.21% (all 5 runs) |
| `active_streams_avg` | 41 (all 5 runs) |

### Notes

- This benchmark measures the **default renderer workload** (cosmic rain
  animation at 120x40).  Heavy message or matrix-mode workloads are not
  comparable to the default benchmark and will yield different FPS numbers.
- The 50k FPS lab target was **not reached** and is **not promised**.
- `terminal_writer` remains `single-owner`: terminal writes are never
  parallelized.
- `compute_parallelism` remains `disabled`: no parallel frame computation.
- `actual_execution` remains `single-threaded-renderer`: the renderer executes
  on a single thread in benchmark mode.

These numbers are local measurements on a single machine, not a portable
promise.  Benchmark FPS is **synthetic uncapped throughput** — it measures how
many frames the renderer can compute per second in a tight loop, not the FPS
the user will see at runtime.  Treat stability, p95, and p99 as far more
important than raw FPS.

## v4.8.0 — Zactrix Integration + Terminal Cleanup Hardening

v4.8.0 integrates the Zactrix color pipeline optimization and hardens terminal
cleanup on signal exit (fork-guard stdout race fix, viewport clear before
alternate screen switch). 5-run release benchmark from `pro-linux-v3` binary
(commit `ec1214b`), default 120x40 terminal size.

- Binary version: `v4.8.0`
- Commit: `ec1214b`
- Profile: `pro-linux-v3` (linux-x86_64-v3)
- Run count: 5

### 5-Run Table

| Run | Avg FPS | Median FPS | P95 frame time | P99 frame time | Stability | Dirty ratio | Active streams |
|-----|--------:|-----------:|---------------:|---------------:|-----------|------------:|---------------:|
| 1 | 28445.2 | 28737.3 | 0.039 ms | 0.042 ms | excellent | 7.21% | 41 |
| 2 | 28406.5 | 28808.1 | 0.039 ms | 0.041 ms | excellent | 7.21% | 41 |
| 3 | 28305.6 | 28565.7 | 0.039 ms | 0.041 ms | excellent | 7.21% | 41 |
| 4 | 28410.4 | 28582.9 | 0.038 ms | 0.040 ms | excellent | 7.21% | 41 |
| 5 | 28429.9 | 28769.1 | 0.038 ms | 0.040 ms | excellent | 7.21% | 41 |

- **Mean avg_fps**: 28399.5
- **P95 range**: 0.038–0.039 ms
- **P99 range**: 0.040–0.042 ms

### Invariants

| Field | Value |
|-------|-------|
| `actual_execution` | `single-threaded-renderer` |
| `terminal_writer` | `single-owner` |
| `compute_parallelism` | `disabled` |
| `frame_time_stability` | `excellent` (all 5 runs) |
| `avg_dirty_cell_ratio` | 7.21% (all 5 runs) |
| `active_streams_avg` | 41 (all 5 runs) |

### Notes

- This benchmark measures the **default renderer workload** (cosmic rain
  animation at 120x40).  Heavy message or matrix-mode workloads are not
  comparable to the default benchmark and will yield different FPS numbers.
- The 50k FPS lab target was **not reached** and is **not promised**.  The
  ~28,400 FPS plateau reflects the v4.8.0 default workload on this machine.
- `terminal_writer` remains `single-owner`: terminal writes are never
  parallelized.
- `compute_parallelism` remains `disabled`: no parallel frame computation.
- `actual_execution` remains `single-threaded-renderer`: the renderer executes
  on a single thread in benchmark mode.

These numbers are local measurements on a single machine, not a portable
promise.  Benchmark FPS is **synthetic uncapped throughput** — it measures how
many frames the renderer can compute per second in a tight loop, not the FPS
the user will see at runtime.  Treat stability, p95, and p99 as far more
important than raw FPS.

## v4.7.0 Local Benchmark Baseline

The v4.7.0 release prep phase (profile ecosystem contract, profile examples,
config dump / list-profiles profile docs pointers, profile validation UX
polish, profile RC smoke coverage) is docs/test-only with no runtime
changes.  Local benchmark at 120x40 from `pro-linux-v3` binary (commit
`c07dc5f`):

- avg_fps: approximately 16,914
- median_fps: approximately 17,141
- p95_frame_time: approximately 0.069 ms
- p99_frame_time: approximately 0.072 ms
- frame_time_stability: excellent
- avg_dirty_cell_ratio_percent: approximately 7.22%
- actual_execution: single-threaded-renderer
- terminal_writer: single-owner

These numbers are approximate local measurements, not a portable promise.
Benchmark FPS is **synthetic uncapped throughput** — it measures how many
frames the renderer can compute per second in a tight loop, not the FPS the
user will see at runtime. Treat stability, p95, and p99 as far more
important than raw FPS.

## v4.6.0 Local Benchmark Baseline

The v4.6.0 release prep phase (atmosphere expansion contract, preset registry,
CLI discoverability, RC smoke coverage) is docs/test-only with no runtime
changes.  Local benchmark at 120x40 from `pro-linux-v3` binary (commit
`1729390`):

- avg_fps: approximately 16,674
- median_fps: approximately 16,918
- p95_frame_time: approximately 0.070 ms
- p99_frame_time: approximately 0.074 ms
- frame_time_stability: excellent
- avg_dirty_cell_ratio_percent: approximately 7.22%
- actual_execution: single-threaded-renderer
- terminal_writer: single-owner

These numbers are approximate local measurements, not a portable promise.
Benchmark FPS is **synthetic uncapped throughput** — it measures how many
frames the renderer can compute per second in a tight loop, not the FPS the
user will see at runtime. Treat stability, p95, and p99 as far more
important than raw FPS.

## v4.5.0 Local Benchmark Baseline

The v4.5 foundation phase (architecture split, depth regression, test
pressure relief) is docs-only with no runtime changes. 5-run local
benchmark plateau at 120x40 after v4.5.0 stabilization:

- avg_fps: approximately 16,700
- median_fps: approximately 17,000
- p99_frame_time: approximately 0.074 ms
- frame_time_stability: excellent

These numbers are approximate local measurements, not a portable promise.
Benchmark FPS is **synthetic uncapped throughput** — it measures how many
frames the renderer can compute per second in a tight loop, not the FPS the
user will see at runtime. Treat stability, p95, and p99 as far more
important than raw FPS. Do not treat a difference between 10k and 13k FPS
as a user-visible regression unless p99, frame_time_stability, runtime CPU,
or visual behavior also regress.

## Metric Notes

- `draw_ratio` is a legacy compatibility field. It means frames with at least
  one dirty cell, not percentage of cell coverage.
- `active_frame_ratio_percent` is the clearer name for that same active-frame
  concept.
- `avg_dirty_cell_ratio_percent` is average dirty-cell coverage across all
  measured frames.
- `dirty_all_frames` counts logical frames where every cell was dirty.
- `estimated_full_redraw_frames` and
  `estimated_full_redraw_ratio_percent` estimate how often the terminal draw
  path is likely to cross its full-redraw threshold. They are not the same as
  `dirty_all_frames`.

### Throughput Stability

The premium benchmark (`--benchmark`) reports several frame-time stability
metrics alongside raw FPS:

- `p95_frame_time` and `p99_frame_time` are percentile measurements of frame
  computation time, computed after trimming the top and bottom 1% of samples
  to eliminate cold-path and OS scheduling noise.
- `frame_time_stability` classifies jitter (frame time standard deviation)
  as excellent (< 0.3ms), good (< 0.5ms), moderate (< 2.0ms), or high.
- `frame_jitter` reports the raw standard deviation in milliseconds.

**Interpreting stability**: A benchmark showing high `avg_fps` with
`frame_time_stability` of "moderate" or "high" indicates uneven frame
pacing that may cause visible micro-stutter despite the high average.
Always check `p95_frame_time` and `p99_frame_time` alongside `avg_fps`.

For detailed visual depth expectations and stability metric interpretation,
see [Visual Stability](../docs/VISUAL_STABILITY.md).

## Benchmark Sizes

The default benchmark size is 120x40:

```bash
COSMOSTRIX_BENCH_COLS=120 COSMOSTRIX_BENCH_LINES=40 \
  target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix --benchmark
```

Use 200x60 for a larger terminal-like stress case:

```bash
COSMOSTRIX_BENCH_COLS=200 COSMOSTRIX_BENCH_LINES=60 \
  target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix --benchmark
```

## How to Reproduce

Build an optimized profile:

```bash
cargo pro-linux-v3
```

Run the recommended benchmark:

```bash
target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix --benchmark
```

Run the legacy fixed-frame benchmark:

```bash
COSMOSTRIX_BENCH_COLS=120 COSMOSTRIX_BENCH_LINES=40 \
  target/release/cosmostrix --fps 60 --bench-frames 10000
```

Run the full comparison script:

```bash
bash benchmark/benchmark.sh
```

Generate a release benchmark report (prints Markdown to stdout):

```bash
./scripts/release-benchmark-report.sh X.Y.Z
```

The release report script runs N benchmark iterations, validates renderer
invariants, and prints a Markdown section ready for review and pasting
into this file.  It does not auto-edit files.  See `docs/RELEASE_GUARD.md`
Gate 4 for details.

The script builds comparison profiles and records optional `hyperfine`, `perf`,
and Valgrind outputs when those tools are installed. CI intentionally does not
gate on benchmark numbers; they are measurement aids, not stable pass/fail
thresholds.

## Release Benchmark Rule

Before tagging a stable release, update this file with a fresh local benchmark
from the release-candidate binary.

Required pre-tag flow:

```bash
cargo pro-linux-v3
target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix --benchmark
```

Record at minimum:

* version / commit
* build variant
* terminal size
* avg_fps
* median_fps
* p95_frame_time
* p99_frame_time
* frame_time_stability
* avg_dirty_cell_ratio_percent
* actual_execution

After the tag is published, verify the GitHub Release/AUR artifact separately.
Do not move or recreate a signed release tag just to update benchmark notes.
If benchmark documentation was missed, update `benchmark/README.md` on `main`
as a post-release process fix and apply the rule to the next release.

## Generated Outputs

The comparison script generates gitignored files in this folder:

- `hyperfine.md` - release vs optimized comparison table
- `time-*.txt` - `/usr/bin/time -v` output
- `perf-*.txt` - `perf stat` output
- `massif-*-*.out` - Valgrind heap profiles
