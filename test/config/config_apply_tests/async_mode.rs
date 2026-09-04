// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the `--async-mode` CLI flag (v50-beta.3: replaces `--uniform`).
//! Extracted from `config_apply_tests/mod.rs` to keep that source file under
//! the 800-LOC cap. Pure code motion — no behavior change.

#![cfg(test)]

use super::{args_from_cli, args_with_config, ensure_test_config_dir_allowed};
use crate::configfile::dump_config_text;

// ── --async-mode CLI flag (v50-beta.3: replaces --uniform) ──

#[test]
fn async_mode_flag_false_disables_variable_pacing() {
    // --async-mode false sets args.async_mode = Some(false).
    // main.rs computes effective_async = args.async_mode.unwrap_or(true) = false.
    let args = args_from_cli(&["--async-mode", "false"]);
    assert_eq!(
        args.async_mode,
        Some(false),
        "--async-mode false must set args.async_mode = Some(false)"
    );
}

#[test]
fn async_mode_flag_true_enables_variable_pacing() {
    let args = args_from_cli(&["--async-mode", "true"]);
    assert_eq!(args.async_mode, Some(true));
}

#[test]
fn async_mode_defaults_to_none_when_unset() {
    // When neither CLI nor config provides a value, args.async_mode stays None.
    // main.rs applies the default (true) via unwrap_or(true).
    let args = args_from_cli(&[]);
    assert_eq!(args.async_mode, None);
}

#[test]
fn async_mode_flag_accepts_yes_no_aliases() {
    let args = args_from_cli(&["--async-mode", "yes"]);
    assert_eq!(args.async_mode, Some(true));
    let args = args_from_cli(&["--async-mode", "no"]);
    assert_eq!(args.async_mode, Some(false));
    let args = args_from_cli(&["--async-mode", "1"]);
    assert_eq!(args.async_mode, Some(true));
    let args = args_from_cli(&["--async-mode", "0"]);
    assert_eq!(args.async_mode, Some(false));
}

#[test]
fn async_mode_flag_rejects_invalid_value() {
    // parse_true_false rejects non-boolean values.
    assert!(crate::config::test_parse_true_false("maybe").is_err());
}

#[test]
fn uniform_flag_removed_and_reports_migration_error() {
    // --uniform was removed in v50-beta.3. It should surface in
    // REMOVED_FLAGS with a migration message pointing to --async-mode false.
    // We verify the REMOVED_FLAGS list contains "--uniform".
    let removed: Vec<&str> = crate::validation::REMOVED_FLAGS
        .iter()
        .map(|(f, _)| *f)
        .collect();
    assert!(
        removed.contains(&"--uniform"),
        "--uniform must be in REMOVED_FLAGS: {:?}",
        removed
    );
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
    assert_eq!(args.color, "energy-zen");
    assert_eq!(args.fps, 60.0);
    // Default scene is cinematic, which sets speed=9.0.
    assert_eq!(args.speed, 9.0);
    // v14.0.0: invalid `scene = unknown` does not set scene; default cinematic applies.
    assert_eq!(args.scene.as_deref(), Some("cinematic"));
}

#[test]
fn legacy_keys_no_longer_apply_v17() {
    // v17 mastery: legacy advanced keys (glitchpct, shortpct, rippct, maxdpc)
    // are REMOVED. In production (without COSMOSTRIX_SKIP_STARTUP_VALIDATION)
    // they are rejected as unknown keys; this test bypasses startup validation
    // to verify the apply path's defense-in-depth. Values come from
    // --glitch-level preset only. Default glitch_level is Subtle (from cinematic
    // scene default).
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
        "shading-mode",
        "color-bg",
        "crystal-dragon",
        "async-mode",
        "intro",
        // simplification: legacy/historical key mentions removed
        // from dump config (mouse flag was v17-deletion note, preset was
        // removal note). Both are gone in the simplified dump.
        // v50 simplification: `low-power` (a scene NAME, not a config key)
        // removed — the scene list is now "See: cosmostrix --list-scenes"
        // instead of inline. Scene names are not config keys.
    ] {
        assert!(dump.contains(key), "dump config should contain {key}");
    }
    // the standard example values are still present.
    // All string values are now quoted (standard TOML convention).
    assert!(dump.contains("glitch-level = \"subtle\""));
    assert!(dump.contains("scene = \"cinematic\""));
    assert!(dump.contains("speed = 9"));
    assert!(dump.contains("density = 0.75"));
}

// ── Atmosphere engine subsystem fully eliminated (Dragon Hunt v2 Phase 6
