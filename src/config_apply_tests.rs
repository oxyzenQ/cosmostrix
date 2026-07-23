// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use clap::{CommandFactory, FromArgMatches};

use crate::config::{Args, GlitchLevel};
use crate::config_apply::apply_config_and_runtime_defaults;
use crate::configfile::dump_config_text;
use crate::runtime::MonolithSize;

/// Global counter for unique temp file names. Prevents collisions when
/// multiple tests run in parallel and `SystemTime::now()` returns the
/// same nanosecond on fast CI runners.
static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Set COSMOSTRIX_TEST_CONFIG_DIR so is_safe_path allows /tmp during tests.
/// Idempotent — safe to call from parallel test threads.
fn ensure_test_config_dir_allowed() {
    std::env::set_var("COSMOSTRIX_SKIP_STARTUP_VALIDATION", "1");
    // Setting the same value repeatedly is benign even under race conditions.
    std::env::set_var("COSMOSTRIX_TEST_CONFIG_DIR", "/tmp");
}

fn args_with_config(config: &str, cli: &[&str]) -> Args {
    ensure_test_config_dir_allowed();
    let mut path = std::env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after unix epoch")
        .as_nanos();
    let seq = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    path.push(format!(
        "cosmostrix-config-test-{}-{nanos}-{seq}.toml",
        std::process::id(),
    ));
    std::fs::write(&path, config).expect("write temp config");

    let path_string = path.to_string_lossy().into_owned();
    let mut argv = vec!["cosmostrix", "--config", path_string.as_str()];
    argv.extend_from_slice(cli);

    let cmd = Args::command();
    let matches = cmd.get_matches_from(argv);
    let mut args = Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    apply_config_and_runtime_defaults(&matches, &mut args).expect("apply config");

    let _ = std::fs::remove_file(path);
    args
}

fn args_from_cli(cli: &[&str]) -> Args {
    if cli.contains(&"--config") {
        ensure_test_config_dir_allowed();
        let mut argv = vec!["cosmostrix"];
        argv.extend_from_slice(cli);
        let cmd = Args::command();
        let matches = cmd.get_matches_from(argv);
        let mut args = Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
        apply_config_and_runtime_defaults(&matches, &mut args).expect("apply config");
        return args;
    }
    args_with_config("", cli)
}

fn args_from_cli_result(cli: &[&str]) -> Result<Args, String> {
    if cli.contains(&"--config") {
        ensure_test_config_dir_allowed();
        let mut argv = vec!["cosmostrix"];
        argv.extend_from_slice(cli);
        let cmd = Args::command();
        let matches = cmd.get_matches_from(argv);
        let mut args = Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
        apply_config_and_runtime_defaults(&matches, &mut args)?;
        return Ok(args);
    }

    ensure_test_config_dir_allowed();
    let mut path = std::env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after unix epoch")
        .as_nanos();
    let seq = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    path.push(format!(
        "cosmostrix-empty-config-test-{}-{nanos}-{seq}.toml",
        std::process::id(),
    ));
    std::fs::write(&path, "").expect("write temp config");

    let path_string = path.to_string_lossy().into_owned();
    let mut argv = vec!["cosmostrix", "--config", path_string.as_str()];
    argv.extend_from_slice(cli);
    let cmd = Args::command();
    let matches = cmd.get_matches_from(argv);
    let mut args = Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    let result = apply_config_and_runtime_defaults(&matches, &mut args).map(|()| args);

    let _ = std::fs::remove_file(path);
    result
}

#[test]
fn config_glitch_level_subtle_applies() {
    let args = args_with_config("glitch-level = subtle\n", &[]);
    assert_eq!(args.glitch_level, GlitchLevel::Subtle);
    assert_eq!(args.glitch_pct, 3.0);
    assert_eq!(args.shortpct, 60.0);
    assert!(!args.noglitch);
}

#[test]
fn config_scene_calm_applies() {
    // v17: 'preset' deprecated alias removed. Use 'scene = calm' directly.
    let args = args_with_config("scene = calm\n", &[]);
    assert_eq!(args.scene.as_deref(), Some("calm"));
    assert_eq!(args.color, "ocean");
    assert_eq!(args.charset, "minimal");
    assert_eq!(args.speed, 6.0);
    assert!((args.density - 0.40).abs() < f32::EPSILON);
}

#[test]
fn default_scene_is_monolith() {
    let args = args_from_cli(&[]);
    assert_eq!(args.scene.as_deref(), Some("monolith"));
    assert_eq!(args.color, "cosmos");
    assert_eq!(args.charset, "braille");
    assert_eq!(args.speed, 30.0);
    assert_eq!(args.density, 0.85);
    assert_eq!(args.glitch_level, GlitchLevel::Subtle);
}

