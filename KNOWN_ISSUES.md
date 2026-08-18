# Known Issues
<!-- SPDX-License-Identifier: GPL-3.0-only -->

This file documents known platform-specific quirks, workarounds, and
planned fixes for cosmostrix. Items here are not bugs in the renderer
itself — they are interactions with terminal emulators, OS event
delivery, or PTY behavior that cosmostrix cannot fully work around.

For design-scope limitations (CPU-only, no audio, terminal-bounded FPS),
see the **Limitations** section of [README.md](README.md).

---

## Windows / Android (Termux): `i` key (HUD toggle) may cause sudden exit

### Symptom

Pressing `i` (lowercase only — uppercase `I` is a no-op) to toggle the live HUD
during an interactive run sometimes causes cosmostrix to exit abruptly
on **Windows** (Windows Terminal, ConHost, PowerShell) and **Android**
(Termux). The exit is unexpected — no panic message, no error, just a
return to the shell prompt.

### Affected platforms

- **Windows 10/11**: Windows Terminal, ConHost (`cmd.exe`), PowerShell,
  Windows Terminal Preview. Reproducible on some configurations but not
  others — appears related to the host's keyboard event coalescing and
  the crossterm event polling interval.
- **Android (Termux)**: All recent Termux versions. The Termux terminal
  emulator delivers key events through a different path than Linux PTYs,
  and certain printable-key sequences arrive inconsistently when the
  process is in raw mode.

### Root cause (suspected)

The `i` key is bound to `hud_state.toggle()` in
`src/interactive/event_loop.rs`. On Linux/macOS, crossterm delivers the
`KeyEvent` cleanly through the standard PTY `read()` path. On Windows,
crossterm uses the `Console` API which can coalesce rapid key events;
on Termux, the Android terminal layer sometimes delivers a key event
followed immediately by a synthetic EOF or focus-loss event, which the
event loop interprets as a shutdown signal.

This is a **platform event-delivery issue**, not a renderer bug — the
render pipeline itself is unaffected. The renderer continues correctly
up to the moment of exit.

### Workarounds

Pick whichever applies to your setup:

1. **Avoid pressing `i`.** The HUD is purely informational — you can
   run cosmostrix without it indefinitely. All HUD info (FPS, frame
   time, droplet count) is also available in `--benchmark --json`
   output for scripted collection.

2. **Change the HUD toggle key.** cosmostrix does NOT currently
   expose a config-level keybinding remap for the HUD toggle — the
   `i` binding (lowercase only) is hardcoded in `src/interactive/event_loop.rs`
   (not `input.rs`). As a
   workaround on affected platforms, run cosmostrix inside `tmux`
   (see option 5 below) which normalizes key event delivery, or use
   `--benchmark` mode (option 3) which does not enter the interactive
   event loop at all.

3. **Use `--benchmark` instead of interactive mode** for any
   measurement where you need guaranteed stability. Benchmark mode does
   not enter the interactive event loop, so the `i` key issue does not
   arise.

4. **On Windows, use Windows Terminal Preview** instead of ConHost.
   The Preview build has improved keyboard event delivery that
   reduces (but does not eliminate) the issue.

5. **On Termux, run cosmostrix inside `tmux`**. The tmux layer
   normalizes key event delivery and absorbs the synthetic EOF that
   causes the abrupt exit. Start `tmux`, then run `cosmostrix` inside
   the tmux session.

### Planned fix

A proper fix is planned for a future release. The current thinking is
to **filter synthetic focus-loss / EOF events** in the crossterm event
loop before they reach the keybinding dispatcher, and to add a
**per-platform key-event validation layer** that rejects events with
implausible timing (e.g. an EOF arriving <1ms after a printable
KeyEvent). This requires careful testing across crossterm versions and
platforms to avoid regressing legitimate fast-keypress scenarios.

Tracking: no fix currently scheduled — see workaround above.

---

## Windows Terminal: forced-termination cleanup is best-effort

### Symptom

