// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! CLI --help-detail output.
//! Extracted from config.rs to keep that file under 1000 LOC.

pub fn print_help_detail() {
    let text = "USAGE:
  cosmostrix [OPTIONS]

COMMON OPTIONS:
  -c, --color <name>
      Color theme. See --list-colors for available names.
      cosmostrix --color rainbow

  --color-tune <key=value[,key=value]>
      Tune theme colors. Accepted keys: sat, bright, head, body, tail.
      Range 0.0-3.0 (1.0 = no change).
      cosmostrix --color-tune sat=1.5,bright=0.9
      cosmostrix --color-tune sat=0.0           # grayscale
      cosmostrix --color-tune head=1.5,tail=0.5 # bright head, dim tail
      Also configurable in config.toml via [color.tune] section.

  --charset <name>
      Character set. See --list-charsets for available sets.
      cosmostrix --charset binary

  --charset-file <path>
      Load custom characters from a file. Overrides --charset.
      One character per line, or a single line of characters.
      Wide/zero-width characters (emoji, CJK fullwidth) are auto-filtered.
      cosmostrix --charset-file ~/my-chars.txt

  -f, --fps <1-240>
      Target FPS.
      cosmostrix --fps 30

  -S, --speed <1-100>
      Rain speed as a canonical integer. Runtime Up/Down controls use the
      same safe range.
      cosmostrix --speed 12

  -d, --density <0.01-5.0>
      Rain density multiplier.
      cosmostrix --density 1.25

  --monolith-size <small|normal|large>
      Monolith-only terminal-cell segment scale, not pixel size.
      cosmostrix --scene monolith --monolith-size large

  --uniform
      Uniform column speeds. Disables the default async variable pacing
      so all columns move at the same speed. Useful for testing or when
      you want a rigid, mechanical look instead of organic rain.
      cosmostrix --uniform

  -s, --screensaver
      Screensaver mode. Only 'q' exits — all other keys are silently
      ignored (no accidental exit). Recognized keys
      (c/s/x/p/i/h, Space, Up/Down, 0-9, etc.) still work for
      interactive control — cycle colors, toggle HUD, pause, etc.
      Mouse click does NOT exit (v17: removed for consistency with the
      only-q-quits policy). Mouse events are still captured to block
      text selection and trigger the always-on glow/click wave effects.
      cosmostrix --screensaver

  --intro
      Dragon's Awakening cinematic intro. A majestic ASCII dragon fades
      in at center, breathes a stream of fire particles downward, and
      the fire morphs into Matrix rain as the dragon fades away —
      handing off seamlessly to the rain engine. Duration: ~6.5s.
      Skip with any key. Auto-skipped in --screensaver and on terminals
      smaller than 100x24 (dragon art needs ~92 cols of width).
      cosmostrix --intro
      cosmostrix --intro --scene matrix

  Mouse interaction (always on, no flag needed)
      Cursor glow: cells near the mouse cursor get a subtle brightness
      boost (ambient halo). Click flash: a strong dual-ring glow wave
      radiates from the click point — a primary bright ring followed by
      a secondary dimmer echo, creating a cinematic stone-in-water
      ripple effect. Mouse reporting is always active to block text
      selection (drag-select is captured; Shift+drag still selects in
      most terminals — this is a terminal-emulator feature that cannot
      be disabled).

  -m, --message <text>
      Display overlay message (no border).
      cosmostrix -m \"hello\"

  -mb <text>
      Display overlay message with border.
      cosmostrix -mb \"hello\"

  --glitch-level <none|subtle|default|intense>
      Glitch intensity preset.

  --scene <name>
      Apply a built-in scene atmosphere. Scenes set color, charset,
      fps, speed, density, glitch-level, and rain style to curated
      values. Explicit CLI flags always override scene-managed values.
      Built-in scenes: matrix, monolith (default), signal, classic,
      cinematic, calm, storm, cosmos, neon, hacker, low-power.
      Use --list-scenes to see all entries with descriptions.
      cosmostrix
      cosmostrix --scene matrix
      cosmostrix --scene signal --fps 60
      cosmostrix --scene storm
      cosmostrix --scene low-power

  --scene-custom <name>
      Apply a user-defined custom scene from config. Custom scenes use
      the [scene-custom.<name>] namespace. If --scene-custom references
      a name that only exists as [profile.<name>], the profile is loaded
      with a deprecation warning guiding migration to [scene-custom.<name>].
      Explicit CLI flags always override custom-scene values.
      cosmostrix --scene-custom hacker-mode
      cosmostrix --scene-custom nightcore --fps 60

CONFIG:
  --config <path>
      Load config from an explicit path instead of the default
      $XDG_CONFIG_HOME/cosmostrix/config.toml (Linux) or
      ~/.config/cosmostrix/config.toml (Linux/macOS) or
      %APPDATA%/cosmostrix/config.toml (Windows).
      Security: strict whitelist — path must be inside
      ~/.config/cosmostrix/ or /etc/cosmostrix/ (Linux/macOS),
      %APPDATA%/cosmostrix/ or %ProgramData%/cosmostrix/ (Windows).
      Everything else is rejected (current directory, /tmp/, ~/, etc.).
      Must have .toml extension.

  --dump-config [path]
      Print a complete, commented example config and exit.
      With a path argument, writes to that file instead of stdout.

      Without a path: prints to stdout. Shell redirection (>, >|) is
      BLOCKED — cosmostrix detects stdout-redirected-to-file and refuses
      to write, because the shell bypasses the whitelist. Use the
      explicit path form for file output. Piping to another command
      (cosmostrix --dump-config | less) is allowed.

      With a path: writes to that file. The path must:
        1. Be inside ~/.config/cosmostrix/ or /etc/cosmostrix/
           (strict whitelist, same as --config)
        2. Have a .toml extension (strict, same as --config)
      Everything else is rejected.

      Examples (correct):
        cosmostrix --dump-config                                   # view on TTY
        cosmostrix --dump-config | less                            # pipe to pager
        cosmostrix --dump-config ~/.config/cosmostrix/config.toml  # write to file
      Examples (rejected):
        cosmostrix --dump-config > /tmp/a.txt                      # blocked (shell redirect)
        cosmostrix --dump-config ~/.config/cosmostrix/test.conf    # wrong extension
        cosmostrix --dump-config /tmp/a.toml                       # outside whitelist

      Config policy: invalid values print an error and exit (code 2).
      No silent fallback — strict validation.

  --config-path
      Print the resolved default config path and exit.

  --testconf
      Validate config file and report errors
      (typos, unknown keys, invalid values). Exit 0 = pass, 2 = fail.
      Run --config-path to see the resolved path for your platform.

  Precedence (highest wins):
      built-in defaults < scene defaults (fills unset keys only)
      < config values < config scene-custom
      < CLI scene < CLI scene-custom
      < explicit CLI flags.

      Key rule: a value set in config.toml ALWAYS wins over a scene's
      hardcoded default. Scenes only fill keys the user did NOT set.

APPEARANCE:
  --colormode <0|16|256|24>
      Force color depth. Auto-detected by default.

  -b, --bold <0|1|2>
      Bold style (off, random, all).

  -M, --shadingmode <0|1>
      Shading mode (random, cinematic).

  --color-bg <black|default-background>
      Background rendering mode. 'default-background' (default) means
      Cosmostrix does not paint a solid background — it follows the
      terminal emulator background. It does not change terminal emulator
      opacity.
      Example: if Alacritty uses a cyan background, default-background
      will show cyan behind the rain. 'black' forces solid #000000.

GENERAL:
  -F, --fullwidth
      Use full terminal width.

  --duration <seconds>
      Auto-stop after N seconds (0.1-86400).

DIAGNOSTICS:
  --doctor       Build info, renderer details, environment diagnostics, and
      terminal compatibility check. (v17: --info merged into --doctor)
  --benchmark    Renderer benchmark (5 seconds default; override with --bench-duration).
  --bench-duration <1-600>
      Benchmark duration in seconds (default 5). Use with --benchmark
      for long-run drift / leak / thermal-throttle detection. The DRIFT
      section of the report compares first-half FPS vs second-half FPS.
  --json         Output benchmark as JSON (use with --benchmark).
      Machine-readable single-line JSON for CI/scripts. Mirrors the text
      report's 13 sections. Option fields emit null; NaN/Inf emit null.
  --reset-terminal
      Emergency terminal recovery — the nuclear option.
      Use after SIGKILL (kill -9) or crash leaves the terminal broken.
      5-layer defense-in-depth recovery:
        1. ANSI restore: disable mouse, focus, paste, alt screen, sync output
        2. ANSI reset: clear screen + scrollback + cursor home
        3. crossterm: LeaveAlternateScreen, Clear, Show cursor, EnableLineWrap
        4. stty sane: restore terminal line discipline (raw mode off)
        5. reset + tput reset: external terminal reset utilities
      Also resets scroll region, character set, and auto-wrap.
      cosmostrix --reset-terminal
  --verbose      Print diagnostic info to stderr before launching. Shows
      config path, resolved values, terminal detection, atmosphere state.

DISCOVERY:
  --list-colors         Show color theme names.
                        There are 52 built-in themes.
  --list-charsets       Show available character sets.
  --list-scenes         Show built-in and custom scenes (from config).
  --show-scene <NAME>   Show full details for a built-in or custom scene.

RUNTIME CONTROLS:
  q             Quit              p          Pause / resume
  c / C         Cycle theme       s / S      Cycle charset
  x / X         Cycle scene       [ / ]      Density
  Up / Down     Speed
  Space         Reset animation
  i             Toggle live HUD (FPS / p99 / max / RSS / uptime)
  H or h        Move HUD to opposite corner (left ↔ right)

ADAPTIVE ATMOSPHERE (default, v15 Dragon):
  Cosmostrix breathes with the local time of day. Five emotional phases
  modulate rain density, speed, brightness, glitch, AND color scheme:

  00:00-03:00  Deep Void     deepspace palette, dense + dark + glitchy
  03:00-06:00  Compression   blackhole palette, extreme density
  06:00-12:00  Pulse         aurora palette, sparse + fast + bright
  12:00-18:00  Calm          cosmos palette, balanced + full brightness
  18:00-24:00  Signal        neon palette, rising glitch at dusk

  Color shifts every 30s via smooth palette transition wave.
  Disable: atmosphere-mode = disabled in config.toml.

HELP:
  --help          Show common options.
  --help-detail   Show this full reference.
  -V, --version    Print complete version and build information.
  --check-update   Check the latest upstream release.

RENDERING PHILOSOPHY:
  Cosmostrix is CPU-only by design. The terminal is a text medium —
  ANSI escape sequences are the brush, glyphs are the pixels. No GPU
  context (OpenGL/Vulkan/Metal/DirectX/WebGPU) is ever created. GPU
  image-mode would change Cosmostrix from \"terminal rain\" to \"image
  rain\" — a different program. See --doctor RENDERER for the field-level
  declaration and docs/PHILOSOPHY.md for the full rationale.
";

    if crate::config::color_enabled_stdout() {
        print!("{}", crate::config::colorize_help_detail(text));
    } else {
        print!("{}", text);
    }
}
