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
        // v51 msg-fill-style: default keeps the classic typewriter reveal.
        msg_fill_style: crate::msg_fill_style::MsgFillStyle::Typewriter,
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
        intro: crate::intro_style::IntroType::None,
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

// ── v51 msg-fill-style live-reload tests ──

/// The engrave style (v51 follow-up) live-reloads exactly like the
/// other styles — the spark sidecar arms on the next style change via
/// `set_msg_fill_style`.
#[test]
fn rebuild_applies_msg_fill_style_engrave() {
    let mut cfg = HashMap::new();
    cfg.insert("msg-fill-style".to_string(), "engrave".to_string());
    let new = rebuild_cloud_config(&minimal_cloud_config(), &cfg);
    assert_eq!(
        new.msg_fill_style,
        crate::msg_fill_style::MsgFillStyle::Engrave,
        "config msg-fill-style=engrave must be applied on live reload"
    );
}

/// The hologram style (post-engrave follow-up) live-reloads exactly
/// like the other styles. Hologram is fully stateless (no sidecar
/// to arm), so the test only needs to assert the enum variant — the
/// scanline pass self-gates on the next draw_message frame.
#[test]
fn rebuild_applies_msg_fill_style_hologram() {
    let mut cfg = HashMap::new();
    cfg.insert("msg-fill-style".to_string(), "hologram".to_string());
    let new = rebuild_cloud_config(&minimal_cloud_config(), &cfg);
    assert_eq!(
        new.msg_fill_style,
        crate::msg_fill_style::MsgFillStyle::Hologram,
        "config msg-fill-style=hologram must be applied on live reload"
    );
}

/// Editing `msg-fill-style` in config.toml mid-run must switch the
/// reveal style on the next rebuild (no restart needed).
#[test]
fn rebuild_applies_msg_fill_style_from_config() {
    let mut cfg = HashMap::new();
    cfg.insert("msg-fill-style".to_string(), "pulse".to_string());
    let base = minimal_cloud_config();
    assert_eq!(
        base.msg_fill_style,
        crate::msg_fill_style::MsgFillStyle::Typewriter,
        "baseline must start at typewriter"
    );
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.msg_fill_style,
        crate::msg_fill_style::MsgFillStyle::Pulse,
        "config msg-fill-style=pulse must be applied on live reload"
    );
}

/// The config surface is case-insensitive (mirrors every other enum
/// key: intro, glitch-level, monolith-size).
#[test]
fn rebuild_msg_fill_style_config_is_case_insensitive() {
    let mut cfg = HashMap::new();
    cfg.insert("msg-fill-style".to_string(), "Fade".to_string());
    let new = rebuild_cloud_config(&minimal_cloud_config(), &cfg);
    assert_eq!(
        new.msg_fill_style,
        crate::msg_fill_style::MsgFillStyle::Fade
    );
}

/// An invalid style value soft-fails: logged, style unchanged (same
/// policy as intro-color live reload — don't crash a running session).
#[test]
fn rebuild_msg_fill_style_invalid_soft_fails() {
    let mut cfg = HashMap::new();
    cfg.insert("msg-fill-style".to_string(), "scanner".to_string());
    let new = rebuild_cloud_config(&minimal_cloud_config(), &cfg);
    assert_eq!(
        new.msg_fill_style,
        crate::msg_fill_style::MsgFillStyle::Typewriter,
        "invalid msg-fill-style must keep the previous style (soft-fail)"
    );
}

/// When the key is absent (commented out), the startup style is
/// preserved — enums have no reset-on-comment semantics.
#[test]
fn rebuild_msg_fill_style_absent_keeps_startup_value() {
    let mut base = minimal_cloud_config();
    base.msg_fill_style = crate::msg_fill_style::MsgFillStyle::Slide;
    let cfg = HashMap::new();
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.msg_fill_style,
        crate::msg_fill_style::MsgFillStyle::Slide,
        "absent msg-fill-style key must preserve the startup style"
    );
}

