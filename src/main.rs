// Copyright (c) 2026 rezky_nightky

mod bench;
mod cell;
mod charset;
mod cloud;
mod config;
mod configfile;
mod constants;
mod doctor;
mod droplet;
mod frame;
mod interactive;
mod palette;
mod runtime;
mod terminal;
mod validation;

use std::env;

#[cfg(target_os = "linux")]
use std::io::IsTerminal;

#[cfg(unix)]
use clap::builder::styling::{AnsiColor as ClapAnsiColor, Color as ClapColor};
use clap::builder::styling::{Effects as ClapEffects, Style as ClapStyle};
use clap::builder::Styles as ClapStyles;
use clap::parser::ValueSource;
use clap::{CommandFactory, FromArgMatches};

use crate::charset::{build_chars, charset_from_str, parse_user_hex_chars};
use crate::cloud::Cloud;
use crate::config::{
    color_enabled_stdout, default_params_usage_for_help, print_help_detail, print_list_charsets,
    print_list_colors, Args, ColorBg,
};
use crate::constants::*;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};
use crate::terminal::restore_terminal_best_effort;
use crate::validation::{
    validate_f32_range, validate_f64_range, validate_u16_range, validate_u8_range,
};

// --- Named constants are centralized in constants.rs ---

const HELP_TEMPLATE_PLAIN: &str = "\
{before-help}{about-with-newline}
USAGE:
  {usage}

{all-args}{after-help}";

const HELP_TEMPLATE_COLOR: &str = "\
{before-help}{about-with-newline}
\x1b[1;36mUSAGE:\x1b[0m
  {usage}

{all-args}{after-help}";

// --- CloudConfig struct for deduplicating cloud initialization ---

/// Aggregated configuration for creating and running a `Cloud` instance.
/// Collected from CLI args and config file, then passed to the interactive
/// loop or benchmark runner.
pub struct CloudConfig {
    pub color_mode: ColorMode,
    pub fullwidth: bool,
    pub shading_mode: ShadingMode,
    pub bold_mode: BoldMode,
    pub async_mode: bool,
    pub default_bg: bool,
    pub color_scheme: ColorScheme,
    pub noglitch: bool,
    pub glitch_pct: f32,
    pub glitch_low: u16,
    pub glitch_high: u16,
    pub linger_low: u16,
    pub linger_high: u16,
    pub short_pct: f32,
    pub die_early_pct: f32,
    pub max_dpc: u8,
    pub density: f32,
    pub speed: f32,
    pub chars: Vec<char>,
    pub message: Option<String>,
    pub message_no_border: bool,
    pub target_fps: f64,
    pub duration: Option<f64>,
    pub duration_s: Option<f64>,
    pub bench_frames: Option<u64>,
    pub density_auto: bool,
    pub base_density: f32,
    pub perf_stats: bool,
    pub screensaver: bool,
    pub charset_preset: String,
    pub user_ranges: Vec<(char, char)>,
    pub def_ascii: bool,
}

impl CloudConfig {
    pub fn create_cloud(&self, density: f32) -> Cloud {
        let mut cloud = Cloud::new(
            self.color_mode,
            self.fullwidth,
            self.shading_mode,
            self.bold_mode,
            self.async_mode,
            self.default_bg,
            self.color_scheme,
        );

        cloud.glitchy = !self.noglitch;
        cloud.set_glitch_pct(self.glitch_pct / 100.0);
        cloud.set_glitch_times(self.glitch_low, self.glitch_high);
        cloud.set_linger_times(self.linger_low, self.linger_high);
        cloud.short_pct = self.short_pct / 100.0;
        cloud.die_early_pct = self.die_early_pct / 100.0;
        cloud.set_max_droplets_per_column(self.max_dpc);
        cloud.set_droplet_density(density);
        cloud.set_chars_per_sec(self.speed);

        cloud.init_chars(self.chars.clone());
        cloud.reset(DENSITY_AUTO_DEFAULT_COLS, DENSITY_AUTO_DEFAULT_LINES);

        if let Some(msg) = &self.message {
            cloud.set_message_border(!self.message_no_border);
            cloud.set_message(msg);
        }

        cloud
    }
}

// --- Helper functions (shared across modules) ---

#[must_use]
fn build_info() -> &'static str {
    env!("COSMOSTRIX_BUILD")
}

#[must_use]
fn build_commit_short() -> Option<&'static str> {
    match option_env!("COSMOSTRIX_GIT_SHA") {
        Some(s) if !s.is_empty() => Some(s),
        _ => None,
    }
}

