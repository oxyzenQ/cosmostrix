// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! v25.7: Auto-promote forgiving parser tests.
//!
//! When the user writes a top-level key (e.g. `intro`) AFTER
//! a `[scene-custom.<name>]` table header, TOML parsing rules nest it under
//! the table (`scene-custom.<name>.intro`). The v25.7 parser
//! detects this mis-nesting when the un-prefixed key is itself a known
//! top-level key, and silently re-homes it to root scope so scene-custom and
//! top-level keys coexist without forcing the user to learn TOML scope rules.
//!
//! These tests verify the promotion behavior end-to-end via `parse_config_text`.
//!
//! Aggregated umbrella module: also groups `bug7.rs` regression tests so
//! `main.rs` declares only one `mod configfile_tests;` instead of two flat
//! modules.

#![cfg(test)]

mod bug7;

use crate::configfile::parse_config_text;

#[test]
fn scene_custom_then_top_level_key_promotes_to_root() {
    // The exact scenario reported in the v25.6 depth test: user uncomments
    // [scene-custom.hacker-mode] and a top-level key in the same file.
    // Pre-v25.7 this errored with "unknown key: scene-custom.hacker-mode.intro".
    // v25.7: auto-promote to root scope, no error.
    let content = "\
[scene-custom.hacker-mode]
color = green
speed = 28

intro = cosmic
";
    let parsed = parse_config_text(content);
    assert!(
        parsed.unknown_keys.is_empty(),
        "expected no unknown keys, got: {:?}",
        parsed.unknown_keys
    );
    assert!(
        parsed.malformed_lines.is_empty(),
        "expected no malformed lines, got: {:?}",
        parsed.malformed_lines
    );
    // The scene-custom block is correctly stored under its nested key.
    assert_eq!(
        parsed
            .values
            .get("scene-custom.hacker-mode.color")
            .map(String::as_str),
        Some("green")
    );
    // The top-level key was promoted to root scope.
    assert_eq!(
        parsed.values.get("intro").map(String::as_str),
        Some("cosmic")
    );
    // Promotion was recorded for --testconf transparency.
    assert_eq!(
        parsed.promoted_keys,
        vec![(
            "scene-custom.hacker-mode.intro".to_string(),
            "intro".to_string()
        )]
    );
}

#[test]
fn scene_custom_then_colors_custom_flat_keys_promote() {
    // FLAT form (no [colors-custom.<name>] header) written after a
    // [scene-custom.<name>] block — these get nested under scene-custom
    // and need promotion. (If the user writes [colors-custom.<name>] as a
    // new section header, scope is correctly reset — no promotion needed.)
    let content = "\
[scene-custom.hacker-mode]
color = green
colors-custom.mythme.bg = \"#0a0a12\"
colors-custom.mythme.rain = \"#1a0033, #4d0080\"
";
    let parsed = parse_config_text(content);
    assert!(
        parsed.unknown_keys.is_empty(),
        "unknown keys: {:?}",
        parsed.unknown_keys
    );
    // Both flat keys were promoted to root scope.
    assert!(parsed.values.contains_key("colors-custom.mythme.bg"));
    assert!(parsed.values.contains_key("colors-custom.mythme.rain"));
    assert_eq!(parsed.promoted_keys.len(), 2);
    assert!(parsed
        .promoted_keys
        .iter()
        .any(|(from, _)| from == "scene-custom.hacker-mode.colors-custom.mythme.bg"));
}

#[test]
fn scene_custom_then_charset_custom_flat_key_promotes() {
    // FLAT form `charset-custom.<name>.set = "..."` written after a
    // [scene-custom.<name>] block — promoted to root scope.
    let content = "\
[scene-custom.hacker-mode]
color = green
charset-custom.zen.set = \"|\"
";
    let parsed = parse_config_text(content);
    assert!(parsed.unknown_keys.is_empty());
    assert_eq!(
        parsed
            .values
            .get("charset-custom.zen.set")
            .map(String::as_str),
        Some("\"|\"")
    );
    assert_eq!(parsed.promoted_keys.len(), 1);
}

#[test]
fn color_tune_then_top_level_key_promotes() {
    // Bug #4 from v25.6 depth test: user writes [color.tune] then `bold = 1`.
    // Pre-v25.7 this errored. v25.7: promote to root scope `bold = 1`.
    let content = "\
[color.tune]
brightness = 1.0
bold = 1
";
    let parsed = parse_config_text(content);
    assert!(
        parsed.unknown_keys.is_empty(),
        "unknown keys: {:?}",
        parsed.unknown_keys
    );
    // brightness stays under color.tune (correctly recognized field).
    assert_eq!(
        parsed
            .values
            .get("color.tune.brightness")
            .map(String::as_str),
        Some("1.0")
    );
    // bold was promoted to root scope.
    assert_eq!(parsed.values.get("bold").map(String::as_str), Some("1"));
    assert_eq!(
        parsed.promoted_keys,
        vec![("color.tune.bold".to_string(), "bold".to_string())]
    );
}

