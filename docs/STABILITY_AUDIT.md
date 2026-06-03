# Cosmostrix Terminal Stability Audit Report

**Date**: Section C audit  
**Scope**: Terminal safety, input safety, redraw safety, pause/resume, resize, signal handling  
**Files analyzed**: `terminal.rs`, `interactive/event_loop.rs`, `interactive/input.rs`, `main.rs`, plus supporting modules (`watchdog.rs`, `cloud/mod.rs`, `cloud/rain.rs`, `frame.rs`, `constants.rs`, `activity.rs`)

---

## 1. Terminal Cleanup on Exit

### Existing Safety Mechanisms

The cleanup system is implemented as a **four-layer defense-in-depth** strategy, ensuring the terminal is restored to a usable state under virtually any exit scenario.

**Layer 1: RAII Drop Guard (`Terminal::drop`, terminal.rs:503–525)**  
The `Terminal` struct implements `Drop`, guaranteeing cleanup runs even during panic unwinding. The `Drop` implementation spawns a force-exit watchdog thread *before* performing cleanup. This thread sleeps for `SHUTDOWN_TIMEOUT_SECS` (2 seconds, `constants.rs:131`) and then checks an `Arc<AtomicBool>` (`shutdown_complete`). If cleanup finished normally, the flag is set to `true` (`terminal.rs:523`) and the watchdog exits harmlessly. If cleanup is stuck (e.g., stdout pipe is broken and `flush()` blocks), the watchdog calls `process::exit(0)` as a last resort. This prevents the process from hanging indefinitely during shutdown.

**Layer 2: Idempotent Cleanup (`cleanup_terminal`, terminal.rs:195–225)**  
The cleanup function is strictly idempotent — the `cleaned_up` boolean guard at line 197 prevents any cleanup step from executing twice. Each state flag (`mouse_capture_enabled`, `bracketed_paste_enabled`, `cursor_hidden`, `line_wrap_disabled`, `alternate_screen_enabled`, `raw_mode_enabled`) is checked individually and cleared after the corresponding ANSI command is issued. The cleanup order is **reverse-LIFO** relative to setup:

1. Disable mouse capture (and focus change events) — via `disable_mouse_capture()` which also calls `clear_mouse_capture_flag()` to keep the global atomic in sync
2. Disable bracketed paste mode (`?2004l`)
3. Reset attributes and colors (`SetAttribute(Reset)`, `ResetColor`)
4. Show cursor (`cursor::Show`)
5. Re-enable line wrap (`EnableLineWrap`)
6. Leave alternate screen (`LeaveAlternateScreen`)
7. Disable raw mode (`terminal::disable_raw_mode()`)
8. Flush stdout

This LIFO ordering is verified by the unit test `terminal_cleanup_plan_is_reverse_order_and_idempotent` (terminal.rs:635–659) which uses a `CleanupFlags` struct that mirrors the real cleanup logic and asserts the exact sequence.

**Layer 3: Best-Effort Restore (`restore_terminal_best_effort`, terminal.rs:528–541)**  
A standalone function that can be called from any context (signal handlers, panic hooks, other threads) without access to the `Terminal` instance. It issues a comprehensive `TERMINAL_RESET_SEQUENCE` (terminal.rs:543–544) containing all known terminal reporting mode resets: `?1000l` (basic mouse tracking), `?1002l` (button-event tracking), `?1003l` (any-event tracking), `?1006l` (SGR mouse encoding), `?1015l` (URXVT mouse encoding), `?2004l` (bracketed paste), `?1004l` (focus reporting), `?1049l` (alternate screen buffer), `?25h` (show cursor), followed by `0m` (attribute reset). The coverage is verified by the test `emergency_reset_sequence_disables_terminal_reporting_modes` (terminal.rs:622–631).