#[must_use]
pub fn env_var_truthy(name: &str) -> bool {
    match env::var(name) {
        Ok(v) => {
            let v = v.trim();
            if v.is_empty() {
                return false;
            }
            let v = v.to_ascii_lowercase();
            !(v == "0" || v == "false" || v == "no" || v == "off")
        }
        Err(env::VarError::NotPresent) => false,
        Err(env::VarError::NotUnicode(_)) => true,
    }
}

#[must_use]
#[cfg(unix)]
fn clap_styles() -> ClapStyles {
    ClapStyles::styled()
        .header(
            ClapStyle::new()
                .effects(ClapEffects::BOLD)
                .fg_color(Some(ClapColor::Ansi(ClapAnsiColor::Cyan))),
        )
        .usage(
            ClapStyle::new()
                .effects(ClapEffects::BOLD)
                .fg_color(Some(ClapColor::Ansi(ClapAnsiColor::Green))),
        )
        .literal(ClapStyle::new().fg_color(Some(ClapColor::Ansi(ClapAnsiColor::Yellow))))
        .placeholder(ClapStyle::new().fg_color(Some(ClapColor::Ansi(ClapAnsiColor::Magenta))))
}

#[cfg(target_os = "linux")]
pub fn spawn_kill9_terminal_guard() {
    if env_var_truthy("COSMOSTRIX_NO_FORK_GUARD") {
        return;
    }

    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return;
    }

    unsafe {
        let mut orig: std::mem::MaybeUninit<libc::termios> = std::mem::MaybeUninit::uninit();
        if libc::tcgetattr(libc::STDIN_FILENO, orig.as_mut_ptr()) != 0 {
            return;
        }
        let orig = orig.assume_init();

        let pid = libc::fork();
        if pid != 0 {
            return;
        }

        // Initialize sigset_t via MaybeUninit — sigemptyset will fully
        // initialize it, so this is safe.
        let mut set = std::mem::MaybeUninit::<libc::sigset_t>::uninit();
        libc::sigemptyset(set.as_mut_ptr());
        libc::sigaddset(set.as_mut_ptr(), libc::SIGTERM);
        let _ = libc::pthread_sigmask(libc::SIG_BLOCK, set.as_ptr(), std::ptr::null_mut());
        let set = set.assume_init();

        let _ = libc::prctl(
            libc::PR_SET_NAME,
            c"cx-term-guard".as_ptr() as usize,
            0,
            0,
            0,
        );
        let _ = libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM, 0, 0, 0);

        if libc::getppid() == 1 {
            let _ = libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &orig);
            restore_terminal_best_effort();
            libc::_exit(0);
        }

        let mut sig: libc::c_int = 0;
        let _ = libc::sigwait(&set, &mut sig);
        if sig == libc::SIGTERM {
            let _ = libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &orig);
            restore_terminal_best_effort();
        }

        libc::_exit(0);
    }
}

#[must_use]
fn default_to_ascii() -> bool {
    let lang = env::var("LANG").unwrap_or_default();
    !lang.to_ascii_uppercase().contains("UTF")
}

#[must_use]
fn detect_color_mode_auto() -> ColorMode {
    let colorterm = env::var("COLORTERM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if colorterm.contains("truecolor") || colorterm.contains("24bit") {
        return ColorMode::TrueColor;
    }

    #[cfg(windows)]
    {
        if env::var_os("WT_SESSION").is_some() {
            return ColorMode::TrueColor;
        }
    }

    let term = env::var("TERM").unwrap_or_default().to_ascii_lowercase();
    if term == "dumb" {
        return ColorMode::Mono;
    }
    if term.contains("-truecolor") {
        return ColorMode::TrueColor;
    }
    if term.contains("256color") {
        return ColorMode::Color256;
    }

    ColorMode::Color16
}

pub fn detect_color_mode(args: &Args) -> ColorMode {
    if let Some(m) = args.colormode {
        return match m {
            0 => ColorMode::Mono,
            16 => ColorMode::Color16,
            8 | 256 => ColorMode::Color256,
            24 | 32 => ColorMode::TrueColor,
            _ => {
                eprintln!("invalid --colormode: {} (allowed: 0,16,8/256,24/32)", m);
                std::process::exit(1);
            }
        };
    }

    detect_color_mode_auto()
}

