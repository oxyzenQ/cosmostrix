// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-hunt-38-supermassive parser tests: separator-typo rejection.
//!
//! Owner fatal report (commit 6198431, manual testing, 2026-09-13): the
//! forgiving parser silently accepted `key == value` lines —
//! `split_once('=')` stored `= "x"` as the VALUE, so a
//! `[charset-custom.test]` + `set == "x"` passed --testconf (PASS) and
//! produced a charset of garbage glyphs, while an
//! `ambient.06-00 == "signal"` misfired the "legacy multi-field
//! format" migration essay (value `= "signal"` contains `=`).
//!
//! Both typo classes now land in `malformed_lines` with a targeted
//! `# ERROR:` note, and legitimate quoted `=`-leading values are
//! unaffected.

use crate::configfile::parse_config_text;

#[test]
fn double_equals_line_is_malformed_with_targeted_note() {
    // The owner's repro 3: `[charset-custom.test]` + `set == "x"` —
    // previously 17 keys parsed, 0 errors, PASS.
    let parsed = parse_config_text("[charset-custom.test]\n set == \"x\"\n");
    assert!(
        parsed.malformed_lines.len() == 1,
        "the == line must be the only malformed line, got: {:?}",
        parsed.malformed_lines
    );
    let line = &parsed.malformed_lines[0];
    assert!(
        line.contains("set == \"x\""),
        "malformed record must quote the line, got: {line}"
    );
    assert!(
        line.contains("double '='"),
        "malformed record must flag the double-equals typo, got: {line}"
    );
    // The typo'd key must NOT be stored — the silent-accept path is dead.
    assert!(
        !parsed.values.contains_key("charset-custom.test.set"),
        "the == typo must not store a value, got: {:?}",
        parsed.values
    );
    // But the section header is still recorded — the completeness layer
    // can see the (now incomplete) block and reports it as such.
    assert_eq!(
        parsed.custom_block_headers,
        vec!["charset-custom.test".to_string()]
    );
}

#[test]
fn spaced_double_equals_is_also_malformed() {
    // `key = = value` — the same typo with a space between the equals.
    let parsed = parse_config_text("msg-mode = = true\n");
    assert_eq!(parsed.malformed_lines.len(), 1);
    assert!(parsed.malformed_lines[0].contains("double '='"));
    assert!(parsed.values.is_empty());
}

#[test]
fn ambient_double_equals_is_a_syntax_error_not_legacy_format() {
    // The owner's repro 4 mechanism: `ambient.06-00 == "signal"` used to
    // store `= "signal"` and misfire the legacy-multi-field essay. Now
    // the line is malformed at the parser level — the ambient value
    // never exists, so the stale essay cannot fire.
    let parsed = parse_config_text("ambient.06-00 == \"signal\"\n");
    assert_eq!(parsed.malformed_lines.len(), 1);
    assert!(parsed.malformed_lines[0].contains("double '='"));
    assert!(parsed.values.is_empty());
}

#[test]
fn quoted_leading_equals_value_is_still_legal() {
    // bug #19 invariant extension: a QUOTED value whose content starts
    // with `=` is a string, not a double-equals typo. `set = "=x"` must
    // keep working (charset pools legitimately contain '=' glyphs).
    let parsed = parse_config_text("[charset-custom.eq]\n set = \"=x\"\n");
    assert!(
        parsed.malformed_lines.is_empty(),
        "{:?}",
        parsed.malformed_lines
    );
    assert_eq!(
        parsed
            .values
            .get("charset-custom.eq.set")
            .map(String::as_str),
        Some("=x")
    );
}

#[test]
fn colon_separator_line_is_malformed_with_targeted_note() {
    // The owner's repro 2: `[charset-custom.test]` + `set : "x"` — was
    // already rejected, but with only the generic diagnostic. Now the
    // `key : value` shape (YAML/JSON habit) gets the colon note.
    let parsed = parse_config_text("[charset-custom.test]\n set : \"x\"\n");
    assert!(parsed.malformed_lines.len() == 1);
    let line = &parsed.malformed_lines[0];
    assert!(line.contains("set : \"x\""), "got: {line}");
    assert!(
        line.contains("':' is not a TOML separator"),
        "malformed record must flag the colon habit, got: {line}"
    );
    assert!(!parsed.values.contains_key("charset-custom.test.set"));
}

#[test]
fn colon_note_requires_key_shaped_lhs() {
    // Stray text without '=' is still malformed, but does not get the
    // colon hint unless the LHS looks like a config key (token of
    // alphanumerics/-/_/.). A pasted prose line has spaces in the LHS.
    let parsed = parse_config_text("hello world: something\n");
    assert_eq!(parsed.malformed_lines.len(), 1);
    assert!(
        !parsed.malformed_lines[0].contains("':' is not a TOML separator"),
        "prose must keep the generic diagnostic, got: {}",
        parsed.malformed_lines[0]
    );
}

#[test]
fn colon_note_fires_for_namespaced_keys() {
    // `ambient.06-00 : "signal"` — dotted key LHS is key-shaped.
    let parsed = parse_config_text("ambient.06-00 : \"signal\"\n");
    assert_eq!(parsed.malformed_lines.len(), 1);
    assert!(parsed.malformed_lines[0].contains("':' is not a TOML separator"));
}

#[test]
fn normal_equals_lines_are_unaffected() {
    // Regression guard: the typo gates must not disturb the ordinary
    // grammar — plain keys, quoted values, arrays, sections.
    let parsed = parse_config_text(
        "scene = \"cinematic\"\nfps = 60\n\n[color.tune]\nbrightness = 1.0\n\n[charset-custom.zen]\nset = \"|=+\"\n\n[colors-custom.zen]\nbg = \"#0a0a0a\"\nrain = [\"#111111\", \"#222222\"]\n",
    );
    assert!(
        parsed.malformed_lines.is_empty(),
        "{:?}",
        parsed.malformed_lines
    );
    assert!(parsed.unknown_keys.is_empty(), "{:?}", parsed.unknown_keys);
    assert_eq!(parsed.values.len(), 6);
    assert_eq!(
        parsed
            .values
            .get("charset-custom.zen.set")
            .map(String::as_str),
        Some("|=+")
    );
}
