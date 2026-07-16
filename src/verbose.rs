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
use crate::{configfile, is_safe_path};

#[allow(clippy::too_many_arguments)]
pub(crate) fn print_verbose(
    version: &str,
    scene: Option<&str>,
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
    mouse: bool,
    screensaver: bool,
    auto_drift: bool,
    atmosphere_mode: AtmosphereApplicationMode,
    atmosphere_modulation: &AtmosphereRuntimeModulation,
    message: Option<&str>,
    message_border: bool,
    duration: Option<f64>,
    charset_file: Option<&str>,
    screen_size: Option<(u16, u16)>,
) {
    eprintln!(
        "{}",
        output::brand_bold(&format!(
            "[verbose]  cosmostrix v{version} — runtime configuration"
        ))
    );
    output::eprintln_verbose("scene:", &format!(" {:?}", scene.unwrap_or("default")));
    output::eprintln_verbose("rain_style:", &format!(" {rain_style:?}"));
    output::eprintln_verbose("color_scheme:", &format!(" {color_scheme:?}"));
    output::eprintln_verbose("color_mode:", &format!(" {color_mode:?}"));
    output::eprintln_verbose(
        "color_tune:",
        &format!(
            " sat={:.2} bright={:.2}",
            color_tune.saturation, color_tune.brightness
        ),
    );
    output::eprintln_verbose("color_bg:", &format!(" {default_bg:?}"));
    output::eprintln_verbose(
        "charset:",
        &format!(" {charset_preset} ({} glyphs)", chars.len()),
    );
    output::eprintln_verbose("fullwidth:", &format!(" {fullwidth}"));
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
    output::eprintln_verbose("mouse:", &format!(" {mouse}"));
    output::eprintln_verbose("screensaver:", &format!(" {screensaver}"));
    output::eprintln_verbose("auto_drift:", &format!(" {auto_drift}"));
    output::eprintln_verbose(
        "atmosphere:",
        &format!(" {atmosphere_mode:?} / {atmosphere_modulation:?}"),
    );
    // Screen size: fixed (--screen-size) or dynamic (terminal-detected)
    let (sw, sh, size_mode) = match screen_size {
        Some((w, h)) => (w, h, "fixed"),
        None => {
            // Detect actual terminal size for display
            let (tw, th) = crossterm::terminal::size().unwrap_or((0, 0));
            (tw, th, "auto")
        }
    };
    output::eprintln_verbose("screen_size:", &format!(" {sw}x{sh} ({size_mode})"));
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
    let config_path = configfile::default_config_file_path();
    output::eprintln_verbose("config_path:", &format!(" {}", config_path.display()));
    output::eprintln_verbose("config exists:", &format!(" {}", config_path.exists()));
    if let Some(cf) = charset_file {
        output::eprintln_verbose(
            "charset_file:",
            &format!(" {cf} (safe: {})", is_safe_path(cf)),
        );
    }
    let is_android = std::env::var("TERMUX_VERSION").is_ok()
        || std::env::var("PREFIX").is_ok_and(|p| p.contains("com.termux"));
    output::eprintln_verbose("android:", &format!(" {is_android}"));
}