**Layer 4: Fork-Based SIGKILL Guard (`spawn_kill9_terminal_guard`, main.rs:103–156)**  
On Linux, before the main loop starts, the process forks a child that saves the current `termios` state and then waits for `SIGTERM`. If the parent is killed with `SIGKILL` (which cannot be caught), the kernel reparents the child to `init` (PID 1). The child detects this via `getppid() == 1`, restores the terminal via `tcsetattr()`, and calls `restore_terminal_best_effort()`. This provides safety even against unkillable signal scenarios. The guard is opt-out via the `COSMOSTRIX_NO_FORK_GUARD` environment variable and only activates when stdin/stdout are confirmed to be TTY devices. It uses `PR_SET_PDEATHSIG` as a belt-and-suspenders measure, blocks `SIGTERM` in the child via `pthread_sigmask`, and uses `_exit()` (not `exit()`) to avoid atexit handlers in the child.

**Additional: Panic Hook (main.rs:163–166)**  
`std::panic::set_hook` is installed at the very start of `main()` to call `restore_terminal_best_effort()` before printing the panic info, ensuring a broken terminal never results from a panic.

### Assessment

The cleanup system is **exemplary**. The four-layer defense (RAII drop → idempotent cleanup → best-effort restore → fork SIGKILL guard) covers all realistic failure modes including normal exit, panic, SIGKILL, and stuck flush. The LIFO ordering is correct and tested. One minor observation: `cleanup_terminal()` delegates to `disable_mouse_capture()` for mouse/focus cleanup, which uses `execute()` (synchronous) rather than `queue()` (buffered). In theory this means a broken stdout pipe could cause an early return from cleanup, leaving later steps unexecuted. However, the `let _ =` error suppression ensures the function continues regardless, and subsequent steps are all guarded by their own `let _ =` wrappers. This is acceptable best-effort behavior.

---

## 2. Input Safety

### Bracketed Paste Detection and Suppression

The input safety system addresses the critical problem of pasted text being interpreted as rapid keypresses, which could trigger unwanted shortcuts (e.g., pasting `"css"` would toggle color, charset, and speed).

**Bracketed Paste Mode** is enabled during terminal setup (`terminal.rs:133`), causing the terminal emulator to wrap pasted text in `\x1b[200~` and `\x1b[200~` escape sequences. The crossterm library translates these into `Event::Paste(String)` events.

**PasteBurstGuard (input.rs:34–61)** provides two independent suppression mechanisms:

1. **Bracketed paste window**: When a `Event::Paste(_)` event is received, `note_bracketed_paste()` (input.rs:58–60) sets `suppress_until` to `now + PASTE_BURST_SUPPRESS_MS` (50ms, `input.rs:32`). Any plain printable keys arriving within this window are silently consumed by `ignore_plain_key()`.

2. **Queued-event heuristic**: Even without bracketed paste support, `ignore_plain_key()` (input.rs:40–56) takes a `queued_event_ready` parameter. The event loop checks `Terminal::poll_event(Duration::from_millis(0))?` (event_loop.rs:242) immediately after receiving a keypress. If more events are already queued in the input buffer, this strongly suggests a paste (a human cannot type two characters within a single `poll` cycle), and the key is suppressed. Suppression also self-extends: once active, the 50ms window refreshes on each suppressed key.

**Plain printable key detection** (`is_plain_printable_key`, input.rs:63–70) is deliberately conservative — it only matches `KeyCode::Char(_)` with `KeyModifiers::NONE` or `KeyModifiers::SHIFT`. Special keys (arrows, function keys, Escape), control keys (Ctrl+C, Ctrl+Z), and Alt-modified keys are never suppressed, ensuring legitimate shortcuts always work.

In the event loop (event_loop.rs:243–254), suppressed keys still trigger `register_activity()` and `force_draw_everything()` to update the idle timer and ensure the display stays responsive during paste operations. They simply skip `handle_keybinding()` to prevent shortcut activation.

### Control Character Handling

Control character handling is straightforward:
- **Ctrl+C**: Explicitly mapped to `cloud.raining = false` (input.rs:113–115), providing an alternative exit path to Escape/Q
- **Ctrl+Z**: Explicitly mapped to SIGSTOP with full terminal restore (input.rs:93–108), identical behavior to SIGTSTP signal handler
- **Escape**: Maps to `cloud.raining = false` (input.rs:91)
- **Tab/BackTab**: Explicitly ignored (input.rs:178–185) with detailed comment explaining the historical bug that motivated this: Tab previously toggled shading mode, which caused a ghost background glyph flood via `set_shading_mode()` → `semantic_invalidate` → `invalidate_semantic()` → frame clear without clearing `phosphor_base_ch`

