// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tests for `cli_parse` — extracted from `cli_parse.rs` to keep that
//! file under the 800-LOC hard cap (see `src/RULES_LOC.md`; the
//! S-master-HUNT-5 24h-ceiling test additions pushed it over).
//!
//! Declared from `cli_parse.rs` via `#[cfg(test)] #[path =
//! "cli_parse_tests.rs"] mod tests;` — `use super::*` still resolves
//! to the parser functions.

use super::*;

// ── parse_duration tests ────────────────────────────────────────────

#[test]
fn parse_duration_bare_number_is_seconds() {
    assert_eq!(parse_duration("--bench-duration", "5").unwrap(), 5);
    assert_eq!(parse_duration("--bench-duration", "90").unwrap(), 90);
}

// ── parse_secs_f64 tests (v80.0.0-alpha.2 human duration contract) ──

#[test]
fn parse_secs_f64_bare_float_backward_compat() {
    assert_eq!(parse_secs_f64("45").unwrap(), 45.0);
    assert_eq!(parse_secs_f64("45.5").unwrap(), 45.5);
    assert_eq!(parse_secs_f64("0.5").unwrap(), 0.5);
    assert_eq!(parse_secs_f64("0").unwrap(), 0.0);
}

#[test]
fn parse_secs_f64_suffixed_units() {
    assert_eq!(parse_secs_f64("6s").unwrap(), 6.0);
    assert_eq!(parse_secs_f64("45s").unwrap(), 45.0);
    assert_eq!(parse_secs_f64("1m").unwrap(), 60.0);
    assert_eq!(parse_secs_f64("30m").unwrap(), 1800.0);
    assert_eq!(parse_secs_f64("1h").unwrap(), 3600.0);
    assert_eq!(parse_secs_f64("86400s").unwrap(), 86400.0);
    // Day/week units (S-master-HUNT-5): sub-ceiling day values parse.
    assert_eq!(parse_secs_f64("0.5d").unwrap(), 43_200.0);
    assert_eq!(parse_secs_f64("1d").unwrap(), 86_400.0);
    assert_eq!(parse_secs_f64("0.25d").unwrap(), 21_600.0);
}

#[test]
fn parse_secs_f64_24h_hard_ceiling() {
    // S-master-HUNT-5 (owner security mandate 2026-09-03): the f64
    // grammar (CLI --duration / --crystal-dragon-secs + the config
    // -secs keys via parse_secs_config) enforces the same 24h
    // ceiling as the bench grammar — structurally, inside the parser.
    assert_eq!(parse_secs_f64("24h").unwrap(), 86_400.0);
    assert!(parse_secs_f64("86400.5").is_err());
    for over in ["222h", "2d", "1w", "86401", "90000", "1.5d"] {
        let err = parse_secs_f64(over).unwrap_err();
        assert!(
            err.contains("24h (86400s) hard ceiling"),
            "{over} rejection must state the ceiling: {err}"
        );
        assert!(
            err.contains("resolves to"),
            "{over} rejection must show the resolved seconds: {err}"
        );
    }
}

#[test]
fn parse_secs_f64_calendar_units_rejected() {
    // mo/y are display units (clock::format_uptime_tiered), NOT input
    // grammar units — their calendar lengths are not fixed.
    let err = parse_secs_f64("1mo").unwrap_err();
    assert!(err.contains("unknown unit 'mo'"), "got: {err}");
    let err = parse_secs_f64("1y").unwrap_err();
    assert!(err.contains("unknown unit 'y'"), "got: {err}");
    // The unit hint lists the full accepted set.
    assert!(err.contains("s/m/h/d/w"), "hint must include d/w: {err}");
}

#[test]
fn parse_secs_f64_fractional_suffixed() {
    assert_eq!(parse_secs_f64("0.5s").unwrap(), 0.5);
    assert_eq!(parse_secs_f64("1.5m").unwrap(), 90.0);
    assert!((parse_secs_f64("0.25h").unwrap() - 900.0).abs() < 1e-9);
}

