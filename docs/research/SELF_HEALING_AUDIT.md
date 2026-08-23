<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Self-Healing Audit — Current State & Gap Analysis

> **Historical research snapshot.** File paths, symbol names, and counts
> reflect the codebase at audit time; modules have since moved (flat
> `src/*.rs` files became module directories). Preserved as a record -
> cross-check the live source tree before relying on any path.

**Date**: 2026-07-30
**Scope**: Audit existing self-healing mechanisms in cosmostrix against the
three-tier vision proposed in research (Visual Self-Cleaning, State Recovery,
Adaptive Degradation). Identify what exists, what's missing, and what's
realistic to build.

**TL;DR**: The codebase already has **substantial** self-healing
infrastructure — watchdog, phosphor decay, signal handlers, force-draw
invalidation, perf-gated degradation, adaptive pacing. The gaps were narrow
but targeted: (1) no periodic **stuck-cell sweep** that catches dirty-tracking
edge cases, (2) no `/dev/tty` fallback when stdout breaks mid-run,
(3) no **automatic scene downgrade** when sustained CPU pressure is detected,
(4) `EnduranceHealth.score()` is computed but never consumed by the runtime,
(5) no proactive `isatty` probe to detect fd corruption during idle periods.

**All five gaps are now closed** (P1-P5, commits `35a6acd` through `2ed4a27`).

**Implementation Status (2026-07-30)**:

| # | Gap | Status | Commit |
|---|-----|--------|--------|
| P1 | Auto scene downgrade on sustained high perf_pressure | OK Done | `35a6acd`, `0edffa2` |
| P2 | Wire EnduranceHealth.score() to mitigations | OK Done | `35a6acd`, `0edffa2` |
| P3 | `/dev/tty` fallback for mid-run stdout corruption | OK Done | `22a2aa3` |
| P4 | Periodic stuck-cell sweep (debug mode only) | OK Done | `4827ddb`, `e73da86` |
| P5 | Periodic fd health check (isatty probe) | OK Done | `feeac76`, `2ed4a27` |

---

## 1. Visual Self-Cleaning

### What already exists

#### 1a. Phosphor Decay System — `cloud/phosphor.rs` (706 LOC)

The core "afterglow nurse." Three-pass algorithm running every frame:

| Pass | Purpose | Implementation |
|------|---------|----------------|
| 1 | Mark cells drawn this frame as fresh | Dirty-index scan (skips O(W×H) when dirty list is populated) |
| 2 | Protect active droplet trail cells from decay | Iterates `self.droplets`; skipped for Monolith (has own cleanup) |
| 3 | Decay non-fresh cells with phosphor energy | Iterates `phosphor_active` (BitVec-tracked); fades color via `apply_brightness_rgb`; mutates glyphs via `TRAIL_CYCLE_PROBABILITY` (2% per step) |

Key cells:

- `phosphor_active: Vec<usize>` — swap-remove pattern, O(1) cleanup
- `phosphor_in_active: BitVec` — O(1) membership check, prevents dupes
- `phosphor_last_fresh: SmallVec` — incremental clear (avoids per-frame alloc)
- `PHOSPHOR_DEAD_THRESHOLD` — cells below this energy get blanked
- `captured_phosphor_energy()` — bottom-edge taper (edge fade rows)

**This is already the "watchdog visual" the user envisioned.** It runs every
frame, not "every few minutes," because the design is *preventive* not
*reactive* — cells decay continuously rather than being detected as stuck.

#### 1b. Periodic Full Redraw — `cloud/rain.rs:638-647`

```rust
self.frames_since_full_redraw += 1;
if self.frames_since_full_redraw >= FULL_REDRAW_INTERVAL_FRAMES {
    self.frames_since_full_redraw = 0;
    self.force_draw_everything = true;
}
```

`FULL_REDRAW_INTERVAL_FRAMES = 18000` (~5 min at 60fps). This is the
"scheduled cleanup" the research brief mentioned — it catches accumulated
ANSI state desync from resize, scroll, or differential-rendering edge cases.

#### 1c. Semantic Invalidation — `cloud/runtime_controls.rs:198-200` + `frame.rs:149-152`

`set_shading_mode()` sets `semantic_invalidate = true`. The rain loop sees
this, calls `frame.invalidate_semantic()` which bumps `semantic_gen` and
runs `clear_with_bg()`. The Terminal's differential renderer detects the
gen mismatch and forces a complete redraw without `Clear(All)`.

This is the mechanism that prevents "ghost background glyph flood" when
shading mode changes (historical bug, documented in STABILITY_AUDIT.md:72).

#### 1d. force_draw_everything Triggers

