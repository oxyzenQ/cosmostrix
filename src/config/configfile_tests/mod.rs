// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Auto-promote forgiving parser tests.
//!
//! When the user writes a top-level key (e.g. `intro`) AFTER
//! a `[scene-custom.<name>]` table header, TOML parsing rules nest it under
//! the table (`scene-custom.<name>.intro`). The  parser
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

mod bug19;
mod bug7;

use crate::configfile::parse_config_text;

#[test]
fn scene_custom_then_top_level_key_promotes_to_root() {
    // v50.0.0-beta.6: top-level keys accidentally nested under a
    // [scene-custom.<name>] block are NO LONGER auto-promoted — they
    // surface as unknown_keys so the user gets a clear error. The user
    // must move top-level keys OUT of the [scene-custom] section (add
    // a blank line before them, or put them at the top of the file).
    // This prevents silent side-effects like `color = green` inside
    // `[charset-custom.quantum]` changing the global color scheme.
    let content = "\
[scene-custom.hacker-mode]
color = green
speed = 28

intro = cosmic
";
    let parsed = parse_config_text(content);
    // `intro` is NOT a valid scene-custom field → unknown_key (no promote).
    assert!(
        parsed
            .unknown_keys
            .contains(&"scene-custom.hacker-mode.intro".to_string()),
        "expected unknown key for intro, got: {:?}",
        parsed.unknown_keys
    );
    // The scene-custom block's valid fields are correctly stored.
    assert_eq!(
        parsed
            .values
            .get("scene-custom.hacker-mode.color")
            .map(String::as_str),
        Some("green")
    );
    assert!(parsed.promoted_keys.is_empty(), "no promotion expected");
}

#[test]
fn scene_custom_then_colors_custom_flat_keys_promote() {
    // v50.0.0-beta.6: flat colors-custom keys nested under a
    // [scene-custom] block are NO LONGER promoted — they surface as
    // unknown_keys. The user must write [colors-custom.<name>] as its
    // own section header to reset scope.
    let content = "\
[scene-custom.hacker-mode]
color = green
colors-custom.mythme.bg = \"#0a0a12\"
colors-custom.mythme.rain = \"#1a0033, #4d0080\"
";
    let parsed = parse_config_text(content);
    // Both flat keys are unknown (nested under scene-custom, not promoted).
    assert!(
        parsed
            .unknown_keys
            .iter()
            .any(|k| k == "scene-custom.hacker-mode.colors-custom.mythme.bg"),
        "expected unknown key for nested colors-custom, got: {:?}",
        parsed.unknown_keys
    );
    assert!(parsed.promoted_keys.is_empty(), "no promotion expected");
}

#[test]
fn scene_custom_then_charset_custom_flat_key_promotes() {
    // v50.0.0-beta.6: flat charset-custom keys nested under a
    // [scene-custom] block are NO LONGER promoted — they surface as
    // unknown_keys. The user must write [charset-custom.<name>] as its
    // own section header.
    let content = "\
[scene-custom.hacker-mode]
color = green
charset-custom.zen.set = \"|\"
";
    let parsed = parse_config_text(content);
    // Flat key is unknown (nested under scene-custom, not promoted).
    assert!(
        parsed
            .unknown_keys
            .iter()
            .any(|k| k == "scene-custom.hacker-mode.charset-custom.zen.set"),
        "expected unknown key for nested charset-custom, got: {:?}",
        parsed.unknown_keys
    );
    assert!(parsed.promoted_keys.is_empty(), "no promotion expected");
}

#[test]
fn color_tune_then_top_level_key_promotes() {
    // Bug #4 from  depth test: user writes [color.tune] then `bold = 1`.
    // Pre- this errored: promote to root scope `bold = 1`.
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
    // v50.0.0-beta.6: `[]` is rejected as malformed (empty section name)
    // and does NOT reset current_section. A top-level key written after
    // `[]` is still nested under the previous [scene-custom] block —
    // but now it surfaces as unknown_key (no auto-promote inside custom
    // blocks). The user must remove the `[]` line and add a blank line
    // before the top-level key.
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
    // intro is unknown (nested under scene-custom, not promoted).
    assert!(
        parsed
            .unknown_keys
            .iter()
            .any(|k| k == "scene-custom.hacker-mode.intro"),
        "expected unknown key for intro, got: {:?}",
        parsed.unknown_keys
    );
    assert!(parsed.promoted_keys.is_empty());
}

