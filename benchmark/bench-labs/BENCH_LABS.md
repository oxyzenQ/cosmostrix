# Bench Labs

<!-- SPDX-License-Identifier: GPL-3.0-only -->

Hardware-verified benchmark results for cosmostrix. Covers a full size
sweep from 4x4 (engine minimum) to 7680x4320 (8K UHD) on the
current host.

## Environment

| Item | Value |
| --- | --- |
| CPU | Intel(R) Xeon(R) Processor @ 3200 MHz (2 vCPUs, no SMT) |
| RAM | 3.9 GiB, no swap |
| OS | Alibaba Cloud Linux 5.10, glibc |
| Rust | 1.97.1 |
| Build | `pro-linux-v4` (fat LTO, x86-64-v4/AVX-512) |
| Terminal | `dumb` (headless) |

## Size Sweep (lean + monolith + zen)

All sizes use `--bench-scene lean --scene monolith`. Duration is adaptive:
5s for <5K cells, 3s for 5K-500K, 2s for >500K cells.

| Size | Cells | Avg FPS | Peak FPS | p99 (ms) | Dirty cells/f | RSS (MiB) | Stability |
| ------ | ------: | --------: | ---------: | ---------: | ---------------: | -----------: | ---------- |
| 4x4 | 16 | 1,448,741 | 987,167 | 0.001 | 0.5 | 4 | excellent |
| 20x6 | 120 | 692,257 | 999,001 | 0.002 | 3.0 | 4 | excellent |
| 80x24 | 1,920 | 96,971 | 133,815 | 0.015 | 56.7 | 4 | excellent |
| 120x40 | 4,800 | 55,899 | 73,584 | 0.023 | 107.3 | 4 | excellent |
| 200x80 | 16,000 | 28,958 | 36,220 | 0.063 | 221.3 | 5 | excellent |
| 480x160 | 76,800 | 11,369 | 12,963 | 0.110 | 596.9 | 9 | excellent |
| 960x270 | 259,200 | 4,965 | 5,707 | 0.249 | 1,271.9 | 20 | excellent |
| 1920x540 | 1,036,800 | 4,115 | 4,824 | 0.309 | 1,386.8 | 62 | excellent |
| 3840x1080 | 4,147,200 | 4,098 | 4,774 | 0.313 | 1,384.1 | 218 | excellent |
| 7680x4320 | 33,177,600 | 3,864 | 4,824 | 0.334 | 1,380.1 | 1,649 | moderate |

FPS scales sub-linearly with cell count thanks to differential rendering.
Dirty cell count plateaus ~1,380 cells/frame above 960x270 — the
monolith scene produces a fixed number of active streams regardless
of grid size, so larger grids just have more empty space.

At 8K (33M cells), memory pressure shifts to the allocator (538 MiB
heap retained, 1.6 GiB RSS) and frame-time stability degrades to
"moderate". This is a memory benchmark, not a render benchmark — the
engine itself remains compute-bound at all practical sizes.

## Notes

- Zero memory leaks across all sizes; `frame_time_stability: excellent` up to 4K.
- RAPL energy and perf microarchitecture counters unavailable (cloud VM).
- Regenerate with: `./benchmark/benchmark.sh sweep --auto` (requires Rust toolchain)
  or `SWEEP_BIN=target/<triple>/pro-linux-v4/cosmostrix ./benchmark/benchmark.sh sweep`
- Raw logs and CSV are generated alongside this file in `benchmark/bench-labs/`.

## See Also

- [BENCHMARKING.md](../../docs/BENCHMARKING.md) — main benchmarking guide
- [BENCHMARK_ADVANCED.md](../../docs/BENCHMARK_ADVANCED.md) — energy / uarch setup