Comprehensive list of events that force a complete frame clear:

| Trigger | File | Line |
|---------|------|------|
| Paste event | event_loop.rs | 326 |
| Focus regain | event_loop.rs | (via register_activity) |
| Idle resync | event_loop.rs | 530 |
| User input after idle | event_loop.rs | 615 |
| Periodic full redraw (5 min) | cloud/rain.rs | 644 |
| Resize (debounced) | event_loop.rs | 900 |
| Scene/theme change | cloud/scene_runtime.rs | 82, 112, 141 |
| SIGCONT reinit | event_loop.rs | 552 |

When `force_draw_everything` fires, `cloud/rain.rs:232-249` also clears
`phosphor_base_ch` to prevent stale ghost glyphs from reappearing during
the full redraw.

#### 1e. Monolith Spine Phosphor Cleanup — `cloud/monolith.rs:271`

`clear_spine_phosphor()` runs every frame in Monolith mode to clear stale
spine cells (DrawnCellKind::Spine). This is scene-specific self-cleaning.

### Gap (Closed P4): Periodic Stuck-Cell Sweep

**What was missing**: A background watchdog that scans the frame buffer for
cells in an inconsistent state — e.g., a glyph that should have been
overwritten by a droplet pass but wasn't due to a dirty-tracking edge case.

**Why it's narrow**: The phosphor system already handles cells with
`phosphor[i] > 0`. The risk is a cell where:

- `cell_gen[i] == current_gen` (looks fresh)
- `cell.fg.is_some()` (has a glyph)
- But the glyph is stale because a droplet's tail_put_line was incorrectly
  computed, leaving a "ghost trail" the phosphor system never sees.

**Implementation (commit `4827ddb`)**: A periodic (every N=3600 frames,
~1 min at 60fps) debug-mode-only sweep that:

1. Gates on `enable_component_timing` (i.e., `--perf-stats`) — zero cost
   in production interactive runs.
2. Skips when a message box is active (overlay cells would be false positives).
3. Pre-computes each active droplet's visible trail range for O(droplets)
   coverage check per cell.
4. For each non-blank cell at current_gen with `phosphor[i] == 0`, checks
   if any droplet covers (col, line). If not, force-clears it.
5. Caps at `STUCK_CELL_MAX_PER_SWEEP = 256` cells per pass.
6. On stuck cell detection, sets `force_draw_everything` so the cleared
   cells are emitted next frame.

**Cost**: O(W×H + droplets) every 60s ≈ 12,100 ops for 200×60 + ~100
droplets. Negligible.

**Tests** (commit `e73da86`): 7 unit tests covering gating logic, orphan
glyph clearing, droplet-covered cell preservation, max-per-sweep cap, and
no-op behavior on clean frames.

---

## 2. State Recovery

### What already exists

#### 2a. Signal Handlers — `interactive/signal_handlers.rs`

| Signal | Action | Quality |
|--------|--------|---------|
| SIGTERM, SIGHUP, SIGQUIT | Set `GRACEFUL_SHUTDOWN` + `signal_exit` flags, wait up to 20s for `SHUTDOWN`, then exit | Excellent — atomic-flag coordination, no stdout races |
| SIGINT (Ctrl+C) | **Intentionally NOT handled** — user must press 'q' to quit (cinematic design principle) | Deliberate |
| SIGTSTP (Ctrl+Z) | Disable mouse capture → `restore_terminal_best_effort()` → set `term_reinit` → raise SIGSTOP | Excellent — restores terminal *before* suspending so user gets a usable shell |
| SIGCONT | Set `term_reinit` flag (main loop recreates Terminal on next iteration) | Clean |
| Windows Ctrl+Break | Set graceful flags, sleep 1s, force restore + exit(130) if main loop didn't shut down | Adequate |

#### 2b. SIGCONT Terminal Reinit — `event_loop.rs:541-556`

```rust
if term_reinit.swap(false, Ordering::AcqRel) {
    drop(term);
    term = Terminal::with_signal_exit(signal_exit.clone())?;
    if term.enable_mouse_capture().is_ok() {
        MOUSE_CAPTURE_ACTIVE.store(true, Ordering::Release);
    }
    let (nw, nh) = term.size()?;
    pending_resize = Some((nw, nh));
    cloud.force_draw_everything();
    let reinit_time = Instant::now();
    last_resync_time = reinit_time;
    next_frame = reinit_time;
}
```

Full lifecycle: drop old Terminal (runs cleanup) → create new → re-enable
mouse → query size → schedule resize → force draw → reset timing.

#### 2c. Resize Debounce — `event_loop.rs:539, 823-833, 899-911`

