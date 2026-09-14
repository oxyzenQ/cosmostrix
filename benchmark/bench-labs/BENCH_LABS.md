# Bench Labs

<!-- SPDX-License-Identifier: GPL-3.0-only -->

Hardware-verified benchmark results for cosmostrix. Covers a full size
sweep from 1x1 (engine minimum) to 7680x4320 (8K UHD) on the
current host.

## Environment

| Item | Value |
| --- | --- |
| CPU | Intel(R) Xeon(R) Processor (2 vCPUs) |
| RAM | 4.1 GiB |
| OS | Debian GNU/Linux 13 (trixie), glibc |
| Rust | 1.98.1 |
| Build | `pro` |
| Terminal | `dumb` |

## Size Sweep (monolith)

All sizes use `--scene monolith`. Duration is adaptive:
5s for <5K cells, 3s for 5K-500K, 2s for >500K cells.

| Size | Cells | Avg FPS | Peak FPS | p99 (ms) | Dirty cells/f | RSS (MiB) | Stability |
| ------ | ------: | --------: | ---------: | ---------: | ---------------: | -----------: | ---------- |
| 1x1 | 1 | 1,531,791 | 992,063 | 0.001 | 0.1 | 5 | excellent |
| 20x6 | 120 | 633,109 | 999,001 | 0.002 | 3.0 | 5 | excellent |
| 80x24 | 1,920 | 85,698 | 114,038 | 0.015 | 56.9 | 5 | excellent |
| 120x40 | 4,800 | 49,711 | 64,666 | 0.026 | 107.1 | 5 | excellent |
| 200x80 | 16,000 | 26,515 | 31,655 | 0.045 | 219.8 | 6 | excellent |
| 480x160 | 76,800 | 9,773 | 11,063 | 0.125 | 588.5 | 10 | excellent |
| 960x270 | 259,200 | 3,826 | 4,557 | 0.312 | 1,268.3 | 23 | excellent |
| 1920x540 | 1,036,800 | 1,279 | 1,525 | 1.073 | 2,550.7 | 68 | excellent |
| 3840x1080 | 4,147,200 | 584 | 664 | 2.090 | 5,130.9 | 253 | good |
| 7680x4320 | 33,177,600 | 215 | 292 | 4.856 | 10,979.0 | 1,843 | high |

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

## PGO A/B (2026-08-23)

First PGO measurement on record: +4.5% median FPS and **−35% worst-case
frame time** vs the non-PGO release build, zero visual change
(deterministic per-frame metrics identical). See
[`PGO_AB_20260823.md`](PGO_AB_20260823.md). IPC/mispredict not measurable
in the container (perf counters blocked) — re-run on a bare-metal rig to
complete the `docs/archive/research/IPC_RESEARCH.md` verification.

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
