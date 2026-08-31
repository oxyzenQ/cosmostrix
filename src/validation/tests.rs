// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the CLI validation module.
//!
//! Extracted from `mod.rs` to keep the production file under the 800-LOC cap.
//! Pure code motion — no behavior change.

use super::*;

#[test]
fn speed_accepts_safe_range_edges() {
    assert_eq!(validate_speed(SPEED_MIN).unwrap(), SPEED_MIN);
    assert_eq!(validate_speed(SPEED_MAX).unwrap(), SPEED_MAX);
}

#[test]
fn speed_rejects_unsafe_values_with_human_error() {
    for value in [
        "0", "0.5", "100.1", "1000", "100000", "01", "0000", "000,1", "000.1",
    ] {
        let err = parse_canonical_speed("--speed", value).expect_err("speed should reject");
        assert!(err.contains(&format!("error: invalid value for --speed: {value}")));
        assert!(err.contains("expected: canonical integer in range 1..=100"));
        assert!(!err.contains("Custom {"));
        assert!(!err.contains("0.001"));
        assert!(!err.contains("min 0.001 max 1000"));
    }
}

#[test]
fn cli_prevalidation_rejects_raw_numeric_values_cleanly() {
    let argv = ["cosmostrix", "--speed", "000,1"]
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    let err = prevalidate_cli_args(&argv).expect_err("invalid speed");
    assert_eq!(
        err,
        "error: invalid value for --speed: 000,1\nexpected: canonical integer in range 1..=100"
    );
}

#[test]
fn representative_cli_values_reject_cleanly() {
    // v17 mastery: --maxdpc removed from CLI. Replaced with --fps edge case.
    let cases = [
        ("--fps", "0", "expected: number in range 1..=240"),
        ("--density", "nope", "expected: number in range 0.01..=5"),
        ("--fps", "500", "expected: number in range 1..=240"),
        (
            "--monolith-size",
            "huge",
            "expected one of: small, normal, large",
        ),
    ];
    for (flag, value, expected) in cases {
        let argv = ["cosmostrix", flag, value]
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>();
        let err = prevalidate_cli_args(&argv).expect_err("invalid value");
        assert!(err.contains(expected), "{err}");
        assert!(!err.contains("Custom {"));
    }
}

// ── Stage 4b: removed-flag migration error tests ─────────────────────

#[test]
fn check_removed_flags_passes_clean_argv() {
    let argv = ["cosmostrix", "--scene", "storm", "--fps", "60"]
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    assert!(check_removed_flags(&argv).is_ok());
}

#[test]
fn check_removed_flags_passes_empty_argv() {
    let argv: Vec<OsString> = vec![OsString::from("cosmostrix")];
    assert!(check_removed_flags(&argv).is_ok());
}

#[test]
fn check_removed_flags_intercepts_preset() {
    let argv = ["cosmostrix", "--preset", "cinematic"]
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    let err = check_removed_flags(&argv).expect_err("--preset must be intercepted");
    assert!(err.contains("--preset has been removed"));
    assert!(err.contains("--scene <name>"));
    assert!(err.contains("v14.0.0"));
}

#[test]
fn check_removed_flags_intercepts_profile() {
    let argv = ["cosmostrix", "--profile", "nightcore"]
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    let err = check_removed_flags(&argv).expect_err("--profile must be intercepted");
    assert!(err.contains("--profile has been removed"));
    assert!(err.contains("--scene-custom <name>"));
    assert!(err.contains("[profile.<name>]"));
    assert!(err.contains("[scene-custom.<name>]"));
}

#[test]
fn check_removed_flags_intercepts_low_power() {
    let argv = ["cosmostrix", "--low-power"]
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    let err = check_removed_flags(&argv).expect_err("--low-power must be intercepted");
    assert!(err.contains("--low-power has been removed"));
    assert!(err.contains("--scene low-power"));
}

#[test]
fn check_removed_flags_intercepts_list_presets() {
    let argv = ["cosmostrix", "--list-presets"]
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    let err = check_removed_flags(&argv).expect_err("--list-presets must be intercepted");
    assert!(err.contains("--list-presets has been removed"));
    assert!(err.contains("--list-scenes"));
}

#[test]
fn check_removed_flags_intercepts_list_profiles() {
    let argv = ["cosmostrix", "--list-profiles"]
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    let err = check_removed_flags(&argv).expect_err("--list-profiles must be intercepted");
    assert!(err.contains("--list-profiles has been removed"));
    assert!(err.contains("--list-scenes"));
}

#[test]
fn check_removed_flags_intercepts_show_preset() {
    let argv = ["cosmostrix", "--show-preset", "cinematic"]
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    let err = check_removed_flags(&argv).expect_err("--show-preset must be intercepted");
    assert!(err.contains("--show-preset has been removed"));
    assert!(err.contains("--show-scene <name>"));
}

#[test]
fn check_removed_flags_intercepts_dump_profile() {
    let argv = ["cosmostrix", "--dump-profile", "nightcore"]
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    let err = check_removed_flags(&argv).expect_err("--dump-profile must be intercepted");
    assert!(err.contains("--dump-profile has been removed"));
    assert!(err.contains("--show-scene <name>"));
}

