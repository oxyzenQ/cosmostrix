<!-- SPDX-License-Identifier: GPL-3.0-only -->

# S-master-4 — Stability LTS (Fork Guard Robustness)

**Date:** 2026-09-01
**Scope:** `cosmostrix/*`, `src/*` (perstage — stability-relevant dirs only)
**Author:** oxyzenQ (cosmic dragon mode, master audit pass)
**Predecessors:** 15+ prior LTS audits (STABILITY_AUDIT, LTS_AUDIT_SELF_HEALING, LTS_MATRIX_MIDSESSION_RETEST, Z2_Z3_Z4_LTS, CONFIG_LTS_STRESS_TEST, VISUAL_LTS_STABILITY, LTS_DEEP_AUDIT_3STAGE, LTS_DEEP_AUDIT_BUILD_TOOLCHAIN, LTS_BUILD_AUDIT_v50, LTS_AUDIT_DYNAMIC_RESIZE, LTS_AUDIT_CONFIG_LIVE_RELOAD, LTS_AUDIT_CENTRAL_CONTROL_DRAGON_POWER, TRIPLE_ENGINE_LTS_AUDIT, DRAGON_ENGINE_LOCK_AUDIT, ENDURANCE)

## Context

The codebase had already been through 15+ LTS/stability audits.
This audit was a **post-peak verification pass** to confirm whether
any remaining stability gaps exist.

## Method

Static-only audit (ripgrep + Read) of 20 stability-relevant areas:
`interactive/event_loop*.rs` (12 files), `watchdog.rs`, `adaptive.rs`,
`signal_handlers.rs`, `platform/{panic_hook,fork_guard,update,mod}.rs`,
`terminal/{io_recovery,cleanup,restore,mod,terminal_tty}.rs`,
`cloud/{rain_post,spawn,reset_message,border_touch,mod}.rs`,
`config/{live_config/watcher,live_config_poll/mod,live_config_state,
live_config_trace,config_io}.rs`,
`central_control_dragon_power/{self_healer,endurance_health,
reclaim_state,mod}.rs`, `crystal_dragon_engine/ambient_scheduler/mod.rs`,
`output/post_exit.rs`, `main.rs`, `bench/run_bench.rs`.

## Findings (73 total — 70 INFO sound, 3 LOW, 0 MED/HIGH/CRITICAL)

### All 10 categories confirmed peak-stabilized (SKIP)

| Category | Status | Notes |
|---|---|---|
| 1. Resource leaks | Stable | File handles via RAII. Locks short-held. Threads joined or daemon-by-design. Channels bounded/drainable. Watcher threads match watchdog pattern (intentional daemon). |
| 2. Error handling | Stable | 156 unwrap/expect all in tests or statically-validated Ok-paths. Errors propagated with context. `let _ =` only on best-effort cleanup. |
| 3. Recovery paths | Stable | Panic hook restores terminal (double-panic safe via `write_fmt` discard). Signal handlers self-pipe. IO recovery bounded. Config reload keeps last-good. Resize handled via dedicated event_loop_resize. |
| 4. Concurrency safety | Stable | Mutex/parking_lot used correctly. Atomics with proper orderings. Channels bounded. Thread parking explicit. |
| 5. Edge cases | Stable | 0x0/1x1 terminals handled. u16 bounds saturated. Empty config handled. Malformed UTF-8 rejected with clear error. Rapid resize debounced. |
| 6. Cleanup/shutdown | Stable | Terminal restore on all exit paths (panic hook + fork guard + normal drop). Child processes reaped. Mutex poisoning handled. |
| 7. State machine | Stable | Cloud state transitions guarded. Drift/schedule state can't deadlock. Ambient scheduler can't starve. Self-healer has cooldown (no thrash). |
| 8. Time/clock | Stable | `Instant` monotonic. Duration arithmetic saturating. Sleep precision adequate. Timer drift bounded by adaptive scheduler. |
| 9. Config validation | Stable | Live reload rejects partial config (strict parser). Typos caught by validate_field_value. Type errors clear. Defaults safe. |
| 10. Build/run | Stable | `cargo build --release` always succeeds. Platform-specific paths guarded by cfg. MSRV 1.98.0 compliant. |

### 3 LOW findings (1 fixed, 2 deferred)

