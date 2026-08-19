# Cloud Xeon Benchmark

<!-- SPDX-License-Identifier: GPL-3.0-only -->

Third-party hardware verification: cosmostrix v50.0.0-beta.2 on a 2-vCPU
Intel Xeon cloud VM (Alibaba Cloud Linux, kernel 5.10). Builds and runs
cleanly on hardware unrelated to the owner's Ryzen 5800HS.

## Environment

| Item | Value |
|---|---|
| CPU | Intel Xeon @ 3200 MHz (2 vCPUs, no SMT) |
| RAM | 3.9 GiB, no swap |
| OS | Alibaba Cloud Linux 5.10, glibc |
| Rust | 1.97.1 |
| Build | `pro-linux-v4` (fat LTO, x86-64-v4/AVX-512) |
| Terminal | `dumb` (headless) |

## Run 1 — Headline (lean + monolith + zen, 30s)

```
--benchmark --bench-scene lean --scene monolith --bench-duration 30
```

| Metric | Value |
|---|---:|
| avg_fps | **56,718** |
| peak_fps | 73,638 |
| median_fps | 58,560 |
| p99_frame_time | 0.023 ms |
| max_frame_time | 0.076 ms |
| frame_time_stability | excellent |
| dirty_glyphs_per_second | 6.09M |
| avg_dirty_cells_per_frame | 107.3 |
| peak_rss | 4.43 MiB |
| avg_cpu | 99.3% |
| fps_drift | -0.40% (stable) |
| heap_retained | 86 KiB |

Timing breakdown: sim 70.9% / render 26.8% / io 2.3%.

## Run 2 — Heavy Charset (lean + matrix + katakana, 15s)

```
--benchmark --bench-scene lean --scene matrix --charset katakana --bench-duration 15
```

| Metric | Value |
|---|---:|
| avg_fps | **10,896** |
| peak_fps | 12,199 |
| median_fps | 9,957 |
| p99_frame_time | 0.137 ms |
| max_frame_time | 0.450 ms |
| frame_time_stability | excellent |
| dirty_glyphs_per_second | 10.66M |
| avg_dirty_cells_per_frame | 978.6 |
| peak_rss | 4.91 MiB |
| avg_cpu | 98.6% |
| fps_drift | -3.42% (stable) |
| heap_retained | 102 KiB |

9.1× more dirty cells/frame than Run 1 (katakana vs zen charset).
Balanced sim/render split (41.7% / 55.9%).

## Run 3 — Production Draw (full Terminal::draw, 15s)

```
--benchmark --bench-scene production-draw --bench-io --scene monolith --bench-duration 15
```

| Metric | Value |
|---|---:|
| avg_fps | **26,211** |
| peak_fps | 30,491 |
| median_fps | 27,047 |
| p99_frame_time | 0.056 ms |
| max_frame_time | 0.109 ms |
| frame_time_stability | excellent |
| dirty_glyphs_per_second | 2.81M |
| avg_dirty_cells_per_frame | 107.3 |
| peak_rss | 4.41 MiB |
| avg_cpu | 98.7% |
| fps_drift | +0.04% (stable) |
| heap_retained | 0 |

Full ANSI serialization shifts cost to io (55.1% vs 2.3% in lean mode).
Still sustains 26K FPS — ~437× the 60 FPS display target.

## Notes

- Zero memory leaks, `frame_jitter: low`, `frame_time_stability: excellent` on all runs.
- RAPL energy and perf microarchitecture counters unavailable (cloud VM cgroup restriction).
- Raw logs: `run1_lean_monolith_30s.txt`, `run2_lean_matrix_katakana_15s.txt`, `run3_production_draw_15s.txt` in this directory.

## See Also

- [BENCHMARKING.md](../../docs/BENCHMARKING.md) — main benchmarking guide
- [BENCHMARK_ADVANCED.md](../../docs/BENCHMARK_ADVANCED.md) — energy / uarch setup