### Assessment

The paste suppression system is **well-designed** with defense-in-depth against both bracketed-paste-aware and bracket-paste-unaware terminals. The queued-event heuristic is particularly clever — it detects pastes even on terminals that don't support bracketed paste mode. The 50ms suppression window is long enough to cover any paste burst but short enough to not interfere with fast typing. Tests verify suppression activation, expiration, and that shortcut actions are not triggered during suppression (`interactive/tests.rs:103–154`).

One theoretical concern: If a user types `p` and then within 50ms types another shortcut key (e.g., `c` to change color), the `c` would be suppressed. However, this requires sub-50ms inter-key timing which is physically unrealistic for human typing (typical inter-key interval is 100–200ms). The test `paste_burst_suppression_expires` (tests.rs:121–127) confirms the window expires after 52ms.

---

## 3. Redraw Safety

### Periodic Full Redraw (ANSI Drift Correction)

Long-running terminal applications can accumulate ANSI state desynchronization — the terminal emulator's internal state (cursor position, active attributes, scroll region) may diverge from what the application believes. To correct this, the renderer performs a **periodic full redraw** every `FULL_REDRAW_INTERVAL_FRAMES` (18,000 frames, `constants.rs:619`). At 60 FPS, this triggers approximately every 5 minutes. The counter is maintained in `cloud.frames_since_full_redraw` (cloud/mod.rs:155) and checked in the `rain()` function (cloud/rain.rs:350–354):

```rust
if self.frames_since_full_redraw >= FULL_REDRAW_INTERVAL_FRAMES {
    self.frames_since_full_redraw = 0;
    self.force_draw_everything = true;
}
```

The full redraw path in `Terminal::draw()` (terminal.rs:263–369) resets the cursor to `(0,0)`, iterates every cell, issues style changes and characters, then resets attributes and colors. This effectively re-synchronizes all terminal state. The test `periodic_full_redraw_survives_until_next_frame` (cloud/tests/mod.rs:74–88) verifies the mechanism.

### Semantic Invalidation on Mode Changes

When the renderer's *semantic identity* changes (charset switch, shading mode toggle, color scheme change), simply dirtying cells is insufficient — the terminal emulator may still have cached the old visual representation for cells that happen to have the same content in the new mode. The system handles this via `Frame::invalidate_semantic()` (frame.rs:92–95), which increments `semantic_gen` and performs a full logical clear. The `Terminal::draw()` method (terminal.rs:241–249) checks both dimension changes and semantic generation mismatches:

```rust
let dim_changed = l.width != frame.width || l.height != frame.height;
let sem_changed = l.semantic_gen != frame.semantic_gen;
(dim_changed || sem_changed, dim_changed)
```

Crucially, dimension changes trigger a `Clear(All)` to handle stale content at terminal edges, while semantic-only changes skip the clear to avoid visible flicker in fullscreen terminals. This optimization is well-documented in comments (terminal.rs:233–240).

Semantic invalidation is triggered by:
- `set_shading_mode()` (cloud/mod.rs:614–620) — sets `semantic_invalidate = true`
- `set_color_scheme()` — handled via the transition system (does not use semantic_invalidate)
- Charset transitions — handled via `transition_chars()` (wave-based, no semantic_invalidate)

The `semantic_invalidate` flag is processed in `rain()` (cloud/rain.rs:98–104) *before* `force_draw_everything`, ensuring the generation bump happens first. The phosphor ghost character array is also cleared at this point to prevent stale glyphs from appearing during the full redraw.

### `force_draw_everything` and Phosphor State Clearing

