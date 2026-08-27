// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the CLI "Did you mean?" suggestion engine.
//!
//! v50.0.0-beta.7: the custom Levenshtein-based suggestion engine
//! (KNOWN_LONG_FLAGS + cli_edit_distance + suggest_cli_flag) was removed
//! and replaced by `extract_clap_suggestion` in main.rs, which reads
//! clap's own "tip:" line. These tests verify the parser correctly
//! extracts the suggested flag name from clap's error string in both
//! singular and plural forms.

use crate::extract_clap_suggestion;

#[test]
fn extracts_singular_suggestion() {
    // Singular form: "tip: a similar argument exists: '--no-effects'"
    let err = "error: unexpected argument '--no-effecs' found\n\n  tip: a similar argument exists: '--no-effects'\n\nUsage: cosmostrix --verbose --no-effects";
    assert_eq!(extract_clap_suggestion(err), Some("no-effects".to_string()));
}

#[test]
fn extracts_plural_suggestion() {
    // Plural form: "tip: some similar arguments exist: '--color', '--color-bg'"
    let err = "error: unexpected argument '--clr' found\n\n  tip: some similar arguments exist: '--color', '--color-bg'\n\nUsage: cosmostrix --verbose";
    // Should extract the FIRST suggestion
    assert_eq!(extract_clap_suggestion(err), Some("color".to_string()));
}

#[test]
fn returns_none_when_no_tip_line() {
    // No "tip:" line — clap didn't find a close match
    let err = "error: unexpected argument '--xyz' found\n\nUsage: cosmostrix --verbose";
    assert_eq!(extract_clap_suggestion(err), None);
}

#[test]
fn returns_none_when_tip_has_no_flag() {
    // Malformed tip line (no '--FLAG' pattern after "tip:")
    let err = "error: unexpected argument '--foo' found\n\n  tip: check the documentation\n\nUsage: cosmostrix";
    assert_eq!(extract_clap_suggestion(err), None);
}

#[test]
fn extracts_suggestion_with_dashes_in_name() {
    // Flag names with multiple dash segments
    let err = "error: unexpected argument '--crystal-dragns' found\n\n  tip: a similar argument exists: '--crystal-dragon'\n\nUsage: cosmostrix";
    assert_eq!(
        extract_clap_suggestion(err),
        Some("crystal-dragon".to_string())
    );
}

#[test]
fn extracts_no_effects_typo() {
    // Regression test: the --no-effecs typo (missing 't') must now
    // produce a "Did you mean --no-effects?" suggestion. Before the fix,
    // the custom KNOWN_LONG_FLAGS list was missing "no-effects" (missed
    // during the --disable-effects -> --no-effects rename), so no
    // suggestion was shown — only the clap "tip:" line appeared.
    let err = "error: unexpected argument '--no-effecs' found\n\n  tip: a similar argument exists: '--no-effects'\n\nUsage: cosmostrix --verbose --no-effects";
    assert_eq!(extract_clap_suggestion(err), Some("no-effects".to_string()));
}