#[test]
fn parse_secs_f64_compound() {
    assert_eq!(parse_secs_f64("1h30m").unwrap(), 5400.0);
    assert_eq!(parse_secs_f64("2h15m30s").unwrap(), 8130.0);
    assert_eq!(parse_secs_f64("1m30s").unwrap(), 90.0);
    assert_eq!(parse_secs_f64("1h 30m").unwrap(), 5400.0);
}

#[test]
fn parse_secs_f64_long_units() {
    assert_eq!(parse_secs_f64("1min").unwrap(), 60.0);
    assert_eq!(parse_secs_f64("1hour").unwrap(), 3600.0);
    assert_eq!(parse_secs_f64("1minute").unwrap(), 60.0);
    assert_eq!(parse_secs_f64("1second").unwrap(), 1.0);
}

#[test]
fn parse_secs_f64_rejects_invalid() {
    assert!(parse_secs_f64("abc").is_err());
    assert!(parse_secs_f64("6x").is_err());
    assert!(parse_secs_f64("").is_err());
    assert!(parse_secs_f64("s").is_err());
    assert!(parse_secs_f64("1.2.3s").is_err());
}

#[test]
fn parse_secs_f64_rejects_negative_and_nonfinite() {
    assert!(parse_secs_f64("-5").is_err());
    assert!(parse_secs_f64("-5s").is_err());
    assert!(parse_secs_f64("inf").is_err());
    assert!(parse_secs_f64("NaN").is_err());
}

#[test]
fn parse_secs_f64_zero_is_valid() {
    // 0.0 is in-range for the dragon knobs (instant / degenerate poll);
    // range gates live at the flag/config layers.
    assert_eq!(parse_secs_f64("0").unwrap(), 0.0);
    assert_eq!(parse_secs_f64("0s").unwrap(), 0.0);
}

#[test]
fn parse_duration_seconds() {
    assert_eq!(parse_duration("--bench-duration", "6s").unwrap(), 6);
    assert_eq!(parse_duration("--bench-duration", "1s").unwrap(), 1);
    assert_eq!(parse_duration("--bench-duration", "100s").unwrap(), 100);
}

#[test]
fn parse_duration_minutes() {
    assert_eq!(parse_duration("--bench-duration", "1m").unwrap(), 60);
    assert_eq!(parse_duration("--bench-duration", "30m").unwrap(), 1800);
}

#[test]
fn parse_duration_hours() {
    assert_eq!(parse_duration("--bench-duration", "1h").unwrap(), 3600);
    assert_eq!(parse_duration("--bench-duration", "2h").unwrap(), 7200);
}

#[test]
fn parse_duration_compound() {
    assert_eq!(parse_duration("--bench-duration", "1h30m").unwrap(), 5400);
    assert_eq!(
        parse_duration("--bench-duration", "2h15m30s").unwrap(),
        8130
    );
    assert_eq!(parse_duration("--bench-duration", "1m30s").unwrap(), 90);
}

#[test]
fn parse_duration_long_units() {
    assert_eq!(parse_duration("--bench-duration", "1min").unwrap(), 60);
    assert_eq!(parse_duration("--bench-duration", "1hour").unwrap(), 3600);
    assert_eq!(parse_duration("--bench-duration", "1minute").unwrap(), 60);
    assert_eq!(parse_duration("--bench-duration", "1second").unwrap(), 1);
}

#[test]
fn parse_duration_rejects_zero() {
    assert!(parse_duration("--bench-duration", "0").is_err());
    assert!(parse_duration("--bench-duration", "0s").is_err());
}

#[test]
fn parse_duration_rejects_invalid() {
    assert!(parse_duration("--bench-duration", "abc").is_err());
    assert!(parse_duration("--bench-duration", "6x").is_err());
    assert!(parse_duration("--bench-duration", "").is_err());
    assert!(parse_duration("--bench-duration", "6").is_ok()); // bare number is valid
}

