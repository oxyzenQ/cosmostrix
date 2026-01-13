// Copyright (c) 2026 rezky_nightky

mod cell;
mod charset;
mod cloud;
mod config;
mod droplet;
mod frame;
mod palette;
mod runtime;
mod terminal;

use std::env;
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use std::io::IsTerminal;

#[cfg(unix)]
use std::thread;

#[cfg(unix)]
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use clap::builder::styling::{AnsiColor as ClapAnsiColor, Color as ClapColor};
use clap::builder::styling::{Effects as ClapEffects, Style as ClapStyle};
use clap::builder::Styles as ClapStyles;
use clap::parser::ValueSource;
use clap::{CommandFactory, FromArgMatches};
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};

#[cfg(unix)]
use signal_hook::consts::{SIGCONT, SIGHUP, SIGINT, SIGSTOP, SIGTERM, SIGTSTP};
#[cfg(unix)]
use signal_hook::iterator::Signals;

#[cfg(unix)]
use signal_hook::low_level;

use crate::charset::{build_chars, charset_from_str, parse_user_hex_chars, Charset};
use crate::cloud::Cloud;
use crate::config::{
    color_enabled_stdout, default_params_usage_for_help, print_help_detail, print_list_charsets,
    print_list_colors, Args, ColorBg,
};
use crate::frame::Frame;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};
use crate::terminal::{restore_terminal_best_effort, Terminal};

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

fn build_info() -> &'static str {
    env!("COSMOSTRIX_BUILD")
}

fn build_commit_short() -> Option<&'static str> {
    match option_env!("COSMOSTRIX_GIT_SHA") {
        Some(s) if !s.is_empty() => Some(s),
        _ => None,
    }
}