/// CLI -mfs/--msg-fill-style explicit wins over config on live reload
/// (priority contract: CLI > config.toml).
#[test]
fn rebuild_preserves_cli_explicit_msg_fill_style_over_config() {
    let mut cfg = HashMap::new();
    cfg.insert("msg-fill-style".to_string(), "fade".to_string());
    let mut base = minimal_cloud_config();
    // Simulate the user running `cosmostrix -mfs slide`: the CLI flag is
    // recorded as explicit, and the style is set to Slide.
    base.cli_explicit.msg_fill_style = true;
    base.msg_fill_style = crate::msg_fill_style::MsgFillStyle::Slide;
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.msg_fill_style,
        crate::msg_fill_style::MsgFillStyle::Slide,
        "CLI -mfs slide must NOT be overridden by config msg-fill-style=fade"
    );
}

// ── v51 live-reload custom palette / custom scene switching (owner audit 2026-08-30) ──
//
// Owner report: "some functions in config.toml don't work at live-reload."
// Four confirmed bugs, all in the custom-palette / custom-scene switching
// paths of rebuild_cloud_config:
//   A. `color = "<custom-palette-name>"` was a silent no-op at runtime
//      (the block only parsed BUILTIN scheme names, unlike startup's
//      custom-first lookup in main.rs).
//   B. Switching `color` away from an active custom palette left the stale
//      palette loaded — and create_cloud applies custom_palette AFTER the
//      scheme, so the builtin the user switched to never took effect.
//   C. Switching `scene` to a custom scene name updated scene_name but left
//      rain_style/color/charset/speed/density at the previous scene's values.
//   D. Switching `scene` away from a custom scene re-applied the stale
//      startup custom-scene layer on top of every builtin scene.

/// Bug A: switching `color` to a custom palette name at runtime must load
/// the palette (startup parity — custom wins over builtin on collision).
#[test]
fn rebuild_switches_color_to_custom_palette_at_runtime() {
    let mut cfg = HashMap::new();
    cfg.insert("color".to_string(), "mycustompal".to_string());
    cfg.insert(
        "colors-custom.mycustompal.rain".to_string(),
        "#1a0033, #4d0080, #9933ff".to_string(),
    );
    let base = minimal_cloud_config();
    let new = rebuild_cloud_config(&base, &cfg);
    assert!(
        new.custom_palette.is_some(),
        "custom palette must load when color names one at live-reload"
    );
    assert_eq!(new.custom_palette_name.as_deref(), Some("mycustompal"));
    // Palette stops must come from the config block (3 hex stops →
    // interpolated palette), not from any builtin scheme.
    assert!(!new.custom_palette.unwrap().colors.is_empty());
}

/// Bug B: switching `color` from an active custom palette to a builtin must
/// clear the palette — otherwise create_cloud's `set_palette` keeps
/// overriding the builtin scheme the user just switched to.
#[test]
fn rebuild_switches_color_away_from_custom_palette() {
    let mut base = minimal_cloud_config();
    base.custom_palette_name = Some("mycustompal".to_string());
    base.custom_palette = Some(crate::palette::build_palette(
        crate::runtime::ColorScheme::NeonPurple,
        crate::runtime::ColorMode::TrueColor,
        true,
    ));
    let mut cfg = HashMap::new();
    cfg.insert("color".to_string(), "green".to_string());
    // The custom block still exists in config (user only changed the color key).
    cfg.insert(
        "colors-custom.mycustompal.rain".to_string(),
        "#1a0033, #4d0080, #9933ff".to_string(),
    );
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(new.color_scheme, crate::runtime::ColorScheme::Green);
    assert!(
        new.custom_palette.is_none(),
        "switching to a builtin must clear the custom palette"
    );
    assert!(new.custom_palette_name.is_none());
}

