// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Final-state tracker cases. The FINAL_* statics are process-wide
//! OnceLocks shared by every test in the binary, so the defaults and the
//! round-trip are asserted in ONE deterministic sequence (defaults first
//! — a pristine binary never ran set_final_state).
//!
//! NIGHT-hunter-22: the printers now exchange the `SessionState`
//! value struct instead of 25 positional params, so the round-trip
//! constructs the struct literal directly and the two constructors
//! (`from_startup`, `from_live`) get their own mapping tests — they are
//! the production builders for the post-exit diff.

use super::super::*;
use crate::runtime::ColorScheme;

/// Duplicated from `tests_v51_intro_brand_pause.rs::make_test_config()`
/// (test fixture, stable) — the baseline the constructor assertions read.
fn make_test_config() -> crate::CloudConfig {
    crate::CloudConfig {
        color_mode: crate::runtime::ColorMode::TrueColor,
        shading_mode: crate::runtime::ShadingMode::Random,
        bold_mode: crate::runtime::BoldMode::Off,
        async_mode: false,
        default_bg: true,
        color_scheme: ColorScheme::NeonGreen,
        custom_palette: None,
        custom_palette_name: None,
        rain_style: crate::rain_style::RainStyle::Glyph,
        glitch_enabled: false,
        glitch_level: crate::config::GlitchLevel::None,
        glitch_pct: 0.0,
        glitch_low: 0,
        glitch_high: 0,
        linger_low: 0,
        linger_high: 0,
        short_pct: 0.0,
        die_early_pct: 0.0,
        max_dpc: 1,
        density: 0.8,
        speed: 8.0,
        monolith_size: crate::runtime::MonolithSize::Normal,
        chars: vec!['0', '1'],
        message: None,
        message_border: false,
        msg_fill_style: crate::msg_fill_style::MsgFillStyle::Typewriter,
        target_fps: 60.0,
        xtermjs_host: false,
        default_fps_cap: 240.0,
        duration_s: None,
        bench_frames: None,
        benchmark: false,
        bench_duration: None,
        save_baseline: None,
        compare_baseline: None,
        bench_io: false,
        bench_all: false,
        bench_scene: None,
        screen_size: None,
        color_tune: crate::color_tune::ColorTune::IDENTITY,
        json: false,
        verbose: false,
        density_auto: false,
        base_density: 0.8,
        perf_stats: false,
        screensaver: false,
        intro: crate::intro_style::IntroType::None,
        intro_color: None,
        mouse: false,
        charset_preset: String::from("binary"),
        user_ranges: vec![],
        def_ascii: true,
        crystal_dragon: false,
        power_dragon: true,
        msg_mode: true,
        effects_enabled: true,
        config_path_for_watcher: None,
        scene_name: "monolith".to_string(),
        scene_custom_name: None,
        scene_custom_config_owned: false,
        cli_explicit: crate::app::CliExplicit::default(),
        ambient_schedule: crate::crystal_dragon_engine::ambient::AmbientSchedule::default(),
        ambient_snapback_secs: None,
        crystal_dragon_secs: None,
    }
}