#[must_use]
pub fn color_mode_label(m: ColorMode) -> &'static str {
    match m {
        ColorMode::TrueColor => "24-bit truecolor",
        ColorMode::Color256 => "8-bit (256-color)",
        ColorMode::Mono => "mono",
        ColorMode::Color16 => "16-color",
    }
}

#[must_use]
fn all_color_schemes() -> &'static [ColorScheme] {
    &[
        ColorScheme::Green,
        ColorScheme::Green2,
        ColorScheme::Green3,
        ColorScheme::Yellow,
        ColorScheme::Orange,
        ColorScheme::Red,
        ColorScheme::Blue,
        ColorScheme::Cyan,
        ColorScheme::Gold,
        ColorScheme::Rainbow,
        ColorScheme::Purple,
        ColorScheme::Neon,
        ColorScheme::Fire,
        ColorScheme::Ocean,
        ColorScheme::Forest,
        ColorScheme::Vaporwave,
        ColorScheme::Gray,
        ColorScheme::Snow,
        ColorScheme::Aurora,
        ColorScheme::FancyDiamond,
        ColorScheme::Cosmos,
        ColorScheme::Nebula,
        ColorScheme::Spectrum20,
        ColorScheme::Stars,
        ColorScheme::Mars,
        ColorScheme::Venus,
        ColorScheme::Mercury,
        ColorScheme::Jupiter,
        ColorScheme::Saturn,
        ColorScheme::Uranus,
        ColorScheme::Neptune,
        ColorScheme::Pluto,
        ColorScheme::Moon,
        ColorScheme::Sun,
        ColorScheme::Comet,
        ColorScheme::Galaxy,
        ColorScheme::Supernova,
        ColorScheme::BlackHole,
        ColorScheme::Andromeda,
        ColorScheme::Stardust,
        ColorScheme::Meteor,
        ColorScheme::Eclipse,
        ColorScheme::DeepSpace,
    ]
}

#[must_use]
fn cycle_color_scheme(current: ColorScheme, dir: i32) -> ColorScheme {
    let list = all_color_schemes();
    let Some(pos) = list.iter().position(|&c| c == current) else {
        return ColorScheme::Green;
    };

    let n = list.len() as i32;
    let mut idx = pos as i32 + dir;
    idx = ((idx % n) + n) % n;
    list[idx as usize]
}

#[must_use]
fn all_charset_presets() -> &'static [&'static str] {
    &[
        "auto",
        "matrix",
        "ascii",
        "extended",
        "english",
        "digits",
        "punc",
        "binary",
        "hex",
        "katakana",
        "greek",
        "cyrillic",
        "hebrew",
        "blocks",
        "symbols",
        "arrows",
        "retro",
        "cyberpunk",
        "hacker",
        "minimal",
        "code",
        "dna",
        "braille",
        "runic",
    ]
}

#[must_use]
fn normalize_charset_preset_name(s: &str) -> String {
    match s.trim().to_ascii_lowercase().as_str() {
        "bin" | "01" => "binary".to_string(),
        "dec" | "decimal" => "digits".to_string(),
        "hexadecimal" => "hex".to_string(),
        other => other.to_string(),
    }
}

#[must_use]
fn cycle_charset_preset(current: &str, dir: i32) -> &'static str {
    let list = all_charset_presets();
    let Some(pos) = list.iter().position(|&c| c == current) else {
        return "binary";
    };

    let n = list.len() as i32;
    let mut idx = pos as i32 + dir;
    idx = ((idx % n) + n) % n;
    list[idx as usize]
}

#[must_use]
pub fn auto_density_factor(cols: u16, lines: u16, fullwidth: bool) -> f32 {
    let eff_cols = if fullwidth {
        (cols / 2).max(1)
    } else {
        cols.max(1)
    } as f32;
    let eff_lines = lines.max(1) as f32;

    let area = eff_cols * eff_lines;
    let base = DENSITY_BASE_COLS * DENSITY_BASE_LINES;
    let factor = (area / base).sqrt();
    factor.clamp(DENSITY_AUTO_MIN, DENSITY_AUTO_MAX)
}

