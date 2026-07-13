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
      Tune any theme's saturation/brightness at load time.
      Accepted keys (case-insensitive): saturation/sat, brightness/bright.
      Value range: 0.0 to 3.0 (1.0 = identity, no change).
      cosmostrix --color green --color-tune saturation=1.5,brightness=0.9
      cosmostrix --color aurora --color-tune sat=0.0     # grayscale
      cosmostrix --color red --color-tune bright=1.3     # +30% brightness
      This turns the 43 built-in themes into 43 x infinite variants
      without adding new presets.

  --charset <name>
      Character preset. See --list-charsets for available presets.
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
      Screensaver mode (exit on any keypress).

  --mouse
      Enable mouse hover/click effects. This turns on terminal mouse reporting
      while Cosmostrix is running; it is off by default for safer recovery
      after abrupt process termination.

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
      Falls back to legacy 'config' (no extension) for pre-v10 compatibility.

  --dump-config
      Print a complete, commented example config and exit.

      Config policy: invalid values print an error and fall back to defaults.

  --config-path
      Print the resolved default config path and exit.

  --testconf
      Validate config file and report errors
      (typos, unknown keys, invalid values). Exit 0 = pass, 2 = fail.
      Run --config-path to see the resolved path for your platform.

  Precedence (highest wins):
      built-in defaults < scene defaults (fills unset keys only)
      < config values < config preset < config profile
      < CLI preset < CLI scene < CLI profile
      < low-power < explicit CLI flags.

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
  -a, --async
      Variable column speeds for organic rain (default: on).
      Each column gets a random speed multiplier (33%-100% of base),
      producing desynchronized streams. Despite the name, this is NOT
      Rust async/await — cosmostrix remains single-threaded. The name
      'async' means 'asynchronous column pacing' — a legacy naming for
      the variable-speed visual effect. Press 'a' at runtime to toggle.

  -F, --fullwidth
      Use full terminal width.

  --duration <seconds>
      Auto-stop after N seconds (0.1-86400).

DIAGNOSTICS:
  --doctor       System compatibility report.
  --benchmark    Renderer benchmark (5 seconds default; override with --bench-duration).
  --bench-duration <1-600>
      Benchmark duration in seconds (default 5). Use with --benchmark
      for long-run drift / leak / thermal-throttle detection. The DRIFT
      section of the report compares first-half FPS vs second-half FPS.
  --json         Output benchmark as JSON (use with --benchmark).
      Machine-readable single-line JSON for CI/scripts. Mirrors the text
      report's 13 sections. Option fields emit null; NaN/Inf emit null.
  -i, --info     Build and runtime information.
  --reset-terminal
      Restore raw mode, alternate screen, cursor, focus, and mouse reporting
      after an interrupted run. Also clears the visible screen, moves the
      cursor home, and attempts scrollback purge when supported.
  --verbose      Print diagnostic info to stderr before launching. Shows
      config path, resolved values, terminal detection, atmosphere state.
  --completions <shell>
      Print shell completion script to stdout. Pipe to your shell's
      completions directory.
      Supported shells: bash, zsh, fish, elvish.
      Example: cosmostrix --completions bash > /etc/bash_completion.d/cosmostrix

DISCOVERY:
  --list-colors         Show color theme names.
                        There are 43 built-in themes.
  --list-charsets       Show available charset presets.
  --list-scenes         Show built-in and custom scenes (from config).
  --show-scene <NAME>   Show full details for a built-in or custom scene.

RUNTIME CONTROLS:
  q / Esc       Quit              p          Pause / resume
  c / C         Cycle theme       s / S      Cycle charset
  x / X         Cycle scene       [ / ]      Density
  Up / Down     Speed             g          Toggle glitch
  m             Cycle profile     Space      Reset animation
  a             Toggle async      1-0        Direct color scheme
  i             Toggle live HUD (FPS / p99 / max / RSS / uptime)
  H or h        Move HUD to opposite corner (left ↔ right)

HELP:
  --help          Show common options.
  --help-detail   Show this full reference.
  -V, --version    Print complete version and build information.
  --check-update   Check the latest upstream release.
";

    if crate::config::color_enabled_stdout() {
        print!("{}", crate::config::colorize_help_detail(text));
    } else {
        print!("{}", text);
    }
}
