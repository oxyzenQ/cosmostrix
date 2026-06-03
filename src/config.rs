// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! CLI argument definitions and help output generators.
//!
//! Cosmostrix follows a **curated simplicity** philosophy:
//! - `--help` shows a minimal, premium first impression
//! - `--help-detail` is an advanced reference — curated, not dumped
//! - `--glitch-level` provides a grouped interface over individual tuning knobs
//! - Advanced parameters remain fully functional but are intentionally hidden
//!   from the casual user.

use std::io::IsTerminal;
use std::str::FromStr;

use clap::Parser;

#[must_use]
pub fn color_enabled_stdout() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    if matches!(std::env::var("CLICOLOR").ok().as_deref(), Some("0")) {
        return false;
    }
    std::io::stdout().is_terminal()
}

fn colorize_help_detail(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 64);
    for chunk in text.split_inclusive('\n') {
        let (line, nl) = chunk
            .strip_suffix('\n')
            .map(|l| (l, "\n"))
            .unwrap_or((chunk, ""));

        let is_heading =
            !line.starts_with(' ') && line.ends_with(':') && line == line.to_ascii_uppercase();

        if is_heading {
            out.push_str("\x1b[1;36m");
            out.push_str(line);
            out.push_str("\x1b[0m");
            out.push_str(nl);
            continue;
        }

        if let Some(rest) = line.strip_prefix("      Example:") {
            out.push_str("      \x1b[32mExample:\x1b[0m");
            out.push_str(rest);
            out.push_str(nl);
            continue;
        }

        if let Some(rest) = line.strip_prefix("  cosmostrix") {
            out.push_str("  \x1b[1;34mcosmostrix\x1b[0m");
            out.push_str(rest);
            out.push_str(nl);
            continue;
        }

        if let Some(rest) = line.strip_prefix("  -") {
            out.push_str("  \x1b[33m-");
            out.push_str(rest);
            out.push_str("\x1b[0m");
            out.push_str(nl);
            continue;
        }

        if let Some(rest) = line.strip_prefix("  --") {
            out.push_str("  \x1b[33m--");
            out.push_str(rest);
            out.push_str("\x1b[0m");
            out.push_str(nl);
            continue;
        }

        out.push_str(line);
        out.push_str(nl);
    }
    out
}

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorBg {
    #[value(name = "black")]
    Black,
    #[value(name = "default-background")]
    DefaultBackground,
    #[value(name = "transparent")]
    Transparent,
}

/// Glitch intensity presets. Provides a grouped interface over individual
/// glitch tuning parameters (glitchpct, glitchms, shortpct, rippct).
#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlitchLevel {
    #[value(name = "none")]
    None,
    #[value(name = "subtle")]
    Subtle,
    #[value(name = "default")]
    Default,
    #[value(name = "intense")]
    Intense,
}

// ---------------------------------------------------------------------------
// U16Range
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct U16Range {
    pub low: u16,
    pub high: u16,
}

impl FromStr for U16Range {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (a, b) = s
            .split_once(',')
            .ok_or_else(|| "expected: NUM1,NUM2".to_string())?;
        let low: u16 = a
            .trim()
            .parse()
            .map_err(|_| "invalid low value".to_string())?;
        let high: u16 = b
            .trim()
            .parse()
            .map_err(|_| "invalid high value".to_string())?;
        if low == 0 || high == 0 || low > high {
            return Err("range must be >0 and low <= high (min allowed value is 1)".to_string());
        }
        Ok(Self { low, high })
    }
}

// ---------------------------------------------------------------------------
// Args — curated two-tier help design
//
// VISIBLE args appear in --help (the first impression).
// HIDDEN args are still fully functional but intentionally undocumented.
// ---------------------------------------------------------------------------