Rapid resize events (e.g., window drag) coalesce into a single
`pending_resize`. The debounce window is checked after the event drain
loop. Applied via `cloud.reset(nw, nh)` + `Frame::new(nw, nh, ...)` +
density recompute + `force_draw_everything()`.

Fixed mode (`--screen-size`) ignores resize entirely — virtual size is
preserved.

#### 2d. Watchdog Thread — `interactive/watchdog.rs`

```rust
std::thread::spawn(move || loop {
    if shutdown.load(Ordering::Acquire) { return; }
    std::thread::sleep(Duration::from_secs(WATCHDOG_INTERVAL_SECS));
    // ... check FRAME_COUNTER, increment stuck_count
    // After 2 stuck checks (~20s): restore_terminal_best_effort() + exit(1)
});
```

Two-tier response:

- First stuck check: warn to stderr (write_fmt, broken-pipe-safe)
- Second stuck check: force restore + exit(1)

Checks `SHUTDOWN` flag before AND after each sleep to terminate cleanly.

#### 2e. Drop-Based Cleanup + Force-Exit Watchdog — `terminal.rs`

`Terminal::Drop` runs cleanup in reverse order (LIFO, idempotent).
**Before** cleanup, spawns a force-exit watchdog thread (2s timeout,
`SHUTDOWN_TIMEOUT_SECS`). If cleanup finishes normally, sets
`shutdown_complete = true` and the watchdog exits harmlessly. If cleanup
is stuck (e.g., stdout pipe broken and `flush()` blocks), the watchdog
calls `process::exit(0)`.

#### 2f. Panic Hook — `main.rs:163-166`

Calls `restore_terminal_best_effort()` *before* printing panic info, so
even unexpected panics don't leave the terminal broken.

#### 2g. Fork-Based SIGKILL Guard — `main.rs:103-156`

Parent forks; child runs cosmostrix. Parent waits. If child died via
SIGKILL (which bypasses all handlers), parent restores the terminal
before exiting. This is the only defense against `kill -9`.

### Gap (Closed P3): `/dev/tty` Fallback for Mid-Run stdout Corruption

**What was missing**: If stdout's file descriptor becomes invalid mid-run
(e.g., terminal emulator crashes, SSH disconnects, parent process dies),
`Terminal::flush_ansi` propagates `write_all` errors via `?` to
`terminal.draw()`, which propagates to the event loop, which exits via
`?`. The process exits cleanly but doesn't attempt to *recover* by
reopening `/dev/tty`.

**Why it's narrow**: The Drop-based cleanup + force-exit watchdog
already handle the "stdout is broken" case at shutdown. The gap was
*mid-run* recovery — keeping cosmostrix alive after a transient stdout
failure.

**Implementation (commit `22a2aa3`)**:

1. `Terminal` gained `tty_fallback: Option<File>` (lazily opened, cached)
   and `tty_recoveries: u32` (capped at `STDOUT_FALLBACK_MAX_RECOVERIES = 3`).
2. `flush_ansi` routes all writes through `write_with_recovery()`.
3. On `write_all` error, `recover_to_tty()` checks
   `is_recoverable_io_error()` (BrokenPipe, EBADF, ENXIO, EIO,
   PermissionDenied) before attempting `/dev/tty`.
4. On successful recovery: writes the buffer to `/dev/tty`, sets
   `GRACEFUL_SHUTDOWN` so the process exits cleanly via the normal
   shutdown path (Terminal::drop still runs).
5. 8 unit tests cover the error classification matrix.
6. Windows stub returns the original error (CONOUT$ reopen needs Win32).

**Cost**: ~30 LOC, zero per-frame overhead in steady state — the recovery
path only fires when stdout `write_all` returns an error.

**Tests**: 8 unit tests in `terminal::p3_tests` covering BrokenPipe,
PermissionDenied, EBADF, ENXIO, EIO classifications, plus negative
cases (Interrupted, WriteZero are NOT recoverable).

### Gap (Closed P5): Periodic fd Health Check (isatty Probe)

**What was missing**: No proactive `isatty(stdout)` check to detect fd
 corruption before a write fails. The reactive P3 path catches write
 failures during active rendering, but during idle periods (no redraws)
stdout could break (SSH disconnect, terminal crash, parent death) and
we wouldn't notice until the next render attempt.

**Why it was originally skipped**: The audit's concern was "per-frame
syscall overhead for a rare failure mode." The realized design solves
this by running the probe on a slow interval (every 3600 frames ≈ 60s
at 60fps), not per-frame. The isatty syscall is ≈1μs, amortized to
0.0017 syscalls/sec — completely negligible.