#[must_use]
pub fn effective_density(base: f32, cols: u16, lines: u16, fullwidth: bool, auto: bool) -> f32 {
    let base = base.clamp(DENSITY_CLAMP_MIN, DENSITY_CLAMP_MAX);
    if !auto {
        return base;
    }
    (base * auto_density_factor(cols, lines, fullwidth)).clamp(DENSITY_CLAMP_MIN, DENSITY_CLAMP_MAX)
}

// --- HashMap-based color scheme parser ---

use std::collections::HashMap;
use std::sync::LazyLock;

static COLOR_SCHEME_MAP: LazyLock<HashMap<&'static str, ColorScheme>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("green", ColorScheme::Green);
    m.insert("green2", ColorScheme::Green2);
    m.insert("green3", ColorScheme::Green3);
    m.insert("yellow", ColorScheme::Yellow);
    m.insert("orange", ColorScheme::Orange);
    m.insert("red", ColorScheme::Red);
    m.insert("blue", ColorScheme::Blue);
    m.insert("cyan", ColorScheme::Cyan);
    m.insert("gold", ColorScheme::Gold);
    m.insert("rainbow", ColorScheme::Rainbow);
    m.insert("purple", ColorScheme::Purple);
    m.insert("neon", ColorScheme::Neon);
    m.insert("synthwave", ColorScheme::Neon);
    m.insert("fire", ColorScheme::Fire);
    m.insert("inferno", ColorScheme::Fire);
    m.insert("ocean", ColorScheme::Ocean);
    m.insert("deep-sea", ColorScheme::Ocean);
    m.insert("deep_sea", ColorScheme::Ocean);
    m.insert("deepsea", ColorScheme::Ocean);
    m.insert("forest", ColorScheme::Forest);
    m.insert("jungle", ColorScheme::Forest);
    m.insert("vaporwave", ColorScheme::Vaporwave);
    m.insert("gray", ColorScheme::Gray);
    m.insert("grey", ColorScheme::Gray);
    m.insert("snow", ColorScheme::Snow);
    m.insert("aurora", ColorScheme::Aurora);
    m.insert("fancy-diamond", ColorScheme::FancyDiamond);
    m.insert("fancy_diamond", ColorScheme::FancyDiamond);
    m.insert("fancydiamond", ColorScheme::FancyDiamond);
    m.insert("cosmos", ColorScheme::Cosmos);
    m.insert("nebula", ColorScheme::Nebula);
    m.insert("spectrum20", ColorScheme::Spectrum20);
    m.insert("spectrum-20", ColorScheme::Spectrum20);
    m.insert("spectrum_20", ColorScheme::Spectrum20);
    m.insert("theme20", ColorScheme::Spectrum20);
    m.insert("theme-20", ColorScheme::Spectrum20);
    m.insert("theme_20", ColorScheme::Spectrum20);
    m.insert("stars", ColorScheme::Stars);
    m.insert("star", ColorScheme::Stars);
    m.insert("mars", ColorScheme::Mars);
    m.insert("venus", ColorScheme::Venus);
    m.insert("mercury", ColorScheme::Mercury);
    m.insert("jupiter", ColorScheme::Jupiter);
    m.insert("saturn", ColorScheme::Saturn);
    m.insert("uranus", ColorScheme::Uranus);
    m.insert("neptune", ColorScheme::Neptune);
    m.insert("pluto", ColorScheme::Pluto);
    m.insert("moon", ColorScheme::Moon);
    m.insert("sun", ColorScheme::Sun);
    m.insert("comet", ColorScheme::Comet);
    m.insert("galaxy", ColorScheme::Galaxy);
    m.insert("supernova", ColorScheme::Supernova);
    m.insert("super-nova", ColorScheme::Supernova);
    m.insert("super_nova", ColorScheme::Supernova);
    m.insert("blackhole", ColorScheme::BlackHole);
    m.insert("black-hole", ColorScheme::BlackHole);
    m.insert("black_hole", ColorScheme::BlackHole);
    m.insert("andromeda", ColorScheme::Andromeda);
    m.insert("stardust", ColorScheme::Stardust);
    m.insert("star-dust", ColorScheme::Stardust);
    m.insert("star_dust", ColorScheme::Stardust);
    m.insert("meteor", ColorScheme::Meteor);
    m.insert("eclipse", ColorScheme::Eclipse);
    m.insert("deepspace", ColorScheme::DeepSpace);
    m.insert("deep-space", ColorScheme::DeepSpace);
    m.insert("deep_space", ColorScheme::DeepSpace);
    m
});