#[test]
fn root_scope_value_wins_over_promoted_duplicate() {
    // If the user explicitly wrote `color = green` at root scope and then
    // later has `color = red` nested under a [section], the root value
    // wins (first writer wins, matching TOML duplicate-key semantics).
    let content = "\
color = green

[scene-custom.hacker-mode]
color = red
";
    let parsed = parse_config_text(content);
    // scene-custom.hacker-mode.color is a known key — it gets stored as-is.
    assert_eq!(
        parsed
            .values
            .get("scene-custom.hacker-mode.color")
            .map(String::as_str),
        Some("red")
    );
    // Root `color` is also stored (it's a known key at root scope).
    assert_eq!(
        parsed.values.get("color").map(String::as_str),
        Some("green")
    );
    // No promotion needed — both keys are recognized in their own scope.
    assert!(parsed.promoted_keys.is_empty());
}

#[test]
fn genuine_typo_under_section_still_errors() {
    // A typo like `colro = green` under [scene-custom.hacker-mode] should
    // STILL be flagged as unknown — auto-promote only fires when the
    // un-prefixed key is itself a known key. `colro` is not.
    let content = "\
[scene-custom.hacker-mode]
colro = green
";
    let parsed = parse_config_text(content);
    assert_eq!(
        parsed.unknown_keys,
        vec!["scene-custom.hacker-mode.colro".to_string()]
    );
    assert!(parsed.promoted_keys.is_empty());
}

#[test]
fn empty_section_header_is_malformed_and_promotion_still_fires() {
    // `[]` is rejected by the parser as malformed (empty section name) —
    // it does NOT reset `current_section` to root scope. So a flat
    // top-level key written after `[]` still gets nested
    // under the previous [scene-custom.<name>] block and needs promotion.
    // This is exactly the v25.6 depth-test scenario.
    let content = "\
[scene-custom.hacker-mode]
color = green
[]
intro = cosmic
";
    let parsed = parse_config_text(content);
    assert!(
        parsed.malformed_lines.iter().any(|l| l == "[]"),
        "expected [] to be malformed, got: {:?}",
        parsed.malformed_lines
    );
    // intro was still nested under scene-custom.hacker-mode
    // (because [] didn't reset scope), so promotion fires.
    assert!(
        parsed.promoted_keys.iter().any(|(_, to)| to == "intro"),
        "expected promotion to intro, got: {:?}",
        parsed.promoted_keys
    );
    assert!(parsed.unknown_keys.is_empty());
}

#[test]
fn multiple_top_level_keys_all_promote() {
    // A realistic config with top-level keys, all written after a scene-custom
    // block. All should be promoted (they are NOT valid scene-custom fields).
    //
    // v30.3: `bold`, `shadingmode`, `async` ARE now valid scene-custom fields
    // per owner contract — so they no longer get promoted when written under
    // a `[scene-custom.*]` block. This test now uses fields that remain
    // FORBIDDEN in scene-custom (`intro`, `auto-color-drift`, `color-bg`,
    // `monolith-size`) to verify the promotion path still works.
    let content = "\
[scene-custom.hacker-mode]
color = green
speed = 28

intro = cosmic
auto-color-drift = on
color-bg = black
monolith-size = large
";
    let parsed = parse_config_text(content);
    assert!(
        parsed.unknown_keys.is_empty(),
        "unknown keys: {:?}",
        parsed.unknown_keys
    );
    assert_eq!(parsed.promoted_keys.len(), 4);
    // All 4 root-scope keys are stored.
    for key in &["intro", "auto-color-drift", "color-bg", "monolith-size"] {
        assert!(
            parsed.values.contains_key(*key),
            "expected promoted key {key} in values"
        );
    }
}

#[test]
fn promoted_keys_record_original_and_target() {
    // The promoted_keys tuple format is (original_nested, promoted_root).
    // --testconf uses this to show the user what was moved.
    let content = "\
[scene-custom.hacker-mode]
intro = cosmic
";
    let parsed = parse_config_text(content);
    assert_eq!(parsed.promoted_keys.len(), 1);
    let (from, to) = &parsed.promoted_keys[0];
    assert_eq!(from, "scene-custom.hacker-mode.intro");
    assert_eq!(to, "intro");
}