#[test]
fn parse_duration_24h_hard_ceiling() {
    // v80.0.0-alpha.1 S-master-HUNT-5 (owner security mandate
    // 2026-09-03): the old "no maximum cap — user responsibility"
    // contract (this test previously asserted 100h and ~1-year
    // durations parse) let `--bench-duration 222h` launch an
    // unbounded benchmark run. Every time-scale input is now
    // hard-capped at 24h; the rejection carries the policy reason.
    assert_eq!(
        parse_duration("--bench-duration", "24h").unwrap(),
        86_400,
        "exactly 24h is the ceiling itself — valid"
    );
    assert_eq!(
        parse_duration("--bench-duration", "1d").unwrap(),
        86_400,
        "day unit at the ceiling — valid"
    );
    for over in ["25h", "222h", "2d", "1w", "86401", "8784h"] {
        let err = parse_duration("--bench-duration", over).unwrap_err();
        assert!(
            err.contains("24h (86400s) hard ceiling"),
            "{over} rejection must state the ceiling: {err}"
        );
        assert!(
            err.contains("courteous-guest policy"),
            "{over} rejection must state the OS-protection reason: {err}"
        );
    }
}

#[test]
fn parse_duration_error_message_uses_correct_flag_label() {
    // Error messages must attribute failure to the actual flag the user
    // passed, not a hardcoded "--duration". This was a regression where
    // `--bench-duration foo` produced "error: --duration 'foo'...".
    let err = parse_duration("--bench-duration", "abc").unwrap_err();
    assert!(
        err.contains("--bench-duration"),
        "expected error to mention --bench-duration, got: {err}"
    );
    assert!(
        !err.contains("--duration '"),
        "error should not mention bare --duration, got: {err}"
    );

    let err = parse_duration("--duration", "6x").unwrap_err();
    assert!(
        err.contains("--duration"),
        "expected error to mention --duration, got: {err}"
    );
}

// ── parse_screen_size tests ─────────────────────────────────────────

#[test]
fn parse_screen_size_basic() {
    assert_eq!(parse_screen_size("120x40").unwrap(), (120, 40));
    assert_eq!(parse_screen_size("12x12").unwrap(), (12, 12));
    // 1x1 is the minimum accepted (MIN_TERMINAL_COLS x MIN_TERMINAL_LINES = 1x1)
    assert!(parse_screen_size("1x1").is_ok());
}

#[test]
fn parse_screen_size_case_insensitive_x() {
    assert_eq!(parse_screen_size("200X60").unwrap(), (200, 60));
    assert_eq!(parse_screen_size("80X24").unwrap(), (80, 24));
}

#[test]
fn parse_screen_size_with_spaces() {
    assert_eq!(parse_screen_size(" 120x40 ").unwrap(), (120, 40));
    assert_eq!(parse_screen_size("120 x 40").unwrap(), (120, 40));
}

#[test]
fn parse_screen_size_rejects_zero() {
    assert!(parse_screen_size("0x0").is_err());
    assert!(parse_screen_size("0x10").is_err());
    assert!(parse_screen_size("10x0").is_err());
}

#[test]
fn parse_screen_size_rejects_too_small() {
    // Minimum is 1x1 (MIN_TERMINAL_COLS x MIN_TERMINAL_LINES = 1)
    // 0 is rejected (below minimum), 1 is accepted.
    assert!(parse_screen_size("0x0").is_err());
    assert!(parse_screen_size("0x1").is_err());
    assert!(parse_screen_size("1x0").is_err());
    // 1x1 is the minimum accepted
    assert!(parse_screen_size("1x1").is_ok());
}

#[test]
fn parse_screen_size_rejects_invalid_format() {
    assert!(parse_screen_size("120").is_err());
    assert!(parse_screen_size("120x").is_err());
    assert!(parse_screen_size("x40").is_err());
    assert!(parse_screen_size("120x40x30").is_err());
    assert!(parse_screen_size("abc").is_err());
}

#[test]
fn parse_screen_size_rejects_non_numeric() {
    assert!(parse_screen_size("abcx40").is_err());
    assert!(parse_screen_size("120xabc").is_err());
}

#[test]
fn parse_screen_size_large_values() {
    assert_eq!(parse_screen_size("65535x65535").unwrap(), (65535, 65535));
    assert_eq!(parse_screen_size("1000x1000").unwrap(), (1000, 1000));
}

// ── depth-test: --charset-custom alias tests ────────────────

