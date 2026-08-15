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
| Konsole | Excellent | Truecolor is expected on modern Konsole. |
| Kitty | Excellent | Truecolor and Unicode rendering are expected. |
| Ghostty | Excellent | Truecolor and Unicode rendering are expected. |
| GNOME Terminal | Good | Truecolor usually works through VTE-based detection. |
| Windows Terminal / PowerShell | Good | `--reset-terminal` is best-effort; user confirmation on Windows builds is still useful. |
| tmux | Good with config | The outer terminal and tmux must both support RGB for truecolor. |
| SSH | Depends on remote env | Forward `TERM`/`COLORTERM` carefully; remote font and locale also matter. |
| Linux console / minimal TTY | Basic | Use `--colormode 256` or `--charset minimal` if colors or glyphs look wrong. Synchronized output (mode 2026) is disabled because vt.c does not understand it. **Known issue:** scrollback is not preserved on `q` quit — see [Known Issues](#known-issues) below. |
| VSCode integrated terminal | Good (capped) | Auto-detected via `TERM_PROGRAM=vscode`. Tier 2 defenses apply: (1) synchronized output (mode 2026) disabled because xterm.js's buffer implementation amplifies memory pressure; (2) FPS capped at 30 to keep the worst-case byte rate under ~7 MB/sec; (3) byte-budget backpressure suppresses flushes when the rolling window exceeds 40 MB; (4) periodic RIS reset (ESC c) every ~50 MB clears xterm.js's scrollback buffer to prevent the multi-hour V8 OOM (SIGTRAP) crash. Override with `--fps 15` for even lower throughput. See `docs/SECURITY_AUDIT.md` §12 for the full crash analysis. |
| Hyper | Good (capped) | Auto-detected via `TERM_PROGRAM=Hyper`. Same Tier 2 defenses as VSCode (Hyper embeds xterm.js as its terminal renderer). |
| WaveTerminal | Good (capped) | Auto-detected via `TERM_PROGRAM=WaveTerminal`. Same Tier 2 defenses as VSCode (WaveTerminal embeds xterm.js in its tiling panes). |
| Tabby | Good (capped) | Auto-detected via `TERM_PROGRAM=Tabby`. Same Tier 2 defenses as VSCode (Tabby embeds xterm.js for its terminal pane). |
| WarpTerminal | Good (capped) | Auto-detected via `TERM_PROGRAM=WarpTerminal`. Same Tier 2 defenses as VSCode (Warp's renderer pane is xterm.js). |

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