#[test]
fn check_removed_flags_intercepts_completions() {
    // audit: --completions was removed in v15 but was missing from
    // the REMOVED_FLAGS table — users got a generic clap "unexpected
    // argument" error instead of a helpful migration message.
    let argv = ["cosmostrix", "--completions", "bash"]
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    let err = check_removed_flags(&argv).expect_err("--completions must be intercepted");
    assert!(err.contains("--completions"));
    assert!(err.contains("v15.0.0"));
    assert!(
        err.contains("clap_complete"),
        "migration message should point to clap_complete: {err}"
    );
}

#[test]
fn check_removed_flags_intercepts_equals_form() {
    // `--preset=cinematic` must also be intercepted.
    let argv = ["cosmostrix", "--preset=cinematic"]
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    let err = check_removed_flags(&argv).expect_err("--preset= form must be intercepted");
    assert!(err.contains("--preset has been removed"));
}

#[test]
fn check_removed_flags_intercepts_first_match_only() {
    // If multiple removed flags are present, the first one in argv wins.
    let argv = ["cosmostrix", "--low-power", "--preset", "storm"]
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    let err = check_removed_flags(&argv).expect_err("must intercept");
    assert!(
        err.contains("--low-power has been removed"),
        "should report --low-power first, got: {err}"
    );
}

#[test]
fn check_removed_flags_ignores_non_flag_tokens() {
    // Positional values that happen to contain "preset" must NOT match.
    let argv = ["cosmostrix", "preset"]
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    assert!(check_removed_flags(&argv).is_ok());
}

#[test]
fn prevalidate_cli_args_intercepts_removed_flags_before_other_checks() {
    // The full prevalidate_cli_args must also intercept removed flags
    // (this verifies the integration — prevalidate calls check_removed_flags).
    let argv = ["cosmostrix", "--preset", "storm"]
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    let err = prevalidate_cli_args(&argv).expect_err("must intercept via prevalidate");
    assert!(err.contains("--preset has been removed"));
    assert!(err.contains("--scene <name>"));
}

#[test]
fn force_flag_does_not_match_any_removed_flag_pattern() {
    // v30 (2026-08-05): --force is a new flag scoped to --dump-config.
    // Verify it is NOT accidentally caught by check_removed_flags
    // (which would reject it as a removed flag). --force must parse
    // cleanly through prevalidate so it reaches main() where the
    // dump-config overwrite logic reads args.force.
    let argv = ["cosmostrix", "--force"]
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    assert!(
        check_removed_flags(&argv).is_ok(),
        "--force must not be intercepted as a removed flag"
    );
    assert!(
        prevalidate_cli_args(&argv).is_ok(),
        "--force must pass prevalidate so it reaches main()"
    );
}

#[test]
fn force_flag_parses_alongside_dump_config() {
    // Verify --force parses cleanly when combined with --dump-config
    // (the canonical use case). We don't verify the actual file write
    // here — that requires a subprocess integration test. This test
    // just locks in that the two flags compose without clap errors.
    let argv = [
        "cosmostrix",
        "--dump-config",
        "/tmp/should-not-be-written.toml",
        "--force",
    ]
    .into_iter()
    .map(OsString::from)
    .collect::<Vec<_>>();
    // Both flags are valid; prevalidate must accept them.
    assert!(
        prevalidate_cli_args(&argv).is_ok(),
        "--dump-config + --force must pass prevalidate"
    );
}

// ── v51 did-you-mean audit: enum value typos must suggest ──────────────

#[test]
fn enum_typo_glitch_level_suggests_closest() {
    let err = prevalidate_cli_args(&[
        "cosmostrix".into(),
        "--glitch-level".into(),
        "sutble".into(),
    ])
    .unwrap_err();
    assert!(
        err.contains("tip: a similar value exists: 'subtle'"),
        "glitch-level typo must suggest the closest value, got: {err}"
    );
}

#[test]
fn enum_typo_monolith_size_suggests_closest() {
    let err = prevalidate_cli_args(&["cosmostrix".into(), "--monolith-size".into(), "larg".into()])
        .unwrap_err();
    assert!(
        err.contains("tip: a similar value exists: 'large'"),
        "monolith-size typo must suggest the closest value, got: {err}"
    );
}

#[test]
fn enum_typo_color_bg_attached_form_suggests() {
    // The `=` attached form also flows through validate_cli_value.
    // "default-backgroun" is distance 1 from "default-background".
    let err = prevalidate_cli_args(&["cosmostrix".into(), "--color-bg=default-backgroun".into()])
        .unwrap_err();
    assert!(
        err.contains("tip: a similar value exists: 'default-background'"),
        "color-bg typo must suggest the closest value, got: {err}"
    );
}

#[test]
fn enum_typo_color_bg_transposed_word_no_bogus_tip() {
    // "defualt" is distance ~14 from "black" and ~11 from
    // "default-background" — no suggestion must be fabricated.
    let err =
        prevalidate_cli_args(&["cosmostrix".into(), "--color-bg=defualt".into()]).unwrap_err();
    assert!(
        !err.contains("tip: a similar"),
        "a transposed word with no close candidate must not fabricate a tip, got: {err}"
    );
}

#[test]
fn enum_far_off_value_gets_no_suggestion() {
    let err = prevalidate_cli_args(&[
        "cosmostrix".into(),
        "--glitch-level".into(),
        "extravagant".into(),
    ])
    .unwrap_err();
    assert!(
        !err.contains("tip: a similar"),
        "a distant value must not produce a bogus suggestion, got: {err}"
    );
}