#[derive(Parser, Debug, Clone)]
#[command(
    name = "cosmostrix",
    version,
    disable_version_flag = true,
    about = "High-performance cinematic Matrix rain renderer for the terminal."
)]
pub struct Args {
    // === COMMON OPTIONS (visible in --help) ===
    #[arg(
        short = 'c',
        long = "color",
        default_value = "green",
        help_heading = "COMMON OPTIONS",
        display_order = 10,
        help = "Color theme (see --list-colors)"
    )]
    pub color: String,

    #[arg(
        long = "charset",
        default_value = "binary",
        help_heading = "COMMON OPTIONS",
        display_order = 20,
        help = "Character preset (see --list-charsets)"
    )]
    pub charset: String,

    #[arg(
        short = 'f',
        long = "fps",
        default_value_t = 60.0,
        help_heading = "COMMON OPTIONS",
        display_order = 30,
        help = "Target FPS"
    )]
    pub fps: f64,

    #[arg(
        short = 'S',
        long = "speed",
        default_value_t = 8.0,
        help_heading = "COMMON OPTIONS",
        display_order = 40,
        help = "Rain speed"
    )]
    pub speed: f32,

    #[arg(
        short = 'd',
        long = "density",
        default_value_t = 1.0,
        help_heading = "COMMON OPTIONS",
        display_order = 50,
        help = "Rain density"
    )]
    pub density: f32,

    #[arg(
        short = 's',
        long = "screensaver",
        help_heading = "COMMON OPTIONS",
        display_order = 60,
        help = "Screensaver mode (exit on keypress)"
    )]
    pub screensaver: bool,

    #[arg(
        long = "mouse",
        help_heading = "COMMON OPTIONS",
        display_order = 65,
        help = "Enable mouse hover/click effects"
    )]
    pub mouse: bool,

    #[arg(
        short = 'm',
        long = "message",
        help_heading = "COMMON OPTIONS",
        display_order = 70,
        help = "Overlay message"
    )]
    pub message: Option<String>,

    #[arg(
        long = "low-power",
        help_heading = "COMMON OPTIONS",
        display_order = 80,
        help = "Power-saving mode (30 FPS, reduced density/speed)"
    )]
    pub low_power: bool,

    #[arg(
        long = "glitch-level",
        default_value = "default",
        value_enum,
        help_heading = "COMMON OPTIONS",
        display_order = 90,
        help = "Glitch intensity"
    )]
    pub glitch_level: GlitchLevel,

    // === DIAGNOSTICS (visible in --help) ===
    #[arg(
        long = "doctor",
        help_heading = "DIAGNOSTICS",
        display_order = 100,
        help = "System compatibility report"
    )]
    pub doctor: bool,

    #[arg(
        long = "benchmark",
        help_heading = "DIAGNOSTICS",
        display_order = 110,
        help = "Renderer benchmark"
    )]
    pub benchmark: bool,

    #[arg(
        long = "info",
        short = 'i',
        help_heading = "DIAGNOSTICS",
        display_order = 120,
        help = "Build and runtime information"
    )]
    pub info: bool,

    #[arg(
        long = "reset-terminal",
        help_heading = "DIAGNOSTICS",
        display_order = 130,
        help = "Restore terminal modes after an interrupted run"
    )]
    pub reset_terminal: bool,

    // === DISCOVERY (visible in --help) ===
    #[arg(
        long = "list-colors",
        help_heading = "DISCOVERY",
        display_order = 200,
        help = "Show available color themes"
    )]
    pub list_colors: bool,

    #[arg(
        long = "list-charsets",
        help_heading = "DISCOVERY",
        display_order = 210,
        help = "Show available charset presets"
    )]
    pub list_charsets: bool,

    #[arg(
        long = "defaults",
        help_heading = "DISCOVERY",
        display_order = 220,
        help = "Show the default runtime profile"
    )]
    pub defaults: bool,

    // === HELP (visible in --help) ===
    #[arg(
        long = "help-detail",
        help_heading = "HELP",
        display_order = 300,
        help = "Full advanced documentation"
    )]
    pub help_detail: bool,

    #[arg(
        long = "version",
        short = 'v',
        help_heading = "HELP",
        display_order = 320,
        help = "Show version"
    )]
    pub version: bool,

    // === HIDDEN (functional but intentionally undocumented) ===
    #[arg(
        short = 'a',
        long = "async",
        default_value_t = false,
        action = clap::ArgAction::Set,
        num_args = 0..=1,
        default_missing_value = "true",
        hide = true,
        help = "Async rendering (default: off)"
    )]
    pub async_mode: bool,

    #[arg(
        short = 'b',
        long = "bold",
        default_value_t = 1,
        hide = true,
        help = "Bold style: 0=off, 1=random, 2=all (min 0 max 2)"
    )]
    pub bold: u8,

    #[arg(
        long = "color-bg",
        default_value_t = ColorBg::Black,
        value_enum,
        hide = true,
        help = "Background mode (black, default-background, transparent)"
    )]
    pub color_bg: ColorBg,

    #[arg(
        short = 'F',
        long = "fullwidth",
        hide = true,
        help = "Use full terminal width"
    )]
    pub fullwidth: bool,

    #[arg(
        long = "duration",
        hide = true,
        help = "Stop after N seconds (min 0.1 max 86400; <=0 disables)"
    )]
    pub duration: Option<f64>,

    #[arg(
        long = "perf-stats",
        hide = true,
        help = "Print performance statistics summary on exit"
    )]
    pub perf_stats: bool,

    #[arg(
        long = "bench-frames",
        hide = true,
        help = "Run headless benchmark for N frames and exit"
    )]
    pub bench_frames: Option<u64>,

    #[arg(
        short = 'g',
        long = "glitchms",
        default_value = "300,400",
        hide = true,
        help = "Glitch duration range in ms: LOW,HIGH (min 1 max 5000)"
    )]
    pub glitch_ms: U16Range,

    #[arg(
        short = 'G',
        long = "glitchpct",
        default_value_t = 10.0,
        hide = true,
        help = "Glitch chance in percent (min 0 max 100)"
    )]
    pub glitch_pct: f32,

    #[arg(
        short = 'l',
        long = "lingerms",
        default_value = "1,3000",
        hide = true,
        help = "Linger time range in ms: LOW,HIGH (min 1 max 60000)"
    )]
    pub linger_ms: U16Range,

    #[arg(
        short = 'M',
        long = "shadingmode",
        default_value_t = 1,
        hide = true,
        help = "Shading: 0=random, 1=cinematic (min 0 max 1)"
    )]
    pub shading_mode: u8,

    #[arg(
        long = "message-no-border",
        hide = true,
        help = "Draw message box without border (use with --message; shorthand: -mB)"
    )]
    pub message_no_border: bool,

    #[arg(
        long = "maxdpc",
        default_value_t = 3,
        hide = true,
        help = "Stream layering (min 1 max 3)"
    )]
    pub max_droplets_per_column: u8,

    #[arg(
        long = "noglitch",
        default_value_t = true,
        action = clap::ArgAction::Set,
        num_args = 0..=1,
        default_missing_value = "true",
        hide = true,
        help = "Disable glitch effects (default: on)"
    )]
    pub noglitch: bool,

    #[arg(
        short = 'r',
        long = "rippct",
        default_value_t = 33.33333,
        hide = true,
        help = "Stream decay chance in percent (min 0 max 100)"
    )]
    pub rippct: f32,

    #[arg(
        long = "shortpct",
        default_value_t = 50.0,
        hide = true,
        help = "Fragmented stream chance in percent (min 0 max 100)"
    )]
    pub shortpct: f32,

    #[arg(long = "chars", hide = true, help = "Custom characters override")]
    pub chars: Option<String>,

    #[arg(
        long = "colormode",
        hide = true,
        help = "Force color mode (allowed: 0,16,8/256,24/32). Default: 24-bit if supported (COLORTERM), else 8-bit (TERM=...256color), else 16-color"
    )]
    pub colormode: Option<u16>,

    #[arg(
        long = "check-bitcolor",
        hide = true,
        help = "Print detected terminal color capability and exit"
    )]
    pub check_bitcolor: bool,
}

