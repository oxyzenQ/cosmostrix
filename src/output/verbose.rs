// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Verbose diagnostic output for --verbose flag.
//!
//! Extracted from main.rs to keep that file under 800 LOC.
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

/// Aggregated context for `print_verbose()`.
///
/// Replaces the former 30+ parameter list. Construct this in main.rs
/// and pass a single reference — extending verbose output is now O(1)
/// at the call site (add a field to this struct, update the print body).
pub(crate) struct VerboseCtx<'a> {
    pub version: &'a str,
    pub scene_name: Option<&'a str>,
    pub rain_style: RainStyle,
    pub color_scheme: crate::runtime::ColorScheme,
    pub color_mode: ColorMode,
    pub color_tune: ColorTune,
    pub color_bg: ColorBg,
    pub custom_palette_bg: Option<Color>,
    pub charset_preset: &'a str,
    pub chars: &'a [char],
    pub target_fps: f64,
    /// Which resolution layer produced target_fps.
    /// One of: cli / scene / config / dynamic_default / xtermjs_cap.
    pub fps_precedence: &'static str,
    pub speed: f32,
    pub base_density: f32,
    pub density_auto: bool,
    pub monolith_size: MonolithSize,
    pub async_mode: bool,
    pub bold_mode: BoldMode,
    pub shading_mode: ShadingMode,
    pub glitch_enabled: bool,
    pub glitch_pct: f32,
    pub glitch_low: u16,
    pub glitch_high: u16,
    pub glitch_level: &'a str,
    pub screensaver: bool,
    pub crystal_dragon: bool,
    pub message: Option<&'a str>,
    pub message_border: bool,
    /// v50.0.0-alpha.7: msg_mode master switch (true = overlay active,
    /// false = suppressed). Verbose MUST report this so users can see
    /// why their config `message = "hello"` is being ignored (msg_mode=false
    /// suppresses config messages; CLI -m/-mb always wins).
    pub msg_mode: bool,
    /// v80.0.0-beta.1 msg-fill-style: message overlay reveal style label
    /// (typewriter/fade/words/slide/instant/engrave/hologram/
    /// glitch/scorch/cascade). Printed right after the msg_mode/message
    /// lines so the overlay block reads as one unit: master switch →
    /// text → reveal style.
    pub msg_fill_style: crate::msg_fill_style::MsgFillStyle,
    pub duration: Option<f64>,
    pub screen_size: Option<(u16, u16)>,
    pub custom_palette_name: Option<&'a str>,
    pub scene_arg: &'a Option<String>,
    pub config_path: Option<&'a std::path::Path>,
    pub cli_explicit_color: bool,
    pub intro_type_label: &'a str,
    pub commit_sha: &'a str,
    /// --benchmark forces palette drift=false (palette rebuild injectes
    /// timing spikes, breaks p99/max determinism). Pass bench_mode so
    /// verbose can disclose the override BEFORE the benchmark report.
    pub bench_mode: bool,
    /// Power Dragon: when false, disables aggressive_throttle + idle FPS
    /// reduction. Default: true (protection enabled). Config-only toggle.
    pub power_dragon: bool,
    /// Intro color override (config-only). When set, the intro animation
    /// uses this color theme instead of the rain color.
    pub intro_color: Option<&'a str>,
    /// Active custom scene (--scene-custom <name>).
    pub scene_custom: Option<&'a str>,
    /// Ambient schedule (time-of-day scene switching).
    pub ambient_schedule: &'a crate::crystal_dragon_engine::ambient::AmbientSchedule,
    /// v50.0.0-beta.7 LTS: Effective `ambient-snapback-secs` config value
    /// (None = unset in config → runtime falls back to
    /// `AUTO_SNAPBACK_DELAY_SECS` = 30.0). Verbose MUST report the actual
    /// user-set value, NOT the constant — otherwise the user sets
    /// `ambient-snapback-secs = 10` and verbose lies "30s" while the
    /// runtime uses 10s. Owner audit found this dishonesty.
    pub ambient_snapback_secs: Option<f64>,
    /// v80.0.0-alpha.1: effective Crystal Dragon polling interval
    /// (None = unset → runtime falls back to
    /// `CRYSTAL_DRAGON_POLLING_SECS` = 60.0). Same honesty contract as
    /// `ambient_snapback_secs` — the printed value must be the value
    /// the engine will actually use (CLI > config > default).
    pub crystal_dragon_secs: Option<f64>,
}

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

