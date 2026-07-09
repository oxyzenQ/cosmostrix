// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Color detection and canonicalization tests.
//!
//! Extracted from main.rs to keep that file under 1000 LOC.

use clap::{CommandFactory, FromArgMatches};

use crate::cli::detect_color_mode_from_terms;
use crate::config::Args;
use crate::config_apply::apply_config_and_runtime_defaults;
use crate::runtime::ColorMode;

fn args_from_empty_config(cli: &[&str]) -> Args {
    let mut path = std::env::temp_dir();
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
        .max(1);
    path.push(format!(
        "cosmostrix-main-color-test-{}-{unique}.toml",
        std::process::id(),
    ));
    std::fs::write(&path, "").expect("write temp config");

    let path_string = path.to_string_lossy().into_owned();
    let mut argv = vec!["cosmostrix", "--config", path_string.as_str()];
    argv.extend_from_slice(cli);

    let cmd = Args::command();
    let matches = cmd.get_matches_from(argv);
    let mut args = Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    apply_config_and_runtime_defaults(&matches, &mut args).expect("apply config");
    crate::canonicalize_runtime_args(&mut args);

    let _ = std::fs::remove_file(path);
    args
}

#[test]
fn runtime_profile_color_display_uses_canonical_alias_names() {
    for (alias, canonical) in [
        ("white", "snow"),
        ("silver", "gray"),
        ("deepblue", "deepspace"),
        ("deep-blue", "deepspace"),
        ("deep_blue", "deepspace"),
        ("grey", "gray"),
    ] {
        let args = args_from_empty_config(&["--color", alias, "-i"]);
        assert_eq!(args.color, canonical);
    }
}

#[test]
fn term_xterm_direct_detects_truecolor_without_colorterm() {
    assert_eq!(
        detect_color_mode_from_terms("", "xterm-direct"),
        ColorMode::TrueColor
    );
}

#[test]
fn term_tmux_direct_detects_truecolor_without_colorterm() {
    assert_eq!(
        detect_color_mode_from_terms("", "tmux-direct"),
        ColorMode::TrueColor
    );
}

#[test]
fn term_xterm_256color_preserves_256color_detection() {
    assert_eq!(
        detect_color_mode_from_terms("", "xterm-256color"),
        ColorMode::Color256
    );
}

#[test]
fn colorterm_truecolor_still_overrides_term() {
    assert_eq!(
        detect_color_mode_from_terms("truecolor", "xterm"),
        ColorMode::TrueColor
    );
}
