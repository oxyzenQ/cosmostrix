// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

use super::*;

#[test]
fn validate_rejects_invalid_speed() {
    let mut cfg = HashMap::new();
    cfg.insert("speed".to_string(), "100000".to_string());
    let result = crate::testconf::validate_config_strictly(&cfg);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("speed"));
}

#[test]
fn validate_rejects_invalid_density() {
    let mut cfg = HashMap::new();
    cfg.insert("density".to_string(), "99.0".to_string());
    let result = crate::testconf::validate_config_strictly(&cfg);
    assert!(result.is_err());
}

#[test]
fn validate_accepts_valid_config() {
    let mut cfg = HashMap::new();
    cfg.insert("speed".to_string(), "30".to_string());
    cfg.insert("density".to_string(), "0.85".to_string());
    cfg.insert("fps".to_string(), "60".to_string());
    let result = crate::testconf::validate_config_strictly(&cfg);
    assert!(result.is_ok());
}

#[test]
fn validate_skips_block_keys() {
    let mut cfg = HashMap::new();
    cfg.insert(
        "scene-custom.test.base-scene".to_string(),
        "monolith".to_string(),
    );
    cfg.insert("speed".to_string(), "30".to_string());
    let result = crate::testconf::validate_config_strictly(&cfg);
    assert!(result.is_ok());
}

#[test]
fn validate_rejects_invalid_charset() {
    let mut cfg = HashMap::new();
    cfg.insert("charset".to_string(), "hackeres".to_string());
    let result = crate::testconf::validate_config_strictly(&cfg);
    assert!(result.is_err());
}

#[test]
fn validate_rejects_invalid_atmosphere_regime() {
    // atmosphere-regime is a removed key (atmosphere engine eliminated).
    // It is rejected as an unknown key by parse_config_text (not by
    // validate_config_strictly, which only validates values for known
    // keys). (CLI-D-3): the dead validator for this key was
    // removed; this test now verifies the actual rejection path.
    let cfg_text = "atmosphere-regime = \"adaptivee\"\n";
    let parsed = crate::configfile::parse_config_text(cfg_text);
    assert!(
        parsed.unknown_keys.iter().any(|k| k == "atmosphere-regime"),
        "atmosphere-regime should be classified as unknown: {:?}",
        parsed.unknown_keys
    );
}

// ── Termux fix: triple-signal tests live in
// `live_config_poll::tests` (split keeps this file under LOC cap).

// ── v20: scene-custom live reload tests ──

/// Build a minimal CloudConfig for testing rebuild_cloud_config.
pub(super) fn minimal_cloud_config() -> crate::app::CloudConfig {
    use crate::rain_style::RainStyle;
    use crate::runtime::{BoldMode, ColorMode, ColorScheme, MonolithSize, ShadingMode};

    crate::app::CloudConfig {
        color_mode: ColorMode::TrueColor,
        shading_mode: ShadingMode::Random,
        bold_mode: BoldMode::Random,
        async_mode: true,
        default_bg: true,
        color_scheme: ColorScheme::NeonPurple,
        custom_palette: None,
        custom_palette_name: None,
        rain_style: RainStyle::Glyph,
        glitch_enabled: true,
        glitch_level: crate::config::GlitchLevel::Default,
        glitch_pct: 10.0,
        glitch_low: 300,
        glitch_high: 400,
        linger_low: 400,
        linger_high: 600,
        short_pct: 50.0,
        die_early_pct: 33.0,
        max_dpc: 5,
        density: 0.75,
        speed: 9.0,
        monolith_size: MonolithSize::Normal,
        chars: vec!['0', '1'],
        message: None,
        message_border: false,
        target_fps: 60.0,
        xtermjs_host: false,
        default_fps_cap: 240.0,
        duration: None,
        duration_s: None,
        bench_frames: None,
        benchmark: false,
        bench_duration: None,
        screen_size: None,
        color_tune: crate::color_tune::ColorTune::IDENTITY,
        json: false,
        save_baseline: None,
        compare_baseline: None,
        bench_io: false,
        bench_all: false,
        bench_scene: None,
        verbose: false,
        density_auto: true,
        base_density: 0.75,
        perf_stats: false,
        screensaver: false,
        intro: crate::config::IntroType::None,
        intro_color: None,
        mouse: false,
        charset_preset: "binary".to_string(),
        user_ranges: vec![],
        def_ascii: false,
        crystal_dragon: false,
        power_dragon: true,
        msg_mode: true,
        effects_enabled: true,
        monolith_density_map: None,
        config_path_for_watcher: None,
        scene_name: "test-scene".to_string(),
        scene_custom_name: Some("test-scene".to_string()),
        cli_explicit: crate::app::CliExplicit::default(),
        ambient_schedule: crate::crystal_dragon_engine::ambient::AmbientSchedule::default(),
        ambient_snapback_secs: None,
    }
}