fn parse_color_scheme(s: &str) -> Result<ColorScheme, String> {
    let key = s.trim().to_ascii_lowercase();
    COLOR_SCHEME_MAP
        .get(key.as_str())
        .copied()
        .ok_or_else(|| format!("invalid color: {} (see --list-colors)", s))
}

// --- Memory budget estimation ---

#[must_use]
fn estimate_memory_budget(w: u16, h: u16) -> usize {
    // Use actual Cell size rather than a magic number for accuracy
    let cell_size = std::mem::size_of::<crate::cell::Cell>();
    let frame_cells = (w as usize) * (h as usize) * cell_size;

    // Cloud internal buffers: char_pool (2048), glitch_pool (1024), color_map, glitch_map
    let cloud_pools = 2048 * 4 + 1024 * 4;
    let cloud_maps = (w as usize) * (h as usize) * 2; // color_map + glitch_map

    // Droplets: ~1.5 * cols droplets, each ~100 bytes
    let droplet_count = (1.5 * w as f32) as usize;
    let droplets_size = droplet_count * std::mem::size_of::<crate::droplet::Droplet>().max(100);

    // Terminal: LastFrame + row_dirty + touched_rows
    let terminal_last = (w as usize) * (h as usize) * cell_size;

    frame_cells * 2 + cloud_pools + cloud_maps + droplets_size + terminal_last
}

#[must_use]
fn format_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    }
}

// --- Helper to convert String errors to io::Error for main() ---

/// Convert a `Result<T, String>` validation error to `io::Error`.
/// Side effect: restores the terminal and prints the error message to stderr
/// before returning the error, so the user doesn't see a broken terminal.
fn validate_err<T>(name: &str, r: Result<T, String>) -> std::io::Result<T> {
    r.map_err(|e| {
        restore_terminal_best_effort();
        eprintln!("{}", e);
        std::io::Error::new(std::io::ErrorKind::InvalidInput, name)
    })
}

// --- Config file defaults integration ---