fn env_var_truthy(name: &str) -> bool {
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
fn spawn_kill9_terminal_guard() {
    if env_var_truthy("COSMOSTRIX_NO_FORK_GUARD") {
        return;
    }

    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return;
    }

    unsafe {
        let mut orig: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(libc::STDIN_FILENO, &mut orig) != 0 {
            return;
        }

        let pid = libc::fork();
        if pid != 0 {
            return;
        }

        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGTERM);
        let _ = libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut());

        let _ = libc::prctl(
            libc::PR_SET_NAME,
            b"cx-term-guard\0".as_ptr() as usize,
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

fn require_f64_range(name: &str, v: f64, min: f64, max: f64) -> f64 {
    if !v.is_finite() {
        restore_terminal_best_effort();
        eprintln!("failed to apply {} {} (must be a finite number)", name, v);
        std::process::exit(1);
    }
    if v < min || v > max {
        restore_terminal_best_effort();
        eprintln!("failed to apply {} {} (min {} max {})", name, v, min, max);
        std::process::exit(1);
    }
    v
}

fn require_f32_range(name: &str, v: f32, min: f32, max: f32) -> f32 {
    if !v.is_finite() {
        restore_terminal_best_effort();
        eprintln!("failed to apply {} {} (must be a finite number)", name, v);
        std::process::exit(1);
    }
    if v < min || v > max {
        restore_terminal_best_effort();
        eprintln!("failed to apply {} {} (min {} max {})", name, v, min, max);
        std::process::exit(1);
    }
    v
}

fn require_u8_range(name: &str, v: u8, min: u8, max: u8) -> u8 {
    if v < min || v > max {
        restore_terminal_best_effort();
        eprintln!("failed to apply {} {} (min {} max {})", name, v, min, max);
        std::process::exit(1);
    }
    v
}

fn require_u16_range(name: &str, v: u16, min: u16, max: u16) -> u16 {
    if v < min || v > max {
        restore_terminal_best_effort();
        eprintln!("failed to apply {} {} (min {} max {})", name, v, min, max);
        std::process::exit(1);
    }
    v
}

fn default_to_ascii() -> bool {
    let lang = env::var("LANG").unwrap_or_default();
    !lang.to_ascii_uppercase().contains("UTF")
}

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

fn detect_color_mode(args: &Args) -> ColorMode {
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

fn color_mode_label(m: ColorMode) -> &'static str {
    match m {
        ColorMode::TrueColor => "24-bit truecolor",
        ColorMode::Color256 => "8-bit (256-color)",
        ColorMode::Mono => "mono",
        ColorMode::Color16 => "16-color",
    }
}

fn print_doctor_report(args: &Args) {
    let lang = env::var("LANG").unwrap_or_default();
    let lc_all = env::var("LC_ALL").unwrap_or_default();
    let lc_ctype = env::var("LC_CTYPE").unwrap_or_default();
    let term = env::var("TERM").unwrap_or_default();
    let colorterm = env::var("COLORTERM").unwrap_or_default();

    let stdin_tty = std::io::IsTerminal::is_terminal(&std::io::stdin());
    let stdout_tty = std::io::IsTerminal::is_terminal(&std::io::stdout());

    let locale_blob = format!("{}{}{}", lc_all, lc_ctype, lang);
    let locale_utf8 = locale_blob.to_ascii_uppercase().contains("UTF");

    let auto = detect_color_mode_auto();
    let effective = detect_color_mode(args);

    println!("DOCTOR REPORT:");
    println!("  stdin_is_tty: {}", if stdin_tty { "yes" } else { "no" });
    println!("  stdout_is_tty: {}", if stdout_tty { "yes" } else { "no" });

    println!(
        "  LANG: {}",
        if lang.is_empty() { "(unset)" } else { &lang }
    );
    println!(
        "  LC_ALL: {}",
        if lc_all.is_empty() {
            "(unset)"
        } else {
            &lc_all
        }
    );
    println!(
        "  LC_CTYPE: {}",
        if lc_ctype.is_empty() {
            "(unset)"
        } else {
            &lc_ctype
        }
    );
    println!("  locale_utf8: {}", if locale_utf8 { "yes" } else { "no" });

    println!(
        "  TERM: {}",
        if term.is_empty() { "(unset)" } else { &term }
    );
    println!(
        "  COLORTERM: {}",
        if colorterm.is_empty() {
            "(unset)"
        } else {
            &colorterm
        }
    );

    #[cfg(target_os = "linux")]
    {
        let no_fork_guard = env_var_truthy("COSMOSTRIX_NO_FORK_GUARD");
        println!(
            "  fork_guard: {}",
            if no_fork_guard {
                "disabled (COSMOSTRIX_NO_FORK_GUARD)"
            } else {
                "enabled"
            }
        );
    }

    println!("  color_auto_detected: {}", color_mode_label(auto));
    if args.colormode.is_some() {
        println!("  color_forced: {}", color_mode_label(effective));
    }
    println!("  color_effective: {}", color_mode_label(effective));

    let def_ascii = default_to_ascii();
    println!(
        "  default_to_ascii: {}",
        if def_ascii { "yes" } else { "no" }
    );

    let charset_preset = normalize_charset_preset_name(&args.charset);
    println!(
        "  charset: {}",
        if args.charset.is_empty() {
            "(empty)"
        } else {
            &args.charset
        }
    );
    if charset_preset != args.charset {
        println!("  charset_normalized: {}", charset_preset);
    }
    if let Some(spec) = &args.chars {
        println!("  chars_override: {}", spec);
    }

    let cs = match charset_from_str(&charset_preset, def_ascii) {
        Ok(v) => v,
        Err(e) => {
            println!("  charset_parse_error: {}", e);
            Charset::NONE
        }
    };

    let uses_katakana = cs.contains(Charset::KATAKANA);
    let uses_unicode = uses_katakana
        || cs.contains(Charset::GREEK)
        || cs.contains(Charset::CYRILLIC)
        || cs.contains(Charset::HEBREW)
        || cs.contains(Charset::BRAILLE)
        || cs.contains(Charset::RUNIC)
        || cs.contains(Charset::SYMBOLS)
        || cs.contains(Charset::ARROWS)
        || cs.contains(Charset::BLOCKS)
        || cs.contains(Charset::BOXDRAW)
        || cs.contains(Charset::MINIMAL);

    if locale_utf8 {
        println!();
        println!("SAMPLE GLYPHS:");
        println!("  ascii: 01 ABC abc !@#");
        if uses_katakana {
            println!("  katakana: ｱｲｳｴｵｶｷｸｹｺ");
        }
        if cs.contains(Charset::GREEK) {
            println!("  greek: ΩλπΔ");
        }
        if cs.contains(Charset::CYRILLIC) {
            println!("  cyrillic: ЯЖЮШ");
        }
        if cs.contains(Charset::HEBREW) {
            println!("  hebrew: אבגד");
        }
        if cs.contains(Charset::BRAILLE) {
            println!("  braille: ⣿⣷⣯⣟");
        }
        if cs.contains(Charset::RUNIC) {
            println!("  runic: ᚠᚢᚦᚨ");
        }
        if cs.contains(Charset::SYMBOLS) {
            println!("  symbols: ∞∑∫√π");
        }
        if cs.contains(Charset::ARROWS) {
            println!("  arrows: ←→↑↓");
        }
        if cs.contains(Charset::BLOCKS) {
            println!("  blocks: ░▒▓█");
        }
        if cs.contains(Charset::BOXDRAW) {
            println!("  boxdraw: ┌┐└┘─│");
        }
        if cs.contains(Charset::MINIMAL) {
            println!("  minimal: ·•○●◇◆");
        }
    }

    println!();
    println!("ADVICE:");
    let mut printed = false;
    if !stdin_tty || !stdout_tty {
        println!("  - run cosmostrix directly in a terminal (avoid piping/redirect)");
        printed = true;
    }
    if !locale_utf8 {
        println!("  - locale does not look like UTF-8; unicode charsets may render incorrectly");
        println!("    try: export LANG=en_US.UTF-8");
        printed = true;
    }
    if effective != ColorMode::TrueColor {
        println!("  - for best colors use a truecolor terminal (COLORTERM=truecolor)");
        printed = true;
    }
    if uses_unicode {
        println!(
            "  - selected charset uses unicode glyphs; if you see □□, change your terminal font"
        );
        if uses_katakana {
            println!("    font suggestions (CJK): Noto Sans CJK JP, Source Han Sans, IPAexGothic");
        } else {
            println!("    font suggestions: Noto Sans Mono, DejaVu Sans Mono");
        }
        printed = true;
    }

    #[cfg(target_os = "linux")]
    {
        if env_var_truthy("COSMOSTRIX_NO_FORK_GUARD") {
            println!(
                "  - fork-based SIGKILL terminal guard is disabled; SIGKILL (-9) may leave your terminal broken"
            );
            printed = true;
        }
    }
    if !printed {
        println!("  - no issues detected");
    }
}

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

fn normalize_charset_preset_name(s: &str) -> String {
    match s.trim().to_ascii_lowercase().as_str() {
        "bin" | "01" => "binary".to_string(),
        "dec" | "decimal" => "digits".to_string(),
        "hexadecimal" => "hex".to_string(),
        other => other.to_string(),
    }
}

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

fn auto_density_factor(cols: u16, lines: u16, fullwidth: bool) -> f32 {
    let eff_cols = if fullwidth {
        (cols / 2).max(1)
    } else {
        cols.max(1)
    } as f32;
    let eff_lines = lines.max(1) as f32;

    let area = eff_cols * eff_lines;
    let base = 80.0 * 25.0;
    let factor = (area / base).sqrt();
    factor.clamp(0.5, 2.0)
}

fn effective_density(base: f32, cols: u16, lines: u16, fullwidth: bool, auto: bool) -> f32 {
    let base = base.clamp(0.01, 5.0);
    if !auto {
        return base;
    }
    (base * auto_density_factor(cols, lines, fullwidth)).clamp(0.01, 5.0)
}

fn parse_color_scheme(s: &str) -> Result<ColorScheme, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "green" => Ok(ColorScheme::Green),
        "green2" => Ok(ColorScheme::Green2),
        "green3" => Ok(ColorScheme::Green3),
        "yellow" => Ok(ColorScheme::Yellow),
        "orange" => Ok(ColorScheme::Orange),
        "red" => Ok(ColorScheme::Red),
        "blue" => Ok(ColorScheme::Blue),
        "cyan" => Ok(ColorScheme::Cyan),
        "gold" => Ok(ColorScheme::Gold),
        "rainbow" => Ok(ColorScheme::Rainbow),
        "purple" => Ok(ColorScheme::Purple),
        "neon" | "synthwave" => Ok(ColorScheme::Neon),
        "fire" | "inferno" => Ok(ColorScheme::Fire),
        "ocean" | "deep-sea" | "deep_sea" | "deepsea" => Ok(ColorScheme::Ocean),
        "forest" | "jungle" => Ok(ColorScheme::Forest),
        "vaporwave" => Ok(ColorScheme::Vaporwave),
        "gray" | "grey" => Ok(ColorScheme::Gray),
        "snow" => Ok(ColorScheme::Snow),
        "aurora" => Ok(ColorScheme::Aurora),
        "fancy-diamond" | "fancy_diamond" | "fancydiamond" => Ok(ColorScheme::FancyDiamond),
        "cosmos" => Ok(ColorScheme::Cosmos),
        "nebula" => Ok(ColorScheme::Nebula),
        "spectrum20" | "spectrum-20" | "spectrum_20" | "theme20" | "theme-20" | "theme_20" => {
            Ok(ColorScheme::Spectrum20)
        }
        "stars" | "star" => Ok(ColorScheme::Stars),
        "mars" => Ok(ColorScheme::Mars),
        "venus" => Ok(ColorScheme::Venus),
        "mercury" => Ok(ColorScheme::Mercury),
        "jupiter" => Ok(ColorScheme::Jupiter),
        "saturn" => Ok(ColorScheme::Saturn),
        "uranus" => Ok(ColorScheme::Uranus),
        "neptune" => Ok(ColorScheme::Neptune),
        "pluto" => Ok(ColorScheme::Pluto),
        "moon" => Ok(ColorScheme::Moon),
        "sun" => Ok(ColorScheme::Sun),
        "comet" => Ok(ColorScheme::Comet),
        "galaxy" => Ok(ColorScheme::Galaxy),
        "supernova" | "super-nova" | "super_nova" => Ok(ColorScheme::Supernova),
        "blackhole" | "black-hole" | "black_hole" => Ok(ColorScheme::BlackHole),
        "andromeda" => Ok(ColorScheme::Andromeda),
        "stardust" | "star-dust" | "star_dust" => Ok(ColorScheme::Stardust),
        "meteor" => Ok(ColorScheme::Meteor),
        "eclipse" => Ok(ColorScheme::Eclipse),
        "deepspace" | "deep-space" | "deep_space" => Ok(ColorScheme::DeepSpace),
        _ => Err(format!("invalid color: {} (see --list-colors)", s)),
    }
}