pub(crate) fn print_verbose(ctx: &VerboseCtx) {
    let VerboseCtx {
        version,
        scene_name,
        rain_style,
        color_scheme,
        color_mode,
        color_tune,
        color_bg,
        custom_palette_bg,
        charset_preset,
        chars,
        target_fps,
        fps_precedence,
        speed,
        base_density,
        density_auto,
        monolith_size,
        async_mode,
        bold_mode,
        shading_mode,
        glitch_enabled,
        glitch_pct,
        glitch_low,
        glitch_high,
        glitch_level,
        screensaver,
        crystal_dragon,
        message,
        message_border,
        msg_mode,
        msg_fill_style,
        duration,
        screen_size,
        custom_palette_name,
        scene_arg,
        config_path,
        cli_explicit_color,
        intro_type_label,
        commit_sha,
        bench_mode,
        power_dragon,
        intro_color,
        scene_custom,
        ambient_schedule,
        ambient_snapback_secs,
        crystal_dragon_secs,
    } = ctx;

    let color_source = resolve_color_source(
        *custom_palette_name,
        *cli_explicit_color,
        scene_arg,
        *config_path,
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
    let pipeline = ColorPipeline::detect(*color_mode);
    output::eprintln_verbose(
        "color_pipeline:",
        &format!(" {} ({})", pipeline.label(), pipeline.description()),
    );
    if pipeline.is_chroma() {
        let jitter_amp = crate::chroma_dragon_engine::tuning::SUBPIXEL_JITTER_AMPLITUDE;
        let halo_factor = crate::chroma_dragon_engine::tuning::HEAD_HALO_FACTOR;
        output::eprintln_verbose(
            "  chroma_features:",
            &format!(
                " oklab_gradient, perceptual_blend, climate_post_fx, head_halo(factor={halo_factor:.2}), l_smoothing, subpixel_jitter(amplitude={jitter_amp})"
            ),
        );
    } else if let Some(reason) = pipeline.disable_reason(*color_mode) {
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
    let bg_label = describe_color_bg(*color_bg, *custom_palette_name, *custom_palette_bg);
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
    let async_desc = if *async_mode {
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
            " {} ({glitch_pct:.1}%, {glitch_low}-{glitch_high}ms)",
            glitch_enabled
        ),
    );
    output::eprintln_verbose("glitch_level:", &format!(" {glitch_level}"));

    // ── Interaction ───────────────────────────────────────────────
    eprintln!("{}", output::brand_bold("  ── Interaction ──"));
    output::eprintln_verbose("mouse:", " always-on (glow + click wave)");
    output::eprintln_verbose("screensaver:", &format!(" {screensaver}"));
    output::eprintln_verbose("intro:", &format!(" {intro_type_label}"));
    if let Some(ic) = intro_color {
        output::eprintln_verbose("intro_color:", &format!(" {ic}"));
    } else {
        // Honest reporting: when intro_color is None, say so explicitly
        // rather than omitting the line. Users editing config need to see
        // whether their intro-color key was accepted or rejected.
        output::eprintln_verbose("intro_color:", " (unset — intro uses brand energy-zen)");
    }
    // v50.0.0-alpha.7: msg_mode master switch — always print so users see
    // WHY their config `message = "hello"` is being ignored (msg_mode=false
    // suppresses config messages; CLI -m/-mb always wins over msg_mode).
    let msg_mode_desc = if *msg_mode {
        "true (overlay active — config message/message-border honored)"
    } else {
        "false (overlay suppressed — config message/message-border ignored; CLI -m/-mb still works)"
    };
    output::eprintln_verbose("msg_mode:", &format!(" {msg_mode_desc}"));
    if let Some(msg) = message {
        output::eprintln_verbose(
            "message:",
            &format!(
                " \"{msg}\" ({} chars, border: {message_border})",
                msg.chars().count()
            ),
        );
    } else if *msg_mode {
        // msg_mode=true but no message → honest reporting.
        output::eprintln_verbose(
            "message:",
            " (none — no CLI -m/-mb, no config message, no default fallback)",
        );
    } else {
        // msg_mode=false → message is None by design.
        output::eprintln_verbose("message:", " (suppressed by msg_mode=false)");
    }
    // v80.0.0-beta.1 msg-fill-style: always printed (even when msg_mode=false) so
    // users editing config can confirm the key was accepted — mirrors
    // the honest-reporting policy of the intro_color line above.
    output::eprintln_verbose(
        "msg_fill_style:",
        &format!(" {}", msg_fill_style.verbose_label()),
    );
    if let Some(d) = duration {
        output::eprintln_verbose("duration:", &format!(" {d:.1}s"));
    }

    // ── Dragon Systems ──────────────────────────────────────────────
    eprintln!("{}", output::brand_bold("  ── Dragon Systems ──"));
    output::eprintln_verbose(
        "power_dragon:",
        &format!(" {power_dragon} (aggressive throttle + idle FPS reduction)"),
    );
    output::eprintln_verbose("crystal_dragon:", &format!(" {crystal_dragon}"));
    let palette_drift_label = if *bench_mode && *crystal_dragon {
        "enabled (overridden to disabled in benchmark mode — see note below)"
    } else if *crystal_dragon {
        if custom_palette_name.is_some() {
            "enabled (custom palette active — first drift replaces it with builtin)"
        } else {
            "enabled"
        }
    } else {
        "disabled"
    };
    let tick_secs = crate::central_control_rains::COLOR_ECOSYSTEM_TICK_SECS;
    output::eprintln_verbose(
        "  palette_drift:",
        &format!(" {palette_drift_label} (tick every {tick_secs:.1}s)"),
    );
    if *bench_mode && *crystal_dragon {
        output::eprintln_verbose(
            "  bench_override:",
            " palette drift forced OFF during benchmark for deterministic p99/max metrics",
        );
    }
    output::eprintln_verbose(
        "  climate_drift:",
        " always-on (luminance/saturation/hue accumulate regardless of crystal_dragon flag)",
    );
    // v80.0.0-alpha.1: report the EFFECTIVE polling interval with its
    // provenance — same honesty contract as the ambient_snapback_secs line
    // below (the printed value must be the value the engine will use).
    // Also prints the harmony hint so users combining ambient + crystal
    // dragon can read the drift-cycle math right off the verbose dump.
    let effective_cd_secs = crystal_dragon_secs.unwrap_or(
        crate::crystal_dragon_engine::crystal_dragon_control::CRYSTAL_DRAGON_POLLING_SECS as f64,
    );
    let cd_secs_src = if crystal_dragon_secs.is_some() {
        "CLI/config"
    } else {
        "default (unset — 60.0s)"
    };
    output::eprintln_verbose(
        "crystal_dragon_secs:",
        &format!(
            " {effective_cd_secs:.1}s ({cd_secs_src}; drift cadence — keep ambient-snapback-secs below this for a clean drift cycle, min-dwell floor min(60s, cadence))"
        ),
    );

    // ── Ambient ───────────────────────────────────────────────────
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
        let idle_secs = crate::central_control_dragon_power::IDLE_THRESHOLD_SECS;
        // v50.0.0-beta.7 LTS audit: verbose MUST report the actual effective
        // snapback delay (user-set config value, not the constant). Before
        // this fix, `ambient-snapback-secs = 10` in config.toml produced a
        // dishonest "30.0s" line in verbose while the runtime used 10s —
        // owner found this while debugging crystal-dragon drift visibility.
        let effective_snapback = ambient_snapback_secs
            .unwrap_or(crate::central_control_dragon_power::AUTO_SNAPBACK_DELAY_SECS);
        let snapback_src = if ambient_snapback_secs.is_some() {
            "from config"
        } else {
            "default (unset in config)"
        };
        output::eprintln_verbose(
            "ambient_snapback_secs:",
            &format!(
                " {effective_snapback:.1}s ({snapback_src} — drift visible for {effective_snapback:.1}s before ambient reverts)"
            ),
        );
        output::eprintln_verbose(
            "auto_snapback:",
            &format!(
                " {idle_secs:.1}s idle threshold, {effective_snapback:.1}s snapback delay (user overrides via 'c'/'C'/'x'/'s' revert after {effective_snapback:.1}s)"
            ),
        );
    }

    // ── Terminal ──────────────────────────────────────────────────
    eprintln!("{}", output::brand_bold("  ── Terminal ──"));
    let (sw, sh, size_mode) = match screen_size {
        Some((w, h)) => (*w, *h, "fixed"),
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
            output::eprintln_verbose("vscode_integrated:", " true (detected)");
        }
    }
    output::eprintln_verbose(
        "sync_output:",
        &format!(
            " {} (ESC[?2026h synchronized output framing)",
            caps.sync_output
        ),
    );
    // S-master-HUNT-24: surface the effects auto-gate decision so the
    // user can verify WHY effects are (not) disabled — the same
    // [auto-fx] runtime diagnostic pushed at startup.
    if caps.cpu_rendered || caps.console_tty {
        output::eprintln_verbose(
            "effects_gate:",
            &format!(
                " auto-disabled ({} — cosmetic effects off; rain-core visuals unaffected)",
                caps.effects_gate_source
            ),
        );
    } else {
        output::eprintln_verbose(
            "effects_gate:",
            " on (no CPU-renderer/console marker detected; runtime congestion gate armed)",
        );
    }

    // ── Config ────────────────────────────────────────────────────
    eprintln!("{}", output::brand_bold("  ── Config ──"));
    let resolved_config_path = if let Some(p) = config_path {
        p.to_path_buf()
    } else {
        let (resolved, _) = configfile::resolve_watcher_config_path(None);
        resolved
    };
    output::eprintln_verbose(
        "config_path:",
        &format!(" {}", resolved_config_path.display()),
    );
    output::eprintln_verbose(
        "config exists:",
        &format!(" {}", resolved_config_path.exists()),
    );
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
/// (ambiguity fix #2): the previous version checked `color_bg` first,
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
    // overrides --color-bg, because `cloud.set_palette(name, custom)` overwrites
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
