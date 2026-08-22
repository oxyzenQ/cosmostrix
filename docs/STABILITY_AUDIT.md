# Cosmostrix Terminal Stability Audit Report
<!-- SPDX-License-Identifier: GPL-3.0-only -->

**Scope**: Terminal safety, input safety, redraw safety, pause/resume, resize, signal handling. **Files analyzed**: `src/cosmic_dragon_engine/terminal/mod.rs`, `src/interactive/event_loop.rs`, `src/interactive/input.rs`, `src/main.rs`, plus supporting modules (`src/interactive/watchdog.rs`, `src/cosmic_dragon_engine/cloud/mod.rs`, `src/cosmic_dragon_engine/cloud/rain.rs`, `src/cosmic_dragon_engine/frame.rs`, `src/types/constants.rs`, `src/interactive/activity.rs`).

## 1. Terminal Cleanup on Exit — Four-Layer Defense-in-Depth

**Layer 1: RAII Drop Guard** (`Terminal::drop`, `src/cosmic_dragon_engine/terminal/mod.rs:909`). `Terminal` implements `Drop`, guaranteeing cleanup runs even during panic unwinding. The `Drop` impl spawns a force-exit watchdog thread *before* performing cleanup. This thread sleeps for `SHUTDOWN_TIMEOUT_SECS` (2 seconds, `src/central_control_dragon_power/mod.rs:270`) and then checks an `Arc<AtomicBool>` (`shutdown_complete`). If cleanup finished normally, the flag is set to `true` and the watchdog exits harmlessly. If cleanup is stuck (e.g., stdout pipe is broken and `flush()` blocks), the watchdog calls `process::exit(0)` as a last resort.

**Layer 2: Idempotent Cleanup** (`cleanup_terminal`, `src/cosmic_dragon_engine/terminal/mod.rs:773`). Strictly idempotent — the `cleaned_up` boolean guard at line 775 prevents any cleanup step from executing twice. Each state flag (`mouse_capture_enabled`, `bracketed_paste_enabled`, `cursor_hidden`, `line_wrap_disabled`, `alternate_screen_enabled`, `raw_mode_enabled`) is checked individually and cleared after the corresponding ANSI command is issued. Cleanup order is **reverse-LIFO** relative to setup: disable mouse capture → disable bracketed paste → reset attributes/colors → show cursor → re-enable line wrap → leave alternate screen → disable raw mode → flush stdout. LIFO ordering verified by unit test `terminal_cleanup_plan_is_reverse_order_and_idempotent`.

**Layer 3: Best-Effort Restore** (`restore_terminal_best_effort`, `src/cosmic_dragon_engine/terminal/mod.rs:944`). Standalone function callable from any context (signal handlers, panic hooks, other threads) without access to the `Terminal` instance. Issues a comprehensive `TERMINAL_RESET_SEQUENCE` containing all known terminal reporting mode resets: `?1000l`/`?1002l`/`?1003l` (mouse tracking), `?1006l`/`?1015l` (mouse encoding), `?2004l` (bracketed paste), `?1004l` (focus reporting), `?1049l` (alternate screen buffer), `?25h` (show cursor), followed by `0m` (attribute reset). Coverage verified by test `emergency_reset_sequence_disables_terminal_reporting_modes`.

**Layer 4: Fork-Based SIGKILL Guard** (`spawn_kill9_terminal_guard`, main.rs:103–156). On Linux, before the main loop starts, the process forks a child that saves the current `termios` state and then waits for `SIGTERM`. If the parent is killed with `SIGKILL` (which cannot be caught), the kernel reparents the child to `init` (PID 1). The child detects this via `getppid() == 1`, restores the terminal via `tcsetattr()`, and calls `restore_terminal_best_effort()`. Opt-out via `COSMOSTRIX_NO_FORK_GUARD` env var; only activates when stdin/stdout are confirmed TTY devices. Uses `PR_SET_PDEATHSIG` as belt-and-suspenders, blocks `SIGTERM` in the child via `pthread_sigmask`, uses `_exit()` (not `exit()`) to avoid atexit handlers.

**Additional: Panic Hook** (main.rs:163–166). `std::panic::set_hook` installed at the very start of `main()` calls `restore_terminal_best_effort()` before printing the panic info, ensuring a broken terminal never results from a panic.

