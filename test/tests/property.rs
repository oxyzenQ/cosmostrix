// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! Pillar 5: Property-based tests for config parser + screen size edge cases.
//!
//! These tests use `proptest` to generate thousands of random inputs and
//! verify the parser never panics, never produces invalid output, and
//! that valid configs round-trip correctly. This catches edge cases that
//! unit tests miss (Unicode, embedded nulls, extreme lengths, etc.).
//!
//! The regression store (night-netrunner-Z2, the zelynic proof-layer
//! shape): every counterexample proptest finds and shrinks is persisted
//! to `test/tests/property-regressions.txt` and replayed on every
//! subsequent run before any novel case is generated. The store is
//! committed to source control so every contributor and CI lane re-runs
//! the exact witness of every past failure. The path is pinned with
//! `FileFailurePersistence::Direct`, not the default `SourceParallel`:
//! the macro stamps its own `source_file` from `file!()`, and this
//! crate's tests live under `test/tests/` included into the bin target
//! via `#[path]` — the walk-up from there never sees `src/main.rs`, so
//! the default would fall back to a `.txt` beside each source file and
//! scatter one store per file instead of one committed ledger.

use crate::configfile::parse_config_text;
use proptest::prelude::*;
use proptest::test_runner::{Config, FileFailurePersistence};

fn regression_store() -> Config {
    Config {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "test/tests/property-regressions.txt",
        ))),
        ..Config::default()
    }
}

proptest! {
    #![proptest_config(regression_store())]

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

    /// The auto-promote typo-guard law (v50.0.0-beta.6 FATAL FIX +
    /// v80.0.0-beta.1 relaxation, see configfile_promote.rs): a SCALAR
    /// known top-level key nested inside a custom block
    /// ([charset-custom.<name>] / [colors-custom.<name>] /
    /// [scene-custom.<name>]) is a field-name typo and must NEVER be
    /// promoted to root scope — the whole point of the guard is that
    /// `color = green` inside `[charset-custom.quantum]` must not
    /// silently change the global color scheme. The property quantifies
    /// over every valid custom-block shape and every scalar key on the
    /// known-keys list, holding the promotion contract that until now
    /// was only pinned by hand-picked unit vectors (night-netrunner-Z2).
    ///
    /// The first counterexample the store caught (seed cc 8ec86c94…,
    /// shrunk to `scene-custom.a` + `color`) sharpened the model the
    /// way zelynic's fraction law did: for [scene-custom.<name>],
    /// the scalar key may ALSO be a legitimate block FIELD (color,
    /// charset, fps, speed, density, glitch-level, rain) — those route
    /// to values under the dotted key and are the intended content of
    /// the block, not typos. The law therefore has two destination
    /// classes: field keys land in values, non-field keys surface in
    /// unknown_keys. In NO class does the bare key reach root scope.
    #[test]
    fn prop_scalar_key_in_custom_block_never_promotes(
        block in prop::sample::select(vec![
            "charset-custom",
            "colors-custom",
            "scene-custom",
        ]),
        name in "[a-z][a-z0-9_]{0,10}",
        scalar_key in prop::sample::select(SCALAR_ROOT_KEYS),
        value in "[a-z0-9]{1,10}"
    ) {
        let config = format!("[{block}.{name}]\n{scalar_key} = \"{value}\"\n");
        let parsed = parse_config_text(&config);
        let full_key = format!("{block}.{name}.{scalar_key}");
        // The typo guard, both classes: no root-scope re-home under
        // the bare name, ever.
        prop_assert!(
            !parsed.values.contains_key(scalar_key),
            "scalar key '{scalar_key}' escaped the custom block [{block}.{name}] \
             into root scope — the v50 typo guard regressed"
        );
        prop_assert!(parsed.promoted_keys.is_empty());
        if block == "scene-custom" && SCENE_CUSTOM_FIELD_KEYS.contains(&scalar_key) {
            // A legitimate block field: stored under the dotted key.
            prop_assert!(
                parsed.values.get(&full_key) == Some(&value),
                "scene-custom field '{full_key}' must land in values"
            );
        } else {
            // A typo: the rejection is visible, not silent — the full
            // dotted key surfaces in unknown_keys so config_hints can
            // attach the targeted field-name message.
            prop_assert!(
                parsed.unknown_keys.iter().any(|u| u == &full_key),
                "unknown scalar key '{full_key}' must surface in unknown_keys, \
                 not vanish"
            );
        }
    }
}

/// The scalar (dot-free) subset of the root known-keys list — the keys
/// the v50 typo guard exists for. Dot-bearing keys (ambient.01-50,
/// color.tune.brightness) are the v80 relaxation's promoted class and
/// are covered by prop_section_prefixes_keys' three-destination model.
const SCALAR_ROOT_KEYS: &[&str] = &[
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
    "crystal-dragon-secs",
    "power-dragon",
    "async-mode",
    "intro",
    "intro-color",
    "message",
    "message-border",
    "msg-mode",
    "msg-fill-style",
    "ambient-snapback-secs",
];

/// The scene-custom block's own field allowlist (SCENE_CUSTOM_FIELDS in
/// src/scene_custom/mod.rs): a scalar root key that also names one of
/// these is the block's intended content, not a typo. Only the
/// dot-free members matter here — colors-custom/charset-custom are
/// namespaced and never collide with the scalar key class.
const SCENE_CUSTOM_FIELD_KEYS: &[&str] = &[
    "rain",
    "color",
    "charset",
    "fps",
    "speed",
    "density",
    "glitch-level",
];
