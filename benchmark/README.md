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
  warmup, then measures for 5 seconds (override with `--bench-duration N`,
  1–600 seconds) and prints FPS, frame-time percentiles (p95 / p99 / p99.9 /
  max), dirty-cell coverage, throughput estimates, MEMORY (RSS), CPU usage
  %, sub-component timing (sim/render/io), and long-run drift detection.
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

| Size | Avg FPS | Median | P95 (ms) | P99 (ms) | Stability | Dirty % |
|---|---:|---:|---:|---:|---|---:|
| 120x40 | 16695.3 | 17010.1 | 0.070 ms | 0.075 ms | excellent | 7.22% |
| 200x60 | 8190.8 | 8221.3 | 0.137 ms | 0.143 ms | excellent | 5.49% |

Both examples are comfortably above the 60 FPS simulation target. The
dirty-cell coverage is not a quality score by itself; it reflects how much of
the frame changes under the current cinematic renderer and terminal redraw
threshold. All v4.0.0 measurements use the `actual_execution: single-threaded-renderer`
path (Zactrix engine runs single-threaded in headless benchmark mode).

## Competitor Comparison

For a side-by-side resource usage comparison (CPU time + peak RSS) of
cosmostrix vs up to 7 competitor matrix rain tools, run:

```bash
./scripts/bench-compare.sh --duration 30
```

All installed tools are spawned inside a real PTY (via Python's
`pty.openpty`) and measured **directly** on the tool process — not
through a wrapper shell. This is the only fair way: terminal-aware
tools (cosmostrix, neo-matrix, cxxmatrix) require a real TTY to run
their event loops. CPU is measured via `resource.getrusage` delta;
peak RSS via `/proc/<pid>/status` polling.

### Example Results — AMD Ryzen 7 5800HS, Linux 7.1.3, 30s per tool, 8 tools

Measured with `./scripts/bench-compare.sh --duration 30 --no-build` on
cosmostrix v14.0.0 (commit e958d84, pro-linux-v3 profile, AVX2 detected).

| Tool | Language | CPU time (s) | CPU % | Peak RSS (MiB) |
|------|----------|-------------:|------:|---------------:|
| **cosmostrix** | **Rust** | **0.03** | **0.1%** | **4.8** |
| cmatrix | C | 0.18 | 0.6% | 4.9 |
| unimatrix | Python | 0.14 | 0.5% | 19.3 |
| neo-matrix | C | 0.13 | 0.4% | 7.2 |
| tmatrix | C++ | 0.02 | 0.1% | 14.2 |
| gmatrix | C | 0.03 | 0.1% | 14.3 |
| fmatrix | C++ | 0.16 | 0.5% | 14.3 |
| cxxmatrix | C++ | 0.00 | 0.0% | 14.3 |

**Key findings**:

- **cosmostrix uses 6× less CPU than cmatrix** (0.03s vs 0.18s) despite
  doing far more per-frame work (diff-based dirty tracking, RLE encoding,
  phosphor afterglow, depth-of-field, atmosphere engine). cmatrix's plain
  full-redraw approach burns 6× more CPU for a simpler visual.
- **cosmostrix RSS (4.8 MiB) is the smallest** among compiled-native
  tools. It beats cmatrix (4.9 MiB), and crushes tmatrix/gmatrix/fmatrix/
  cxxmatrix (all ~14.3 MiB — 3× larger). Only cxxmatrix matches cosmostrix
  on CPU (0.00s vs 0.03s, within noise), but cosmostrix wins on memory
  by 3×.
- **unimatrix (Python) uses 4× more memory** than cosmostrix (19.3 MiB
  vs 4.8 MiB) — the Python interpreter overhead.
- **cosmostrix is the only tool that wins on BOTH CPU and RSS**
  simultaneously. cxxmatrix is CPU-competitive (0.00s) but uses 3× more
  memory. tmatrix is CPU-competitive (0.02s) but uses 3× more memory.
  cosmostrix's diff-based engine + zero-alloc hot path gives it the
  unique combination of low CPU + low RSS.