#[test]
fn final_state_defaults_then_round_trip() {
    // 1) Early-exit path (set_final_state never ran): the accessors
    //    degrade to the same values a default startup would show, so
    //    the post-exit section stays honest instead of printing
    //    arbitrary zeros. NIGHT-hunter-22: the default block now pins
    //    ALL 23 accessors (the v50 family was previously untested —
    //    only the v80 additions had default assertions).
    assert_eq!(last_color_scheme(), "cosmos");
    assert_eq!(last_scene_name(), "monolith");
    assert_eq!(last_charset_preset(), "binary");
    assert_eq!(last_speed(), 9.0);
    assert_eq!(last_density(), 0.75);
    assert!(last_msg_mode());
    assert_eq!(last_message(), None);
    assert!(!last_message_border());
    assert_eq!(last_msg_fill_style(), "typewriter");
    assert!(last_power_dragon());
    assert!(!last_crystal_dragon());
    assert!(last_async_mode());
    assert_eq!(last_intro_color(), None);
    assert_eq!(last_ambient_snapback_secs(), None);
    assert_eq!(last_ambient_entries(), 0);
    assert_eq!(last_fps(), 60.0);
    assert_eq!(last_crystal_dragon_secs(), None);
    assert_eq!(last_glitch_level(), "Subtle");
    assert_eq!(last_bold_mode(), "Random");
    assert_eq!(last_shading_mode(), "DistanceFromHead");
    assert_eq!(last_monolith_size(), "Normal");
    assert!(!last_color_bg());
    assert_eq!(
        last_color_tune(),
        "sat=1.00 bright=1.00 head=1.00 body=1.00 tail=1.00"
    );

    // 2) Round-trip: every field stored by set_final_state is read back
    //    verbatim (labels are the Debug formats the printer compares for
    //    the (was X) suffixes). NIGHT-hunter-22: the 25 positional params
    //    are now named fields — the round-trip covers the full 23-field
    //    storage mapping, not just the v80 additions.
    set_final_state(SessionState {
        color: "Green".to_string(),
        scene: "cinematic".to_string(),
        charset: "zen".to_string(),
        speed: 9.0,
        density: 0.75,
        msg_mode: true,
        message: Some("msg".to_string()),
        message_border: true,
        msg_fill_style: "engrave".to_string(),
        power_dragon: true,
        crystal_dragon: false,
        async_mode: true,
        intro_color: None,
        ambient_snapback_secs: Some(30.0),
        ambient_entries: 1,
        crystal_dragon_secs: Some(45.0),
        fps: 12.0,
        glitch_level: "None".to_string(),
        bold_mode: "Off".to_string(),
        shading_mode: "Random".to_string(),
        monolith_size: "Large".to_string(),
        color_bg: true,
        color_tune: "sat=1.20 bright=1.00 head=1.00 body=1.00 tail=1.00".to_string(),
    });
    assert_eq!(last_color_scheme(), "Green");
    assert_eq!(last_scene_name(), "cinematic");
    assert_eq!(last_charset_preset(), "zen");
    assert_eq!(last_speed(), 9.0);
    assert_eq!(last_density(), 0.75);
    assert!(last_msg_mode());
    assert_eq!(last_message(), Some("msg"));
    assert!(last_message_border());
    assert_eq!(last_msg_fill_style(), "engrave");
    assert!(last_power_dragon());
    assert!(!last_crystal_dragon());
    assert!(last_async_mode());
    assert_eq!(last_intro_color(), None);
    assert_eq!(last_ambient_snapback_secs(), Some(30.0));
    assert_eq!(last_ambient_entries(), 1);
    assert_eq!(last_crystal_dragon_secs(), Some(45.0));
    assert_eq!(last_fps(), 12.0);
    assert_eq!(last_glitch_level(), "None");
    assert_eq!(last_bold_mode(), "Off");
    assert_eq!(last_shading_mode(), "Random");
    assert_eq!(last_monolith_size(), "Large");
    assert!(last_color_bg());
    assert_eq!(
        last_color_tune(),
        "sat=1.20 bright=1.00 head=1.00 body=1.00 tail=1.00"
    );
}

#[test]
fn session_state_from_startup_maps_config_fields() {
    let cfg = make_test_config();
    let state = SessionState::from_startup(&cfg, ColorScheme::NeonGreen);

    // Core mapping: every live-reload-able dimension lands in the
    // snapshot with the config resolution (post CLI > config > scene).
    assert_eq!(state.color, "NeonGreen");
    assert_eq!(state.scene, "monolith");
    assert_eq!(state.charset, "binary");
    assert_eq!(state.speed, 8.0);
    assert_eq!(state.density, 0.8);
    assert!(state.msg_mode);
    assert_eq!(state.message, None);
    assert!(!state.message_border);
    assert_eq!(state.msg_fill_style, "typewriter");
    assert!(state.power_dragon);
    assert!(!state.crystal_dragon);
    assert!(!state.async_mode);
    assert_eq!(state.intro_color, None);
    assert_eq!(state.ambient_snapback_secs, None);
    assert_eq!(state.ambient_entries, 0);
    assert_eq!(state.crystal_dragon_secs, None);
    assert_eq!(state.fps, 60.0);
    assert_eq!(state.glitch_level, "None");
    assert_eq!(state.bold_mode, "Off");
    assert_eq!(state.shading_mode, "Random");
    assert_eq!(state.monolith_size, "Normal");
    assert!(state.color_bg);
    assert_eq!(
        state.color_tune,
        "sat=1.00 bright=1.00 head=1.00 body=1.00 tail=1.00"
    );
}

#[test]
fn session_state_from_startup_labels_custom_palette() {
    let mut cfg = make_test_config();
    cfg.custom_palette_name = Some("diamondish".to_string());
    let state = SessionState::from_startup(&cfg, ColorScheme::NeonGreen);
    // The startup label mirrors the exit label's shape so a mid-run
    // palette switch diffs cleanly ("diamondish (custom)" -> "NeonBlue").
    assert_eq!(state.color, "diamondish (custom)");
}

#[test]
fn session_state_from_live_reads_cloud_not_config() {
    let cfg = make_test_config();
    let mut cloud = cfg.create_cloud(0.65);
    // A mid-session edit writes the CLOUD trackers (this is exactly what
    // the ambient/live-reload paths do); the config copy is stale.
    cloud.set_droplet_density(0.42);
    cloud.set_chars_per_sec(3.5);

    let state = SessionState::from_live(&cloud, &cfg, "matrix", "katakana");

    // Live values win for the cloud-owned dimensions.
    assert_eq!(state.density, 0.42);
    assert_eq!(state.speed, 3.5);
    assert_eq!(state.scene, "matrix");
    assert_eq!(state.charset, "katakana");
    assert_eq!(state.color, "NeonGreen");
    assert_eq!(state.glitch_level, "None");
    // Config-owned dimensions still read the config (fixture values).
    assert_eq!(state.fps, 60.0);
    assert_eq!(state.ambient_entries, 0);
    assert_eq!(state.msg_fill_style, "typewriter");
}