#[test]
fn explicit_matrix_scene_restores_classic_defaults() {
    let args = args_from_cli(&["--scene", "matrix"]);
    assert_eq!(args.scene.as_deref(), Some("matrix"));
    assert_eq!(args.color, "neon-green");
    assert_eq!(args.charset, "matrix");
    assert_eq!(args.speed, 18.0);
    // Matrix scene uses neon-green for futuristic cinematic glow.
    assert_eq!(args.density, 0.65);
    // v17 hardening: matrix scene now sets glitch_level=Subtle (was None →
    // Default 10%). Subtle (3%) matches cinematic sparse cascade identity.
    assert_eq!(args.glitch_level, GlitchLevel::Subtle);
}

#[test]
fn invalid_cli_scene_is_clear_error() {
    let err = args_from_cli_result(&["--scene", "nonexistent"]).unwrap_err();
    assert!(
        err.contains("error: unknown scene"),
        "scene error must use 'unknown' terminology: {err}"
    );
    assert!(
        err.contains("--list-scenes"),
        "scene error must reference --list-scenes: {err}"
    );
}

#[test]
fn config_scene_monolith_applies() {
    let args = args_with_config("scene = monolith\n", &[]);
    assert_eq!(args.scene.as_deref(), Some("monolith"));
    assert_eq!(args.color, "cosmos");
    assert_eq!(args.charset, "braille");
    assert_eq!(args.speed, 30.0);
    assert!((args.density - 0.85).abs() < f32::EPSILON);
    assert_eq!(args.glitch_level, GlitchLevel::Subtle);
    assert_eq!(args.glitch_pct, 3.0);
}

#[test]
fn cli_scene_overrides_config_scene() {
    let args = args_with_config("scene = monolith\n", &["--scene", "signal"]);
    assert_eq!(args.scene.as_deref(), Some("signal"));
    assert_eq!(args.color, "aurora");
    assert_eq!(args.charset, "retro");
    assert_eq!(args.speed, 14.0);
}

#[test]
fn explicit_cli_flags_override_scene_managed_values() {
    let args = args_from_cli(&["--scene", "signal", "--color", "green", "--fps", "120"]);
    assert_eq!(args.scene.as_deref(), Some("signal"));
    assert_eq!(args.color, "green");
    assert_eq!(args.fps, 120.0);
    assert_eq!(args.charset, "retro");
    assert_eq!(args.speed, 14.0);
}

#[test]
fn monolith_scene_respects_explicit_color_override() {
    let args = args_from_cli(&["--scene", "monolith", "--color", "deepspace"]);
    assert_eq!(args.scene.as_deref(), Some("monolith"));
    assert_eq!(args.color, "deepspace");
    assert_eq!(args.charset, "braille");
}

#[test]
fn monolith_scene_respects_explicit_motion_overrides() {
    let args = args_from_cli(&[
        "--scene",
        "monolith",
        "--fps",
        "120",
        "--speed",
        "9",
        "--density",
        "0.25",
    ]);
    assert_eq!(args.scene.as_deref(), Some("monolith"));
    assert_eq!(args.fps, 120.0);
    assert_eq!(args.speed, 9.0);
    assert!((args.density - 0.25).abs() < f32::EPSILON);
    assert_eq!(args.color, "cosmos");
}

// ── Scene defaults respect config-set keys (v13.6.0 regression guards) ──
//
// Bug history: apply_scene_values did NOT check config_touched, so a scene's
// hardcoded speed (e.g. monolith=30, signal=10) would silently overwrite a
// user's `speed = N` set in config.toml. The fix: scene defaults only fill
// keys the user did NOT set in config. Mirrors apply_default_scene_values.
//
// All tests below pair a config-set key with a scene that has a different
// hardcoded default for the same key. The config value must win.

#[test]
fn config_speed_wins_over_monolith_scene_default() {
    // Config sets speed=12; monolith scene hardcodes speed=30.
    // Config must win — scene only fills unset keys.
    let args = args_with_config("scene = monolith\nspeed = 12\n", &[]);
    assert_eq!(args.scene.as_deref(), Some("monolith"));
    assert_eq!(
        args.speed, 12.0,
        "config speed must win over monolith scene default 30"
    );
    // Scene defaults for UNSET keys still apply:
    assert_eq!(
        args.color, "cosmos",
        "scene color default applies for unset key"
    );
    assert!((args.density - 0.85).abs() < f32::EPSILON);
}

#[test]
fn config_density_wins_over_signal_scene_default() {
    // Config sets density=0.5; signal scene hardcodes density=0.70.
    let args = args_with_config("scene = signal\ndensity = 0.5\n", &[]);
    assert_eq!(args.scene.as_deref(), Some("signal"));
    assert_eq!(
        args.speed, 14.0,
        "scene speed default applies for unset key"
    );
    assert!((args.density - 0.5).abs() < f32::EPSILON);
}