#[test]
fn multiple_top_level_keys_all_promote() {
    // v50.0.0-beta.6: top-level keys nested under a [scene-custom] block
    // are NO LONGER promoted — they all surface as unknown_keys. The user
    // must move them out of the [scene-custom] section.
    let content = "\
[scene-custom.hacker-mode]
color = green
speed = 28

intro = cosmic
crystal-dragon = on
color-bg = black
monolith-size = large
";
    let parsed = parse_config_text(content);
    // All 4 nested top-level keys are unknown (not promoted).
    for key in &[
        "scene-custom.hacker-mode.intro",
        "scene-custom.hacker-mode.crystal-dragon",
        "scene-custom.hacker-mode.color-bg",
        "scene-custom.hacker-mode.monolith-size",
    ] {
        assert!(
            parsed.unknown_keys.iter().any(|k| k == *key),
            "expected unknown key {key}, got: {:?}",
            parsed.unknown_keys
        );
    }
    assert!(parsed.promoted_keys.is_empty(), "no promotion expected");
}

#[test]
fn promoted_keys_record_original_and_target() {
    // v50.0.0-beta.6: top-level keys inside [scene-custom] blocks are
    // no longer promoted — they surface as unknown_keys. This test now
    // verifies the NON-custom-block promotion path still works: a
    // top-level key nested under a NON-custom section (e.g. [color.tune])
    // still promotes, since [color.tune] is not a custom block.
    let content = "\
[color.tune]
brightness = 1.0
intro = cosmic
";
    let parsed = parse_config_text(content);
    // brightness stays under color.tune (correctly recognized field).
    assert_eq!(
        parsed
            .values
            .get("color.tune.brightness")
            .map(String::as_str),
        Some("1.0")
    );
    // intro is a top-level key nested under [color.tune] (not a custom
    // block) → still promoted to root scope.
    assert_eq!(parsed.promoted_keys.len(), 1);
    let (from, to) = &parsed.promoted_keys[0];
    assert_eq!(from, "color.tune.intro");
    assert_eq!(to, "intro");
}

// ── v50.0.0-beta.6 FATAL FIX: no auto-promote inside custom blocks ──

#[test]
fn charset_custom_block_rejects_unknown_field_color() {
    // Owner-reported FATAL bug: `color = green` inside `[charset-custom.quantum]`
    // was auto-promoted to root `color = green`, silently changing the global
    // color scheme. Now it surfaces as unknown_key so the user gets a clear
    // error. `color` is NOT a valid charset-custom field (only `set` is).
    let content = "\
[charset-custom.quantum]
set = \"abcdef\"
color = green
";
    let parsed = parse_config_text(content);
    // `color` must be unknown (not promoted to root).
    assert!(
        parsed
            .unknown_keys
            .iter()
            .any(|k| k == "charset-custom.quantum.color"),
        "expected unknown key for color in charset-custom, got: {:?}",
        parsed.unknown_keys
    );
    // `color` must NOT be at root scope.
    assert!(
        !parsed.values.contains_key("color"),
        "color must NOT be promoted to root scope"
    );
    // `set` is valid → stored correctly.
    assert_eq!(
        parsed
            .values
            .get("charset-custom.quantum.set")
            .map(String::as_str),
        Some("abcdef")
    );
    assert!(parsed.promoted_keys.is_empty());
}

#[test]
fn colors_custom_block_rejects_unknown_field_speed() {
    // Same bug class: `speed` inside `[colors-custom.sun]` must NOT
    // promote to root. `speed` is not a valid colors-custom field
    // (only `bg`, `rain`, `stops` are).
    let content = "\
[colors-custom.sun]
rain = \"#000000, #ffffff\"
speed = 28
";
    let parsed = parse_config_text(content);
    assert!(
        parsed
            .unknown_keys
            .iter()
            .any(|k| k == "colors-custom.sun.speed"),
        "expected unknown key for speed in colors-custom, got: {:?}",
        parsed.unknown_keys
    );
    assert!(
        !parsed.values.contains_key("speed"),
        "speed must NOT be promoted to root scope"
    );
    assert!(parsed.promoted_keys.is_empty());
}

#[test]
fn scene_custom_block_rejects_unknown_field_intro() {
    // `intro` inside `[scene-custom.hacker-mode]` must NOT promote to
    // root. `intro` is not a valid scene-custom field.
    let content = "\
[scene-custom.hacker-mode]
color = green
intro = cosmic
";
    let parsed = parse_config_text(content);
    assert!(
        parsed
            .unknown_keys
            .iter()
            .any(|k| k == "scene-custom.hacker-mode.intro"),
        "expected unknown key for intro in scene-custom, got: {:?}",
        parsed.unknown_keys
    );
    assert!(
        !parsed.values.contains_key("intro"),
        "intro must NOT be promoted to root scope"
    );
    assert!(parsed.promoted_keys.is_empty());
}
