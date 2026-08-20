# Advanced Benchmarking Guide
<!-- SPDX-License-Identifier: GPL-3.0-only -->

> How to unlock the MICROARCHITECTURE and ENERGY sections of `cosmostrix --benchmark`, and how to interpret the resulting metrics.

## Overview

`cosmostrix --benchmark` always emits a baseline report covering FPS, frame-time percentiles, dirty-cell ratio, and allocator activity. Two additional sections require elevated privileges and are gated behind platform-specific kernel interfaces:

| Section            | Source                     | Platform        | Default state    |
|--------------------|----------------------------|-----------------|------------------|
| MICROARCHITECTURE  | `perf_event_open` syscall  | Linux x86 only  | not available    |
| ENERGY             | RAPL powercap sysfs        | Linux only      | not available    |

When either is unavailable the report prints `status: not available` with a hint pointing back to this document. The sections are entirely opt-in — cosmostrix never silently probes privileged interfaces when you only want FPS numbers.

## 1. Enabling MICROARCHITECTURE Metrics

The microarchitecture section reads hardware performance counters via the Linux `perf_event_open(2)` syscall: CPU cycles, retired instructions, branch instructions, and branch misses. From these it derives IPC (instructions per cycle) and branch mispredict rate.

**Why off by default**: Linux exposes `kernel.perf_event_paranoid` sysctl, controlling unprivileged access. Value 3 = no perf events for unprivileged users; 2 = kernel-level only (default on many distros); 1 = kernel + user-level for own processes; 0 = all counters to all users. cosmostrix only needs its own process counters, so `perf_event_paranoid=1` is the right setting. Anything stricter causes `perf_event_open` to return `EACCES`.

**Temporary setup** (one command, resets on reboot):

```bash
sudo sysctl -w kernel.perf_event_paranoid=1
cat /proc/sys/kernel/perf_event_paranoid   # should print: 1
cosmostrix --benchmark --screen-size 120x40 --bench-duration 10s
```

**Permanent setup**:

```bash
echo 'kernel.perf_event_paranoid=1' | sudo tee /etc/sysctl.d/99-perf.conf
sudo sysctl --system
```

**Verification**: after setup, the MICROARCHITECTURE section should show `status: available (perf_event_open)` with `cycles`, `instructions`, `ipc`, `branch_instructions`, `branch_misses`, `branch_mispredict_rate`. A healthy IPC is >2.0; BOLT lookup tables keep branch mispredict rate <2%. If the section still reports "not available" after setting `perf_event_paranoid=1`, you are likely running inside a VM or container that blocks `perf_event_open` at the hypervisor or seccomp-filter layer — bare metal or a privileged container usually resolves this.

## 2. Enabling ENERGY Metrics (RAPL)

The energy section reads Intel/AMD RAPL (Running Average Power Limit) counters from `/sys/class/powercap/intel-rapl:*/energy_uj` to compute joules consumed, average power draw, and energy per frame/cell during the benchmark run.

**Why off by default**: the `energy_uj` files are owned by `root:root` with mode `0400` on most distributions — world-readable access is denied to prevent side-channel attacks through power observations.

**Temporary setup** (resets on reboot — `sysfs` is a virtual filesystem):

```bash
sudo chmod a+r /sys/class/powercap/intel-rapl:*/energy_uj
cat /sys/class/powercap/intel-rapl:0/energy_uj   # should print microjoules since boot
cosmostrix --benchmark --screen-size 120x40 --bench-duration 10s
```

**Permanent setup** (systemd tmpfiles — see [RAPL_ACCESS.md](RAPL_ACCESS.md) for the full procedure and alternative methods like `setcap`):

```bash
sudo tee /etc/tmpfiles.d/rapl.conf << 'EOF'
f /sys/class/powercap/intel-rapl:0/energy_uj 0444 - - -
f /sys/class/powercap/intel-rapl:0/intel-rapl:0:0/energy_uj 0444 - - -
EOF
sudo systemd-tmpfiles --create
```

**Verification**: after setup, the ENERGY section should show `status: available (RAPL)` with `packages`, `total_energy` (J), `avg_power` (W), `energy_per_frame` (µJ), `energy_per_cell` (nJ).

## 3. Running a Detailed Benchmark

With both privileged sections enabled, capture a full machine-readable report:

```bash
cosmostrix --benchmark --bench-duration 60s --json > report.json
```

Useful flag combinations: `cosmostrix --benchmark` (5s smoke test); `--bench-duration 60s` (steady-state); `--json` (CI/scripts); `--screen-size 200x60` (fixed size for cross-run compare); `--bench-all` (scaling sweep); `--save-baseline base.json` / `--compare-baseline base.json` (regression detection); `--bench-io` (wet I/O throughput to /dev/null); `--bench-io --bench-scene production-draw` (production render path, BOLT-backed); `--bench-all --bench-io --bench-scene production-draw --bench-duration 3s` (wet I/O scaling sweep). See [BENCHMARKING.md](BENCHMARKING.md) for the full flag reference.

