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

- **Konsole** (KDE): all versions. CPU-rendered (QPainter-based KonsolePart).
- **GNOME Terminal / kgx (GNOME Console)**: all versions. VTE-based,
  CPU-rendered.
- **Other VTE-based terminals**: XFCE Terminal, Mate Terminal, etc.
- **foot**: CPU-rendered (fast parser, but the paint pass is CPU).
  HUNT-24 removed foot from the high-perf tier — the old 144 FPS
  default was 2.4x the byte rate a CPU renderer can drain at
  fullscreen; foot now defaults to 60 FPS with cosmetic effects
  auto-disabled, same as the VTE family.
- **Not affected**: Alacritty, kitty, WezTerm, ghostty (GPU-rendered
  or heavily optimized renderers that sustain the ANSI throughput).

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
   performance — and for the cosmetic effects layer (particles, flash
   waves, anomaly zones), which HUNT-24 auto-disables on CPU-rendered
   terminals. See `docs/TERMINAL_COMPATIBILITY.md` for the full list
   of recommended terminals.
2. **Run cosmostrix in a smaller window** (non-fullscreen) on VTE
   terminals. The lag only manifests at high cell counts (typically
   >5000 cells = ~120x40+).
3. **For benchmarking or performance-critical use**, always use
   Alacritty. The `--benchmark` mode is terminal-independent (headless),
   but interactive fullscreen requires a fast terminal.
4. **Cosmetic effects on a CPU-rendered terminal**: automatic since
   HUNT-24 (no manual action needed) — the effects layer silences
   itself at startup on VTE/konsole/foot/xterm.js/console-TTY. On a
   terminal the env markers miss, the runtime congestion gate
   (`--verbose` shows `[auto-fx]` diagnostics) disables them after 4s
   of sustained output congestion. `--no-effects` remains the explicit
   manual equivalent.

### Status

**Particle stuck/hang FIXED (HUNT-21 + HUNT-22 + HUNT-23 + HUNT-24 + HUNT-25) —
five layers, the last two strategic.** HUNT-23 (round 3) is the
systemic output layer; HUNT-24 (round 4) is the terminal-class gate;
HUNT-25 (round 5) is the resync redraw fix; the first three are
summarized below and detailed in the CHANGELOG.

*Layer 5 (HUNT-25): stop resetting render state at maintenance redraws —
the "glitch rain shift" on ALL terminals.* The owner's post-HUNT-24
report: snow-ice confirmed fixed, but the rain still suddenly shifted
sideways for a few seconds around the first minute ("at certain
minutes, or simply at 57 seconds from start") — including on
GPU-accelerated Alacritty, ruling out the CPU-renderer story. Empirical
PTY audit (90s capture, per-frame size/content analysis): glyph
positions never shift and the forced repaints were content-identical,
but every periodic maintenance redraw (idle resync every 20s, stuck-cell
sweep every 3600 frames, ANSI drift redraw every 18000 frames) called
`frame.clear_with_bg` + wiped the whole `phosphor_base_ch` array —
resetting the phosphor decay state wholesale and emitting a 12-18 frame,
2-3x-sized ANSI burst (3-4.5MB) that terminals cannot drain instantly,
visibly tearing the screen. Fix: resyncs now use the new
`Frame::force_repaint` (sets only the repaint flag — cells, generation,
and phosphor bookkeeping untouched) and `phosphor_decay_pass` prefers
the dirty-index scan on resync frames. Verified: frame sizes uniform
(max 133KB vs 297KB; zero frames above 180KB vs 40+ before).

*Layer 4 (HUNT-24): stop feeding the pipe — the effects auto-gate.*
The owner's post-HUNT-23 report (foot + GNOME/kgx still reproducing
snow-ice sparks, plus a new congestion-stretched "glitch rain"
visual drift) plus an empirical PTY audit (the ANSI stream itself is
cursor-consistent — zero wrap/width violations across 583 KB of
congested output; the app's own screen content shows no horizontal
drift) pinned the remaining reproductions on one fact: a pure-CPU
renderer cannot drain the effects layer's ANSI volume at fullscreen
cell counts, so every congestion symptom regenerates the moment the
layer runs there — no matter how correct its clocks are. The fix is
the owner's own directive: **cosmetic effects (particles, flash
waves, anomaly zones, ghost events, hover glow, CRT vignette) are
now auto-disabled on CPU-rendered and TTY terminals** — detected via
`VTE_VERSION` (GNOME Terminal/kgx/Xfce/Mate), `KONSOLE_VERSION`,
`TERM=foot`/`TERM_PROGRAM=foot`, xterm.js hosts, and `TERM=linux`/
`TERM=dumb` (raw console). foot and konsole were also removed from
the high-perf tier (they are CPU renderers, not 144 FPS terminals —
that misclassification was the foot flood amplifier). A dynamic
congestion gate disables effects mid-session (sticky, 4s sustained
drain backoff) for CPU terminals the env markers cannot see. GPU
terminals and unknown terminals keep effects; `--no-effects`
remains the explicit manual equivalent. Rain-core visuals are
untouched everywhere.