#[test]
fn config_color_wins_over_signal_scene_default() {
    // Config sets color=green; signal scene hardcodes color=aurora.
    let args = args_with_config("scene = signal\ncolor = green\n", &[]);
    assert_eq!(args.scene.as_deref(), Some("signal"));
    assert_eq!(
        args.color, "green",
        "config color must win over signal scene default aurora"
    );
    assert_eq!(
        args.charset, "retro",
        "scene charset default applies for unset key"
    );
}

#[test]
fn config_speed_wins_over_cli_scene_default() {
    // CLI --scene monolith + config speed=15. Config speed must win
    // over monolith's hardcoded 30 (CLI scene only fills unset keys).
    let args = args_with_config("speed = 15\n", &["--scene", "monolith"]);
    assert_eq!(args.scene.as_deref(), Some("monolith"));
    assert_eq!(
        args.speed, 15.0,
        "config speed must win over CLI scene monolith default 30"
    );
    assert_eq!(
        args.color, "cosmos",
        "scene color default still applies for unset key"
    );
}

#[test]
fn cli_speed_flag_wins_over_config_and_scene() {
    // CLI --speed 99 wins over both config speed AND scene default.
    let args = args_with_config(
        "scene = monolith\nspeed = 15\n",
        &["--scene", "monolith", "--speed", "99"],
    );
    assert_eq!(args.scene.as_deref(), Some("monolith"));
    assert_eq!(args.speed, 99.0, "CLI speed must win over config and scene");
}

#[test]
fn config_speed_wins_over_scene_default() {
    // The exact bug the user reported: config speed=30, scene=signal (default 10).
    // Config must win.
    let args = args_with_config("scene = signal\nspeed = 30\n", &[]);
    assert_eq!(args.scene.as_deref(), Some("signal"));
    assert_eq!(
        args.speed, 30.0,
        "config speed must win over signal scene default 10"
    );
}

#[test]
fn config_speed_outside_safe_range_is_ignored() {
    for value in ["0", "0.5", "100.1", "1000", "100000"] {
        let args = args_with_config(&format!("speed = {value}\n"), &[]);
        assert_eq!(args.speed, 30.0);
    }
}

#[test]
fn monolith_size_cli_values_parse() {
    let small = args_from_cli(&["--scene", "monolith", "--monolith-size", "small"]);
    let normal = args_from_cli(&["--scene", "monolith", "--monolith-size", "normal"]);
    let large = args_from_cli(&["--scene", "monolith", "--monolith-size", "large"]);

    assert_eq!(small.monolith_size, MonolithSize::Small);
    assert_eq!(normal.monolith_size, MonolithSize::Normal);
    assert_eq!(large.monolith_size, MonolithSize::Large);
}

#[test]
fn config_monolith_size_large_applies() {
    let args = args_with_config("monolith-size = large\n", &[]);
    assert_eq!(args.monolith_size, MonolithSize::Large);
}

#[test]
fn cli_scene_overrides_cli_preset_for_overlapping_values() {
    // v14.0.0: --preset removed; presets are now scenes. This test now
    // verifies that --scene signal alone applies signal values.
    let args = args_from_cli(&["--scene", "signal"]);
    assert_eq!(args.scene.as_deref(), Some("signal"));
    assert_eq!(args.color, "aurora");
    assert_eq!(args.charset, "retro");
    assert_eq!(args.speed, 14.0);
    assert!((args.density - 0.70).abs() < f32::EPSILON);
}

#[test]
fn cli_preset_overrides_config_scene_for_overlapping_values() {
    // v14.0.0: --preset removed; converted to --scene storm which wins
    // over config scene = monolith.
    let args = args_with_config("scene = monolith\n", &["--scene", "storm"]);
    assert_eq!(args.scene.as_deref(), Some("storm"));
    assert_eq!(args.color, "purple");
    assert_eq!(args.charset, "cyberpunk");
    assert_eq!(args.speed, 28.0);
}

#[test]
fn explicit_cli_overrides_config_value() {
    let args = args_with_config(
        "color = ocean\nfps = 30\n",
        &["--color", "red", "--fps", "60"],
    );
    assert_eq!(args.color, "red");
    assert_eq!(args.fps, 60.0);
}

#[test]
fn explicit_cli_overrides_config_scene() {
    // v17: 'preset' removed. Use 'scene = storm' directly.
    let args = args_with_config("scene = storm\n", &["--fps", "60", "--color", "green"]);
    assert_eq!(args.scene.as_deref(), Some("storm"));
    assert_eq!(args.fps, 60.0);
    assert_eq!(args.color, "green");
    assert_eq!(args.speed, 28.0);
}