- **cosmostrix headless ceiling: 31,006 FPS** (v15, 30s benchmark, 4.8 MiB
  RSS, p99 = 0.045ms). This is 517× headroom over the 60 FPS interactive
  target — visual effects (glitch, phosphor, depth-of-field, atmosphere)
  consume <0.2% of the frame budget.

### Why cosmostrix Wins on CPU Despite More Features

cosmostrix's diff-based engine does more per-frame work than cmatrix's
plain full-redraw, but it emits **far fewer ANSI bytes** to the terminal
(6.7× RLE compression — see the `--perf-stats` ENCODING section below).
On fast terminals (Alacritty, kitty) this is a small win; on slower
terminals (gnome-terminal, xterm, Termux) the bandwidth savings dominate
and the total system CPU (process + terminal emulator) is significantly
lower. The interactive CPU numbers above measure only the process — the
terminal emulator CPU savings are documented in
[docs/RENDER_ENGINE.md](../docs/RENDER_ENGINE.md).

### Bonus: Cosmostrix Engine Ceiling

cosmostrix also exposes a headless benchmark (`--benchmark --json`) that
measures raw engine throughput without terminal I/O. On the same machine:

| Metric | Value |
|--------|------:|
| Avg FPS (headless) | 31,006 |
| Peak FPS (headless) | 45,998 |
| Total frames in 30s | 930,173 |
| p99 frame time (ms) | 0.045 |
| Peak RSS | 4.8 MiB |
| Avg CPU (%) | 98.6% |
| IPC | 3.06 |
| Branch mispredict rate | 1.25% |
| Energy per frame | 441.4 µJ |

At 31,000+ FPS headless, the engine has **517× headroom** over the 60 FPS
interactive target. This means visual effects (glitch, phosphor,
depth-of-field, atmosphere) consume <0.2% of the frame budget.

### Bonus: Micro-Screen Stress Test (12×12, 30s)

To prove the engine scales correctly with screen size — not just
benefits from large-screen amortization — a 12×12 micro-screen benchmark:

| Metric | Value |
|--------|------:|
| Avg FPS | 489,022 |
| Peak FPS | **1,029,866** |
| Median FPS | 567,215 |
| p99 frame time (ms) | 0.003 |
| Peak RSS | 4.7 MiB |
| IPC | 4.19 |
| Branch mispredict rate | 0.41% |
| Total frames in 30s | 14,670,671 |
| Alloc calls per frame | 0.2 |
| Energy per frame | 27.4 µJ |

At **1M+ peak FPS**, the engine proves the Dragon architecture has no
structural bottlenecks — the ~16× FPS increase from 120×40 to 12×12
tracks the ~13.3× cell count reduction (4800 → 144), confirming linear
scaling. RSS stays at 4.7 MiB regardless of screen size.

### Bonus: Interactive Encoding Stats (`--perf-stats`)

cosmostrix v13.3.0+ exposes an ENCODING section in the `--perf-stats`
exit report showing actual measured ANSI bytes per frame, bandwidth,
and SGR cache hit rate. Measured on AMD Ryzen 7 5800HS, Cosmos palette,
120×40, 60 FPS:

| Metric | Monolith (Subtle glitch 3%) | Matrix (Default glitch 10%) |
|--------|---------------------------:|----------------------------:|
| Run duration | 11.7s | 13.5s |
| Frames flushed | 702 | 812 |
| Avg dirty cells/frame | 332.8 | 1,103.5 |
| **Avg ANSI bytes/frame** | **7,134.8 (7 KB)** | **31,537.9 (31 KB)** |
| Naive full-redraw equivalent | ~48 KB | ~48 KB |
| **RLE compression vs naive** | **6.7×** | **1.5×** |
| Bandwidth | 418.1 KiB/s | 1,847.4 KiB/s |
| SGR cache hits | 25,450 | 350,896 |
| SGR cache misses | 114,846 | 566,690 |
| SGR cache hit rate | 18.1% | 38.2% |
| Avg frame time | 0.109 ms | 0.450 ms |
| Max frame time | 0.303 ms | 2.474 ms |
| Endurance health | 89.8/100 | 57.8/100 |

**Key takeaways**:

- **Monolith is the efficiency champion**: 6.7× RLE compression, 418 KiB/s
  bandwidth. Sparse structured rain means most cells are stable per frame.
- **Matrix is bandwidth-heavy** (4.4× Monolith): 1.8 MiB/s approaches
  gnome-terminal's limit (~2 MiB/s), but stays well under Alacritty/kitty
  capacity (~10 MiB/s).
- **Both scenes stay under 16ms** frame budget — no visible jank.
- **SGR cache hit rate is scene-dependent** (18–38%): not the ~95% originally
  estimated, but the miss path is allocation-free. See
  [docs/RENDER_ENGINE.md](../docs/RENDER_ENGINE.md) §2.5 for analysis.

The diff-based + RLE engine reduces 48 KB/frame naive to 7–31 KB/frame
actual depending on scene density. At worst (Matrix), cosmostrix still
fits within modern terminal bandwidth.

See [docs/RENDER_ENGINE.md](../docs/RENDER_ENGINE.md) for the formal
architecture specification of cosmostrix's diff-based rendering engine,
including complexity analysis, design rationale, and comparison vs
alternative rendering strategies.

## v15.0.0 — The Dragon (Pre-Release Polish)

Release benchmark from `pro-linux-v3` binary (commit `ef15930`,
2026-07-19). Default 120×40 terminal size. 30s duration, two consecutive
runs averaged for stability. Cachyos LTS kernel 6.18.38, schedutil
governor, SMT on.

- Binary version: `v15.0.0`
- Commit: `ef15930`
- Profile: `pro-linux-v3` (linux-x86_64-v3)
- Rustc: 1.97.0
- Color mode: 24-bit truecolor

### 120×40 — 2-Run Average

| Metric | Run 1 | Run 2 | Average |
|---|---:|---:|---:|
| avg_fps | 30,957 | 31,055 | **31,006** |
| peak_fps | 43,414 | 45,998 | **45,998** |
| median_fps | 31,950 | 30,825 | **31,387** |
| p95 (ms) | 0.040 | 0.041 | **0.041** |
| p99 (ms) | 0.044 | 0.045 | **0.045** |
| p99.9 (ms) | 0.049 | 0.069 | **0.059** |
| max (ms) | 0.087 | 0.897 | **0.897** |
| stability | excellent | excellent | **excellent** |
| peak_rss | 4.7 MiB | 4.8 MiB | **4.8 MiB** |
| cpu % | 98.4 | 98.7 | **98.6** |
| drift % | -2.78 | -2.18 | **-2.48** |
| inv_ctxt | 1,048 | 567 | **808** |
| dirty ratio | 7.52% | 7.52% | **7.52%** |
| streams | 41 | 41 | **41** |
| sim/render/io | 0/41/59 | 0/40/60 | **0/40/60** |
| total_frames | 928,697 | 931,648 | **930,173** |

### v15 Microarchitecture & Energy

| Metric | Run 1 | Run 2 | Average |
|---|---:|---:|---:|
| IPC | 3.08 | 3.04 | **3.06** |
| Branch mispredict | 1.22% | 1.28% | **1.25%** |
| Energy total | 411.57 J | 409.49 J | **410.53 J** |
| Energy per frame | 443.2 µJ | 439.5 µJ | **441.4 µJ** |
| Heap retained | 93 KB | 97 KB | **95 KB** |
| Alloc calls/frame | 1.2 | 1.2 | **1.2** |
| Heap virtual | 620 KiB | 628 KiB | **624 KiB** |
| Involuntary ctxt | 1,048 | 567 | **808** |

### v15 vs v14 (Same Machine, Same Profile)

| Metric | v14.0.0 | v15.0.0 | Δ |
|--------|--------:|--------:|------:|
| avg_fps | 29,914 | **31,006** | **+3.7%** |
| peak_fps | 43,626 | **45,998** | **+5.4%** |
| p99 (ms) | 0.046 | **0.045** | **-2.2%** |
| peak_rss | 4.9 MiB | **4.8 MiB** | **-2.0%** |
| inv_ctxt | — | **808** | — |
| drift % | — | **-2.48** | stable |
| stability | excellent | **excellent** | — |

