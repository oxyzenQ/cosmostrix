// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! CLI argument definitions and help output generators.
//!
//! Cosmostrix follows a **curated simplicity** philosophy:
//! - `--help` shows a minimal, premium first impression
//! - `--help-detail` is an advanced reference — curated, not dumped
//! - `--glitch-level` provides a grouped interface over individual tuning knobs
//! - Advanced parameters remain fully functional but are intentionally hidden
//!   from the casual user.

use std::io::IsTerminal;
use std::path::PathBuf;
use std::str::FromStr;

use clap::Parser;

use crate::runtime::MonolithSize;
use crate::scene;
use crate::theme;
use crate::{configfile, profile, scene_custom};

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

pub(crate) fn colorize_help_detail(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 64);
    for chunk in text.split_inclusive('\n') {
        let (line, nl) = chunk
            .strip_suffix('\n')
            .map(|l| (l, "\n"))
            .unwrap_or((chunk, ""));

        let is_heading =
            !line.starts_with(' ') && line.ends_with(':') && line == line.to_ascii_uppercase();

        if is_heading {
            // Bold magenta (purple) for section headings
            out.push_str("\x1b[1;35m");
            out.push_str(line);
            out.push_str("\x1b[0m");
            out.push_str(nl);
            continue;
        }

        if let Some(rest) = line.strip_prefix("      Example:") {
            // Bold white for "Example:" labels
            out.push_str("      \x1b[1mExample:\x1b[0m");
            out.push_str(rest);
            out.push_str(nl);
            continue;
        }

        if let Some(rest) = line.strip_prefix("  cosmostrix") {
            // Bold white for command examples
            out.push_str("  \x1b[1mcosmostrix\x1b[0m");
            out.push_str(rest);
            out.push_str(nl);
            continue;
        }

        if let Some(rest) = line.strip_prefix("  -") {
            // Bold white for short flags (-c, -S, etc.)
            out.push_str("  \x1b[1m-");
            out.push_str(rest);
            out.push_str("\x1b[0m");
            out.push_str(nl);
            continue;
        }

        if let Some(rest) = line.strip_prefix("  --") {
            // Bold white for long flags (--color, --fps, etc.)
            out.push_str("  \x1b[1m--");
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
    about = "Production-grade cinematic Matrix rain renderer for serious terminal environments."
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
        long = "color-tune",
        help_heading = "COMMON OPTIONS",
        display_order = 11,
        help = "Tune any theme's saturation/brightness at load time. \
                Example: --color-tune saturation=1.2,brightness=0.9 \
                (keys: saturation/sat, brightness/bright; range 0.0-3.0)"
    )]
    pub color_tune: Option<String>,

    #[arg(
        long = "charset",
        default_value = "binary",
        help_heading = "COMMON OPTIONS",
        display_order = 20,
        help = "Character preset (see --list-charsets)"
    )]
    pub charset: String,

    #[arg(
        long = "charset-file",
        value_name = "PATH",
        help_heading = "COMMON OPTIONS",
        display_order = 21,
        help = "Load custom characters from a file (one character per line, or a single line of characters). Overrides --charset."
    )]
    pub charset_file: Option<String>,

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
        long = "monolith-size",
        default_value = "normal",
        value_enum,
        help_heading = "ADVANCED",
        display_order = 56,
        help = "Monolith segment cell scale"
    )]
    pub monolith_size: MonolithSize,

    #[arg(
        long = "uniform",
        help_heading = "ADVANCED",
        display_order = 57,
        help = "Uniform column speeds (disables async variable pacing). \
                By default cosmostrix uses variable column speeds for organic rain; \
                --uniform makes all columns move at the same speed."
    )]
    pub uniform: bool,

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
        help = "Overlay message (no border). Use -mb for border."
    )]
    pub message: Option<String>,

    #[arg(
        long = "glitch-level",
        default_value = "default",
        value_enum,
        help_heading = "COMMON OPTIONS",
        display_order = 90,
        help = "Glitch intensity"
    )]
    pub glitch_level: GlitchLevel,

    #[arg(
        long = "scene",
        help_heading = "COMMON OPTIONS",
        display_order = 96,
        help = "Apply a scene atmosphere (see --list-scenes)"
    )]
    pub scene: Option<String>,

    #[arg(
        long = "profile",
        help_heading = "COMMON OPTIONS",
        display_order = 97,
        help = "Apply a user-defined profile from config (see --list-profiles)"
    )]
    pub profile: Option<String>,

    #[arg(
        long = "scene-custom",
        value_name = "NAME",
        help_heading = "COMMON OPTIONS",
        display_order = 98,
        help = "Apply a user-defined custom scene from config (see --list-scenes)"
    )]
    pub scene_custom: Option<String>,

    // === CONFIG (visible in --help) ===
    #[arg(
        long = "config",
        value_name = "PATH",
        help_heading = "CONFIG",
        display_order = 98,
        help = "Load config from an explicit file path"
    )]
    pub config: Option<PathBuf>,

    #[arg(
        long = "dump-config",
        help_heading = "CONFIG",
        display_order = 99,
        help = "Print a complete example config and exit"
    )]
    pub dump_config: bool,

    #[arg(
        long = "dump-profile",
        value_name = "NAME",
        help_heading = "CONFIG",
        display_order = 100,
        help = "Print one user profile from config and exit"
    )]
    pub dump_profile: Option<String>,

    #[arg(
        long = "config-path",
        help_heading = "CONFIG",
        display_order = 101,
        help = "Print the default config path and exit"
    )]
    pub config_path: bool,

    #[arg(
        long = "testconf",
        help_heading = "CONFIG",
        display_order = 102,
        help = "Validate config.toml and report errors (typos, unknown keys, invalid values). Run --config-path to see the location."
    )]
    pub testconf: bool,

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
        help = "Renderer benchmark (5s default; override with --bench-duration)"
    )]
    pub benchmark: bool,

    #[arg(
        long = "bench-duration",
        help_heading = "DIAGNOSTICS",
        display_order = 111,
        help = "Benchmark duration (default 5). Accepts bare seconds (5) or \
                compound format (6s, 30m, 1h30m). Min: 1s. No max cap. \
                Use with --benchmark for long-run drift / leak detection."
    )]
    pub bench_duration: Option<String>,

    #[arg(
        long = "screen-size",
        help_heading = "DIAGNOSTICS",
        display_order = 113,
        help = "Fixed screen size WxH (e.g. 120x40, 12x12). \
                Benchmark: override terminal size. \
                Interactive: render to fixed virtual size (ignores terminal resize). \
                Minimum: 1x1. If larger than terminal, renders to top-left and clips."
    )]
    pub screen_size: Option<String>,

    #[arg(
        long = "json",
        help_heading = "DIAGNOSTICS",
        display_order = 112,
        help = "Output benchmark report as JSON (use with --benchmark). \
                Machine-readable for CI/scripts. Pairs with --bench-duration."
    )]
    pub json: bool,

    #[arg(
        long = "save-baseline",
        help_heading = "DIAGNOSTICS",
        display_order = 114,
        help = "Save benchmark JSON to file for later comparison. \
                Use with --benchmark --json. Example: --save-baseline v13.5.0.json"
    )]
    pub save_baseline: Option<String>,

    #[arg(
        long = "compare-baseline",
        help_heading = "DIAGNOSTICS",
        display_order = 115,
        help = "Compare current benchmark against saved baseline JSON. \
                Flags regressions (>5% FPS drop) and improvements (>5% FPS gain). \
                Example: --compare-baseline v13.4.0.json"
    )]
    pub compare_baseline: Option<String>,

    #[arg(
        long = "bench-io",
        help_heading = "DIAGNOSTICS",
        display_order = 116,
        help = "Benchmark with wet terminal I/O (writes ANSI to /dev/null). \
                Measures real write bandwidth + latency. Default: dry (no I/O)."
    )]
    pub bench_io: bool,

    #[arg(
        long = "bench-all",
        help_heading = "DIAGNOSTICS",
        display_order = 117,
        help = "Run benchmark across multiple screen sizes (6x6 to 200x60). \
                Prints a SCALING SUMMARY table. Use with --bench-duration for \
                per-size duration (default 2s each)."
    )]
    pub bench_all: bool,

    #[arg(
        long = "tune-visual",
        help_heading = "DIAGNOSTICS",
        display_order = 118,
        help = "Auto-tune parameters to match target visual metrics. \
                Format: entropy=5.2,gini=0.6. Runs iterative benchmark to find \
                best density/glitch combination. Prints recommended config."
    )]
    pub tune_visual: Option<String>,

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
        help = "Destructive terminal recovery: clears screen, purges scrollback, resets modes"
    )]
    pub reset_terminal: bool,

    // === DISCOVERY (visible in --help) ===
    #[arg(
        long = "list-colors",
        help_heading = "DISCOVERY",
        display_order = 200,
        help = "Show compact color theme names"
    )]
    pub list_colors: bool,

    #[arg(
        long = "list-colors-detail",
        help_heading = "DISCOVERY",
        display_order = 205,
        help = "Show grouped color themes with descriptions and aliases"
    )]
    pub list_colors_detail: bool,

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

    #[arg(
        long = "list-scenes",
        help_heading = "DISCOVERY",
        display_order = 230,
        help = "Show available scene atmospheres"
    )]
    pub list_scenes: bool,

    #[arg(
        long = "show-scene",
        value_name = "NAME",
        help_heading = "DISCOVERY",
        display_order = 231,
        help = "Show full details for a built-in or custom scene"
    )]
    pub show_scene: Option<String>,

    #[arg(
        long = "list-profiles",
        help_heading = "DISCOVERY",
        display_order = 235,
        help = "Show user-defined profiles from config"
    )]
    pub list_profiles: bool,

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
        short = 'V',
        help_heading = "HELP",
        display_order = 320,
        help = "Print complete version and build information"
    )]
    pub version: bool,

    #[arg(
        long = "check-update",
        alias = "check-updated",
        help_heading = "HELP",
        display_order = 330,
        help = "Check the latest upstream release"
    )]
    pub check_update: bool,

    #[arg(
        long = "verbose",
        help_heading = "DIAGNOSTICS",
        display_order = 130,
        help = "Print diagnostic info to stderr (config resolution, path \
                validation, terminal detection, atmosphere state). Useful \
                for debugging config/loading issues."
    )]
    pub verbose: bool,

    #[arg(
        long = "completions",
        help_heading = "DIAGNOSTICS",
        display_order = 140,
        help = "Print shell completion script. Usage: --completions <shell>\n\
                Supported: bash, zsh, fish, elvish.\n\
                Install: cosmostrix --completions bash > ~/.config/cosmostrix/completions.bash"
    )]
    pub completions: Option<String>,

    // === HIDDEN (functional but intentionally undocumented in --help) ===
    #[arg(
        short = 'a',
        long = "async",
        default_value_t = true,
        action = clap::ArgAction::Set,
        num_args = 0..=1,
        default_missing_value = "true",
        hide = true,
        help = "Variable column speeds for organic rain (default: on)"
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
        default_value_t = ColorBg::DefaultBackground,
        value_enum,
        hide = true,
        help = "Background mode (black, default-background)"
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
        long = "auto-color-drift",
        hide = true,
        help = "Enable autonomous palette drift (default: off)"
    )]
    pub auto_color_drift: bool,

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
        long = "message-border",
        hide = true,
        help = "Draw message box with border (use with --message; shorthand: -mb)"
    )]
    pub message_border: bool,

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

    // Atmosphere engine config (gated/internal-first — Phase 10).
    // NOT exposed as public CLI flags. Resolved from config/profile only.
    #[arg(
        long = "atmosphere-mode",
        hide = true,
        help = "Atmosphere mode (config only: disabled, controlled-live)"
    )]
    pub atmosphere_mode_str: Option<String>,

    #[arg(
        long = "atmosphere-regime",
        hide = true,
        help = "Atmosphere regime (config only: calm, pulse, signal, compression, void, monolith-pressure)"
    )]
    pub atmosphere_regime_str: Option<String>,
}

