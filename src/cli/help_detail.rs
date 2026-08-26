// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! CLI --help output: the full curated reference manual.
//!
//! v30 simplify: this used to be `--help-detail` (a separate advanced
//! reference). It is now printed by `--help` itself — cosmostrix has a
//! single-tier help surface. The file name is kept as `help_detail.rs`
//! for git-blame continuity; the public function is `print_help()`.

pub(crate) fn print_help() {
    let text = "USAGE:
  cosmostrix [OPTIONS]

COMMON OPTIONS:
  -c, --color <name>
      Color theme or custom palette name. See --list-colors.
      cosmostrix --color rainbow
      cosmostrix -c cyberpunk_2077   # custom palette from config

  --color-tune <key=value[,key=value]>
      Tune theme colors. Accepted keys: sat/saturation, bright/brightness,
      head, body, tail. Range 0.0-3.0 (1.0 = no change).
      cosmostrix --color-tune sat=1.5,bright=0.9
      cosmostrix --color-tune sat=0.0           # grayscale
      cosmostrix --color-tune head=1.5,tail=0.5 # bright head, dim tail
      Also configurable in config.toml via [color.tune] section.

  -C, --charset [--charset-custom] <name>
      Character set. See --list-charsets for available sets.
      Accepts built-in presets or custom names from [charset-custom.<name>].
      cosmostrix --charset binary
      cosmostrix -C cyberpunk_2077   # custom charset from config

      Custom charsets can be defined in config.toml under
      [charset-custom.<name>] and loaded by name. Custom names take
      precedence over built-in presets with the same name. Example:

        [charset-custom.zen]
        set = \"|\"

      Then: cosmostrix --charset zen
      Or set in config: charset = \"zen\"

      Live reload: editing the [charset-custom] block takes effect on
      the next config save (no restart needed).

  -f, --fps <1-240>
      Target FPS (interactive mode frame limiter). The loop sleeps between
      frames to maintain this cap; press 'i' to see it as `tgt:` in the HUD
      (alongside the render-work `fps:` line, which can be much higher
      because it measures 1000/work_ms, not the capped cadence). Adaptive
      idle throttle may halve the effective rate after 30s of no input
      (shown as `tgt: 30 idle`). In --benchmark mode this sets the
      simulation rate only — avg_fps in the report is unconstrained render
      throughput; check `target_fps` to confirm what you set.
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

  --async-mode <true|false>
      Async variable column speeds (default: true). When false, all
      columns move at the same speed (uniform pacing) for a rigid,
      mechanical look instead of organic rain. Replaces the old
      --uniform flag (removed in v50-beta.3).
      cosmostrix --async-mode false

  --crystal-dragon <true|false>
      Crystal Dragon ambient color drift (default: false). When true,
      enables point-based temperature group system that drifts the
      color palette based on CPU usage or wall-clock time.
      cosmostrix --crystal-dragon true

  --power-dragon <true|false>
      Power Dragon adaptive protection (default: true). When false,
      disables aggressive throttle + idle FPS reduction — rain stays
      at user-configured density/speed regardless of CPU pressure.
      cosmostrix --power-dragon false

  --msg-mode <true|false>
      Message overlay master switch (default: true). When false,
      disables BOTH the default message AND any config message/
      message-border. CLI -m / -mb always wins over msg-mode=false.
      cosmostrix --msg-mode false

  --intro-color <name>
      Intro animation color override. Accepts any builtin theme name
      (see --list-colors) or custom palette name. When unset, the
      intro uses the same color as the rain.
      cosmostrix --intro-color energy-zen

  -s, --screensaver
      Screensaver mode. Only 'q' exits — all other keys are silently
      ignored (no accidental exit). Recognized keys
      (c/s/x/p/i, Space, Up/Down.) still work for
      interactive control — cycle colors, toggle HUD, pause, etc.
      Mouse click does NOT exit (v17: removed for consistency with the
      only-q-quits policy). Mouse events are still captured to block
      text selection and trigger the always-on glow/click wave effects.
      cosmostrix --screensaver

  --intro [cosmic|logo|none]
      Cinematic intro played before the rain engine starts. Pick one
      of three modes (default: logo). The intro plays automatically
      when cosmostrix starts; pass --intro none to skip it entirely.
        logo    cosmostrix Logo intro (~6.25s). The ASCII logo fades in
                character by character, a spark falls from the top of
                the screen and ignites the logo on impact, then the
                logo dissolves into Matrix rain starting from the
                outer edges and moving inward. Rain engine takes
                over seamlessly.
        cosmic  Cosmic Burst intro (~5s). A singularity appears at
                center screen, pulses with rising frequency, then
                explodes into cosmic particles (gold, purple, cyan)
                that spiral outward. The particles slow and morph
                into Matrix rain.
        none    Skip the intro entirely.
      Press q (or Q) to skip the intro mid-animation. No other key
      skips — stray keypresses can't cut the cinematic short. Auto-
      skipped in --screensaver mode and on terminals smaller than
      80x24. The intro choice can be set permanently in config.toml
      via the 'intro' key (e.g. intro = \"cosmic\"). CLI flag wins
      over config.
      
      cosmostrix --intro logo
      cosmostrix --intro cosmic --scene matrix
      cosmostrix --intro none

  Mouse interaction (always on, no flag needed)
      Cursor glow: cells near the mouse cursor get a subtle brightness
      boost (ambient halo). Click flash: a strong dual-ring glow wave
      radiates from the click point — a primary bright ring followed by
      a secondary dimmer echo, creating a cinematic stone-in-water
      ripple effect. Mouse reporting is always active to block text
      selection (drag-select is captured; Shift+drag still selects in
      most terminals — this is a terminal-emulator feature that cannot
      be disabled).

  -m <text>
      Display overlay message (no border).
      cosmostrix -m \"hello\"

  -mb <text>
      Display overlay message with border.
      cosmostrix -mb \"hello\"

      Both -m and -mb can also be set in config.toml:
        message         = \"hello\"   (no border, matches -m)
        message-border  = \"hello\"   (with border, matches -mb)
      When neither CLI nor config provides a message, interactive mode
      defaults to a bordered overlay showing \"cosmostrix v<VERSION>\"
      (version is dynamic, read from Cargo.toml at compile time).
      Benchmark mode never shows a message overlay.
      msg-mode = false in config (or --msg-mode false on CLI) disables
      BOTH the default message AND any config message/message-border.
      CLI -m / -mb always wins over msg-mode=false.

  --glitch-level <none|subtle|default|intense>
      Glitch intensity preset.

  --scene <name>
      Apply a built-in scene atmosphere. Scenes set color, charset,
      fps, speed, density, glitch-level, and rain style to curated
      values. Explicit CLI flags always override scene-managed values.
      Built-in scenes: cinematic (default), matrix, monolith, signal, classic,
      calm, storm, cosmos, neon, hacker, low-power, matrix_film, cosmic-dragon,
      carbonic, dragon-crystal, orange-cat, north-stars, curiosity.
      Use --list-scenes to see all entries with descriptions.

      cosmic-dragon is the temporal-prediction milestone scene — a
      deep-space binary rain commemorating the  breakthrough
      where horizon=12 + skip-draw + persistent cells slashed dirty_ratio
      from 18.33% to 0.39% and boosted avg_fps from 7,843 to 29,773
      (+280%). It is the visible reward for the achievement, not part
      of the interactive x cycle.

      carbonic is a tribute to that same experiment. The temporal-
      prediction code was ultimately reverted in v25 because it
      compromised the cinematic visual quality, but the lessons learned
      about prediction, drift tolerance, and the tension between
      performance and beauty remain invaluable. `carbonic` evokes the
      aesthetic of carbon fiber: dark, dense, futuristic, and resilient.
      Palette `carbon` (dark-grey-to-silver ramp) + charset `binary`
      + speed 18 + density 0.95 produce a dense, energetic metallic
      rain that showcases the engine's throughput.

      cosmostrix --scene matrix
      cosmostrix --scene signal --fps 60
      cosmostrix --scene storm
      cosmostrix --scene low-power
      cosmostrix --scene cosmic-dragon
      cosmostrix --scene carbonic

  --scene-custom <name>
      Apply a user-defined custom scene from config. Custom scenes use
      the [scene-custom.<name>] namespace. Explicit CLI flags always
      override custom-scene values.

      Custom scenes are first-class citizens.  restores the
      `base-scene` field with cleaner inheritance semantics: when set,
      the custom scene inherits ALL scene-managed defaults (color,
      charset, fps, speed, density, glitch-level, rain_style) from the
      named built-in scene BEFORE applying its own overrides. Without
      `base-scene`, missing fields fall back to the global default scene
      (cinematic). When active, the verbose output shows `scene: <name>`
      and live reload applies edits to the block immediately
      (color/charset/speed/density/density-map/glitch-level/base-scene).

      cosmostrix --scene-custom hacker-mode
      cosmostrix --scene-custom nightcore --fps 60

CONFIG:
  --config <path>
      Load config from an explicit path instead of the default
      ~/.config/cosmostrix/config.toml (Linux, macOS, FreeBSD, Android
      Termux — or $XDG_CONFIG_HOME/cosmostrix/config.toml if set) or
      %APPDATA%/cosmostrix/config.toml (Windows).
      Security: strict whitelist — path must be inside one of:
        ~/.config/cosmostrix/                  (Linux, macOS, FreeBSD, Termux)
        /etc/cosmostrix/                       (Linux, macOS — system-wide)
        /usr/local/etc/cosmostrix/             (FreeBSD — system-wide)
        $PREFIX/etc/cosmostrix/                (Termux — system-wide)
        /sdcard/cosmostrix/                    (Termux — external storage)
        %APPDATA%/cosmostrix/ or %ProgramData%/cosmostrix/ (Windows)
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

  --force
      Force overwrite when writing files. Currently scoped to --dump-config
      ONLY: allows overwriting an existing config at the target path.
      Default --dump-config refuses to overwrite (data-loss guard).
      Other write operations (--save-baseline, etc.) are NOT affected
      by --force — they have their own per-flag overwrite policy.
      Examples:
        cosmostrix --dump-config ~/.config/cosmostrix/config.toml --force
      Destructive: replaces the existing file with the example template.

  Precedence (highest wins):
      built-in defaults < scene defaults (fills unset keys only)
      < config values < config scene-custom
      < CLI scene < CLI scene-custom
      < explicit CLI flags.

      Key rule: a value set in config.toml ALWAYS wins over a scene's
      hardcoded default. Scenes only fill keys the user did NOT set.

DIAGNOSTICS:
  --doctor       Build info, renderer details, environment diagnostics, and
      terminal compatibility check. (v17: --info merged into --doctor)
  --docs         Print engine documentation and architecture overview.
      Plain-text dump of The Cosmic Dragon Diff-Based Rendering Engine: the
      five cooperating subsystems (diff-based cell renderer, 3-layer
      parallax, phosphor persistence, density noise + wind gusts),
      the performance profile, and the design constraints. Pipes cleanly into less, grep, or docs
      generators (no ANSI codes).
      cosmostrix --docs
  --benchmark    Renderer benchmark (5 seconds default; override with --bench-duration).
      Runs DRY by default (no ANSI written to any file descriptor) to
      measure pure engine throughput. Add --bench-io for wet I/O.
  --bench-duration <N>
      Benchmark duration (default 5s). Accepts compound forms: 5, 6s, 30m,
      1h30m. Minimum 1s, no maximum (use for endurance runs). Use with
      --benchmark for long-run drift / leak / thermal-throttle detection.
      The DRIFT section of the report compares first-half FPS vs second-half FPS.
  --screen-size <WxH>
      Fixed virtual screen size (e.g. 120x40). Min 1x1. Max 1024x500 in
      interactive mode, 7680x4320 (8K UHD) in --benchmark mode. Useful
      for benchmarking at exact dimensions or rendering independent of
      terminal resize.
  --json         Output benchmark as JSON (use with --benchmark).
      Machine-readable single-line JSON for CI/scripts. Mirrors the text
      report's 13 sections. Option fields emit null; NaN/Inf emit null.
  --bench-io     Benchmark with wet terminal I/O — writes ANSI to /dev/null
      so the kernel syscall path is exercised without terminal emulator
      overhead. Measures real write bandwidth + latency. Default is dry
      (no I/O).
      cosmostrix --benchmark --bench-io --bench-duration 30
  --bench-all    Run benchmark across a fixed ladder of screen sizes
      (6x6 -> 20x20 -> 40x20 -> 80x24 -> 120x40 -> 200x60). Prints a
      SCALING SUMMARY table at the end. Use with --bench-duration to set
      per-size duration (default 2s each).
      cosmostrix --bench-all --bench-duration 5s
  --bench-scene <NAME>
      Benchmark I/O scene (used with --bench-io). Selects which render
      path the wet benchmark exercises:
        lean             (default) emit_cell_lean path — per-dirty-cell
                         SGR emission. The fastest interactive path.
        production-draw  mirrors Terminal::draw full-redraw path:
                         MoveTo per row + ColorCache SGR + BOLT bold
                         escape. Use to measure the BOLT-backed production
                         render path the user actually sees.
      Strict: only the two values above are accepted. Typos (e.g.
      \"leanax\", \"production-drawmadadadaxa\") are rejected with an error
      at parse time — never silently fallback'd to the default lean path.
      cosmostrix --benchmark --bench-io --bench-scene production-draw
  --save-baseline <path>
      Save benchmark JSON to a file (whitelist-enforced path, same as
      --config). Use to lock in a regression baseline for later diffing.
      cosmostrix --benchmark --save-baseline base.json
  --compare-baseline <path>
      Compare the current benchmark run against a saved baseline JSON.
      Flags >5% FPS regressions with a clear PASS/FAIL verdict.
      cosmostrix --benchmark --compare-baseline base.json
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
  -v, --verbose   Print diagnostic info to stderr before launching. Shows
      config path, resolved values, terminal detection, system feeling
      state. On exit, prints a final runtime state section with the local
      exit time and total run duration (v50.0.0-rc.1), followed by any
      live-reload field changes and the ambient diagnostics summary.

DISCOVERY:
  --list-colors         Show color theme names.
                        There are 44 built-in themes.
  --list-charsets       Show available character sets.
  --list-scenes         Show built-in and custom scenes (from config).
  --show-scene <NAME>   Show full details for a built-in or custom scene.

ADVANCED (intentionally not in clap's auto-list, but documented here — honest disclosure):
  These flags are intentionally excluded from clap's auto-generated argument
  list to keep the first impression clean, but they ARE documented in this
  manual. They are NOT deprecated, NOT experimental, and NOT internal-only
  — every one of them is a stable, supported knob. Most have an equivalent
  config key (see config.toml via --dump-config).

  APPEARANCE (advanced):
  -b, --bold <0|1|2>
      Bold style (0=off, 1=random [default], 2=all).
      Config: bold = 1
  --color-bg <black|default-background>
      Background rendering mode. 'default-background' (default) follows
      the terminal emulator background; 'black' forces solid #000000.
      Config: color-bg = \"black\"
  -M, --shadingmode <0|1>
      Shading mode (0=random, 1=cinematic [default]).
      Config: shadingmode = 1
  --colormode <0|16|256|24>
      Force color depth (auto-detected by default). Allowed: 0 (mono),
      16, 8/256 (8-bit), 24/32 (truecolor). Default: 24-bit if supported
      (COLORTERM), else 8-bit (TERM=...256color), else 16-color.

  TIMING & GLITCH (advanced):
  -g, --glitchms <LOW,HIGH>
      Glitch duration range in ms (min 1, max 5000). Default: 300,400.
      Config: (via --glitch-level preset)
  -l, --lingerms <LOW,HIGH>
      Linger time range in ms (min 1, max 60000). Default: 1,3000.
      Config: (via --glitch-level preset)
  --duration <seconds>
      Interactive auto-exit after N seconds (min 0.1, max 86400; <=0 disables).
      Bare float only (e.g. --duration 5 or --duration 0.5). For compound format
      (5s, 30m, 1h30m) use --bench-duration. NOOP in --benchmark/--bench-frames/
      --bench-all mode (warned at startup).
  --perf-stats
      Print performance statistics summary on exit (interactive mode).
      In --benchmark mode the BenchReportData is always emitted; this
      flag is for interactive runs that want a final perf summary.

  BENCH (advanced):
  --bench-frames <N>
      Run headless benchmark for exactly N frames and exit. Alternative
      to --bench-duration when you want frame-count-based measurement
      instead of time-based. Useful for cross-machine A/B at identical
      workloads. Dispatch precedence: --bench-all > --benchmark > --bench-frames.
      If --bench-frames is set alongside a higher-precedence flag, --bench-frames
      is ignored (warned). If --bench-frames is set with --bench-duration and
      neither --bench-all nor --benchmark is set, --bench-frames wins
      (--bench-duration ignored, warned).

RUNTIME CONTROLS:
  q             Quit              p          Pause / resume
  c / C         Cycle theme       s / S      Cycle charset
  x             Cycle scene       [ / ]      Density
  Up / Down     Speed
  Space         Reset animation
  i             Toggle live HUD (fps / tgt / max / p99 / cpu / rss / ehs / prs /
                sped / dsty / scn / chr / clr / up / screensize / cid)
                See docs/HUD.md for what each line means, why `fps:` ≠ `--fps`,
                and diagnostic recipes for common symptoms.

HELP:
  -h, --help      Print this full reference manual.
  -V, --version    Print complete version and build information.
  --check-update   Check the latest upstream release.

RENDERING PHILOSOPHY:
  cosmostrix is CPU-only by design. The terminal is a text medium —
  ANSI escape sequences are the brush, glyphs are the pixels. No GPU
  context (OpenGL/Vulkan/Metal/DirectX/WebGPU) is ever created. GPU
  image-mode would change cosmostrix from \"terminal rain\" to \"image
  rain\" — a different program. See --doctor RENDERER for the field-level
  declaration and docs/PHILOSOPHY.md for the full rationale.
";

    if crate::config::color_enabled_stdout() {
        print!("{}", crate::config::colorize_help(text));
    } else {
        print!("{}", text);
    }
}

// v50 (honesty audit): verify help text drift parameters match the
// source-of-truth constants. If someone retunes the constants without
// updating the help text, this test catches the drift.
#[cfg(test)]
mod honesty_tests {
    use crate::central_control_rains::COLOR_ECOSYSTEM_TICK_SECS;

    #[test]
    fn help_text_drift_params_match_constants() {
        // The help text mentions the ecosystem tick rate.
        // Verify it matches the actual constant.
        assert_eq!(
            COLOR_ECOSYSTEM_TICK_SECS, 3.0,
            "help text says 3s tick but COLOR_ECOSYSTEM_TICK_SECS changed"
        );
    }
}