The `force_draw_everything` mechanism (triggered by paste events, focus regain, idle resync, user input after idle, periodic full redraw) sets `frame.clear_with_bg()` which bumps the frame generation, making all cells appear dirty. However, this alone is insufficient because the **phosphor persistence system** maintains a separate `phosphor_base_ch` array that stores the original character glyph for ghost afterglow cells. Without clearing this array, a full redraw would expose all ghost glyphs as visible background characters — the "ghost background" bug documented in comments (cloud/rain.rs:92–95 and cloud/rain.rs:109–117).

The fix is applied in both `semantic_invalidate` and `force_draw_everything` paths:
```rust
for ch in self.phosphor_base_ch.iter_mut() {
    *ch = '\0';
}
```

Active trail cells repopulate their `phosphor_base_ch` entries through the normal Pass 1 and Pass 2 mechanisms of `phosphor_decay_pass`, so the clear only affects stale afterglow cells.

### Dirty Threshold for Full Redraw

When differential rendering is active, if the number of dirty cells exceeds `total_cells / DIRTY_THRESHOLD_RATIO` (ratio of 3, `constants.rs:128`), the renderer switches to a full redraw automatically (terminal.rs:259–261). This prevents pathological cases where nearly every cell is dirty but differential rendering incurs more overhead than a full redraw due to per-cell cursor movement.

### Assessment

The redraw safety system is **comprehensive and well-layered**. The combination of periodic full redraws, semantic invalidation, phosphor state clearing, and dirty threshold fallback covers all known ANSI drift scenarios. The distinction between dimension-change (with `Clear(All)`) and semantic-change (without `Clear(All)`) is a subtle but important optimization that prevents flicker during mode transitions.

One observation: the semantic invalidation for color scheme changes is handled differently — via the palette transition system rather than `semantic_invalidate`. This means a rapid color scheme change (e.g., cycling through all 16 schemes quickly) does not trigger a full redraw per change but instead relies on the transition wave mechanism. If a user presses `c` repeatedly, each press replaces the pending transition. This is intentional and avoids visual thrashing, but it means the renderer relies on the periodic full redraw (every 5 minutes) to clean up any accumulated state from incomplete transitions.

---

## 4. Pause/Resume Safety

### Timing Debt Reset on Resume

The `toggle_pause()` function (cloud/mod.rs:530–583) implements thorough timing debt cancellation on resume:

1. **Spawn debt reset**: `self.last_spawn_time = now` and `self.spawn_remainder = 0.0` (lines 540–541). Without this, the accumulated spawn remainder during a long pause would cause a burst of hundreds of new droplets on the first resumed frame, creating visual chaos.

2. **Per-droplet timing reset**: Each alive droplet has its time incremented via `d.increment_time(elapsed)` and `d.last_time` set to `now` with `d.advance_remainder = 0.0` (lines 542–547). This ensures droplets don't jump multiple rows on resume.

3. **Phase 3 subsystem timer shifting** (lines 549–571): All timed subsystems have their "last tick" timestamps shifted forward by the pause duration:
   - Phosphor decay timer (`last_phosphor_time`)
   - Glitch timers (`last_glitch_time`, `next_glitch_time`)
   - RNG reseed timer (`last_reseed_time`)
   - Color ecosystem tick (`color_ecosystem.last_tick`)
   - Atmospheric evolution tick (`atmosphere.last_tick`)
   - Memory sampling tick (`memory.last_sample`)
   - Storytelling tick (`storytelling.last_tick`) plus cooldown timer
   - Palette transition timer (`transition_start`)
   - Profile transition timer (`profile_transition_start`)
   - Charset transition timer (`charset_transition_start`)

   This prevents every subsystem from burst-firing on the first tick after unpause (each would otherwise see a large `elapsed` value and trigger immediate action).

4. **Spawn remainder cap**: Even outside of pause/resume, the spawn remainder is clamped to `SPAWN_REMAINDER_CAP` (4.0, `constants.rs:221`) to prevent accumulation from timing spikes. The test `spawn_remainder_is_clamped` (cloud/tests/mod.rs:353–365) verifies this.

### Smoothstep Resume Easing

The resume transition uses a **smoothstep S-curve** to ramp the simulation time scale from 0% to 100% over `RESUME_EASE_DURATION_SECS` (0.18 seconds, `constants.rs:609`). This is computed in `rain()` (cloud/rain.rs:57–72):

