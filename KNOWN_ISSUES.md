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

Pressing `i` (lowercase) or `I` (uppercase) to toggle the live HUD
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

2. **Change the HUD toggle key.** Edit your config file
   (`cosmostrix --dump-config > ~/.config/cosmostrix/config.toml`) and
   remap the HUD toggle to a function key or `Ctrl+H` under the
   `[keybindings]` section. Function keys (`F1`–`F12`) are delivered
   more reliably than printable keys on the affected platforms.

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

Tracking: planned for v16.0.0 milestone.

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

If you encounter an issue not listed here, please open a GitHub issue
at <https://github.com/oxyzenQ/cosmostrix/issues> with:

1. **Platform**: OS version, terminal emulator, terminal version
2. **Cosmostrix version**: `cosmostrix --version`
3. **Reproduction**: exact command and key sequence
4. **Expected vs actual**: what you expected, what happened
5. **Logs**: stderr output (use `-v` for verbose mode if available)

For crash-related issues, a `RUST_BACKTRACE=1` backtrace is invaluable.