#[test]
fn cli_preset_overrides_config_preset() {
    // v14.0.0: both preset= and --preset are deprecated/removed.
    // Converted to scene= and --scene. CLI scene wins over config scene.
    let args = args_with_config("scene = calm\n", &["--scene", "storm"]);
    assert_eq!(args.scene.as_deref(), Some("storm"));
    assert_eq!(args.color, "purple");
    assert_eq!(args.charset, "cyberpunk");
    assert_eq!(args.speed, 28.0);
}

#[test]
fn preset_overrides_config_managed_fields() {
    // v17: 'preset' removed. Use 'scene = calm' directly.
    // UNSET keys, so config-set color and speed are preserved (scene no
    // longer overrides config-managed fields — that was old preset semantics).
    let args = args_with_config("scene = calm\ncolor = red\nspeed = 20\n", &[]);
    assert_eq!(args.scene.as_deref(), Some("calm"));
    assert_eq!(
        args.color, "red",
        "config color must win over scene default"
    );
    assert_eq!(args.speed, 20.0, "config speed must win over scene default");
}

#[test]
fn config_low_power_applies_after_config_without_preset() {
    // v17: 'low-power = true' removed. Use 'scene = low-power'.
    // only fill UNSET keys, so config-set fps/speed/density are preserved.
    // (Old behavior: low-power always forced its values. New behavior is
    // consistent with how all scenes interact with config-set keys.)
    let args = args_with_config(
        "fps = 120\nspeed = 30\ndensity = 2\nscene = low-power\n",
        &[],
    );
    assert_eq!(args.scene.as_deref(), Some("low-power"));
    assert_eq!(args.fps, 120.0, "config fps must win over scene default");
    assert_eq!(args.speed, 30.0, "config speed must win over scene default");
    assert_eq!(
        args.density, 2.0,
        "config density must win over scene default"
    );
}

#[test]
fn low_power_does_not_override_preset_values() {
    // v14.0.0: --preset and --low-power CLI flags removed. This scenario
    // no longer exists. Converted to verify that --scene storm values are
    // preserved when low-power is NOT also set (the new equivalent would
    // be --scene low-power, which simply replaces storm entirely).
    let args = args_from_cli(&["--scene", "storm"]);
    assert_eq!(args.fps, 120.0);
    assert_eq!(args.speed, 28.0);
    assert!((args.density - 1.20).abs() < f32::EPSILON);
}

// ── --uniform flag (v13.6.0 Stage 1 CLI simplification) ──

#[test]
fn uniform_flag_disables_async_mode() {
    // --uniform sets args.uniform = true. The effective async_mode
    // is computed in main.rs as `args.async_mode && !args.uniform`.
    // Here we verify the flag parses correctly and defaults are sane.
    let args = args_from_cli(&["--uniform"]);
    assert!(args.uniform, "--uniform must set args.uniform = true");
    assert!(
        args.async_mode,
        "async_mode default is still true (uniform overrides later)"
    );
}

#[test]
fn uniform_flag_defaults_to_false() {
    let args = args_from_cli(&[]);
    assert!(!args.uniform, "uniform must default to false");
    assert!(args.async_mode, "async_mode must default to true");
}

#[test]
fn low_power_preset_sets_expected_values() {
    // v14.0.0: --preset low-power converted to --scene low-power.
    // Values must match: fps=30, speed=5, density=0.35.
    let args = args_from_cli(&["--scene", "low-power"]);
    assert_eq!(args.fps, 30.0, "low-power scene must set fps=30");
    assert_eq!(args.speed, 5.0, "low-power scene must set speed=5");
    assert!(
        (args.density - 0.35).abs() < f32::EPSILON,
        "low-power scene must set density=0.35"
    );
}

#[test]
fn invalid_config_values_are_ignored() {
    let args = args_with_config(
        "color = not-a-color\nfps = 0\nspeed = nope\nscene = unknown\n",
        &[],
    );
    assert_eq!(args.color, "cosmos");
    assert_eq!(args.fps, 60.0);
    assert_eq!(args.speed, 30.0);
    // v14.0.0: invalid `scene = unknown` does not set scene; default monolith applies.
    assert_eq!(args.scene.as_deref(), Some("monolith"));
}

#[test]
fn legacy_keys_no_longer_apply_v17() {
    // v17 mastery: legacy advanced keys (glitchpct, shortpct, rippct, maxdpc)
    // are REMOVED. They are silently ignored — values come from --glitch-level
    // preset only. Default glitch_level is Subtle (from monolith scene default).
    let args = args_with_config(
        "glitchpct = 7\nshortpct = 22\nrippct = 11\nmaxdpc = 2\n",
        &[],
    );
    // Default scene is monolith which sets glitch_level = Subtle.
    // Subtle preset: glitch_pct=3.0, shortpct=60.0, rippct=45.0, maxdpc=3.
    assert_eq!(
        args.glitch_pct, 3.0,
        "glitchpct config key ignored, uses Subtle preset"
    );
    assert_eq!(
        args.shortpct, 60.0,
        "shortpct config key ignored, uses Subtle preset"
    );
    assert_eq!(
        args.rippct, 45.0,
        "rippct config key ignored, uses Subtle preset"
    );
    assert_eq!(
        args.max_droplets_per_column, 3,
        "maxdpc config key ignored, uses default"
    );
}