// ---------------------------------------------------------------------------
// List printers — clean, no alias noise
// ---------------------------------------------------------------------------

pub fn print_list_charsets() {
    if color_enabled_stdout() {
        println!("\x1b[1;35mAVAILABLE CHARSET PRESETS:\x1b[0m");
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
    println!();
    println!("  Or use --charset-file <path> to load custom characters from a file.");
}

pub fn print_list_colors() {
    if color_enabled_stdout() {
        println!("\x1b[1;35mAVAILABLE COLOR THEMES:\x1b[0m");
    } else {
        println!("AVAILABLE COLOR THEMES:");
    }
    println!();
    print!("{}", theme::compact_list_text());
    println!();
    println!(
        "{} built-in themes. Use --list-colors-detail for descriptions and aliases.",
        theme::theme_count()
    );
}

pub fn print_list_colors_detail() {
    if color_enabled_stdout() {
        println!("\x1b[1;35mCOLOR THEME CATALOG:\x1b[0m");
    } else {
        println!("COLOR THEME CATALOG:");
    }
    println!();

    print!("{}", theme::detail_list_text());
    println!("{} built-in themes.", theme::theme_count());
}

pub fn print_list_scenes() {
    if color_enabled_stdout() {
        println!("\x1b[1;35mAVAILABLE SCENES:\x1b[0m");
    } else {
        println!("AVAILABLE SCENES:");
    }
    println!();
    print!("{}", scene::list_scenes_text());

    // Append custom scenes from config (if any) under a separate heading.
    let cfg = configfile::load_config_file(None);
    let custom_scenes = scene_custom::collect_custom_scenes(&cfg);
    if !custom_scenes.is_empty() {
        println!();
        if color_enabled_stdout() {
            println!("\x1b[1;35mCUSTOM SCENES (from config):\x1b[0m");
        } else {
            println!("CUSTOM SCENES (from config):");
        }
        println!();
        print!("{}", scene_custom::list_custom_scenes_text(&custom_scenes));
        println!();
        println!("  Load with: cosmostrix --scene-custom <name>");
    }
}

/// Print details for a single scene by name. Looks up built-in scenes first,
/// then custom scenes from config. Returns `Ok(())` on success or an error
/// message suitable for `ux::die_config`.
pub fn print_show_scene(
    name: &str,
    cfg: &std::collections::HashMap<String, String>,
) -> Result<(), String> {
    // 1. Built-in scene lookup.
    if let Some(info) = scene::get_scene(name) {
        print!("{}", scene::show_scene_text(info));
        return Ok(());
    }

    // 2. Custom scene lookup (also falls back to legacy [profile.X] entries).
    let custom_scenes = scene_custom::collect_custom_scenes(cfg);
    let profiles = profile::collect_profiles(cfg);
    let normalized = name.trim().to_ascii_lowercase();
    if let Some(custom) = custom_scenes.get(&normalized) {
        print!(
            "{}",
            scene_custom::show_custom_scene_text(&normalized, custom, /*from_profile=*/ false)
        );
        return Ok(());
    }
    if let Some(legacy) = profiles.get(&normalized) {
        eprintln!(
            "warning: '{normalized}' is defined as [profile.{normalized}] — migrate to [scene-custom.{normalized}] (rename prefix only)"
        );
        print!(
            "{}",
            scene_custom::show_custom_scene_text(&normalized, legacy, /*from_profile=*/ true)
        );
        return Ok(());
    }

    // 3. Not found.
    let mut available: Vec<String> = scene::all_scene_names()
        .iter()
        .map(|s| s.to_string())
        .collect();
    available.extend(custom_scenes.keys().cloned());
    available.extend(profiles.keys().cloned());
    available.sort();
    available.dedup();
    let list = if available.is_empty() {
        "<none defined>".to_string()
    } else {
        available.join(", ")
    };
    Err(format!(
        "error: unknown scene '{name}'\n\n  Available: {list}\n  Use --list-scenes to see all scenes."
    ))
}

pub fn print_defaults() {
    if color_enabled_stdout() {
        println!("\x1b[1mCOSMOSTRIX DEFAULT PROFILE\x1b[0m");
    } else {
        println!("COSMOSTRIX DEFAULT PROFILE");
    }
    println!("{}", "\u{2500}".repeat(27));
    println!("cosmostrix \\");
    println!("  --scene monolith \\");
    println!("  --fps 60 \\");
    println!("  --speed 30 \\");
    println!("  --density 0.85 \\");
    println!("  --color cosmos \\");
    println!("  --charset binary \\");
    println!("  --glitch-level subtle \\");
    println!("  --monolith-size normal");
}

// ---------------------------------------------------------------------------
// --help-detail: curated advanced reference
//
// Design principle: guide, don't dump. No embedded catalogs, no schema dumps,
// no verbose alias disclosures. Discovery commands handle discovery.
// ---------------------------------------------------------------------------

// print_help_detail() moved to src/help_detail.rs