```rust
let normalized = (t / RESUME_EASE_DURATION_SECS).min(1.0);
self.resume_blend = normalized * normalized * (3.0 - 2.0 * normalized);
```

The smoothstep formula `3t² - 2t³` produces slow start, fast middle, and slow end — matching the physical intuition of inertia recovery. The `resume_blend` factor is applied to:

- **Spawn rate** (cloud/rain.rs:84): `spawn_scale *= self.resume_blend`, so new streams appear gradually
- **Droplet advance** (cloud/rain.rs:153): `d.advance(adv_now, self.resume_blend)`, so existing droplets accelerate smoothly
- **Phosphor decay** (cloud/rain.rs:251–254): `phosphor_elapsed * self.resume_blend`, so afterglow fades at the same rate as the rain wakes up, preventing temporal inconsistency

On pause entry, `resume_blend` is set to 0.0 and `resume_start` is set to `Some(now)` (cloud/mod.rs:577–578). Once the easing completes, `resume_start` is set to `None` (cloud/rain.rs:70) and `resume_blend` remains at 1.0 permanently until the next pause.

### Tests

The pause/resume behavior is covered by multiple tests:
- `pause_stops_rain_and_unpause_resumes` (cloud/tests/mod.rs:48–71): Verifies pause stops dirty output and resume restarts it
- `pause_freezes_simulation_time` (cloud/tests/mod.rs:220–234): Verifies timing doesn't advance during pause
- `resume_resets_timing_debt` (cloud/tests/mod.rs:237–251): Verifies spawn_remainder is zeroed, resume_blend starts at 0, last_spawn_time is current
- `repeated_pause_resume_does_not_accumulate_timing_debt` (cloud/tests/mod.rs:254–269): Verifies three consecutive pause/resume cycles don't accumulate debt

### Assessment

The pause/resume system is **thoroughly engineered**. The timing debt reset covers all subsystems (spawn, per-droplet advance, and all nine Phase 3 subsystems). The smoothstep easing applies to all three physics channels (spawn, advance, phosphor decay) simultaneously, preventing temporal inconsistency during the transition. The 180ms easing duration is short enough to feel responsive but long enough to eliminate the harsh catch-up snap that would otherwise occur.

One observation: The `increment_time(elapsed)` call on alive droplets during resume (cloud/mod.rs:544) increments the droplet's internal time accumulator by the pause duration. This means droplets that were mid-trail when paused will have their internal timers advanced, potentially causing them to die sooner after resume (their tail catches up to where their head "would have been"). This is intentional — it prevents droplets from appearing frozen at their pause-time position. The smoothstep easing then handles the visual acceleration smoothly from there.

---

## 5. Resize Handling

### Terminal Resize Detection

Resize events are received via crossterm's `Event::Resize(nw, nh)` in the event loop (event_loop.rs:230–238). Raw crossterm values are immediately clamped to safe bounds:

```rust
let cw = nw.clamp(MIN_TERMINAL_COLS, MAX_TERMINAL_COLS);
let ch = nh.clamp(MIN_TERMINAL_LINES, MAX_TERMINAL_LINES);
```

Where `MIN_TERMINAL_COLS = 4`, `MIN_TERMINAL_LINES = 4`, `MAX_TERMINAL_COLS = 1024`, `MAX_TERMINAL_LINES = 500` (constants.rs:136–147). This prevents degenerate dimensions (0×0 or 65535×65535) that could cause panics in `Uniform::new()` or massive memory allocations during `cloud.reset()`. The clamping rationale is documented: `1024 cols × 500 lines × ~48 bytes/cell ≈ 24 MiB`.

### Resize Debouncing

Rapid resize events (e.g., window drag in a tiling WM) are coalesced using a debounce window of `RESIZE_DEBOUNCE_MS` (150ms, `constants.rs:152`). The event loop stores the latest resize dimensions in `pending_resize` and records `last_resize_event` timestamp. The inner poll loop only breaks for resize processing when the debounce window has elapsed (event_loop.rs:340–347):

