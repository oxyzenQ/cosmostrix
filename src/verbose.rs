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

use crate::color_tune::ColorTune;
use crate::config::ColorBg;
use crate::output;
use crate::palette;
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorPipeline, MonolithSize, ShadingMode};
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
    // v30.6: which resolution layer produced target_fps. One of:
    // cli / scene / config / dynamic_default / xtermjs_cap.
    fps_precedence: &'static str,
    speed: f32,
    base_density: f32,
    density_auto: bool,
    monolith_size: MonolithSize,
    async_mode: bool,
    bold_mode: BoldMode,
    shading_mode: ShadingMode,
    // v30 simplify: was `noglitch: bool` (inverse polarity). Renamed to
    // `glitch_enabled` for clarity and consistency with CloudConfig.
    glitch_enabled: bool,
    glitch_pct: f32,
    glitch_low: u16,
    glitch_high: u16,
    glitch_level: &str,
    screensaver: bool,
    auto_drift: bool,
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
    // v30 (Bug #1 doc): --benchmark forces auto_color_drift=false (palette
    // rebuild injects timing spikes, breaks p99/max determinism). Pass
    // bench_mode so verbose can disclose the override BEFORE the benchmark
    // report prints `auto_color_drift: false` (otherwise the user sees
    // `auto_drift: true` in verbose and thinks it's a bug).
    bench_mode: bool,
    // v40 (verbose completeness audit): disclose the active custom scene
    // (--scene-custom <name>) and the ambient schedule (time-of-day scene
    // switching). Without these the user cannot tell from --verbose that
    // a custom scene is in effect or that scenes will auto-switch.
    scene_custom: Option<&str>,
    ambient_schedule: &crate::ambient::AmbientSchedule,
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
    // v40 (verbose completeness): disclose --scene-custom <name> so the user
    // can tell a custom scene is in effect (otherwise `scene:` shows "default"
    // even though the custom scene's parameters ARE applied via config_apply).
    if let Some(name) = scene_custom {
        output::eprintln_verbose(
            "scene_custom:",
            &format!(" {name} (loaded from [scene-custom.{name}])"),
        );
    }
    output::eprintln_verbose("rain_style:", &format!(" {rain_style:?}"));
    if let Some(name) = custom_palette_name {
        output::eprintln_verbose("color_palette:", &format!(" {name} (custom)"));
    } else if let Some(src) = color_source {
        output::eprintln_verbose("color_scheme:", &format!(" {color_scheme:?} ({src})"));
    } else {
        output::eprintln_verbose("color_scheme:", &format!(" {color_scheme:?}"));
    }
    output::eprintln_verbose("color_mode:", &format!(" {color_mode:?}"));
    // v30.3 (chroma dragon audit): disclose the active color pipeline so the
    // user can verify "is the chroma dragon engine running, or did I fall
    // back to legacy sRGB-linear?". Owner directive: "all color -> chroma
    // dragon first -> fallback legacy rgb/srgb". The pipeline label and its
    // feature description go right under `color_mode:` so a user reading
    // top-to-bottom sees the full color story in three lines: mode -> pipeline
    // -> tune. Without this disclosure the user had to guess from the color
    // mode alone, which was misleading (a TrueColor terminal could still be
    // running a chroma-bypass effect that did raw RGB math).
    let pipeline = ColorPipeline::detect(color_mode);
    output::eprintln_verbose(
        "color_pipeline:",
        &format!(" {} ({})", pipeline.label(), pipeline.description()),
    );
    if pipeline.is_chroma() {
        output::eprintln_verbose(
            "  chroma_features:",
            " oklab_gradient, perceptual_blend, climate_post_fx, head_halo, l_smoothing, subpixel_jitter",
        );
    } else if let Some(reason) = pipeline.disable_reason(color_mode) {
        output::eprintln_verbose(
            "  chroma_features:",
            " disabled -- legacy sRGB-linear fallback in effect for this color mode",
        );
        output::eprintln_verbose("  chroma_disable_reason:", &format!(" {reason}"));
    }
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
    // v30.5: show which detection layer set the dynamic fps default.
    // v30.6: also show fps_precedence (which RESOLUTION layer won:
    // cli / scene / config / dynamic_default / xtermjs_cap).
    let caps_for_source = crate::termdetect::detect();
    output::eprintln_verbose(
        "fps_source:",
        &format!(" {} (dynamic default)", caps_for_source.dynamic_fps_source),
    );
    output::eprintln_verbose("fps_precedence:", &format!(" {fps_precedence}"));
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
            glitch_enabled
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

    // ── Color Climate ────────────────────────────────────────────────
    // v30: renamed from "── Atmosphere ──" to "── Color Climate ──" to
    // disambiguate from the deleted atmosphere engine subsystem. The
    // fields below describe the surviving ColorEcosystem drift (luminance/
    // saturation/hue accumulation) and the palette-scheme replacement flag —
    // none of these have any relation to the deleted AtmosphereRuntimeModulation.
    eprintln!("{}", output::brand_bold("  ── Color Climate ──"));
    // Phase D Strengthen #12: expand drift disclosure. Previously only
    // `auto_drift: bool` was shown, which was misleading — climate drift
    // (luminance/saturation/hue) is ALWAYS ON regardless of the flag.
    // The flag only gates palette scheme replacement. Now verbose honestly
    // discloses both: the flag state + the always-on climate drift + the
    // cooldown (Phase D Bug #7 fix).
    output::eprintln_verbose("auto_drift:", &format!(" {auto_drift}"));
    // v30 (Bug #1 doc clarification): when --benchmark is active, the
    // benchmark loop forces palette drift OFF regardless of the user's
    // config/CLI value (see bench.rs::run_benchmark line ~201). Without
    // this notice, the verbose output here shows `auto_drift: true` (from
    // config) while the benchmark report later prints
    // `auto_color_drift: false` — the mismatch looked like a bug. The
    // benchmark override is intentional (palette rebuilds inject
    // non-deterministic timing spikes that corrupt p99/max metrics), so
    // we disclose it here instead of changing the behavior.
    let palette_drift_label = if bench_mode && auto_drift {
        "enabled (overridden to disabled in benchmark mode — see note below)"
    } else if auto_drift {
        "enabled"
    } else {
        "disabled"
    };
    output::eprintln_verbose(
        "  palette_drift:",
        &format!(" {palette_drift_label} (3% chance per 3s tick, 30s cooldown between events)"),
    );
    if bench_mode && auto_drift {
        output::eprintln_verbose(
            "  bench_override:",
            " palette drift forced OFF during benchmark for deterministic p99/max metrics; report will show `auto_color_drift: false`",
        );
    }
    output::eprintln_verbose(
        "  climate_drift:",
        " always-on (luminance/saturation/hue accumulate regardless of auto_drift flag)",
    );

    // ── Ambient ───────────────────────────────────────────────────
    // v40 (verbose completeness): disclose the ambient schedule so the user
    // can verify time-of-day scene switches are loaded. Without this, a user
    // debugging "why did my scene change at 15:00?" has zero visibility.
    eprintln!("{}", output::brand_bold("  ── Ambient ──"));
    let entries = &ambient_schedule.entries;
    if entries.is_empty() {
        output::eprintln_verbose(
            "schedule:",
            " 0 entries (scheduler idles, no auto-snapback)",
        );
    } else {
        let summary: Vec<String> = entries
            .iter()
            .map(|e| format!("{:02}-{:02}→{}", e.hour, e.minute, e.scene))
            .collect();
        output::eprintln_verbose(
            "schedule:",
            &format!(" {} entries [{}]", entries.len(), summary.join(", ")),
        );
        // Auto-snapback note: hardcoded 30s idle threshold. The user
        // cannot tune this at runtime; disclose so they understand the
        // snapback behavior is fixed.
        output::eprintln_verbose(
            "auto_snapback:",
            " 30s idle threshold (hardcoded — user overrides via 'c'/'C'/'x'/'s' revert after 30s)",
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
    // v30 (VSCode crash fix) + Tier 2 (xterm.js host extension): disclose
    // terminal capability detection so the user can see why sync_output /
    // FPS cap / byte-budget backpressure changed. This is especially
    // important for xterm.js hosts where the cap is applied silently in
    // non-verbose mode via the warning in main.rs.
    let caps = crate::termdetect::detect();
    if caps.xtermjs_host {
        let host_name = std::env::var("TERM_PROGRAM").unwrap_or_default();
        output::eprintln_verbose(
            "xtermjs_host:",
            &format!(
                " true (TERM_PROGRAM={}, sync_output disabled, fps capped to {:.0}, byte-budget + RIS reset enabled)",
                host_name, caps.default_fps_cap
            ),
        );
        if caps.vscode_integrated {
            output::eprintln_verbose(
                "vscode_integrated:",
                " true (back-compat alias; new code should key off xtermjs_host)",
            );
        }
    }
    output::eprintln_verbose(
        "sync_output:",
        &format!(
            " {} (ESC[?2026h synchronized output framing)",
            caps.sync_output
        ),
    );

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
///
/// Thin wrapper around the shared `palette::format_color_hex` helper so the
/// verbose output and benchmark CONFIG section report identical hex values.
#[must_use]
fn format_bg_color(bg: Option<Color>) -> String {
    palette::format_color_hex(bg)
}

/// Produce a descriptive label for the `color_bg` verbose line.
///
/// Priority contract (matches actual runtime behavior in `app.rs::create_cloud`):
///   1. Custom palette with `bg` field → bg ALWAYS wins (set_palette overwrites
///      `cloud.palette` wholesale, ignoring `--color-bg`).
///   2. Custom palette WITHOUT `bg` field → falls back to `--color-bg` setting.
///   3. No custom palette → `--color-bg` decides (black or default-background).
///
/// v25.17 (ambiguity fix #2): the previous version checked `color_bg` first,
/// which produced the misleading line `color_bg: default-background (terminal
/// native bg, no override)` even when a custom palette's `bg = "#0000ce"`
/// was actively painting the screen blue. The user saw the lie in real time
/// when they switched their mythme palette's bg from #0a0a12 to #0000ce —
/// the screen turned blue but verbose still claimed "no override".
#[must_use]
fn describe_color_bg(
    color_bg: ColorBg,
    custom_palette_name: Option<&str>,
    custom_palette_bg: Option<Color>,
) -> String {
    // Case 1 & 2: custom palette is active. Its bg field (if present) ALWAYS
    // overrides --color-bg, because `cloud.set_palette(custom)` overwrites
    // `cloud.palette` wholesale in app.rs::create_cloud.
    if let Some(name) = custom_palette_name {
        return match custom_palette_bg {
            Some(bg) => {
                let hex = format_bg_color(Some(bg));
                format!(
                    "custom palette '{name}' bg={hex} (palette bg overrides --color-bg)"
                )
            }
            None => match color_bg {
                ColorBg::DefaultBackground => format!(
                    "default-background (custom palette '{name}' has no bg field, terminal native shows through)"
                ),
                ColorBg::Black => format!(
                    "black (custom palette '{name}' has no bg field, solid black)"
                ),
            },
        };
    }

    // Case 3: no custom palette — --color-bg decides alone.
    match color_bg {
        ColorBg::DefaultBackground => {
            "default-background (terminal native bg, no override)".to_string()
        }
        ColorBg::Black => "black (solid black bg)".to_string(),
    }
}
