<!-- SPDX-License-Identifier: MIT -->

# Terminal Kill Cleanup

Cosmostrix uses crossterm alternate screen mode. Normal exit, Ctrl-C, and
SIGTERM (`pkill -f cosmostrix`) should all restore the terminal cleanly
without leaving residue in scrollback or on the prompt line.

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
is preserved underneath the alternate screen buffer.

### Ctrl-C (SIGINT)

Signal handler sets `GRACEFUL_SHUTDOWN` flag. Main loop notices the flag,
exits the render loop, sets `SHUTDOWN`, and `Terminal::drop()` runs the
same cleanup sequence as normal exit. The signal handler thread blocks
until `SHUTDOWN` is observed.

### SIGTERM (`pkill -f cosmostrix`)

Identical to Ctrl-C path. SIGTERM is registered in the same signal handler
as SIGINT and SIGHUP. The main loop exits cleanly via `Terminal::drop()`.

### SIGHUP (terminal disconnect)

Same path as SIGINT/SIGTERM via the shared signal handler.

### SIGKILL (`kill -9`, `pkill -9`)

**Cannot be caught by any process.** No signal handler runs. The terminal
may be left in raw mode with the alternate screen active.

On Linux, Cosmostrix spawns a fork-based guard process (`cx-term-guard`)
that watches for the parent's death. If the parent is killed with SIGKILL,
the kernel sends SIGTERM to the child, which restores the original
`termios` state via `tcsetattr(TCSANOW)` and calls
`restore_terminal_best_effort()`.

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