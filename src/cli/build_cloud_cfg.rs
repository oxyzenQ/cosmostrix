// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! CloudConfig construction — extracted from `main.rs` to keep that file
//! under the 800-LOC cap. Pure code motion — no behavior change.
//!
//! Builds the final `CloudConfig` from validated CLI args + config file
//! values. The ~155-field struct literal is the largest single block in
//! main.rs; extracting it as a function keeps main.rs readable.

use crate::app::CloudConfig;
use crate::config::Args;
use crate::configfile;
use crate::termdetect::TerminalCaps;
use crate::ux;

use crate::color_tune::ColorTune;
use crate::palette::Palette;
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};
use crate::types::constants::{default_message_text, MESSAGE_MAX_LEN};

/// Inputs collected from CLI arg validation in `main()`.
#[allow(clippy::too_many_arguments, clippy::missing_docs_in_private_items)]
pub(crate) struct CfgInputs<'a> {
    pub args: &'a Args,
    pub color_mode: ColorMode,
    pub shading_mode: ShadingMode,
    pub bold_mode: BoldMode,
    pub effective_async: bool,
    pub default_bg: bool,
    pub color_scheme: ColorScheme,
    pub custom_palette: Option<Palette>,
    pub custom_palette_name: Option<String>,
    pub rain_style: RainStyle,
    pub glitch_pct: f32,
    pub glitch_low: u16,
    pub glitch_high: u16,
    pub linger_low: u16,
    pub linger_high: u16,
    pub short_pct: f32,
    pub die_early_pct: f32,
    pub max_dpc: u8,
    pub base_density: f32,
    pub density_auto: bool,
    pub speed: f32,
    pub chars: Vec<char>,
    pub charset_preset: String,
    pub user_ranges: Vec<(char, char)>,
    pub def_ascii: bool,
    pub target_fps: f64,
    pub color_tune: ColorTune,
    pub screen_size: Option<(u16, u16)>,
    pub term_caps: &'a TerminalCaps,
    pub duration_s: Option<f64>,
    pub bench_mode: bool,
    pub monolith_density_map: Option<&'static [f64]>,
    pub cli_explicit: crate::app::CliExplicit,
}

