# dragon-multicore A/B Benchmark Results

<!-- SPDX-License-Identifier: GPL-3.0-only -->

> Experimental branch. Numbers are machine-dependent and
> only meaningful as a relative comparison between the two
> binaries on the same host.

**Host**: `Linux x86_64`
**Cores**: `2`
**Bench duration**: 3s per run, 3 runs (median), +1s warmup

| Size | Cols | Serial FPS | Multi-core FPS | Speedup | Serial sim ms | Multi sim ms | Serial render ms | Multi render ms |
|------|------|------------|----------------|---------|---------------|--------------|------------------|------------------|
| 80x24 | 80 | 73154.7 | 73046.0 | 0.999x | 0.0000 | 0.0000 | 0.0042 | 0.0041 |
| 200x60 | 200 | 21262.9 | 21537.7 | 1.013x | 0.0000 | 0.0000 | 0.0164 | 0.0158 |
| 400x200 | 400 | 8026.6 | 8128.5 | 1.013x | 0.0000 | 0.0000 | 0.0421 | 0.0407 |

**Verdict**: NEUTRAL — average speedup 1.008x. Within noise. More droplets or larger terminals may shift the balance.