/// Bug A negative: an unknown color name that is neither builtin nor custom
/// keeps the current state (soft-fail; upstream strict validation rejects
/// the whole config anyway).
#[test]
fn rebuild_unknown_color_name_keeps_current_palette() {
    let mut base = minimal_cloud_config();
    base.custom_palette_name = Some("mycustompal".to_string());
    base.custom_palette = Some(crate::palette::build_palette(
        crate::runtime::ColorScheme::NeonPurple,
        crate::runtime::ColorMode::TrueColor,
        true,
    ));
    let mut cfg = HashMap::new();
    cfg.insert("color".to_string(), "definitely-not-a-color".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert!(
        new.custom_palette.is_some(),
        "invalid color value must soft-fail and keep the current palette"
    );
    assert_eq!(new.color_scheme, crate::runtime::ColorScheme::NeonPurple);
}

/// Bug C: switching `scene` to a custom scene at runtime must apply the
/// custom scene's field layer (base-scene defaults + overrides), not just
/// the name.
#[test]
fn rebuild_switches_scene_to_custom_scene_at_runtime() {
    let mut cfg = HashMap::new();
    cfg.insert("scene".to_string(), "myscene".to_string());
    cfg.insert(
        "scene-custom.myscene.base-scene".to_string(),
        "cinematic".to_string(),
    );
    cfg.insert(
        "scene-custom.myscene.color".to_string(),
        "cosmos".to_string(),
    );
    cfg.insert("scene-custom.myscene.speed".to_string(), "3".to_string());
    let base = minimal_cloud_config();
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(new.scene_name, "myscene");
    assert_eq!(
        new.color_scheme,
        crate::runtime::ColorScheme::Cosmos,
        "custom scene color field must apply at live-reload"
    );
    assert_eq!(new.speed, 3.0, "custom scene speed field must apply");
    assert_eq!(
        new.scene_custom_name.as_deref(),
        Some("myscene"),
        "custom scene must be tracked as active so later field edits re-apply"
    );
}

/// Bug C (rain_style): a custom scene with a monolith base-scene must
/// resolve RainStyle::Monolith at live-reload (mirrors the startup
/// construction path via rain_style_for_custom_scene).
#[test]
fn rebuild_custom_scene_resolves_rain_style_from_base_scene() {
    let mut cfg = HashMap::new();
    cfg.insert("scene".to_string(), "pillars".to_string());
    cfg.insert(
        "scene-custom.pillars.base-scene".to_string(),
        "monolith".to_string(),
    );
    let base = minimal_cloud_config();
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.rain_style,
        crate::rain_style::RainStyle::Monolith,
        "custom scene base-scene rain_style must apply at live-reload"
    );
}

/// Bug C (no base-scene): a custom scene without base-scene defaults to
/// Glyph rain (same fallback as startup construction).
#[test]
fn rebuild_custom_scene_without_base_scene_defaults_to_glyph() {
    let mut cfg = HashMap::new();
    cfg.insert("scene".to_string(), "bare".to_string());
    cfg.insert("scene-custom.bare.color".to_string(), "cosmos".to_string());
    let mut base = minimal_cloud_config();
    base.rain_style = crate::rain_style::RainStyle::Monolith;
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(new.rain_style, crate::rain_style::RainStyle::Glyph);
}

/// Bug D: switching `scene` from an active custom scene to a builtin must
/// NOT re-apply the stale custom layer — the builtin's own defaults win.
#[test]
fn rebuild_switches_scene_away_from_custom_scene() {
    let mut cfg = HashMap::new();
    cfg.insert("scene".to_string(), "cinematic".to_string());
    // Stale custom block still in config (user did not delete it).
    cfg.insert(
        "scene-custom.test-scene.base-scene".to_string(),
        "monolith".to_string(),
    );
    cfg.insert(
        "scene-custom.test-scene.color".to_string(),
        "cosmos".to_string(),
    );
    cfg.insert("scene-custom.test-scene.speed".to_string(), "3".to_string());
    let base = minimal_cloud_config();
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(new.scene_name, "cinematic");
    assert_eq!(
        new.color_scheme,
        crate::runtime::ColorScheme::EnergyZen,
        "cinematic's default color must win — stale custom layer must not re-apply"
    );
    assert_eq!(
        new.speed, 9.0,
        "cinematic's default speed must win over the stale custom layer"
    );
    assert!(
        new.scene_custom_name.is_none(),
        "switching to a builtin must clear the custom scene tracker"
    );
}

