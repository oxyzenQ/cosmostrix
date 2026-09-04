// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the CLI argument-suggestion contract.
//!
//! v100.0.0-nightly.1 (2026-09-04 CLI UX centralization): the old
//! string-parsing `extract_clap_suggestion` engine (it scraped clap's
//! RENDERED "tip:" line so main.rs could append a duplicate tip) was
//! deleted. What matters now is the structured contract the render
//! relies on:
//!
//! 1. clap's `suggestions` feature is wired (unknown flags close to a
//!    known flag carry a `SuggestedArg` context);
//! 2. `cli::ux::exit_clap_error` renders clap's own tip exactly once
//!    and always with the real usage line (covered by the tests in
//!    `cli/ux.rs`);
//! 3. the value-suggestion engine (`cli::suggestion::closest_value_match`)
//!    keeps catching value typos.
//!
//! Historical regression kept from the v50.0.0-beta.7 audit: the
//! `--disable-effects` → `--no-effects` rename once missed the
//! hand-maintained flag list, so typo suggestions silently died.
//! With clap's own suggestion engine there is no list to miss, and
//! these tests lock that in.

use clap::error::ContextKind;
use clap::Parser;

/// Extract the first suggested argument from a clap error's structured
/// context (the render path reads the same context).
fn suggested_arg(err: &clap::Error) -> Option<String> {
    match err.get(ContextKind::SuggestedArg) {
        Some(clap::error::ContextValue::String(s)) => Some(s.clone()),
        Some(clap::error::ContextValue::Strings(v)) => v.first().cloned(),
        _ => None,
    }
}

#[test]
fn unknown_flag_close_to_testconf_carries_suggestion() {
    // The owner's original 2026-09-04 report case: `--test` must
    // suggest `--testconf` via clap's own engine.
    let err = crate::config::Args::try_parse_from(["cosmostrix", "--test"])
        .expect_err("--test must be unknown");
    let suggestion = suggested_arg(&err).expect("a SuggestedArg context must be present");
    assert!(
        suggestion.contains("testconf"),
        "suggestion must point at --testconf, got {suggestion:?}"
    );
}

#[test]
fn no_effects_rename_regression_still_suggests() {
    // v50.0.0-beta.7 regression: --no-effecs (typo of the RENAMED
    // --no-effects) must still suggest --no-effects. The rename once
    // missed a hand-maintained list; clap's engine cannot miss it.
    let err = crate::config::Args::try_parse_from(["cosmostrix", "--no-effecs"])
        .expect_err("--no-effecs must be unknown");
    let suggestion = suggested_arg(&err).expect("a SuggestedArg context must be present");
    assert!(
        suggestion.contains("no-effects"),
        "suggestion must point at --no-effects, got {suggestion:?}"
    );
}

#[test]
fn far_off_unknown_flag_has_no_suggestion() {
    // A distant typo must NOT carry a suggestion (distance policy) —
    // the render then omits the tip line entirely, no noise.
    let err = crate::config::Args::try_parse_from(["cosmostrix", "--zzzzqqqq"])
        .expect_err("--zzzzqqqq must be unknown");
    assert!(
        suggested_arg(&err).is_none(),
        "distant typo must not carry a SuggestedArg context"
    );
}

#[test]
fn clap_parses_all_msg_fill_style_values() {
    // The ValueEnum registration must accept exactly the documented
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
        ("instant", crate::msg_fill_style::MsgFillStyle::Instant),
        ("engrave", crate::msg_fill_style::MsgFillStyle::Engrave),
        ("hologram", crate::msg_fill_style::MsgFillStyle::Hologram),
        ("glitch", crate::msg_fill_style::MsgFillStyle::Glitch),
        ("scorch", crate::msg_fill_style::MsgFillStyle::Scorch),
        ("cascade", crate::msg_fill_style::MsgFillStyle::Cascade),
        ("radar", crate::msg_fill_style::MsgFillStyle::Radar),
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
fn clap_default_msg_fill_style_is_engrave() {
    // v80.0.0-beta.2 champion: no flag → engrave (owner champion winner).
    // The pre-beta.2 default was typewriter for LTS bit-identical parity.
    use clap::Parser;
    let args = crate::config::Args::try_parse_from(["cosmostrix"]).unwrap();
    assert_eq!(
        args.msg_fill_style,
        crate::msg_fill_style::MsgFillStyle::Engrave
    );
}
