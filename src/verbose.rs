// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Verbose diagnostic output for --verbose flag.
//!
//! Extracted from main.rs to keep that file under 1000 LOC.
//! Prints comprehensive runtime configuration to stderr for
//! power users / hackers debugging config and loading issues.
//!
//! Uses branded purple output: [verbose] prefix is bold purple,
//! field labels are purple, values stay in terminal default color
//! for readability.

use crate::atmosphere_apply::{AtmosphereApplicationMode, AtmosphereRuntimeModulation};
use crate::color_tune::ColorTune;
use crate::output;
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, MonolithSize, ShadingMode};
use crate::{configfile, is_safe_path, scene};

/// Determine color provenance for verbose annotation.
/// Returns None when a custom palette is active (it has its own line).
#[must_use]
fn resolve_color_source(
    custom_palette_name: Option<&str>,
    cli_explicit_color: bool,
    scene: &Option<String>,
    config_path: Option<&std::path::Path>,
) -> Option<&'static str> {
    if custom_palette_name.is_some() {
        return None;
    }
    if cli_explicit_color {
        return Some("CLI flag");
    }
    let cfg_has_color = configfile::load_config_file(config_path)
        .keys()
        .any(|k| k == "color" || k.starts_with("color."));
    match scene {
        Some(name)
            if scene::get_scene(name)
                .and_then(|s| s.config.color)
                .is_some() =>
        {
            Some("scene override")
        }
        Some(_) if cfg_has_color => Some("config file"),
        Some(_) => Some("CLI default — scene has no color override"),
        None if cfg_has_color => Some("config file"),
        None => Some("CLI default"),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn print_verbose(
    version: &str,
    scene_name: Option<&str>,
    rain_style: RainStyle,
    color_scheme: crate::runtime::ColorScheme,
    color_mode: ColorMode,
    color_tune: ColorTune,
    default_bg: bool,
    charset_preset: &str,
    chars: &[char],
    fullwidth: bool,
    target_fps: f64,
    speed: f32,
    base_density: f32,
    density_auto: bool,
    monolith_size: MonolithSize,
    async_mode: bool,
    bold_mode: BoldMode,
    shading_mode: ShadingMode,
    noglitch: bool,
    glitch_pct: f32,
    glitch_low: u16,
    glitch_high: u16,
    glitch_level: &str,
    screensaver: bool,
    auto_drift: bool,
    atmosphere_mode: AtmosphereApplicationMode,
    atmosphere_modulation: &AtmosphereRuntimeModulation,
    message: Option<&str>,
    message_border: bool,
    duration: Option<f64>,
    charset_file: Option<&str>,
    screen_size: Option<(u16, u16)>,
    custom_palette_name: Option<&str>,
    scene_arg: &Option<String>,
    config_path: Option<&std::path::Path>,
    cli_explicit_color: bool,
) {
    let color_source = resolve_color_source(
        custom_palette_name,
        cli_explicit_color,
        scene_arg,
        config_path,
    );
    eprintln!(
        "{}",
        output::brand_bold(&format!(
            "[verbose] {}  cosmostrix v{version} — runtime configuration",
            output::now_hhmm()
        ))
    );

    // ── Scene & Color ──────────────────────────────────────────────
    eprintln!("{}", output::brand_bold("  ── Scene & Color ──"));
    output::eprintln_verbose("scene:", &format!(" {:?}", scene_name.unwrap_or("default")));
    output::eprintln_verbose("rain_style:", &format!(" {rain_style:?}"));
    if let Some(name) = custom_palette_name {
        output::eprintln_verbose("color_palette:", &format!(" {name} (custom)"));
    } else if let Some(src) = color_source {
        output::eprintln_verbose("color_scheme:", &format!(" {color_scheme:?} ({src})"));
    } else {
        output::eprintln_verbose("color_scheme:", &format!(" {color_scheme:?}"));
    }
    output::eprintln_verbose("color_mode:", &format!(" {color_mode:?}"));
    output::eprintln_verbose(
        "color_tune:",
        &format!(
            " sat={:.2} bright={:.2}",
            color_tune.saturation, color_tune.brightness
        ),
    );
    output::eprintln_verbose("color_bg:", &format!(" {default_bg:?}"));

    // ── Glyphs ────────────────────────────────────────────────────
    eprintln!("{}", output::brand_bold("  ── Glyphs ──"));
    output::eprintln_verbose(
        "charset:",
        &format!(" {charset_preset} ({} glyphs)", chars.len()),
    );
    output::eprintln_verbose("fullwidth:", &format!(" {fullwidth}"));
    if let Some(cf) = charset_file {
        output::eprintln_verbose(
            "charset_file:",
            &format!(" {cf} (safe: {})", is_safe_path(cf)),
        );
    }

    // ── Motion ────────────────────────────────────────────────────
    eprintln!("{}", output::brand_bold("  ── Motion ──"));
    output::eprintln_verbose("fps:", &format!(" {target_fps:.1}"));
    output::eprintln_verbose("speed:", &format!(" {speed:.1}"));
    output::eprintln_verbose(
        "density:",
        &format!(" {base_density:.2} (auto: {density_auto})"),
    );
    output::eprintln_verbose("monolith:", &format!(" {monolith_size:?}"));
    let async_desc = if async_mode {
        "on (variable column speeds)"
    } else {
        "off (uniform column speeds)"
    };
    output::eprintln_verbose("async_mode:", &format!(" {async_desc}"));

    // ── Style ─────────────────────────────────────────────────────
    eprintln!("{}", output::brand_bold("  ── Style ──"));
    output::eprintln_verbose("bold:", &format!(" {bold_mode:?}"));
    output::eprintln_verbose("shading:", &format!(" {shading_mode:?}"));
    output::eprintln_verbose(
        "glitch:",
        &format!(
            " {} ({glitch_pct}%, {glitch_low}-{glitch_high}ms)",
            !noglitch
        ),
    );
    output::eprintln_verbose("glitch_level:", &format!(" {glitch_level:?}"));

    // ── Interaction ───────────────────────────────────────────────
    eprintln!("{}", output::brand_bold("  ── Interaction ──"));
    output::eprintln_verbose("mouse:", " always-on (glow + click wave)");
    output::eprintln_verbose("screensaver:", &format!(" {screensaver}"));
    if let Some(msg) = message {
        output::eprintln_verbose(
            "message:",
            &format!(
                " \"{msg}\" ({} chars, border: {message_border})",
                msg.chars().count()
            ),
        );
    }
    if let Some(d) = duration {
        output::eprintln_verbose("duration:", &format!(" {d:.1}s"));
    }

    // ── Atmosphere ────────────────────────────────────────────────
    eprintln!("{}", output::brand_bold("  ── Atmosphere ──"));
    output::eprintln_verbose("auto_drift:", &format!(" {auto_drift}"));
    output::eprintln_verbose(
        "atmosphere:",
        &format!(" {atmosphere_mode:?} / {atmosphere_modulation:?}"),
    );

    // ── Terminal ──────────────────────────────────────────────────
    eprintln!("{}", output::brand_bold("  ── Terminal ──"));
    let (sw, sh, size_mode) = match screen_size {
        Some((w, h)) => (w, h, "fixed"),
        None => {
            let (tw, th) = crossterm::terminal::size().unwrap_or((0, 0));
            (tw, th, "auto")
        }
    };
    output::eprintln_verbose("screen_size:", &format!(" {sw}x{sh} ({size_mode})"));
    let term = std::env::var("TERM").unwrap_or_else(|_| "(unset)".into());
    let colorterm = std::env::var("COLORTERM").unwrap_or_else(|_| "(unset)".into());
    let term_program = std::env::var("TERM_PROGRAM").unwrap_or_else(|_| "(unset)".into());
    let term_version = std::env::var("TERM_PROGRAM_VERSION").unwrap_or_else(|_| "(unset)".into());
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "(unset)".into());
    let lang = std::env::var("LANG").unwrap_or_else(|_| "(unset)".into());
    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stderr());
    let is_stdout_tty = std::io::IsTerminal::is_terminal(&std::io::stdout());
    output::eprintln_verbose("TERM:", &format!(" {term}"));
    output::eprintln_verbose("COLORTERM:", &format!(" {colorterm}"));
    output::eprintln_verbose("TERM_PROGRAM:", &format!(" {term_program}"));
    output::eprintln_verbose("TERM_VERSION:", &format!(" {term_version}"));
    output::eprintln_verbose("SHELL:", &format!(" {shell}"));
    output::eprintln_verbose("LANG:", &format!(" {lang}"));
    output::eprintln_verbose("isatty(stderr):", &format!(" {is_tty}"));
    output::eprintln_verbose("isatty(stdout):", &format!(" {is_stdout_tty}"));
    let is_android = std::env::var("TERMUX_VERSION").is_ok()
        || std::env::var("PREFIX").is_ok_and(|p| p.contains("com.termux"));
    output::eprintln_verbose("android:", &format!(" {is_android}"));

    // ── Config ────────────────────────────────────────────────────
    eprintln!("{}", output::brand_bold("  ── Config ──"));
    let config_path = configfile::default_config_file_path();
    output::eprintln_verbose("config_path:", &format!(" {}", config_path.display()));
    output::eprintln_verbose("config exists:", &format!(" {}", config_path.exists()));
}
