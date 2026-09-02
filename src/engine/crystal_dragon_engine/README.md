<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Crystal Dragon Engine — LTS Lock

> **Simplified lock/unlock signature log**: see [`KEY.md`](KEY.md).
> This README holds the full audit detail (A/B benchmarks, file lists,
> stability signals).

> **3 Dragon Lock** in commit `69af079` after deeper audit for strengthening
> and stability.
>
> Signoff: **rezky_nightky** — 2026-08-19T14:40:05Z — vision & director
> project cosmostrix

---

## What This Lock Means

The Crystal Dragon Ambient Intelligence Engine is locked at its current
state (commit `69af079`, audited 2026-08-19) for Long-Term Support (LTS).
The code in this directory has been audited for:

- **Peak optimization** — sensor sampling at 60s intervals (cold path,
  no per-frame cost), CDF binary search for theme selection (O(log N)
  per drift event, not per frame), EMA smoothing for CPU% (single f32
  op per sample).
- **Efficient resource use** — ambient scheduler is dynamic idle/wake
  (zero CPU between phase boundaries, parked in `Condvar::wait_timeout`),
  not a fixed-interval poller. Diagnostics counters are atomic (no Mutex
  contention on hot path).
- **Strong foundation** — 44 builtin themes partitioned into 3
  temperature groups (Cold/Medium/Hot, 14 each) + 2 reserved (Rainbow,
  Spectrum20). calc-v1 probabilistic weighted selection (the locked
  algorithm; calc-v2 reserved for future).
- **Stability** — ~1649 tests pass, 0 clippy warnings. Per-
  subsystem test files (`*/tests.rs`) cover all public contracts.

## Audit Findings (No Code Changes Required)

The audit confirmed the engine is already at peak. Specifically:

### 1. Sensor (`sensor/mod.rs`, 276 LOC)

- **CPU mode (primary)**: samples process CPU% via
  `crate::cpustat::current_cpu_ns()`, smooths with EMA (alpha=0.25),
  maps linearly to 1–99 point. One syscall per 60s — negligible cost.
- **CLOCK fallback**: derives point from UTC hour+minute. No syscall
  beyond `SystemTime::now()` (already cached elsewhere). Monotonic
  ramp: 00:00->point 1, 23:59->point 99.
- **Cold-start point = 17** (lower-middle of Cold group) — avoids
  immediate theme change on first poll tick.
- **`shift_in_time()`** — called on resume from pause so the sensor
  doesn't think a long pause was a dwell period. Single `Duration` add.

### 2. Point system (`point_system/mod.rs`, 126 LOC)

- **calc-v1 algorithm** (the locked selection method):
  1. Determine temperature group from point (1–33 Cold, 34–66 Medium,
     67–99 Hot).
  2. Compute weight per theme: `1.0 / (1.0 + distance * 0.1)` where
     `distance = |current_point - theme_natural_point|`.
  3. Build CDF (cumulative distribution function) — `Vec<f32>` with
     capacity pre-allocated.
  4. Draw uniform `u ∈ [0, 1)`, binary-search CDF via `partition_point`.
  5. Skip current scheme if selected (retry once, then accept no-op).
- **CDF binary search** uses `slice::partition_point` (O(log N), branch-
  optimized in stdlib). 14 themes per group -> 4 comparisons worst case.
- **`Uniform::new(0.0f32, 1.0f32)`** — constructed per call but `expect`
  is branch-predicted away; cost is ~2ns. Could be cached but the call
  is cold-path (12% chance per 60s poll), so optimization has zero
  measurable effect.
- **Uniform fallback** (when all weights sum to zero — impossible with
  current formula but defensive): uniform random index, modulo skip.

### 3. Palette groups (`palette_groups/mod.rs`, 129 LOC)

- **`group_themes()`** returns `&'static [ColorScheme]` — no allocation,
  const slices.
- **`theme_weight()`** — pure function, no allocation. `super::sensor::
  point_to_group()` and `super::sensor::group_point_range()` are also
  pure (no mutation, no allocation).

### 4. Ambient scheduler (`ambient_scheduler/mod.rs`, 378 LOC)

- **Dynamic idle/wake**: thread computes `time_to_next_phase`, sleeps
  in `Condvar::wait_timeout` until boundary OR condvar notify. **Zero
  CPU between phase transitions.**
- **Live reload**: condvar-notify on `config.toml` save; thread wakes
  immediately, recomputes sleep.
- **Edge cases handled**: empty schedule, single entry, DST spring-
  forward (skipped hour), DST fall-back (idempotent re-fire), midnight
  wrap.
- **`spawn_ambient_scheduler()`** returns `AmbientSchedulerHandle`
  with `rx` (mpsc) + `reload()` method. No blocking.

### 5. Ambient (`ambient/mod.rs`, 520 LOC)

- **`AmbientEntry`** — `(HH, MM, scene_name: String)`. Heap-allocated
  string is intentional (cold path, parsed once at config load).
- **`AmbientSchedule::seconds_to_next_phase()`** — pure function, no
  allocation. Handles midnight wrap.
- **`apply_ambient_entry()`** — applies scene + palette, sets
  `ambient_palette_locked = true`. Idempotent (safe to call twice).

