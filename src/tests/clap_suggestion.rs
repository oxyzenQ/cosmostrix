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

#[test]
fn extracts_msg_fill_style_long_flag_typo() {
    // v51 msg-fill-style: a long-flag typo (missing the final 'e') must
    // extract the msg-fill-style suggestion so main.rs can append the
    // "Did you mean --msg-fill-style?" line.
    let err = "error: unexpected argument '--msg-fill-styl' found\n\n  tip: a similar argument exists: '--msg-fill-style'\n\nUsage: cosmostrix --verbose";
    assert_eq!(
        extract_clap_suggestion(err),
        Some("msg-fill-style".to_string())
    );
}

#[test]
fn extracts_msg_fill_style_plural_suggestion() {
    // Plural form: clap may offer both --msg-mode and --msg-fill-style
    // for near-miss typos; the FIRST suggestion is extracted.
    let err = "error: unexpected argument '--msg-fill' found\n\n  tip: some similar arguments exist: '--msg-mode', '--msg-fill-style'\n\nUsage: cosmostrix --verbose";
    assert_eq!(extract_clap_suggestion(err), Some("msg-mode".to_string()));
}

#[test]
fn clap_parses_all_msg_fill_style_values() {
    // The ValueEnum registration must accept exactly the seven documented
    // styles via the long flag (the -mfs short form is argv-expanded to
    // this flag before clap runs — see cli/argv_expand.rs tests).
    use clap::Parser;
    for (value, expected) in [
        (
            "typewriter",
            crate::msg_fill_style::MsgFillStyle::Typewriter,
        ),
        ("fade", crate::msg_fill_style::MsgFillStyle::Fade),
        ("words", crate::msg_fill_style::MsgFillStyle::Words),
        ("slide", crate::msg_fill_style::MsgFillStyle::Slide),
        ("pulse", crate::msg_fill_style::MsgFillStyle::Pulse),
        ("instant", crate::msg_fill_style::MsgFillStyle::Instant),
        ("engrave", crate::msg_fill_style::MsgFillStyle::Engrave),
        ("hologram", crate::msg_fill_style::MsgFillStyle::Hologram),
        ("glitch", crate::msg_fill_style::MsgFillStyle::Glitch),
        ("scorch", crate::msg_fill_style::MsgFillStyle::Scorch),
    ] {
        let args = crate::config::Args::try_parse_from(["cosmostrix", "--msg-fill-style", value])
            .unwrap_or_else(|e| panic!("--msg-fill-style {value} must parse: {e}"));
        assert_eq!(args.msg_fill_style, expected);
    }
}

#[test]
fn clap_rejects_invalid_msg_fill_style_value() {
    use clap::Parser;
    let err = crate::config::Args::try_parse_from(["cosmostrix", "--msg-fill-style", "scanner"])
        .expect_err("invalid style value must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("invalid value") && msg.contains("typewriter"),
        "error must name the invalid value and list the possible values, got: {msg}"
    );
}

#[test]
fn clap_default_msg_fill_style_is_typewriter() {
    // LTS guarantee: no flag → typewriter (pre-v51 behavior preserved).
    use clap::Parser;
    let args = crate::config::Args::try_parse_from(["cosmostrix"]).unwrap();
    assert_eq!(
        args.msg_fill_style,
        crate::msg_fill_style::MsgFillStyle::Typewriter
    );
}
