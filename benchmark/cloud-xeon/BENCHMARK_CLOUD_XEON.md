# Cloud Xeon Benchmark

<!-- SPDX-License-Identifier: GPL-3.0-only -->

Third-party hardware verification: cosmostrix v50.0.0-beta.2 on a 2-vCPU
Intel Xeon cloud VM (Alibaba Cloud Linux, kernel 5.10). Includes a
full size sweep from 4x4 (engine minimum) to 7680x4320 (8K UHD).

## Environment

| Item | Value |
|---|---|
| CPU | Intel Xeon @ 3200 MHz (2 vCPUs, no SMT) |
| RAM | 3.9 GiB, no swap |
| OS | Alibaba Cloud Linux 5.10, glibc |
| Rust | 1.97.1 |
| Build | `pro-linux-v4` (fat LTO, x86-64-v4/AVX-512) |
| Terminal | `dumb` (headless) |

## Size Sweep (lean + monolith + zen)

All sizes use `--bench-scene lean --scene monolith`. Duration is adaptive:
5s for <5K cells, 3s for 5K-500K, 2s for >500K cells.

| Size | Cells | Avg FPS | Peak FPS | p99 (ms) | Dirty cells/f | RSS (MiB) | Stability |
|------|------:|--------:|---------:|---------:|---------------:|-----------:|-----------|
| 4x4 | 16 | 1,439,837 | 994,036 | 0.001 | 0.5 | 3.7 | excellent |
| 20x6 | 120 | 665,888 | 999,001 | 0.002 | 3.0 | 3.7 | excellent |
| 80x24 | 1,920 | 97,141 | 133,976 | 0.014 | 56.8 | 4.5 | excellent |
| 120x40 | 4,800 | 56,803 | 73,000 | 0.023 | 107.4 | 4.5 | excellent |
| 200x80 | 16,000 | 30,004 | 36,895 | 0.043 | 220.4 | 5.4 | excellent |
| 480x160 | 76,800 | 11,260 | 13,019 | 0.110 | 597.1 | 9.0 | excellent |
| 960x270 | 259,200 | 4,842 | 5,585 | 0.255 | 1,279.1 | 19.6 | excellent |
| 1920x540 | 1,036,800 | 4,022 | 4,717 | 0.313 | 1,387.3 | 61.5 | excellent |
| 3840x1080 | 4,147,200 | 3,962 | 4,686 | 0.324 | 1,376.7 | 216.6 | excellent |
| 7680x4320 | 33,177,600 | 2,907 | 4,648 | 0.348 | 1,375.3 | 1,610 | high |

FPS scales sub-linearly with cell count thanks to differential rendering.
Dirty cell count plateaus ~1,375 cells/frame above 960x270 — the
monolith scene produces a fixed number of active streams regardless
of grid size, so larger grids just have more empty space.

At 8K (33M cells), memory pressure shifts to the allocator (564 MiB
heap retained, 1.6 GiB RSS) and frame-time stability degrades to
"high". This is a memory benchmark, not a render benchmark — the
engine itself remains compute-bound at all practical sizes.

## Scenario Comparison (120x40)

| Scene | Avg FPS | p99 (ms) | Dirty cells/f | Heap retained |
|---|---:|---:|---:|---:|
| lean + monolith (zen) | 56,803 | 0.023 | 107.4 | 86 KiB |
| lean + matrix (katakana) | 10,896 | 0.137 | 978.6 | 102 KiB |
| production-draw (monolith) | 26,211 | 0.056 | 107.3 | 0 |

## Notes

- Zero memory leaks across all sizes; `frame_time_stability: excellent` up to 4K.
- RAPL energy and perf microarchitecture counters unavailable (cloud VM).
- Raw logs: `sweep_*_20260819_201133.txt`, `run{1,2,3}_*.txt` in this directory.
- Sweep CSV: `sweep_20260819_201133.csv`.

## See Also

- [BENCHMARKING.md](../../docs/BENCHMARKING.md) — main benchmarking guide
- [BENCHMARK_ADVANCED.md](../../docs/BENCHMARK_ADVANCED.md) — energy / uarch setup