// ---------------------------------------------------------------------------
// List printers — clean, no alias noise
// ---------------------------------------------------------------------------

pub fn print_list_charsets() {
    if color_enabled_stdout() {
        println!("\x1b[1;36mAVAILABLE CHARSET PRESETS:\x1b[0m");
    } else {
        println!("AVAILABLE CHARSET PRESETS:");
    }
    println!();
    println!("  auto         Auto-select (ASCII_SAFE when non-UTF, otherwise matrix)");
    println!("  matrix       Letters + digits + katakana");
    println!("  ascii        Letters + digits + punctuation");
    println!("  extended     Digits + punctuation + katakana");
    println!("  english      Letters only");
    println!("  digits       Digits only");
    println!("  punc         Punctuation only");
    println!("  binary       0 and 1");
    println!("  hex          0-9 and A-F");
    println!("  katakana     Katakana");
    println!("  greek        Greek");
    println!("  cyrillic     Cyrillic");
    println!("  hebrew       Hebrew");
    println!("  blocks       Block elements");
    println!("  symbols      Math / technical symbols");
    println!("  arrows       Arrow symbols");
    println!("  retro        Box-drawing characters");
    println!("  cyberpunk    Katakana + hex + symbols");
    println!("  hacker       Letters + hex + punctuation + symbols");
    println!("  minimal      Dots and simple shapes");
    println!("  code         Letters + digits + punctuation + symbols");
    println!("  dna          DNA bases (ACGT)");
    println!("  braille      Braille");
    println!("  runic        Runic");
}

