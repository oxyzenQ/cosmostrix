// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Aggregated umbrella module for `config_apply` tests.
//!
//! Groups sibling test files (`profiles`) so `main.rs` declares only one
//! `mod config_apply_tests;` instead of two flat modules.

#![cfg(test)]

mod profiles;

use std::sync::atomic::{AtomicU64, Ordering};

use clap::{CommandFactory, FromArgMatches};

use crate::config::{Args, GlitchLevel, IntroType};
use crate::config_apply::{apply_config_and_runtime_defaults, parse_bool_config};
use crate::configfile::dump_config_text;
use crate::runtime::MonolithSize;

/// Global counter for unique temp file names. Prevents collisions when
/// multiple tests run in parallel and `SystemTime::now()` returns the
/// same nanosecond on fast CI runners.
static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Set COSMOSTRIX_TEST_CONFIG_DIR so is_safe_path allows the actual temp
/// directory during tests. Uses `std::env::temp_dir()` instead of hardcoding
/// `/tmp` because on some CI runners (macOS Arch packaging) temp_dir()
/// returns `/var/cache/makepkg-build/tmp/` rather than `/tmp`, causing
/// path validation to reject config files in the "wrong" temp location.
/// Idempotent — safe to call from parallel test threads.
fn ensure_test_config_dir_allowed() {
    std::env::set_var("COSMOSTRIX_SKIP_STARTUP_VALIDATION", "1");
    // Setting the same value repeatedly is benign even under race conditions.
    std::env::set_var(
        "COSMOSTRIX_TEST_CONFIG_DIR",
        std::env::temp_dir().to_string_lossy().into_owned(),
    );
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
    // v30 simplify: --noglitch CLI flag removed; glitch_enabled is now
    // derived from glitch_level != None. Subtle is not None, so a Cloud
    // built from this args would have glitch_enabled = true.
    assert!(args.glitch_level != GlitchLevel::None);
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
fn default_scene_is_cinematic() {
    let args = args_from_cli(&[]);
    assert_eq!(args.scene.as_deref(), Some("cinematic"));
    assert_eq!(args.color, "neon-purple");
    assert_eq!(args.charset, "zen");
    assert_eq!(args.speed, 9.0);
    assert_eq!(args.density, 0.75);
    assert_eq!(args.glitch_level, GlitchLevel::Subtle);
}

/// v25.16: benchmark mode (--benchmark or --bench-all) without an explicit
/// --scene must default to "monolith" instead of the interactive default
/// "cinematic". This is the fix for the FPS regression where users running
/// `cosmostrix --benchmark` got cinematic (slow) instead of monolith (peak),
/// producing misleadingly low FPS numbers vs the headline 38k FPS claims.
///
/// The override is applied in main.rs BEFORE apply_config_and_runtime_defaults
/// is called, so all scene config (color, charset, speed, density, rain_style)
/// is correctly populated for monolith. This test replicates that pre-apply
/// injection to pin the contract.
#[test]
fn benchmark_mode_defaults_to_monolith_scene() {
    // Replicate the main.rs pre-apply override: if benchmark mode and no
    // --scene was passed, inject args.scene = Some("monolith") BEFORE
    // apply_config_and_runtime_defaults. Then verify the resolved scene
    // and monolith's signature config fields.
    let cli = &["--benchmark"];
    let mut argv = vec!["cosmostrix"];
    argv.extend_from_slice(cli);
    let cmd = Args::command();
    let matches = cmd.get_matches_from(argv);
    let mut args = Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());

    // Mirror main.rs: bench_mode && args.scene.is_none() → inject monolith.
    let bench_mode = args.benchmark || args.bench_all;
    assert!(bench_mode, "--benchmark must set args.benchmark = true");
    assert!(
        args.scene.is_none(),
        "args.scene must be None before the override (no --scene passed)"
    );
    if bench_mode && args.scene.is_none() {
        args.scene = Some("monolith".to_string());
    }

    apply_config_and_runtime_defaults(&matches, &mut args).expect("apply config");

    // After apply: scene is monolith, with monolith's signature config.
    assert_eq!(args.scene.as_deref(), Some("monolith"));
    assert_eq!(args.color, "neon-purple");
    assert_eq!(args.charset, "zen");
    assert_eq!(args.speed, 30.0);
    assert_eq!(args.density, 0.85);
    assert_eq!(args.glitch_level, GlitchLevel::Subtle);
}

