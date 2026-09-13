// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-hunt-37 custom-block header-completeness tests: header-only
//! blocks (charset without `set`, colors without bg/rain, scene with
//! every field commented out), name-shape checks on headers, and the
//! pass-through for complete blocks and flat (headerless) blocks.

use super::*;

fn headers(names: &[&str]) -> Vec<String> {
    names.iter().map(|s| s.to_string()).collect()
}

#[test]
fn empty_header_list_is_none() {
    let mut cfg = HashMap::new();
    cfg.insert("color".to_string(), "green".to_string());
    assert_eq!(validate_custom_block_headers(&[], &cfg), None);
    assert_eq!(validate_custom_block_headers(&headers(&[]), &cfg), None);
}

#[test]
fn charset_header_without_set_is_an_error() {
    // The owner's case: "[charset-custom.zen]" opened, set missing
    // (or commented out) — invisible to every key-level validator.
    let mut cfg = HashMap::new();
    let e = validate_custom_block_headers(&headers(&["charset-custom.zen"]), &cfg)
        .expect("charset block without set must error");
    assert!(
        e.contains("charset-custom 'zen'") && e.contains("'set' is missing"),
        "error must name the block and the missing field, got: {e}"
    );
    // Defining set satisfies the contract (value validity is the
    // value layer's job, not the header layer's).
    cfg.insert("charset-custom.zen.set".to_string(), "|".to_string());
    assert_eq!(
        validate_custom_block_headers(&headers(&["charset-custom.zen"]), &cfg),
        None
    );
}

#[test]
fn colors_header_missing_bg_or_rain_is_an_error() {
    // Header-only.
    let mut cfg = HashMap::new();
    let e = validate_custom_block_headers(&headers(&["colors-custom.test"]), &cfg)
        .expect("header-only colors block must error");
    assert!(
        e.contains("missing bg and rain"),
        "header-only block reports both missing fields, got: {e}"
    );
    // rain only (the owner's "missing one field" case).
    cfg.insert(
        "colors-custom.test.rain".to_string(),
        "#111111, #222222".to_string(),
    );
    let e = validate_custom_block_headers(&headers(&["colors-custom.test"]), &cfg)
        .expect("colors block without bg must error");
    assert!(e.contains("missing bg"), "got: {e}");
    // Complete: bg + rain passes.
    cfg.insert("colors-custom.test.bg".to_string(), "#0a0a0a".to_string());
    assert_eq!(
        validate_custom_block_headers(&headers(&["colors-custom.test"]), &cfg),
        None
    );
    // The deprecated `stops` alias satisfies the rain slot.
    let mut legacy = HashMap::new();
    legacy.insert(
        "colors-custom.old.stops".to_string(),
        "#111111, #222222".to_string(),
    );
    legacy.insert("colors-custom.old.bg".to_string(), "#0a0a0a".to_string());
    assert_eq!(
        validate_custom_block_headers(&headers(&["colors-custom.old"]), &legacy),
        None
    );
}

#[test]
fn scene_header_only_reports_all_seven_dimensions() {
    let cfg = HashMap::new();
    let e = validate_custom_block_headers(&headers(&["scene-custom.hacker"]), &cfg)
        .expect("header-only scene block must error");
    assert!(
        e.contains("scene-custom 'hacker' is incomplete")
            && e.contains("rain, color|colors-custom, charset|charset-custom, fps, speed, density, glitch-level"),
        "error must name the block and all seven dimensions, got: {e}"
    );
    // Once the block has at least one key, the scene_custom
    // completeness validator owns the per-field reporting — the
    // header layer must NOT fire.
    let mut cfg = HashMap::new();
    cfg.insert("scene-custom.hacker.rain".to_string(), "glyph".to_string());
    assert_eq!(
        validate_custom_block_headers(&headers(&["scene-custom.hacker"]), &cfg),
        None,
        "scene header with at least one key is the completeness validator's case"
    );
}

#[test]
fn bare_namespace_header_is_malformed() {
    let cfg = HashMap::new();
    let e = validate_custom_block_headers(&headers(&["colors-custom"]), &cfg)
        .expect("bare [colors-custom] header must error");
    assert!(
        e.contains("malformed custom block header '[colors-custom]'")
            && e.contains("block name is missing"),
        "got: {e}"
    );
}

#[test]
fn header_only_oversized_name_is_an_error() {
    // A 65+-char name on a header-ONLY block was invisible to the
    // key-scanning name-length validators (no keys, no scan hit).
    let long_name = "x".repeat(COLORS_CUSTOM_MAX_NAME_LEN + 1);
    let header = format!("colors-custom.{long_name}");
    let cfg = HashMap::new();
    let e = validate_custom_block_headers(&headers(&[&header]), &cfg)
        .expect("oversized header-only name must error");
    assert!(
        e.contains("over the 64-char limit"),
        "error must state the limit, got: {e}"
    );
}

#[test]
fn header_name_with_invalid_characters_is_an_error() {
    let cfg = HashMap::new();
    let e = validate_custom_block_headers(&headers(&["charset-custom.we ird!"]), &cfg)
        .expect("invalid header name must error");
    assert!(e.contains("invalid custom block name"), "got: {e}");
}

#[test]
fn flat_block_without_header_is_not_this_validators_case() {
    // Keys written flat at root (no [section] header) are complete
    // without a header — the key-level validators own them.
    let mut cfg = HashMap::new();
    cfg.insert(
        "colors-custom.flat.rain".to_string(),
        "#111111, #222222".to_string(),
    );
    cfg.insert("colors-custom.flat.bg".to_string(), "#0a0a0a".to_string());
    assert_eq!(validate_custom_block_headers(&[], &cfg), None);
}

#[test]
fn multiple_headers_report_the_first_sorted_block() {
    // Headers arrive sorted; the first violating block (alphabetical
    // by full header string) is the one reported — deterministic
    // across hash seeds.
    let cfg = HashMap::new();
    // The parser delivers headers SORTED (custom_block_headers_from);
    // the validator relies on that order for its first-error contract.
    let e = validate_custom_block_headers(
        &headers(&["charset-custom.alpha", "charset-custom.zulu"]),
        &cfg,
    )
    .expect("both header-only blocks violate the contract");
    assert!(
        e.contains("charset-custom 'alpha'"),
        "sorted order means alpha is reported first, got: {e}"
    );
}

#[test]
fn full_parse_detects_header_only_blocks_end_to_end() {
    // Parser integration: a config text with a header-only charset
    // block produces the header record (this is the exact wiring the
    // three surfaces rely on).
    let parsed = crate::configfile::parse_config_text(
        "[charset-custom.zen]\n# set = \"|\" (commented out)\n\n[colors-custom.ok]\nbg = \"#0a0a0a\"\nrain = \"#111111, #222222\"\n",
    );
    assert_eq!(
        parsed.custom_block_headers,
        vec![
            "charset-custom.zen".to_string(),
            "colors-custom.ok".to_string()
        ],
        "headers are recorded sorted, deduplicated"
    );
    let e = validate_custom_block_headers(&parsed.custom_block_headers, &parsed.values)
        .expect("the header-only zen block must be rejected");
    assert!(e.contains("'set' is missing"), "got: {e}");
}