*Layer 3 (HUNT-23): the output-side feedback loop.* The particle
clock was real-time after HUNT-22, yet foot and GNOME/kgx still
reproduced: slow over minutes, stuck for a few seconds,
auto-dismiss. The freeze lived upstream of particle physics —
three interlocking output-side defects:

1. **Open-loop output pacing.** `effective_fps()` responded to
   pause/idle but never to the terminal's drain rate. When the ANSI
   byte rate exceeds what a CPU-rendered terminal drains (VTE at
   60 FPS, foot at its 144 FPS high-perf default), the PTY buffer
   fills and the frame's `flush()` syscall blocks until the terminal
   catches up — the whole event loop freezes with it.
2. **The blocking syscall was untimed.** `last_write_ns` timed only
   the in-memory `write_all` into the 256 KB BufWriter; the
   `BufWriter::flush()` — where the block actually happens — was
   invisible to the power system.
3. **P2 health mitigation bomb.** `EnduranceHealth` scored frame
   work in ABSOLUTE ms (`100 - ms*10`, zero at >= 10ms — an
   Alacritty-class calibration). A healthy busy VTE/foot frame
   (12ms of a 16.7ms budget) was classified "investigate"
   permanently, arming the P2 self-healer every 30s. Its cure,
   `force_draw_everything()`, is the largest possible ANSI burst
   (100-400 KB) — bombed into the already saturated pipe, it
   produced multi-second freezes ("stuck") followed by mass
   particle expiry on drain ("auto-dismiss"). Persistent clicking
   deepened the congestion and stretched frames past the 250ms
   particle anti-teleport cap — bursts lost their velocity in 1-2
   giant decay steps and hung as near-motionless sparks (the
   "snow/sleet" degradation).

The fix closes the loop: the flush syscall is timed
(`flush_stdout_timed`), `PowerManager` converts write-latency
overshoot into a `drain_backoff` that scales `effective_fps` toward
the terminal's sustainable rate (up to 75%, floor 12, gated on
`power-dragon`), the P2 mitigation skips the full-redraw burst under
congestion (madvise kept; the redraw is reserved for
low-pressure/genuinely-unhealthy runs), and the health frame signal
is now utilization (`work_s / frame_period_s`, floored at 40) so
slow-but-keeping-up terminals score healthy. The HUD `tgt:` line
shows a `drain` suffix while the backoff is engaged. On a
saturated terminal the visible behavior is: cadence settles at what
the terminal can drain, blocked-write stalls shrink to pipe transit
time, and the 30s stuck-then-clear cycle is gone.

*Layer 1 (HUNT-21): motion/aging unification.* Particle aging was
real-time (`now - birth`) while motion used the clamped frame dt —
particles expired before finishing their trajectory. `sim_age` now
accumulates the same dt that drives motion. Correct but
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

## All platforms: redirecting interactive output produces an ANSI frame dump

### Symptom

Running `cosmostrix > file` (or `nohup cosmostrix &`, which redirects
stdout to `nohup.out`) writes megabytes of raw ANSI escape-sequence
frames into the file: no line terminators, one giant "line", full
escape-sequence soup. The run burns a CPU core for roughly 30-40 s
(until the periodic stdout-health probe ends it gracefully) and the
user sees no rain — the frames went to the file.

### Hazard

The dump file is dangerous to inspect casually: `cat`-ing it into a
live terminal replays RIS/DECSET sequences, cursor addressing, and
alternate-screen toggles, which can clear the screen, resize the
viewport, or recolor the session. If you must inspect a dump, use
`less -R` on a copy, or simply delete it.

### Workaround

Do not pipe or redirect the interactive mode. Use `--benchmark` for
pipeline-friendly plain-text measurement, `--doctor` / `--dump-config`
/ `--docs` for text reports, or `-v 2> verbose.log` to capture verbose
stderr while watching the rain. A stderr warning at frame zero names
these alternatives whenever interactive mode starts with a non-tty
stdout. Full catalog: `docs/USAGE_PIPE_REDIRECT.md`
(NIGHT-hunter-6, 2026-09-05).

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
