// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! v80.0.0-alpha.2 (S-master-HUNT-4) tests: human-duration config parse
//! (`parse_secs_config`) + the bool-flag numeric typo hint (owner's exact
//! `--crystal-dragon 10` input).

use crate::config_apply::parse_secs_config;

// ── parse_secs_config: the shared human vocabulary on config keys ────

#[test]
fn parse_secs_config_accepts_bare_and_human_forms() {
    for (input, expect) in [
        ("45", 45.0),
        ("45.5", 45.5),
        ("45s", 45.0),
        ("1m", 60.0),
        ("30m", 1800.0),
        ("1h", 3600.0),
        ("1h30m", 5400.0),
        ("86400", 86400.0),
    ] {
        let got = parse_secs_config("crystal-dragon-secs", input, 0.0, 86400.0);
        assert_eq!(
            got,
            Some(expect),
            "crystal-dragon-secs = {input} must resolve to {expect}s"
        );
    }
}

#[test]
fn parse_secs_config_rejects_out_of_range() {
    // In-range parse but above the 86400s cap.
    assert_eq!(
        parse_secs_config("crystal-dragon-secs", "25h", 0.0, 86400.0),
        None,
        "25h (90000s) exceeds the 86400 cap — must be rejected"
    );
    // Below a nonzero floor (the --duration contract uses 0.1).
    assert_eq!(
        parse_secs_config("--duration", "0.05", 0.1, 86400.0),
        None,
        "below-floor values must be rejected by the caller's range"
    );
}

#[test]
fn parse_secs_config_rejects_malformed() {
    for bad in ["abc", "6x", "-5s", "1.2.3s", ""] {
        assert_eq!(
            parse_secs_config("ambient-snapback-secs", bad, 0.0, 86400.0),
            None,
            "'{bad}' must be rejected"
        );
    }
}

// ── B6: numeric input on a bool flag hints the -secs twin ───────────

#[test]
fn parse_true_false_numeric_input_hints_secs_twin() {
    // The owner's exact typo: `cosmostrix --crystal-dragon 10` → the
    // error must point at --crystal-dragon-secs instead of a bare
    // boolean rejection.
    let err = crate::config::test_parse_true_false("10").unwrap_err();
    assert!(
        err.contains("--crystal-dragon-secs"),
        "numeric bool rejection must hint the -secs twin, got: {err}"
    );
    // Non-numeric garbage keeps the plain boolean message.
    let err = crate::config::test_parse_true_false("maybe").unwrap_err();
    assert!(
        !err.contains("--crystal-dragon-secs"),
        "non-numeric rejection must NOT carry the secs hint, got: {err}"
    );
    assert!(err.contains("invalid boolean value 'maybe'"));
}

// ── Startup config merge: human forms reach args (config surface) ────

#[test]
fn startup_config_merge_accepts_human_crystal_dragon_secs() {
    // args_with_config runs the full clap parse + config merge; the CLI
    // flag is unset so the config value (15s → 15.0) wins.
    let args = super::args_with_config("crystal-dragon-secs = 15s\n", &[]);
    assert_eq!(
        args.crystal_dragon_secs,
        Some(15.0),
        "config crystal-dragon-secs = 15s must reach the merged args as 15.0"
    );
    // Compound form through the same merge path.
    let args = super::args_with_config("crystal-dragon-secs = 1h30m\n", &[]);
    assert_eq!(
        args.crystal_dragon_secs,
        Some(5400.0),
        "config crystal-dragon-secs = 1h30m must reach the merged args as 5400.0"
    );
}