/// v25.16: --benchmark with explicit --scene <name> must NOT override to
/// monolith. The user's choice wins. This pins the override-with-override
/// contract: `cosmostrix --benchmark --scene cinematic` benchmarks cinematic.
#[test]
fn benchmark_mode_with_explicit_scene_keeps_user_choice() {
    let cli = &["--benchmark", "--scene", "cinematic"];
    let mut argv = vec!["cosmostrix"];
    argv.extend_from_slice(cli);
    let cmd = Args::command();
    let matches = cmd.get_matches_from(argv);
    let mut args = Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());

    // Mirror main.rs: only inject if args.scene.is_none(). Here it's Some.
    let bench_mode = args.benchmark || args.bench_all;
    assert!(bench_mode);
    assert_eq!(
        args.scene.as_deref(),
        Some("cinematic"),
        "user-supplied --scene must be present before apply"
    );
    if bench_mode && args.scene.is_none() {
        args.scene = Some("monolith".to_string());
    }

    apply_config_and_runtime_defaults(&matches, &mut args).expect("apply config");

    // User's cinematic choice is honored, NOT overridden to monolith.
    assert_eq!(args.scene.as_deref(), Some("cinematic"));
    assert_eq!(args.color, "neon-purple");
    assert_eq!(args.charset, "zen");
    assert_eq!(args.speed, 9.0);
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
    assert_eq!(args.color, "neon-purple");
    assert_eq!(args.charset, "zen");
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
    let args = args_from_cli(&["--scene", "monolith", "--color", "cosmos"]);
    assert_eq!(args.scene.as_deref(), Some("monolith"));
    assert_eq!(args.color, "cosmos");
    assert_eq!(args.charset, "zen");
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
    assert_eq!(args.color, "neon-purple");
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
        args.color, "neon-purple",
        "scene color default applies for unset key"
    );
    assert!((args.density - 0.85).abs() < f32::EPSILON);
}

#[test]
fn config_density_wins_over_signal_scene_default() {
    // Config sets density=0.5; signal scene hardcodes density=0.55.
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
        args.color, "neon-purple",
        "scene color default still applies for unset key"
    );
}

/// v25 priority contract (corrected): `--scene <name>` selects the base
/// scene, but config.toml CAN override scene-managed fields. Only per-field
/// CLI flags block config overrides. This test verifies:
///   `cosmostrix --scene cinematic` with config color=carbon produces
///   color=carbon (config wins over cinematic's neon-purple).
#[test]
fn config_overrides_scene_managed_fields_when_scene_is_cli() {
    let args = args_with_config(
        "color = carbon\nfps = 300\nspeed = 50\n",
        &["--scene", "cinematic"],
    );
    assert_eq!(args.scene.as_deref(), Some("cinematic"));
    // config color=carbon wins over cinematic's neon-purple
    assert_eq!(
        args.color, "carbon",
        "config color must win over CLI scene cinematic's neon-purple"
    );
    // config fps=300 wins over cinematic's fps=60
    assert_eq!(
        args.fps, 300.0,
        "config fps must win over CLI scene cinematic's fps=60"
    );
    // config speed=50 wins over cinematic's speed=9
    assert_eq!(
        args.speed, 50.0,
        "config speed must win over CLI scene cinematic's speed=9"
    );
}

