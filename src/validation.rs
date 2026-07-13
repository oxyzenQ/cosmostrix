// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Result-returning argument validation functions.
//!
//! Replaces the old `require_*_range` helpers with proper `Result`-returning
//! functions so that validation errors can be propagated without `process::exit`.

use std::ffi::OsString;

use crate::constants::{DENSITY_CLAMP_MAX, SPEED_MAX, SPEED_MIN};

/// Migration map for CLI flags removed in v14.0.0.
///
/// Each entry maps a removed long-flag name to a single-line migration message
/// that points the user to its replacement. Order matters: longer flag names
/// (e.g. `--list-presets`) appear before shorter ones (e.g. `--low-power`) so
/// the `starts_with`-based matcher in [`check_removed_flags`] picks the most
/// specific match first.
const REMOVED_FLAGS: &[(&str, &str)] = &[
    (
        "--list-presets",
        "error: --list-presets has been removed in v14.0.0.\n  Use --list-scenes to see all built-in and custom scenes.",
    ),
    (
        "--list-profiles",
        "error: --list-profiles has been removed in v14.0.0.\n  Use --list-scenes to see all built-in and custom scenes.",
    ),
    (
        "--list-colors-detail",
        "error: --list-colors-detail has been removed in v14.0.0.\n  Use --list-colors to see all color themes.",
    ),
    (
        "--show-preset",
        "error: --show-preset has been removed in v14.0.0.\n  Use --show-scene <name> to preview a built-in or custom scene.",
    ),
    (
        "--dump-profile",
        "error: --dump-profile has been removed in v14.0.0.\n  Use --show-scene <name> to display a custom scene's configuration.",
    ),
    (
        "--tune-visual",
        "error: --tune-visual has been removed in v14.0.0.\n  Use --benchmark for performance measurement.",
    ),
    (
        "--defaults",
        "error: --defaults has been removed in v14.0.0.\n  Use --dump-config to see the default configuration template.",
    ),
    (
        "--low-power",
        "error: --low-power has been removed in v14.0.0.\n  Use --scene low-power instead.",
    ),
    (
        "--preset",
        "error: --preset has been removed in v14.0.0.\n  Use --scene <name> instead. All former presets (classic, cinematic, calm, monolith, storm, cosmos, neon, hacker, low-power) are now built-in scenes. Run --list-scenes to see them.",
    ),
    (
        "--profile",
        "error: --profile has been removed in v14.0.0.\n  Use --scene-custom <name> instead. Rename [profile.<name>] to [scene-custom.<name>] in config.toml (prefix-only rename — fields are identical).",
    ),
];

/// Scan raw argv for any flag removed in v14.0.0 and return a migration error.
///
/// This runs before clap parsing so we can intercept the removed flag with a
/// clear, actionable message rather than letting clap report it as an
/// "unexpected argument". The matcher accepts both `--flag value` and
/// `--flag=value` forms because we only inspect the flag token itself.
///
/// Returns `Ok(())` if no removed flag is found, or `Err(message)` with the
/// migration hint for the first match. The check is case-sensitive on the
/// long-flag prefix (clap long-flags are always lowercase).
pub fn check_removed_flags(argv: &[OsString]) -> Result<(), String> {
    for arg in argv.iter().skip(1) {
        let Some(s) = arg.to_str() else {
            continue;
        };
        // Normalize `--flag=value` to `--flag` for matching purposes.
        let token = s.split_once('=').map_or(s, |(flag, _)| flag);
        // Skip non-flag tokens (positional values, etc.).
        if !token.starts_with("--") {
            continue;
        }
        // Longest-match-first: REMOVED_FLAGS is ordered so multi-word flags
        // (--list-presets) are checked before single-word ones (--preset).
        for (flag, message) in REMOVED_FLAGS {
            if token == *flag {
                return Err((*message).to_string());
            }
        }
    }
    Ok(())
}

/// Validate that a `f64` value is finite and within `[min, max]`.
pub fn validate_f64_range(name: &str, v: f64, min: f64, max: f64) -> Result<f64, String> {
    if !v.is_finite() {
        return Err(format!(
            "error: invalid value for {name}: {v}\nexpected a finite number"
        ));
    }
    if v < min || v > max {
        return Err(range_error(name, v, min, max));
    }
    Ok(v)
}

/// Validate user-facing rain speed.
pub fn validate_speed(v: f32) -> Result<f32, String> {
    validate_f32_range("--speed", v, SPEED_MIN, SPEED_MAX)
}

