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
| 4x4 | 16 | 1,419,655 | 965,251 | 0.001 | 0.5 | 4 | excellent |
| 20x6 | 120 | 648,717 | 999,001 | 0.002 | 3.0 | 4 | excellent |
| 80x24 | 1,920 | 89,073 | 123,213 | 0.015 | 56.8 | 5 | excellent |
| 120x40 | 4,800 | 53,572 | 69,818 | 0.024 | 107.4 | 5 | excellent |
| 200x80 | 16,000 | 28,815 | 34,428 | 0.043 | 221.0 | 5 | excellent |
| 480x160 | 76,800 | 10,703 | 12,125 | 0.112 | 596.2 | 9 | excellent |
| 960x270 | 259,200 | 4,424 | 5,176 | 0.281 | 1,271.9 | 19 | excellent |
| 1920x540 | 1,036,800 | 1,550 | 1,831 | 0.807 | 2,596.9 | 65 | excellent |
| 3840x1080 | 4,147,200 | 754 | 878 | 1.649 | 5,186.6 | 244 | good |
| 7680x4320 | 33,177,600 | 271 | 403 | 3.666 | 11,165.3 | 1,812 | high |

FPS scales sub-linearly with cell count thanks to differential rendering;
dirty cells now scale with grid size (~2x per 4x cell increase) because the
simulation covers the full bench-bounded grid.

Historical note (2026-08-23, LTS audit LOW-2 fix c1c7779): before the
`Cloud::reset_bench` fix, the oversized tiers (>1024 cols) ran a hybrid
state — the rain simulation was clamped to the interactive 1024x500 bounds
while the frame buffer used the raw bench dimensions. Dirty cells were
stuck at ~1,380/frame for every tier above 960x270 (the sim only knew the
clamped area), and the pre-fix sweeps reported ~3,300-3,700 FPS at
1920-8K — measurements of a partially-dead simulation, not the full grid.
The benchmark path now routes through `reset_bench` (mirroring
`Frame::new_bench`), so these numbers are the first honest full-grid
measurements at the oversized tiers.

At 8K (33M cells), memory pressure shifts to the allocator (538 MiB
heap retained, 1.8 GiB RSS) and frame-time stability degrades to
"high". This is a memory benchmark, not a render benchmark — the
engine itself remains compute-bound at all practical sizes.

This sweep was regenerated on 2026-08-23 with the default `release`
profile (no AVX-512 target). The previous `pro-linux-v4` sweep
(2026-08-20) reached ~3,864 FPS at 8K — but note that sweep pre-dates
the LOW-2 fix, so its oversized-tier numbers measured the hybrid
(partially clamped) simulation state and are not comparable.

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
