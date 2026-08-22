// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! (bug #7) regression tests — unquoted '#' inside array + multi-line
//! array consumer over-eating.
//!
//! Bug summary: `rain = [#ff0000, #00ff00]` (unquoted hex inside array) had
//! strip_inline_comment strip at first '#' → value became '[' → multi-line
//! array consumer entered (starts with '[', doesn't end with ']') → blindly
//! scanned forward looking for ']' → ate subsequent lines including unrelated
//! key=value pairs and even [section] headers (which end with ']' and got
//! mistaken for the closing bracket). Result: silent data corruption.
//!
//! Fix: detect unquoted '#' inside array value via
//! `unquoted_hash_inside_array()` and reject explicitly. Also harden the
//! multi-line consumer to NOT consume [section] headers.

#![cfg(test)]

use crate::configfile::{parse_config_text, unquoted_hash_inside_array};

#[test]
fn unquoted_hash_in_array_emits_malformed_not_silent_truncate() {
    let parsed = parse_config_text("[colors-custom.mytheme]\nrain = [#ff0000, #00ff00]\n");
    assert!(
        !parsed.values.contains_key("colors-custom.mytheme.rain"),
        "rain key with unquoted # must NOT be silently accepted; got: {:?}",
        parsed.values
    );
    assert!(
        !parsed.malformed_lines.is_empty(),
        "must emit at least one malformed line entry; got: {:?}",
        parsed.malformed_lines
    );
}

#[test]
fn unquoted_hash_in_array_does_not_eat_next_key_value_line() {
    let parsed =
        parse_config_text("[colors-custom.mytheme]\nrain = [#ff0000, #00ff00]\nbg = #0a0a12\n");
    assert!(
        !parsed.values.contains_key("colors-custom.mytheme.rain"),
        "rain must not silently absorb bg line; got: {:?}",
        parsed.values
    );
    assert!(
        !parsed.values.contains_key("colors-custom.mytheme.bg"),
        "bg with unquoted # must also be rejected; got: {:?}",
        parsed.values
    );
    assert!(
        parsed.malformed_lines.len() >= 2,
        "expected >=2 malformed entries (one per bad line); got: {:?}",
        parsed.malformed_lines
    );
}

#[test]
fn multiline_array_consumer_does_not_eat_section_header() {
    let parsed = parse_config_text(
        "[colors-custom.mytheme]\n\
         rain = [#ff0000, #00ff00]\n\
         \n\
         [scene-custom.hacker-mode]\n\
         color = green\n\
         speed = 28\n",
    );
    assert!(
        !parsed.values.contains_key("colors-custom.mytheme.rain"),
        "rain must not silently absorb the [scene-custom.hacker-mode] header; got: {:?}",
        parsed.values
    );
    assert_eq!(
        parsed
            .values
            .get("scene-custom.hacker-mode.color")
            .map(String::as_str),
        Some("green"),
        "scene-custom.hacker-mode.color must be preserved; got: {:?}",
        parsed.values
    );
    assert_eq!(
        parsed
            .values
            .get("scene-custom.hacker-mode.speed")
            .map(String::as_str),
        Some("28"),
        "scene-custom.hacker-mode.speed must be preserved; got: {:?}",
        parsed.values
    );
}

#[test]
fn multiline_array_consumer_does_not_eat_root_keys() {
    let parsed = parse_config_text(
        "[colors-custom.mytheme]\n\
         rain = [#ff0000, #00ff00]\n\
         fps = 60\n\
         speed = 30\n",
    );
    assert!(
        !parsed.values.contains_key("colors-custom.mytheme.rain"),
        "rain must not silently absorb fps/speed lines; got: {:?}",
        parsed.values
    );
}

#[test]
fn unquoted_hash_in_single_value_is_malformed() {
    let parsed = parse_config_text("[colors-custom.mytheme]\nbg = #0a0a12\n");
    assert!(
        !parsed.values.contains_key("colors-custom.mytheme.bg"),
        "bg with unquoted # must be rejected; got: {:?}",
        parsed.values
    );
    assert!(!parsed.malformed_lines.is_empty());
}

#[test]
fn quoted_array_with_trailing_comment_still_works() {
    let parsed =
        parse_config_text("[colors-custom.mytheme]\nrain = [\"#ff0000\", \"#00ff00\"] # comment\n");
    assert_eq!(
        parsed
            .values
            .get("colors-custom.mytheme.rain")
            .map(String::as_str),
        Some("[\"#ff0000\", \"#00ff00\"]"),
        "quoted array with trailing comment must be preserved; got: {:?}",
        parsed.values
    );
}

#[test]
fn multiline_quoted_array_still_works() {
    let parsed = parse_config_text(
        "[colors-custom.mytheme]\n\
         rain = [\n\
         \x20 \"#ff0000\",\n\
         \x20 \"#00ff00\",\n\
         ]\n",
    );
    assert!(
        parsed.values.contains_key("colors-custom.mytheme.rain"),
        "legitimate multi-line array must be preserved; got: {:?}",
        parsed.values
    );
    let v = parsed.values.get("colors-custom.mytheme.rain").unwrap();
    assert!(v.contains("#ff0000"), "must contain first stop; got: {v:?}");
    assert!(
        v.contains("#00ff00"),
        "must contain second stop; got: {v:?}"
    );
}

#[test]
fn unquoted_hash_inside_array_detects_bad_pattern() {
    assert!(unquoted_hash_inside_array("rain = [#ff0000, #00ff00]").is_some());
    assert!(unquoted_hash_inside_array("bg = #0a0a12").is_none());
    assert!(unquoted_hash_inside_array("rain = [\"#ff0000\"] # comment").is_none());
    assert!(unquoted_hash_inside_array("rain = [\"#ff0000\", \"#00ff00\"]").is_none());
}
