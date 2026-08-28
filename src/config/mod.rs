// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! CLI argument definitions and help output generators.
//!
//! cosmostrix follows a **curated simplicity** philosophy:
//! - `--help` prints the full curated reference manual (single-tier help)
//! - `--glitch-level` provides a grouped interface over individual tuning knobs
//! - Advanced parameters remain fully functional but are intentionally hidden
//!   from the casual user.

// Submodule declarations: all config*.rs, live_config*.rs, and test
// subdirs now live as siblings under src/config/. Re-exported as `pub`
// so that `pub(crate) use config::*;` in main.rs keeps the 92 existing
// `crate::config_X::Foo` and `crate::live_config_X::Foo` call sites
// working unchanged.
//
// ORDER MATTERS: `live_config_trace` must be declared before `live_config`
// so the `lr_trace!` macro is in scope when live_config.rs is parsed.
// The `#[macro_use]` attribute is defense-in-depth (the macro is also
// `#[macro_export]`-ed from inside live_config_trace itself).
pub mod config_apply;
#[cfg(test)]
pub mod config_apply_tests;
pub mod config_hints;
pub mod config_io;
pub mod configfile;
#[cfg(test)]
pub mod configfile_tests;
#[macro_use]
pub mod live_config_trace;
pub mod live_config;
pub mod live_config_poll;
pub mod live_config_state;

use std::io::IsTerminal;
use std::path::PathBuf;
use std::str::FromStr;

use clap::Parser;

use crate::runtime::MonolithSize;
use crate::scene;
use crate::theme;
use crate::{colors_custom, scene_custom};

/// v50-beta.3: clap value_parser for boolean CLI flags that MUST receive
/// an explicit `true`/`false` value (no bare-flag toggle). This prevents
/// the silent-ignore class of bugs where a user types `--crystal-dragon`
/// expecting an error or a toggle, but clap quietly sets the bool to true.
///
/// Accepted values (case-insensitive): `true`, `false`, `1`, `0`, `yes`,
/// `no`, `on`, `off`. Any other input → clap error.
///
/// Used by: `--crystal-dragon`, `--power-dragon`, `--msg-mode`.
fn parse_true_false(input: &str) -> Result<bool, String> {
    match input.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        other => Err(format!(
            "invalid boolean value '{other}' (expected: true|false|1|0|yes|no|on|off)"
        )),
    }
}

/// Test-only accessor for the `parse_true_false` value_parser. Tests can't
/// reach the private fn directly, so this pub(crate) wrapper exposes it.
#[cfg(test)]
pub(crate) fn test_parse_true_false(input: &str) -> Result<bool, String> {
    parse_true_false(input)
}

#[must_use]
pub(crate) fn color_enabled_stdout() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    if matches!(std::env::var("CLICOLOR").ok().as_deref(), Some("0")) {
        return false;
    }
    std::io::stdout().is_terminal()
}

