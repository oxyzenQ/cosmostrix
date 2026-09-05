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
/// v80.0.0-beta.2 (S-master-HUNT, owner bug 3): strict validation no longer
/// skips `[scene-custom.*]` block keys — field VALUES are validated (same
/// rules as `--testconf`). Retired/unknown block fields are rejected.
fn validate_rejects_removed_block_field() {
    let mut cfg = HashMap::new();
    cfg.insert(
        "scene-custom.test.base-scene".to_string(),
        "monolith".to_string(),
    );
    cfg.insert("speed".to_string(), "30".to_string());
    let result = crate::testconf::validate_config_strictly(&cfg);
    assert!(result.is_err());
    let msg = result.unwrap_err();
    assert!(
        msg.contains("base-scene"),
        "error must name the removed field, got: {msg}"
    );
}

#[test]
fn validate_rejects_invalid_scene_custom_block_values() {
    // S-master-HUNT (owner bug 3): block field VALUES are validated by
    // strict validation (startup + live-reload + testconf in lockstep) —
    // `colors-custom = "cosmos"` (a BUILTIN, not a block) used to pass
    // silently and no-op at runtime. Range rules match the top-level keys.
    let mut cfg = HashMap::new();
    complete_cp77_block(&mut cfg);
    cfg.insert(
        "scene-custom.cp77.colors-custom".to_string(),
        "cosmos".to_string(), // a BUILTIN color, not a [colors-custom.*] block
    );
    let msg = crate::testconf::validate_config_strictly(&cfg)
        .expect_err("missing colors-custom reference must be rejected");
    assert!(
        msg.contains("unknown colors-custom block 'cosmos'"),
        "{msg}"
    );
    assert!(
        msg.contains("BUILT-IN color name"),
        "hint for built-ins: {msg}"
    );
    // Out-of-range block fps: same range rules as the top-level key.
    let mut cfg2 = HashMap::new();
    complete_cp77_block(&mut cfg2);
    cfg2.insert("scene-custom.cp77.fps".to_string(), "999".to_string());
    let msg2 = crate::testconf::validate_config_strictly(&cfg2)
        .expect_err("out-of-range block fps must be rejected");
    assert!(
        msg2.contains("out of range") && msg2.contains("fps"),
        "{msg2}"
    );
}

/// A complete, valid v2 `[scene-custom.cp77]` block + its referenced
/// custom blocks — shared by the block-value validation tests.
/// NIGHT-research-5: includes the `rain` field (now required for
/// completeness — see SCENE_CUSTOM_REQUIRED_FIELDS).
fn complete_cp77_block(cfg: &mut HashMap<String, String>) {
    for (k, v) in [
        ("colors-custom.p1.bg", "#0a0a0a"),
        ("colors-custom.p1.rain", "#00ff41,#00b32d"),
        ("scene-custom.cp77.rain", "vortex"),
        ("scene-custom.cp77.colors-custom", "p1"),
        ("scene-custom.cp77.charset-custom", "cyberpunk_2077"),
        ("charset-custom.cyberpunk_2077.set", "01AB"),
        ("scene-custom.cp77.fps", "90"),
        ("scene-custom.cp77.speed", "12"),
        ("scene-custom.cp77.density", "0.9"),
        ("scene-custom.cp77.glitch-level", "none"),
    ] {
        cfg.insert(k.to_string(), v.to_string());
    }
}

#[test]
fn validate_rejects_invalid_charset() {
    let mut cfg = HashMap::new();
    cfg.insert("charset".to_string(), "hackeres".to_string());
    assert!(crate::testconf::validate_config_strictly(&cfg).is_err());
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
        // v80.0.0-beta.2 msg-fill-style: pinned to Typewriter explicitly so these
        // style-agnostic tests never depend on the champion default (engrave
        // since v80.0.0-beta.2 — the default contract is locked separately in
        // tests_msg_fill_style.rs and clap_suggestion.rs).
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
        config_path_for_watcher: None,
        scene_name: "test-scene".to_string(),
        scene_custom_name: Some("test-scene".to_string()),
        // v80.0.0-beta.2 (S-master-HUNT): lock default — see CloudConfig doc.
        scene_custom_config_owned: false,
        cli_explicit: crate::app::CliExplicit::default(),
        ambient_schedule: crate::crystal_dragon_engine::ambient::AmbientSchedule::default(),
        ambient_snapback_secs: None,
        crystal_dragon_secs: None,
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

/// v50 fix: editing `scene = "monolith"` in config.toml must update
/// new.scene_name to match — before the fix scene_name stayed at the
/// previous scene, so the HUD 'scn:' line showed the old scene. This is
/// the source-of-truth fix; the event_loop.rs else branch is the
/// consumer-side mirror.
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

/// v80.0.0-beta.1 (owner contract, 2026-09-01): a runtime config `scene` key
/// WINS over the CLI-locked scene — temporal precedence, the file edit
/// is the most recent user intent. The CLI value survives as the
/// FALLBACK: the end-to-end comment-out scenario is pinned in
/// tests_cli_fallback.rs (scene reverts to the locked startup values).
#[test]
fn rebuild_config_scene_key_overrides_cli_locked_scene() {
    let mut cfg = HashMap::new();
    cfg.insert("scene".to_string(), "monolith".to_string());
    let mut base = minimal_cloud_config();
    base.cli_explicit.scene = true;
    base.scene_name = "matrix".to_string();
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.scene_name, "monolith",
        "runtime config scene key must override the CLI-locked scene (v80.0.0-beta.1)"
    );
}