/// Apply config file defaults to CLI args that were not explicitly provided.
///
/// All numeric values are validated against the same ranges enforced for CLI
/// arguments, so a malformed config file cannot cause panics (e.g. fps=0
/// leading to division-by-zero) or out-of-range behaviour.
fn apply_config_defaults(matches: &clap::ArgMatches, args: &mut Args) {
    use clap::parser::ValueSource;

    let cfg = configfile::load_config_file();
    if cfg.is_empty() {
        return;
    }

    // Only override args that were not explicitly provided (still at default)
    let apply = |key: &str, matches: &clap::ArgMatches| -> Option<String> {
        if matches.value_source(key) == Some(ValueSource::DefaultValue) {
            cfg.get(key).cloned()
        } else {
            None
        }
    };

    if let Some(v) = apply("color", matches) {
        args.color = v;
    }
    if let Some(v) = apply("charset", matches) {
        args.charset = v;
    }
    if let Some(v) = apply("fps", matches) {
        match v.parse::<f64>() {
            Ok(n) if n.is_finite() => {
                if let Ok(f) = validate_f64_range("config fps", n, 1.0, 240.0) {
                    args.fps = f;
                } else {
                    eprintln!("config: ignoring invalid fps={v} (min 1 max 240)");
                }
            }
            _ => eprintln!("config: ignoring unparseable fps='{v}' (expected a number)"),
        }
    }
    if let Some(v) = apply("speed", matches) {
        match v.parse::<f32>() {
            Ok(n) if n.is_finite() => {
                if let Ok(f) = validate_f32_range("config speed", n, 0.001, 1000.0) {
                    args.speed = f;
                } else {
                    eprintln!("config: ignoring invalid speed={v} (min 0.001 max 1000)");
                }
            }
            _ => eprintln!("config: ignoring unparseable speed='{v}' (expected a number)"),
        }
    }
    if let Some(v) = apply("density", matches) {
        match v.parse::<f32>() {
            Ok(n) if n.is_finite() => {
                if let Ok(f) =
                    validate_f32_range("config density", n, DENSITY_CLAMP_MIN, DENSITY_CLAMP_MAX)
                {
                    args.density = f;
                } else {
                    eprintln!("config: ignoring invalid density={v} (min {DENSITY_CLAMP_MIN} max {DENSITY_CLAMP_MAX})");
                }
            }
            _ => eprintln!("config: ignoring unparseable density='{v}' (expected a number)"),
        }
    }
    if let Some(v) = apply("bold", matches) {
        match v.parse::<u8>() {
            Ok(n) => {
                if let Ok(valid) = validate_u8_range("config bold", n, 0, 2) {
                    args.bold = valid;
                } else {
                    eprintln!("config: ignoring invalid bold={v} (min 0 max 2)");
                }
            }
            _ => eprintln!("config: ignoring unparseable bold='{v}' (expected integer 0-2)"),
        }
    }
    if let Some(v) = apply("shadingmode", matches) {
        match v.parse::<u8>() {
            Ok(n) => {
                if let Ok(valid) = validate_u8_range("config shadingmode", n, 0, 1) {
                    args.shading_mode = valid;
                } else {
                    eprintln!("config: ignoring invalid shadingmode={v} (min 0 max 1)");
                }
            }
            _ => eprintln!("config: ignoring unparseable shadingmode='{v}' (expected integer 0-1)"),
        }
    }
    if let Some(v) = apply("glitchpct", matches) {
        match v.parse::<f32>() {
            Ok(n) if n.is_finite() => {
                if let Ok(f) = validate_f32_range("config glitchpct", n, 0.0, 100.0) {
                    args.glitch_pct = f;
                } else {
                    eprintln!("config: ignoring invalid glitchpct={v} (min 0 max 100)");
                }
            }
            _ => eprintln!("config: ignoring unparseable glitchpct='{v}' (expected a number)"),
        }
    }
    if let Some(v) = apply("shortpct", matches) {
        match v.parse::<f32>() {
            Ok(n) if n.is_finite() => {
                if let Ok(f) = validate_f32_range("config shortpct", n, 0.0, 100.0) {
                    args.shortpct = f;
                } else {
                    eprintln!("config: ignoring invalid shortpct={v} (min 0 max 100)");
                }
            }
            _ => eprintln!("config: ignoring unparseable shortpct='{v}' (expected a number)"),
        }
    }
    if let Some(v) = apply("rippct", matches) {
        match v.parse::<f32>() {
            Ok(n) if n.is_finite() => {
                if let Ok(f) = validate_f32_range("config rippct", n, 0.0, 100.0) {
                    args.rippct = f;
                } else {
                    eprintln!("config: ignoring invalid rippct={v} (min 0 max 100)");
                }
            }
            _ => eprintln!("config: ignoring unparseable rippct='{v}' (expected a number)"),
        }
    }
    if let Some(v) = apply("maxdpc", matches) {
        match v.parse::<u8>() {
            Ok(n) => {
                if let Ok(valid) = validate_u8_range("config maxdpc", n, 1, 3) {
                    args.max_droplets_per_column = valid;
                } else {
                    eprintln!("config: ignoring invalid maxdpc={v} (min 1 max 3)");
                }
            }
            _ => eprintln!("config: ignoring unparseable maxdpc='{v}' (expected integer 1-3)"),
        }
    }
}

// --- Main entry point ---

/// Runtime CPU feature check for x86-64 builds.
///
/// Detects if the CPU supports the required instruction set for the
/// compiled target level (v3 = AVX2, v4 = AVX-512). Prints a clear
/// error message and exits instead of crashing with SIGILL.
#[cfg(target_arch = "x86_64")]
fn check_cpu_features() {
    let build = option_env!("COSMOSTRIX_BUILD").unwrap_or("");
    if build.contains("-v4") {
        if !std::arch::is_x86_feature_detected!("avx512f") {
            eprintln!(
                "\x1b[1;31mFATAL:\x1b[0m This binary requires \x1b[1mAVX-512\x1b[0m (x86-64-v4)"
            );
            eprintln!("       but your CPU does not support it.");
            eprintln!();
            eprintln!("Rebuild with a compatible target:");
            eprintln!("  cargo pro-linux-v2    # x86-64-v2 (SSE4.2, POPCNT) — most CPUs");
            eprintln!("  cargo pro-linux-v3    # x86-64-v3 (AVX2) — modern CPUs");
            std::process::exit(1);
        }
    } else if build.contains("-v3") && !std::arch::is_x86_feature_detected!("avx2") {
        eprintln!("\x1b[1;31mFATAL:\x1b[0m This binary requires \x1b[1mAVX2\x1b[0m (x86-64-v3)");
        eprintln!("       but your CPU does not support it.");
        eprintln!();
        eprintln!("Rebuild with:");
        eprintln!("  cargo pro-linux-v1    # x86-64-v1 (baseline)");
        eprintln!("  cargo pro-linux-v2    # x86-64-v2 (SSE4.2, POPCNT)");
        std::process::exit(1);
    }
}

