# Advanced Benchmarking Guide

<!-- SPDX-License-Identifier: GPL-3.0-only -->

> How to unlock the MICROARCHITECTURE and ENERGY sections of
> `cosmostrix --benchmark`, and how to interpret the resulting metrics.

## Overview

`cosmostrix --benchmark` always emits a baseline report covering FPS,
frame-time percentiles, dirty-cell ratio, and allocator activity. Two
additional sections require elevated privileges and are gated behind
platform-specific kernel interfaces:

| Section            | Source                     | Platform        | Default state    |
|--------------------|----------------------------|-----------------|------------------|
| MICROARCHITECTURE  | `perf_event_open` syscall  | Linux x86 only  | not available    |
| ENERGY             | RAPL powercap sysfs        | Linux only      | not available    |

When either is unavailable the report prints `status: not available`
with a hint pointing back to this document. The sections are entirely
opt-in — cosmostrix never silently probes privileged interfaces when
you only want FPS numbers.

## 1. Enabling MICROARCHITECTURE Metrics

The microarchitecture section reads hardware performance counters via
the Linux `perf_event_open(2)` syscall: CPU cycles, retired
instructions, branch instructions, and branch misses. From these it
derives IPC (instructions per cycle) and branch mispredict rate.

### Why it is off by default

Linux exposes a sysctl knob, `kernel.perf_event_paranoid`, that
controls unprivileged access to perf counters:

| Value | Meaning                                                          |
|-------|------------------------------------------------------------------|
| 3     | No perf events available to unprivileged users (most locked-down)|
| 2     | Kernel-level counters only (default on many distros)             |
| 1     | Kernel + user-level counters for your own processes              |
| 0     | All counters accessible to all users (permissive)                |

cosmostrix only needs to read counters for its own process, so
`perf_event_paranoid=1` is the right setting. Anything stricter
causes `perf_event_open` to return `EACCES` and the section falls
back to "not available".

### Temporary setup (one command)

```bash
# Relax the paranoid level for the current boot session
sudo sysctl -w kernel.perf_event_paranoid=1

# Verify
cat /proc/sys/kernel/perf_event_paranoid
# Should print: 1

# Run the benchmark — MICROARCHITECTURE section now populates
cosmostrix --benchmark --screen-size 120x40 --bench-duration 10s
```

The change resets on reboot because sysctl values are not persistent
unless written to a config file under `/etc/sysctl.d/`.

### Permanent setup

```bash
# Persist across reboots
echo 'kernel.perf_event_paranoid=1' | sudo tee /etc/sysctl.d/99-perf.conf

# Apply immediately
sudo sysctl --system
```

### Verification

After setup, run any benchmark and inspect the MICROARCHITECTURE
section of the report. A healthy result looks like:

```
MICROARCHITECTURE
  status: available (perf_event_open)
  cycles: 4.21B
  instructions: 5.83B
  ipc: 1.39
  branch_instructions: 612M
  branch_misses: 18.4M
  branch_mispredict_rate: 3.01%
  note: Linux x86 perf counters; varies by CPU model
```

If the section still reports "not available" after setting
`perf_event_paranoid=1`, you are likely running inside a VM or
container that blocks `perf_event_open` at the hypervisor or
seccomp-filter layer. Bare metal or a privileged container usually
resolves this.

## 2. Enabling ENERGY Metrics (RAPL)

The energy section reads Intel/AMD RAPL (Running Average Power Limit)
counters from `/sys/class/powercap/intel-rapl:*/energy_uj` to compute
joules consumed, average power draw, and energy per frame/cell during
the benchmark run.

### Why it is off by default

The `energy_uj` files are owned by `root:root` with mode `0400` on
most distributions — world-readable access is denied to prevent
side-channel attacks through power observations.

### Temporary setup (one command)

```bash
# Make all RAPL energy_uj files world-readable for this boot session
sudo chmod a+r /sys/class/powercap/intel-rapl:*/energy_uj

# Verify
cat /sys/class/powercap/intel-rapl:0/energy_uj
# Should print a number (microjoules since boot)

# Run the benchmark — ENERGY section now populates
cosmostrix --benchmark --screen-size 120x40 --bench-duration 10s
```