/// depth-test fix: --charset-custom is now an alias for --charset.
/// Depth-test user expected `--charset-custom cat` to work by analogy
/// with --colors-custom and --scene-custom. The existing --charset
/// already handles BOTH built-in presets AND custom names, so the alias
/// is pure UX parity. These tests verify clap accepts both forms and
/// routes them to the same `charset` field.
#[test]
fn charset_custom_alias_resolves_to_charset_field() {
    use crate::config::Args;
    use clap::Parser;
    // Long form --charset-custom
    let args = Args::try_parse_from(["cosmostrix", "--charset-custom", "cat"]).unwrap();
    assert_eq!(args.charset, "cat");
}

#[test]
fn charset_long_form_still_works() {
    use crate::config::Args;
    use clap::Parser;
    // Original --charset long form must still work after alias addition.
    let args = Args::try_parse_from(["cosmostrix", "--charset", "hex"]).unwrap();
    assert_eq!(args.charset, "hex");
}

#[test]
fn charset_short_form_still_works() {
    use crate::config::Args;
    use clap::Parser;
    // Short -C form must still work.
    let args = Args::try_parse_from(["cosmostrix", "-C", "binary"]).unwrap();
    assert_eq!(args.charset, "binary");
}

// ── v80.0.0-alpha.1: --crystal-dragon-secs flag surface ───────────

/// The flag parses as Option<f64> with no clap-level range gate (the
/// range is enforced by the same startup validate_f64_range path as
/// --duration, which main.rs owns — clap value_parser f64 range is
/// intentionally not duplicated here to keep one error-message voice).
/// v80.0.0-alpha.2: the clap value_parser IS parse_secs_f64, so the
/// human forms (6s, 1m, 1h30m, 45.5s) resolve at parse time — the field
/// type stays Option<f64>, all downstream consumers unchanged.
#[test]
fn crystal_dragon_secs_parses_from_cli() {
    use crate::config::Args;
    use clap::Parser;
    let args = Args::try_parse_from(["cosmostrix", "--crystal-dragon-secs", "120"]).unwrap();
    assert_eq!(args.crystal_dragon_secs, Some(120.0));
    // Fractional values are accepted (float flag).
    let args = Args::try_parse_from(["cosmostrix", "--crystal-dragon-secs", "45.5"]).unwrap();
    assert_eq!(args.crystal_dragon_secs, Some(45.5));
    // Default: unset (None) → engine uses the 60s constant.
    let args = Args::try_parse_from(["cosmostrix"]).unwrap();
    assert_eq!(args.crystal_dragon_secs, None);
}

// ── v80.0.0-alpha.2: human duration forms on every seconds flag ───

#[test]
fn crystal_dragon_secs_accepts_human_duration_forms() {
    use crate::config::Args;
    use clap::Parser;
    let cases = [
        ("6s", 6.0),
        ("45s", 45.0),
        ("1m", 60.0),
        ("15s", 15.0),
        ("30m", 1800.0),
        ("1h", 3600.0),
        ("1h30m", 5400.0),
        ("45.5s", 45.5),
        ("90", 90.0),
    ];
    for (input, expect) in cases {
        let args = Args::try_parse_from(["cosmostrix", "--crystal-dragon-secs", input]).unwrap();
        assert_eq!(
            args.crystal_dragon_secs,
            Some(expect),
            "--crystal-dragon-secs {input} must parse to {expect}"
        );
    }
}

#[test]
fn crystal_dragon_secs_rejects_invalid_human_duration() {
    use crate::config::Args;
    use clap::Parser;
    for bad in ["6x", "abc", "-5s", "1.2.3s"] {
        let err = Args::try_parse_from(["cosmostrix", "--crystal-dragon-secs", bad]);
        assert!(
            err.is_err(),
            "--crystal-dragon-secs {bad} must be rejected at parse time"
        );
    }
}

#[test]
fn duration_flag_accepts_human_duration_forms() {
    use crate::config::Args;
    use clap::Parser;
    let cases = [
        ("5", 5.0),
        ("0.5", 0.5),
        ("5s", 5.0),
        ("1m", 60.0),
        ("1h30m", 5400.0),
    ];
    for (input, expect) in cases {
        let args = Args::try_parse_from(["cosmostrix", "--duration", input]).unwrap();
        assert_eq!(
            args.duration,
            Some(expect),
            "--duration {input} must parse to {expect}"
        );
    }
}