fn main() -> std::io::Result<()> {
    // MUST be first — checks CPU features before any v3/v4 instructions execute
    #[cfg(target_arch = "x86_64")]
    check_cpu_features();

    std::panic::set_hook(Box::new(|info| {
        restore_terminal_best_effort();
        eprintln!("{}", info);
    }));

    let mut cmd = Args::command();
    #[cfg(unix)]
    {
        cmd = cmd.styles(clap_styles());
    }
    cmd = cmd.before_help(default_params_usage_for_help());
    let help_template = if color_enabled_stdout() {
        HELP_TEMPLATE_COLOR
    } else {
        HELP_TEMPLATE_PLAIN
    };
    cmd = cmd.help_template(help_template);
    cmd.build();

    if cmd.get_arguments().any(|a| a.get_id().as_str() == "help") {
        cmd = cmd.mut_arg("help", |a| a.help_heading("HELP"));
    }
    cmd.build();

    let mut argv: Vec<std::ffi::OsString> = env::args_os().collect();
    for arg in argv.iter_mut().skip(1) {
        if arg == "-mB" || arg == "-mb" {
            *arg = "--message-no-border".into();
        }
    }

    let matches = cmd.get_matches_from(argv);
    let mut args = Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());

    // Apply config file defaults for args not explicitly set by user
    apply_config_defaults(&matches, &mut args);

    if args.list_charsets {
        print_list_charsets();
        return Ok(());
    }

    if args.list_colors {
        print_list_colors();
        return Ok(());
    }

    if args.help_detail {
        print_help_detail();
        return Ok(());
    }

    if args.doctor {
        doctor::print_doctor_report(&args);
        return Ok(());
    }

    if args.check_bitcolor {
        let colorterm = env::var("COLORTERM").unwrap_or_default();
        let term = env::var("TERM").unwrap_or_default();
        let auto = detect_color_mode_auto();
        let effective = detect_color_mode(&args);

        println!("BITCOLOR CHECK:");
        println!(
            "  COLORTERM: {}",
            if colorterm.is_empty() {
                "(unset)"
            } else {
                &colorterm
            }
        );
        println!(
            "  TERM: {}",
            if term.is_empty() { "(unset)" } else { &term }
        );
        println!("  auto_detected: {}", color_mode_label(auto));
        if args.colormode.is_some() {
            println!("  forced: {}", color_mode_label(effective));
        }
        println!("  effective: {}", color_mode_label(effective));
        return Ok(());
    }

    if args.version {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    if args.info {
        println!("Version: v{}", env!("CARGO_PKG_VERSION"));
        if let Some(sha) = build_commit_short() {
            println!("Build: {} ({})", build_info(), sha);
        } else {
            println!("Build: {}", build_info());
        }
        println!("Copyright: (c) 2026 {}", env!("CARGO_PKG_AUTHORS"));
        println!("License: {}", env!("CARGO_PKG_LICENSE"));
        println!("Source: {}", env!("CARGO_PKG_REPOSITORY"));
        println!(
            "  est_memory_per_frame (120x40): {}",
            format_bytes(estimate_memory_budget(120, 40))
        );
        println!(
            "  est_memory_per_frame (200x60): {}",
            format_bytes(estimate_memory_budget(200, 60))
        );
        return Ok(());
    }

    // --- Validate all arguments using Result-based validators ---
    let def_ascii = default_to_ascii();
    let color_mode = detect_color_mode(&args);

    let shading_mode = match validate_u8_range("--shadingmode", args.shading_mode, 0, 1) {
        Ok(1) => ShadingMode::DistanceFromHead,
        _ => ShadingMode::Random,
    };

    let bold_mode = match validate_u8_range("--bold", args.bold, 0, 2) {
        Ok(0) => BoldMode::Off,
        Ok(2) => BoldMode::All,
        _ => BoldMode::Random,
    };

    let target_fps = validate_err("--fps", validate_f64_range("--fps", args.fps, 1.0, 240.0))?;

    let duration_s = args.duration.map(|s| {
        if !s.is_finite() {
            eprintln!("failed to apply --duration {} (must be a finite number)", s);
            std::process::exit(1);
        }
        if s > 0.0 {
            return validate_err(
                "--duration",
                validate_f64_range("--duration", s, 0.1, 86400.0),
            )
            .unwrap();
        }
        s
    });

    let color_scheme = validate_err("--color", parse_color_scheme(&args.color))?;

    let glitch_pct = validate_err(
        "--glitchpct",
        validate_f32_range("--glitchpct", args.glitch_pct, 0.0, 100.0),
    )?;
    let glitch_low = validate_err(
        "--glitchms low",
        validate_u16_range("--glitchms low", args.glitch_ms.low, 1, 5000),
    )?;
    let glitch_high = validate_err(
        "--glitchms high",
        validate_u16_range("--glitchms high", args.glitch_ms.high, 1, 5000),
    )?;
    let linger_low = validate_err(
        "--lingerms low",
        validate_u16_range("--lingerms low", args.linger_ms.low, 1, 60000),
    )?;
    let linger_high = validate_err(
        "--lingerms high",
        validate_u16_range("--lingerms high", args.linger_ms.high, 1, 60000),
    )?;
    let short_pct = validate_err(
        "--shortpct",
        validate_f32_range("--shortpct", args.shortpct, 0.0, 100.0),
    )?;
    let die_early_pct = validate_err(
        "--rippct",
        validate_f32_range("--rippct", args.rippct, 0.0, 100.0),
    )?;
    let max_dpc = validate_err(
        "--maxdpc",
        validate_u8_range("--maxdpc", args.max_droplets_per_column, 1, 3),
    )?;
    let speed = validate_err(
        "--speed",
        validate_f32_range("--speed", args.speed, 0.001, 1000.0),
    )?;

    let mut user_ranges: Vec<(char, char)> = Vec::new();
    if let Some(spec) = &args.chars {
        match parse_user_hex_chars(spec) {
            Ok(list) => {
                if list.len() % 2 != 0 {
                    eprintln!("--chars: odd number of unicode chars given (must be even)");
                    std::process::exit(1);
                }
                for pair in list.chunks(2) {
                    user_ranges.push((pair[0], pair[1]));
                }
            }
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
    }

    let charset = match charset_from_str(&args.charset, def_ascii) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    let charset_preset = normalize_charset_preset_name(&args.charset);

    let chars = build_chars(charset, &user_ranges, def_ascii);

    let density_auto = matches.value_source("density") == Some(ValueSource::DefaultValue);
    let base_density = validate_err(
        "--density",
        validate_f32_range(
            "--density",
            args.density,
            DENSITY_CLAMP_MIN,
            DENSITY_CLAMP_MAX,
        ),
    )?;

    let default_bg = matches!(
        args.color_bg,
        ColorBg::DefaultBackground | ColorBg::Transparent
    );

    let cloud_cfg = CloudConfig {
        color_mode,
        fullwidth: args.fullwidth,
        shading_mode,
        bold_mode,
        async_mode: args.async_mode,
        default_bg,
        color_scheme,
        noglitch: args.noglitch,
        glitch_pct,
        glitch_low,
        glitch_high,
        linger_low,
        linger_high,
        short_pct,
        die_early_pct,
        max_dpc,
        density: base_density,
        speed,
        chars,
        message: args.message.clone(),
        message_no_border: args.message_no_border,
        target_fps,
        duration: args.duration,
        duration_s,
        bench_frames: args.bench_frames,
        density_auto,
        base_density,
        perf_stats: args.perf_stats,
        screensaver: args.screensaver,
        charset_preset,
        user_ranges,
        def_ascii,
    };

    if let Some(_bench_frames) = args.bench_frames {
        return bench::run_benchmark(&cloud_cfg);
    }

    interactive::run_interactive(&cloud_cfg)
}
