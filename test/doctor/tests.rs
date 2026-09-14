// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Doctor module tests, extracted from inline `mod tests { ... }` block
//! in doctor.rs (Pattern D → Pattern C unification).
//!
//! Uses `use super::*;` to access doctor.rs's private items unchanged.

use super::*;

#[test]
fn terminal_family_detects_common_terms() {
    assert_eq!(terminal_family("xterm-direct"), "xterm-direct");
    assert_eq!(terminal_family("xterm-256color"), "xterm-256color");
    assert_eq!(terminal_family("tmux-256color"), "tmux");
    assert_eq!(terminal_family("screen-256color"), "screen");
    assert_eq!(terminal_family("dumb"), "dumb/unknown");
}

#[test]
fn doctor_guidance_distinguishes_truecolor_and_256_color() {
    assert_eq!(color_capability(ColorMode::TrueColor), "truecolor");
    assert_eq!(color_capability(ColorMode::Color256), "256-color");
    assert!(should_advise_truecolor(
        "xterm-256color",
        "",
        ColorMode::Color256
    ));
    assert!(!should_advise_truecolor(
        "xterm-direct",
        "",
        ColorMode::TrueColor
    ));
}

#[test]
fn doctor_background_guidance_mentions_modes() {
    assert_eq!(
        background_guidance(ColorBg::Black),
        "black paints solid black"
    );
    assert_eq!(
        background_guidance(ColorBg::DefaultBackground),
        "default-background uses terminal default background"
    );
}

#[test]
fn doctor_environment_hints_are_actionable() {
    let hints = environment_hints("tmux-256color", "", false, true, true, true);
    assert!(hints.contains(&"tmux detected"));
    assert!(hints.contains(&"ssh detected"));
    assert!(hints.contains(&"headless/non-TTY detected"));
    assert!(hints.contains(&"COLORTERM missing"));
    assert!(hints.contains(&"locale not UTF-8"));
}

// NIGHT-hunt-47-depthbore: pin the CONFIG FILE status classification.
// The doctor report must never again print "healthy" while the user's
// default config is present but unreadable (invalid UTF-8 / past the
// 1 MiB cap) — the runtime silently runs defaults in that case, and
// the report is the only surface that can say so without changing
// exit codes.

fn cfg_field(fields: &[(String, String)], name: &str) -> String {
    fields
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

fn cfg_tmp(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "cosmostrix-doctor-cfg-{name}-{}",
        std::process::id()
    ))
}

#[test]
fn config_file_status_readable_reports_path_and_bytes() {
    let p = cfg_tmp("readable.toml");
    std::fs::write(&p, b"fps = 60\n").unwrap();
    let fields = config_file_status(Some(&p), Path::new("/nonexistent-default"));
    assert_eq!(cfg_field(&fields, "path"), p.display().to_string());
    let status = cfg_field(&fields, "status");
    assert!(status.starts_with("readable ("), "got: {status}");
    assert!(status.ends_with("bytes)"), "got: {status}");
    // A readable file needs no fallback or hint fields.
    assert_eq!(cfg_field(&fields, "effective"), "");
    assert_eq!(cfg_field(&fields, "hint"), "");
    let _ = std::fs::remove_file(&p);
}

#[test]
fn config_file_status_missing_default_is_first_run() {
    let fields = config_file_status(None, Path::new("/nonexistent-default/config.toml"));
    assert_eq!(cfg_field(&fields, "status"), "missing (first run)");
    // Presence only: a machine with /etc/cosmostrix installed legitimately
    // reports the system-wide source instead of built-in defaults.
    assert!(!cfg_field(&fields, "effective").is_empty());
}

#[test]
fn config_file_status_missing_explicit_names_the_error() {
    let fields = config_file_status(
        Some(Path::new("/nonexistent-explicit.toml")),
        Path::new("/nonexistent-default"),
    );
    let status = cfg_field(&fields, "status");
    assert!(status.starts_with("missing: "), "got: {status}");
}

#[test]
fn config_file_status_invalid_utf8_is_unreadable_with_hint() {
    let p = cfg_tmp("corrupt.toml");
    std::fs::write(&p, b"\xff\xfe garbage\xff").unwrap();
    let fields = config_file_status(None, &p);
    let status = cfg_field(&fields, "status");
    assert!(status.starts_with("unreadable: "), "got: {status}");
    assert!(status.contains("UTF-8"), "got: {status}");
    assert!(!cfg_field(&fields, "effective").is_empty());
    assert!(
        cfg_field(&fields, "hint").contains("--testconf"),
        "hint must point at testconf"
    );
    let _ = std::fs::remove_file(&p);
}

#[test]
fn config_file_status_oversize_reports_the_cap() {
    let p = cfg_tmp("oversize.toml");
    let oversize = crate::constants::CONFIG_FILE_MAX_BYTES as usize + 1024;
    std::fs::write(&p, vec![b'#'; oversize]).unwrap();
    let fields = config_file_status(None, &p);
    let status = cfg_field(&fields, "status");
    assert!(status.starts_with("unreadable: "), "got: {status}");
    assert!(
        status.contains("exceeds"),
        "the cap reason must be named, got: {status}"
    );
    assert!(cfg_field(&fields, "hint").contains("--testconf"));
    let _ = std::fs::remove_file(&p);
}
