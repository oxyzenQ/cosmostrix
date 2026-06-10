# Benchmark

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
