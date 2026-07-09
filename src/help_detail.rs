// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! CLI --help-detail output.
//! Extracted from config.rs to keep that file under 1000 LOC.

pub fn print_help_detail() {
    let text = "USAGE:
  cosmostrix [OPTIONS]

COMMON OPTIONS:
  -c, --color <name>
      Color theme. See --list-colors for compact names, or
      --list-colors-detail for grouped descriptions and aliases.
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

  --low-power
      Power-saving mode. Applies FPS 30, speed 5, density 0.5
      for parameters not explicitly provided.

  --glitch-level <none|subtle|default|intense>
      Glitch intensity preset.

  --preset <name>
      Apply a named parameter preset. Presets set color, charset,
      fps, speed, density, and glitch-level to curated values.
      Explicit CLI flags always override preset values.
      cosmostrix --preset cinematic
      cosmostrix --preset storm --fps 60

  --scene <matrix|monolith|signal>
      Apply a scene atmosphere. Monolith Rain is the default signature
      structured segmented rain experience.
      Charset cycling changes Monolith segment glyph style.
      Explicit CLI flags always override scene-managed values.
      cosmostrix
      cosmostrix --scene matrix
      cosmostrix --scene signal --fps 60

  --profile <name>
      Apply a user-defined profile from config. A profile starts from a
      base scene and overrides existing validated runtime fields.
      Explicit CLI flags always override profile values.
      cosmostrix --profile nightcore

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

  --dump-profile <name>
      Print one user-defined profile from config and exit.

  --config-path
      Print the resolved default config path and exit.

  --testconf
      Validate config file and report errors
      (typos, unknown keys, invalid values). Exit 0 = pass, 2 = fail.
      Run --config-path to see the resolved path for your platform.

  Precedence:
      built-in defaults < config values < config preset < config scene
      < config profile < CLI preset < CLI scene < CLI profile
      < low-power < explicit CLI flags.

APPEARANCE:
  --colormode <0|16|256|24>
      Force color depth. Auto-detected by default.

  -b, --bold <0|1|2>
      Bold style (off, random, all).

  -M, --shadingmode <0|1>
      Shading mode (random, cinematic).

  --color-bg <black|default-background|transparent>
      Background rendering mode. 'transparent' means Cosmostrix does not
      paint a solid background — it follows the terminal emulator
      background. It does not change terminal emulator opacity.
      Example: if Alacritty uses a black background, transparent will
      still look black.

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

  --perf-stats
      Print performance summary on exit.

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
  --list-colors         Show compact color theme names.
                        There are 43 built-in themes.
  --list-colors-detail  Show grouped theme descriptions and aliases.
  --list-charsets       Show available charset presets.
  --list-presets        Show available presets.
  --show-preset <NAME>  Show full details for a named preset.
  --list-scenes         Show available scene atmospheres.
  --list-profiles       Show user-defined profiles from config.
  --defaults            Show the default runtime profile.

RUNTIME CONTROLS:
  q / Esc       Quit              p          Pause / resume
  c / C         Cycle theme       s / S      Cycle charset
  x / X         Cycle scene       [ / ]      Density
  Up / Down     Speed             g          Toggle glitch
  m             Cycle profile     Space      Reset animation
  a             Toggle async      1-0        Direct color scheme
  ?             Toggle live HUD (FPS / p99 / max / RSS / uptime)
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