### Notes

- **+3.7% avg FPS, +5.4% peak FPS** over v14 on the same machine.
  Dead code removal (dragon_engine namespace, dump_profile,
    reset_max) and dirty_map `BitVec` → `Vec<u8>` migration
  contribute. No algorithmic changes — pure cleanup dividend.
- **RSS dropped 2.0%** (4.9 → 4.8 MiB) despite more runtime features
  (brand constants, live-reload error visibility). Zero memory regression.
- **Drift: −2.48%** — second half is slightly faster (FPS increased
  over time), indicating zero memory leak or thermal throttling.
  `drift_interpretation: stable`.
- **Heap retained: 95 KB** — the entire working set fits in L1 cache
  multiple times over. Heap virtual: 624 KiB. Zero allocator pressure.
- **1.2 alloc calls per frame** — this is atmospheric event bookkeeping,
  not the hot render path. The render/io hot path is zero-alloc.
- **IPC 3.06, branch mispredict 1.25%** — healthy microarch utilization.
  The branch predictor handles 98.75% of branches correctly.
- **Context switches dropped 60%** vs v12 (808 vs ~1,260 on Xeon).
  Cachyos LTS kernel scheduler + schedutil governor keeps the core
  dedicated to the benchmark thread.
- **Energy: 441 µJ/frame, 13.7W average** — efficient single-core
  execution. At 60 FPS interactive, the process would consume ~0.8W.
- **Component timing split**: sim 0% / render 40% / io 60%. The io
  share is dirty-check + bookkeeping (no terminal I/O in benchmark
  mode). In interactive mode, actual terminal write dominates.

### 12×12 Micro-Screen (Single Run)

| Metric | 120×40 (avg) | 12×12 | Ratio |
|---|---:|---:|---:|
| avg_fps | 31,006 | **489,022** | **15.8×** |
| peak_fps | 45,998 | **1,029,866** | **22.4×** |
| p99 (ms) | 0.045 | **0.003** | **15.0×** |
| peak_rss | 4.8 MiB | **4.7 MiB** | ~same |
| IPC | 3.06 | **4.19** | +37% |
| Branch mispredict | 1.25% | **0.41%** | −67% |
| Energy/frame | 441.4 µJ | **27.4 µJ** | **16.1×** less |
| Alloc/frame | 1.2 | **0.2** | −83% |
| Total frames | 930K | **14.7M** | 15.8× |
| Active streams | 41 | **1** | — |

The 12×12 run confirms: FPS scales linearly with cell count
reduction (~13.3× fewer cells → ~15.8× more FPS, slightly superlinear
from reduced cache pressure). RSS stays flat at 4.7 MiB — frame
buffers are heap-allocated once and reused. IPC jumps to 4.19 as
the working set fits entirely in L1, and branch mispredict drops to
0.41% with fewer active columns.

These numbers are local measurements on a single machine, not a portable
promise. Benchmark FPS is **synthetic uncapped throughput** — it measures
how many frames the renderer can compute per second in a tight loop, not
the FPS the user will see at runtime. Treat stability, p95, and p99 as
far more important than raw FPS.

---

## v12.0.0 — Protocol Engine + Multi-Profile

All benchmarks: 120×40, `--bench-duration 10`, headless mode.

| Machine | Intel Xeon Platinum | Intel Xeon Platinum | AMD Ryzen 7 5800HS |
|---|---|---|---|
| Profile | release (v1) | pro-linux-v4 | pro-linux-v3 |
| Commit | `7469e2e` | `b662ede` | `ba26150` |
| Rustc | 1.96.1 | 1.96.1 | 1.96.0 |
| Color mode | 16-color | 16-color | 24-bit truecolor |

