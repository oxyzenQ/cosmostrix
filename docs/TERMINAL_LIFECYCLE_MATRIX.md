<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Terminal Lifecycle Matrix

This document defines the expected behavior of cosmostrix across every
terminal lifecycle path.  It serves as the authoritative reference for
cleanup guarantees, terminal state restoration, and destructive vs.
non-destructive behavior.

## Matrix

| # | Path | Cleanup | Visible Screen Cleared | Scrollback Purged | Terminal Mode Restored | Catchable | Owner/Manual Verification | Destructive |
|---|------|---------|----------------------|-------------------|----------------------|-----------|--------------------------|-------------|
| 1 | Normal `q` exit | Full via `Terminal::drop()` | No | No | Yes | Yes | No | No |
| 2 | SIGINT (Ctrl+C) — **DEPRECATED**: SIGINT is no longer caught. Only `q` exits. Ctrl+C at the key level is ignored; `kill -INT` will use OS default (terminate without cleanup). Use SIGTERM/SIGHUP/SIGQUIT for graceful signal exit. | N/A (signal not caught) | No | No | No (OS default) | No | N/A | Depends on OS |
| 3 | SIGTERM / `pkill -TERM` | Full via `Terminal::drop()` + signal-exit viewport clear | Yes (alternate buffer cleared before switch) | No | Yes | Yes | No | No |
| 4 | SIGHUP | Full via `Terminal::drop()` + signal-exit viewport clear | Yes (alternate buffer cleared before switch) | No | Yes | Yes | No | No |
| 5 | SIGTSTP / Ctrl-Z | Partial — terminal mode suspended, no explicit cleanup | No | No | Deferred (on SIGCONT) | Yes | No | No |
| 6 | SIGCONT resume | Terminal mode restored, render loop continues | No | No | Yes | Yes | No | No |
| 7 | SIGKILL / `kill -9` | None — cannot be caught | No | No | Fork guard best-effort (Linux only) | No | Yes (residual state possible) | Potentially |
| 8 | `--reset-terminal` | Full destructive reset | Yes | Yes | Yes (`stty sane` + `reset`) | N/A (intentional) | N/A (explicit user action) | Yes |
| 9 | Windows Terminal reset path | Manual user recovery | User action | User action | User action | N/A | Yes (issue #15) | User-controlled |
| 10 | tmux | Same as paths 1-7 within tmux pane | Same as triggering path | No (tmux scrollback preserved) | Yes | Same as triggering path | No | No |
| 11 | ssh | Same as paths 1-7 over remote PTY | Same as triggering path | No (remote scrollback preserved) | Yes | Same as triggering path | Recommended | No |
| 12 | headless / non-TTY (no controlling terminal) | No terminal state set up — plain interactive run fails fast: cleanup burst, then one branded `error: os error 6` (ENXIO) render with a headless tip pointing at `--benchmark` / `--doctor` / `--dump-config`, exit 1 (v100.0.0-nightly.1: the second raw Rust Debug `Error: Os { ... }` render was removed); `--benchmark` / `--doctor` run cleanly (exit 0) | N/A | N/A | N/A | N/A | No | No |
| 13 | Benchmark mode (`--benchmark`) | Full via `Terminal::drop()` (same as normal exit) | No | No | Yes | Yes | No | No |
| 14 | Doctor mode (`--doctor`) | No terminal mode changes — no cleanup needed | N/A | N/A | N/A | N/A | No | No |

## Detailed Path Descriptions

### 1. Normal `q` Exit

The user presses `q` during interactive rendering. The main loop
detects the quit key, sets `SHUTDOWN`, exits the render loop, and
`Terminal::drop()` runs the full cleanup sequence:

1. Disables mouse capture and bracketed paste mode.
2. Resets text attributes and colors.
3. Shows the cursor.
4. Enables line wrap.
5. Leaves the alternate screen (revealing original terminal content).
6. Disables raw mode.
7. Flushes the buffered writer.

No screen clear, no scrollback modification. The alternate screen buffer
preserves the original terminal content underneath the rain. This path is
intentionally non-destructive — the user's shell history and previous
output are fully intact.

### 2. Ctrl-C / SIGINT (DEPRECATED)

**Change**: SIGINT (Ctrl+C) is no longer in the graceful-shutdown
signal list. Only `q` exits cosmostrix. This deprecation aligns with the
cinematic design principle: the user must deliberately press `q` to quit —
no accidental exits from terminal Ctrl+C muscle memory.

- Ctrl+C at the key level: ignored (crossterm key event, no match in
  `input.rs` — falls through to `_ => {}`).
- `kill -INT cosmostrix`: OS default SIGINT disposition applies (terminate
  without graceful cleanup). The user should use `kill -TERM` instead for
  graceful shutdown, or press `q`.

SIGTERM/SIGHUP/SIGQUIT remain caught for system-initiated shutdown
(terminal close, parent death, `kill(1)`).

### 3. SIGTERM / `pkill -TERM`

Same signal-exit path as SIGTERM. The signal handler sets
`GRACEFUL_SHUTDOWN` and `signal_exit`. Both the parent process and the
fork guard child receive SIGTERM. The child checks `getppid()` — if the
parent is still alive (ppid != 1), the child exits silently without
touching stdout, avoiding a race with the parent's buffered writer. If
the parent is already dead (ppid == 1), the child performs terminal
restoration. The parent handles all cleanup via `Terminal::drop()` with
the signal-exit viewport clear. Visible residue should be fully cleaned
after Phase 4B fix.

### 4. SIGHUP

SIGHUP is delivered when the controlling terminal is disconnected (e.g.
SSH session drops, terminal emulator closes). Same signal-exit path as
SIGTERM/SIGQUIT. The signal handler sets `GRACEFUL_SHUTDOWN` and
`signal_exit`, and the main loop runs the full cleanup including the
viewport clear. Note that if the terminal is already disconnected, the
ANSI escape sequences may not reach a display — but the terminal mode
restoration via `tcsetattr()` is still attempted.

### 5. SIGTSTP / Ctrl-Z Suspend

SIGTSTP suspends the process. cosmostrix does not install a custom
SIGTSTP handler, so the OS default behavior applies: the process is
suspended and the shell regains control. The terminal remains in raw
mode with the alternate screen active while the process is suspended.
No cleanup runs at suspend time. This is a known limitation — the
terminal state is deferred until SIGCONT.

### 6. SIGCONT Resume

When the process is resumed (via `fg` or `kill -CONT`), the main loop
continues from where it left off. The terminal is already in the state
it was in when suspended (alternate screen, raw mode). No additional
restoration is needed because cosmostrix never released the terminal.
If the terminal was externally modified while suspended (e.g. another
program wrote to the TTY), the display may be corrupted — this is an
inherent limitation of suspend/resume.

### 7. SIGKILL / `kill -9`

**SIGKILL cannot be caught or cleaned up by the process.** No signal
handler runs. No `Terminal::drop()` executes. The terminal may be left
in raw mode with the alternate screen active, mouse capture enabled, and
the cursor hidden.

On Linux, cosmostrix spawns a fork-based guard process (`cx-term-guard`)
that watches for the parent's death via `PR_SET_PDEATHSIG`. When the
parent is killed with SIGKILL, the kernel sends SIGTERM to the child,
and the child (noticing `getppid() == 1`) restores the original
`termios` state via `tcsetattr(TCSANOW)` and calls
`restore_terminal_best_effort()`. This is best-effort only.

The fork guard does NOT run if:

- `COSMOSTRIX_NO_FORK_GUARD=1` is set.
- stdin/stdout are not a TTY (e.g. piped, redirected, headless).
- Not running on Linux (macOS/BSD are not covered).

Terminal residue after SIGKILL is expected and documented. The user
should run manual recovery if needed (`stty sane`, `printf '\033c'`,
or `cosmostrix --reset-terminal`).

Live verification 2026-08-30 (v80.0.0-beta.1.0-beta.1,
`docs/audits/LTS_MATRIX_MIDSESSION_RETEST.md`): with the normal
terminal topology (cosmostrix as a foreground child, shell as session
leader), `kill -9` left the guard child alive and termios was fully
restored to cooked mode within 0.5 s. Edge case: when cosmostrix itself
is the session leader (launched via `setsid`, some desktop launchers),
the session-leader exit delivers SIGHUP to the foreground process
group; the guard blocks/waits only on SIGTERM, so the SIGHUP default
disposition kills the guard before it can restore — residue remains in
that topology. Hardening candidate (add SIGHUP to the guard's signal
set) is documented as finding F3 of the retest report.

### 8. `--reset-terminal`

This is an explicitly destructive recovery option. It performs:

1. Full screen clear.
2. Scrollback purge (terminal emulator dependent).
3. `stty sane` — restores sane TTY line discipline.
4. `reset` — full terminal emulator reset.

`--reset-terminal` should only be used when the terminal is in a broken
state after a SIGKILL, crash, or other unexpected termination. This is
not part of normal operation and is intentionally destructive.

### 9. Windows Terminal Reset Path

Windows Terminal behavior on process termination differs from Unix PTY
semantics. The Windows-specific reset path is user-verified and tracked
via issue #15. cosmostrix does not currently claim specific cleanup
guarantees on Windows Terminal beyond what crossterm provides. Windows
users experiencing broken terminal state after forced termination should
use the Windows Terminal "Reset" tab option or close and reopen the
terminal.

### 10. tmux

When running inside a tmux pane, all signal and exit paths behave
identically to the descriptions above. The alternate screen operates
within the tmux pane, so cleanup affects only the pane, not the outer
terminal. tmux preserves its own scrollback independently — cosmostrix
cleanup does not purge tmux scrollback. The fork guard (Linux) operates
normally within tmux. `pkill -TERM -f cosmostrix` works correctly; the
child guard correctly detects parent death via `getppid()`.

### 11. ssh

When running over an SSH session, all signal and exit paths behave
identically to local execution. The alternate screen operates on the
remote PTY. SIGHUP is delivered if the SSH connection drops, triggering
the signal-exit cleanup path. If the SSH connection drops abruptly, the
remote terminal state may not be fully cleaned up — the user should run
`stty sane` on reconnect. Owner-side visual smoke testing is recommended
over SSH before release if terminal code changes.

### 12. Headless / Non-TTY

When the process has no controlling terminal at all (CI environment,
cron job, `ssh -T`, piped stdin/stdout with no ctty), a plain
interactive run fails fast during terminal setup: cosmostrix emits a
cleanup burst of reset sequences (restore_terminal_best_effort), then
prints one branded `error: No such device or address (os error 6)`
(ENXIO from the tty open) plus a headless tip pointing at the
non-interactive modes (`--benchmark`, `--doctor`, `--dump-config`),
and exits with code 1 — no hang, no partial render loop, no terminal
state left to clean. v100.0.0-nightly.1 (owner hunt 2026-09-04): the
error used to render TWICE (a plain `error: {e}` line followed by
Rust's default main-Err Debug render `Error: Os { code: 6, ... }`);
the fatal path now renders once, branded, via eprintln_error_labeled
with an explicit exit(1) after the warning drain. Verified live
2026-08-30 (see
`docs/audits/LTS_MATRIX_MIDSESSION_RETEST.md` finding F2).

`--benchmark` and `--doctor` are the supported headless paths: they
run to completion and exit 0 without any alternate screen, raw mode,
or mouse capture. When a controlling terminal exists but stdout is
piped, interactive mode still initializes terminal state through
`/dev/tty`, so use `--benchmark` for measurement pipelines.

### 13. Benchmark Mode (`--benchmark`)

Benchmark mode runs the renderer, prints performance statistics to
stdout, and exits. The terminal lifecycle is identical to a normal `q`
exit: `Terminal::drop()` runs the full non-destructive cleanup. The
alternate screen is entered and left cleanly. No viewport clear is
performed (benchmark exit is not a signal exit). Benchmark output goes
to stdout and is captured by the calling process.

### 14. Doctor Mode (`--doctor`)

Doctor mode prints diagnostic information and exits without entering the
render loop. It does not enter alternate screen mode, does not set raw
mode, and does not modify terminal state. No cleanup is needed.

## Honesty Notes

- **Alt-screen usage is TERM-conditional.** Only `dumb` and an unset
  TERM lack alternate screen support (`termdetect::has_alternate_screen`).
  In that fallback cosmostrix renders directly on the MAIN screen — the
  visible screen content is overwritten (scrollback is untouched) and
  the non-destructive "alternate buffer preserves original content"
  guarantee of path 1 does not apply. Verified live 2026-08-30: with
  TERM unset no `\x1b[?1049h/l` pair is emitted; with
  TERM=xterm-256color the enter/leave pair is correct.

- **SIGKILL cannot be caught or cleaned up by the process.** The fork
  guard is best-effort and Linux-only. Terminal residue after SIGKILL
  is expected.

- **`--reset-terminal` is explicitly destructive recovery.** It clears
  the screen and purges scrollback. It is not part of normal operation.

- **Normal `q` exit is non-destructive.** The alternate screen
  buffer preserves original terminal content. No scrollback modification.
  (Esc and Ctrl+C are intentionally ignored at the key level — only `q`
  quits. SIGINT is no longer caught at the signal level either.
  SIGTERM/SIGHUP/SIGQUIT from `kill` are still caught and trigger
  the signal-exit cleanup path.)

- **SIGTERM should be clean for visible residue after Phase 4B fix.**
  The signal-exit viewport clear prevents rain frame residue on the
  main screen during buffer switch.

- **Windows Terminal reset path is user-verified via issue #15.**
  cosmostrix does not claim specific cleanup guarantees on Windows
  Terminal beyond crossterm's cross-platform support.

- **Heavy message/matrix mode is not comparable to the default
  benchmark.** These modes have different computational costs and must
  not be used to inflate or deflate release benchmark numbers.

- **SIGTSTP suspend leaves the terminal in raw mode.** This is a known
  limitation. Recovery is automatic on SIGCONT if no external
  modification occurred.

## Cross-Reference

- `docs/TERMINAL_KILL_CLEANUP.md` — detailed signal handling
  implementation and fork guard behavior.
- `docs/RELEASE_GUARD.md` — mandatory pre-tag gates including terminal
  lifecycle verification.
- `test/docs_tests/` — static docs tests that
  guard the correctness of this document.

## Doctor / Report

`--doctor` is diagnostic/report-only. It does not enter alternate screen
mode, does not set raw mode, and does not modify terminal state. It
prints environment, compatibility, and lifecycle contract information
then exits. Owner visual smoke testing remains required before release
if terminal or runtime behavior changes.
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
