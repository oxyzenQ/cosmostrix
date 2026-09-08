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

/// Inputs collected in `main()` for the `--verbose` pre-launch dump.
///
/// NIGHT-hunter-25: the 25-positional-parameter signature is bundled
/// into one value object (the same pattern as hunter-22's
/// `CfgInputs`) — the printer reads named fields and the single call
/// site in main.rs constructs them by name, so same-typed neighbors
/// (two f32 densities, three u16 glitch bounds, fps/speed pairs) can
/// no longer cross-wire. The dead `custom_palette` param (never read
/// by the body — its `allow(unused_variables)` was the band-aid) is
/// dropped outright: the dump prints the palette NAME and BG, both
/// still carried here.
pub(crate) struct VerboseInputs<'a> {
    /// CLI args (scene/intro/config path/fps precedence source).
    pub args: &'a Args,
    /// Resolved rain style.
    pub rain_style: crate::rain_style::RainStyle,
    /// Resolved color scheme.
    pub color_scheme: ColorScheme,
    /// Resolved color mode.
    pub color_mode: ColorMode,
    /// Resolved color tune.
    pub color_tune: crate::color_tune::ColorTune,
    /// Active custom palette NAME (None when a builtin scheme runs).
    pub custom_palette_name: &'a Option<String>,
    /// Active custom palette BG color (None = terminal default).
    pub custom_palette_bg: Option<crossterm::style::Color>,
    /// Resolved charset preset label.
    pub charset_preset: &'a str,
    /// Resolved charset glyphs.
    pub chars: &'a [char],
    /// Target FPS.
    pub target_fps: f64,
    /// Which source won the FPS resolution.
    pub fps_precedence: &'static str,
    /// Rain speed.
    pub speed: f32,
    /// Base density.
    pub base_density: f32,
    /// Whether density auto-adjusted from terminal size.
    pub density_auto: bool,
    /// Effective async mode.
    pub effective_async: bool,
    /// Bold mode.
    pub bold_mode: BoldMode,
    /// Shading mode.
    pub shading_mode: ShadingMode,
    /// Glitch percentage.
    pub glitch_pct: f32,
    /// Glitch low bound.
    pub glitch_low: u16,
    /// Glitch high bound.
    pub glitch_high: u16,
    /// Parsed --screen-size (validated once in main).
    pub screen_size: Option<(u16, u16)>,
    /// Bench mode flag.
    pub bench_mode: bool,
    /// Whether the color came from an explicit CLI flag.
    pub cli_explicit_color: bool,
    /// Default message text.
    pub default_message_text: &'a str,
}

pub(crate) fn run_verbose_startup(vi: VerboseInputs<'_>) {
    let VerboseInputs {
        args,
        rain_style,
        color_scheme,
        color_mode,
        color_tune,
        custom_palette_name,
        custom_palette_bg,
        charset_preset,
        chars,
        target_fps,
        fps_precedence,
        speed,
        base_density,
        density_auto,
        effective_async,
        bold_mode,
        shading_mode,
        glitch_pct,
        glitch_low,
        glitch_high,
        screen_size,
        bench_mode,
        cli_explicit_color,
        default_message_text,
    } = vi;
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
