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
fn minimal_cloud_config() -> crate::app::CloudConfig {
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
        monolith_density_map: None,
        config_path_for_watcher: None,
        scene_name: "test-scene".to_string(),
        scene_custom_name: Some("test-scene".to_string()),
        cli_explicit: crate::app::CliExplicit::default(),
        ambient_schedule: crate::crystal_dragon_engine::ambient::AmbientSchedule::default(),
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

/// (bug #16): Serialize every test that touches the global
/// `LIVE_RELOAD_VALIDATION_REJECTIONS` log (directly or indirectly via
/// `validate_and_send`). Without this lock, cargo test's default
/// thread-pool runs these tests in parallel and one test drains another
/// test's expected rejection — `assert_eq!(rejections.len(), 1)` then
/// sees 0 or 2+ and fails spuriously.
static TEST_REJECTION_LOCK: Mutex<()> = Mutex::new(());

/// FIX D: validate_and_send returns Err on bad config, but the
/// render thread NO LONGER sets LIVE_RELOAD_EXIT_CODE — only true
/// watcher-thread panics do. FIX E: error includes a hint.
#[test]
fn validate_and_send_returns_err_without_setting_exit_code() {
    let _guard = TEST_REJECTION_LOCK.lock().unwrap();
    let _ = drain_validation_rejections();
    let (tx, _rx) = std::sync::mpsc::sync_channel(64);
    let mut parsed = configfile::ParsedConfig::default();
    parsed.unknown_keys.push("color.tune.bold".to_string());
    let result = validate_and_send(&parsed, &tx);
    assert!(result.is_err());
    let msg = result.unwrap_err();
    assert!(msg.contains("color.tune.bold"));
    assert!(msg.contains("top-level"), "need structural hint: {msg}");
    assert!(msg.contains("[color.tune]"), "need section ref: {msg}");
    assert_eq!(LIVE_RELOAD_EXIT_CODE.load(Ordering::Acquire), 0);
}

/// (bug #9): color.tune.* changes must propagate via live reload.
/// Before the fix, `rebuild_cloud_config` never touched `color_tune`,
/// so editing `brightness = 0.0` while running had zero effect until
/// restart. Verify brightness/saturation/head/body/tail all flow through.
#[test]
fn rebuild_applies_color_tune_live_reload_brightness() {
    let base = minimal_cloud_config();
    assert_eq!(
        base.color_tune.brightness, 1.0,
        "base config should start at identity brightness"
    );
    let mut cfg = HashMap::new();
    cfg.insert("color.tune.brightness".to_string(), "0.5".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert!(
        (new.color_tune.brightness - 0.5).abs() < 1e-6,
        "brightness should propagate to live-reloaded config (got {})",
        new.color_tune.brightness
    );
}

/// (bug #9): all 5 color.tune.* fields propagate, not just brightness.
#[test]
fn rebuild_applies_color_tune_live_reload_all_fields() {
    let base = minimal_cloud_config();
    let mut cfg = HashMap::new();
    cfg.insert("color.tune.brightness".to_string(), "1.5".to_string());
    cfg.insert("color.tune.saturation".to_string(), "0.7".to_string());
    cfg.insert("color.tune.head".to_string(), "2.0".to_string());
    cfg.insert("color.tune.body".to_string(), "1.2".to_string());
    cfg.insert("color.tune.tail".to_string(), "0.8".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert!((new.color_tune.brightness - 1.5).abs() < 1e-6);
    assert!((new.color_tune.saturation - 0.7).abs() < 1e-6);
    assert!((new.color_tune.head - 2.0).abs() < 1e-6);
    assert!((new.color_tune.body - 1.2).abs() < 1e-6);
    assert!((new.color_tune.tail - 0.8).abs() < 1e-6);
}

/// (bug #9): when no color.tune.* keys are in config, the tune
/// stays at the base value (identity by default). This protects users
/// who never set [color.tune] from accidentally dimming their rain.
#[test]
fn rebuild_without_color_tune_keys_keeps_base_tune() {
    let mut base = minimal_cloud_config();
    // Pretend the user set brightness = 2.0 at startup (CLI --color-tune).
    base.color_tune.brightness = 2.0;
    let cfg = HashMap::new(); // no color.tune.* keys
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.color_tune.brightness, 2.0,
        "no color.tune.* in config → keep base tune (CLI --color-tune wins)"
    );
}

/// (bug #14): `validate_and_send` must push every rejection to
/// the session log so the post-exit verbose summary can surface silent
/// rejections. Before the fix, an OOR value like `color.tune.tail = 5.0`
/// got silently rejected by `validate_config_strictly` — the watcher
/// kept watching, the rain kept running on the last valid config, and
/// the user had no idea their edit was rejected.
#[test]
fn validate_and_send_pushes_oor_rejection_to_session_log() {
    // (bug #16): hold the serialization lock so parallel tests
    // cannot drain our rejection mid-test.
    let _guard = TEST_REJECTION_LOCK.lock().unwrap();
    // Drain any prior rejections from earlier tests in this process.
    let _ = drain_validation_rejections();

    let (tx, _rx) = std::sync::mpsc::sync_channel(64);
    let mut parsed = configfile::ParsedConfig::default();
    parsed
        .values
        .insert("color.tune.tail".to_string(), "5.0".to_string());
    let result = validate_and_send(&parsed, &tx);
    assert!(result.is_err(), "OOR color.tune.tail must be rejected");

    let rejections = drain_validation_rejections();
    assert_eq!(
        rejections.len(),
        1,
        "exactly one rejection should be in the session log"
    );
    let entry = &rejections[0];
    assert!(
        entry.contains("color.tune.tail"),
        "rejection must name the bad field: {entry}"
    );
    assert!(
        entry.contains("out of range"),
        "rejection must mention range: {entry}"
    );

    // Drain must empty the log — next call returns empty Vec.
    let again = drain_validation_rejections();
    assert!(again.is_empty(), "drain must empty the log");
}

/// (bug #14): malformed lines and unknown keys must ALSO push to
/// the session log, not just strict value validation failures. All three
/// rejection paths in `validate_and_send` must be visible under `-v`.
#[test]
fn validate_and_send_pushes_unknown_key_to_session_log() {
    let _guard = TEST_REJECTION_LOCK.lock().unwrap();
    let _ = drain_validation_rejections();

    let (tx, _rx) = std::sync::mpsc::sync_channel(64);
    let mut parsed = configfile::ParsedConfig::default();
    parsed.unknown_keys.push("collor".to_string());
    let result = validate_and_send(&parsed, &tx);
    assert!(result.is_err());

    let rejections = drain_validation_rejections();
    assert_eq!(rejections.len(), 1);
    assert!(
        rejections[0].contains("collor"),
        "unknown-key rejection must be in session log: {}",
        rejections[0]
    );
}

/// (bug #14): cap at MAX_REJECTION_LOG (64) to avoid unbounded
/// growth on a misbehaving editor that saves 1000 times per second.
#[test]
fn rejection_log_caps_at_max() {
    let _guard = TEST_REJECTION_LOCK.lock().unwrap();
    let _ = drain_validation_rejections();

    for _ in 0..100 {
        push_validation_rejection("test rejection");
    }
    let rejections = drain_validation_rejections();
    assert_eq!(
        rejections.len(),
        MAX_REJECTION_LOG,
        "log must cap at MAX_REJECTION_LOG (64), got {}",
        rejections.len()
    );

    // Drain must reset — fresh log after drain.
    let again = drain_validation_rejections();
    assert!(again.is_empty());
}

/// (bug #14): valid config does NOT push to the session log.
/// Only rejections are logged; valid reloads are silent (the rebuild
/// trace already covers the success path).
#[test]
fn validate_and_send_does_not_log_valid_config() {
    let _guard = TEST_REJECTION_LOCK.lock().unwrap();
    let _ = drain_validation_rejections();

    let (tx, _rx) = std::sync::mpsc::sync_channel(64);
    let mut parsed = configfile::ParsedConfig::default();
    parsed
        .values
        .insert("color.tune.brightness".to_string(), "1.5".to_string());
    let result = validate_and_send(&parsed, &tx);
    assert!(result.is_ok(), "1.5 is in range [0.0, 3.0]");

    let rejections = drain_validation_rejections();
    assert!(
        rejections.is_empty(),
        "valid config must not push to rejection log, got: {rejections:?}"
    );
}

// ── v50.0.0-alpha.7: live-reload message / message-border / msg-mode ──

#[test]
fn live_reload_message_border_from_config() {
    // Config `message-border = "hello"` → new.message = "hello", border=true.
    let base = minimal_cloud_config();
    let mut cfg = HashMap::new();
    cfg.insert("message-border".to_string(), "hello".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(new.message.as_deref(), Some("hello"));
    assert!(
        new.message_border,
        "message-border config must set border=true"
    );
}

#[test]
fn live_reload_message_bare_from_config() {
    // Config `message = "hello"` (no border) → new.message = "hello", border=false.
    let base = minimal_cloud_config();
    let mut cfg = HashMap::new();
    cfg.insert("message".to_string(), "hello".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(new.message.as_deref(), Some("hello"));
    assert!(!new.message_border, "message config must keep border=false");
}

#[test]
fn live_reload_message_border_wins_over_message() {
    // Both keys present → message-border wins (border=true).
    let base = minimal_cloud_config();
    let mut cfg = HashMap::new();
    cfg.insert("message".to_string(), "plain".to_string());
    cfg.insert("message-border".to_string(), "boxed".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(new.message.as_deref(), Some("boxed"));
    assert!(new.message_border);
}

#[test]
fn live_reload_msg_mode_false_suppresses_config_message() {
    // msg-mode=false + config message-border → message suppressed.
    let base = minimal_cloud_config();
    let mut cfg = HashMap::new();
    cfg.insert("msg-mode".to_string(), "false".to_string());
    cfg.insert("message-border".to_string(), "hello".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.message, None,
        "msg-mode=false must suppress config message"
    );
    assert!(!new.message_border);
    assert!(!new.msg_mode, "msg_mode field must reflect false");
}

#[test]
fn live_reload_msg_mode_true_keeps_config_message() {
    // msg-mode=true + config message → preserved.
    let base = minimal_cloud_config();
    let mut cfg = HashMap::new();
    cfg.insert("msg-mode".to_string(), "true".to_string());
    cfg.insert("message-border".to_string(), "hello".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(new.message.as_deref(), Some("hello"));
    assert!(new.message_border);
    assert!(new.msg_mode);
}

#[test]
fn live_reload_msg_mode_defaults_true_when_unset() {
    // No msg-mode in config → default true.
    let base = minimal_cloud_config();
    let cfg = HashMap::new();
    let new = rebuild_cloud_config(&base, &cfg);
    assert!(new.msg_mode, "msg_mode must default to true when unset");
}

#[test]
fn live_reload_cli_message_wins_over_config() {
    // CLI -m explicit (cli.message=true) → config message ignored.
    let mut base = minimal_cloud_config();
    base.message = Some("from-cli".to_string());
    base.message_border = false;
    base.cli_explicit.message = true;
    let mut cfg = HashMap::new();
    cfg.insert("message-border".to_string(), "from-config".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(new.message.as_deref(), Some("from-cli"));
    assert!(!new.message_border, "CLI -m must keep border=false");
}

#[test]
fn live_reload_cli_msg_mode_wins_over_config() {
    // CLI --msg-mode false explicit → config msg-mode=true ignored.
    let mut base = minimal_cloud_config();
    base.msg_mode = false;
    base.cli_explicit.msg_mode = true;
    let mut cfg = HashMap::new();
    cfg.insert("msg-mode".to_string(), "true".to_string());
    cfg.insert("message-border".to_string(), "hello".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    // CLI msg-mode=false wins → message suppressed even though config has msg-mode=true + message.
    assert!(!new.msg_mode, "CLI msg-mode=false must win");
    assert_eq!(
        new.message, None,
        "msg-mode=false must suppress config message"
    );
}

#[test]
fn live_reload_power_dragon_respects_cli_explicit() {
    // CLI --power-dragon false explicit → config power-dragon=true ignored.
    let mut base = minimal_cloud_config();
    base.power_dragon = false;
    base.cli_explicit.power_dragon = true;
    let mut cfg = HashMap::new();
    cfg.insert("power-dragon".to_string(), "true".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert!(
        !new.power_dragon,
        "CLI --power-dragon false must win over config"
    );
}

#[test]
fn live_reload_async_mode_respects_cli_explicit() {
    // CLI --async-mode false explicit → config async-mode=true ignored.
    let mut base = minimal_cloud_config();
    base.async_mode = false;
    base.cli_explicit.async_mode = true;
    let mut cfg = HashMap::new();
    cfg.insert("async-mode".to_string(), "true".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert!(
        !new.async_mode,
        "CLI --async-mode false must win over config"
    );
}

#[test]
fn live_reload_intro_color_from_config() {
    // Config intro-color = "energy-zen" (valid) → new.intro_color set.
    let base = minimal_cloud_config();
    let mut cfg = HashMap::new();
    cfg.insert("intro-color".to_string(), "energy-zen".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(new.intro_color.as_deref(), Some("energy-zen"));
}

#[test]
fn live_reload_intro_color_invalid_soft_fails() {
    // Config intro-color = "not-a-color" (invalid) → soft-fail: clear field.
    // Unlike startup (hard error + exit), live-reload soft-fails to avoid
    // crashing a running session. User can fix config and save again.
    let base = minimal_cloud_config();
    let mut cfg = HashMap::new();
    cfg.insert("intro-color".to_string(), "not-a-color".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.intro_color, None,
        "invalid intro-color must soft-fail (clear)"
    );
}

#[test]
fn live_reload_intro_color_cli_explicit_wins() {
    // CLI --intro-color explicit → config intro-color ignored.
    let mut base = minimal_cloud_config();
    base.intro_color = Some("green".to_string());
    base.cli_explicit.intro_color = true;
    let mut cfg = HashMap::new();
    cfg.insert("intro-color".to_string(), "energy-zen".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(new.intro_color.as_deref(), Some("green"), "CLI must win");
}

#[test]
fn live_reload_monolith_size_respects_cli_explicit() {
    // CLI --monolith-size large explicit → config monolith-size=small ignored.
    // v50.0.0-alpha.7: Issue #4 fix — was config-only path, no CLI guard.
    use crate::runtime::MonolithSize;
    let mut base = minimal_cloud_config();
    base.monolith_size = MonolithSize::Large;
    base.cli_explicit.monolith_size = true;
    let mut cfg = HashMap::new();
    cfg.insert("monolith-size".to_string(), "small".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.monolith_size,
        MonolithSize::Large,
        "CLI --monolith-size large must win over config"
    );
}

#[test]
fn live_reload_monolith_size_from_config_when_no_cli() {
    // No CLI flag → config monolith-size=small applied.
    use crate::runtime::MonolithSize;
    let base = minimal_cloud_config();
    let mut cfg = HashMap::new();
    cfg.insert("monolith-size".to_string(), "small".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(new.monolith_size, MonolithSize::Small);
}