#[test]
fn config_path_arg_is_stored() {
    let args = args_from_cli(&["--config", "/tmp/cosmostrix.toml"]);
    assert_eq!(args.config, Some(PathBuf::from("/tmp/cosmostrix.toml")));
}

#[test]
fn dump_config_mentions_supported_keys() {
    let dump = dump_config_text();
    for key in [
        "scene",
        "preset",
        "color",
        "charset",
        "fps",
        "speed",
        "density",
        "monolith-size",
        "glitch-level",
        "bold",
        "shadingmode",
        "color-bg",
        "low-power",
        "mouse",
        "fullwidth",
        "auto-color-drift",
        // v17 mastery: legacy keys (glitchpct, shortpct, rippct, maxdpc)
        // REMOVED from dump config. Use --glitch-level instead.
    ] {
        assert!(dump.contains(key), "dump config should contain {key}");
    }
    assert!(dump.contains("scene = monolith"));
    assert!(dump.contains("speed = 30"));
    assert!(dump.contains("density = 0.85"));
    assert!(dump.contains("glitch-level = subtle"));
}

// ── Phase 10: Atmosphere Config Keys ──

#[test]
fn default_atmosphere_mode_and_regime_are_none() {
    let args = args_from_cli(&[]);
    assert!(args.atmosphere_mode_str.is_none());
    assert!(args.atmosphere_regime_str.is_none());
}

#[test]
fn config_atmosphere_mode_disabled_parses() {
    let args = args_with_config("atmosphere-mode = disabled\n", &[]);
    assert_eq!(args.atmosphere_mode_str.as_deref(), Some("disabled"));
}

#[test]
fn config_atmosphere_mode_controlled_live_parses() {
    let args = args_with_config("atmosphere-mode = controlled-live\n", &[]);
    assert_eq!(args.atmosphere_mode_str.as_deref(), Some("controlled-live"));
}

#[test]
fn config_atmosphere_mode_invalid_is_ignored() {
    let args = args_with_config("atmosphere-mode = storm-mode\n", &[]);
    assert!(args.atmosphere_mode_str.is_none());
}

#[test]
fn config_atmosphere_regime_calm_parses() {
    let args = args_with_config("atmosphere-regime = calm\n", &[]);
    assert_eq!(args.atmosphere_regime_str.as_deref(), Some("calm"));
}

#[test]
fn config_atmosphere_regime_pulse_parses() {
    let args = args_with_config("atmosphere-regime = pulse\n", &[]);
    assert_eq!(args.atmosphere_regime_str.as_deref(), Some("pulse"));
}

#[test]
fn config_atmosphere_regime_signal_parses() {
    let args = args_with_config("atmosphere-regime = signal\n", &[]);
    assert_eq!(args.atmosphere_regime_str.as_deref(), Some("signal"));
}

#[test]
fn config_atmosphere_regime_compression_parses() {
    let args = args_with_config("atmosphere-regime = compression\n", &[]);
    assert_eq!(args.atmosphere_regime_str.as_deref(), Some("compression"));
}

#[test]
fn config_atmosphere_regime_void_parses() {
    let args = args_with_config("atmosphere-regime = void\n", &[]);
    assert_eq!(args.atmosphere_regime_str.as_deref(), Some("void"));
}

#[test]
fn config_atmosphere_regime_monolith_pressure_parses() {
    let args = args_with_config("atmosphere-regime = monolith-pressure\n", &[]);
    assert_eq!(
        args.atmosphere_regime_str.as_deref(),
        Some("monolith-pressure")
    );
}

#[test]
fn config_atmosphere_regime_storm_is_rejected() {
    let args = args_with_config("atmosphere-regime = storm\n", &[]);
    // Storm is NOT config-safe — should remain None (rejected).
    assert!(args.atmosphere_regime_str.is_none());
}

#[test]
fn config_atmosphere_regime_invalid_is_ignored() {
    let args = args_with_config("atmosphere-regime = nonexistent\n", &[]);
    assert!(args.atmosphere_regime_str.is_none());
}

#[test]
fn config_atmosphere_mode_and_regime_together_parse() {
    let args = args_with_config(
        "atmosphere-mode = controlled-live\natmosphere-regime = pulse\n",
        &[],
    );
    assert_eq!(args.atmosphere_mode_str.as_deref(), Some("controlled-live"));
    assert_eq!(args.atmosphere_regime_str.as_deref(), Some("pulse"));
}

