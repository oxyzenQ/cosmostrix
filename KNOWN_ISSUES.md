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

### Partial mitigation (v30)

The v30 event loop restructure added a guard that checks for the HUD
toggle key BEFORE the screensaver-exit check, preventing the self-exit
on Termux when the terminal delivers a synthetic EOF immediately after
the `i` key event. This guard is active in all current builds
(`hud_toggle_accepted` in `src/interactive/input.rs`).

However, the root cause (synthetic focus-loss / EOF events on Windows)
is not fully filtered. The partial mitigation reduces the Termux case
but does not eliminate the Windows ConHost case.

### Workarounds

Pick whichever applies to your setup:

1. **Avoid pressing `i`.** The HUD is purely informational — you can
   run cosmostrix without it indefinitely. All HUD info (FPS, frame
   time, droplet count) is also available in `--benchmark --json`
   output for scripted collection.

2. **Use `--benchmark` instead of interactive mode** for any
   measurement where you need guaranteed stability. Benchmark mode does
   not enter the interactive event loop, so the `i` key issue does not
   arise.

3. **On Windows, use Windows Terminal Preview** instead of ConHost.
   The Preview build has improved keyboard event delivery that
   reduces (but does not eliminate) the issue.

4. **On Termux, run cosmostrix inside `tmux`**. The tmux layer
   normalizes key event delivery and absorbs the synthetic EOF that
   causes the abrupt exit. Start `tmux`, then run `cosmostrix` inside
   the tmux session.

### Status

**Partially mitigated (v30 guard).** The Termux case is largely
resolved by the pre-screensaver-exit guard. The Windows ConHost case
may still occur. A full fix (filtering synthetic EOF/focus-loss events
in the crossterm event loop) is not currently scheduled due to the
complexity of testing across crossterm versions and platforms.

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

### Workaround

No workaround available. This is a terminal-level limitation of the Linux
VT's alternate screen implementation. On terminal emulators (GNOME Terminal,
Alacritty, kitty, etc.) that properly implement alternate screen buffers,
the screen is restored correctly after quit.

### Status

**Accepted as a known limitation.** No further fix planned — the issue
is in the terminal, not in cosmostrix.

---

## VTE-Based Terminals (Konsole, GNOME Terminal): Fullscreen Performance

### Symptom

When running cosmostrix in **fullscreen** on VTE-based terminals
(Konsole, GNOME Terminal, most terminals in GNOME/KDE environments),
performance drops below 100 FPS. Visual effects (particle sparks,
mouse-click ripples) experience lag and leave stale trails on screen.
The issue occurs in **all scenes**, not just specific ones. Alacritty
is unaffected.

### Affected platforms

- **Konsole** (KDE): all versions. VTE-based, CPU-rendered.
- **GNOME Terminal**: all versions. VTE-based, CPU-rendered.
- **Other VTE-based terminals**: XFCE Terminal, Mate Terminal, etc.
- **Not affected**: Alacritty, kitty, WezTerm, ghostty, foot (GPU-rendered
  or highly optimized software renderers that keep up with ANSI throughput).

### Root cause

VTE terminals are **CPU-rendered** (software rendering), unlike Alacritty
which uses GPU acceleration. At fullscreen cell counts, the ANSI byte
volume overwhelms VTE's parser, causing the lag. The self-healer
`aggressive_throttle` and phosphor decay boost hysteresis reduce dirty
cells, and the v50.0.0-beta.6 phosphor decay rate increase (5.5 to 8.0
for cross-terminal consistency) shortened trails to ~400 ms on all
terminals, but VTE's internal buffering creates a feedback delay that
prevents perfect stabilization under sustained fullscreen load.

### Workarounds

1. **Use Alacritty** (or another GPU-accelerated terminal) for the best
   performance. See `docs/TERMINAL_COMPATIBILITY.md` for the full list
   of recommended terminals.
2. **Run cosmostrix in a smaller window** (non-fullscreen) on VTE
   terminals. The lag only manifests at high cell counts (typically
   >5000 cells = ~120x40+).
3. **For benchmarking or performance-critical use**, always use
   Alacritty. The `--benchmark` mode is terminal-independent (headless),
   but interactive fullscreen requires a fast terminal.

### Status

**Particle stuck/hang FIXED (S-master-HUNT-21 + S-master-HUNT-22).**
Two layers were involved; the second completed the fix.

*Layer 1 (HUNT-21): motion/aging unification.* Particle aging was
real-time (`now - birth`) while motion used the clamped frame dt —
particles expired before finishing their trajectory. `sim_age` now
accumulates the same dt that drives motion. This was correct but
insufficient: it made aging *consistent* with motion without fixing
what fed both clocks.

*Layer 2 (HUNT-22): the dilated clock itself.* Particle physics
integrated `dt = min(dt_raw, 1/30, max_sim_delta) * resume_blend` per
frame. On VTE (10-15 FPS real frames of 67-200ms, with
`max_sim_delta` pinned at 15ms under saturated perf pressure) each
frame admitted only 15-33ms of particle time — a permanent 10-30%
time dilation. A 4.0s click ripple stretched to 20-40 wall-clock
seconds of slow drift ("snow ice"), border-touch sparks lingered
~2.3s instead of 350ms, the velocity decay froze particles mid-air
("stuck"), and they only vanished once the diluted `sim_age` finally
crossed the lifetime ("disappears by itself"). The co-spawned flash
wave aged in real time, which is why the click ring looked normal
while its sparks crawled.

The fix: all transient particle systems (QuantumParticle click
ripples + border-touch sparks, EngraveSpark, ScorchSmoke) now
integrate REAL elapsed time bounded by the
`PARTICLE_MAX_FRAME_DT_SECS` (250ms) anti-teleport cap — the same
real-time clock the flash wave, border-touch pulse, and ghost events
already use. Effects complete in their intended wall-clock duration
at any terminal speed; pause decel/resume easing is preserved via
`resume_blend`, and unpause shifts the per-system clocks forward so
no anti-teleport budget is burned after a pause. The rain and
monolith deliberately keep the dilated `max_sim_delta` clock: the
rain is an ambient field where slow motion reads as calm, while
click sparks are interaction impulses whose latency is a
responsiveness signal.

The VTE ANSI throughput bottleneck itself (slow frame rate on CPU-rendered
terminals) remains a known limitation — the fix addresses the particle
*visual* behavior, not the terminal's rendering speed.

---

## Reporting new issues

If you encounter an issue not listed here, please open a GitHub issue
at <https://github.com/oxyzenQ/cosmostrix/issues> with:

1. **Platform**: OS version, terminal emulator, terminal version
2. **cosmostrix version**: `cosmostrix --version`
3. **Reproduction**: exact command and key sequence
4. **Expected vs actual**: what you expected, what happened
5. **Logs**: stderr output (use `-v` for verbose mode if available)

For crash-related issues, a `RUST_BACKTRACE=1` backtrace is invaluable.
<!-- COSMOSTRIX-DISCLAIMER -->
<!--
  Documentation Disclaimer — read before relying on any data point.

  This document may contain stale data, hardcoded counts, or outdated
  file paths and symbol names. Maintainers update source code but may
  forget to sync every doc — the project ships 57 active .md files
  (plus historical docs in docs/archive/) and perfect sync is a known
  maintenance burden with diminishing returns.

  Source code (`src/**/*.rs`) is the single source of truth.
  Always cross-check against the actual `.rs` files before relying on
  any specific number (test count, LOC, FPS, ms timeout), file path,
  function name, or config key.

  If you find a discrepancy, please open a PR — the doc is wrong, not
  the source.
-->