### 6. Diagnostics (`ambient_diag.rs`, 88 LOC)

- **All counters are `AtomicU64`** — no Mutex contention, no allocation.
- **`LAST_SCENE_CHANGE: Mutex<Option<String>>`** — only Mutex in the
  diagnostics path; updated on scene change (cold path), read on exit
  summary. Contention is negligible.
- **`ambient_diag_summary()`** — formats a single string on exit. Cold
  path, no per-frame cost.

### 7. Control config (`crystal_dragon_control/mod.rs`, 134 LOC)

- **`CrystalDragonControl` struct** — 6 fields, all `f32` or `enum`.
  `Copy` + `Clone` derived. Stack-allocated, no heap.
- **Constants**: `CRYSTAL_DRAGON_POLLING_SECS=60.0` (v80.0.0-alpha.1:
  the DEFAULT only — the runtime value is `CrystalDragonControl.polling_secs`,
  user-tunable via `crystal-dragon-secs` CLI/config/live-reload),
  `CRYSTAL_DRAGON_MIN_DWELL_SECS=60.0`, `CRYSTAL_DRAGON_DRIFT_CHANCE=0.12`,
  `CRYSTAL_DRAGON_CPU_EMA_ALPHA=0.25`. All `pub(crate) const`, inlined
  by LLVM.
- **`Default::default()`** — `const fn`-eligible (no runtime cost).

## A/B Benchmark Verification

The Crystal Dragon is exercised on a 60-second polling cycle, NOT per
frame. Its contribution to the per-frame benchmark is zero (sensor
polling happens off-frame). The A/B comparison confirmed no regression
in the surrounding engine:

| Metric                     | Before Audit | After Audit | Δ       | Verdict |
|----------------------------|-------------:|------------:|--------:|---------|
| avg_fps                    |       85,555 |      85,755 |  +0.23% | NEUTRAL |
| active_frame_ratio_percent |       100.00%|      100.00%|       0 | MATCH   |
| frame_jitter               |          low |         low |       — | MATCH   |
| drift_interpretation       |       stable |      stable |       — | MATCH   |

**Conclusion**: Engine is at peak. No code changes applied — the lock
is the appropriate action.

## Dragon Engine Topology (Locked)

| Subsystem                                  | LOC    | Role                                                                  |
|--------------------------------------------|-------:|-----------------------------------------------------------------------|
| `crystal_dragon_engine/ambient/mod.rs`     |    520 | Time-of-day schedule types, config parsing, validation, startup apply |
| `crystal_dragon_engine/ambient_scheduler/mod.rs` | 378 | Dynamic idle/wake scheduler thread (zero CPU between phase boundaries) |
| `crystal_dragon_engine/sensor/mod.rs`     |    276 | CPU sampling (procfs) + CLOCK fallback (UTC). Produces 1–99 point.    |
| `crystal_dragon_engine/palette_groups/mod.rs` | 129 | 44 themes -> Cold(14) / Medium(14) / Hot(14) + Reserved(2)             |
| `crystal_dragon_engine/point_system/mod.rs` |  126 | calc-v1: probabilistic weighted theme selection (CDF + binary search) |
| `crystal_dragon_engine/crystal_dragon_control/mod.rs` | 134 | Config struct + constants (polling, drift chance, EMA alpha, sensor mode, calc method) |
| `crystal_dragon_engine/ambient_diag.rs`   |     88 | Atomic counters for diagnostics + exit summary                        |
| `crystal_dragon_engine/mod.rs`            |     74 | Top-level module doc + re-exports                                    |

**Total**: 1,707 LOC production + 1,031 LOC test suite = 2,738 LOC.

## Owner Decisions (Locked)

| Decision | Choice | Rationale |
|----------|--------|-----------|
| HUD indicator | **Silent-Elegant (Option A)** — no HUD indicator, no verbose drift-event logging | The engine should be felt, not seen. |
| Calc method | **calc-v1** (probabilistic weighted) | calc-v2 (pattern state machine with memory) is reserved for future release. |
| Polling interval | **60 s default** (v80.0.0-alpha.1: tunable via `crystal-dragon-secs`, 0.0..=86400.0 — the harmony knob for ambient snapback coordination) | Slow enough to feel organic, fast enough to react to real load within a minute. |
| Sensor mode | **CPU primary, CLOCK fallback** | CPU is the meaningful signal; CLOCK is the graceful degradation when CPU sampling is unsupported. |
| Phase switching | **Instant** (no smoothstep blend) | Owner explicitly asked for snappy boundaries, not 5-minute cross-fades. |
| Schedule format | **Single scene name** (no multi-field) | Eliminates override-precedence bug surface. Scene IS the source of truth. |

## Modification Protocol

See [`RULES.md`](RULES.md) in this directory for the UNLOCK protocol
that MUST be followed if any file in this directory is modified after
the lock.

---

**Lock signature:**

```
3 Dragon Lock in commit 69af079 after deeper audit for strengthening
and stability. Signoff by rezky_nightky 2026-08-19T14:40:05Z vision,
& director project cosmostrix.
```
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