pub fn print_list_colors() {
    if color_enabled_stdout() {
        println!("\x1b[1;36mAVAILABLE COLOR THEMES:\x1b[0m");
    } else {
        println!("AVAILABLE COLOR THEMES:");
    }
    println!();
    println!("  green        green2       green3");
    println!("  yellow       orange       red");
    println!("  blue         cyan         gold");
    println!("  rainbow      purple       neon");
    println!("  fire         ocean        forest");
    println!("  vaporwave    spectrum20   gray");
    println!("  snow         aurora       fancy-diamond");
    println!("  cosmos       nebula       stars");
    println!("  mars         venus        mercury");
    println!("  jupiter      saturn       uranus");
    println!("  neptune      pluto        moon");
    println!("  sun          comet        galaxy");
    println!("  supernova    blackhole    andromeda");
    println!("  stardust     meteor       eclipse");
    println!("  deepspace");
}

pub fn print_defaults() {
    if color_enabled_stdout() {
        println!("\x1b[1mCOSMOSTRIX DEFAULT PROFILE\x1b[0m");
    } else {
        println!("COSMOSTRIX DEFAULT PROFILE");
    }
    println!("{}", "\u{2500}".repeat(27));
    println!("cosmostrix \\");
    println!("  --fps 60 \\");
    println!("  --speed 8 \\");
    println!("  --density 1 \\");
    println!("  --color green \\");
    println!("  --charset binary \\");
    println!("  --glitch-level default");
}

// ---------------------------------------------------------------------------
// --help-detail: curated advanced reference
//
// Design principle: guide, don't dump. No embedded catalogs, no schema dumps,
// no verbose alias disclosures. Discovery commands handle discovery.
// ---------------------------------------------------------------------------

pub fn print_help_detail() {
    let text = "USAGE:
  cosmostrix [OPTIONS]

COMMON OPTIONS:
  -c, --color <name>
      Color theme. See --list-colors for available themes.
      cosmostrix --color rainbow

  --charset <name>
      Character preset. See --list-charsets for available presets.
      cosmostrix --charset binary

  -f, --fps <1-240>
      Target FPS.
      cosmostrix --fps 30

  -S, --speed <0.001-1000>
      Rain speed (characters per second).
      cosmostrix --speed 12

  -d, --density <0.01-5.0>
      Rain density multiplier.
      cosmostrix --density 1.25

  -s, --screensaver
      Screensaver mode (exit on any keypress).

  --mouse
      Enable mouse hover/click effects. This turns on terminal mouse reporting
      while Cosmostrix is running; it is off by default for safer recovery
      after abrupt process termination.

  -m, --message <text>
      Display overlay message.
      cosmostrix -m \"hello\"

  --low-power
      Power-saving mode. Applies FPS 30, speed 5, density 0.5
      for parameters not explicitly provided.

  --glitch-level <none|subtle|default|intense>
      Glitch intensity preset.

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
      Enable legacy async-style rain pacing compatibility mode.
      Advanced option; default adaptive renderer is recommended.

  -F, --fullwidth
      Use full terminal width.

  --duration <seconds>
      Auto-stop after N seconds (0.1-86400).

  --perf-stats
      Print performance summary on exit.

DIAGNOSTICS:
  --doctor       System compatibility report.
  --benchmark    Renderer benchmark (5 seconds).
  -i, --info     Build and runtime information.
  --reset-terminal
      Restore raw mode, alternate screen, cursor, focus, and mouse reporting
      after an interrupted run.

DISCOVERY:
  --list-colors    Show available color themes.
  --list-charsets  Show available charset presets.

RUNTIME CONTROLS:
  q / Esc       Quit              p          Pause / resume
  c / C         Cycle theme       s / S      Cycle charset
  [ / ]         Density           Up / Down  Speed
  g             Toggle glitch     m          Cycle profile
  Tab           Ignored safely    Space      Reseed animation

HELP:
  --help          Show common options.
  --help-detail   Show this full reference.
  -v, --version    Print version.
";

    if color_enabled_stdout() {
        print!("{}", colorize_help_detail(text));
    } else {
        print!("{}", text);
    }
}
