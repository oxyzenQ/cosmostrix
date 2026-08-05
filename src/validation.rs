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
/// that points the user to its replacement.
///
/// The matcher in [`check_removed_flags`] uses exact token equality
/// (`token == *flag`), so the order of entries in this table does not affect
/// matching — `--preset` and `--list-presets` are distinct tokens and never
/// collide. Entries are kept in a roughly longest-first order for human
/// readability when scanning the table.
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
    (
        "--mouse",
        "error: --mouse has been removed in v17.0.0.\n  Mouse hover/click visual effects are now ALWAYS ON (cursor glow + dual-ring click wave).\n  Mouse reporting is also always active (blocks text selection).\n  No flag needed — the effect is part of cosmostrix's signature interactive experience.",
    ),
    (
        "--charset-file",
        "error: --charset-file has been removed in v25.0.0.\n  Custom charsets now live in config.toml under [charset-custom.<name>] and are loaded via --charset <name>.\n  Migration: move your custom characters from the file into a [charset-custom.<name>] block, then activate with --charset <name> or `charset = \"<name>\"` in config.\n  Example:\n    [charset-custom.zen]\n    set = \"|\"\n  Then: cosmostrix --charset zen\n  See `cosmostrix --dump-config` for the full template.",
    ),
    (
        "--chars",
        "error: --chars has been removed (audit FLAGS_AUDIT_bench-frames_chars_bold.md §2).\n  Custom charsets now exclusively come from config.toml under [charset-custom.<name>] and are loaded via --charset <name>.\n  Migration: --chars accepted hex Unicode ranges (e.g. \"0x30-0x39,0x41-0x5A\"). The [charset-custom.<name>] block accepts literal characters directly in the `set` field — TOML is UTF-8 native, so you can type the actual characters you want.\n  Example: --chars \"0x30-0x39,0x41-0x5A\" becomes\n    [charset-custom.my-range]\n    set = \"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ\"\n  Then: cosmostrix --charset my-range\n  See `cosmostrix --dump-config` for the full template.",
    ),
    (
        "--completions",
        "error: --completions <shell> has been removed in v15.0.0.\n  Shell completion scripts are no longer shipped. The `clap_complete` dependency was dropped to reduce maintenance surface.\n  To regenerate completions externally, use `clap_complete` in a downstream tool, or write them by hand from `cosmostrix --help`.",
    ),
    (
        "--fullwidth",
        "error: --fullwidth has been removed in v25.0.0-alpha.3.\n  The legacy horizontal-spacing mode (which doubled the column stride for monolith streams) was purged.\n  The Cosmic Dragon principle forbids wide chars permanently; the charset is always single-width.\n  No replacement needed — monolith streams now render at the natural single-cell stride, which is the only mode that has ever been the default.",
    ),
    (
        "--noglitch",
        "error: --noglitch has been removed in v30.0.0-alpha.1.\n  It was a strict duplicate of `--glitch-level none` (the only behavior --noglitch\n  had was to disable glitch, which is exactly what `--glitch-level none` does).\n  Use `--glitch-level none` instead. Glitch parameters are now fully owned by\n  --glitch-level (the documented contract from configfile.rs is now enforced).",
    ),
    (
        "--help-detail",
        "error: --help-detail has been removed in v30.0.0-alpha.1.\n  The curated advanced reference manual that --help-detail used to print is now\n  printed by --help itself. cosmostrix now has a single-tier help surface.\n  Use `cosmostrix --help` instead.",
    ),
    (
        "--check-bitcolor",
        "error: --check-bitcolor has been removed in v30.0.0-alpha.1.\n  It was a strict subset of `--doctor` output — every field it printed\n  (COLORTERM, TERM, auto_detected, forced, effective color depth) is already\n  shown by `cosmostrix --doctor` under the RENDERER and COLOR sections, plus\n  much more (terminal caps, env, perf hints, config paths).\n  Use `cosmostrix --doctor` instead.",
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
pub(crate) fn check_removed_flags(argv: &[OsString]) -> Result<(), String> {
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
        // Exact-match lookup: token is normalized to "--flag" form above
        // (split on '='), and REMOVED_FLAGS contains only exact flag names.
        // No prefix matching, so order does not affect correctness.
        for (flag, message) in REMOVED_FLAGS {
            if token == *flag {
                return Err((*message).to_string());
            }
        }
    }
    Ok(())
}