```rust
if pending_resize.is_some() {
    let debounce_elapsed = last_resize_event
        .map(|t| t.elapsed() >= Duration::from_millis(RESIZE_DEBOUNCE_MS))
        .unwrap_or(true);
    if debounce_elapsed { break; }
}
```

This prevents redundant `cloud.reset()` calls and visual thrashing during continuous resize.

### Frame Reallocation

When a resize is applied (event_loop.rs:391–405), the cloud and frame are fully reallocated:

```rust
cloud.reset(nw, nh);
frame = Frame::new(nw, nh, cloud.palette.bg);
```

If `density_auto` is enabled, the droplet density is recalculated for the new terminal dimensions (event_loop.rs:394–401). A `force_draw_everything()` is issued to ensure the new frame is fully rendered.

### SIGCONT/Terminal Reinit Path

After a SIGTSTP/SIGCONT cycle (event_loop.rs:212–224), the Terminal is dropped and recreated. The new terminal's size is queried, and a resize is applied. This handles the case where the terminal dimensions changed while the process was suspended (e.g., switching between terminal emulator tabs). The `term_reinit` flag is checked at the top of each main loop iteration.

### Size Clamping in Terminal

The `Terminal::size()` method (terminal.rs:149–158) also clamps terminal dimensions to the safe range, providing a second line of defense beyond the event loop clamping.

### Assessment

Resize handling is **robust**. The combination of immediate clamping, debouncing, full reallocation, and auto-density recalculation covers all resize scenarios. The SIGCONT path correctly reinitializes the terminal to handle dimension changes during suspension. The size bounds (4–1024 cols, 4–500 lines) are reasonable and well-documented.

---

## 6. Signal Handling

### SIGINT/SIGTERM/SIGHUP — Graceful Shutdown

Signal handling (event_loop.rs:49–72) uses the `signal-hook` crate to register a dedicated signal thread. The signal handler does **not** directly write ANSI sequences to stdout (which would race with the main thread). Instead, it:

1. Sets `GRACEFUL_SHUTDOWN` atomic flag (`event_loop.rs:55`)
2. Sleeps 1 second to give the main loop time to notice
3. Checks `SHUTDOWN` flag; if the main loop hasn't exited, falls back to `restore_terminal_best_effort()` + `process::exit(128 + sig)`

The main loop checks `GRACEFUL_SHUTDOWN` at the top of each iteration (event_loop.rs:189–192) and breaks cleanly, allowing `Terminal::drop()` to perform orderly cleanup. This avoids the race condition between the signal handler thread and the main thread writing to stdout simultaneously.

### SIGTSTP (Ctrl-Z) — Suspend

The SIGTSTP handler (event_loop.rs:75–99) performs a full terminal restore before suspending:

1. Checks and disables mouse capture if active (`event_loop.rs:82–87`)
2. Calls `restore_terminal_best_effort()` to restore raw mode, alternate screen, cursor, etc.
3. Sets `term_reinit` flag so the main loop reinitializes the terminal on SIGCONT
4. Raises `SIGSTOP` via `low_level::raise(SIGSTOP)` to actually suspend the process

### SIGCONT — Resume After Suspend

The SIGCONT handler (event_loop.rs:92–93) sets `term_reinit` to `true`. The main loop checks this flag (event_loop.rs:212), drops the old `Terminal` (triggering cleanup), creates a new one, re-enables mouse capture if configured, queries the new size, and applies a resize with `force_draw_everything()`. All timing is reset (`last_resync_time`, `next_frame`) to prevent scheduling glitches.

### Ctrl+Z Keybinding (input.rs:93–108)

The Ctrl+Z key handler performs the same sequence as the SIGTSTP signal handler — disables mouse capture, restores terminal, sets `term_reinit`, and raises SIGSTOP. This provides consistent behavior whether the user presses Ctrl+Z or the terminal emulator sends SIGTSTP.

### Windows Ctrl-C Handler (event_loop.rs:103–118)

On Windows, a `ctrlc::set_handler` is installed with the same graceful-shutdown-then-force-exit pattern as the Unix signal handler.

### Watchdog Thread (watchdog.rs:44–84)

