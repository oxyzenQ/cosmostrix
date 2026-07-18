<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Deep Dragon Architecture

Cosmostrix v15 "The Dragon" is built on a **defense-in-depth** philosophy.
Every critical path has multiple recovery layers, so if one fails, the next
catches it. This document maps the Dragon's anatomy to the actual codebase
layers and explains how the "deep" structure provides world-class reliability.

## The 7-Layer Dragon

```
┌─────────────────────────────────────────────────────────────────┐
│  Layer 7: Dragon Incubator (src/dragon/)                        │
│  Experimental features, future v15+ subsystems                  │
│  → Never touches stable engine internals                         │
├─────────────────────────────────────────────────────────────────┤
│  Layer 6: CLI + Helpers (src/cli.rs, output.rs, help_detail.rs) │
│  Argument parsing, color output, help text, version info        │
│  → Strict whitelist security on all file ops                    │
├─────────────────────────────────────────────────────────────────┤
│  Layer 5: Config System (src/config.rs, config_apply.rs,        │
│           configfile.rs, testconf.rs, safepath.rs)              │
│  TOML config parsing, validation, live reload                   │
│  → Strict validation: invalid = exit 2, no silent fallback      │
├─────────────────────────────────────────────────────────────────┤
│  Layer 4: Interactive Engine (src/interactive/)                 │
│  Event loop, HUD, input, watchdog, adaptive pacing              │
│  → Only 'q' quits (consistent policy, no accidental exit)       │
├─────────────────────────────────────────────────────────────────┤
│  Layer 3: Atmosphere Engine (src/atmosphere_*.rs)               │
│  Adaptive time-driven modulation, custom time mapping            │
│  → Disabled by default (opt-in via config)                      │
├─────────────────────────────────────────────────────────────────┤
│  Layer 2: Cloud Renderer (src/cloud/)                           │
│  Droplet lifecycle, phosphor, monolith, scene runtime           │
│  → Zero-allocation hot path (dirty_map Vec<u8>, phosphor reuse) │
├─────────────────────────────────────────────────────────────────┤
│  Layer 1: Terminal I/O (src/terminal.rs, frame.rs, color_cache) │
│  ANSI escape sequencing, diff-based rendering, sync output      │
│  → 5-layer --reset-terminal recovery for SIGKILL survival       │
└─────────────────────────────────────────────────────────────────┘
```

## Defense-in-Depth: Terminal Recovery

The `--reset-terminal` feature is the exemplar of the Dragon's defense-in-depth
philosophy. When `kill -9` (SIGKILL) hits cosmostrix, the process dies instantly
— no cleanup runs. The terminal is left in a broken state:

