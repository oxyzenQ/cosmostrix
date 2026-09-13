// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-hunt-37 parser tests: `custom_block_headers` recording.
//!
//! The values map carries no trace of a header-only custom block, so
//! the parser records every custom-block section header for the
//! completeness layer. These tests pin the recording contract:
//! namespace matching, dedup, sorting, case folding, duplicate
//! headers, and the extraction helper.

use crate::configfile::parse_config_text;
use crate::configfile::{is_custom_block_section, ParsedConfig};

#[test]
fn custom_block_headers_are_recorded_sorted_and_deduped() {
    let parsed = parse_config_text(
        "[colors-custom.beta]\nbg = \"#0a0a0a\"\nrain = \"#111111, #222222\"\n\n[charset-custom.alpha]\nset = \"AB\"\n\n[scene-custom.gamma]\nrain = \"glyph\"\n",
    );
    assert_eq!(
        parsed.custom_block_headers,
        vec![
            "charset-custom.alpha".to_string(),
            "colors-custom.beta".to_string(),
            "scene-custom.gamma".to_string(),
        ],
        "headers sorted by full string, not source order"
    );
}

#[test]
fn header_only_blocks_are_recorded() {
    // The blind spot this closes: a block whose fields are all
    // commented out leaves zero keys — the header record is the only
    // trace the validation layer gets.
    let parsed = parse_config_text("[charset-custom.zen]\n# set = \"|\"\n");
    assert_eq!(
        parsed.custom_block_headers,
        vec!["charset-custom.zen".to_string()]
    );
    assert!(parsed.values.is_empty());
}

#[test]
fn duplicate_headers_appear_once() {
    // The duplicate itself lands in duplicate_sections (the
    // NIGHT-depthtest-2 contract); the header record stays deduped.
    let parsed = parse_config_text(
        "[colors-custom.a]\nbg = \"#000000\"\n\n[colors-custom.a]\nrain = \"#111111, #222222\"\n",
    );
    assert_eq!(
        parsed.custom_block_headers,
        vec!["colors-custom.a".to_string()]
    );
    assert_eq!(
        parsed.duplicate_sections,
        vec!["colors-custom.a".to_string()]
    );
}

#[test]
fn headers_are_case_folded() {
    let parsed = parse_config_text("[COLORS-CUSTOM.Test]\nbg = \"#0a0a0a\"\n");
    assert_eq!(
        parsed.custom_block_headers,
        vec!["colors-custom.test".to_string()]
    );
}

#[test]
fn bare_namespace_headers_are_recorded() {
    let parsed = parse_config_text("[colors-custom]\n");
    assert_eq!(
        parsed.custom_block_headers,
        vec!["colors-custom".to_string()]
    );
}

#[test]
fn non_custom_sections_are_not_recorded() {
    let parsed = parse_config_text("[color.tune]\nbrightness = 1.0\n\n[ambient]\n");
    assert!(parsed.custom_block_headers.is_empty());
}

#[test]
fn no_sections_means_no_headers() {
    let parsed = parse_config_text("color = green\nfps = 60\n");
    assert!(parsed.custom_block_headers.is_empty());
}

#[test]
fn is_custom_block_section_matches_the_three_namespaces() {
    assert!(is_custom_block_section("scene-custom.x"));
    assert!(is_custom_block_section("colors-custom.x"));
    assert!(is_custom_block_section("charset-custom.x"));
    assert!(is_custom_block_section("scene-custom"));
    assert!(is_custom_block_section("colors-custom"));
    assert!(is_custom_block_section("charset-custom"));
    // Look-alikes and unrelated sections: no.
    assert!(!is_custom_block_section("color.tune"));
    assert!(!is_custom_block_section("ambient"));
    assert!(!is_custom_block_section("colors-customish"));
    assert!(!is_custom_block_section(""));
}

#[test]
fn extraction_helper_sorts_and_filters() {
    let mut seen = std::collections::HashSet::new();
    seen.insert("scene-custom.b".to_string());
    seen.insert("colors-custom.a".to_string());
    seen.insert("color.tune".to_string());
    seen.insert("ambient.01-50".to_string());
    let headers = ParsedConfig::custom_block_headers_from(&seen);
    assert_eq!(
        headers,
        vec!["colors-custom.a".to_string(), "scene-custom.b".to_string()]
    );
}