pub(crate) fn colorize_help(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 64);
    for chunk in text.split_inclusive('\n') {
        let (line, nl) = chunk
            .strip_suffix('\n')
            .map(|l| (l, "\n"))
            .unwrap_or((chunk, ""));

        let is_heading =
            !line.starts_with(' ') && line.ends_with(':') && line == line.to_ascii_uppercase();

        if is_heading {
            // Bold brand purple for section headings (matches --help USAGE:).
            out.push_str(crate::output::brand_bold_open());
            out.push_str(line);
            out.push_str(crate::output::reset());
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

// Enums

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorBg {
    #[value(name = "black")]
    Black,
    // Both "default-background" (kebab-case, canonical CLI form) and
    // "default_background" (snake_case) are accepted by config.toml
    // parsing (configfile.rs, config_apply.rs, profile.rs, live_config.rs,
    // testconf.rs) via explicit match arms. The CLI exposes only the
    // canonical kebab-case name to avoid duplicate entries in error output.
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

/// Which cinematic intro to play before the rain engine takes over.
/// Exposed as a clap `ValueEnum` for CLI parsing and consumed by the
/// runtime intro dispatcher in `crate::interactive::intro`.
///
/// * `Cosmic` — Cosmic Burst: singularity → explosion → morph → rain.
/// * `Logo`   — cosmostrix Logo: fade in → ignition → dissolve → rain.
/// * `None`   — No intro; skip straight to the rain engine.
#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntroType {
    #[value(name = "cosmic")]
    Cosmic,
    #[value(name = "logo")]
    Logo,
    #[value(name = "none")]
    None,
}

// U16Range

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

// Args — curated two-tier help design
//
// VISIBLE args appear in --help (the first impression).
// HIDDEN args are still fully functional but intentionally undocumented.

#[derive(Parser, Debug, Clone)]
#[command(
    name = "cosmostrix",
    version,
    disable_version_flag = true,
    disable_help_flag = true,
    about = env!("CARGO_PKG_DESCRIPTION"),
    after_help = "cosmostrix uses a diff-based rendering engine — only changed cells are redrawn, not the full screen.\nSee `cosmostrix --docs` for the full technical breakdown."
)]
pub struct Args {
    // === COMMON OPTIONS (visible in --help) ===
    #[arg(
        short = 'c',
        long = "color",
        default_value = "green",
        help_heading = "COMMON OPTIONS",
        display_order = 10,
        help = "Color theme or custom palette name (see --list-colors)"
    )]
    pub color: String,

    #[arg(
        long = "colors-custom",
        value_name = "NAME",
        help_heading = "COMMON OPTIONS",
        display_order = 12,
        help = "Load a custom color palette from config (see --list-colors). Equivalent to -c <name> for custom palettes."
    )]
    pub colors_custom: Option<String>,

    #[arg(
        long = "color-tune",
        help_heading = "COMMON OPTIONS",
        display_order = 11,
        help = "Tune theme colors (keys: sat=, bright=, head=, body=, tail=; range 0.0-3.0)"
    )]
    pub color_tune: Option<String>,

    // v30 simplify: --brightness/--saturation skip fields REMOVED.
    // These were v17 ghosts — never read by any consumer. The actual
    // color-tune path uses [color.tune] config / --color-tune CLI, which
    // has its own dedicated field below (`color_tune`).
    #[arg(
        short = 'C',
        long = "charset",
        // depth-test fix: --charset-custom as alias. Depth-test user
        // expected `--charset-custom cat` to work by analogy with
        // --colors-custom and --scene-custom. The existing --charset flag
        // already handles BOTH built-in presets AND custom names (loaded
        // from [charset-custom.<name>] in config.toml), so the alias is
        // pure UX parity — no behavioral difference. Clap's `alias`
        // (not `long_alias`) makes the alternate name visible in --help
        // suggestions and error tips.
        alias = "charset-custom",
        default_value = "binary",
        help_heading = "COMMON OPTIONS",
        display_order = 20,
        help = "Character set (see --list-charsets). Accepts built-in presets or custom names from [charset-custom.<name>]."
    )]
    pub charset: String,

    // v25: --charset-file CLI flag REMOVED. Custom charsets now live in
    // config.toml under [charset-custom.<name>] and are loaded via
    // --charset <name> (or `charset = "<name>"` in config). See
    // --help and `cosmostrix --dump-config` for the new format.
    #[arg(
        short = 'f',
        long = "fps",
        default_value_t = 60.0,
        help_heading = "COMMON OPTIONS",
        display_order = 30,
        help = "Target FPS (interactive mode frame limiter). The loop sleeps \
                between frames to maintain this cap; press 'i' to see it as \
                `tgt:` in the HUD. In --benchmark mode sets simulation rate \
                only — does NOT cap render throughput (avg_fps in the report \
                is unconstrained; check `target_fps` to confirm what you set)."
    )]
    pub fps: f64,

    #[arg(
        short = 'S',
        long = "speed",
        default_value_t = 30.0,
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
        long = "async-mode",
        value_name = "BOOL",
        num_args = 1,
        value_parser = parse_true_false,
        help_heading = "ADVANCED",
        display_order = 57,
        help = "Async variable column speeds (true|false, default: true). false = uniform column speeds"
    )]
    pub async_mode: Option<bool>,

    #[arg(
        short = 's',
        long = "screensaver",
        help_heading = "COMMON OPTIONS",
        display_order = 60,
        help = "Screensaver mode: only q exits (all other input ignored)"
    )]
    pub screensaver: bool,

    #[arg(
        long = "intro",
        help_heading = "COMMON OPTIONS",
        display_order = 61,
        value_enum,
        num_args = 0..=1,
        default_missing_value = "logo",
        help = "Show cinematic intro before rain begins (cosmic|logo|none, default: logo)"
    )]
    pub intro: Option<IntroType>,

    /// v50: intro color override. CLI flag `--intro-color <name>` accepts
    /// any builtin theme name (see --list-colors) or custom palette name.
    /// Also configurable via `intro-color = "energy-zen"` in config.toml.
    /// When set, the intro animation uses this color theme instead of
    /// the rain color.
    #[arg(
        long = "intro-color",
        value_name = "NAME",
        help_heading = "COMMON OPTIONS",
        display_order = 62,
        help = "Intro color override (builtin theme name or custom palette, see --list-colors)"
    )]
    pub intro_color: Option<String>,

    // v17 mastery: --mouse flag DELETED. Mouse hover/click visual effects are
    // now ALWAYS ON (cursor glow + strong dual-ring click wave). Mouse reporting
    // is also always on (blocks text selection). No flag needed — the effect
    // is part of cosmostrix's signature interactive experience.
    #[arg(
        short = 'm',
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
        help = "Apply a built-in scene (curated color + charset + speed + density, see --list-scenes)"
    )]
    pub scene: Option<String>,

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
        num_args = 0..=1,
        default_missing_value = "",
        help = "Print example config to stdout, or write to <path> (whitelist-enforced)"
    )]
    pub dump_config: Option<String>,

    // v30 (2026-08-05): --force flag. Currently affects --dump-config ONLY
    // (allows overwriting an existing file at the target path). Other write
    // operations (--save-baseline, etc.) have their own per-flag overwrite
    // policy and are not affected by --force. Documented as scoped to make
    // the contract explicit — users should not assume --force is a global
    // "yes to all prompts" flag.
    #[arg(
        long = "force",
        help_heading = "CONFIG",
        display_order = 100,
        help = "Force overwrite when writing files. Currently affects \
                --dump-config ONLY: allows overwriting an existing file \
                at the target path. Other write operations are unaffected."
    )]
    pub force: bool,

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
        help = "Validate config.toml and report errors"
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
        long = "docs",
        help_heading = "DIAGNOSTICS",
        display_order = 105,
        help = "Print engine documentation and architecture overview"
    )]
    pub docs: bool,

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
        help = "Benchmark duration (default 5s). Accepts: 5, 6s, 30m, 1h30m"
    )]
    pub bench_duration: Option<String>,

    #[arg(
        long = "screen-size",
        help_heading = "DIAGNOSTICS",
        display_order = 113,
        help = "Fixed screen size WxH (e.g. 120x40). Min 1x1, \
                max 1024x500 interactive / 7680x4320 (8K UHD) bench"
    )]
    pub screen_size: Option<String>,

    #[arg(
        long = "json",
        help_heading = "DIAGNOSTICS",
        display_order = 112,
        help = "Output benchmark as JSON (use with --benchmark)"
    )]
    pub json: bool,

    #[arg(
        long = "save-baseline",
        help_heading = "DIAGNOSTICS",
        display_order = 114,
        help = "Save benchmark JSON to file (whitelist-enforced)"
    )]
    pub save_baseline: Option<String>,

    #[arg(
        long = "compare-baseline",
        help_heading = "DIAGNOSTICS",
        display_order = 115,
        help = "Compare benchmark against saved baseline JSON"
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
        long = "bench-scene",
        value_name = "NAME",
        value_parser = ["lean", "production-draw"],
        help_heading = "DIAGNOSTICS",
        display_order = 118,
        help = "Benchmark I/O scene: 'lean' (default, emit_cell_lean) or \
                'production-draw' (mirrors Terminal::draw full-redraw path — \
                MoveTo per row + ColorCache SGR + BOLT bold escape). Use \
                'production-draw' to measure the BOLT-backed production \
                render path; pair with --bench-io to write ANSI to /dev/null. \
                Strict: typos are rejected at parse time, not silently \
                fallback'd to the default lean path."
    )]
    pub bench_scene: Option<String>,

    // v30 simplify: --info skip field REMOVED. Was a v17 ghost (CLI flag
    // deleted in v17, merged into --doctor). No consumer ever read this.
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
        long = "list-charsets",
        help_heading = "DISCOVERY",
        display_order = 210,
        help = "Show available charset presets"
    )]
    pub list_charsets: bool,

    #[arg(
        long = "list-scenes",
        help_heading = "DISCOVERY",
        display_order = 230,
        help = "Show available built-in and custom scenes"
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

    // === HELP (visible in --help) ===
    //
    // v30 simplify: --help-detail was merged into --help. The curated
    // advanced reference manual that --help-detail used to print is now
    // printed by --help itself. cosmostrix now has a single-tier help
    // surface: --help is the full reference, not a stripped-down summary.
    //
    // disable_help_flag = true on the #[command] macro prevents clap from
    // auto-generating its own --help (which would only print the {all-args}
    // auto-list). We define our own --help field below and intercept it in
    // main.rs to call help_detail::print_help().
    #[arg(
        long = "help",
        short = 'h',
        help_heading = "HELP",
        display_order = 300,
        help = "Print the full reference manual"
    )]
    pub help: bool,

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
        help_heading = "HELP",
        display_order = 330,
        help = "Check the latest upstream release"
    )]
    pub check_update: bool,

    #[arg(
        long = "verbose",
        short = 'v',
        help_heading = "DIAGNOSTICS",
        display_order = 130,
        help = "Print diagnostic info to stderr (for debugging)"
    )]
    pub verbose: bool,

    // v50-beta.3: --async-mode CLI flag now exists (was config-only).
    // See the ADVANCED section above for the flag definition.
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
        help = "Run headless benchmark for N frames and exit",
        // Phase 5 (P3-6): reject 0 — a 0-frame benchmark produces a 0-FPS
        // report with warmup running, which looks like a real measurement.
        // value_parser range(1..) makes clap reject --bench-frames 0 at
        // parse time with a clear error before any allocation or warmup.
        value_parser = clap::value_parser!(u64).range(1..)
    )]
    pub bench_frames: Option<u64>,

    /// Crystal Dragon Engine: ambient intelligence for palette drift.
    ///
    /// Maps system state (CPU or clock) to a temperature group (Cold/Medium/Hot)
    /// and selects color themes via probabilistic weighted calculation.
    /// Polls every 60 seconds with 300ms OKLab smooth transitions.
    /// v50-beta.3: CLI flag accepts explicit `true`/`false` value.
    /// Bare `--crystal-dragon` (no value) errors to prevent silent toggle.
    #[arg(
        long = "crystal-dragon",
        value_name = "BOOL",
        num_args = 1,
        value_parser = parse_true_false,
        help = "Enable Crystal Dragon ambient color drift (true|false, default: false)"
    )]
    pub crystal_dragon: Option<bool>,

    /// v50: Power Dragon toggle. CLI flag `--power-dragon <true|false>`.
    /// When false: disables aggressive_throttle + idle FPS reduction.
    /// Default: true (protection enabled). Also configurable via
    /// `power-dragon = false` in config.toml. When false, rain stays
    /// at user-configured density/speed regardless of CPU pressure.
    /// v50-beta.3: bare `--power-dragon` (no value) errors to prevent
    /// silent toggle (was bool flag, now requires explicit true/false).
    #[arg(
        long = "power-dragon",
        value_name = "BOOL",
        num_args = 1,
        value_parser = parse_true_false,
        help = "Power Dragon adaptive protection (true|false, default: true)"
    )]
    pub power_dragon: Option<bool>,

    /// v50-beta.3: msg-mode toggle. CLI flag `--msg-mode <true|false>`.
    /// Master switch for the message overlay subsystem. When false,
    /// disables BOTH the default message AND any `message`/`message-border`
    /// config key. CLI `-m`/`-mb` always wins over this (CLI precedence).
    /// Default: true (message overlay active). Also configurable via
    /// `msg-mode = false` in config.toml.
    /// Bare `--msg-mode` (no value) errors to prevent silent toggle.
    #[arg(
        long = "msg-mode",
        value_name = "BOOL",
        num_args = 1,
        value_parser = parse_true_false,
        help = "Message overlay master switch (true|false, default: true)"
    )]
    pub msg_mode: Option<bool>,

    /// PERF-4: disable ALL particle effects (quantum ripple, border spark,
    /// mouse-click flash waves, anomaly zones). CLI-only, no config. When set,
    /// spawn_quantum_ripple + spawn_border_spark are no-ops, set_mouse_click
    /// skips flash-wave activation, and spawn_anomaly returns early. Useful
    /// for VTE terminals where particle effects cause lag.
    /// Default: false (effects on).
    ///
    /// Renamed from --disable-effects to --no-effects in v50.0.0-beta.7 for
    /// CLI ergonomics (mirrors --no-color / --no-border convention). Typing
    /// --disable-effects now triggers clap's built-in "did you mean?" hint
    /// (enabled via the `suggestions` clap feature in Cargo.toml).
    #[arg(
        long = "no-effects",
        help = "Disable all particle effects (quantum ripple, border spark, click flash, anomaly zones) — useful for slow terminals"
    )]
    pub no_effects: bool,

    // Helper: default true for power_dragon (clap defaults bool to false,
    // so we set it true in main.rs after parse).
    // The config_apply.rs parse path overrides this when config.toml
    // has `power-dragon = false`.
    #[arg(
        short = 'g',
        long = "glitchms",
        default_value = "300,400",
        hide = true,
        help = "Glitch duration range in ms: LOW,HIGH (min 1 max 5000)"
    )]
    pub glitch_ms: U16Range,

    // v17 mastery: --glitchpct CLI flag REMOVED. Use --glitch-level instead.
    // Field kept for internal use (set by glitch_level preset via config_apply).
    #[arg(skip = 10.0_f32)]
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

    // Hidden boolean: set only by the -mb argv pre-expansion in main.rs.
    // No long form exposed; users interact via -m / -mb exclusively.
    #[arg(
        long = "message-border",
        hide = true,
        help = "(internal) message box border flag; use -mb instead"
    )]
    pub message_border: bool,

    // v17 mastery: --maxdpc CLI flag REMOVED. Use --glitch-level instead.
    #[arg(skip = 3u8)]
    pub max_droplets_per_column: u8,

    // v30 simplify: --noglitch CLI flag REMOVED. Was a strict duplicate of
    // `--glitch-level none` (the only behavior `--noglitch` had was to flip
    // `cloud.glitchy` to false, which is exactly what `--glitch-level none`
    // does). The `noglitch` field is replaced by `glitch_enabled` (positive
    // polarity) on CloudConfig, derived from `glitch_level != GlitchLevel::None`.
    // See REMOVED_FLAGS in src/validation.rs for the migration message.

    // v17 mastery: --rippct / -r CLI flag REMOVED. Use --glitch-level instead.
    #[arg(skip = 33.33333_f32)]
    pub rippct: f32,

    // v17 mastery: --shortpct CLI flag REMOVED. Use --glitch-level instead.
    #[arg(skip = 50.0_f32)]
    pub shortpct: f32,

    #[arg(
        long = "colormode",
        hide = true,
        help = "Force color mode (allowed: 0,16,8/256,24/32). Default: 24-bit if supported (COLORTERM), else 8-bit (TERM=...256color), else 16-color"
    )]
    pub colormode: Option<u16>,
}

