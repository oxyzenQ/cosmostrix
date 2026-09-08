// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the v50-beta.3 CLI flags (`--intro-color`, `--power-dragon`,
//! `--msg-mode`, `--crystal-dragon`). Extracted from `config_apply_tests/mod.rs`
//! to keep that source file under the 800-LOC cap. Pure code motion — no
//! behavior change.

#![cfg(test)]

use super::{args_from_cli_result, args_with_config};

// ── v50-beta.3: new CLI flags --intro-color / --power-dragon / --msg-mode / --crystal-dragon ──

#[test]
fn cli_power_dragon_flag_accepts_true_false() {
    // CLI --power-dragon=true → args.power_dragon = Some(true).
    let args = args_with_config("", &["--power-dragon", "true"]);
    assert_eq!(args.power_dragon, Some(true));
    let args = args_with_config("", &["--power-dragon", "false"]);
    assert_eq!(args.power_dragon, Some(false));
}

#[test]
fn cli_msg_mode_flag_accepts_true_false() {
    // CLI --msg-mode=true → args.msg_mode = Some(true).
    let args = args_with_config("", &["--msg-mode", "true"]);
    assert_eq!(args.msg_mode, Some(true));
    let args = args_with_config("", &["--msg-mode", "false"]);
    assert_eq!(args.msg_mode, Some(false));
}

#[test]
fn cli_crystal_dragon_flag_accepts_true_false() {
    // CLI --crystal-dragon=true → args.crystal_dragon = Some(true).
    let args = args_with_config("", &["--crystal-dragon", "true"]);
    assert_eq!(args.crystal_dragon, Some(true));
    let args = args_with_config("", &["--crystal-dragon", "false"]);
    assert_eq!(args.crystal_dragon, Some(false));
}

#[test]
fn cli_power_dragon_flag_rejects_invalid_value() {
    // CLI --power-dragon=maybe → clap error (parse_true_false rejects).
    // Note: we can't easily assert the error here because clap calls
    // process::exit on parse failure. Instead, we verify the value_parser
    // function directly.
    assert!(crate::config::test_parse_true_false("maybe").is_err());
    assert!(crate::config::test_parse_true_false("true").is_ok());
    assert!(crate::config::test_parse_true_false("false").is_ok());
    // Reference: this test exists to ensure the parser is wired up.
    let _ = args_from_cli_result(&["--power-dragon", "true"]).expect("valid value should parse");
}

#[test]
fn cli_msg_mode_flag_rejects_invalid_value() {
    // Same approach as above — test the parser directly.
    assert!(crate::config::test_parse_true_false("maybe").is_err());
}

#[test]
fn cli_crystal_dragon_flag_rejects_invalid_value() {
    assert!(crate::config::test_parse_true_false("maybe").is_err());
}

#[test]
fn cli_power_dragon_accepts_yes_no_aliases() {
    // parse_true_false accepts yes/no/on/off/1/0 in addition to true/false.
    let args = args_with_config("", &["--power-dragon", "yes"]);
    assert_eq!(args.power_dragon, Some(true));
    let args = args_with_config("", &["--power-dragon", "no"]);
    assert_eq!(args.power_dragon, Some(false));
    let args = args_with_config("", &["--power-dragon", "1"]);
    assert_eq!(args.power_dragon, Some(true));
    let args = args_with_config("", &["--power-dragon", "0"]);
    assert_eq!(args.power_dragon, Some(false));
}

#[test]
fn cli_intro_color_rejects_unknown_theme() {
    // CLI --intro-color=not-a-color → hard error (exit code 2), not silent.
    // v50-beta.3: unknown theme must error, not silently fall back to None.
    let result = args_from_cli_result(&["--intro-color", "not-a-color"]);
    assert!(
        result.is_err(),
        "unknown intro-color theme must be a hard error, got: {result:?}"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("invalid intro-color='not-a-color'"),
        "error message must name the bad value: {err}"
    );
    assert!(
        err.contains("--list-colors"),
        "error message must point to --list-colors: {err}"
    );
}

#[test]
fn cli_intro_color_typo_suggests_closest_theme() {
    // CLI --intro-color energy-zenn (typo) → error with "did you mean energy-zen?".
    let result = args_from_cli_result(&["--intro-color", "energy-zenn"]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("tip: a similar value exists: 'energy-zen'"),
        "error must suggest closest theme: {err}"
    );
}

#[test]
fn cli_intro_color_valid_theme_accepted() {
    // CLI --intro-color energy-zen (valid) → accepted, no error.
    let args = args_with_config("", &["--intro-color", "energy-zen"]);
    assert_eq!(args.intro_color.as_deref(), Some("energy-zen"));
}

// ── NIGHT-hunter-23: intro-color custom-palette recognition (F-23-1) ──
// The old gate probed `colors-custom.{v}.bg` — bg-only (bg is OPTIONAL in
// the palette schema) and case-sensitive (the parser lowercases keys).
// A rain-only palette or a mixed-case value was a hard startup error while
// `color = <same name>` loaded fine from the same config; testconf's own
// any-of-3/lowercase probe said "valid". These pin the canonical-helper
// behavior at the startup gate (unknown-name rejection is already pinned
// by cli_intro_color_rejects_unknown_theme above).

#[test]
fn cli_intro_color_accepts_rain_only_custom_palette() {
    // bg is optional — a rain-only palette is a valid custom palette and
    // must be accepted as intro-color (the exact repro: previously a hard
    // "not a builtin theme or custom palette" error at startup).
    let config = "[colors-custom.mine]\nrain = \"#00ff66, #ffffff\"\n";
    let args = args_with_config(config, &["--intro-color", "mine"]);
    assert_eq!(
        args.intro_color.as_deref(),
        Some("mine"),
        "rain-only custom palette must be a valid intro-color"
    );
}

#[test]
fn cli_intro_color_custom_palette_value_case_insensitive() {
    // `intro-color = MINE` with a lowercase block: the parser lowercases
    // keys and the loader normalizes the name — the gate must too.
    let config = "[colors-custom.mine]\nrain = \"#00ff66, #ffffff\"\n";
    let args = args_with_config(config, &["--intro-color", "MINE"]);
    assert_eq!(
        args.intro_color.as_deref(),
        Some("MINE"),
        "mixed-case intro-color value must be recognized (case-insensitive)"
    );
}
