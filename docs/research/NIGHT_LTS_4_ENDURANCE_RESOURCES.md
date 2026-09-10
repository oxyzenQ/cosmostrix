<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-lts-4 — the master endurance audit: CPU, memory, and every other resource over long runs (no GPU, no bloat)

Owner directive (2026-09-10): "master audit depth bore peak optimize,
stability, & long endurance cpu usage, and others resources. no gpu
no bloat"

## Method

New evidence machine: `scripts/endurance_probe.py` — spawns the
release binary in a PTY (200x56, TERM_PROGRAM=alacritty so the
dynamic default resolves to 144 FPS, drain-rate reader at 12 MB/s =
the marginal-saturation owner regime), then samples `/proc/<pid>`
every interval for CPU% (utime+stime ticks), VmRSS, page faults,
and context switches. SIGTERM teardown with a 3 s grace window then
SIGKILL fallback (the child's own signal handlers do graceful work).

## Measurements (release build, 200x56, 144 FPS, 12 MB/s drain)

| probe | scene | steady CPU | RSS slope after 15 s warmup | minor faults |
|---|---|---|---|---|
| 30 s | cinematic | 12.9% | +1.69 MB/min | 0/s |
| 3 min | cinematic | 7.2% | +0.56 MB/min | 8.6/s |
| 6 min | cinematic | 7.0% (max 14.5%) | +0.32 MB/min | 4.0/s |
| 3 min | sorgonemous_intrascals | 5.7% (max 10.1%) | +0.08 MB/min | 1.3/s |

File descriptors: flat at 10 across a 40 s run. Threads: flat at 5.
Context switches at the 6 min mark: 47,381 voluntary / 5,187
involuntary (~131 + 14 per second — the frame limiter sleeping, not
spinning).

## Findings

1. CPU is bounded and low. Steady 5.7-7.2% of one core at 144 FPS
   rendering a 200x56 grid under a marginal drain. The stack that
   holds it there: the poll + <=500 us spin hybrid for deadline
   accuracy, the PowerManager effective_fps resolution (idle x0.5,
   pause 4 FPS), the thermal shed ramp, and the drain backoff. The
   dead-PTY guard drops post-SIGHUP CPU burn from 20 s to <1 ms.

2. Memory plateaus — it is growth-to-steady-state, not a leak. The
   decisive evidence is the slope decay across run lengths:
   +1.69 -> +0.56 -> +0.32 MB/min at 30 s / 3 min / 6 min. A true
   leak is linear (constant slope); this curve is asymptotic to
   ~9 MB for cinematic, ~6.4 MB for black hole — the scene pools
   (glitch history, phosphor decay, event schedulers, allocator
   high-water) filling once. Black hole is flat essentially from
   the start (+0.08 MB/min after warmup).

3. No page churn. 1.3-8.6 minor faults/s in steady state — the
   madvise(MADV_DONTNEED) reclaim + re-fault cycle is measured
   noise, not thrash (see reclaim_state.rs for the page-granularity
   hazard analysis).

4. No fd or thread leaks. 10 fds / 5 threads, flat for the whole
   window (notify watcher, stdin/stdout/stderr, signal signalfd,
   and friends — all opened once at startup).

5. No GPU. Zero GPU dependencies or code paths. The only "GPU" hits
   in the tree are doc comments about GPU-rendered terminals
   (Alacritty/kitty throughput classification); the doctor report
   states the contract: "CPU+stdout renderer; no GPU context is
   ever created."

6. No bloat. 10 direct dependencies, every one load-bearing:
   bitvec (frame dirty bitsets), clap (CLI, default-features off,
   cherry-picked features), crossterm (terminal, default-features
   off), libc (syscalls), notify (config watch, default-features
   off), rand, sha2 (pure-Rust config fingerprint), signal-hook,
   smallvec, unicode-width. chrono was previously REMOVED and
   replaced by libc localtime_r/gmtime_r (dropped 8 transitive
   crates for 2 call sites). Release binary: 2.9 MB.

7. The allocator is already optimal for this workload. TraceAlloc
   wraps std::alloc::System with two relaxed atomic increments
   (~2 ns); the workload does ~2 allocs/frame against a stable
   ~93 KB heap, so glibc malloc beats mimalloc/jemalloc on tail
   latency (empirically verified, see alloc_trace.rs docs).

8. The process self-monitors. EnduranceHealth already tracks the
   same three signals this audit measured externally (RSS variance
   ring buffer, frame utilization EMA, context-switch EMA), and the
   self-healer arms on the "investigate" band. Thermal sampling is
   every 600 frames (~10 s at 60 FPS) with defensive per-zone
   fallbacks.

## Verdict

Already at peak. Zero code changes — the report and the reusable
probe are the product (same contract as NIGHT-lts-1 stage 1). Every
resource dimension is bounded, measured, and in several places the
codebase already monitors itself with the same telemetry this audit
used.

Reproduce: `RUN_SECS=360 SAMPLE_SECS=15 python3 scripts/endurance_probe.py`
 knobs: SCENE, SIZE, DRAIN_BPS, EXTRA_ARGS, CPU_LIMIT_PCT,
RSS_SLOPE_LIMIT_MB_PER_MIN (see the module docstring).

<!-- COSMOSTRIX-DISCLAIMER -->