## 4. Key Metrics Explained

The benchmark report (and JSON output) exposes metrics that together describe cosmostrix's diff-based rendering architecture. Understanding them is the difference between "fast" and "proven O(1) scaling".

- **`total_ns_per_cell`** — the O(1) scaling proof. Total nanoseconds spent rendering divided by total dirty (changed) cells written to the terminal. Because cosmostrix uses diff-based rendering, only cells that actually change between frames are written — so this metric is per dirty cell, not per screen cell. As the terminal grows larger, `total_ns_per_cell` stays flat (±10% jitter from cache effects) while `avg_fps` drops simply because there are more dirty cells per frame. Single most important metric for verifying the diff engine's O(1) per-cell claim.
- **`dirty_cell_ratio`** — diff engine efficiency. Fraction of screen cells that changed this frame. Lower = more efficient diff. v30 monolith: ~6%; v30 400×200: ~1.8%. As screen size grows, this ratio drops (most cells unchanged per frame), which is why `avg_fps` doesn't fall off a cliff at large sizes.
- **`ipc`** — instructions per cycle. >2.0 = healthy; >3.0 = excellent. v50 PGO v3: 2.52–2.62 (Zen 3). Below 2.0 suggests the binary was built without AVX2 (`pro-linux-v3`) or AVX-512 (`pro-linux-v4`).
- **`branch_mispredict_rate`** — branch predictor failure rate. <2% = BOLT lookup tables working. v30: 0.57–2.41%. High mispredict rates suggest BOLT was not applied (use `pro-linux-v3`/`pro-linux-v4` profile or `nitro-pgo`).
- **`energy_per_cell`** — size-independent energy metric (nJ). Lower = more efficient. v50 PGO v3 wet: 1,520.7 nJ. Useful for comparing energy efficiency across screen sizes — total energy grows with screen size, but `energy_per_cell` should stay flat.
- **`alloc_calls_per_frame`** — fresh allocations per frame. v30 baseline: 3.00 (constant). Higher = leaking heap. The render + I/O hot path is zero-alloc by design; any non-zero growth across runs indicates a regression.
- **`heap_retained`** — bytes allocated and never freed. v30: 0 (zero retained). Non-zero = investigate. Use `--bench-duration 5m` to confirm a leak — `heap_retained` should remain 0 even on long runs.

## 5. Interpreting Results + Pitfalls

**Don't compare across different CPUs** without normalizing. A 163K FPS result on one machine is not directly comparable to a 100K FPS result on a Ryzen 5800HS — different microarchitectures, cache hierarchies, memory bandwidth. Compare same-CPU before/after changes, or use `energy_per_cell` (size-independent) for cross-machine efficiency comparison. **Warmup matters**: the first 2 seconds of any benchmark run are warmup (caches filling, branch predictor training). The default 5s benchmark (2s warmup + 3s measurement) handles this; for `--bench-duration 60s`, discard the first ~5s when computing steady-state numbers. **Wet vs dry is not a small difference**: dry benchmarks skip the kernel syscall path entirely — `write_bandwidth`, `avg_write_latency`, `backpressure_events` are all zero/missing. Always use `--bench-io` when measuring real-world I/O performance. The production render path (`--bench-scene production-draw`) additionally routes through `BenchIoWriter` to measure the full-redraw I/O cost. **Energy readings are package-level, not per-core**: RAPL measures the entire CPU package's energy draw, including idle cores. On a multi-core system running nothing but cosmostrix, `avg_power` includes the idle power of all other cores. For per-core energy, use a different tool (e.g., `perf stat -e power/energy-pkg/`). **`perf_event_open` in containers**: Docker/Podman block `perf_event_open` by default via seccomp. Run with `--security-opt seccomp=unconfined` or `--cap-add SYS_PTRACE` (more conservative). Kubernetes pods need the same via securityContext.

## See Also

- [BENCHMARKING.md](BENCHMARKING.md) — main benchmarking guide (modes, metrics, honesty contract)
- [RAPL_ACCESS.md](RAPL_ACCESS.md) — full RAPL access setup (setcap, systemd tmpfiles, kernel module options)
- [PERFORMANCE_ACROSS_SCALES.md](PERFORMANCE_ACROSS_SCALES.md) — how FPS scales with screen size
- [ENDURANCE.md](ENDURANCE.md) — long-run stability and leak detection
- [RELEASE_GUARD.md](RELEASE_GUARD.md) — performance regression gates for releases