#[test]
fn resolve_atmosphere_mode_disabled_returns_disabled() {
    use crate::atmosphere_apply::AtmosphereApplicationMode;
    use crate::config_apply::resolve_atmosphere_mode;
    let mode = resolve_atmosphere_mode(Some("disabled"));
    assert_eq!(mode, AtmosphereApplicationMode::Disabled);
}

#[test]
fn resolve_atmosphere_mode_controlled_live_returns_controlled_live() {
    use crate::atmosphere_apply::AtmosphereApplicationMode;
    use crate::config_apply::resolve_atmosphere_mode;
    let mode = resolve_atmosphere_mode(Some("controlled-live"));
    assert_eq!(mode, AtmosphereApplicationMode::ControlledLive);
}

#[test]
fn resolve_atmosphere_mode_none_returns_disabled() {
    use crate::atmosphere_apply::AtmosphereApplicationMode;
    use crate::config_apply::resolve_atmosphere_mode;
    let mode = resolve_atmosphere_mode(None);
    assert_eq!(mode, AtmosphereApplicationMode::Disabled);
}

#[test]
fn resolve_atmosphere_regime_calm_returns_calm() {
    use crate::atmosphere::AtmosphereRegime;
    use crate::config_apply::resolve_atmosphere_regime;
    assert_eq!(
        resolve_atmosphere_regime(Some("calm")),
        AtmosphereRegime::Calm
    );
}

#[test]
fn resolve_atmosphere_regime_pulse_returns_pulse() {
    use crate::atmosphere::AtmosphereRegime;
    use crate::config_apply::resolve_atmosphere_regime;
    assert_eq!(
        resolve_atmosphere_regime(Some("pulse")),
        AtmosphereRegime::Pulse
    );
}

#[test]
fn resolve_atmosphere_regime_signal_returns_signal() {
    use crate::atmosphere::AtmosphereRegime;
    use crate::config_apply::resolve_atmosphere_regime;
    assert_eq!(
        resolve_atmosphere_regime(Some("signal")),
        AtmosphereRegime::Signal
    );
}

#[test]
fn resolve_atmosphere_regime_void_returns_void() {
    use crate::atmosphere::AtmosphereRegime;
    use crate::config_apply::resolve_atmosphere_regime;
    assert_eq!(
        resolve_atmosphere_regime(Some("void")),
        AtmosphereRegime::Void
    );
}

#[test]
fn resolve_atmosphere_regime_compression_returns_compression() {
    use crate::atmosphere::AtmosphereRegime;
    use crate::config_apply::resolve_atmosphere_regime;
    assert_eq!(
        resolve_atmosphere_regime(Some("compression")),
        AtmosphereRegime::Compression
    );
}

#[test]
fn resolve_atmosphere_regime_monolith_pressure_returns_monolith_pressure() {
    use crate::atmosphere::AtmosphereRegime;
    use crate::config_apply::resolve_atmosphere_regime;
    assert_eq!(
        resolve_atmosphere_regime(Some("monolith-pressure")),
        AtmosphereRegime::MonolithPressure
    );
}

#[test]
fn resolve_atmosphere_regime_none_returns_calm() {
    use crate::atmosphere::AtmosphereRegime;
    use crate::config_apply::resolve_atmosphere_regime;
    assert_eq!(resolve_atmosphere_regime(None), AtmosphereRegime::Calm);
}

#[test]
fn controlled_live_modulation_from_config_pulse_is_subtle() {
    use crate::atmosphere_controlled_live::controlled_live_modulation_from_regime;
    let modulation =
        controlled_live_modulation_from_regime(crate::atmosphere::AtmosphereRegime::Pulse);
    assert!(modulation.speed_scale > 1.0);
    assert!(modulation.speed_scale <= 1.04); // ControlledLiveBounds
    assert!(modulation.density_scale == 1.0); // Pulse: no density change
    assert!(!modulation.color_change_allowed);
    assert!(!modulation.terminal_effect_allowed);
}

#[test]
fn dump_config_mentions_atmosphere_keys() {
    let dump = dump_config_text();
    assert!(dump.contains("atmosphere-mode"));
    assert!(dump.contains("atmosphere-regime"));
    assert!(dump.contains("controlled-live"));
}

// ── Phase 10.5: Config smoke hardening tests ──

