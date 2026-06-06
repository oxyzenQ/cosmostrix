// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use clap::{CommandFactory, FromArgMatches};

use crate::config::{Args, GlitchLevel};
use crate::config_apply::apply_config_and_runtime_defaults;
use crate::configfile::dump_config_text;
use crate::runtime::MonolithSize;

fn args_with_config(config: &str, cli: &[&str]) -> Args {
    let mut path = std::env::temp_dir();
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after unix epoch")
        .as_nanos();
    path.push(format!(
        "cosmostrix-config-test-{}-{unique}.conf",
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
        let mut argv = vec!["cosmostrix"];
        argv.extend_from_slice(cli);
        let cmd = Args::command();
        let matches = cmd.get_matches_from(argv);
        let mut args = Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
        apply_config_and_runtime_defaults(&matches, &mut args)?;
        return Ok(args);
    }

    let mut path = std::env::temp_dir();
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after unix epoch")
        .as_nanos();
    path.push(format!(
        "cosmostrix-empty-config-test-{}-{unique}.conf",
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
fn config_preset_calm_applies() {
    let args = args_with_config("preset = calm\n", &[]);
    assert_eq!(args.preset.as_deref(), Some("calm"));
    assert_eq!(args.color, "ocean");
    assert_eq!(args.charset, "minimal");
    assert_eq!(args.speed, 5.0);
    assert!((args.density - 0.65).abs() < f32::EPSILON);
}

#[test]
fn default_scene_is_monolith() {
    let args = args_from_cli(&[]);
    assert_eq!(args.scene.as_deref(), Some("monolith"));
    assert_eq!(args.color, "cosmos");
    assert_eq!(args.charset, "binary");
    assert_eq!(args.speed, 20.0);
    assert_eq!(args.density, 0.75);
    assert_eq!(args.glitch_level, GlitchLevel::Subtle);
}

#[test]
fn explicit_matrix_scene_restores_classic_defaults() {
    let args = args_from_cli(&["--scene", "matrix"]);
    assert_eq!(args.scene.as_deref(), Some("matrix"));
    assert_eq!(args.color, "green");
    assert_eq!(args.charset, "binary");
    assert_eq!(args.speed, 8.0);
    assert_eq!(args.density, 1.0);
    assert_eq!(args.glitch_level, GlitchLevel::Default);
}

#[test]
fn invalid_cli_scene_is_clear_error() {
    let err = args_from_cli_result(&["--scene", "nonexistent"]).unwrap_err();
    assert_eq!(err, "invalid scene: nonexistent (see --list-scenes)");
}

#[test]
fn config_scene_monolith_applies() {
    let args = args_with_config("scene = monolith\n", &[]);
    assert_eq!(args.scene.as_deref(), Some("monolith"));
    assert_eq!(args.color, "cosmos");
    assert_eq!(args.charset, "binary");
    assert_eq!(args.speed, 20.0);
    assert!((args.density - 0.75).abs() < f32::EPSILON);
    assert_eq!(args.glitch_level, GlitchLevel::Subtle);
    assert_eq!(args.glitch_pct, 3.0);
}

#[test]
fn cli_scene_overrides_config_scene() {
    let args = args_with_config("scene = monolith\n", &["--scene", "signal"]);
    assert_eq!(args.scene.as_deref(), Some("signal"));
    assert_eq!(args.color, "cyan");
    assert_eq!(args.charset, "code");
    assert_eq!(args.speed, 10.0);
}

#[test]
fn explicit_cli_flags_override_scene_managed_values() {
    let args = args_from_cli(&["--scene", "signal", "--color", "green", "--fps", "120"]);
    assert_eq!(args.scene.as_deref(), Some("signal"));
    assert_eq!(args.color, "green");
    assert_eq!(args.fps, 120.0);
    assert_eq!(args.charset, "code");
    assert_eq!(args.speed, 10.0);
}

#[test]
fn monolith_scene_respects_explicit_color_override() {
    let args = args_from_cli(&["--scene", "monolith", "--color", "deepspace"]);
    assert_eq!(args.scene.as_deref(), Some("monolith"));
    assert_eq!(args.color, "deepspace");
    assert_eq!(args.charset, "binary");
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

#[test]
fn config_speed_outside_safe_range_is_ignored() {
    for value in ["0", "0.5", "100.1", "1000", "100000"] {
        let args = args_with_config(&format!("speed = {value}\n"), &[]);
        assert_eq!(args.speed, 20.0);
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
    let args = args_from_cli(&["--preset", "calm", "--scene", "signal"]);
    assert_eq!(args.preset.as_deref(), Some("calm"));
    assert_eq!(args.scene.as_deref(), Some("signal"));
    assert_eq!(args.color, "cyan");
    assert_eq!(args.charset, "code");
    assert_eq!(args.speed, 10.0);
    assert!((args.density - 0.95).abs() < f32::EPSILON);
}

#[test]
fn cli_preset_overrides_config_scene_for_overlapping_values() {
    let args = args_with_config("scene = monolith\n", &["--preset", "storm"]);
    assert_eq!(args.scene.as_deref(), Some("monolith"));
    assert_eq!(args.preset.as_deref(), Some("storm"));
    assert_eq!(args.color, "purple");
    assert_eq!(args.charset, "cyberpunk");
    assert_eq!(args.speed, 24.0);
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
fn explicit_cli_overrides_config_preset() {
    let args = args_with_config("preset = storm\n", &["--fps", "60", "--color", "green"]);
    assert_eq!(args.preset.as_deref(), Some("storm"));
    assert_eq!(args.fps, 60.0);
    assert_eq!(args.color, "green");
    assert_eq!(args.speed, 24.0);
}

#[test]
fn cli_preset_overrides_config_preset() {
    let args = args_with_config("preset = calm\n", &["--preset", "storm"]);
    assert_eq!(args.preset.as_deref(), Some("storm"));
    assert_eq!(args.color, "purple");
    assert_eq!(args.charset, "cyberpunk");
    assert_eq!(args.speed, 24.0);
}

#[test]
fn preset_overrides_config_managed_fields() {
    let args = args_with_config("preset = calm\ncolor = red\nspeed = 20\n", &[]);
    assert_eq!(args.color, "ocean");
    assert_eq!(args.speed, 5.0);
}

#[test]
fn config_low_power_applies_after_config_without_preset() {
    let args = args_with_config(
        "fps = 120\nspeed = 30\ndensity = 2\nlow-power = true\n",
        &[],
    );
    assert_eq!(args.fps, 30.0);
    assert_eq!(args.speed, 5.0);
    assert_eq!(args.density, 0.5);
}

#[test]
fn low_power_does_not_override_preset_values() {
    let args = args_from_cli(&["--preset", "storm", "--low-power"]);
    assert_eq!(args.fps, 120.0);
    assert_eq!(args.speed, 24.0);
    assert!((args.density - 1.35).abs() < f32::EPSILON);
}

#[test]
fn invalid_config_values_are_ignored() {
    let args = args_with_config(
        "color = not-a-color\nfps = 0\nspeed = nope\nlow-power = maybe\npreset = unknown\n",
        &[],
    );
    assert_eq!(args.color, "cosmos");
    assert_eq!(args.fps, 60.0);
    assert_eq!(args.speed, 20.0);
    assert!(!args.low_power);
    assert!(args.preset.is_none());
}

#[test]
fn legacy_keys_still_apply() {
    let args = args_with_config(
        "glitchpct = 7\nshortpct = 22\nrippct = 11\nmaxdpc = 2\n",
        &[],
    );
    assert_eq!(args.glitch_pct, 7.0);
    assert_eq!(args.shortpct, 22.0);
    assert_eq!(args.rippct, 11.0);
    assert_eq!(args.max_droplets_per_column, 2);
}

#[test]
fn config_path_arg_is_stored() {
    let args = args_from_cli(&["--config", "/tmp/cosmostrix.conf"]);
    assert_eq!(args.config, Some(PathBuf::from("/tmp/cosmostrix.conf")));
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
        "glitchpct",
        "shortpct",
        "rippct",
        "maxdpc",
    ] {
        assert!(dump.contains(key), "dump config should contain {key}");
    }
    assert!(dump.contains("scene = monolith"));
    assert!(dump.contains("speed = 20"));
    assert!(dump.contains("density = 0.75"));
    assert!(dump.contains("glitch-level = subtle"));
}