pub fn parse_canonical_speed(name: &str, raw: &str) -> Result<f32, String> {
    let min = SPEED_MIN as u32;
    let max = SPEED_MAX as u32;
    if !is_canonical_integer(raw) {
        return Err(expected_canonical_integer_error(name, raw, min, max));
    }
    let value = raw
        .parse::<u32>()
        .map_err(|_| expected_canonical_integer_error(name, raw, min, max))?;
    if value < min || value > max {
        return Err(expected_canonical_integer_error(name, raw, min, max));
    }
    Ok(value as f32)
}

pub fn parse_canonical_u8_range(name: &str, raw: &str, min: u8, max: u8) -> Result<u8, String> {
    let value = parse_canonical_u32_range(name, raw, min as u32, max as u32)?;
    Ok(value as u8)
}

pub fn parse_canonical_u32_range(name: &str, raw: &str, min: u32, max: u32) -> Result<u32, String> {
    if !is_canonical_integer(raw) {
        return Err(expected_range_error(name, raw, min, max));
    }
    let value = raw
        .parse::<u32>()
        .map_err(|_| expected_range_error(name, raw, min, max))?;
    if value < min || value > max {
        return Err(expected_range_error(name, raw, min, max));
    }
    Ok(value)
}

pub fn parse_canonical_f32_range(name: &str, raw: &str, min: f32, max: f32) -> Result<f32, String> {
    if !is_canonical_decimal(raw) {
        return Err(expected_range_error(name, raw, min, max));
    }
    let value = raw
        .parse::<f32>()
        .map_err(|_| expected_range_error(name, raw, min, max))?;
    validate_f32_range(name, value, min, max).map_err(|_| expected_range_error(name, raw, min, max))
}

pub fn parse_canonical_f64_range(name: &str, raw: &str, min: f64, max: f64) -> Result<f64, String> {
    if !is_canonical_decimal(raw) {
        return Err(expected_range_error(name, raw, min, max));
    }
    let value = raw
        .parse::<f64>()
        .map_err(|_| expected_range_error(name, raw, min, max))?;
    validate_f64_range(name, value, min, max).map_err(|_| expected_range_error(name, raw, min, max))
}

pub fn prevalidate_cli_args(argv: &[OsString]) -> Result<(), String> {
    // Stage 4b: intercept flags removed in v14.0.0 with migration hints.
    // This runs before any other validation so users see the migration
    // message instead of clap's generic "unexpected argument" error.
    check_removed_flags(argv)?;

    let mut idx = 1usize;
    while idx < argv.len() {
        let Some(arg) = argv[idx].to_str() else {
            idx += 1;
            continue;
        };
        if let Some((flag, value)) = arg.split_once('=') {
            validate_cli_value(flag, value)?;
            idx += 1;
            continue;
        }
        if let Some(spec) = cli_spec(arg) {
            let Some(value) = argv.get(idx + 1).and_then(|v| v.to_str()) else {
                return Ok(());
            };
            validate_cli_value(spec.name, value)?;
            idx += 2;
            continue;
        }
        idx += 1;
    }
    Ok(())
}

fn validate_cli_value(flag: &str, value: &str) -> Result<(), String> {
    let Some(spec) = cli_spec(flag) else {
        return Ok(());
    };
    match spec.kind {
        CliKind::Integer { min, max } => {
            parse_canonical_u32_range(spec.name, value, min, max).map(|_| ())
        }
        CliKind::Speed => parse_canonical_speed(spec.name, value).map(|_| ()),
        CliKind::DecimalF32 { min, max } => {
            parse_canonical_f32_range(spec.name, value, min, max).map(|_| ())
        }
        CliKind::DecimalF64 { min, max } => {
            parse_canonical_f64_range(spec.name, value, min, max).map(|_| ())
        }
        CliKind::Enum { allowed } => validate_enum_value(spec.name, value, allowed),
    }
}

fn validate_enum_value(name: &str, raw: &str, allowed: &[&str]) -> Result<(), String> {
    if allowed.iter().any(|value| raw.eq_ignore_ascii_case(value)) {
        Ok(())
    } else {
        Err(format!(
            "error: invalid value for {name}: {raw}\nexpected one of: {}",
            allowed.join(", ")
        ))
    }
}

#[derive(Clone, Copy)]
struct CliSpec {
    name: &'static str,
    kind: CliKind,
}

#[derive(Clone, Copy)]
enum CliKind {
    Speed,
    Integer { min: u32, max: u32 },
    DecimalF32 { min: f32, max: f32 },
    DecimalF64 { min: f64, max: f64 },
    Enum { allowed: &'static [&'static str] },
}