- Alternate screen still active (rain visible, shell hidden)
- Raw mode still on (no echo, can't type commands)
- Mouse reporting still on (clicks do weird things)
- Cursor hidden
- Synchronized output stuck (terminal buffers output)

### 5-Layer Recovery

`--reset-terminal` runs 5 independent recovery layers, each best-effort:

| Layer | What | Why |
|-------|------|-----|
| 1. ANSI restore | `\x1b[?2026l ?1000l ?1002l ?1003l ?1006l ?1015l ?2004l ?1004l ?1049l ?25h` | Disables all optional terminal modes cosmostrix may have enabled |
| 2. ANSI reset | `\x1b[H \x1b[2J \x1b[3J \x1b[H` | Clears visible screen + scrollback + cursor home |
| 3. crossterm | LeaveAlternateScreen, Clear, Show, EnableLineWrap | Redundant with ANSI but belt-and-suspenders |
| 4. stty sane | External `stty sane` command | Restores kernel termios (raw mode off, echo on) — ANSI can't fix this |
| 5. reset + tput | External `reset` + `tput reset` | Full terminal reset utility, clears everything |

### Why 5 Layers?

- **Layer 1-2** (ANSI): Fast, always available, but only works if the
  terminal is still processing escape sequences. If the terminal is
  stuck in a broken state, ANSI might be ignored.
- **Layer 3** (crossterm): Same as ANSI but via a library — redundant
  but ensures the sequences are well-formed.
- **Layer 4** (stty): Critical — raw mode is a kernel termios setting,
  not an ANSI mode. ANSI escapes CANNOT fix raw mode. `stty sane`
  restores the kernel's terminal line discipline.
- **Layer 5** (reset/tput): Nuclear option — these external utilities
  send the terminal's full init sequence from the terminfo database.
  May not exist on all systems (embedded, minimal containers), so
  failure is silently ignored.

### What's New in v15

The restore/reset sequences were expanded with 4 additional mode resets:

| Mode | Escape | Why |
|------|--------|-----|
| Synchronized output | `\x1b[?2026l` | If cosmostrix was killed mid-sync, the terminal buffers output forever |
| Scroll region | `\x1b[r` | If cosmostrix set a scroll region, it persists after SIGKILL |
| Character set | `\x1b(B` | Reset to US ASCII (in case cosmostrix changed it for box drawing) |
| Auto-wrap | `\x1b[?7h` | Enable auto-wrap (in case it was disabled) |

## Defense-in-Depth: Process Lifecycle

The Dragon survives every kill signal through a 3-layer process guard:

```
                    ┌──────────────────────────────┐
                    │  User presses Ctrl+C (key)   │
                    │  → ignored (only q quits)     │
                    └──────────────────────────────┘
                                  │
                    ┌─────────────▼────────────────┐
                    │  User presses 'q'             │
                    │  → Terminal::drop() cleanup   │
                    │  → Layer 1-3 ANSI restore     │
                    └──────────────────────────────┘
                                  │
                    ┌─────────────▼────────────────┐
                    │  pkill -TERM cosmostrix       │
                    │  → Signal handler sets        │
                    │    GRACEFUL_SHUTDOWN          │
                    │  → Main loop exits cleanly    │
                    │  → Terminal::drop() cleanup   │
                    │  → Signal-exit viewport clear │
                    └──────────────────────────────┘
                                  │
                    ┌─────────────▼────────────────┐
                    │  kill -9 cosmostrix (SIGKILL) │
                    │  → Process dies instantly     │
                    │  → No cleanup runs            │
                    │  → Fork guard child detects   │
                    │    getppid()==1               │
                    │  → Child restores termios     │
                    │  → Child runs restore_terminal │
                    │  → best_effort()              │
                    └──────────────────────────────┘
                                  │
                    ┌─────────────▼────────────────┐
                    │  Terminal still broken?       │
                    │  → cosmostrix --reset-terminal│
                    │  → 5-layer nuclear recovery   │
                    └──────────────────────────────┘
```

### Layer 1: Clean Exit (q or SIGTERM)

- `q` key → `cloud.raining = false` → main loop exits → `Terminal::drop()`
- SIGTERM → signal handler → `GRACEFUL_SHUTDOWN` atomic → main loop exits → `Terminal::drop()`
- `Terminal::drop()` runs the full restore sequence (Layer 1-3 ANSI + crossterm)
- Signal-exit viewport clear: clears the visible screen BEFORE leaving
  alternate screen, so rain residue doesn't flash on the main screen

### Layer 2: Fork Guard (SIGKILL, Linux-only)

- Before the main loop, cosmostrix forks a child process
- Child saves the original `termios` state via `tcgetattr`
- Child blocks SIGTERM, sets `PR_SET_PDEATHSIG(SIGTERM)`
- Child calls `sigwait(SIGTERM)` — sleeps until parent dies
- When parent dies (ANY reason, including SIGKILL), kernel sends
  SIGTERM to child via `PR_SET_PDEATHSIG`
- Child checks `getppid() == 1` (parent reparented to init = dead)
- If parent dead: child restores `termios` via `tcsetattr` + runs
  `restore_terminal_best_effort()`
- Child calls `_exit(0)` (not `exit()` — avoids atexit handlers)

This is Linux-only because it requires `fork()` + `PR_SET_PDEATHSIG`.
macOS and Windows don't have this guard — they rely on Layer 3.

### Layer 3: Manual Recovery (--reset-terminal)

- If both Layer 1 and Layer 2 fail (or on macOS/Windows), the user
  runs `cosmostrix --reset-terminal`
- This runs the full 5-layer recovery sequence described above
- It's the "nuclear option" — clears screen + scrollback, but
  guarantees a clean terminal state

## Defense-in-Depth: Config Security

All file-reading/writing CLI flags enforce the same strict whitelist:

```
┌─────────────────────────────────────────────────────────┐
│  --config <path>          → whitelist + .toml extension │
│  --charset-file <path>    → whitelist (any extension)   │
│  --dump-config <path>     → whitelist + .toml extension │
│  --dump-config (no arg)   → stdout only, redirect blocked│
└─────────────────────────────────────────────────────────┘
```

### Whitelist (strict, no exceptions)

- `~/.config/cosmostrix/` (Linux/macOS, user config)
- `/etc/cosmostrix/` (Linux/macOS, system-wide)
- `%APPDATA%\cosmostrix\` (Windows, user config)
- `%ProgramData%\cosmostrix\` (Windows, system-wide)

### Rejected (everything else)

- Current directory (`.`)
- `/tmp/`
- Home root (`~`)
- `~/.local/`, `/usr/`, `/opt/`, `/var/`
- All relative paths
- All other absolute paths
- Shell redirection (`--dump-config > file`) — blocked via fstat detection

## Defense-in-Depth: Color Capability

The CLI color system degrades gracefully across 4 tiers:

```
TrueColor (24-bit)  →  #A855F7 exact RGB (modern terminals)
Color256 (8-bit)    →  index 135 closest palette match (older xterm)
Color16 (4-bit)     →  ANSI Magenta (legacy terminals)
Mono                →  plain text (NO_COLOR, dumb, piped)
```

Detection follows the de-facto standard (NO_COLOR, CLICOLOR, CLICOLOR_FORCE,
COLORTERM, TERM) used by `bat`, `fd`, `ripgrep`, and `cargo`.

## Defense-in-Depth: Screensaver Policy

Consistent with the "only q quits" policy:

| Key | Normal mode | Screensaver mode |
|-----|-------------|------------------|
| q | Quit | Quit |
| c/C/s/S/x/X/g/a/p/m | Interactive control | Interactive control |
| i/I/h/H | HUD toggle | HUD toggle |
| Space/Up/Down/0-9 | Interactive control | Interactive control |
| B/b/z/F1/Home/Esc/Ctrl+C | Silently ignored | Silently ignored |
| Mouse click (with --mouse) | Click effect | Exit screensaver |

No key causes a visual glitch. No key causes an accidental exit.
Only `q` or a mouse click (screensaver + --mouse) exits.

## Module Organization

The Dragon's codebase is organized by responsibility:

| Layer | Modules | LOC | Tests |
|-------|---------|-----|-------|
| Terminal I/O | terminal.rs, frame.rs, color_cache.rs, sgr_format.rs, termdetect.rs | ~2,500 | 50+ |
| Cloud Renderer | cloud/ (12 files) | ~5,000 | 100+ |
| Atmosphere Engine | atmosphere_*.rs (17 files) | ~3,000 | 80+ |
| Interactive Engine | interactive/ (7 files) | ~2,500 | 60+ |
| Config System | config.rs, config_apply.rs, configfile.rs, testconf.rs, safepath.rs | ~2,500 | 100+ |
| CLI + Helpers | cli.rs, output.rs, help_detail.rs, info.rs, ux.rs, verbose.rs | ~1,500 | 30+ |
| Diagnostics | bench*.rs (16 files), doctor.rs, report.rs | ~4,000 | 40+ |
| Dragon Incubator | dragon/ (egg/) | ~200 | 2 |

**Total: 128 files, 45K+ LOC, 874 tests.**

## The Dragon's Promise

1. **No silent failures** — every error prints a message and exits with
   a meaningful code (0 = success, 1 = runtime error, 2 = config/input error)
2. **No security bypass** — all file ops enforce the strict whitelist,
   no shell-redirection escape hatch
3. **No accidental exit** — only `q` quits, all other keys are silently
   ignored (no glitch, no surprise)
4. **No terminal left broken** — 5-layer recovery for SIGKILL, 3-layer
   process guard for clean exit, fork guard for Linux
5. **No color leak** — capability-aware escapes, plain text when piped,
   ANSI never leaks into scripts or log files
6. **No config ambiguity** — strict .toml extension, strict whitelist,
   strict validation (invalid = exit 2, no silent fallback)
7. **No stale docs** — CLI helpers, help text, and docs are kept in sync;
   the "semut" (ants) are regularly evicted from the house
