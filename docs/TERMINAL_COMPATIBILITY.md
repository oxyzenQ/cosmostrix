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
- Mouse mode only when `--mouse` is enabled.

## Terminal Matrix

| Terminal | Expected result | Notes |
| --- | --- | --- |
| Alacritty | Excellent | Truecolor is expected. `color-bg = transparent` follows Alacritty's configured background and opacity. |
| Konsole | Excellent | Truecolor is expected on modern Konsole. |
| Kitty | Excellent | Truecolor and Unicode rendering are expected. |
| Ghostty | Excellent | Truecolor and Unicode rendering are expected. |
| GNOME Terminal | Good | Truecolor usually works through VTE-based detection. |
| Windows Terminal / PowerShell | Good | `--reset-terminal` is best-effort; user confirmation on Windows builds is still useful. |
| tmux | Good with config | The outer terminal and tmux must both support RGB for truecolor. |
| SSH | Depends on remote env | Forward `TERM`/`COLORTERM` carefully; remote font and locale also matter. |
| Linux console / minimal TTY | Basic | Use `--colormode 256` or `--charset minimal` if colors or glyphs look wrong. |

## Background Behavior

| Setting | What Cosmostrix does | What it does not do |
| --- | --- | --- |
| `color-bg = black` | Paints a solid black background. | Does not use terminal transparency. |
| `color-bg = transparent` | Does not paint a solid background; it follows the terminal emulator background. | It does not change terminal emulator opacity. |
| `color-bg = default-background` | Uses the terminal default background color. | Does not force transparency or black. |

## Reset Behavior

Normal exit is non-destructive. Quit with `q`, `Esc`, Ctrl-C, or duration end
and Cosmostrix restores modes/styles without clearing your visible shell
history.

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
cosmostrix --color-bg transparent
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
cosmostrix --color-bg transparent
```

Transparent mode follows the terminal emulator background. It does not change
terminal emulator opacity. Configure opacity in the terminal emulator itself.

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
cosmostrix --info
cosmostrix --doctor
```
