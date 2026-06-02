// Copyright (c) 2026 rezky_nightky

//! CLI presentation helpers: help templates, clap styling, color/charset scheme
//! parsing, cycling, and terminal color detection.

use std::collections::HashMap;
use std::env;
use std::sync::LazyLock;

use crate::config::Args;
use crate::runtime::{ColorMode, ColorScheme};

// --- Help template constants ---

pub(crate) const HELP_TEMPLATE_PLAIN: &str = "\
{name} {version}
{about-with-newline}\
USAGE:
  {usage}

{all-args}{after-help}";

pub(crate) const HELP_TEMPLATE_COLOR: &str = "\
{name} {version}
{about-with-newline}\
\x1b[1;36mUSAGE:\x1b[0m
  {usage}

{all-args}{after-help}";

// --- Clap styling ---

#[cfg(unix)]
use clap::builder::styling::{AnsiColor as ClapAnsiColor, Color as ClapColor};
use clap::builder::styling::{Effects as ClapEffects, Style as ClapStyle};
use clap::builder::Styles as ClapStyles;

#[must_use]
#[cfg(unix)]
pub(crate) fn clap_styles() -> ClapStyles {
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

// --- Charset helpers ---

#[must_use]
pub fn default_to_ascii() -> bool {
    let lang = env::var("LANG").unwrap_or_default();
    !lang.to_ascii_uppercase().contains("UTF")
}

// --- Color mode detection ---

#[must_use]
pub(crate) fn detect_color_mode_from_terms(colorterm: &str, term: &str) -> ColorMode {
    let colorterm = colorterm.to_ascii_lowercase();
    if colorterm.contains("truecolor") || colorterm.contains("24bit") {
        return ColorMode::TrueColor;
    }

    let term = term.to_ascii_lowercase();
    if term == "dumb" {
        return ColorMode::Mono;
    }
    if term.contains("-truecolor") || term.ends_with("-direct") {
        return ColorMode::TrueColor;
    }
    if term.contains("256color") {
        return ColorMode::Color256;
    }

    ColorMode::Color16
}

#[must_use]
pub fn detect_color_mode_auto() -> ColorMode {
    #[cfg(windows)]
    {
        if env::var_os("WT_SESSION").is_some() {
            return ColorMode::TrueColor;
        }
    }

    let colorterm = env::var("COLORTERM").unwrap_or_default();
    let term = env::var("TERM").unwrap_or_default();
    detect_color_mode_from_terms(&colorterm, &term)
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

// --- Color scheme helpers ---

#[must_use]
pub(crate) fn all_color_schemes() -> &'static [ColorScheme] {
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
pub fn cycle_color_scheme(current: ColorScheme, dir: i32) -> ColorScheme {
    let list = all_color_schemes();
    let Some(pos) = list.iter().position(|&c| c == current) else {
        return ColorScheme::Green;
    };

    let n = list.len() as i32;
    let mut idx = pos as i32 + dir;
    idx = ((idx % n) + n) % n;
    list[idx as usize]
}

// --- Charset preset helpers ---

#[must_use]
pub(crate) fn all_charset_presets() -> &'static [&'static str] {
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
pub fn normalize_charset_preset_name(s: &str) -> String {
    match s.trim().to_ascii_lowercase().as_str() {
        "bin" | "01" => "binary".to_string(),
        "dec" | "decimal" => "digits".to_string(),
        "hexadecimal" => "hex".to_string(),
        other => other.to_string(),
    }
}

#[must_use]
pub fn cycle_charset_preset(current: &str, dir: i32) -> &'static str {
    let list = all_charset_presets();
    let Some(pos) = list.iter().position(|&c| c == current) else {
        return "binary";
    };

    let n = list.len() as i32;
    let mut idx = pos as i32 + dir;
    idx = ((idx % n) + n) % n;
    list[idx as usize]
}

// --- HashMap-based color scheme parser ---

pub(crate) static COLOR_SCHEME_MAP: LazyLock<HashMap<&'static str, ColorScheme>> =
    LazyLock::new(|| {
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

pub fn parse_color_scheme(s: &str) -> Result<ColorScheme, String> {
    let key = s.trim().to_ascii_lowercase();
    COLOR_SCHEME_MAP
        .get(key.as_str())
        .copied()
        .ok_or_else(|| format!("invalid color: {} (see --list-colors)", s))
}