Permissions reset on reboot because `sysfs` is a virtual filesystem.

### Permanent setup (systemd tmpfiles)

For persistence across reboots, use systemd tmpfiles. See
[docs/RAPL_ACCESS.md](RAPL_ACCESS.md) for the full procedure — the
short version:

```bash
sudo tee /etc/tmpfiles.d/rapl.conf << 'EOF'
f /sys/class/powercap/intel-rapl:0/energy_uj 0444 - - -
f /sys/class/powercap/intel-rapl:0/intel-rapl:0:0/energy_uj 0444 - - -
EOF
sudo systemd-tmpfiles --create
```

Other methods (setcap, running as root) are documented in
[RAPL_ACCESS.md](RAPL_ACCESS.md).

### Verification

After setup, the ENERGY section should show:

```
ENERGY
  status: available (RAPL)
  packages: 1
  total_energy: 38.42 J
  avg_power: 3.84 W
  energy_per_frame: 64.0 µJ
  energy_per_cell: 13.3 nJ
```

## 3. Running a Detailed Benchmark

With both privileged sections enabled, capture a full machine-readable
report with:

```bash
cosmostrix --benchmark --bench-duration 60s --json > report.json
```

Useful flag combinations:

| Goal                                   | Command                                                                  |
|----------------------------------------|--------------------------------------------------------------------------|
| Quick 5s smoke test                    | `cosmostrix --benchmark`                                                 |
| 60s steady-state measurement           | `cosmostrix --benchmark --bench-duration 60s`                            |
| JSON for CI/scripts                    | `cosmostrix --benchmark --json --bench-duration 30s > report.json`       |
| Fixed screen size for cross-run compare| `cosmostrix --benchmark --screen-size 200x60 --bench-duration 30s`       |
| Automated scaling sweep                | `cosmostrix --bench-all --bench-duration 5s`                             |
| Save baseline for later diffing        | `cosmostrix --benchmark --save-baseline base.json`                       |
| Compare current run to saved baseline  | `cosmostrix --benchmark --compare-baseline base.json`                    |
| Wet I/O throughput (writes to /dev/null)| `cosmostrix --benchmark --bench-io --bench-duration 30s`                |
| Production render path (BOLT-backed)   | `cosmostrix --benchmark --bench-io --bench-scene production-draw --bench-duration 30s` |
| Wet I/O scaling sweep                  | `cosmostrix --bench-all --bench-io --bench-scene production-draw --bench-duration 3s` |

## 4. Key Metrics Explained

The benchmark report (and JSON output) exposes a handful of metrics
that together describe cosmostrix's diff-based rendering architecture.
Understanding them is the difference between "fast" and "proven O(1)
scaling".

### `total_ns_per_cell` — the O(1) scaling proof

Total nanoseconds spent rendering, divided by the total number of
dirty (changed) cells written to the terminal. Because cosmostrix
uses diff-based rendering, only cells that actually change between
frames are written — so this metric is **per dirty cell**, not per
screen cell.

The implication: as the terminal grows larger, `total_ns_per_cell`
stays flat (with maybe ±10% jitter from cache effects) while
`total_ns_per_screen_cell` collapses. If you plot it across screen
sizes via `--bench-all`, the line should be near-horizontal. That is
the signature of O(1) scaling — the renderer's cost tracks the
*change set*, not the canvas size.

Typical values on modern x86_64 hardware:

| Hardware class             | `total_ns_per_cell` |
|----------------------------|---------------------|
| High-end desktop (Ryzen 9) | 40–60 ns/cell       |
| Mid-range laptop           | 70–110 ns/cell      |
| Raspberry Pi 4 (ARM Cortex-A72) | 300–500 ns/cell |
| Slow VM / container        | 100–300 ns/cell     |

If your number is much higher than these, the bottleneck is usually
I/O, not CPU — see `io_share_percent` below.

### `io_share_percent` — write-side I/O bottleneck detector

