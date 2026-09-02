// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Verbose startup block — extracted from `main.rs` to keep that file
//! under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns `run_verbose_startup()` — the --verbose pre-launch dump that
//! prints the VerboseCtx (scene/color/glyphs/motion/style/interaction/
//! dragon/ambient/terminal/config sections).

use crate::config::Args;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_verbose_startup(
    args: &Args,
    rain_style: crate::rain_style::RainStyle,
    color_scheme: ColorScheme,
    color_mode: ColorMode,
    color_tune: crate::color_tune::ColorTune,
    #[allow(unused_variables)] custom_palette: &Option<crate::palette::Palette>,
    custom_palette_name: &Option<String>,
    custom_palette_bg: Option<crossterm::style::Color>,
    charset_preset: &str,
    chars: &[char],
    target_fps: f64,
    fps_precedence: &'static str,
    speed: f32,
    base_density: f32,
    density_auto: bool,
    effective_async: bool,
    bold_mode: BoldMode,
    shading_mode: ShadingMode,
    glitch_pct: f32,
    glitch_low: u16,
    glitch_high: u16,
    screen_size: Option<(u16, u16)>,
    bench_mode: bool,
    cli_explicit_color: bool,
    default_message_text: &str,
) {
    // Resolve the intro type label for verbose output. Mirrors the
    // resolution in CloudConfig below: CLI --intro wins, else default
    // Logo. We emit the lowercase value-enum name to match the
    // --intro flag's accepted values (cosmic|logo|none).
    let resolved_intro = args.intro.unwrap_or(crate::intro_style::IntroType::Logo);
    let intro_label = match resolved_intro {
        crate::intro_style::IntroType::Cosmic => "cosmic",
        crate::intro_style::IntroType::Logo => "logo",
        crate::intro_style::IntroType::None => "none",
    };
    let commit_sha = option_env!("COSMOSTRIX_GIT_SHA").unwrap_or("unknown");
    let verbose_ambient_schedule = crate::crystal_dragon_engine::ambient::collect_ambient_schedule(
        &crate::configfile::load_config_file(args.config.as_deref()),
    );
    // v50.0.0-beta.7 LTS: read ambient-snapback-secs directly from config
    // so verbose reports the EFFECTIVE runtime value (user-set), not the
    // constant 30.0 default. Mirrors the live_config apply path: range
    // 0.0..=86400.0; out-of-range parses to None (default 30s).
    // v80.0.0-alpha.2: human-duration forms accepted (30, 30s, 1m, 1h30m)
    // — parse_secs_config shares the CLI flag vocabulary.
    let verbose_ambient_snapback_secs = crate::configfile::load_config_file(args.config.as_deref())
        .get("ambient-snapback-secs")
        .and_then(|v| {
            crate::config_apply::parse_secs_config("ambient-snapback-secs", v, 0.0, 86400.0)
        });
    // v80.0.0-alpha.1: crystal-dragon-secs effective value. args.crystal_dragon_secs
    // already reflects the CLI > config merge (config_apply ran earlier in
    // main(), and main.rs validated the range before this point), so this
    // IS the value the engine will use — no re-reading the file needed.
    let verbose_crystal_dragon_secs = args.crystal_dragon_secs;
    crate::output::verbose::print_verbose(&crate::output::verbose::VerboseCtx {
        version: env!("CARGO_PKG_VERSION"),
        scene_name: args.scene.as_deref(),
        rain_style,
        color_scheme,
        color_mode,
        color_tune,
        color_bg: args.color_bg,
        custom_palette_bg,
        charset_preset,
        chars,
        target_fps,
        fps_precedence,
        speed,
        base_density,
        density_auto,
        monolith_size: args.monolith_size,
        async_mode: effective_async,
        bold_mode,
        shading_mode,
        glitch_enabled: args.glitch_level != crate::config::GlitchLevel::None,
        glitch_pct,
        glitch_low,
        glitch_high,
        glitch_level: &format!("{:?}", args.glitch_level),
        screensaver: args.screensaver,
        crystal_dragon: args.crystal_dragon.unwrap_or(false),
        // v50.0.0-alpha.7: VerboseCtx must reflect the EFFECTIVE message
        // (after msg_mode gate + default fallback). Was dishonest: showed
        // default "cosmostrix v..." even when msg_mode=false suppressed it.
        // Now: if msg_mode=false AND no CLI -m/-mb, message is None.
        message: {
            let msg_mode_on = args.msg_mode.unwrap_or(true);
            let cli_msg = args.message.as_deref();
            if cli_msg.is_some() {
                cli_msg
            } else if !bench_mode && msg_mode_on {
                // Default fallback only fires when msg_mode=true.
                Some(default_message_text.to_string().leak() as &str)
            } else {
                None
            }
        },
        message_border: args.message_border
            || (!bench_mode && args.message.is_none() && args.msg_mode.unwrap_or(true)),
        // v50.0.0-alpha.7: msg_mode field added so verbose can report
        // WHY config message is being ignored (msg_mode=false suppresses
        // config messages; CLI -m/-mb always wins).
        msg_mode: args.msg_mode.unwrap_or(true),
        // v80.0.0-beta.1 msg-fill-style: effective reveal style after the CLI >
        // config resolution (config_apply ran earlier in main(), so
        // args.msg_fill_style already reflects the config key).
        msg_fill_style: args.msg_fill_style,
        duration: args.duration,
        screen_size,
        custom_palette_name: custom_palette_name.as_deref(),
        scene_arg: &args.scene,
        config_path: args.config.as_deref(),
        cli_explicit_color,
        intro_type_label: intro_label,
        commit_sha,
        bench_mode,
        power_dragon: args.power_dragon.unwrap_or(true),
        intro_color: args.intro_color.as_deref(),
        scene_custom: args.scene_custom.as_deref(),
        ambient_schedule: &verbose_ambient_schedule,
        ambient_snapback_secs: verbose_ambient_snapback_secs,
        crystal_dragon_secs: verbose_crystal_dragon_secs,
    });
}
