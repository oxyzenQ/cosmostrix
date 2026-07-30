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
use crate::config::ColorBg;
use crate::output;
use crate::palette;
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, MonolithSize, ShadingMode};
use crate::{configfile, scene};
use crossterm::style::Color;

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
    color_bg: ColorBg,
    custom_palette_bg: Option<Color>,
    charset_preset: &str,
    chars: &[char],
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
    screen_size: Option<(u16, u16)>,
    custom_palette_name: Option<&str>,
    scene_arg: &Option<String>,
    config_path: Option<&std::path::Path>,
    cli_explicit_color: bool,
    intro_type_label: &str,
    commit_sha: &str,
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
    output::eprintln_verbose("scene:", &format!(" {}", scene_name.unwrap_or("default")));
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
            " sat={:.2} bright={:.2} head={:.2} body={:.2} tail={:.2}",
            color_tune.saturation,
            color_tune.brightness,
            color_tune.head,
            color_tune.body,
            color_tune.tail
        ),
    );
    // v25.17 (verbose ambiguity fix): previously printed just `true`/`false`,
    // which was ambiguous — `false` could mean "solid black" OR "custom palette
    // bg from config.toml like `bg = \"#0a0a12\"`". Now we print a descriptive
    // label that distinguishes all three cases so users can verify at a glance
    // which background actually got applied.
    let bg_label = describe_color_bg(color_bg, custom_palette_name, custom_palette_bg);
    output::eprintln_verbose("color_bg:", &format!(" {bg_label}"));

    // ── Glyphs ────────────────────────────────────────────────────
    eprintln!("{}", output::brand_bold("  ── Glyphs ──"));
    output::eprintln_verbose(
        "charset:",
        &format!(" {charset_preset} ({} glyphs)", chars.len()),
    );

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
    output::eprintln_verbose("glitch_level:", &format!(" {glitch_level}"));

    // ── Interaction ───────────────────────────────────────────────
    eprintln!("{}", output::brand_bold("  ── Interaction ──"));
    output::eprintln_verbose("mouse:", " always-on (glow + click wave)");
    output::eprintln_verbose("screensaver:", &format!(" {screensaver}"));
    output::eprintln_verbose("intro:", &format!(" {intro_type_label}"));
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
    // Compact atmosphere summary: show mode label + modulation values on a
    // single line. When mode is Disabled, modulation is always identity, so
    // we skip the modulation dump to avoid noise.
    if atmosphere_mode.allows_modulation() {
        output::eprintln_verbose(
            "atmosphere:",
            &format!(
                " {} (speed={:.2} density={:.2} bright={:.2} glitch_pressure={:.2})",
                atmosphere_mode.as_str(),
                atmosphere_modulation.speed_scale,
                atmosphere_modulation.density_scale,
                atmosphere_modulation.brightness_scale,
                atmosphere_modulation.glitch_pressure
            ),
        );
    } else {
        output::eprintln_verbose(
            "atmosphere:",
            &format!(" {} (modulation inactive)", atmosphere_mode.as_str()),
        );
    }

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
    output::eprintln_verbose("TERM_PROGRAM_VERSION:", &format!(" {term_version}"));
    output::eprintln_verbose("SHELL:", &format!(" {shell}"));
    output::eprintln_verbose("LANG:", &format!(" {lang}"));
    output::eprintln_verbose("isatty(stderr):", &format!(" {is_tty}"));
    output::eprintln_verbose("isatty(stdout):", &format!(" {is_stdout_tty}"));
    let is_android = configfile::is_termux_environment();
    output::eprintln_verbose("android:", &format!(" {is_android}"));

    // ── Config ────────────────────────────────────────────────────
    eprintln!("{}", output::brand_bold("  ── Config ──"));
    let config_path = configfile::default_config_file_path();
    output::eprintln_verbose("config_path:", &format!(" {}", config_path.display()));
    output::eprintln_verbose("config exists:", &format!(" {}", config_path.exists()));
    // v25.2 Termux fix: show ALL candidate paths the live-reload watcher
    // considers, so users can verify which file is being watched. This is
    // critical for Termux debugging where XDG_CONFIG_HOME may point to a
    // different location than $HOME/.config.
    let candidates = configfile::config_candidate_paths();
    output::eprintln_verbose(
        "config candidates:",
        &format!(
            " {}",
            candidates
                .iter()
                .map(|p| {
                    let marker = if p.exists() { " [exists]" } else { "" };
                    format!("{}{marker}", p.display())
                })
                .collect::<Vec<_>>()
                .join(", ")
        ),
    );
    output::eprintln_verbose("commit:", &format!(" {commit_sha}"));
}

/// Format an `Option<Color>` (palette bg) as a human-readable hex string.
/// `None` → `"none"`. `Color::Rgb` → `#rrggbb`. Ansi/named → decoded to hex
/// via `palette::color_to_rgb` so the user sees the actual on-screen color.
#[must_use]
fn format_bg_color(bg: Option<Color>) -> String {
    match bg {
        None => "none".to_string(),
        Some(c) => {
            let (r, g, b) = palette::color_to_rgb(c);
            format!("#{r:02x}{g:02x}{b:02x}")
        }
    }
}

/// Produce a descriptive label for the `color_bg` verbose line.
///
/// Three distinguishable cases:
/// 1. `DefaultBackground` → terminal native bg, no override
/// 2. `Black` + custom palette with bg → custom palette '<name>' bg=#0a0a12
/// 3. `Black` without custom palette → solid black bg
///
/// The label includes the actual hex color when a custom palette's bg is in
/// effect, so the user can verify the config.toml `bg = \"#0a0a12\"` value
/// was correctly parsed and applied.
#[must_use]
fn describe_color_bg(
    color_bg: ColorBg,
    custom_palette_name: Option<&str>,
    custom_palette_bg: Option<Color>,
) -> String {
    match color_bg {
        ColorBg::DefaultBackground => {
            "default-background (terminal native bg, no override)".to_string()
        }
        ColorBg::Black => match (custom_palette_name, custom_palette_bg) {
            (Some(name), Some(bg)) => {
                let hex = format_bg_color(Some(bg));
                format!("black + custom palette '{name}' bg={hex} (palette bg overrides)")
            }
            (Some(name), None) => {
                format!("black + custom palette '{name}' bg=none (palette has no bg, falls back to black)")
            }
            (None, _) => "black (solid black bg)".to_string(),
        },
    }
}