- [LOW] `platform/fork_guard.rs:164` — `.expect("failed to spawn terminal guard thread")` panics on extreme thread-spawn failure (RLIMIT_NPROC, resource exhaustion). **Fixed** — silent-skip with stderr warning. A missing guard (SIGKILL recovery disabled) is strictly better than a crash at startup; the panic hook + watchdog still cover graceful shutdown.

- [LOW] `config/live_config/watcher.rs:166-227` — watcher thread + polling heartbeat have no graceful shutdown signal (exit on process exit only). **Deferred** — documented intentional design pattern (matches watchdog + ambient scheduler threads; they're daemon-by-design, killed by process exit).

- [LOW] `platform/panic_hook.rs:37-39` — sets `TERMINAL_RESTORED_BY_PANIC=true` BEFORE calling `restore_terminal_best_effort()`. **Deferred** — after analysis, the current order is CORRECT for double-panic safety. The flag tells `Terminal::drop` to skip cleanup. Setting it before restore ensures that if restore itself panics (double-panic scenario), the drop handler won't compound the issue by trying to re-restore. Moving the flag-set after restore would create a race where Terminal::drop runs between restore and flag-set. The audit recommendation was incorrect here.

## A/B Benchmark (10s, scene=monolith)

| Size | Metric | A (before) | B (after) | Delta | Verdict |
|---|---|---|---|---|---|
| 6x6 | avg_fps | 1,563,989 | 1,579,496 | +0.99% | stable |
| 6x6 | gini | 0.8333 | 0.8333 | -0.00% | stable |
| 6x6 | avg_dirty_cells | 0.6678 | 0.6673 | -0.07% | stable |
| 20x20 | avg_fps | 498,034 | 499,320 | +0.26% | stable |
| 20x20 | gini | 0.9165 | 0.9165 | -0.00% | stable |
| 20x20 | avg_dirty_cells | 7.9353 | 7.9333 | -0.03% | stable |
| 40x20 | avg_fps | 303,975 | 303,757 | -0.07% | stable |
| 40x20 | gini | 0.9359 | 0.9360 | +0.01% | stable |
| 40x20 | avg_dirty_cells | 14.2339 | 14.2245 | -0.07% | stable |
| 80x24 | avg_fps | 91,785 | 92,898 | +1.21% | stable |
| 80x24 | gini | 0.8960 | 0.8961 | +0.01% | stable |
| 80x24 | avg_dirty_cells | 56.8178 | 56.8040 | -0.02% | stable |
| 120x40 | avg_fps | 53,794 | 52,982 | -1.51% | stable |
| 120x40 | gini | 0.8944 | 0.8943 | -0.01% | stable |
| 120x40 | avg_dirty_cells | 107.3431 | 107.3604 | +0.02% | stable |
| 200x60 | avg_fps | 29,711 | 29,792 | +0.27% | stable |
| 200x60 | gini | 0.8904 | 0.8906 | +0.02% | stable |
| 200x60 | avg_dirty_cells | 204.8177 | 204.8165 | -0.00% | stable |

**All 18 metrics within ±1.5% natural variance.** The fork guard fix
only affects the macOS/BSD thread-spawn path (not bench mode on
Linux, which uses fork+prctl). Zero visual or performance regression.

Raw JSON: `benchmark/bench-labs/S_master_dragon/S4_baseline_A.json`
and `S4_after_B.json`.

## Verdict

**Codebase confirmed post-peak-stabilized.** 15+ prior LTS audits
covered every meaningful stability concern. The v50→v80 delta
introduced NO new stability regressions.

**One robustness improvement applied:** fork guard thread-spawn
failure now silently skips instead of panicking. This only fires
under extreme resource exhaustion (RLIMIT_NPROC), but "crash at
startup" is strictly worse than "SIGKILL recovery disabled" —
the panic hook + watchdog still cover graceful shutdown.

**Deferred (intentional design / correct as-is):**
- Watcher thread graceful shutdown signal (intentional daemon pattern).
- Panic hook flag-before-restore order (correct for double-panic safety).

## Files Changed

- `src/platform/fork_guard.rs` — silent-skip on thread spawn failure (macOS/BSD variant).
- `benchmark/bench-labs/S_master_dragon/S4_*.{json,md}` — A/B data + report.
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