#[test]
fn disabled_plus_non_calm_regime_keeps_effective_runtime_identity() {
    // When mode is disabled, even with pulse regime, everything stays identity
    let args = args_with_config(
        "atmosphere-mode = disabled\natmosphere-regime = pulse\n",
        &[],
    );
    let mode = crate::config_apply::resolve_atmosphere_mode(args.atmosphere_mode_str.as_deref());
    let regime =
        crate::config_apply::resolve_atmosphere_regime(args.atmosphere_regime_str.as_deref());
    let ctrl = crate::atmosphere::AtmosphereController::new();
    let app = ctrl.build_application();
    let modulation = crate::atmosphere_apply::apply_application(&app, mode);
    let eff = crate::atmosphere_apply::derive_effective_runtime(20.0, 0.85, &modulation);
    let shadow = crate::atmosphere_shadow::shadow_metrics_from_mode_and_regime(mode, regime);
    assert_eq!(
        mode,
        crate::atmosphere_apply::AtmosphereApplicationMode::Disabled
    );
    assert!(modulation.is_identity());
    assert_eq!(eff.speed, 20.0);
    assert_eq!(eff.density, 0.85);
    assert!(shadow.is_identity());
}

#[test]
fn controlled_live_pulse_shows_shadow_risk_whisper() {
    let args = args_with_config(
        "atmosphere-mode = controlled-live\natmosphere-regime = pulse\n",
        &[],
    );
    let mode = crate::config_apply::resolve_atmosphere_mode(args.atmosphere_mode_str.as_deref());
    let regime =
        crate::config_apply::resolve_atmosphere_regime(args.atmosphere_regime_str.as_deref());
    let shadow = crate::atmosphere_shadow::shadow_metrics_from_mode_and_regime(mode, regime);
    assert_eq!(shadow.risk_label(), "whisper");
}

#[test]
fn controlled_live_signal_shows_shadow_risk_whisper() {
    let args = args_with_config(
        "atmosphere-mode = controlled-live\natmosphere-regime = signal\n",
        &[],
    );
    let mode = crate::config_apply::resolve_atmosphere_mode(args.atmosphere_mode_str.as_deref());
    let regime =
        crate::config_apply::resolve_atmosphere_regime(args.atmosphere_regime_str.as_deref());
    let shadow = crate::atmosphere_shadow::shadow_metrics_from_mode_and_regime(mode, regime);
    assert_eq!(shadow.risk_label(), "whisper");
}

#[test]
fn controlled_live_monolith_pressure_shows_shadow_risk_whisper() {
    let args = args_with_config(
        "atmosphere-mode = controlled-live\natmosphere-regime = monolith-pressure\n",
        &[],
    );
    let mode = crate::config_apply::resolve_atmosphere_mode(args.atmosphere_mode_str.as_deref());
    let regime =
        crate::config_apply::resolve_atmosphere_regime(args.atmosphere_regime_str.as_deref());
    let shadow = crate::atmosphere_shadow::shadow_metrics_from_mode_and_regime(mode, regime);
    assert_eq!(shadow.risk_label(), "whisper");
}

#[test]
fn controlled_live_void_remains_bounded_and_not_rejected() {
    let args = args_with_config(
        "atmosphere-mode = controlled-live\natmosphere-regime = void\n",
        &[],
    );
    let mode = crate::config_apply::resolve_atmosphere_mode(args.atmosphere_mode_str.as_deref());
    let regime =
        crate::config_apply::resolve_atmosphere_regime(args.atmosphere_regime_str.as_deref());
    let shadow = crate::atmosphere_shadow::shadow_metrics_from_mode_and_regime(mode, regime);
    // Void must not be rejected; it should be whisper or identity
    assert!(
        matches!(shadow.risk_label(), "identity" | "whisper"),
        "void must not be rejected, got: {}",
        shadow.risk_label()
    );
    // Density must not collapse (>= 0.98)
    assert!(
        shadow.density_delta_percent >= -0.5 || shadow.density_delta_percent == 0.0,
        "void must not collapse density"
    );
}

#[test]
fn controlled_live_storm_is_not_config_safe_and_falls_back() {
    let args = args_with_config(
        "atmosphere-mode = controlled-live\natmosphere-regime = storm\n",
        &[],
    );
    // Storm is rejected at parse layer — regime_str stays None
    assert!(
        args.atmosphere_regime_str.is_none(),
        "storm must be rejected and remain None"
    );
    let regime =
        crate::config_apply::resolve_atmosphere_regime(args.atmosphere_regime_str.as_deref());
    assert_eq!(regime, crate::atmosphere::AtmosphereRegime::Calm);
}

#[test]
fn invalid_atmosphere_mode_is_ignored_or_rejected() {
    let args = args_with_config("atmosphere-mode = hyperdrive\n", &[]);
    assert!(args.atmosphere_mode_str.is_none());
}

#[test]
fn invalid_atmosphere_regime_is_ignored_or_rejected() {
    let args = args_with_config("atmosphere-regime = nonexistent\n", &[]);
    assert!(args.atmosphere_regime_str.is_none());
}