The fraction of total frame time spent in `write(2)` syscalls to the
PTY. Diff-based rendering means cosmostrix already minimizes bytes
written, but every byte still crosses the user/kernel boundary.

- **< 30%**: CPU-bound (good — the renderer is the dominant cost)
- **30–60%**: Balanced (typical for fast terminals like kitty, alacritty)
- **> 60%**: I/O-bound — terminal or kernel PTY layer is the bottleneck.
  Switching terminals (try kitty, foot, WezTerm), reducing `--density`,
  or enabling phosphor compression may help.

### `ipc` — instructions per cycle

From the MICROARCHITECTURE section. Healthy code runs at IPC > 1.0 on
modern superscalar CPUs. cosmostrix typically lands at 1.2–1.6 because
its hot loops are linear scans over cell arrays with predictable
branch patterns and few dependencies.

- **IPC < 0.7**: Something is stalling — cache misses, branch
  mispredicts, or memory-bound loops. Investigate with `perf record`.
- **IPC 0.7–1.2**: Normal for code with mixed workloads.
- **IPC > 1.2**: Excellent — the CPU is retiring more than one
  instruction per cycle on average.

### `energy_per_frame` — efficiency at fixed workload

Microjoules consumed per rendered frame. Combine with
`avg_fps` to compute `energy_per_second = energy_per_frame * avg_fps`
if you need a per-second power figure.

This metric shines for comparing optimization A vs. optimization B
on the same hardware: if A renders at the same FPS as B but consumes
30% less `energy_per_frame`, A is strictly better. Raw FPS alone
cannot make that distinction.

### `avg_dirty_cell_ratio_percent` — diff efficiency

The percentage of screen cells that changed between consecutive
frames. Cosmostrix targets < 10% on a steady rain scene — that is the
whole point of diff-based rendering: 90%+ of the screen is untouched
each frame, so the renderer only writes the diff.

- **< 5%**: Excellent — most cells are static phosphor decay.
- **5–15%**: Normal for active rain with several streamers.
- **> 30%**: Something is forcing full-screen redraws (high wind,
  density anomaly, or a bug in the dirty-tracking layer).

## 5. Automated Scaling Tests with `--bench-all`

`--bench-all` runs the benchmark across a fixed ladder of screen
sizes (6x6 to 200x60) and prints a SCALING SUMMARY table. Use it to
verify that `total_ns_per_cell` stays flat as the screen grows — the
empirical proof of O(1) scaling.

```bash
cosmostrix --bench-all --bench-duration 5s
```

Sample output (abbreviated):

```
[bench-all] Running 8 benchmarks (5s each)...

[bench-all] 6x6...
[bench-all] 40x20...
[bench-all] 80x30...
[bench-all] 120x40...
[bench-all] 160x50...
[bench-all] 200x60...

SCALING SUMMARY
size     cells    avg_fps  total_ns_per_cell  avg_dirty_cells  allocs/frame
6x6      36       4200     48                 4                0.0
40x20    800      1850     52                 38               0.0
80x30    2400     1620     55                 105              0.0
120x40   4800     1480     58                 220              0.0
160x50   8000     1330     61                 380              0.0
200x60   12000    1180     64                 560              0.0
```

The key signal: `total_ns_per_cell` stays in the 48–64 ns band across
a 333x cell-count increase. That is the diff-based rendering promise,
quantified. For a deeper analysis of where this scaling comes from
and where it breaks down, see
[docs/PERFORMANCE_ACROSS_SCALES.md](PERFORMANCE_ACROSS_SCALES.md).

## See Also

- [docs/RAPL_ACCESS.md](RAPL_ACCESS.md) — full RAPL access methods
  (tmpfiles, setcap, root)
- [docs/PERFORMANCE_ACROSS_SCALES.md](PERFORMANCE_ACROSS_SCALES.md) —
  scaling audit and optimization history
- [docs/SIMD_FEASIBILITY.md](SIMD_FEASIBILITY.md) — CPU microarchitecture
  analysis that informed the perf-counter design
- [docs/ENDURANCE.md](ENDURANCE.md) — long-run resource monitoring