fn main() -> std::io::Result<()> {
    std::panic::set_hook(Box::new(|info| {
        restore_terminal_best_effort();
        eprintln!("{}", info);
    }));

    let mut cmd = Args::command();
    cmd = cmd.styles(clap_styles());
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
    let args = Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());

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
        print_doctor_report(&args);
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
        return Ok(());
    }

    let def_ascii = default_to_ascii();
    let color_mode = detect_color_mode(&args);

    let shading_mode = match require_u8_range("--shadingmode", args.shading_mode, 0, 1) {
        1 => ShadingMode::DistanceFromHead,
        _ => ShadingMode::Random,
    };

    let bold_mode = match require_u8_range("--bold", args.bold, 0, 2) {
        0 => BoldMode::Off,
        2 => BoldMode::All,
        _ => BoldMode::Random,
    };

    let target_fps = require_f64_range("--fps", args.fps, 1.0, 240.0);
    let duration_s = args.duration.map(|s| {
        if !s.is_finite() {
            eprintln!("failed to apply --duration {} (must be a finite number)", s);
            std::process::exit(1);
        }
        if s > 0.0 {
            return require_f64_range("--duration", s, 0.1, 86400.0);
        }
        s
    });

    let color_scheme = match parse_color_scheme(&args.color) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    let glitch_pct = require_f32_range("--glitchpct", args.glitch_pct, 0.0, 100.0);
    let glitch_low = require_u16_range("--glitchms low", args.glitch_ms.low, 1, 5000);
    let glitch_high = require_u16_range("--glitchms high", args.glitch_ms.high, 1, 5000);
    let linger_low = require_u16_range("--lingerms low", args.linger_ms.low, 1, 60000);
    let linger_high = require_u16_range("--lingerms high", args.linger_ms.high, 1, 60000);
    let short_pct = require_f32_range("--shortpct", args.shortpct, 0.0, 100.0);
    let die_early_pct = require_f32_range("--rippct", args.rippct, 0.0, 100.0);
    let max_dpc = require_u8_range("--maxdpc", args.max_droplets_per_column, 1, 3);
    let speed = require_f32_range("--speed", args.speed, 0.001, 1000.0);

    let mut user_ranges: Vec<(char, char)> = Vec::new();
    if let Some(spec) = &args.chars {
        match parse_user_hex_chars(spec) {
            Ok(list) => {
                if list.len() % 2 != 0 {
                    eprintln!("--chars: odd number of unicode chars given (must be even)");
                    std::process::exit(1);
                }
                for pair in list.chunks(2) {
                    let a = pair[0];
                    let b = pair[1];
                    user_ranges.push((a, b));
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

    let mut charset_preset = normalize_charset_preset_name(&args.charset);

    let chars = build_chars(charset, &user_ranges, def_ascii);

    let density_auto = matches.value_source("density") == Some(ValueSource::DefaultValue);
    let base_density = require_f32_range("--density", args.density, 0.01, 5.0);

    if let Some(bench_frames) = args.bench_frames {
        if bench_frames == 0 {
            eprintln!(
                "failed to apply --bench-frames {} (must be > 0)",
                bench_frames
            );
            std::process::exit(1);
        }

        let (w, h) = (
            env::var("COSMOSTRIX_BENCH_COLS")
                .ok()
                .and_then(|v| v.parse::<u16>().ok())
                .unwrap_or(120),
            env::var("COSMOSTRIX_BENCH_LINES")
                .ok()
                .and_then(|v| v.parse::<u16>().ok())
                .unwrap_or(40),
        );

        let density = effective_density(base_density, w, h, args.fullwidth, density_auto);

        let mut cloud = Cloud::new(
            color_mode,
            args.fullwidth,
            shading_mode,
            bold_mode,
            args.async_mode,
            matches!(
                args.color_bg,
                ColorBg::DefaultBackground | ColorBg::Transparent
            ),
            color_scheme,
        );

        cloud.glitchy = !args.noglitch;
        cloud.set_glitch_pct(glitch_pct / 100.0);
        cloud.set_glitch_times(glitch_low, glitch_high);
        cloud.set_linger_times(linger_low, linger_high);
        cloud.short_pct = short_pct / 100.0;
        cloud.die_early_pct = die_early_pct / 100.0;
        cloud.set_max_droplets_per_column(max_dpc);
        cloud.set_droplet_density(density);
        cloud.set_chars_per_sec(speed);

        cloud.init_chars(chars);
        cloud.reset(w, h);

        if let Some(msg) = &args.message {
            cloud.set_message_border(!args.message_no_border);
            cloud.set_message(msg);
        }

        let mut frame = Frame::new(w, h, cloud.palette.bg);

        let target_period = Duration::from_secs_f64(1.0 / target_fps);
        cloud.set_max_sim_delta(target_period);

        let warmup_frames = (bench_frames / 10).clamp(10, 200);
        let mut sim_now = Instant::now();

        for _ in 0..warmup_frames {
            sim_now += target_period;
            cloud.rain_at(&mut frame, sim_now);
            frame.clear_dirty();
        }

        let start = Instant::now();
        for _ in 0..bench_frames {
            sim_now += target_period;
            cloud.rain_at(&mut frame, sim_now);
            frame.clear_dirty();
        }
        let elapsed_s = start.elapsed().as_secs_f64().max(0.000_001);
        let fps = (bench_frames as f64) / elapsed_s;

        println!("BENCH:");
        println!("  cols: {}", w);
        println!("  lines: {}", h);
        println!("  frames: {}", bench_frames);
        println!("  elapsed_s: {:.6}", elapsed_s);
        println!("  frames_per_s: {:.3}", fps);
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    spawn_kill9_terminal_guard();

    #[cfg(unix)]
    let term_reinit = Arc::new(AtomicBool::new(false));

    #[cfg(unix)]
    {
        if let Ok(mut signals) = Signals::new([SIGINT, SIGTERM, SIGHUP]) {
            thread::spawn(move || {
                if let Some(sig) = signals.forever().next() {
                    restore_terminal_best_effort();
                    std::process::exit(128 + sig);
                }
            });
        }

        let term_reinit = term_reinit.clone();
        if let Ok(mut signals) = Signals::new([SIGTSTP, SIGCONT]) {
            thread::spawn(move || {
                for sig in signals.forever() {
                    match sig {
                        SIGTSTP => {
                            restore_terminal_best_effort();
                            term_reinit.store(true, Ordering::SeqCst);
                            let _ = low_level::raise(SIGSTOP);
                        }
                        SIGCONT => {
                            term_reinit.store(true, Ordering::SeqCst);
                        }
                        _ => {}
                    }
                }
            });
        }
    }

    #[cfg(windows)]
    {
        if let Err(e) = ctrlc::set_handler(|| {
            restore_terminal_best_effort();
            std::process::exit(130);
        }) {
            eprintln!("failed to install Ctrl-C handler: {}", e);
        }
    }

    let mut term = Terminal::new()?;
    let (w, h) = term.size()?;

    let density = effective_density(base_density, w, h, args.fullwidth, density_auto);

    let mut cloud = Cloud::new(
        color_mode,
        args.fullwidth,
        shading_mode,
        bold_mode,
        args.async_mode,
        matches!(
            args.color_bg,
            ColorBg::DefaultBackground | ColorBg::Transparent
        ),
        color_scheme,
    );

    cloud.glitchy = !args.noglitch;
    cloud.set_glitch_pct(glitch_pct / 100.0);
    cloud.set_glitch_times(glitch_low, glitch_high);
    cloud.set_linger_times(linger_low, linger_high);
    cloud.short_pct = short_pct / 100.0;
    cloud.die_early_pct = die_early_pct / 100.0;
    cloud.set_max_droplets_per_column(max_dpc);
    cloud.set_droplet_density(density);
    cloud.set_chars_per_sec(speed);

    cloud.init_chars(chars);
    cloud.reset(w, h);

    if let Some(msg) = &args.message {
        cloud.set_message_border(!args.message_no_border);
        cloud.set_message(msg);
    }

    let mut frame = Frame::new(w, h, cloud.palette.bg);

    let start_time = Instant::now();
    let end_time = args.duration.and_then(|s| {
        if !s.is_finite() || s <= 0.0 {
            return None;
        }
        let s = duration_s.unwrap_or(s);
        Some(start_time + Duration::from_secs_f64(s))
    });

    let target_period = Duration::from_secs_f64(1.0 / target_fps);
    let pause_period = Duration::from_millis(250);
    let mut next_frame = Instant::now();
    let mut perf_pressure: f32 = 0.0;

    let mut perf_frames: u64 = 0;
    let mut perf_drawn_frames: u64 = 0;
    let mut perf_work_sum_s: f64 = 0.0;
    let mut perf_work_max_s: f32 = 0.0;
    let mut perf_pressure_sum: f64 = 0.0;
    let mut perf_pressure_max: f32 = 0.0;
    let mut perf_overshoot_frames: u64 = 0;

    while cloud.raining {
        let frame_period = if cloud.pause {
            pause_period
        } else {
            target_period
        };
        let frame_period_s = frame_period.as_secs_f32().max(0.000_001);

        if end_time.is_some_and(|end| Instant::now() >= end) {
            cloud.raining = false;
            break;
        }
        let mut pending_resize: Option<(u16, u16)> = None;

        #[cfg(unix)]
        if term_reinit.swap(false, Ordering::SeqCst) {
            drop(term);
            term = Terminal::new()?;
            let (nw, nh) = term.size()?;
            pending_resize = Some((nw, nh));
            cloud.force_draw_everything();
            next_frame = Instant::now();
        }

        loop {
            while Terminal::poll_event(Duration::from_millis(0))? {
                let ev = Terminal::read_event()?;
                match ev {
                    Event::Resize(nw, nh) => {
                        pending_resize = Some((nw, nh));
                    }
                    Event::Key(k) if k.kind == KeyEventKind::Press => {
                        if args.screensaver {
                            cloud.raining = false;
                            break;
                        }

                        match (k.code, k.modifiers) {
                            (KeyCode::Esc, _) => cloud.raining = false,
                            (KeyCode::Char('q'), _) => cloud.raining = false,
                            (KeyCode::Char('z'), KeyModifiers::CONTROL) => {
                                #[cfg(unix)]
                                {
                                    restore_terminal_best_effort();
                                    term_reinit.store(true, Ordering::SeqCst);
                                    let _ = low_level::raise(SIGSTOP);
                                }
                            }
                            (KeyCode::Char(' '), _) => {
                                cloud.reset(frame.width, frame.height);
                                cloud.force_draw_everything();
                            }
                            (KeyCode::Char('c'), _) => {
                                let next = cycle_color_scheme(cloud.color_scheme(), 1);
                                cloud.set_color_scheme(next);
                            }
                            (KeyCode::Char('C'), _) => {
                                let prev = cycle_color_scheme(cloud.color_scheme(), -1);
                                cloud.set_color_scheme(prev);
                            }
                            (KeyCode::Char('s'), _) => {
                                let next = cycle_charset_preset(&charset_preset, 1);
                                charset_preset = next.to_string();
                                if let Ok(cs) = charset_from_str(&charset_preset, def_ascii) {
                                    let chars = build_chars(cs, &user_ranges, def_ascii);
                                    cloud.init_chars(chars);
                                    cloud.force_draw_everything();
                                }
                            }
                            (KeyCode::Char('S'), _) => {
                                let prev = cycle_charset_preset(&charset_preset, -1);
                                charset_preset = prev.to_string();
                                if let Ok(cs) = charset_from_str(&charset_preset, def_ascii) {
                                    let chars = build_chars(cs, &user_ranges, def_ascii);
                                    cloud.init_chars(chars);
                                    cloud.force_draw_everything();
                                }
                            }
                            (KeyCode::Char('a'), _) => {
                                cloud.set_async(!cloud.async_mode);
                            }
                            (KeyCode::Char('g'), _) => {
                                cloud.set_glitchy(!cloud.glitchy);
                            }
                            (KeyCode::Char('p'), _) => {
                                cloud.toggle_pause();
                            }
                            (KeyCode::Up, _) => {
                                let mut cps = cloud.chars_per_sec;
                                if cps <= 0.5 {
                                    cps *= 2.0;
                                } else {
                                    cps += 1.0;
                                }
                                cloud.set_chars_per_sec(cps.min(1000.0));
                            }
                            (KeyCode::Down, _) => {
                                let mut cps = cloud.chars_per_sec;
                                if cps <= 1.0 {
                                    cps /= 2.0;
                                } else {
                                    cps -= 1.0;
                                }
                                cloud.set_chars_per_sec(cps.max(0.001));
                            }
                            (KeyCode::Left, _) => {
                                if cloud.glitchy {
                                    let gp = (cloud.glitch_pct - 0.05).max(0.0);
                                    cloud.set_glitch_pct(gp);
                                }
                            }
                            (KeyCode::Right, _) => {
                                if cloud.glitchy {
                                    let gp = (cloud.glitch_pct + 0.05).min(1.0);
                                    cloud.set_glitch_pct(gp);
                                }
                            }
                            (KeyCode::Tab, _) => {
                                let sm = if cloud.shading_distance {
                                    ShadingMode::Random
                                } else {
                                    ShadingMode::DistanceFromHead
                                };
                                cloud.set_shading_mode(sm);
                            }
                            (KeyCode::Char('-'), _)
                            | (KeyCode::Char('['), _)
                            | (KeyCode::Char('_'), _) => {
                                let d = (cloud.droplet_density - 0.25).max(0.01);
                                cloud.set_droplet_density(d);
                            }
                            (KeyCode::Char('+'), _)
                            | (KeyCode::Char('='), KeyModifiers::SHIFT)
                            | (KeyCode::Char(']'), _) => {
                                let d = (cloud.droplet_density + 0.25).min(5.0);
                                cloud.set_droplet_density(d);
                            }
                            (KeyCode::Char('1'), _) => cloud.set_color_scheme(ColorScheme::Green),
                            (KeyCode::Char('2'), _) => cloud.set_color_scheme(ColorScheme::Green2),
                            (KeyCode::Char('3'), _) => cloud.set_color_scheme(ColorScheme::Green3),
                            (KeyCode::Char('4'), _) => cloud.set_color_scheme(ColorScheme::Gold),
                            (KeyCode::Char('5'), _) => cloud.set_color_scheme(ColorScheme::Neon),
                            (KeyCode::Char('6'), _) => cloud.set_color_scheme(ColorScheme::Red),
                            (KeyCode::Char('7'), _) => cloud.set_color_scheme(ColorScheme::Blue),
                            (KeyCode::Char('8'), _) => cloud.set_color_scheme(ColorScheme::Cyan),
                            (KeyCode::Char('9'), _) => cloud.set_color_scheme(ColorScheme::Purple),
                            (KeyCode::Char('0'), _) => cloud.set_color_scheme(ColorScheme::Gray),
                            (KeyCode::Char('!'), _) => cloud.set_color_scheme(ColorScheme::Rainbow),
                            (KeyCode::Char('@'), _) => cloud.set_color_scheme(ColorScheme::Yellow),
                            (KeyCode::Char('#'), _) => cloud.set_color_scheme(ColorScheme::Orange),
                            (KeyCode::Char('$'), _) => cloud.set_color_scheme(ColorScheme::Fire),
                            (KeyCode::Char('%'), _) => {
                                cloud.set_color_scheme(ColorScheme::Vaporwave)
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }

            if !cloud.raining || pending_resize.is_some() {
                break;
            }

            let now = Instant::now();
            if now >= next_frame {
                break;
            }

            let mut timeout = next_frame - now;
            if let Some(end) = end_time {
                if now >= end {
                    break;
                }
                timeout = timeout.min(end - now);
            }
            let _ = Terminal::poll_event(timeout)?;
        }

        if !cloud.raining {
            break;
        }

        if let Some((nw, nh)) = pending_resize {
            cloud.reset(nw, nh);
            frame = Frame::new(nw, nh, cloud.palette.bg);
            if density_auto {
                cloud.set_droplet_density(effective_density(
                    base_density,
                    nw,
                    nh,
                    args.fullwidth,
                    true,
                ));
            }
            cloud.force_draw_everything();
        }

        cloud.set_perf_pressure(perf_pressure);
        let sim_base_s = frame_period.as_secs_f64() * 3.0;
        let sim_factor = (1.0 - (perf_pressure as f64) * 0.7).clamp(0.3, 1.0);
        let sim_min_s = (frame_period.as_secs_f64() * 0.5).max(0.001);
        let sim_max_s = sim_base_s.min(0.5);
        let sim_cap_s = (sim_base_s * sim_factor).clamp(sim_min_s, sim_max_s);
        cloud.set_max_sim_delta(Duration::from_secs_f64(sim_cap_s));

        let work_start = Instant::now();
        cloud.rain(&mut frame);
        let did_draw = frame.is_dirty_all() || !frame.dirty_indices().is_empty();
        if did_draw {
            term.draw(&mut frame)?;
        }
        let work_s = work_start.elapsed().as_secs_f32();
        let overshoot = ((work_s / frame_period_s) - 1.0).clamp(0.0, 2.0);
        if overshoot > 0.0 {
            perf_pressure = (perf_pressure + (overshoot * 0.25)).min(1.0);
        } else {
            perf_pressure = (perf_pressure - 0.02).max(0.0);
        }

        if args.perf_stats {
            perf_frames = perf_frames.saturating_add(1);
            if did_draw {
                perf_drawn_frames = perf_drawn_frames.saturating_add(1);
            }
            perf_work_sum_s += work_s as f64;
            perf_work_max_s = perf_work_max_s.max(work_s);
            perf_pressure_sum += perf_pressure as f64;
            perf_pressure_max = perf_pressure_max.max(perf_pressure);
            if overshoot > 0.0 {
                perf_overshoot_frames = perf_overshoot_frames.saturating_add(1);
            }
        }

        let now = Instant::now();
        next_frame = next_frame.checked_add(frame_period).unwrap_or(now);
        if now > next_frame {
            next_frame = now.checked_add(frame_period).unwrap_or(now);
        }
    }

    if args.perf_stats {
        drop(term);
        let elapsed = start_time.elapsed();
        let elapsed_s = elapsed.as_secs_f64().max(0.000_001);

        let frames = perf_frames.max(1);
        let avg_work_ms = (perf_work_sum_s / frames as f64) * 1000.0;
        let avg_pressure = perf_pressure_sum / frames as f64;
        let avg_fps = (perf_frames as f64) / elapsed_s;
        let drawn_ratio = (perf_drawn_frames as f64) / (perf_frames as f64).max(1.0);

        println!("PERF STATS:");
        println!("  elapsed_s: {:.3}", elapsed_s);
        println!("  target_fps: {:.3}", target_fps);
        println!("  avg_fps: {:.3}", avg_fps);
        println!("  frames: {}", perf_frames);
        println!(
            "  drawn_frames: {} ({:.1}%)",
            perf_drawn_frames,
            drawn_ratio * 100.0
        );
        println!("  avg_work_ms: {:.3}", avg_work_ms);
        println!("  max_work_ms: {:.3}", perf_work_max_s as f64 * 1000.0);
        println!(
            "  overshoot_frames: {} ({:.1}%)",
            perf_overshoot_frames,
            (perf_overshoot_frames as f64) / (perf_frames as f64).max(1.0) * 100.0
        );
        println!("  avg_perf_pressure: {:.3}", avg_pressure);
        println!("  max_perf_pressure: {:.3}", perf_pressure_max);
    }

    Ok(())
}