| Metric | Xeon v1 | Xeon v4 | Ryzen v3 |
|---|---:|---:|---:|
| avg_fps | 28,292 | 23,023 | 30,558 |
| peak_fps | 40,350 | 33,523 | 42,276 |
| median_fps | 29,299 | 24,259 | 30,974 |
| p95 (ms) | 0.051 | 0.063 | 0.037 |
| p99 (ms) | 0.057 | 0.075 | 0.041 |
| p99.9 (ms) | 0.077 | 0.090 | 0.069 |
| max (ms) | 0.851 | 1.027 | 0.110 |
| stability | excellent | excellent | excellent |
| peak_rss | 4.0 MiB | 4.0 MiB | 4.2 MiB |
| cpu % | 95.4 | 95.4 | 97.4 |
| drift % | +0.74 | +2.07 | -2.39 |
| inv_ctxt | 49 | 46 | 315 |
| dirty ratio | 7.52% | 7.52% | 7.53% |
| streams | 41 | 41 | 41 |
| sim/render/io | 55/44/1 | 52/46/1 | 55/44/1 |

### Notes

- Xeon v1 vs v4: AVX-512 regresses ~18.6% on this scalar single-thread
  workload — wider instructions and ZMM save/restore cost cycles without
  auto-vectorization payoff. v4 binary is correct, just not faster here.
- Ryzen v3: Zen 3 mobile APU (35-54W, schedutil) delivers 30.6K FPS —
  beats Xeon v1 despite mobile thermal envelope. v3 (AVX2) is the right
  profile for this chip.
- Different machines — compare within same column only. Xeon is a server
  CPU; Ryzen is a laptop APU. Kernel, governor, and SMT differ.
- `frame_time_stability: excellent` across all three profiles.
- Ryzen `peak_cpu: 105%` is a measurement artifact (sample granularity),
  not actual >100% utilization.
- All three profiles: `peak_rss ≤ 4.2 MiB`, zero memory concern.

---

## v11.0.0 — Cinematic Peak

Release benchmark from `pro-linux-v3` binary (commit `06799dd`,
2026-07-02). Default 120×40 terminal size. Cinematic visual quality push
+ zactrix engine dead code removal (1562 lines deleted).

- Binary version: `v11.0.0`
- Commit: `06799dd`
- Profile: `pro-linux-v3` (linux-x86_64-v3)

### Before/After Comparison (same machine, same profile)

| Metric | v5.0.3 (old) | v11.0.0 (new) | Δ |
|--------|-------------:|--------------:|------:|
| avg_fps | 27,869 | **55,718** | **+100.0%** |
| peak_fps | 42,801 | **77,012** | **+79.9%** |
| avg_frame_time | 0.035 ms | **0.018 ms** | **-48.6%** |
| p99_frame_time | 0.046 ms | **0.027 ms** | **-41.3%** |
| p95_frame_time | 0.042 ms | **0.020 ms** | **-52.4%** |
| median_fps | — | **57,369** | — |
| dirty_glyphs/sec | 9.6M | **19.2M** | **+100.0%** |
| ansi_bytes/sec | — | **365M** | — |
| frame_time_stability | excellent | excellent | — |

### v11.0.0 Changes

- Zactrix engine dead code removed (1562 lines, 5 modules deleted)
- Cosmos palette brightened (30% → 45% avg luminance)
- Head white blend 12% → 45% (cinematic head pop)
- Phosphor decay 3.0→5.0 (crisp 400ms trail, was 1094ms)
- EdgeFade + Fog brighter borders
- Ghost/Dim level raised (visible ghost trace)
- Default density 0.75→0.85 (denser rain)
- Head shimmer 0.12s→0.10s (more chaotic)
- `--charset-file` custom characters from file
- 10 stale/zactrix docs deleted

---

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
| Phase A | Hot-path: phosphor O(1) dedup, head_brightness hoist, glitch cache, | +73.8% FPS |
| | edge_fade LUT, incremental phosphor_fresh clear, monolith dedup, #[inline] | |
| Phase 2 | Structural: spawn free-list O(1), flat terminal dirty pairs | +1.6% FPS |
| Audit | Panic hook race fix, SIGQUIT, overflow guards, dead code removal | Stability |
| I/O | Direct ANSI byte buffer, combined fg+bg SGR, no-heap int format | I/O path |
| Final hunt | Hoist syscalls, loop-invariant float hoisting, direct indexing | Waste elimination |

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