#[test]
fn rebuild_applies_scene_custom_color_change() {
    let mut cfg = HashMap::new();
    cfg.insert(
        "scene-custom.test-scene.color".to_string(),
        "green".to_string(),
    );
    let base = minimal_cloud_config();
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(new.color_scheme, crate::runtime::ColorScheme::Green);
    assert_eq!(new.scene_name, "test-scene");
}

/// user color wins over scene default (depth-test bug fix).
#[test]
fn rebuild_user_color_wins_over_scene_default() {
    let mut cfg = HashMap::new();
    cfg.insert("color".to_string(), "cosmos".to_string());
    cfg.insert("scene".to_string(), "carbonic".to_string());
    let new = rebuild_cloud_config(&minimal_cloud_config(), &cfg);
    assert_eq!(new.color_scheme, crate::runtime::ColorScheme::Cosmos);
}

/// user charset wins over scene default (depth-test bug fix).
#[test]
fn rebuild_user_charset_wins_over_scene_default() {
    let mut cfg = HashMap::new();
    cfg.insert("charset".to_string(), "retro".to_string());
    cfg.insert("scene".to_string(), "carbonic".to_string());
    let new = rebuild_cloud_config(&minimal_cloud_config(), &cfg);
    assert_eq!(new.charset_preset, "retro");
}

/// color-bg live reload (was startup-only — depth-test bug fix).
#[test]
fn rebuild_applies_color_bg_live_reload() {
    let base = minimal_cloud_config();
    assert!(base.default_bg);
    let mut cfg = HashMap::new();
    cfg.insert("color-bg".to_string(), "black".to_string());
    assert!(
        !rebuild_cloud_config(&base, &cfg).default_bg,
        "black → solid black"
    );
    let mut cfg2 = HashMap::new();
    cfg2.insert("color-bg".to_string(), "default-background".to_string());
    assert!(
        rebuild_cloud_config(&base, &cfg2).default_bg,
        "default-background → terminal default"
    );
}

/// unrecognized color-bg keeps old setting.
#[test]
fn rebuild_color_bg_unrecognized_keeps_old() {
    let base = minimal_cloud_config();
    let mut cfg = HashMap::new();
    cfg.insert("color-bg".to_string(), "purple".to_string());
    assert_eq!(
        rebuild_cloud_config(&base, &cfg).default_bg,
        base.default_bg
    );
}

#[test]
fn rebuild_applies_scene_custom_speed_and_density_changes() {
    let mut cfg = HashMap::new();
    cfg.insert(
        "scene-custom.test-scene.speed".to_string(),
        "24".to_string(),
    );
    cfg.insert(
        "scene-custom.test-scene.density".to_string(),
        "0.50".to_string(),
    );
    let base = minimal_cloud_config();
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(new.speed, 24.0);
    assert!((new.density - 0.50).abs() < f32::EPSILON);
    assert!((new.base_density - 0.50).abs() < f32::EPSILON);
}

#[test]
fn rebuild_applies_scene_custom_density_map_change() {
    let mut cfg = HashMap::new();
    cfg.insert(
        "scene-custom.test-scene.density-map".to_string(),
        "1.0,0.5,0.0,0.8".to_string(),
    );
    let base = minimal_cloud_config();
    let new = rebuild_cloud_config(&base, &cfg);
    let map = new
        .monolith_density_map
        .expect("density-map must be parsed and applied");
    assert_eq!(map.len(), 4);
    assert_eq!(map[0], 1.0);
    assert_eq!(map[2], 0.0);
}

#[test]
fn rebuild_without_scene_custom_name_does_not_apply_custom_fields() {
    // When scene_custom_name is None (no --scene-custom active), the
    // scene-custom.* keys in config must NOT be applied — they belong
    // to a different scene and could clobber the active one.
    let mut cfg = HashMap::new();
    cfg.insert(
        "scene-custom.other-scene.color".to_string(),
        "green".to_string(),
    );
    let mut base = minimal_cloud_config();
    base.scene_custom_name = None;
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.color_scheme,
        crate::runtime::ColorScheme::NeonPurple,
        "scene-custom fields must not apply when no custom scene is active"
    );
}

