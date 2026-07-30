<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Self-Healing Audit — Current State & Gap Analysis

**Date**: 2026-07-30
**Scope**: Audit existing self-healing mechanisms in cosmostrix against the
three-tier vision proposed in research (Visual Self-Cleaning, State Recovery,
Adaptive Degradation). Identify what exists, what's missing, and what's
realistic to build.

**TL;DR**: The codebase already has **substantial** self-healing
infrastructure — watchdog, phosphor decay, signal handlers, force-draw
invalidation, perf-gated degradation, adaptive pacing. The gaps are narrow
but targeted: (1) no periodic **stuck-cell sweep** that catches dirty-tracking
edge cases, (2) no `/dev/tty` fallback when stdout breaks mid-run,
(3) no **automatic scene downgrade** when sustained CPU pressure is detected,
(4) `EnduranceHealth.score()` is computed but never consumed by the runtime.

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

### Gap: No Periodic Stuck-Cell Sweep

**What's missing**: A background watchdog that scans the frame buffer for
cells in an inconsistent state — e.g., a glyph that should have been
overwritten by a droplet pass but wasn't due to a dirty-tracking edge case.

**Why it's narrow**: The phosphor system already handles cells with
`phosphor[i] > 0`. The risk is a cell where:
- `cell_gen[i] == current_gen` (looks fresh)
- `cell.fg.is_some()` (has a glyph)
- But the glyph is stale because a droplet's tail_put_line was incorrectly
  computed, leaving a "ghost trail" the phosphor system never sees.

**Realistic mitigation**: A periodic (every N=3600 frames, ~1 min at 60fps)
debug-mode-only sweep that:
1. Iterates `frame.cells`
2. For each non-blank cell, checks if any droplet's `bound_col` matches
   its column AND its `tail_put_line..=head_put_line` range covers the cell
3. If no droplet covers it AND no phosphor energy exists at that index,
   log it as a "stuck cell" candidate
4. In `--debug-stuck-cells` mode, force-clear it

**Cost**: O(W×H) every 60s ≈ 12000 ops for 200×60 — negligible.

**Recommendation**: Low priority. The 5-minute full redraw already catches
this. Only worth building if telemetry shows stuck cells surviving the
5-min cycle.

---

## 2. State Recovery

### What already exists

#### 2a. Signal Handlers — `interactive/signal_handlers.rs`

| Signal | Action | Quality |
|--------|--------|---------|
| SIGTERM, SIGHUP, SIGQUIT | Set `GRACEFUL_SHUTDOWN` + `signal_exit` flags, wait up to 20s for `SHUTDOWN`, then exit | Excellent — atomic-flag coordination, no stdout races |
| SIGINT (Ctrl+C) | **Intentionally NOT handled** — user must press 'q' to quit (cinematic design principle, v25.13) | Deliberate |
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

### Gap: No `/dev/tty` Fallback for Mid-Run stdout Corruption

**What's missing**: If stdout's file descriptor becomes invalid mid-run
(e.g., terminal emulator crashes, SSH disconnects, parent process dies),
`Terminal::flush_ansi` propagates `write_all` errors via `?` to
`terminal.draw()`, which propagates to the event loop, which exits via
`?`. The process exits cleanly but doesn't attempt to *recover* by
reopening `/dev/tty`.

**Why it's narrow**: The Drop-based cleanup + force-exit watchdog
already handle the "stdout is broken" case at shutdown. The gap is
*mid-run* recovery — keeping cosmostrix alive after a transient stdout
failure.

**Realistic mitigation**:
1. In `Terminal::flush_ansi`, on `write_all` error:
   - Check if error is `EBADF` or `EPIPE` (recoverable)
   - If so, attempt to `fs::OpenOptions::new().write(true).open("/dev/tty")`
   - Replace `self.stdout` with the new handle
   - Retry the write
2. If `/dev/tty` open fails too, set `GRACEFUL_SHUTDOWN` and exit

**Cost**: ~30 LOC, no per-frame overhead (only fires on error path).

**Recommendation**: Medium priority. Useful for daemon/screensaver mode
where cosmostrix runs unattended. For interactive mode, exiting cleanly
on stdout death is arguably the right behavior — the user can restart.

### Gap: No Periodic fd Health Check

**What's missing**: No periodic `isatty(stdout)` check or write-probe
to detect fd corruption proactively. Only detected when a write fails.

**Recommendation**: Low priority. The reactive path (write fails → handle
error) is sufficient. A proactive check adds per-frame syscall overhead
for a rare failure mode.

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
| P5 EnduranceHealth | Composite 0-100 score (RSS var 40%, jitter 35%, ctxt switches 25%) | Implemented, **NOT integrated** |

