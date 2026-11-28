# Bench Labs

<!--
  SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
  SPDX-License-Identifier: AGPL-3.0-only OR Commercial
-->

Hardware-verified benchmark results for cosmostrix. Covers a full size
sweep from 1x1 (engine minimum) to 7680x4320 (8K UHD) on the
current host.

## Environment

| Item | Value |
| --- | --- |
| CPU | Intel(R) Xeon(R) Processor (2 vCPUs) |
| RAM | 3.9 GiB |
| OS | Debian GNU/Linux 13 (trixie) |
| Rust | 1.98.1 |
| Build | `release` |
| Terminal | `dumb` |

## Size Sweep (monolith)

All sizes use `--scene monolith`. Duration is adaptive:
5s for <5K cells, 3s for 5K-500K, 2s for >500K cells.

| Size | Cells | Avg FPS | Peak FPS | p99 (ms) | Dirty cells/f | RSS (MiB) | Stability |
| ------ | ------: | --------: | ---------: | ---------: | ---------------: | -----------: | ---------- |
| 1x1 | 1 | 1,456,629 | 671,592 | 0.001 | 0.2 | 6 | excellent |
| 20x6 | 120 | 572,891 | 999,001 | 0.002 | 4.7 | 6 | excellent |
| 80x24 | 1,920 | 65,775 | 92,098 | 0.021 | 90.8 | 6 | excellent |
| 120x40 | 4,800 | 37,586 | 48,452 | 0.034 | 173.2 | 6 | excellent |
| 200x80 | 16,000 | 19,609 | 23,402 | 0.063 | 361.2 | 7 | excellent |
| 480x160 | 76,800 | 7,356 | 8,275 | 0.169 | 977.6 | 11 | excellent |
| 960x270 | 259,200 | 3,084 | 3,587 | 0.405 | 2,041.4 | 24 | excellent |
| 1920x540 | 1,036,800 | 1,073 | 1,225 | 1.169 | 4,218.3 | 76 | excellent |
| 3840x1080 | 4,147,200 | 478 | 525 | 2.563 | 8,701.6 | 285 | good |
| 7680x4320 | 33,177,600 | 198 | 240 | 5.498 | 18,170.5 | 2,099 | high |

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

At 8K (33M cells), the frame buffer dominates memory (~2.0 GiB peak
RSS, heap retained ~0) and frame-time stability degrades to
"high". This is a memory benchmark, not a render benchmark — the
engine itself remains compute-bound at all practical sizes.

This sweep was regenerated on 2026-11-28 with the default `release`
profile (no AVX-512 target), Rust 1.98.1; the 80x24 tier reproduces
the previous committed table within ~1% (65,775 vs 66,343 avg FPS).
The tracked 2026-09-14 sweep CSV recorded 85,698 at the same tier on
that day's host state — sweep numbers are machine- and load-dependent,
per the disclaimer below.

## PGO A/B (2026-08-23)

First PGO measurement on record: +4.5% median FPS and **−35% worst-case
frame time** vs the non-PGO release build, zero visual change
(deterministic per-frame metrics identical). See
[`PGO_AB_20260823.md`](PGO_AB_20260823.md). IPC/mispredict not measurable
in the container (perf counters blocked) — re-run on a bare-metal rig to
complete the `docs/archive/research/IPC_RESEARCH.md` verification.

## Notes

- Zero memory leaks across all sizes; `frame_time_stability`: `excellent`
  up to 1920x540, `good` at 3840x1080, `high` at 7680x4320.
- RAPL energy and perf microarchitecture counters unavailable (cloud VM).
- Regenerate with: `./benchmark/benchmark.sh sweep` (requires Rust toolchain)
  or `SWEEP_BIN=target/<triple>/pro-linux-amd64-v4-gnu/cosmostrix ./benchmark/benchmark.sh sweep`
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