/// Per-field CLI flag wins over both config AND scene default.
/// `cosmostrix --scene cinematic -c snow` produces color=snow.
#[test]
fn per_field_cli_flag_wins_over_config_and_scene() {
    let args = args_with_config("color = carbon\n", &["--scene", "cinematic", "-c", "snow"]);
    assert_eq!(args.scene.as_deref(), Some("cinematic"));
    // CLI -c snow wins over config carbon AND cinematic's neon-purple
    assert_eq!(
        args.color, "snow",
        "per-field CLI -c snow must win over config and scene default"
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
        // Default scene is cinematic, which sets speed=9.0.
        assert_eq!(args.speed, 9.0);
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
    assert!((args.density - 0.55).abs() < f32::EPSILON);
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
    assert!((args.density - 1.10).abs() < f32::EPSILON);
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
    // Values must match: fps=30, speed=5, density=0.45.
    let args = args_from_cli(&["--scene", "low-power"]);
    assert_eq!(args.fps, 30.0, "low-power scene must set fps=30");
    assert_eq!(args.speed, 5.0, "low-power scene must set speed=5");
    assert!(
        (args.density - 0.45).abs() < f32::EPSILON,
        "low-power scene must set density=0.45"
    );
}

#[test]
fn invalid_config_values_are_ignored() {
    let args = args_with_config(
        "color = not-a-color\nfps = 0\nspeed = nope\nscene = unknown\n",
        &[],
    );
    assert_eq!(args.color, "neon-purple");
    assert_eq!(args.fps, 60.0);
    // Default scene is cinematic, which sets speed=9.0.
    assert_eq!(args.speed, 9.0);
    // v14.0.0: invalid `scene = unknown` does not set scene; default cinematic applies.
    assert_eq!(args.scene.as_deref(), Some("cinematic"));
}

#[test]
fn legacy_keys_no_longer_apply_v17() {
    // v17 mastery: legacy advanced keys (glitchpct, shortpct, rippct, maxdpc)
    // are REMOVED. They are silently ignored — values come from --glitch-level
    // preset only. Default glitch_level is Subtle (from cinematic scene default).
    let args = args_with_config(
        "glitchpct = 7\nshortpct = 22\nrippct = 11\nmaxdpc = 2\n",
        &[],
    );
    // Default scene is cinematic which sets glitch_level = Subtle.
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
    // Use a real temp file in std::env::temp_dir() instead of hardcoding
    // "/tmp/cosmostrix.toml" — parallel safepath tests can clear
    // COSMOSTRIX_TEST_CONFIG_DIR between set_var and is_safe_path,
    // rejecting a static /tmp path. A dynamic path always matches the
    // COSMOSTRIX_TEST_CONFIG_DIR prefix set by ensure_test_config_dir_allowed().
    ensure_test_config_dir_allowed();
    let mut path = std::env::temp_dir();
    path.push("cosmostrix-config-path-test.toml");
    std::fs::write(&path, "").expect("write temp config");
    let path_str = path.to_string_lossy().into_owned();

    let args = args_from_cli(&["--config", &path_str]);
    assert_eq!(args.config, Some(path.clone()));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn dump_config_mentions_supported_keys() {
    let dump = dump_config_text();
    for key in [
        "scene",
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
        "auto-color-drift",
        // v30.1 simplification: legacy/historical key mentions removed
        // from dump config (mouse flag was v17-deletion note, preset was
        // v20.1-removal note). Both are gone in the simplified dump.
    ] {
        assert!(dump.contains(key), "dump config should contain {key}");
    }
    // v30.1: the standard example values are still present.
    assert!(dump.contains("glitch-level = subtle"));
    assert!(dump.contains("scene = cinematic"));
    assert!(dump.contains("speed = 9"));
    assert!(dump.contains("density = 0.75"));
}

// ── Atmosphere engine subsystem fully eliminated (Dragon Hunt v2 Phase 6
// Tier E item 31). All atmosphere tests removed. The v4.6.0 contract tests
// previously in atmosphere_expansion_tests.rs / atmosphere_tests/ have been
// deleted along with the atmosphere source files. ──

// ── v20 intro config key tests ──────────────────────────────────────────────

#[test]
fn config_intro_logo_applies() {
    // Setting intro = "logo" in config sets args.intro to Some(Logo).
    let args = args_with_config("intro = logo\n", &[]);
    assert_eq!(args.intro, Some(IntroType::Logo));
}

#[test]
fn config_intro_cosmic_applies() {
    // Setting intro = "cosmic" in config sets args.intro to Some(Cosmic).
    let args = args_with_config("intro = cosmic\n", &[]);
    assert_eq!(args.intro, Some(IntroType::Cosmic));
}

#[test]
fn config_intro_none_applies() {
    // Setting intro = "none" in config sets args.intro to Some(None).
    // This is different from leaving the key unset (which leaves
    // args.intro as None and falls back to the Logo default in main.rs).
    let args = args_with_config("intro = none\n", &[]);
    assert_eq!(args.intro, Some(IntroType::None));
}

#[test]
fn config_intro_missing_leaves_args_none() {
    // When the config key is absent and no CLI flag is provided,
    // args.intro stays None — main.rs resolves the Logo default via
    // unwrap_or(IntroType::Logo). This test verifies the config layer
    // does NOT eagerly set a default.
    let args = args_with_config("", &[]);
    assert_eq!(args.intro, None);
}

#[test]
fn cli_intro_flag_wins_over_config() {
    // CLI --intro flag takes precedence over the config key.
    let args = args_with_config("intro = cosmic\n", &["--intro", "none"]);
    assert_eq!(args.intro, Some(IntroType::None));
}

#[test]
fn cli_intro_bare_flag_wins_over_config() {
    // Bare --intro (no value) uses default_missing_value = "logo" and
    // still wins over the config key.
    let args = args_with_config("intro = cosmic\n", &["--intro"]);
    assert_eq!(args.intro, Some(IntroType::Logo));
}

#[test]
fn dump_config_mentions_intro_key() {
    // The dump-config template should document the new `intro` key so
    // users discover it via `cosmostrix --dump-config`.
    let dump = dump_config_text();
    assert!(
        dump.contains("intro"),
        "dump-config should mention the 'intro' key"
    );
    // Should mention all three valid values.
    assert!(dump.contains("logo"));
    assert!(dump.contains("cosmic"));
    assert!(dump.contains("none"));
}

// ── Phase D Bug #1: parse_bool_config parser unification ───────────────

#[test]
fn parse_bool_config_accepts_lenient_true_values() {
    assert_eq!(parse_bool_config("test", "true"), Some(true));
    assert_eq!(parse_bool_config("test", "yes"), Some(true));
    assert_eq!(parse_bool_config("test", "on"), Some(true));
    assert_eq!(parse_bool_config("test", "1"), Some(true));
    // Case-insensitive
    assert_eq!(parse_bool_config("test", "TRUE"), Some(true));
    assert_eq!(parse_bool_config("test", "Yes"), Some(true));
    assert_eq!(parse_bool_config("test", "ON"), Some(true));
    // Trims whitespace
    assert_eq!(parse_bool_config("test", "  true  "), Some(true));
}

#[test]
fn parse_bool_config_accepts_lenient_false_values() {
    assert_eq!(parse_bool_config("test", "false"), Some(false));
    assert_eq!(parse_bool_config("test", "no"), Some(false));
    assert_eq!(parse_bool_config("test", "off"), Some(false));
    assert_eq!(parse_bool_config("test", "0"), Some(false));
    // Case-insensitive
    assert_eq!(parse_bool_config("test", "FALSE"), Some(false));
    assert_eq!(parse_bool_config("test", "No"), Some(false));
    assert_eq!(parse_bool_config("test", "OFF"), Some(false));
    // Trims whitespace
    assert_eq!(parse_bool_config("test", "  false  "), Some(false));
}

#[test]
fn parse_bool_config_rejects_invalid_values() {
    assert_eq!(parse_bool_config("test", "maybe"), None);
    assert_eq!(parse_bool_config("test", "2"), None);
    assert_eq!(parse_bool_config("test", ""), None);
    assert_eq!(parse_bool_config("test", "enabled"), None);
    assert_eq!(parse_bool_config("test", "disabled"), None);
}
