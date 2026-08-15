<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Terminal Compatibility

Cosmostrix is a terminal renderer. It depends on common terminal features, but
it keeps recovery paths conservative and explicit.

## Supported Features

- ANSI escape sequences for cursor movement, colors, and style reset.
- Alternate screen while the renderer is active.
- Raw mode while the renderer is active.
- 24-bit truecolor when the terminal advertises it.
- 256-color fallback for terminals such as `xterm-256color`.
- 16-color and mono fallback for minimal terminals.
- Bracketed paste cleanup after interrupted sessions.
- Mouse reporting always on (cursor glow + click wave effects; also
  blocks text selection in all modes).

## Terminal Matrix

| Terminal | Expected result | Notes |
| --- | --- | --- |
| Alacritty | Excellent | Truecolor is expected. `color-bg = default-background` follows Alacritty's configured background and opacity. |
| Konsole | Excellent | Truecolor is expected on modern Konsole. **Known issue:** `Super+C` (Windows-key + c) still cycles color — Konsole's kitty keyboard encoder does not set the `SUPER` bit. See [Known Issues](#known-issues) below. |
| Kitty | Excellent | Truecolor and Unicode rendering are expected. |
| Ghostty | Excellent | Truecolor and Unicode rendering are expected. |
| GNOME Terminal | Good | Truecolor usually works through VTE-based detection. |
| Windows Terminal / PowerShell | Good | `--reset-terminal` is best-effort; user confirmation on Windows builds is still useful. |
| tmux | Good with config | The outer terminal and tmux must both support RGB for truecolor. **Known issue:** `Super+C` (Windows-key + c) still cycles color inside tmux — tmux translates kitty protocol back to legacy escape sequences, dropping the `SUPER` bit. See [Known Issues](#known-issues) below. |
| SSH | Depends on remote env | Forward `TERM`/`COLORTERM` carefully; remote font and locale also matter. |
| Linux console / minimal TTY | Basic | Use `--colormode 256` or `--charset minimal` if colors or glyphs look wrong. Synchronized output (mode 2026) is disabled because vt.c does not understand it. **Known issue:** scrollback is not preserved on `q` quit — see [Known Issues](#known-issues) below. |
| VSCode integrated terminal | Capped (degraded) | Auto-detected via `TERM_PROGRAM=vscode`. Tier 2 defenses apply (FPS cap 30, sync disabled, byte-budget backpressure, periodic RIS reset) but residual lag/stutter is unavoidable — see [Known Issues](#known-issues) below. Override with `--fps 15` for even lower throughput. See `docs/SECURITY_AUDIT.md` §12 for the full crash analysis. |
| Hyper | Capped (degraded) | Auto-detected via `TERM_PROGRAM=Hyper`. Same Tier 2 defenses as VSCode (Hyper embeds xterm.js). Same residual lag/stutter — see [Known Issues](#known-issues) below. |
| WaveTerminal | Capped (degraded) | Auto-detected via `TERM_PROGRAM=WaveTerminal`. Same Tier 2 defenses as VSCode (WaveTerminal embeds xterm.js). Same residual lag/stutter — see [Known Issues](#known-issues) below. |
| Tabby | Capped (degraded) | Auto-detected via `TERM_PROGRAM=Tabby`. Same Tier 2 defenses as VSCode (Tabby embeds xterm.js). Same residual lag/stutter — see [Known Issues](#known-issues) below. |
| WarpTerminal | Capped (degraded) | Auto-detected via `TERM_PROGRAM=WarpTerminal`. Same Tier 2 defenses as VSCode (Warp's renderer pane is xterm.js). Same residual lag/stutter — see [Known Issues](#known-issues) below. |

## Background Behavior

| Setting | What Cosmostrix does | What it does not do |
| --- | --- | --- |
| `color-bg = default-background` (default) | Does not paint a solid background; it follows the terminal emulator background. | It does not change terminal emulator opacity. |
| `color-bg = black` | Paints a solid black background. | Does not use terminal transparency. |

## Reset Behavior

Normal exit is non-destructive. Quit with `q` or duration end and Cosmostrix
restores modes/styles without clearing your visible shell history. Only `q`
quits — Esc, Ctrl-C, and all other unrecognized keys are silently ignored
(prevent accidental exit). Mouse click does NOT exit (v17: removed for
consistency with the only-q-quits policy).

`--reset-terminal` is explicit destructive recovery. It resets styles, shows the
cursor, leaves the alternate screen, disables mouse/focus/bracketed-paste modes,
clears the visible screen, moves the cursor home, and attempts scrollback purge
when the terminal supports it.

Windows Terminal and PowerShell support is best-effort. If a Windows terminal
does not clear exactly as expected, report the terminal app, shell, Windows
version, and Cosmostrix build.

## Recommended Commands

```bash
cosmostrix --doctor
cosmostrix --reset-terminal
cosmostrix --color-bg default-background
cosmostrix --colormode 256
cosmostrix --charset minimal
```

PowerShell:

```powershell
.\cosmostrix.exe --doctor
.\cosmostrix.exe --reset-terminal
```

## Troubleshooting

### Colors Look Wrong

Run:

```bash
cosmostrix --doctor
```

If `TERM=xterm-256color` and `COLORTERM` is unset, 256-color output is expected.
Set `COLORTERM=truecolor` only if your terminal really supports truecolor.

Inside tmux or screen, the outer terminal and multiplexer config must both
support RGB. If in doubt, compare outside tmux first.

### Background Is Not Transparent

Use:

```bash
cosmostrix --color-bg default-background
```

`default-background` mode follows the terminal emulator background. It does not
change terminal emulator opacity. Configure opacity in the terminal emulator itself.

### Terminal Left Weird After Kill

Use the explicit recovery command:

```bash
cosmostrix --reset-terminal
```

Normal exit is non-destructive; `--reset-terminal` is the explicit recovery path
that clears visible screen state and attempts scrollback purge.

### Glyphs Appear As Boxes

Use a UTF-8 locale and a font with the selected glyph coverage. For safer output:

```bash
cosmostrix --charset minimal
```

### tmux Truecolor Issue

Run `cosmostrix --doctor` inside and outside tmux. If outside looks correct but
inside does not, adjust tmux truecolor settings and verify the outer terminal
also supports truecolor.

### SSH Or Headless Usage

For SSH, make sure remote `TERM`, `COLORTERM`, locale, and font expectations
match the local terminal. For headless environments, prefer:

```bash
cosmostrix --benchmark
cosmostrix --doctor
```

## Known Issues

### TTY scrollback is not preserved on quit (won't fix)

**Symptom:** On a real Linux virtual console (TTY, reached via
`ctrl+alt+fN`, `TERM=linux`), terminal history emitted before cosmostrix
started — for example the output of `echo hello` — is gone from the
scrollback buffer after quitting cosmostrix with `q`. The visible
screen clears immediately on exit.

**Scope:** This issue is specific to the kernel Linux virtual console
(vt.c). Graphical terminal emulators (Alacritty, Kitty, Ghostty,
Konsole, GNOME Terminal, xterm, etc.) do NOT exhibit this issue —
scrollback is preserved on `q` quit there. The issue is also absent
when cosmostrix runs inside `tmux` or `screen` on a TTY (the
multiplexer maintains its own scrollback buffer that survives).

**Status:** Won't fix. Multiple fix attempts across seven commits
(`6d0574b`, `8b2a19b`, `42c76a8`, `246e9b9e`, `6ed244b`, `01ffda8`,
`2b9be7e`) targeted this issue from different angles (alt-screen
detection, `Clear(All)` removal, sync-mode placement) without producing
a working result on the actual TTY. The remaining hypothesis is that
the Linux vt.c scrollback ring buffer is shared between the primary
and alternate screen buffers, so any frame content that triggers a
scroll event pollutes the shared scrollback and pushes out prior
history. Without kernel-side changes to vt.c, no userspace sequence
combination can reliably preserve scrollback across a full-screen
rendering session on the raw Linux console.

**Confirmation that this is a vt.c limitation, not a cosmostrix
regression:**

1. **Not a v50 regression** — the owner reproduced the same scrollback
   loss on cosmostrix v15 stable and earlier. The bug has existed for
   as long as cosmostrix has rendered full-screen on the Linux console.
2. **Independent of TERM** — the owner tested with both `TERM=linux`
   (vt.c native, sync_output off, alt screen on after `2b9be7e`) and
   the inherited `TERM=xterm-direct` (sync_output on). Both produce
   the same scrollback loss on the same TTY. This rules out any
   TERM-based detection tweak as a fix.
3. **Independent of sync mode** — `TERM=linux` correctly disables
   mode 2026 sequences, yet the bug persists. So the loss is not
   caused by vt.c misparsing mode 2026.
4. **Multiplexer decouples** — running cosmostrix inside `tmux` on
   the same TTY preserves scrollback (`ctrl+b [ PgUp` shows prior
   `echo hello` output). tmux's scrollback is fully decoupled from
   vt.c, confirming the loss source is the kernel vt.c scrollback
   ring buffer itself.

**Workarounds:** Pick whichever fits your workflow.

1. **Run inside `tmux`** — tmux maintains its own scrollback buffer
   that is fully decoupled from the kernel vt.c scrollback. Quit
   cosmostrix with `q`, then scroll up inside tmux
   (`ctrl+b` then `[` then `PgUp`); your prior `echo hello` output
   is preserved in the tmux pane's history.

   ```bash
   tmux new -s rain    # start a tmux session
   echo hello          # this output is now in tmux scrollback
   cosmostrix          # run the renderer
   # press q to quit
   # ctrl+b [ then PgUp to scroll — `hello` is still there
   ```

2. **Run inside `screen`** — same principle as tmux; screen maintains
   its own scrollback (`ctrl+a` then `Esc` to enter copy/scroll mode).

3. **Use a graphical terminal emulator** — if a GUI session is
   available, run cosmostrix in Alacritty, Kitty, Ghostty, Konsole,
   GNOME Terminal, or xterm. None of these exhibit the scrollback
   loss; `echo hello` stays in scrollback after `q`.

4. **Log to a file with `tee`** — if preserving specific output is the
   goal, pipe it through `tee` before launching cosmostrix:

   ```bash
   echo hello | tee /tmp/before-rain.log
   cosmostrix
   # after q:
   cat /tmp/before-rain.log
   ```

The issue is documented here so future maintainers do not re-attempt
the same fix paths. If you believe you have a new angle (for example a
vt.c-specific escape sequence that bounds the scroll region to the
visible viewport only), open a discussion before opening a PR.

### Super+C still cycles color in tmux and Konsole (won't fix)

**Symptom:** Pressing `Super+C` (the Windows-logo key plus `c`) still
cycles the color scheme forward, even though commit `c1aa101` enabled
the kitty keyboard protocol and commit `94d0c88` added an allowlist
modifier guard that rejects `KeyModifiers::SUPER`. Bare `c` and
`Shift+C` continue to work as designed (forward / reverse cycle).

**Scope:** Reported by the owner on:

- **tmux** (any version) — running cosmostrix inside a tmux session,
  regardless of the outer terminal. The outer terminal may itself
  report `SUPER` correctly when cosmostrix runs directly (Alacritty,
  Hyper, Kitty, WezTerm, Ghostty, foot all confirmed working by the
  owner), but the moment cosmostrix is launched inside `tmux`, the
  `SUPER` bit is lost and `Super+C` arrives as a bare `Char('c')` with
  `KeyModifiers::NONE`.
- **Konsole** (any version tested, including 22.04+) — running
  cosmostrix directly inside Konsole. The kitty keyboard protocol is
  correctly enabled (the `KONSOLE_VERSION` env-var detection fires and
  `PushKeyboardEnhancementFlags` is sent), but Konsole's CSI-u encoder
  does not populate the `SUPER` modifier bitfield for the Windows-key
  combo, so crossterm decodes the event as `KeyModifiers::NONE`.

Confirmed working (Super+C correctly blocked) by the owner:

- Alacritty, Hyper, Kitty, WezTerm, Ghostty, foot.

**Status:** Won't fix. The allowlist guard (`is_unmodified_or_shift`)
is correct and complete on cosmostrix's side — the `SUPER` bit is
rejected the moment crossterm exposes it. The failure is downstream of
cosmostrix: the terminal multiplexer (tmux) or terminal emulator
(Konsole) strips the `SUPER` modifier bit before the event reaches
crossterm's decoder. No amount of cosmostrix-side code can recover a
bit that was never delivered. The two layers that would need to change:

1. **tmux** — its `extended-keys` feature forwards the kitty keyboard
   protocol between the outer terminal and tmux, but when re-emitting
   key events to the application inside the pane, tmux translates back
   to legacy escape sequences in the default `extended-keys format`
   setting. Legacy sequences can only encode `SHIFT | ALT | CONTROL`,
   so `SUPER | HYPER | META` are silently dropped. The fix would be
   `tmux` setting `extended-keys format all` (or equivalent) and
   emitting CSI-u to applications — outside cosmostrix's control.
2. **Konsole** — its kitty keyboard protocol implementation reports
   `SHIFT | ALT | CONTROL` but does not set the `SUPER` bit for the
   Windows-key combo. This is a Konsole-side bug; the fix belongs
   upstream in Konsole's input encoder.

**Workarounds:**

1. **Run cosmostrix directly in a reporting terminal** — launch
   cosmostrix inside Alacritty, Hyper, Kitty, WezTerm, Ghostty, or
   foot (not inside tmux/screen, not inside Konsole). `Super+C` is
   correctly blocked in these.
2. **Avoid the Super-key shortcut entirely** — use bare `c` (forward
   cycle) or `Shift+C` (reverse cycle) instead. Both work in every
   terminal, including tmux and Konsole.
3. **If you must use tmux**, configure `extended-keys format all` in
   `~/.tmux.conf` and verify with `tmux -V` >= 3.3. This is untested
   by the cosmostrix owner; even with the setting, tmux may still
   translate to legacy encoding for compatibility reasons.

The issue is documented here so future maintainers do not re-attempt
the same fix path on the cosmostrix side. The allowlist modifier guard
and kitty keyboard protocol push are both correct and complete; any
further work belongs upstream in tmux and Konsole. If a future tmux or
Konsole release fixes `SUPER` bit forwarding, no cosmostrix change is
needed — the existing guard will start rejecting `Super+C` automatically.

### Electron / xterm.js terminals lag and stutter (residual, won't fix)

**Symptom:** Running cosmostrix inside any Electron-based terminal —
VSCode integrated terminal, Hyper, WaveTerminal, Tabby, WarpTerminal —
produces a visibly degraded experience: frame stutter, intermittent
input lag (key presses take 100-500 ms to register), mouse-glow
effects that visibly trail the cursor, and occasional multi-second
freezes during long-running sessions. The renderer never crashes
(Tier 2 mitigations prevent the SIGTRAP/OOM that used to occur), but
the visual quality is far below what cosmostrix delivers in a native
terminal.

**Scope:** Every Electron-based terminal that ships xterm.js as its
renderer:

- VSCode integrated terminal (`TERM_PROGRAM=vscode`)
- Hyper (`TERM_PROGRAM=Hyper`)
- WaveTerminal (`TERM_PROGRAM=WaveTerminal`)
- Tabby (`TERM_PROGRAM=Tabby`)
- WarpTerminal (`TERM_PROGRAM=WarpTerminal`)

Native terminals — Alacritty, Kitty, Ghostty, foot, WezTerm, Konsole,
GNOME Terminal, xterm — are unaffected. They process the same byte
stream 5-20× faster because they run a native ANSI parser in the
terminal process itself, not a JavaScript parser in a V8 isolate
inside Chromium.

**What cosmostrix already does (Tier 2 defenses, in place since
SECURITY_AUDIT §12a):**

1. **FPS cap at 30** (vs 240 on native terminals) — caps the
   instantaneous byte rate at ~7 MB/sec worst case.
2. **Synchronized output disabled** — xterm.js's mode 2026 buffer
   implementation amplifies memory pressure, so mode 2026 is never
   pushed on xterm.js hosts.
3. **Rolling byte-budget backpressure** — a 600-frame (20 s at 30 FPS)
   sliding window tracks cumulative bytes; when the window exceeds
   40 MB, the next flush is suppressed entirely (the rain animation
   keeps advancing internally, only the ANSI write is skipped).
4. **Periodic RIS reset** — every ~50 MB of cumulative output, cosmostrix
   emits `ESC c` (Reset to Initial State) to force xterm.js to flush
   its in-memory scrollback buffer, preventing the multi-hour V8 OOM
   that originally crashed these hosts (SIGTRAP, coredump 2026-08-04).
5. **Hard ceiling** at 200 MB as a belt-and-suspenders last resort.

**Why the residual lag is unavoidable (won't fix on cosmostrix side):**

The fundamental bottleneck is architectural, not algorithmic. In a
native terminal, ANSI bytes flow:

```
cosmostrix → write(2) → kernel PTY → terminal's native ANSI parser → GPU
```

In an Electron/xterm.js host, the same bytes flow:

```
cosmostrix → write(2) → kernel PTY → node-pty (Node.js) → IPC to
renderer process → xterm.js (V8/JavaScript) → Canvas/WebGL → Chromium
compositor → GPU
```

That extra hop through V8 introduces three cosmostrix-cannot-fix
sources of stutter:

1. **V8 garbage collection pauses** — xterm.js's buffer and the
   node-pty pump both allocate JavaScript objects per frame. V8's
   incremental GC pauses typically run 5-50 ms, but major GCs on a
   long-running pane can stall 100-300 ms. Each pause drops 3-9
   frames at 30 FPS, visible as a hitch. No cosmostrix-side fix can
   suppress V8's GC scheduler.
2. **Chromium compositor scheduling** — even when xterm.js's canvas
   paint is fast, Chromium composites the canvas with the rest of the
   Electron window (sidebar, tabs, devtools) on its own vsync
   schedule. Frames that miss the compositor's deadline get dropped,
   producing visible stutter on heavy scenes. cosmostrix has no
   control over Chromium's compositor.
3. **node-pty backpressure is cooperative, not enforced** — node-pty
   reads from the PTY in a JavaScript callback; if the renderer is
   busy, the read is delayed. The kernel PTY buffer (typically
   256 KB on Linux) fills, then `write(2)` in cosmostrix blocks.
   cosmostrix's write-latency backpressure detects this and downgrades
   the scene, but the downgrade itself is visible as a quality drop —
   fewer glyphs, smaller burst, dimmer colors. The user perceives
   this as "the rain looks worse inside VSCode."

**Practical impact on the experience:**

- **Short sessions (< 5 min)**: Usually fine. The V8 heap hasn't grown
  enough for major GCs; the buffer is small; cosmostrix's 30 FPS cap
  is enough to keep up. Mild stutter is visible on dense scenes but
  tolerable.
- **Medium sessions (5-30 min)**: Noticeable stutter every 30-90 s as
  V8 incremental GCs fire. Mouse glow trails become visible. Color
  cycling may feel sluggish (50-200 ms between keypress and visible
  change).
- **Long sessions (> 30 min)**: Major GC pauses (100-300 ms) hit
  every few minutes. RIS resets fire every ~7 s of sustained output,
  producing a brief full-screen flicker. The byte-budget backpressure
  may suppress 5-15% of flushes during heavy scenes, causing the rain
  to visibly drop frames. The renderer does not crash (Tier 2 prevents
  it), but the experience is poor.

**Status:** Won't fix on the cosmostrix side. The Tier 2 defenses
are complete and correctly sized for the 30 FPS cap; they prevent
the crash, but they cannot paper over the architectural cost of
running an ANSI parser inside V8. The remaining work belongs to the
Electron / xterm.js / Chromium stack:

- xterm.js could ship a WebAssembly ANSI parser to escape V8 GC.
- Electron could expose a direct PTY-to-canvas pipe that bypasses
  node-pty's JavaScript pump.
- Chromium could offer a "low-latency compositor mode" that doesn't
  drop frames on missed deadlines.

None of these are cosmostrix's to build.

**Workarounds:**

1. **Use a native terminal for serious viewing** — Alacritty, Kitty,
   Ghostty, foot, WezTerm, Konsole all deliver 5-20× the throughput
   at 1/10th the latency. If you're running cosmostrix to enjoy the
   animation, do it in a native terminal.
2. **Lower the FPS cap further** — `cosmostrix --fps 15` halves the
   byte rate and gives xterm.js more headroom. Useful for long-running
   sessions inside VSCode where stability matters more than smoothness.
3. **Keep sessions short inside Electron hosts** — under 5 minutes,
   the experience is acceptable. Past 30 minutes, expect visible
   degradation.
4. **Reduce the screen size** — `cosmostrix --screen-size 80x24`
   caps the column/row count, shrinking every frame's byte cost.
   Useful when you must run inside an Electron host and want to keep
   the byte budget well under the 40 MB backpressure threshold.
5. **Close other VSCode panes** — Electron's renderer process shares
   one V8 isolate across all panes in a window. Heavy operations in
   another pane (debugger, language server, search) steal V8 time
   from xterm.js and amplify stutter. Closing unrelated panes helps.

**Note on mouse glow:** the cursor-follow glow and click-wave effects
are always on (the `--n` flag was removed in v17 — see
`src/verbose.rs`). There is no CLI toggle to disable them, so they
contribute to the per-frame byte cost on every terminal, including
Electron hosts. If a future cosmostrix release reintroduces a
mouse-effect toggle, it would be a meaningful lever for reducing
xterm.js pressure.

**Verification:** Tier 2 defenses are covered by the test suite
(`tier2::tests::*`, `termdetect::tests::xtermjs_host_*`). The
residual lag is a measured architectural property, not a bug —
benchmark numbers are in `docs/BENCHMARKING.md`. If you observe a
sudden *regression* (e.g., a frame rate that previously held at 30
FPS drops to 5 FPS), that's worth investigating as a cosmostrix-side
issue; file it with `--perf-stats` output attached.

The issue is documented here so future maintainers do not re-attempt
the same mitigation paths. The Tier 2 defenses are correctly sized
and complete; further work belongs upstream in xterm.js, Electron,
and Chromium. If a future xterm.js release ships a WASM parser or
Electron exposes a direct PTY-to-canvas pipe, the FPS cap can be
raised and the byte-budget backpressure can be relaxed — but the
detection logic and defense layers should remain in place as a
safety net.