**Implementation (commit `feeac76`)**:

1. New constant `FD_HEALTH_PROBE_INTERVAL_FRAMES = 3600` (matches P4
   stuck-cell sweep cadence — both are background hygiene passes on
   the same slow tick).
2. New `Terminal::probe_stdout_health()` method. On Unix: calls
   `isatty(stdout_fd)` via `std::io::IsTerminal`. If false, reuses
   the P3 recovery path (`recover_to_tty` with an empty buffer +
   synthetic `BrokenPipe` error) which opens `/dev/tty`, sets
   `GRACEFUL_SHUTDOWN`, logs to stderr. On non-Unix: always returns
   true (Windows console handles don't fail the same way; reactive
   P3 path remains in effect).
3. Wired into `event_loop.rs` after the `perf_rss_samples` increment.
   When the probe returns false, breaks the loop (`GRACEFUL_SHUTDOWN`
   is already set; `Terminal::drop` runs the normal cleanup path).

**Cost**: one `isatty` syscall per minute. Zero per-frame overhead.
The probe reuses the entire P3 recovery machinery — no new `/dev/tty`
opening logic, no new `GRACEFUL_SHUTDOWN` wiring, no new stderr logging.
P5 is purely a detection layer on top of P3's recovery layer.

**Tests** (commit `2ed4a27`): 7 unit tests covering interval bounds,
P4 cadence sync, P3 contract verification (synthetic BrokenPipe is
recoverable), IsTerminal behavior on /dev/null, recovery cap inheritance,
and modulo-check fire-pattern simulation over 3 intervals.

---

## 3. Adaptive Degradation

### What already exists

#### 3a. perf_pressure — `event_loop.rs:987-992`

```rust
let overshoot = ((work_s / frame_period_s) - 1.0).clamp(0.0, 2.0);
if overshoot > 0.0 {
    perf_pressure = (perf_pressure + (overshoot * PERF_PRESSURE_INCREMENT)).min(1.0);
} else {
    perf_pressure = (perf_pressure - PERF_PRESSURE_DECAY).max(0.0);
}
```

Tracks frame overshoot. Range 0.0 (healthy) → 1.0 (saturated).
Accumulates on overshoot, decays when under budget.

#### 3b. Perf-Gated Subsystems

Five subsystems gate on `perf_pressure`:

| Subsystem | Threshold | Effect | File |
|-----------|-----------|--------|------|
| Phosphor decay | > 0.7 | Skip entirely | `cloud/phosphor.rs:51` |
| Glitch effects | > `GLITCH_THRESHOLD` | Skip | `cloud/rain.rs:256` |
| CRT vignette | > `CRT_VIGNETTE_PERF_THRESHOLD` | Skip | `cloud/rain.rs:729` |
| Atmospheric events | > `EVENT_PERF_GATE` | Skip | `cloud/atmospheric_events.rs:148` |
| Simulation step | scaled by `sim_factor` | Slower sim, same FPS | `event_loop.rs:932` |

`sim_factor = (1.0 - perf_pressure * SIM_PRESSURE_SCALE_FACTOR).clamp(SIM_FACTOR_MIN, 1.0)`
— clamps the max simulation delta so the rain slows down instead of
dropping frames.

#### 3c. Adaptive Pacing Subsystems — `interactive/adaptive.rs`

| Subsystem | Purpose | Status |
|-----------|---------|--------|
| P1 PhasePredictor | Learns daily active/idle cycle, predicts idle before reactive threshold | Implemented, integrated |
| P2 adaptive_resync_interval | Stretches idle redraw interval (20s → 60s after 1h, → 120s after 4h) | Implemented, integrated |
| P4 ReclaimState | madvise(MADV_DONTNEED) on Linux during idle (1h min interval) | Implemented, integrated |
| P5 EnduranceHealth | Composite 0-100 score (RSS var 40%, jitter 35%, ctxt switches 25%) | Implemented, **integrated (commit `0edffa2`)** |

#### 3d. EnduranceHealth — Wired to Mitigations (commit `0edffa2`)

`EnduranceHealth::recompute()` runs every 60 frames (~1s at 60fps).
The score is pushed to HUD via `update_metrics()` for display AND now
consumed by `PerformanceSelfHealer` to trigger mitigations when the
score drops into the "investigate" band (score < 60):

- Forces a `cloud.force_draw_everything()` (clears potential stuck state)
- Bypasses ReclaimState's 1h min to issue an immediate madvise hint
- Logs to stderr (write_fmt, broken-pipe-safe)
- 30s cooldown prevents mitigation floods

### Gap (Closed P1): Automatic Scene Downgrade