#### 3d. EnduranceHealth — Computed but Not Consumed

`EnduranceHealth::recompute()` runs every 60 frames (~1s at 60fps).
The score is pushed to HUD via `update_metrics()` for display, but
**the event loop never reads `score()` to trigger mitigations**.

This is the most actionable gap — the metric exists, the gating
infrastructure exists, but they're not wired together.

### Gap: No Automatic Scene Downgrade

**What's missing**: When `perf_pressure` is sustained high (e.g.,
> 0.8 for 30 seconds), cosmostrix doesn't automatically switch to a
lighter scene (e.g., storm → low-power). The user has to press a key.

**Realistic mitigation**:
1. Track `sustained_high_pressure_secs` in the event loop
2. When it crosses a threshold (e.g., 30s at perf_pressure > 0.8):
   - Save the current scene name to `pre_degradation_scene`
   - Call `cloud.apply_scene("low-power")` (or the configured fallback)
   - Set a flag `auto_degraded = true`
3. When perf_pressure drops below 0.3 for 60s AND `auto_degraded`:
   - Restore `pre_degradation_scene`
   - Clear the flag

**Cost**: ~40 LOC in event_loop.rs, no per-frame overhead (just a counter
and conditional).

**Open question**: Should auto-downgrade preserve the user's color
palette, or fully reset to the scene's defaults? Recommend preserve
palette — the user's color choice is intentional, the scene downgrade
is a performance mitigation.

**Recommendation**: Medium-high priority. This is the "Dragon" feature
the user described — cosmostrix proactively healing its own performance
health. Directly matches research vision item C.

### Gap: EnduranceHealth Not Wired to Mitigations

**What's missing**: `EnduranceHealth.score()` returns "healthy" /
"degraded" / "investigate" but nothing acts on it.

**Realistic mitigation**:
1. When `classification() == "investigate"` (score < 60):
   - Force a `cloud.force_draw_everything()` (clears potential stuck state)
   - Trigger `hint_reclaim_pages()` immediately (bypass ReclaimState's 1h min)
   - Log to stderr (write_fmt, broken-pipe-safe)
2. When `classification() == "degraded"` for 5+ minutes:
   - Consider triggering the auto-scene-downgrade above

**Cost**: ~20 LOC, runs every 60 frames.

**Recommendation**: Medium priority. Cheaper than the scene-downgrade
gap, but less user-visible. Pair them together for compound effect.

---

## 4. Summary — Priority Matrix

| # | Gap | Effort | Impact | Priority |
|---|-----|--------|--------|----------|
| 1 | Auto scene downgrade on sustained high perf_pressure | ~40 LOC | High — visible "self-healing" behavior | **P1** |
| 2 | Wire EnduranceHealth.score() to mitigations | ~20 LOC | Medium — closes the loop on existing metric | **P2** |
| 3 | `/dev/tty` fallback for mid-run stdout corruption | ~30 LOC | Medium — daemon/screensaver value | **P3** |
| 4 | Periodic stuck-cell sweep (debug mode only) | ~50 LOC | Low — 5-min full redraw already covers this | **P4** |
| 5 | Periodic fd health check (isatty probe) | ~10 LOC | Low — reactive path is sufficient | **P5** (skip) |

**Recommended next step**: Implement #1 + #2 together. They form a
coherent "performance self-healing" subsystem:
- `EnduranceHealth` detects degradation
- Auto-scene-downgrade responds to it
- Both gate on existing `perf_pressure` infrastructure

Total: ~60 LOC, zero per-frame overhead in steady state, directly
realizes the "Adaptive Degradation (Already Exists, Can Be Strengthened)"
item from the research vision.

---

## 5. Existing Documentation Cross-References

| Doc | Covers |
|-----|--------|
| `docs/STABILITY_AUDIT.md` | Signal handlers, watchdog, force-draw safety, ghost glyph flood bug, regression test inventory |
| `docs/TERMINAL_KILL_CLEANUP.md` | Fork-based SIGKILL guard, panic hook, cleanup ordering |
| `docs/ENDURANCE.md` | 72-hour telemetry analysis, P1/P2/P4/P5 adaptive subsystems |
| `docs/COSMIC_DRAGON_FINDINGS.md` | Architectural findings, cosmic_dragon/ incubator policy |
| `docs/RENDER_ENGINE.md` | Frame generation tracking, semantic invalidation, dirty-cell system |

This audit complements those docs by focusing on the **self-healing** lens
specifically, mapping the research vision's three tiers to concrete code
locations and identifying the narrow gaps that remain.