A background watchdog thread provides protection against main loop hangs:

1. Sleeps for `WATCHDOG_INTERVAL_SECS` (5 seconds, `constants.rs:121`), records `FRAME_COUNTER` value
2. Sleeps another 5 seconds, reads `FRAME_COUNTER` again
3. If unchanged, increments `stuck_count`; after 2 consecutive stuck checks (10 seconds total), calls `restore_terminal_best_effort()` and `process::exit(1)`
4. If the counter changed, resets `stuck_count`

The watchdog checks `SHUTDOWN` before and after each sleep cycle (watchdog.rs:50–60) so it terminates cleanly when the main loop exits normally. The `SHUTDOWN` flag is set at the end of the main loop (event_loop.rs:491) before the `Terminal` is dropped.

### Panic Hook (main.rs:163–166)

A panic hook is installed at the very start of `main()` that calls `restore_terminal_best_effort()` before printing the panic info. This ensures that even unexpected panics don't leave the terminal in a broken state.

### Assessment

The signal handling system is **excellent** and demonstrates careful attention to concurrency safety. The key design decision is to use atomic flags for graceful coordination between signal handler threads and the main thread, avoiding stdout races. The layered defense (graceful flag → 1-second timeout → force restore → watchdog) ensures terminal recovery even if the main loop is stuck. The SIGTSTP handling is particularly thorough — it restores the terminal *before* suspending, so the user gets a usable shell while cosmostrix is in the background, and properly reinitializes on SIGCONT.

---

## 7. Existing Regression Tests

The project has a comprehensive test suite covering terminal safety mechanisms:

### Terminal Cleanup Tests (`terminal.rs:621–660`)
- `emergency_reset_sequence_disables_terminal_reporting_modes`: Verifies all 9 terminal mode resets are present in the escape sequence string
- `terminal_cleanup_plan_is_reverse_order_and_idempotent`: Verifies cleanup order is LIFO and second invocation is a no-op

### Input Safety Tests (`interactive/tests.rs`)
- `plain_shortcut_key_is_not_ignored_without_burst`: Verifies normal typing is not suppressed
- `paste_burst_ignores_shortcut_letters`: Verifies rapid key events trigger suppression
- `paste_burst_suppression_expires`: Verifies 50ms suppression window expires
- `bracketed_paste_starts_printable_suppression_window`: Verifies `Event::Paste` activates suppression
- `paste_suppression_does_not_trigger_shortcut_actions`: Verifies suppressed keys don't reach keybinding handler

### Tab Key Safety Tests (`interactive/tests.rs`)
- `tab_key_is_ignored`: Verifies Tab is a no-op in handle_keybinding
- `backtab_key_is_ignored`: Verifies Shift+Tab is a no-op
- `tab_does_not_toggle_pause`: Verifies Tab doesn't affect pause state
- `tab_does_not_change_color_or_charset`: Verifies Tab doesn't affect visual state
- `tab_does_not_force_ghost_background_redraw`: Verifies Tab doesn't trigger semantic_invalidate or force_draw_everything
- `repeated_tab_is_stable`: Verifies 10 rapid Tab presses produce no cumulative effects

### Pause/Resume Tests (`cloud/tests/mod.rs`)
- `pause_stops_rain_and_unpause_resumes`: Basic pause/resume functionality
- `pause_freezes_simulation_time`: Timing doesn't advance during pause
- `resume_resets_timing_debt`: Spawn remainder, resume_blend, and timing are reset
- `repeated_pause_resume_does_not_accumulate_timing_debt`: Multiple cycles don't accumulate

### Redraw Safety Tests
- `periodic_full_redraw_survives_until_next_frame`: Full redraw counter triggers correctly
- `color_transition_starts_immediately_and_completes`: Transition mechanism works
- `charset_change_enters_transition_state_without_full_swap`: Charset transition doesn't force full swap
- `top_row_glyph_to_blank_is_dirty` and `blank_cells_are_marked_dirty_for_redraw` (`frame.rs`): Dirty tracking correctness

