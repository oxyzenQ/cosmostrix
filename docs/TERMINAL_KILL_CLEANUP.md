<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Terminal Kill Cleanup

Cosmostrix uses crossterm alternate screen mode. Normal exit, Ctrl-C, and
SIGTERM (`pkill -f cosmostrix`) should all restore the terminal cleanly
without leaving residue in scrollback or on the prompt line.

## Process Model

`pgrep -af cosmostrix` shows two processes. This is expected:

- **Parent** (PID 1): The main render process — interactive event loop,
  signal handling, cloud simulation, rendering.
- **Child** (`cx-term-guard`, PID 2): A fork-based guard process created
  at startup (Linux only). It sleeps in `sigwait()` and restores terminal
  modes if the parent is killed with SIGKILL (`kill -9`), which cannot be
  caught by any signal handler.

The child inherits the parent's command line, so `pgrep -f cosmostrix`
matches both. The child renames itself to `cx-term-guard` via `prctl`
(`PR_SET_NAME`), but `pgrep -a` matches against the command line, not the
`comm` name.

To disable the fork guard: `COSMOSTRIX_NO_FORK_GUARD=1 cosmostrix`

## Expected Behavior

### Normal Exit (q / Esc / duration timeout)

Terminal::drop() runs, which:

1. Disables mouse capture and bracketed paste
2. Resets text attributes and colors
3. Shows cursor
4. Enables line wrap
5. Leaves alternate screen (revealing original terminal content)
6. Disables raw mode
7. Flushes the buffered writer

No screen clear, no scrollback modification. The original terminal content
is preserved underneath the alternate screen buffer. Normal exit does NOT
clear the visible screen and is intentionally non-destructive.

### Signal Exit (SIGINT / SIGTERM / SIGHUP)

Signal handler sets `GRACEFUL_SHUTDOWN` and a `signal_exit` flag. Main loop
notices `GRACEFUL_SHUTDOWN`, exits the render loop, sets `SHUTDOWN`, and
`Terminal::drop()` runs cleanup. Because `signal_exit` is set, the cleanup
path includes an additional step before leaving the alternate screen:

- **Signal-exit viewport clear**: `MoveTo(0,0)` + `Clear(All)` + flush
  inside the alternate screen buffer, then `LeaveAlternateScreen`.

This ensures the last rain frame is erased from the visible viewport before
the terminal emulator switches back to the main screen. Without this,
terminal emulators may briefly show the last rain frame content on the
main screen during the alternate-to-main transition.

The signal handler thread blocks until `SHUTDOWN` is observed.

### Ctrl-C (SIGINT)

Follows the signal-exit path above. Same viewport clear behavior as
SIGTERM/SIGHUP.

### SIGTERM (`pkill -f cosmostrix`)

Follows the signal-exit path above. Both parent and child fork-guard
process receive SIGTERM. The parent handles all terminal cleanup via
`Terminal::drop()`. The child checks `getppid()` — if the parent is still
alive (ppid != 1), the child exits silently without touching stdout,
avoiding a race with the parent's buffered writer. If the parent is already
dead (ppid == 1), the child performs terminal restoration.

### SIGHUP (terminal disconnect)

Same signal-exit path as SIGINT/SIGTERM via the shared signal handler.

### SIGKILL (`kill -9`, `pkill -9`)

**Cannot be caught by any process.** No signal handler runs. The terminal
may be left in raw mode with the alternate screen active.

On Linux, Cosmostrix spawns a fork-based guard process (`cx-term-guard`)
that watches for the parent's death. If the parent is killed with SIGKILL,
the kernel sends SIGTERM to the child via `PR_SET_PDEATHSIG`, and the child
(noticing `getppid() == 1`) restores the original `termios` state via
`tcsetattr(TCSANOW)` and calls `restore_terminal_best_effort()`.

This guard does NOT run if:

- `COSMOSTRIX_NO_FORK_GUARD=1` is set
- stdin/stdout are not a TTY
- not running on Linux (macOS/BSD are not covered)

## Stuck-Loop Fallback