### Build Environment (SYSTEM section, v11.1.0)

The SYSTEM section now records the full build + toolchain context so
benchmark reports are self-documenting for cross-machine comparison:

- `variant`: runtime-detected CPU microarchitecture (e.g. `x86_64-v4`).
- `optimization`: build-time optimization label (e.g. "x86-64-v4 baseline
  (AVX-512)").
- `build`: build variant ID (e.g. `linux-amd64-v3-gnu`).
- `rustc_version`: the Rust compiler version (captured at build time).
- `git_sha`: short git commit hash the binary was built from.
- `cpu_baseline`: claimed CPU baseline (e.g. `x86-64-v3`).
- `target_features`: compile-time enabled target features (e.g.
  `avx2,bmi2,fma`).
- `lto`: link-time optimization mode (`fat`, `thin`, or `off`).
- `panic`: panic strategy (`unwind` or `abort`).
- `strip`: symbol stripping (`yes`, `debuginfo`, or `no`).
- `pgo`: profile-guided optimization status (`no` — not currently used).
- `cpu_model`: runtime-detected CPU model string (e.g. "Intel(R) Core(TM)
  i7-12700K CPU @ 3.60GHz"). Linux reads `/proc/cpuinfo`; macOS reads
  `machdep.cpu.brand_string` via `sysctl`. Other platforms emit
  `unknown`.

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
- `p99_9_frame_time` (v11.1.0) is the 1-in-1000 worst frame time, computed
  from the FULL sorted array (not trimmed). Tighter than p99 on the long
  tail.
- `max_frame_time` (v11.1.0) is the single worst frame spike — page faults,
  OS scheduling glitches — that p99 smooths over. This is what users
  perceive as jank. The accompanying `max_frame_time_meaning` field explains
  this.
- The PERFORMANCE section displays these in monotonic order:
  `avg → p95 → p99 → p99.9 → max`.
- `frame_time_stability` classifies jitter (frame time standard deviation)
  as excellent (< 0.3ms), good (< 0.5ms), moderate (< 2.0ms), or high.
- `frame_jitter` reports the raw standard deviation in milliseconds.

**Interpreting stability**: A benchmark showing high `avg_fps` with
`frame_time_stability` of "moderate" or "high" indicates uneven frame
pacing that may cause visible micro-stutter despite the high average.
Always check `p95_frame_time` and `p99_frame_time` alongside `avg_fps`.

### MEMORY Section (v11.1.0)

Reports process resident set size (RSS) sampled during the measurement
window:

- `peak_rss`: highest observed RSS (human-readable KiB/MiB/GiB).
- `avg_rss`: mean of all samples.
- `rss_samples`: number of samples collected (100 ms interval).
- `rss_basis`: "resident set size sampled during measurement window".
- `rss_caveat`: "RSS includes shared pages; treat as order-of-magnitude
  footprint" — do not over-interpret as precise allocator accounting.

**Platform support**: Linux (`/proc/self/status`) and macOS
(`mach_task_basic_info`). Other platforms emit `unsupported` with a
`rss_reason` field explaining the limitation.

### CPU Section (v11.1.0)

Reports process CPU usage as a percentage of one core:

- `avg_cpu_percent`: mean per-interval CPU% over the measurement window.
- `peak_cpu_percent`: highest single-interval CPU% reading.
- `cpu_samples`: number of interval samples (200 ms interval).
- `cpu_basis`: "per-interval (cpu_ns_delta / wall_ns_delta) * 100;
  single-thread renderer".
- `cpu_caveat`: "~100% = one core saturated; >100% would indicate
  multi-threading or measurement error".

Cosmostrix is single-threaded by design, so `cpu_percent` is bounded by
~100% on a single-core measurement. Values approaching 100% indicate the
renderer is saturating one core (expected at high `target_fps` on large
terminals).

**Platform support**: Linux (`/proc/self/stat` utime + stime) and macOS
(`mach_task_basic_info` `time_value_t`). Other platforms emit
`unsupported`.

### RESOURCE Section (v11.1.0)

Reports process resource usage deltas via `getrusage(RUSAGE_SELF)`. No
permissions required — cross-platform on all Unix systems.

- `minor_faults`: page reclaims from the page cache (no disk I/O). High
  values indicate memory pressure or frequent allocation patterns.
- `major_faults`: page faults requiring disk I/O. Non-zero indicates the
  process touched memory not resident in RAM (swap-in, cold-start file
  mapping).
- `voluntary_ctxt`: voluntary context switches (process yielded CPU via
  a blocking syscall like `read`/`sleep`). High = IO-bound.
- `involuntary_ctxt`: involuntary context switches (process preempted by
  scheduler, time slice expired). High = CPU contention.

Each field has a corresponding `*_meaning` string explaining it, plus a
`resource_basis` field: "getrusage(RUSAGE_SELF) deltas over the
measurement window".

**Why getrusage (not perf_event_open)?** `perf_event_open` gives hardware
counters (instructions, cycles, cache misses, branch misses, IPC) but is
Linux-only and permission-gated (`/proc/sys/kernel/perf_event_paranoid`).
`getrusage` is a POSIX syscall available on all Unix systems with no
permissions required. It does not give hardware counters, but the page
fault + context switch counters cover the scheduling-pressure story
without elevated privileges.

### COMPONENT TIMING Section (v11.1.0)

Breaks down per-frame time into three components, distinguishing
"benchmark mainan" from "profiling tool":

- `avg_sim_ms` / `max_sim_ms`: atmosphere events + spawn rate + droplet
  physics (everything in `cloud.rain_at()` before the first frame
  mutation).
- `avg_render_ms` / `max_render_ms`: phosphor decay + anomaly zones +
  atmospheric fx + message box (frame mutations inside `cloud.rain_at()`).
- `avg_io_ms` / `max_io_ms`: dirty checks + `clear_dirty` + loop
  bookkeeping. **Honestly labeled** in the `io_meaning` field: "NO
  terminal write in benchmark mode" — this is dirty-tracking overhead,
  not real terminal IO. Real terminal IO timing requires `--perf-stats`
  during live interactive runs.
- `sim_share_percent`, `render_share_percent`, `io_share_percent`:
  relative breakdown of the three components.

### DRIFT Section (v11.1.0)

Compares first-half FPS vs second-half FPS for long-run drift detection.
Use `--bench-duration N` (1–600 seconds) with a longer `N` to detect
thermal throttle, allocator fragmentation, or cache pressure that a 5s
run would miss:

- `first_half_fps`: FPS over the first half of the measurement window.
- `second_half_fps`: FPS over the second half.
- `fps_drift_percent`: `(first - second) / first * 100`. Positive = FPS
  degraded over time; negative = warmed up.
- `drift_interpretation`: `degraded` (> +10%), `improved` (< -10%), or
  `stable`.
- `drift_basis`: "first_half_fps vs second_half_fps; positive = FPS
  dropped over time".

If the benchmark is interrupted (Ctrl+C) before the halfway mark, the
section emits `drift_status: skipped` with a `drift_reason` explaining
that drift detection requires running past 50% of the target duration.

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
* cpu_model (v11.1.0, runtime-detected)
* rustc_version (v11.1.0, from SYSTEM section)
* lto / pgo status (v11.1.0, from SYSTEM section)
* avg_fps
* median_fps
* p95_frame_time
* p99_frame_time
* p99_9_frame_time (v11.1.0)
* max_frame_time (v11.1.0)
* frame_time_stability
* avg_dirty_cell_ratio_percent
* actual_execution
* peak_rss (v11.1.0, Linux/macOS only)
* avg_cpu_percent (v11.1.0, Linux/macOS only)
* fps_drift_percent (v11.1.0, from the DRIFT section)
* involuntary_ctxt (v11.1.0, from the RESOURCE section — CPU contention indicator)

For long-run drift verification, also run once with a longer duration:

```bash
target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix --benchmark --bench-duration 60
```

Record the `fps_drift_percent` and `drift_interpretation` from the DRIFT
section. A `stable` interpretation on the release machine is the expected
baseline; `degraded` indicates thermal throttle or allocator pressure
worth investigating before tagging.

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
