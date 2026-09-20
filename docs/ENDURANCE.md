# Endurance Testing
<!-- SPDX-License-Identifier: GPL-3.0-only -->

## Purpose

cosmostrix is designed to run as a long-lived terminal screensaver. This document
describes the endurance testing methodology used to verify that the renderer
remains stable for sessions exceeding 24 hours without memory growth, handle
leaks, or crashes.

## Test methodology

> **Note (2026-08-23):** the `scripts/monitor-cosmostrix.sh` and
> `scripts/endurance-summary.sh` helpers were removed in commit `936c7ba`
> (dead-script cleanup). This document is retained as the historical
> methodology record; for current measurements use `--benchmark` output
> and `benchmark/benchmark.sh sweep`.

A cosmostrix binary is launched in headless mode with a configurable duration
cap. The historical sampling script `scripts/monitor-cosmostrix.sh` read
`/proc/<pid>/status`, `/proc/<pid>/stat`, `/proc/<pid>/smaps_rollup`,
`/proc/<pid>/fd`, `/proc/<pid>/io`, and `/proc/stat` at regular intervals
and appends a single CSV row per sample.

### Important: CSV logs are local artifacts

Raw CSV resource logs are **intentionally gitignored**. The repository
`.gitignore` excludes `/logs/`, `/benchmark/logs/`, and the pattern
`*-resource-*.csv`. These files are local diagnostic artifacts and must
never be committed. Store them in `logs/` (the script default) or any
convenient location outside the repository.

### Resource log format (CSV)

The extended CSV format (19 fields per row) written by the historical
monitor:

```
timestamp,pid,elapsed_sec,cpu_pct,rss_kb,hwm_kb,vmsize_kb,rssanon_kb,rssfile_kb,pss_kb,swap_kb,threads,fd_count,minflt,majflt,voluntary_ctxt,nonvoluntary_ctxt,read_bytes,write_bytes
```

| Field | Description |
|---|---|
| `timestamp` | ISO 8601 timestamp with timezone offset (e.g. `2026-05-31T11:35:26+07:00`) |
| `pid` | Process ID of the running cosmostrix instance |
| `elapsed_sec` | Seconds since the first sample (monotonic) |
| `cpu_pct` | CPU usage percentage from `/proc/<pid>/stat` |
| `rss_kb` | Resident Set Size in kB from `VmRSS` in `/proc/<pid>/status` |
| `hwm_kb` | Peak RSS (VmHWM) in kB from `/proc/<pid>/status` |
| `vmsize_kb` | Virtual memory size in kB from `VmSize` in `/proc/<pid>/status` |
| `rssanon_kb` | Anonymous RSS in kB from `RssAnon` in `/proc/<pid>/status` |
| `rssfile_kb` | File-backed RSS in kB from `RssFile` in `/proc/<pid>/status` |
| `pss_kb` | Proportional Set Size in kB from `/proc/<pid>/statm` |
| `swap_kb` | Swap usage in kB from `VmSwap` in `/proc/<pid>/status` |
| `threads` | Number of threads from `Threads` in `/proc/<pid>/status` |
| `fd_count` | Number of open file descriptors from `ls /proc/<pid>/fd \| wc -l` |
| `minflt` | Minor page faults (cumulative) from `/proc/<pid>/stat` |
| `majflt` | Major page faults (cumulative) from `/proc/<pid>/stat` |
| `voluntary_ctxt` | Voluntary context switches (cumulative) from `/proc/<pid>/stat` |
| `nonvoluntary_ctxt` | Involuntary context switches (cumulative) from `/proc/<pid>/stat` |
| `read_bytes` | Bytes read (cumulative) from `/proc/<pid>/io` |
| `write_bytes` | Bytes written (cumulative) from `/proc/<pid>/io` |

#### Legacy format

The legacy 4-column format predates the extended one:

```
timestamp,rss_kb,fd_count,elapsed_secs
```

The removed summary script auto-detected the two formats by the presence
of the extended columns (`pid`, `cpu_pct`, `hwm_kb`, etc.).

### Sampling interval

The recommended interval is 60 seconds (`INTERVAL=60`). For shorter test runs
(e.g. 1-hour smoke tests) a 10-second interval provides higher resolution.

## Acceptance criteria

| Criterion | Threshold | Rationale |
|---|---|---|
| RSS growth | < 2% per hour | Permits minor heap fragmentation; rejects leaks |
| FD count | Monotonically stable or decreasing | Detects file descriptor / handle leaks |
| Swap | Zero throughout run | Non-zero swap indicates memory pressure |
| Crash / panic | None | Renderer must exit cleanly on duration expiry |
| Clean exit | Exit code 0 | Confirms graceful shutdown path |
| Crystal Dragon palette drift | None when `crystal-dragon` is off | Fixed colors must remain sticky |

### Color stability endurance

Starting with v3.7.0, endurance runs should also verify color stability:
if the session was started with an explicit color (e.g., `--color sun`),
the color must remain unchanged for the entire duration. This is enforced
by deterministic in-process tests that simulate many minutes of frames and
assert the `ColorScheme` never changes. The color stability policy is
enforced by `src/engine/chroma_dragon_engine/palette/mod.rs`: palette is sticky for the entire session
unless `--crystal-dragon` is explicitly opted in.

To manually verify during a long endurance run, check that the `--color`
value you passed at startup is still active at the end of the run. If
`crystal-dragon` is enabled (opt-in), color changes are expected and
acceptable.

Use `--doctor` to confirm the drift state at any time:

```bash
cosmostrix --doctor | rg "crystal_dragon"
# crystal_dragon: false   <- default, no autonomous drift
# crystal_dragon: true    <- opt-in, Crystal Dragon may change color
```

### Pass/fail logic

- **PASS**: All criteria met for the full duration. Verdict: `PASS / stable`.
- **FAIL**: Any single criterion violated (RSS exceeds hourly budget, fd_count
  increases by more than a transient spike, non-zero swap detected, unexpected
  exit, or non-zero exit code).

A transient FD spike (e.g. +/-2 handles) during a single sample is acceptable
as long as the count returns to baseline by the next sample.

## Past results

### Run — v80.0.0-beta.1.0-beta.1 — 2026-08-30 — linux-amd64-v1-gnu (SHORT-RUN SMOKE, not a 2h soak)

> Recorded by the LTS matrix mid-session retest audit
> (`docs/archive/audits/LTS_MATRIX_MIDSESSION_RETEST.md`). These are short-run
> stability smoke points that re-establish a fresh baseline for the
> v50/v80.0.0-beta.1 line — the previous record below is v4.0.1 (2026-06-11). A
> full 2 h soak on target hardware is still recommended.

Run A — benchmark mode:

| Item | Value |
|---|---|
| cosmostrix version | 51.0.0-beta.1 (commit 29f2f00a) |
| Build profile | release (linux-amd64-v1-gnu) |
| Duration | 90 s (`--benchmark --bench-io --bench-duration 90s`) |
| Sampling interval | 3 s |
| Terminal size | headless virtual (bench default) |

**Results:**

| Metric | Value | Pass? |
|---|---|---|
| RSS after warmup | 4520 kB (flat from t=3 s to t=90 s) | PASS (0% growth) |
| HWM | 4524 kB | — |
| RSS growth % | 0% (post-warmup) | PASS |
| Threads | 1 stable | — |
| Crashes / panics | no (exit 0) | PASS |
| Allocator | alloc_calls_per_frame 0.0, heap_retained 0 B | PASS (no leak signature) |

Run B — interactive mode (PTY, 120x35, TERM=xterm-256color):

| Item | Value |
|---|---|
| Duration | 120 s with mid-session keys (HUD on/off, pause/resume, density -/+) |
| Sampling interval | 3 s |

**Results:**

| Metric | Value | Pass? |
|---|---|---|
| RSS profile | 5356 kB flat t=3..75 s; one bounded +272 kB step at t~78 s; flat to end | PASS (step = palette/SGR cache rebuild, not creep) |
| HWM | 5628 kB | — |
| Threads | 1 at spawn, 5 from t=6 s, stable | — |
| Exit | 0 via `q` after all interactions | PASS |
| Crashes / panics | no | PASS |

**Notes:** the single +272 kB step coincides with the HUD-off/ambient
window and matches one palette-cache rebuild (bounded per palette). A leak
signature would be monotonic creep; step-then-flat is bounded caching.
Long-run confirmation at the built-in palette ceiling (~12 MiB worst case)
awaits the recommended 2 h soak.

### Run — v4.0.1 — 2026-06-11 — linux-x86_64-v3

| Item | Value |
|---|---|
| cosmostrix version | 4.0.1 |
| Build profile | release (linux-x86_64-v3) |
| Duration target | ~2h |
| Actual duration | ~1h45m |
| Sampling interval | 60s |
| Terminal size | default |
| Color mode | default |
| OS / kernel | Linux |
| CPU | — |
| Exit code | 0 |

**Results:**

| Metric | Value | Pass? |
|---|---|---|
| Start RSS | ~4.3 MiB | — |
| End RSS | ~4.3 MiB | — |
| Max RSS | ~4.3 MiB | — |
| HWM | ~4.3 MiB | — |
| RSS growth % | ~0% | PASS |
| PSS max | — | — |
| Swap max | 0 kB | PASS |
| Start FD count | 10 | — |
| End FD count | 10 | — |
| FD leak detected | no | PASS |
| Threads | 4 stable | — |
| CPU avg | ~0.82% | — |
| Major faults delta | 0 | — |
| Disk I/O | 0 read / 0 write | — |
| Crashes / panics | no | PASS |

**Notes:**

RSS remained flat at ~4.3 MiB for the entire run. FD count held at 10,
threads at 4, swap at 0, major faults at 0, and disk I/O at zero. CPU
averaged ~0.82% which is typical idle behavior for a terminal screensaver
waiting on vsync. No anomalies observed.

---

Template for recording future endurance run results:

### Run — [version] — [date] — [platform]

| Item | Value |
|---|---|
| cosmostrix version | |
| Build profile | |
| Duration target | |
| Actual duration | |
| Sampling interval | |
| Terminal size | |
| Color mode | |
| OS / kernel | |
| CPU | |
| Exit code | |

**Results:**

| Metric | Value | Pass? |
|---|---|---|
| Start RSS | kB | — |
| End RSS | kB | — |
| Max RSS | kB | — |
| HWM | kB | — |
| RSS growth % | % | PASS / FAIL |
| PSS max | kB | — |
| Swap max | kB | PASS / FAIL |
| Start FD count | | — |
| End FD count | | — |
| FD leak detected | yes / no | PASS / FAIL |
| Threads | start / end / max | — |
| CPU avg | % | — |
| Major faults delta | | — |
| Crashes / panics | yes / no | PASS / FAIL |

**Notes:**

_(describe any anomalies, transient spikes, or environmental factors)_

---
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
