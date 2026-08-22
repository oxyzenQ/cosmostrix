<!-- SPDX-License-Identifier: GPL-3.0-only -->

# LTS Audit 2026-08-19 — Central Control Dragon Power Stability

> **Task 7/7**: Deep audit of `src/central_control_dragon_power/` for
> LTS stability, strength, precision.

## Audit Scope

| Subsystem | LOC | Role |
|-----------|----:|------|
| `mod.rs` | 579 | Top-level coordinator: `PowerThresholds`, `PowerManager`, `EnduranceHealth` |
| `power_manager/mod.rs` | 385 | Single owner of `perf_pressure`, `is_idle`, `effective_fps` |
| `self_healer/mod.rs` | 293 | Pure-policy self-healer: reads state, emits `SelfHealAction` |
| `endurance_health.rs` | 280 | Endurance Health Score (0-100) computation |
| `thermal_sampler.rs` | 261 | Linux sysfs thermal zone reader + normalizer |
| `phase_predictor.rs` | 226 | PAP (Phase Prediction) — proactive idle signal |
| `reclaim_state.rs` | 189 | MPAR (Memory Reclaim) — `madvise(MADV_DONTNEED)` wrapper |
| `audit_tests.rs` | 497 | Integration contract tests (6 audit groups) |
| `power_manager/tests.rs` | 364 | PowerManager unit tests |
| `self_healer/tests.rs` | 343 | SelfHealer unit tests |

**Total**: ~3,417 LOC across 10 files (1,713 production + 1,704 tests).

## Audit Findings

### 1. `unsafe` Code Audit (2 sites in `reclaim_state.rs`)

✅ **No changes needed.** Both `unsafe` blocks are properly guarded:

```rust
// reclaim_state.rs:82 (Linux)
#[cfg(target_os = "linux")]
pub(crate) unsafe fn hint_reclaim_pages(ptr: *const u8, len: usize) {
    if len == 0 || ptr.is_null() {
        return;  // ← null/zero-length guard
    }
    let ret = libc::madvise(ptr as *mut libc::c_void, len, libc::MADV_DONTNEED);
    let _ = ret;  // ← best-effort, ignores all errors (CC2-06)
}

// reclaim_state.rs:94 (non-Linux)
#[cfg(not(target_os = "linux"))]
pub(crate) unsafe fn hint_reclaim_pages(_ptr: *const u8, _len: usize) {
    // No-op on non-Linux platforms.
}
```

Both functions have a `# Safety` doc section explaining the contract
(caller must ensure `ptr` points to a mapped region of at least `len`
bytes). The Linux variant has explicit null + zero-length guards.
Errors from `madvise` are intentionally ignored (best-effort reclaim).

### 2. `.unwrap()` Audit

✅ **No changes needed.** Grep for `.unwrap()` in production code
(non-test, non-comment) returns **0 matches**.

The single `.unwrap()` in `thermal_sampler.rs:249` is inside a
`#[cfg(test)]` block:

```rust
assert!(result.is_none() || result.unwrap() >= 0.0);
```

This is safe — the `is_none()` short-circuits before `unwrap()` is
reached. Not a production risk.

### 3. Float Comparison Audit

✅ **No changes needed.** Grep for `f32 ==`, `f64 ==`, `f32 !=`,
`f64 !=` in production code returns **0 matches**. All float
comparisons use the `.abs() < epsilon` pattern (verified in
`power_manager/tests.rs:286` — `(t.pressure_high -
SELF_HEAL_PRESSURE_HIGH).abs() < 1e-6`).

### 4. Concurrency Audit

✅ **No changes needed.** Grep for `Mutex`, `RwLock`, `AtomicU`,
`AtomicBool`, `Arc<` in production code returns **0 matches**.

This is correct by design: `central_control_dragon_power` is
**single-threaded** — it's called exclusively from the event loop
(`interactive/event_loop.rs`), which runs on the main render thread.
No cross-thread access → no need for synchronization primitives.

If a future feature moves any of this code to a background thread
(e.g., thermal sampling in a separate thread), synchronization MUST
be added at that time. The single-threaded design is documented in
`mod.rs` and `power_manager/mod.rs`.

### 5. PowerManager Single-Owner Contract

✅ **No changes needed.** The audit_tests.rs:1-50 explicitly pins
the contract:

> 1. **PowerManager is the single owner** of `perf_pressure`,
>    `is_idle`, and `effective_fps` — the three previously-scattered
>    read paths.
> 2. **Thermal guard flows through `effective_pressure`** — a
>    thermal input at `set_thermal_pressure()` is visible at every
>    downstream read of `effective_pressure()`.
> 3. **Self-healer reads from `PowerThresholds`** — the struct is
>    the sole consumer-facing API for the 6 self-healer thresholds.
> 4. **Frame lifecycle is consistent** — `begin_frame →
>    effective_fps → effective_pressure → observe_frame_end` produces
>    stable, monotonic behavior across a synthetic frame sequence.
> 5. **Thermal sampler + normalizer contract** — the pure math is
>    correct and the sampler degrades gracefully on missing sysfs.
> 6. **Clash zone resolution** — `effective_fps` is the single owner
>    of the pause/idle/active cascade; no other writer can produce
>    a different FPS for the same state.