/// Build the final `CloudConfig` from validated inputs.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_cloud_cfg(inp: CfgInputs<'_>) -> CloudConfig {
    // Note: `args` is `&Args` (immutable). The original main.rs used
    // `args.intro_color.take()` which mutates args. Since this function
    // receives an immutable reference, we clone instead — intro_color is
    // an Option<String>, cheap to clone once at startup.
    let CfgInputs {
        args,
        color_mode,
        shading_mode,
        bold_mode,
        effective_async,
        default_bg,
        color_scheme,
        custom_palette,
        custom_palette_name,
        rain_style,
        glitch_pct,
        glitch_low,
        glitch_high,
        linger_low,
        linger_high,
        short_pct,
        die_early_pct,
        max_dpc,
        base_density,
        density_auto,
        speed,
        chars,
        charset_preset,
        user_ranges,
        def_ascii,
        target_fps,
        color_tune,
        screen_size,
        term_caps,
        duration_s,
        bench_mode,
        monolith_density_map,
        cli_explicit,
    } = inp;
    let cloud_cfg = CloudConfig {
        color_mode,
        shading_mode,
        bold_mode,
        async_mode: effective_async,
        default_bg,
        color_scheme,
        custom_palette,
        custom_palette_name,
        rain_style,
        glitch_enabled: args.glitch_level != crate::config::GlitchLevel::None,
        glitch_level: args.glitch_level,
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
        monolith_size: args.monolith_size,
        chars,
        // v50-beta.3: msg-mode gate + default message fallback.
        // Precedence (highest wins):
        //   1. CLI -m / -mb (always active — CLI wins over msg-mode=false)
        //   2. msg-mode=false → disable BOTH default AND config message
        //      (user must set msg-mode=true to use message/message-border config)
        //   3. config `message` / `message-border` (when msg-mode=true)
        //   4. default fallback "cosmostrix v<CARGO_PKG_VERSION>" with border
        //      (only when !bench_mode AND msg-mode=true)
        // Benchmark mode never shows a message overlay (keeps reports clean).
        // Version is dynamic (env! CARGO_PKG_VERSION), never hardcoded.
        message: {
            // msg_mode_effective: CLI flag wins (already applied via config_value
            // is_explicit); default true when neither CLI nor config sets it.
            let msg_mode_on = args.msg_mode.unwrap_or(true);
            // CLI explicit? Check is via clap's value_source — but for -m / -mb
            // we already have args.message set with the text. So:
            //   - If args.message is Some AND was set via CLI → always show
            //   - If args.message is Some AND was set via config → only if msg_mode_on
            //   - If args.message is None AND !bench_mode AND msg_mode_on → default
            // We can't easily distinguish CLI vs config origin here without
            // tracking is_explicit for the message flag. Instead: trust the
            // config_apply layer — when msg-mode=false AND no CLI -m/-mb,
            // args.message should already be None. main.rs doesn't need to
            // re-check. The msg_mode_on flag here only affects the DEFAULT
            // fallback (when args.message is None).
            let msg: Option<String> = if !bench_mode && args.message.is_none() && msg_mode_on {
                Some(default_message_text())
            } else {
                args.message.clone()
            };
            msg.as_deref().map(|m| {
                if m.len() > MESSAGE_MAX_LEN {
                    ux::die_input(format!(
                        "error: -m text exceeds {MESSAGE_MAX_LEN} character limit (got {})",
                        m.len()
                    ));
                }
                crate::message::sanitize_message_text(m)
            })
        },
        // v50: When the default message fallback fired (args.message was
        // None and !bench_mode), force border=true so the overlay looks
        // intentional. When the user explicitly set -m (no border), keep
        // their choice.
        message_border: args.message_border || (!bench_mode && args.message.is_none()),
        // v80.0.0-beta.1 msg-fill-style: pass the resolved reveal style through
        // (CLI -mfs/--msg-fill-style wins over the config key — resolved
        // earlier in config_apply; default typewriter).
        msg_fill_style: args.msg_fill_style,
        target_fps,
        xtermjs_host: term_caps.xtermjs_host, // (FPS-F1): live-reload cap
        default_fps_cap: term_caps.default_fps_cap,
        duration: args.duration,
        duration_s,
        bench_frames: args.bench_frames,
        benchmark: args.benchmark,
        bench_duration: crate::bench_helpers::resolve_bench_duration_args(&args.bench_duration),
        screen_size,
        color_tune,
        json: args.json,
        save_baseline: args.save_baseline.clone(),
        compare_baseline: args.compare_baseline.clone(),
        bench_io: args.bench_io,
        bench_all: args.bench_all,
        bench_scene: args.bench_scene.clone(),
        verbose: args.verbose,
        density_auto,
        base_density,
        perf_stats: args.perf_stats,
        screensaver: args.screensaver,
        intro: args.intro.unwrap_or(crate::intro_style::IntroType::Logo),
        intro_color: args.intro_color.clone(),
        mouse: true, // v17: always-on (--mouse flag deleted)
        charset_preset,
        user_ranges,
        def_ascii,
        crystal_dragon: args.crystal_dragon.unwrap_or(false),
        power_dragon: args.power_dragon.unwrap_or(true),
        msg_mode: args.msg_mode.unwrap_or(true),
        // Auto-disable particle effects in bench mode — particles are
        // input-driven (mouse clicks, border touches) and never spawn
        // during a benchmark run. This means `cosmostrix --benchmark`
        // is equivalent to `cosmostrix --benchmark --no-effects` — the
        // user no longer needs to pass --no-effects explicitly to get
        // the cleanest bench numbers. The bench CONFIG report's
        // `no_effects` field will automatically show `true` for any
        // bench mode (--benchmark, --bench-all, --bench-frames).
        effects_enabled: !args.no_effects && !bench_mode,
        monolith_density_map,
        config_path_for_watcher: {
            // Termux fix: multi-candidate path resolution so the
            // watcher watches the file the user is ACTUALLY editing. On
            // Termux with XDG_CONFIG_HOME=$PREFIX/etc, the old single-
            // candidate resolver picked a system path the user wasn't
            // editing. The new resolver prioritizes $HOME/.config.
            let (resolved, existed) =
                configfile::resolve_watcher_config_path(args.config.as_deref());
            if crate::live_config_trace::live_reload_debug_enabled() {
                crate::live_config_trace::debug_trace(format_args!(
                    "watcher path resolved: {} (existed candidates: {})\n",
                    resolved.display(),
                    existed
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            Some(resolved)
        },
        scene_name: args
            .scene
            .as_deref()
            .unwrap_or(crate::scene::DEFAULT_SCENE)
            .to_string(),
        // v20: track active custom scene name so live reload can re-apply
        // its fields when the user edits [scene-custom.<name>] in config.
        scene_custom_name: args.scene_custom.clone(),
        // Bug 3: tracker for CLI-explicit flags. v80.0.0-beta.1 owner contract: the
        // flags are the CLI LOCK — startup bakes CLI > config.toml > scene
        // defaults into this config; at runtime a config key overrides the
        // flag only while present (rebuild_cloud_config + the event-loop
        // scene-family restore fall back to these locked values when the
        // key is commented out).
        cli_explicit,
        // Ambient phase schedule (config-only). Collected from
        // `ambient.<HH-MM>` keys; empty = scheduler idles.
        ambient_schedule: crate::crystal_dragon_engine::ambient::collect_ambient_schedule(
            &configfile::load_config_file(args.config.as_deref()),
        ),
        // v50.0.0-beta.7: ambient-snapback-secs config key (config-only,
        // no CLI flag). None = use default AUTO_SNAPBACK_DELAY_SECS (30.0).
        // Range 0.0..=86400.0 validated by parse_f64_config. Invalid values
        // emit a startup error and fall back to None (default).
        ambient_snapback_secs: configfile::load_config_file(args.config.as_deref())
            .get("ambient-snapback-secs")
            .and_then(|v| {
                crate::config_apply::parse_f64_config("ambient-snapback-secs", v, 0.0, 86400.0)
            }),
    };
    cloud_cfg
}