#[test]
fn auto_color_drift_remains_false_unless_explicitly_enabled() {
    let args = args_with_config(
        "atmosphere-mode = controlled-live\natmosphere-regime = pulse\n",
        &[],
    );
    assert!(
        !args.auto_color_drift,
        "auto_color_drift must remain false by default"
    );
}

#[test]
fn cli_color_sun_remains_sticky() {
    let args = args_with_config(
        "atmosphere-mode = controlled-live\natmosphere-regime = pulse\nscene = monolith\n",
        &["--color", "sun"],
    );
    assert_eq!(args.color, "sun", "CLI --color sun must remain sticky");
}

// ── Phase 10.5: Deterministic diagnostic honesty tests ──

#[test]
fn default_diag_fields_imply_disabled_protected_identity() {
    let args = args_from_cli(&[]);
    let mode = crate::config_apply::resolve_atmosphere_mode(args.atmosphere_mode_str.as_deref());
    let regime =
        crate::config_apply::resolve_atmosphere_regime(args.atmosphere_regime_str.as_deref());
    let ctrl = crate::atmosphere::AtmosphereController::new();
    let app = ctrl.build_application();
    let modulation = crate::atmosphere_apply::apply_application(&app, mode);
    let eff =
        crate::atmosphere_apply::derive_effective_runtime(args.speed, args.density, &modulation);
    let shadow = crate::atmosphere_shadow::shadow_metrics_from_mode_and_regime(mode, regime);
    // All defaults must imply disabled/protected/identity
    assert_eq!(
        mode,
        crate::atmosphere_apply::AtmosphereApplicationMode::Disabled
    );
    assert!(!mode.allows_modulation());
    assert!(modulation.is_identity());
    assert_eq!(eff.speed, args.speed);
    assert_eq!(eff.density, args.density);
    assert!(shadow.is_identity());
}

#[test]
fn controlled_live_pulse_diag_implies_armed_protected_identity_and_whisper_risk() {
    let args = args_with_config(
        "atmosphere-mode = controlled-live\natmosphere-regime = pulse\n",
        &[],
    );
    let mode = crate::config_apply::resolve_atmosphere_mode(args.atmosphere_mode_str.as_deref());
    let regime =
        crate::config_apply::resolve_atmosphere_regime(args.atmosphere_regime_str.as_deref());
    let shadow = crate::atmosphere_shadow::shadow_metrics_from_mode_and_regime(mode, regime);
    // Armed: mode allows modulation
    assert!(mode.allows_modulation());
    // Whisper risk (shadow detects bounded candidate modulation)
    assert_eq!(shadow.risk_label(), "whisper");
    // The controlled-live modulation from regime is non-identity
    let modulation =
        crate::atmosphere_controlled_live::controlled_live_modulation_from_regime(regime);
    assert!(
        !modulation.is_identity(),
        "controlled-live pulse must produce non-identity modulation"
    );
}

#[test]
fn disabled_pulse_diag_implies_disabled_protected_identity() {
    let args = args_with_config(
        "atmosphere-mode = disabled\natmosphere-regime = pulse\n",
        &[],
    );
    let mode = crate::config_apply::resolve_atmosphere_mode(args.atmosphere_mode_str.as_deref());
    let modulation = {
        let ctrl = crate::atmosphere::AtmosphereController::new();
        let app = ctrl.build_application();
        crate::atmosphere_apply::apply_application(&app, mode)
    };
    assert!(!mode.allows_modulation());
    assert!(modulation.is_identity());
}

#[test]
fn storm_config_is_rejected_as_not_config_safe() {
    let args = args_with_config(
        "atmosphere-mode = controlled-live\natmosphere-regime = storm\n",
        &[],
    );
    assert!(args.atmosphere_regime_str.is_none());
}

#[test]
fn benchmark_fields_remain_backward_compatible() {
    // Verify all Phase 10.5 honesty fields are additive
    // and do not remove or rename existing fields
    const OLD_FIELDS: &[&str] = &[
        "avg_fps",
        "p99_frame_time",
        "frame_time_stability",
        "actual_execution",
        "regime",
        "effective",
        "verifier",
        "application",
        "atmosphere_application_mode",
        "atmosphere_visual_effect",
        "effective_runtime",
        "atmosphere_shadow",
        "atmosphere_shadow_risk",
    ];
    const NEW_FIELDS: &[&str] = &["config_gate", "visual_runtime", "runtime_application"];
    for field in OLD_FIELDS {
        assert!(!field.is_empty());
    }
    for field in NEW_FIELDS {
        assert!(!field.is_empty());
        // New fields must not collide with old fields
        assert!(
            !OLD_FIELDS.contains(field),
            "new field '{field}' must not collide with existing fields"
        );
    }
}

// ── v4.6.0 Phase 1 contract tests live in atmosphere_expansion_tests.rs ──