**Assessment**: exemplary. The four-layer defense (RAII drop → idempotent cleanup → best-effort restore → fork SIGKILL guard) covers all realistic failure modes including normal exit, panic, SIGKILL, and stuck flush. The LIFO ordering is correct and tested.

## 2. Input Safety — Bracketed Paste Detection + Control Characters

**Bracketed Paste Mode** is enabled during terminal setup (`terminal.rs:133`), causing the terminal emulator to wrap pasted text in `\x1b[200~` and `\x1b[201~` escape sequences. crossterm translates these into `Event::Paste(String)` events.

**PasteBurstGuard** (`input.rs:34–61`) provides two independent suppression mechanisms:

1. **Bracketed paste window**: When a `Event::Paste(_)` event is received, `note_bracketed_paste()` sets `suppress_until` to `now + PASTE_BURST_SUPPRESS_MS` (50ms). Any plain printable keys arriving within this window are silently consumed by `ignore_plain_key()`.
2. **Queued-event heuristic**: Even without bracketed paste support, `ignore_plain_key()` takes a `queued_event_ready` parameter. The event loop checks `Terminal::poll_event(Duration::from_millis(0))?` immediately after receiving a keypress. If more events are already queued, this strongly suggests a paste (a human cannot type two characters within a single `poll` cycle), and the key is suppressed. Suppression self-extends: once active, the 50ms window refreshes on each suppressed key.

**Plain printable key detection** (`is_plain_printable_key`, input.rs:63–70) is deliberately conservative — only matches `KeyCode::Char(_)` with `KeyModifiers::NONE` or `KeyModifiers::SHIFT`. Special keys (arrows, function keys, Escape), control keys (Ctrl+C, Ctrl+Z), and Alt-modified keys are never suppressed, ensuring legitimate shortcuts always work. In the event loop, suppressed keys still trigger `register_activity()` and `force_draw_everything()` to update the idle timer and ensure display responsiveness; they simply skip `handle_keybinding()`.

**Control character handling**: Ctrl+C silently ignored (only `q` quits; SIGINT deprecated at signal level). Ctrl+Z in-app suspend keybind REMOVED (only OS-driven SIGTSTP works via `signal_handlers.rs`). Escape silently ignored (only `q` quits). Tab/BackTab explicitly ignored — historical bug: Tab previously toggled shading mode, which caused a ghost background glyph flood via `set_shading_mode()` → `semantic_invalidate` → `invalidate_semantic()` → frame clear without clearing `phosphor_base_ch`.

**Assessment**: well-designed with defense-in-depth. The 50ms suppression window is long enough to cover any paste burst but short enough to not interfere with fast typing (typical inter-key interval is 100–200ms). Tests verify suppression activation, expiration, and that shortcut actions are not triggered during suppression.

## 3. Redraw Safety

**Periodic Full Redraw (ANSI Drift Correction)**: Long-running terminal applications can accumulate ANSI state desynchronization. The renderer performs a periodic full redraw every `FULL_REDRAW_INTERVAL_FRAMES` (18,000 frames, `src/central_control_rains/mod.rs:899`) — at 60 FPS, approximately every 5 minutes. Counter maintained in `cloud.frames_since_full_redraw`, checked in the `rain_at()` function. The full redraw path resets the cursor to `(0,0)`, iterates every cell, issues style changes and characters, then resets attributes and colors.

**Semantic Invalidation on Mode Changes**: When the renderer's *semantic identity* changes (charset switch, shading mode toggle, color scheme change), simply dirtying cells is insufficient. `Frame::invalidate_semantic()` increments `semantic_gen` and performs a full logical clear. `Terminal::draw()` checks both dimension changes and semantic generation mismatches. Dimension changes trigger `Clear(All)` to handle stale content at terminal edges; semantic-only changes skip the clear to avoid visible flicker. Triggered by `set_shading_mode()`, charset transitions (handled via `transition_chars()` wave-based), and color scheme transitions (via palette transition system, not `semantic_invalidate`).

