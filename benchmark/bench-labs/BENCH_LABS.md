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
| Build | `release` (fat LTO, codegen-units=1, default CPU target) |
| Terminal | `dumb` (headless) |

## Size Sweep (monolith)

All sizes use `--scene monolith`. Duration is adaptive:
5s for <5K cells, 3s for 5K-500K, 2s for >500K cells.

| Size | Cells | Avg FPS | Peak FPS | p99 (ms) | Dirty cells/f | RSS (MiB) | Stability |
| ------ | ------: | --------: | ---------: | ---------: | ---------------: | -----------: | ---------- |
| 4x4 | 16 | 1,410,677 | 995,025 | 0.001 | 0.5 | 4 | excellent |
| 20x6 | 120 | 651,757 | 999,001 | 0.002 | 3.0 | 4 | excellent |
| 80x24 | 1,920 | 91,822 | 125,188 | 0.014 | 56.7 | 5 | excellent |
| 120x40 | 4,800 | 53,766 | 69,171 | 0.024 | 107.3 | 5 | excellent |
| 200x80 | 16,000 | 28,624 | 34,559 | 0.042 | 220.4 | 5 | excellent |
| 480x160 | 76,800 | 10,595 | 12,196 | 0.127 | 595.8 | 9 | excellent |
| 960x270 | 259,200 | 4,464 | 5,359 | 0.379 | 1,271.1 | 20 | excellent |
| 1920x540 | 1,036,800 | 3,735 | 4,433 | 0.339 | 1,393.4 | 62 | excellent |
| 3840x1080 | 4,147,200 | 3,679 | 4,426 | 0.345 | 1,373.8 | 217 | excellent |
| 7680x4320 | 33,177,600 | 2,882 | 4,291 | 0.387 | 1,385.8 | 1,649 | high |

FPS scales sub-linearly with cell count thanks to differential rendering.
Dirty cell count plateaus ~1,380 cells/frame above 960x270 — the
monolith scene produces a fixed number of active streams regardless
of grid size, so larger grids just have more empty space.

At 8K (33M cells), memory pressure shifts to the allocator (538 MiB
heap retained, 1.6 GiB RSS) and frame-time stability degrades to
"high". This is a memory benchmark, not a render benchmark — the
engine itself remains compute-bound at all practical sizes.

This sweep was regenerated on 2026-08-22 with the default `release`
profile (no AVX-512 target). The previous `pro-linux-v4` sweep
(2026-08-20) reached ~3,864 FPS at 8K — the difference is the CPU
microarchitecture target, not a regression.

## Notes

- Zero memory leaks across all sizes; `frame_time_stability: excellent` up to 4K.
- RAPL energy and perf microarchitecture counters unavailable (cloud VM).
- Regenerate with: `./benchmark/benchmark.sh sweep` (requires Rust toolchain)
  or `SWEEP_BIN=target/<triple>/pro-linux-v4/cosmostrix ./benchmark/benchmark.sh sweep`
  for an AVX-512 targeted build.

## See Also

- [BENCHMARKING.md](../../docs/BENCHMARKING.md) — main benchmarking guide
- [BENCHMARK_ADVANCED.md](../../docs/BENCHMARK_ADVANCED.md) — energy / uarch setup
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