Forced termination of cosmostrix on Windows Terminal / ConHost (via
task kill, window close, or signout) may leave the terminal in a
degraded state: scrolled buffer visible, cursor hidden, alternate
screen not restored.

### Workaround

Run `cosmostrix --reset-terminal` to perform 5-layer recovery
(ANSI + crossterm + stty + reset + alternate-screen).

### Status

This is a fundamental limitation — no process can intercept forced
termination. Tracked in
[#15](https://github.com/oxyzenQ/cosmostrix/issues/15). Not planned
for fix; the `--reset-terminal` recovery path is the official remedy.

---

## Reporting new issues

## TTY / Linux VT: screen cleared after quit ('q')

### Symptom

After running cosmostrix on a real TTY (Linux virtual console, e.g.
Ctrl+Alt+F1) and pressing 'q' to quit, the terminal screen is cleared.
Previous terminal history (e.g., `echo hello` output) is gone — the
screen is blank with only the shell prompt visible.

### Affected platforms

- **Linux virtual console** (TTY1-TTY6, TERM=linux): screen clear after
  quit is a terminal-level behavior. The Linux VT's alternate screen
  buffer implementation (vt.c, kernel 2.6.x+) may clear the visible
  screen content when switching back from the alternate screen, even
  though cosmostrix does NOT emit Clear(All) during cleanup.

### Root cause

This is a **terminal-level limitation**, not a cosmostrix bug. cosmostrix
performs the following scrollback-safe cleanup on normal exit (see
`docs/TERMINAL_KILL_CLEANUP.md` for the full sequence):

1. Disable mouse capture, bracketed paste, focus events
2. Reset text attributes and colors
3. Show cursor, enable line wrap
4. Emit SYNC_END (if sync_output supported)
5. Flush BufWriter BEFORE LeaveAlternateScreen (v50 fix)
6. LeaveAlternateScreen (restores main screen buffer)
7. Disable raw mode
8. Final flush

No `Clear(All)` (`\x1b[2J`), no RIS (`\x1bc`), no scrollback-modifying
sequences are emitted during normal exit. The cleanup is intentionally
non-destructive.

However, some TTY/terminal implementations clear the visible screen as
a side effect of the alternate screen switch (`\x1b[?1049l`). This is
terminal-level behavior that cosmostrix cannot control — the escape
sequence for leaving the alternate screen is standardized, but how each
terminal implements the screen restoration varies.

### Previous fix attempts

Multiple fix attempts were made across sessions:
1. Removed `Clear(All)` before `LeaveAlternateScreen` — addressed VTE
   scrollback-clear but did not fix TTY screen clear.
2. Fixed SYNC_START/END ordering (emit SYNC_END before
   `LeaveAlternateScreen`, not after) — addressed sync-mode leak but
   did not fix TTY screen clear.
3. Removed SYNC_START at init time — addressed main-screen sync open
   but did not fix TTY screen clear.
4. Moved `flush()` outside `if sync_output` block to always flush before
   `LeaveAlternateScreen` — addressed BufWriter content leaking to main
   screen but did not fix TTY screen clear.

None of these fixes resolved the TTY screen clear because the root cause
is the terminal's own behavior when processing `LeaveAlternateScreen`, not
any sequence cosmostrix emits.

### Workaround

No workaround available. This is a terminal-level limitation of the Linux
VT's alternate screen implementation. On terminal emulators (GNOME Terminal,
Alacritty, kitty, etc.) that properly implement alternate screen buffers,
the screen is restored correctly after quit.

### Status

**Accepted as a known limitation.** No further fix planned — the issue
is in the terminal, not in cosmostrix.

---

If you encounter an issue not listed here, please open a GitHub issue
at <https://github.com/oxyzenQ/cosmostrix/issues> with:

1. **Platform**: OS version, terminal emulator, terminal version
2. **Cosmostrix version**: `cosmostrix --version`
3. **Reproduction**: exact command and key sequence
4. **Expected vs actual**: what you expected, what happened
5. **Logs**: stderr output (use `-v` for verbose mode if available)

For crash-related issues, a `RUST_BACKTRACE=1` backtrace is invaluable.