fn cli_spec(flag: &str) -> Option<CliSpec> {
    let spec = match flag {
        "--fps" | "-f" => CliSpec {
            name: "--fps",
            kind: CliKind::DecimalF64 {
                min: 1.0,
                max: 240.0,
            },
        },
        "--speed" | "-S" => CliSpec {
            name: "--speed",
            kind: CliKind::Speed,
        },
        "--density" | "-d" => CliSpec {
            name: "--density",
            kind: CliKind::DecimalF32 {
                min: 0.01,
                max: DENSITY_CLAMP_MAX,
            },
        },
        "--duration" => CliSpec {
            name: "--duration",
            kind: CliKind::DecimalF64 {
                min: 0.1,
                max: 86400.0,
            },
        },
        "--glitchpct" => CliSpec {
            name: "--glitchpct",
            kind: CliKind::DecimalF32 {
                min: 0.0,
                max: 100.0,
            },
        },
        "--shortpct" => CliSpec {
            name: "--shortpct",
            kind: CliKind::DecimalF32 {
                min: 0.0,
                max: 100.0,
            },
        },
        "--rippct" | "-r" => CliSpec {
            name: "--rippct",
            kind: CliKind::DecimalF32 {
                min: 0.0,
                max: 100.0,
            },
        },
        "--maxdpc" => CliSpec {
            name: "--maxdpc",
            kind: CliKind::Integer { min: 1, max: 3 },
        },
        "--monolith-size" => CliSpec {
            name: "--monolith-size",
            kind: CliKind::Enum {
                allowed: &["small", "normal", "large"],
            },
        },
        "--color-bg" => CliSpec {
            name: "--color-bg",
            kind: CliKind::Enum {
                allowed: &["black", "default-background"],
            },
        },
        "--glitch-level" => CliSpec {
            name: "--glitch-level",
            kind: CliKind::Enum {
                allowed: &["none", "subtle", "default", "intense"],
            },
        },
        _ => return None,
    };
    Some(spec)
}

fn is_canonical_integer(raw: &str) -> bool {
    if raw.is_empty() || raw.starts_with(['+', '-']) {
        return false;
    }
    if raw.len() > 1 && raw.starts_with('0') {
        return false;
    }
    raw.bytes().all(|b| b.is_ascii_digit())
}

fn is_canonical_decimal(raw: &str) -> bool {
    if raw.is_empty() || raw.starts_with(['+', '-']) {
        return false;
    }
    if raw.eq_ignore_ascii_case("nan") || raw.eq_ignore_ascii_case("inf") {
        return false;
    }
    let Some((whole, frac)) = raw.split_once('.') else {
        return is_canonical_integer(raw);
    };
    if frac.is_empty() || !frac.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    if whole == "0" {
        return true;
    }
    is_canonical_integer(whole)
}

fn expected_range_error(
    name: &str,
    value: &str,
    min: impl std::fmt::Display,
    max: impl std::fmt::Display,
) -> String {
    format!(
        "error: invalid value for {name}: {value}\nexpected: number in range {}",
        format_range(min, max)
    )
}

fn expected_canonical_integer_error(
    name: &str,
    value: &str,
    min: impl std::fmt::Display,
    max: impl std::fmt::Display,
) -> String {
    format!(
        "error: invalid value for {name}: {value}\nexpected: canonical integer in range {}",
        format_range(min, max)
    )
}

fn format_number<T: std::fmt::Display>(value: T) -> String {
    format!("{value}")
}

fn format_range(min: impl std::fmt::Display, max: impl std::fmt::Display) -> String {
    format!("{}..={}", format_number(min), format_number(max))
}

pub fn range_error(
    name: &str,
    value: impl std::fmt::Display,
    min: impl std::fmt::Display,
    max: impl std::fmt::Display,
) -> String {
    format!(
        "error: invalid value for {name}: {value}\nallowed range: {}",
        format_range(min, max)
    )
}

/// Validate that a `f32` value is finite and within `[min, max]`.
pub fn validate_f32_range(name: &str, v: f32, min: f32, max: f32) -> Result<f32, String> {
    if !v.is_finite() {
        return Err(format!(
            "error: invalid value for {name}: {v}\nexpected a finite number"
        ));
    }
    if v < min || v > max {
        return Err(range_error(name, v, min, max));
    }
    Ok(v)
}

/// Validate that a `u8` value is within `[min, max]`.
pub fn validate_u8_range(name: &str, v: u8, min: u8, max: u8) -> Result<u8, String> {
    if v < min || v > max {
        return Err(range_error(name, v, min, max));
    }
    Ok(v)
}

/// Validate that a `u16` value is within `[min, max]`.
pub fn validate_u16_range(name: &str, v: u16, min: u16, max: u16) -> Result<u16, String> {
    if v < min || v > max {
        return Err(range_error(name, v, min, max));
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
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
        let cases = [
            ("--fps", "0", "expected: number in range 1..=240"),
            ("--density", "nope", "expected: number in range 0.01..=5"),
            ("--maxdpc", "4", "expected: number in range 1..=3"),
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
}