/// Validate that a `f64` value is finite and within `[min, max]`.
pub(crate) fn validate_f64_range(name: &str, v: f64, min: f64, max: f64) -> Result<f64, String> {
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
pub(crate) fn validate_speed(v: f32) -> Result<f32, String> {
    validate_f32_range("--speed", v, SPEED_MIN, SPEED_MAX)
}

pub(crate) fn parse_canonical_speed(name: &str, raw: &str) -> Result<f32, String> {
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

pub(crate) fn parse_canonical_u8_range(
    name: &str,
    raw: &str,
    min: u8,
    max: u8,
) -> Result<u8, String> {
    let value = parse_canonical_u32_range(name, raw, min as u32, max as u32)?;
    Ok(value as u8)
}

pub(crate) fn parse_canonical_u32_range(
    name: &str,
    raw: &str,
    min: u32,
    max: u32,
) -> Result<u32, String> {
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

pub(crate) fn parse_canonical_f32_range(
    name: &str,
    raw: &str,
    min: f32,
    max: f32,
) -> Result<f32, String> {
    if !is_canonical_decimal(raw) {
        return Err(expected_range_error(name, raw, min, max));
    }
    let value = raw
        .parse::<f32>()
        .map_err(|_| expected_range_error(name, raw, min, max))?;
    validate_f32_range(name, value, min, max).map_err(|_| expected_range_error(name, raw, min, max))
}

pub(crate) fn parse_canonical_f64_range(
    name: &str,
    raw: &str,
    min: f64,
    max: f64,
) -> Result<f64, String> {
    if !is_canonical_decimal(raw) {
        return Err(expected_range_error(name, raw, min, max));
    }
    let value = raw
        .parse::<f64>()
        .map_err(|_| expected_range_error(name, raw, min, max))?;
    validate_f64_range(name, value, min, max).map_err(|_| expected_range_error(name, raw, min, max))
}

pub(crate) fn prevalidate_cli_args(argv: &[OsString]) -> Result<(), String> {
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
                max: 300.0,
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
        // v17 mastery: --glitchpct, --shortpct, --rippct, --maxdpc CLI flags
        // REMOVED. Use --glitch-level instead. Removed from prevalidator.
        "--monolith-size" => CliSpec {
            name: "--monolith-size",
            kind: CliKind::Enum {
                allowed: &["small", "normal", "large"],
            },
        },
        "--color-bg" => CliSpec {
            name: "--color-bg",
            kind: CliKind::Enum {
                // Phase 5 (P2-5): accept "default_background" (snake_case) as
                // an alias to match config.toml behavior. The clap ValueEnum
                // in config.rs:106 also accepts both forms.
                allowed: &["black", "default-background", "default_background"],
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

pub(crate) fn range_error(
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
pub(crate) fn validate_f32_range(name: &str, v: f32, min: f32, max: f32) -> Result<f32, String> {
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
pub(crate) fn validate_u8_range(name: &str, v: u8, min: u8, max: u8) -> Result<u8, String> {
    if v < min || v > max {
        return Err(range_error(name, v, min, max));
    }
    Ok(v)
}

/// Validate that a `u16` value is within `[min, max]`.
pub(crate) fn validate_u16_range(name: &str, v: u16, min: u16, max: u16) -> Result<u16, String> {
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
        // v17 mastery: --maxdpc removed from CLI. Replaced with --fps edge case.
        let cases = [
            ("--fps", "0", "expected: number in range 1..=300"),
            ("--density", "nope", "expected: number in range 0.01..=5"),
            ("--fps", "500", "expected: number in range 1..=300"),
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
        // v25.10 audit: --completions was removed in v15 but was missing from
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
}
