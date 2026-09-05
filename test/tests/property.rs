// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Pillar 5: Property-based tests for config parser + screen size edge cases.
//!
//! These tests use `proptest` to generate thousands of random inputs and
//! verify the parser never panics, never produces invalid output, and
//! that valid configs round-trip correctly. This catches edge cases that
//! unit tests miss (Unicode, embedded nulls, extreme lengths, etc.).

use crate::configfile::parse_config_text;
use proptest::prelude::*;

proptest! {
    /// Any string input to parse_config_text must NOT panic.
    /// The parser must handle arbitrary input gracefully (malformed → empty
    /// values + malformed_lines, valid → parsed values).
    #[test]
    fn prop_parser_never_panics(input in ".{0,1000}") {
        let _ = parse_config_text(&input);
    }

    /// Any string input with key = value pairs must parse without panic.
    /// Generates random TOML-like key=value lines.
    #[test]
    fn prop_parser_key_value_never_panics(
        lines in prop::collection::vec(
            ("[a-z]{1,20}", "=[^\n]{0,100}"),
            0..50
        )
    ) {
        let config: String = lines.iter()
            .map(|(k, v)| format!("{k}{v}\n"))
            .collect();
        let _ = parse_config_text(&config);
    }

    /// Config with random special characters must not panic.
    #[test]
    fn prop_parser_special_chars_never_panics(
        chars in prop::collection::vec(0u8..255, 0..500)
    ) {
        let input = String::from_utf8_lossy(&chars);
        let _ = parse_config_text(&input);
    }

    /// Valid scene = "name" lines must round-trip correctly.
    #[test]
    fn prop_scene_name_round_trips(
        scene in "[a-z][a-z0-9-]{0,20}"
    ) {
        let config = format!("scene = \"{scene}\"\n");
        let parsed = parse_config_text(&config);
        prop_assert!(parsed.values.contains_key("scene"));
        let value = parsed.values.get("scene").unwrap();
        // Parser strips quotes, so the stored value should match the input.
        prop_assert_eq!(value, &scene);
    }

    /// Valid ambient.HH-MM = "scene" lines must parse correctly.
    #[test]
    fn prop_ambient_entry_round_trips(
        hour in 0u32..24,
        minute in 0u32..60,
        scene in "[a-z][a-z0-9-]{0,20}"
    ) {
        let config = format!("ambient.{hour:02}-{minute:02} = \"{scene}\"\n");
        let parsed = parse_config_text(&config);
        let key = format!("ambient.{hour:02}-{minute:02}");
        prop_assert!(parsed.values.contains_key(&key));
    }

    /// Multiple keys in a [section] block must all be prefixed correctly.
    /// Unknown keys go to unknown_keys, known keys go to values — but
    /// the parser must never panic and must always prefix with section.
    /// A key whose UN-prefixed name is itself a known top-level field
    /// takes the documented third path: auto-promotion to root scope
    /// (recorded in promoted_keys, value re-homed under the bare name —
    /// see configfile.rs "Auto-promote forgiving parser"). The property
    /// models all three destinations (NIGHT-hunter-10: the two-path
    /// model flaked whenever the random generator produced a known
    /// short field name like `fps`/`bg`/`set` nested under a section).
    #[test]
    fn prop_section_prefixes_keys(
        section in "[a-z][a-z0-9-]{0,20}",
        fields in prop::collection::vec(
            ("[a-z][a-z0-9-]{0,20}", "[a-z0-9]{1,50}"),
            1..10
        )
    ) {
        let mut config = format!("[{section}]\n");
        for (k, v) in &fields {
            config.push_str(&format!("{k} = \"{v}\"\n"));
        }
        let parsed = parse_config_text(&config);
        // Each field must appear in values, unknown_keys or the
        // promoted_keys record — prefixed with the section name in the
        // latter two, re-homed to root scope in the first.
        for (k, v) in &fields {
            let full_key = format!("{section}.{k}");
            let in_values = parsed.values.contains_key(&full_key);
            let in_unknown = parsed.unknown_keys.iter().any(|u| u == &full_key);
            let promoted = parsed.promoted_keys.iter().any(|(full, _)| full == &full_key);
            prop_assert!(
                in_values || in_unknown || promoted,
                "key '{full_key}' should be in values, unknown_keys or promoted_keys"
            );
            // If it's in values, verify the value matches (quotes stripped).
            if let Some(got) = parsed.values.get(&full_key) {
                prop_assert_eq!(got, v, "value should match (quotes stripped)");
            }
            // If it was promoted, the value must exist under the BARE
            // root-scope name (quotes stripped).
            if promoted {
                let bare = parsed.values.get(k);
                prop_assert!(
                    bare.is_some(),
                    "promoted key '{full_key}' must be re-homed under the bare name '{k}'"
                );
            }
        }
    }

    /// Empty config must always parse to empty values, no errors.
    #[test]
    fn prop_empty_config_is_clean(input in "[\n\r\t ]{0,100}") {
        let parsed = parse_config_text(&input);
        prop_assert!(parsed.values.is_empty());
        prop_assert!(parsed.malformed_lines.is_empty());
        prop_assert!(parsed.unknown_keys.is_empty());
    }

    /// Comment-only config must always parse to empty values, no errors.
    #[test]
    fn prop_comment_only_config_is_clean(
        comments in prop::collection::vec("#[^\n]{0,200}", 0..50)
    ) {
        let config: String = comments.iter()
            .map(|c| format!("{c}\n"))
            .collect();
        let parsed = parse_config_text(&config);
        prop_assert!(parsed.values.is_empty());
        prop_assert!(parsed.malformed_lines.is_empty());
    }
}