/// v80.0.0-beta.1 (owner contract): a runtime config `color` key WINS over a
/// CLI `-c green` lock (temporal precedence). The CLI color returns
/// when the key is commented back out — the CLI-locked fallback is
/// pinned in tests_cli_fallback.rs.
#[test]
fn rebuild_config_color_key_overrides_cli_locked_color() {
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
        crate::runtime::ColorScheme::Snow,
        "runtime config color key must override the CLI-locked color (v80.0.0-beta.1)"
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

// ── v80.0.0-beta.1 live-reload custom palette / custom scene switching (owner audit 2026-08-30) ──
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

/// v80.0.0-beta.2 (S-master-LOGIC-3): custom scenes are always Glyph rain —
/// base-scene inheritance is removed from the schema. A config that still
/// carries a base-scene key gets it rejected upstream (unknown key), and
/// the rebuild resolves the custom scene to Glyph regardless.
#[test]
fn rebuild_custom_scene_is_always_glyph_rain() {
    let mut cfg = HashMap::new();
    cfg.insert("scene".to_string(), "pillars".to_string());
    cfg.insert(
        "scene-custom.pillars.color".to_string(),
        "cosmos".to_string(),
    );
    let mut base = minimal_cloud_config();
    base.rain_style = crate::rain_style::RainStyle::Monolith;
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.rain_style,
        crate::rain_style::RainStyle::Glyph,
        "custom scenes must resolve to Glyph rain (base-scene removed in v80.0.0-beta.2)"
    );
}

/// A custom scene resolves to Glyph rain even when the previous state
/// was Monolith (startup construction parity).
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

/// v80.0.0-beta.1 startup-parity: switching scenes via config must apply the scene's
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

/// v80.0.0-beta.1 startup-parity: an explicit `fps` key still wins over the scene's
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

/// v80.0.0-beta.1 startup-parity: switching scenes via config must apply the scene's
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

/// v80.0.0-beta.1 startup-parity: an explicit `glitch-level` key still wins over the
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
pub(super) fn cfg2base() -> crate::app::CloudConfig {
    let mut base = minimal_cloud_config();
    base.glitch_level = crate::config::GlitchLevel::None;
    base.glitch_enabled = false;
    base
}

// ── v80.0.0-beta.2 custom-name live-reload parity (owner fatal bug) ──
// The live-reload watcher validates through the same
// validate_config_strictly gate (now accepts custom scene/palette
// references); these tests lock the REBUILD side of the owner's
// scenario: a config edit switching `scene` to a custom scene name
// must apply the custom scene through the scene-custom tail layer.

#[test]
fn rebuild_applies_scene_key_switching_to_custom_scene() {
    // Owner scenario: config edit `scene = hacker-mode` (previously
    // rejected by the watcher's validation, blocking the edit with
    // "Config NOT applied"). After the validator parity fix, the
    // rebuild must apply the custom scene's base + field layers.
    let mut cfg = HashMap::new();
    cfg.insert("scene".to_string(), "hacker-mode".to_string());
    cfg.insert(
        "scene-custom.hacker-mode.base-scene".to_string(),
        "matrix".to_string(),
    );
    cfg.insert(
        "scene-custom.hacker-mode.speed".to_string(),
        "20".to_string(),
    );
    let base = minimal_cloud_config();
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.scene_name, "hacker-mode",
        "scene = <custom> must switch the scene on live-reload rebuild"
    );
    assert_eq!(
        new.scene_custom_name.as_deref(),
        Some("hacker-mode"),
        "the scene-custom tail layer must be armed for the custom scene"
    );
    // matrix base-scene speed 18 vs block override 20 — the block wins.
    assert!((new.speed - 20.0).abs() < 0.01);
}

#[test]
fn rebuild_custom_scene_colors_custom_sets_palette_name() {
    // The rebuild path must surface the palette NAME (CloudConfig.
    // custom_palette_name) for the HUD clr: line — same contract as
    // the startup path.
    let mut cfg = HashMap::new();
    cfg.insert("scene".to_string(), "cp77".to_string());
    cfg.insert(
        "scene-custom.cp77.base-scene".to_string(),
        "storm".to_string(),
    );
    cfg.insert(
        "scene-custom.cp77.colors-custom".to_string(),
        "cyberpunk_2077".to_string(),
    );
    cfg.insert(
        "colors-custom.cyberpunk_2077.bg".to_string(),
        "#0a0a12".to_string(),
    );
    cfg.insert(
        "colors-custom.cyberpunk_2077.rain".to_string(),
        "#00fff7,#ff003c".to_string(),
    );
    let new = rebuild_cloud_config(&minimal_cloud_config(), &cfg);
    assert!(new.custom_palette.is_some());
    assert_eq!(
        new.custom_palette_name.as_deref(),
        Some("cyberpunk_2077"),
        "the rebuilt CloudConfig must carry the palette name for the clr: HUD line"
    );
}