// List printers — clean, no alias noise

pub(crate) fn print_list_charsets() {
    if color_enabled_stdout() {
        println!(
            "{}AVAILABLE CHARSET PRESETS:{}",
            crate::output::brand_bold_open(),
            crate::output::reset()
        );
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
    println!("  zen          Pipe character only (default for cinematic & monolith)");
    println!();
    println!("  Or define a custom charset in config.toml via [charset-custom.<name>] (see --dump-config).");

    // v25: Show custom charsets from config (if any).
    let cfg = configfile::load_config_file(None);
    let custom_charsets = crate::charset_custom::collect_charset_custom(&cfg);
    if !custom_charsets.is_empty() {
        println!();
        if color_enabled_stdout() {
            println!(
                "{}CUSTOM CHARACTER SETS (from config):{}",
                crate::output::brand_bold_open(),
                crate::output::reset()
            );
        } else {
            println!("CUSTOM CHARACTER SETS (from config):");
        }
        println!();
        for (name, def) in &custom_charsets {
            println!("  {name:<20} {} chars", def.chars.len());
        }
        println!();
        println!("  Load with: cosmostrix -C/--charset/--charset-custom <name>");
        println!("  Or set in config: charset = \"<name>\"");
    }
}

pub(crate) fn print_list_colors() {
    if color_enabled_stdout() {
        println!(
            "{}AVAILABLE COLOR THEMES:{}",
            crate::output::brand_bold_open(),
            crate::output::reset()
        );
    } else {
        println!("AVAILABLE COLOR THEMES:");
    }
    println!();
    print!("{}", theme::compact_list_text());
    println!();
    println!("{} built-in themes.", theme::theme_count());

    // v16: Show custom color palettes from config (if any).
    let cfg = configfile::load_config_file(None);
    let custom_palettes = colors_custom::collect_colors_custom(&cfg);
    if !custom_palettes.is_empty() {
        println!();
        if color_enabled_stdout() {
            println!(
                "{}CUSTOM COLOR PALETTES (from config):{}",
                crate::output::brand_bold_open(),
                crate::output::reset()
            );
        } else {
            println!("CUSTOM COLOR PALETTES (from config):");
        }
        println!();
        for name in custom_palettes.keys() {
            println!("  {name:<20} custom palette");
        }
        println!();
        println!("  Load with: cosmostrix -c/--color/--colors-custom <name>");
        println!("  Use in ambient: ambient.HH-MM = <name>");
    }
}

pub(crate) fn print_list_scenes() {
    if color_enabled_stdout() {
        println!(
            "{}AVAILABLE SCENES:{}",
            crate::output::brand_bold_open(),
            crate::output::reset()
        );
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
            println!(
                "{}CUSTOM SCENES (from config):{}",
                crate::output::brand_bold_open(),
                crate::output::reset()
            );
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
pub(crate) fn print_show_scene(
    name: &str,
    cfg: &std::collections::HashMap<String, String>,
) -> Result<(), String> {
    // 1. Built-in scene lookup.
    if let Some(info) = scene::get_scene(name) {
        print!("{}", scene::show_scene_text(info));
        return Ok(());
    }

    // 2. Custom scene lookup (scene-custom namespace only — removed
    //    the [profile.<name>] fallback; users must rename the prefix).
    let custom_scenes = scene_custom::collect_custom_scenes(cfg);
    let normalized = name.trim().to_ascii_lowercase();
    if let Some(custom) = custom_scenes.get(&normalized) {
        print!(
            "{}",
            scene_custom::show_custom_scene_text(&normalized, custom)
        );
        return Ok(());
    }

    // 3. Not found.
    let mut available: Vec<String> = scene::all_scene_names()
        .iter()
        .map(|s| s.to_string())
        .collect();
    available.extend(custom_scenes.keys().cloned());
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

// --help: curated full reference manual
//
// Design principle: guide, don't dump. No embedded catalogs, no schema dumps,
// no verbose alias disclosures. Discovery commands handle discovery.
//
// print_help() lives in src/cli/help_detail.rs.