/// v50 fix: when the user edits `scene = "monolith"` in config.toml,
/// rebuild_cloud_config must update new.scene_name to match. Before the
/// fix, only rain_style/color/charset/speed/density were applied —
/// scene_name stayed at base.scene_name (the previous scene), so the
/// HUD 'scn:' line showed the old scene name even though the rain had
/// already switched. This is the source-of-truth fix; the event_loop.rs
/// else branch (commit 51ccafe) is the consumer-side mirror that
/// compares new_cfg.scene_name against preserved_scene_name to decide
/// whether to re-apply scene runtime defaults.
#[test]
fn rebuild_updates_scene_name_when_config_scene_changes() {
    let mut cfg = HashMap::new();
    cfg.insert("scene".to_string(), "monolith".to_string());
    // Base config has scene_name = "test-scene" (from minimal_cloud_config).
    let base = minimal_cloud_config();
    assert_eq!(
        base.scene_name, "test-scene",
        "baseline must start at test-scene"
    );
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.scene_name, "monolith",
        "scene_name must reflect config's scene value after live reload"
    );
    // Rain style must also switch (proves the scene block executed).
    assert_eq!(
        new.rain_style,
        crate::scene::get_scene("monolith")
            .unwrap()
            .config
            .rain_style,
        "rain_style must also switch to monolith's"
    );
}

/// v50 fix: case sensitivity — config scene value casing is preserved
/// for display, matching startup behavior in main.rs. The HUD shows
/// exactly what the user typed in config.toml.
#[test]
fn rebuild_preserves_scene_name_casing_from_config() {
    let mut cfg = HashMap::new();
    cfg.insert("scene".to_string(), "Monolith".to_string());
    let base = minimal_cloud_config();
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.scene_name, "Monolith",
        "scene_name casing must be preserved as written in config (display fidelity)"
    );
}

/// v50 fix: when CLI --scene was explicit, config's scene key must NOT
/// override scene_name (CLI > config.toml priority contract).
#[test]
fn rebuild_preserves_cli_explicit_scene_name_over_config() {
    let mut cfg = HashMap::new();
    cfg.insert("scene".to_string(), "monolith".to_string());
    let mut base = minimal_cloud_config();
    base.cli_explicit.scene = true;
    base.scene_name = "matrix".to_string();
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.scene_name, "matrix",
        "CLI --scene must NOT be overridden by config.toml scene key"
    );
}

/// Bug 3 test: CLI-explicit color must NOT be overridden by config.toml
/// during live reload. The priority contract is CLI > config.toml > scene.
/// Without the `cli_explicit` tracker, `rebuild_cloud_config` would
/// blindly apply `color = "snow"` from config, clobbering the user's
/// `-c green` CLI flag.
#[test]
fn rebuild_preserves_cli_explicit_color_over_config() {
    let mut cfg = HashMap::new();
    cfg.insert("color".to_string(), "snow".to_string());
    let mut base = minimal_cloud_config();
    // Simulate the user running `cosmostrix -c green`: the CLI flag
    // is recorded as explicit, and the color_scheme is set to Green.
    base.cli_explicit.color = true;
    base.color_scheme = crate::runtime::ColorScheme::Green;
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.color_scheme,
        crate::runtime::ColorScheme::Green,
        "CLI --color green must NOT be overridden by config.toml color=snow"
    );
}

/// Bug 3 test: config.toml overrides scene defaults when CLI did NOT
/// explicitly set the field. This is the normal live-reload path.
#[test]
fn rebuild_applies_config_color_when_cli_not_explicit() {
    let mut cfg = HashMap::new();
    cfg.insert("color".to_string(), "snow".to_string());
    let base = minimal_cloud_config();
    // base.cli_explicit.color is false (default) — no CLI override.
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(new.color_scheme, crate::runtime::ColorScheme::Snow);
}

/// Bug 3 test: CLI-explicit speed must NOT be overridden by scene's
/// speed default during live reload (CLI wins).
#[test]
fn rebuild_preserves_cli_explicit_speed_over_scene() {
    let mut cfg = HashMap::new();
    cfg.insert("scene".to_string(), "matrix".to_string());
    let mut base = minimal_cloud_config();
    base.cli_explicit.speed = true;
    base.cli_explicit.scene = false;
    base.speed = 25.0;
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(new.speed, 25.0, "CLI --speed wins over scene default");
}