**What was missing**: When `perf_pressure` is sustained high (e.g.,
> 0.6 for 30 seconds), cosmostrix didn't automatically switch to a
lighter scene (e.g., storm → low-power). The user had to press a key.

**Implementation (commit `35a6acd`)**:

1. `PerformanceSelfHealer` struct tracks `sustained_high_pressure_secs`
   and `sustained_low_pressure_secs` with hysteresis (0.6 high, 0.3 low).
2. When high pressure sustains for 30s: saves the current scene name to
   `pre_degradation_scene`, calls `cloud.apply_scene("low-power")`,
   sets `auto_degraded = true`.
3. When low pressure sustains for 60s AND `auto_degraded`: restores
   the prior scene, clears the flag.
4. **Preserves the user's color palette** — scene downgrade is a
   performance mitigation, not a visual reset.

**Cost**: ~40 LOC in `adaptive.rs` (PerformanceSelfHealer), wired into
`event_loop.rs` at the EnduranceHealth recompute site (~10 LOC).

**Recommendation (realized)**: This is the "Dragon" feature the user
described — cosmostrix proactively healing its own performance health.
Directly matches research vision item C.

### Gap (Closed P2): EnduranceHealth Wired to Mitigations

**What was missing**: `EnduranceHealth.score()` returned "healthy" /
"degraded" / "investigate" but nothing acted on it.

**Implementation (commit `35a6acd`)**:

1. When `classification() == "investigate"` (score < 60):
   - Force a `cloud.force_draw_everything()` (clears potential stuck state)
   - Trigger `hint_reclaim_pages()` immediately (bypass ReclaimState's 1h min)
   - Log to stderr (write_fmt, broken-pipe-safe)
2. 30s cooldown (`SELF_HEAL_HEALTH_COOLDOWN_SECS`) prevents mitigation
   floods from a persistently unhealthy process.

**Cost**: ~20 LOC in `adaptive.rs`, runs every 60 frames. 15 unit tests
in `adaptive::tests` cover the state machine transitions.

---

## 4. Summary — Priority Matrix

| # | Gap | Effort | Impact | Priority | Status |
|---|-----|--------|--------|----------|--------|
| 1 | Auto scene downgrade on sustained high perf_pressure | ~40 LOC | High — visible "self-healing" behavior | **P1** | OK Done (`35a6acd`, `0edffa2`) |
| 2 | Wire EnduranceHealth.score() to mitigations | ~20 LOC | Medium — closes the loop on existing metric | **P2** | OK Done (`35a6acd`, `0edffa2`) |
| 3 | `/dev/tty` fallback for mid-run stdout corruption | ~30 LOC | Medium — daemon/screensaver value | **P3** | OK Done (`22a2aa3`) |
| 4 | Periodic stuck-cell sweep (debug mode only) | ~50 LOC | Low — 5-min full redraw already covers this | **P4** | OK Done (`4827ddb`, `e73da86`) |
| 5 | Periodic fd health check (isatty probe) | ~30 LOC | Low — closes idle-period detection window | **P5** | OK Done (`feeac76`, `2ed4a27`) |

**Result**: All five actionable gaps are now closed. The self-healing
subsystem comprises:

- **Visual self-cleaning**: phosphor decay + 5-min full redraw + P4 stuck-cell sweep
- **State recovery**: signal handlers + SIGCONT reinit + watchdog + P3 /dev/tty fallback + P5 proactive fd health probe
- **Adaptive degradation**: perf_pressure gating + P1 auto-scene-downgrade + P2 EnduranceHealth mitigations

Total implementation cost: ~190 LOC across 8 commits, zero per-frame
overhead in steady state, 37 new unit tests (8 P3 + 7 P4 + 15 P1+P2 + 7 P5).

---

## 5. Existing Documentation Cross-References

| Doc | Covers |
|-----|--------|
| `docs/STABILITY_AUDIT.md` | Signal handlers, watchdog, force-draw safety, ghost glyph flood bug, regression test inventory |
| `docs/TERMINAL_KILL_CLEANUP.md` | Fork-based SIGKILL guard, panic hook, cleanup ordering |
| `docs/ENDURANCE.md` | 72-hour telemetry analysis, P1/P2/P4/P5 adaptive subsystems |
| `docs/COSMIC_DRAGON_FINDINGS.md` | Architectural findings, cosmic_dragon_incubator/ incubator policy |
| `docs/RENDER_ENGINE.md` | Frame generation tracking, semantic invalidation, dirty-cell system |

This audit complements those docs by focusing on the **self-healing** lens
specifically, mapping the research vision's three tiers to concrete code
locations and identifying the narrow gaps that remain.
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