**`force_draw_everything` + Phosphor State Clearing**: `force_draw_everything` (triggered by paste events, focus regain, idle resync, user input after idle, periodic full redraw) sets `frame.clear_with_bg()` which bumps the frame generation, making all cells appear dirty. However, this alone is insufficient because the **phosphor persistence system** maintains a separate `phosphor_base_ch` array storing the original character glyph for ghost afterglow cells. Without clearing this array, a full redraw would expose all ghost glyphs as visible background characters — the "ghost background" bug. The fix clears `phosphor_base_ch` in both `semantic_invalidate` and `force_draw_everything` paths; active trail cells repopulate their entries through the normal Pass 1 and Pass 2 mechanisms of `phosphor_decay_pass`.

**Dirty Threshold for Full Redraw**: When differential rendering is active, if dirty cells exceed `total_cells / DIRTY_THRESHOLD_RATIO` (ratio of 8, bumped from 3 → 8 based on the `threshold_sweep` cosmic dragon egg benchmark), the renderer switches to a full redraw automatically. Prevents pathological cases where nearly every cell is dirty but differential rendering incurs more overhead than a full redraw due to per-cell cursor movement.

**Assessment**: comprehensive and well-layered. Periodic full redraws, semantic invalidation, phosphor state clearing, and dirty threshold fallback cover all known ANSI drift scenarios. The distinction between dimension-change (with `Clear(All)`) and semantic-change (without `Clear(All)`) is a subtle but important optimization that prevents flicker during mode transitions.

## 4. Pause/Resume + Resize Safety

**Pause/Resume** (`toggle_pause()`, `src/cosmic_dragon_engine/cloud/mod.rs:646`): thorough timing debt cancellation on resume. (1) Spawn debt reset: `last_spawn_time = now` and `spawn_remainder = 0.0` — without this, accumulated spawn remainder during a long pause would cause a burst of hundreds of new droplets on the first resumed frame. (2) Per-droplet timing reset: each alive droplet has its time incremented via `d.increment_time(elapsed)` and `d.last_time` set to `now` with `d.advance_remainder = 0.0` — ensures droplets don't jump multiple rows on resume. (3) Frame timing debt reset: `cloud.frame_time_debt = 0.0` and `cloud.last_render_time = now` — ensures the first resumed frame doesn't try to catch up on the entire pause duration. Tests verify pause/resume preserves droplet positions and that resuming after a long pause doesn't cause visual chaos.

**Resize**: `Terminal::draw()` detects dimension changes by comparing the last frame's `width`/`height` with the current frame's. On change: triggers `Clear(All)` to handle stale content at terminal edges, reallocates `phosphor_base_ch` and `last_frame` buffers to match new dimensions, resets `cloud.frames_since_full_redraw` to force a full redraw on the next frame, and bumps `semantic_gen` to invalidate all cells. The resize path is tested with various terminal sizes and aspect ratios. The `WINCH` signal handler (Unix) updates the cached terminal size and triggers a redraw on the next frame; on Windows, the console API is polled for size changes.

## 5. Signal Handling

**Unix** (`src/interactive/signal_handlers.rs`): `SIGTERM`/`SIGHUP`/`SIGQUIT` → set `GRACEFUL_SHUTDOWN` and `signal_exit` atomic flags, wait up to 3 seconds for main loop to clean up. `SIGTSTP`/`SIGCONT` → disable mouse capture, restore terminal, raise `SIGSTOP` for proper Ctrl+Z suspend. `SIGINT` deliberately NOT handled — only `q` exits cosmostrix (documented policy; SIGINT is no longer in the graceful-shutdown signal list). `SIGWINCH` → triggers terminal size re-detection on next frame.

**Windows**: `ctrlc::set_handler` for CTRL_C_EVENT + CTRL_BREAK_EVENT — same graceful-shutdown atomic flag pattern as Unix.

**Assessment**: signal handlers are defensive and well-scoped. They only set atomic flags or call async-signal-safe functions. The 3-second cleanup timeout prevents indefinite hangs. The SIGTSTP/SIGCONT handling properly restores terminal modes during suspend/resume cycles.

## Cross-References

- `docs/SECURITY_AUDIT.md` — security audit (network, filesystem, subprocess, unsafe)
- `docs/TERMINAL_KILL_CLEANUP.md` — kill/crash recovery details
- `docs/TERMINAL_LIFECYCLE_MATRIX.md` — full terminal lifecycle (init, alt screen, raw mode, cleanup)
- `docs/ENDURANCE.md` — long-running stability methodology
- `docs/archive/audits/UNSAFE_SOUNDNESS_AUDIT.md` — unsafe block soundness audit + Miri methodology