### Phosphor Safety Tests
- `phosphor_blank_cells_are_not_overridden_by_ghost`: Blank cells take priority over ghost afterglow
- `stale_bottom_cells_decay_to_blank_within_bounded_time`: Bottom-row decay acceleration
- `high_speed_does_not_create_unbounded_bottom_accumulation`: Concrete wall prevention
- `spawn_remainder_is_clamped`: Spawn burst prevention

### Activity/Idle Tests
- `idle_resync_uses_wall_clock_time`: Idle resync timing
- `idle_to_active_activity_schedules_resync`: Idle→active transition
- `active_mouse_activity_does_not_force_resync_every_frame`: Mouse movement doesn't cause resync spam
- `focus_activity_can_force_resync_while_active`: Focus regain triggers resync

---

## 8. Gaps and Recommendations

### Gaps Found

1. **No test for SIGCONT reinit path**: The terminal reinitialization after SIGTSTP/SIGCONT (event_loop.rs:212–224) is not directly tested. While individual components are tested, the full sequence (drop terminal → recreate → resize → force_draw) has no integration test. This is difficult to test deterministically in unit tests due to signal handling, but could be covered by an end-to-end test that sends SIGTSTP/SIGCONT to the process.

2. **No test for SIGKILL guard**: The fork-based SIGKILL guard (main.rs:103–156) is inherently difficult to test in unit tests. It relies on process lifecycle behavior. A manual test or shell script that kills the process with `kill -9` and verifies terminal recovery would be valuable.

3. **No explicit test for semantic_gen mismatch triggering full redraw**: While the `invalidate_semantic` mechanism is tested indirectly, there is no test that directly verifies the `Terminal::draw()` path where `semantic_gen` mismatch causes a full redraw without `Clear(All)`. This path (terminal.rs:241–249) is critical for mode-change visual correctness.

4. **Resize during pause not explicitly tested**: If the terminal is resized while paused, the resize event is queued but not applied until the next unpause. This is likely correct behavior (the event loop continues running during pause to process events), but it's not explicitly covered by tests.

5. **Shutdown timeout thread is not joinable**: The force-exit thread spawned in `Terminal::drop` (terminal.rs:513–520) uses `Builder::new().spawn()` without `.join()`, making it a detached thread. This is intentional (joining would block the drop, which is what the thread is guarding against), but it means the thread's lifetime is unbounded. The `shutdown_complete` flag mitigates this — the thread exits quickly if shutdown completes within the timeout — but in pathological cases (extremely slow stdout flush), the thread could outlive the process's main cleanup. This is acceptable since `process::exit(0)` is the fallback.

### Recommendations

1. **Add an integration test for SIGCONT reinit**: Consider a test helper that simulates the term_reinit path by manually dropping and recreating the Terminal and verifying the frame is clean.

2. **Document the SIGKILL guard behavior**: Add a comment or documentation note about the expected behavior when the process is killed with `kill -9` and how to disable the fork guard.

3. **Consider adding a frame-level semantic_gen test**: Create a test that sets up a LastFrame with semantic_gen=0, then draws a Frame with semantic_gen=1, and verifies that a full redraw occurs without Clear(All).

4. **Consider documenting the resize-during-pause behavior**: Add a comment in the event loop explaining that resize events received during pause are applied on the next loop iteration (which still runs during pause at the reduced rate).

---

## 9. Summary

The Cosmostrix terminal safety system is **exceptionally well-engineered**. The codebase demonstrates deep understanding of terminal emulator behavior, signal handling concurrency, and the many ways a TUI application can leave a terminal in a broken state. The defense-in-depth approach (RAII cleanup → best-effort restore → fork SIGKILL guard → panic hook → watchdog) leaves virtually no uncovered failure mode. The paste suppression system is clever and robust, the pause/resume timing management is comprehensive, and the redraw safety system correctly handles the subtle interactions between differential rendering, semantic invalidation, and phosphor persistence. The test coverage is strong with 25+ tests spanning all safety-critical paths.

The primary areas for improvement are testing coverage for signal handling paths (which are inherently difficult to unit test) and a few integration-level gaps. These are minor concerns in an otherwise exemplary terminal safety implementation.