/// Regression: editing the ACTIVE custom scene's fields at runtime must
/// still re-apply the layer (pre-existing v20 behavior preserved).
#[test]
fn rebuild_active_custom_scene_field_edit_still_reapplies() {
    let mut cfg = HashMap::new();
    cfg.insert("scene".to_string(), "test-scene".to_string());
    cfg.insert(
        "scene-custom.test-scene.color".to_string(),
        "cosmos".to_string(),
    );
    let base = minimal_cloud_config();
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(new.scene_name, "test-scene");
    assert_eq!(new.color_scheme, crate::runtime::ColorScheme::Cosmos);
    assert_eq!(new.scene_custom_name.as_deref(), Some("test-scene"));
}

/// Regression: switching `scene` to a custom scene whose colors-custom
/// field names a custom palette must load that palette (scene-level
/// palette activation at runtime).
#[test]
fn rebuild_custom_scene_colors_custom_field_loads_palette() {
    let mut cfg = HashMap::new();
    cfg.insert("scene".to_string(), "branded".to_string());
    cfg.insert(
        "scene-custom.branded.base-scene".to_string(),
        "cinematic".to_string(),
    );
    cfg.insert(
        "scene-custom.branded.colors-custom".to_string(),
        "mycustompal".to_string(),
    );
    cfg.insert(
        "colors-custom.mycustompal.rain".to_string(),
        "#1a0033, #4d0080, #9933ff".to_string(),
    );
    let base = minimal_cloud_config();
    let new = rebuild_cloud_config(&base, &cfg);
    assert!(
        new.custom_palette.is_some(),
        "scene-custom colors-custom field must activate the palette at live-reload"
    );
    assert_eq!(new.custom_palette_name.as_deref(), Some("mycustompal"));
}

/// v51 startup-parity: switching scenes via config must apply the scene's
/// fps default (startup's apply_default_scene_values does; the old
/// live-reload block never did).
#[test]
fn rebuild_scene_switch_applies_scene_fps_default() {
    let mut cfg = HashMap::new();
    // storm defines fps 120 (vs the 60 base default).
    cfg.insert("scene".to_string(), "storm".to_string());
    let mut base = minimal_cloud_config();
    base.target_fps = 60.0;
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.target_fps, 120.0,
        "scene fps default must apply on scene switch at live-reload"
    );
}

/// v51 startup-parity: an explicit `fps` key still wins over the scene's
/// fps default (layering: config > scene defaults).
#[test]
fn rebuild_user_fps_key_wins_over_scene_default() {
    let mut cfg = HashMap::new();
    cfg.insert("scene".to_string(), "storm".to_string());
    cfg.insert("fps".to_string(), "144".to_string());
    let mut base = minimal_cloud_config();
    base.target_fps = 60.0;
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.target_fps, 144.0,
        "user fps key must beat the scene default"
    );
}

/// v51 startup-parity: switching scenes via config must apply the scene's
/// glitch_level preset (cinematic ships Subtle).
#[test]
fn rebuild_scene_switch_applies_scene_glitch_default() {
    let mut cfg = HashMap::new();
    cfg.insert("scene".to_string(), "cinematic".to_string());
    let mut base = minimal_cloud_config();
    base.glitch_level = crate::config::GlitchLevel::None;
    base.glitch_enabled = false;
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.glitch_level,
        crate::config::GlitchLevel::Subtle,
        "scene glitch default must apply on scene switch at live-reload"
    );
    assert!(new.glitch_enabled);
}

/// v51 startup-parity: an explicit `glitch-level` key still wins over the
/// scene's glitch default.
#[test]
fn rebuild_user_glitch_key_wins_over_scene_default() {
    let mut cfg = HashMap::new();
    cfg.insert("scene".to_string(), "cinematic".to_string());
    cfg.insert("glitch-level".to_string(), "intense".to_string());
    let new = rebuild_cloud_config(&cfg2base(), &cfg);
    assert_eq!(new.glitch_level, crate::config::GlitchLevel::Intense);
}

/// Helper: minimal base with glitch fields neutralized for override tests.
fn cfg2base() -> crate::app::CloudConfig {
    let mut base = minimal_cloud_config();
    base.glitch_level = crate::config::GlitchLevel::None;
    base.glitch_enabled = false;
    base
}