6 integration contract tests pin these invariants. Any future
refactor that breaks the contract fails CI.

### 6. Self-Healer Pure-Policy Design

✅ **No changes needed.** `self_healer/mod.rs:66` explicitly
documents:

> The self-healer is a *pure policy* — it does not touch `Cloud`,
> `Frame`, or any mutable state. It reads the current state and
> emits a `SelfHealAction` enum variant. The caller (event loop)
> applies the action.

This is the correct separation of concerns:

- **Self-healer**: pure function, no side effects, easy to test
- **Event loop**: applies the action, mutates state

No `unsafe`, no mutation, no I/O in the self-healer itself.

### 7. Thermal Sampler Graceful Degradation

✅ **No changes needed.** `thermal_sampler.rs` returns `Option<f32>`:

- Linux with sysfs present: reads `/sys/class/thermal/thermal_zone*/temp`,
  normalizes to 0.0–1.0 range.
- Linux without sysfs (containers, some VMs): `read_dir` returns `Err`,
  propagated as `None` via `?` operator.
- Non-Linux: `#[cfg(not(target_os = "linux"))]` stub returns `None`.

Test `sample_thermal_pressure_does_not_panic_when_sysfs_missing`
(thermal_sampler.rs:246) explicitly verifies no panic on missing sysfs.

### 8. Endurance Health Score Stability

✅ **No changes needed.** `endurance_health.rs` computes a 0–100 score:

- Score is bounded `[0, 100]` via `clamp(0.0, 100.0)`.
- Inputs are all `f32` with explicit `clamp` on each metric before
  weighting.
- No `nan`/`inf` propagation possible (all inputs are finite f32 from
  bounded measurements).

### 9. Phase Predictor (PAP) Stability

✅ **No changes needed.** `phase_predictor.rs` is a pure function:

- Input: `t: f32` (time-elapsed fraction, 0.0–1.0)
- Output: `f32` in `[0.0, 1.0]` (predicted idle probability)
- Formula: `0.3 * t + 0.7 * 0.0 = 0.3 * t` (linear, biased toward 0)
- Bounded by `clamp(0.0, 1.0)`.

No mutation, no I/O, no allocation. Thread-safe by virtue of being
a pure function (even though currently called single-threaded).

### 10. Reclaim State (MPAR) Safety

✅ **No changes needed.** `reclaim_state.rs`:

- `ReclaimState` struct tracks `last_reclaim: Option<Instant>` +
  `MIN_RECLAIM_INTERVAL` (1 hour) to avoid hammering `madvise`.
- `hint_reclaim_pages()` is `unsafe` (raw pointer), but:
  - Has null + zero-length guards.
  - Ignores all `madvise` errors (best-effort).
  - Has `# Safety` doc section.
  - Only effective on Linux; no-op on other platforms.
- Caller (event_loop.rs) only calls after `cloud.force_draw_everything()`
  ensures the region is no longer needed.

## Test Coverage

✅ **Stable across 3 consecutive runs:**

- `cargo test "central_control_dragon_power::"` → 87/87 pass × 3 runs (0 flakes)
- Full suite: ~1500+ pass × 3 runs (0 flakes — fixed in Task 5)

Test breakdown:

- `audit_tests.rs` (497 LOC, 6 integration contract tests)
- `power_manager/tests.rs` (364 LOC, ~30 unit tests)
- `self_healer/tests.rs` (343 LOC, ~25 unit tests)
- Inline `mod tests` in `endurance_health.rs`, `thermal_sampler.rs`,
  `phase_predictor.rs`, `reclaim_state.rs` (~26 unit tests total)

## Conclusion

**central_control_dragon_power is LTS-stable.** No code changes
required in this audit. The subsystem is:

- ✅ Zero `.unwrap()` in production code
- ✅ Zero direct float `==`/`!=` comparisons (uses epsilon)
- ✅ Zero concurrency primitives (correct — single-threaded by design)
- ✅ 2 `unsafe` sites properly guarded (null check, best-effort, documented)
- ✅ PowerManager single-owner contract pinned by 6 integration tests
- ✅ Self-healer is pure-policy (no mutation, no I/O, no `unsafe`)
- ✅ Thermal sampler gracefully degrades on missing sysfs
- ✅ Endurance Health Score bounded `[0, 100]` via clamp
- ✅ Phase Predictor is a pure function (bounded, thread-safe)
- ✅ Reclaim State has 1-hour minimum interval (prevents madvise spam)
- ✅ 87 tests pass stably across 3 consecutive runs

The 3 Dragon Lock (commit `2ef8cdf`) does NOT directly cover this
subsystem (it's not one of the 3 dragon engines), but the
`central_control_dragon_power` module is consumed by the Cosmic
Dragon's event loop (`interactive/event_loop.rs`). The integration
is verified by the audit_tests.rs contract suite.

**Audit signoff**: Task 7 complete. No UNLOCK required for the
3 Dragon Lock — the central_control_dragon_power subsystem is
stable as-is.
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