If the main loop is truly stuck (deadlock inside a syscall, infinite loop
that doesn't check `GRACEFUL_SHUTDOWN`), the watchdog thread detects the
stuck state after 20 seconds and calls `restore_terminal_best_effort()`
+ `process::exit(1)` as a last resort.

## Recovery Commands

If the terminal is left in a broken state after any kill method:

```bash
# Reset terminal emulator state
printf '\033c'

# Restore sane TTY line discipline
stty sane

# Nuclear option (clears screen and scrollback)
cosmostrix --reset-terminal
```

## Normal Exit vs --reset-terminal

Normal exit only restores terminal modes and leaves the alternate screen.
It does NOT clear the screen or scrollback. This is intentional — the
alternate screen buffer preserves the original terminal content.

Signal exit (SIGINT/SIGTERM/SIGHUP) clears the visible viewport inside
the alternate screen before leaving, preventing rain frame residue.
It does NOT purge scrollback or modify the main screen.

`--reset-terminal` is the explicit destructive recovery option. It clears
the screen, purges scrollback, runs `stty sane` and `reset`, and should
only be used when the terminal is in a broken state.

## Phase 4 Fix (v4.8)

The signal handler fallback path previously called
`restore_terminal_best_effort()` + `process::exit()` after a 1-second
timeout. This raced on stdout with the main loop's buffered writer and
skipped `Terminal::drop()`, causing interleaved escape sequences that
left glyph residue in scrollback.

Fix: signal handler threads now set `GRACEFUL_SHUTDOWN` and block until
`SHUTDOWN` is observed, never calling `restore_terminal_best_effort()`
or `process::exit()` themselves. The watchdog thread (20 s timeout)
remains the sole fallback for truly stuck loops.

## Phase 4B Fix (v4.8)

After Phase 4, owner-side testing confirmed that `pkill -TERM` still
left visible rain residue on the main screen. Two root causes were
identified and fixed:

### Root Cause 1: Fork Guard Stdout Race

The fork guard child (`cx-term-guard`) previously called
`restore_terminal_best_effort()` on any SIGTERM, including when the
parent received `pkill -TERM` and was handling cleanup itself. This
caused both parent and child to write ANSI escape sequences to the same
stdout fd simultaneously, producing interleaved/garbled output.

Fix: the child now checks `getppid()` before restoring. If ppid is not 1
(parent still alive), the child exits silently. Only when the parent is
already dead (ppid == 1, indicating SIGKILL or crash) does the child
perform terminal restoration.

### Root Cause 2: No Viewport Clear Before Alternate Screen Switch

On signal-exit, `Terminal::drop()` left the alternate screen without
first clearing the visible viewport. The last rain frame content could
momentarily appear on the main screen during the terminal emulator's
alternate-to-main buffer switch, leaving visible glyph residue.

Fix: `Terminal` now accepts a `signal_exit: Arc<AtomicBool>` flag. When
set (by signal handlers for SIGINT/SIGTERM/SIGHUP), `cleanup_terminal()`
writes `MoveTo(0,0)` + `Clear(All)` + flush to the alternate screen
buffer before issuing `LeaveAlternateScreen`. Normal q/esc exit does not
set this flag, so normal exit remains non-destructive.

### Manual Test

```bash
# Terminal 1
printf '\033c'
stty sane
pkill -TERM -f cosmostrix || true
pgrep -af cosmostrix || echo "no cosmostrix process"

cosmostrix -mB --message "one world first seriously matrix rain" \
  --charset hacker --color forest --scene matrix

# Terminal 2
pgrep -af cosmostrix
pkill -TERM -f cosmostrix
sleep 1
pgrep -af cosmostrix || echo "cosmostrix stopped"

# Terminal 1 (after pkill)
echo pkill-term-clean
```

Expected: no rain glyphs left on visible shell output, prompt not
stained, terminal input normal.

v4.8 merge remains blocked until owner-side visual smoke confirms the fix.

## Terminal Lifecycle Matrix

A comprehensive matrix covering all terminal lifecycle paths (normal exit,
SIGINT, SIGTERM, SIGHUP, SIGTSTP/SIGCONT, SIGKILL, `--reset-terminal`,
Windows Terminal, tmux, ssh, headless, benchmark mode, and doctor mode)
is maintained in `docs/TERMINAL_LIFECYCLE_MATRIX.md`. That document is
the authoritative reference for cleanup guarantees across all paths.
This document covers the signal handling implementation details.