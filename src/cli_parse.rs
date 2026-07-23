// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! CLI parsing helpers for `--duration` and `--screen-size` flags.
//!
//! Extracted from config.rs to keep that file under its LOC guard.
//! These are pure functions — parse + validate, no side effects.

/// Minimum benchmark duration: 1 second.
const DURATION_MIN_SECS: u64 = 1;

/// Parse a human-readable duration string into total seconds.
///
/// Accepted formats (compound supported):
///   - `6s` → 6 seconds
///   - `30m` → 1800 seconds
///   - `1h` → 3600 seconds
///   - `1h30m` → 5400 seconds (compound)
///   - `2h15m30s` → 8130 seconds (full compound)
///   - `90` (bare number) → 90 seconds (backward compat)
///
/// Minimum: 1 second. No maximum cap (user responsibility for long runs).
///
/// # Errors
/// Returns `Err(String)` with a human-readable error message if:
///   - Format is invalid (unrecognized unit, missing number)
///   - Value is zero or below minimum
pub fn parse_duration(input: &str) -> Result<u64, String> {
    let input = input.trim();

    // Bare number → seconds (backward compat with --bench-duration)
    if let Ok(n) = input.parse::<u64>() {
        return validate_secs(n);
    }

    // Compound format: parse <N><unit> pairs
    let mut total_secs: u64 = 0;
    let mut chars = input.chars().peekable();
    let mut found_any = false;

    while chars.peek().is_some() {
        // Skip whitespace between components
        while matches!(chars.peek(), Some(c) if c.is_whitespace()) {
            chars.next();
        }
        if chars.peek().is_none() {
            break;
        }

        // Parse number
        let mut num_str = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() {
                num_str.push(c);
                chars.next();
            } else {
                break;
            }
        }
        if num_str.is_empty() {
            return Err(format!(
                "error: --duration '{input}' has invalid format (expected number before unit)"
            ));
        }
        let num: u64 = num_str
            .parse()
            .map_err(|_| format!("error: --duration '{input}' has number too large"))?;

        // Parse unit
        let mut unit_str = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_ascii_alphabetic() {
                unit_str.push(c);
                chars.next();
            } else {
                break;
            }
        }
        if unit_str.is_empty() {
            return Err(format!(
                "error: --duration '{input}' missing unit after {num_str} (use s/m/h)"
            ));
        }

        let multiplier = match unit_str.as_str() {
            "s" | "sec" | "secs" | "second" | "seconds" => 1u64,
            "m" | "min" | "mins" | "minute" | "minutes" => 60u64,
            "h" | "hr" | "hrs" | "hour" | "hours" => 3600u64,
            other => {
                return Err(format!(
                    "error: --duration '{input}' has unknown unit '{other}' (use s/m/h)"
                ));
            }
        };

        total_secs = total_secs.saturating_add(num.saturating_mul(multiplier));
        found_any = true;
    }

    if !found_any {
        return Err(format!(
            "error: --duration '{input}' is empty or invalid (use format like 6s, 30m, 1h30m)"
        ));
    }

    validate_secs(total_secs)
}

fn validate_secs(secs: u64) -> Result<u64, String> {
    if secs < DURATION_MIN_SECS {
        return Err(format!(
            "error: --duration {secs}s is below the {DURATION_MIN_SECS}-second minimum"
        ));
    }
    Ok(secs)
}

/// Parsed screen size: (width, height).
pub type ScreenSize = (u16, u16);

/// Parse a screen size string `WxH` into `(width, height)`.
///
/// Accepted formats:
///   - `120x40` → (120, 40)
///   - `12x12` → (12, 12)
///   - `4x4` → (4, 4) (minimum, enforced by MIN_TERMINAL_COLS/LINES)
///   - `200X60` → (200, 60) (case-insensitive 'x')
///
/// Format range: 1x1 to 65535x65535 (u16 range). However, the renderer
/// enforces a stricter floor of MIN_TERMINAL_COLS × MIN_TERMINAL_LINES
/// (4×4) — sizes below this are rejected at parse time with a clear
/// error. The renderer also clamps to a per-mode ceiling at runtime:
///   - Interactive mode: MAX_TERMINAL_COLS × MAX_TERMINAL_LINES (1024×500)
///   - Benchmark mode:   BENCH_MAX_COLS × BENCH_MAX_LINES (7680×4320 = 8K UHD)
///
/// # Errors
/// Returns `Err(String)` with a human-readable error message if:
///   - Format is invalid (missing 'x', non-numeric, extra characters)
///   - Value is below minimum (0x0, 0x10, 10x0, or below 4x4)
pub fn parse_screen_size(input: &str) -> Result<ScreenSize, String> {
    let input = input.trim();

    // Split on 'x' or 'X' (case-insensitive)
    let parts: Vec<&str> = input.split(['x', 'X']).collect();
    if parts.len() != 2 {
        return Err(format!(
            "error: --screen-size '{input}' is invalid (expected format WxH, e.g. 120x40)"
        ));
    }

    let w: u16 = parts[0].trim().parse().map_err(|_| {
        format!(
            "error: --screen-size '{input}' has invalid width '{}' (expected number 1-65535)",
            parts[0].trim()
        )
    })?;
    let h: u16 = parts[1].trim().parse().map_err(|_| {
        format!(
            "error: --screen-size '{input}' has invalid height '{}' (expected number 1-65535)",
            parts[1].trim()
        )
    })?;

    if w == 0 || h == 0 {
        return Err(format!(
            "error: --screen-size '{input}' has a zero dimension (got {w}x{h}, both must be ≥ 1)"
        ));
    }

    // Strict minimum: cosmostrix needs at least MIN_TERMINAL_COLS x
    // MIN_TERMINAL_LINES to render meaningfully. Smaller sizes cause
    // silent exit (no visible rain, degenerate cloud state).
    // Reject at parse time so the user gets a clear error instead of
    // a silent exit with code 0.
    let min_cols = crate::constants::MIN_TERMINAL_COLS;
    let min_lines = crate::constants::MIN_TERMINAL_LINES;
    if w < min_cols || h < min_lines {
        return Err(format!(
            "error: --screen-size {w}x{h} is too small (minimum {min_cols}x{min_lines})"
        ));
    }

    Ok((w, h))
}

/// Parse optional screen size string. None → None (dynamic mode).
/// Some(s) → parse + validate.
pub fn parse_screen_size_optional(input: &Option<String>) -> Result<Option<ScreenSize>, String> {
    match input {
        None => Ok(None),
        Some(s) => parse_screen_size(s).map(Some),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_duration tests ────────────────────────────────────────────

    #[test]
    fn parse_duration_bare_number_is_seconds() {
        assert_eq!(parse_duration("5").unwrap(), 5);
        assert_eq!(parse_duration("90").unwrap(), 90);
    }

    #[test]
    fn parse_duration_seconds() {
        assert_eq!(parse_duration("6s").unwrap(), 6);
        assert_eq!(parse_duration("1s").unwrap(), 1);
        assert_eq!(parse_duration("100s").unwrap(), 100);
    }

    #[test]
    fn parse_duration_minutes() {
        assert_eq!(parse_duration("1m").unwrap(), 60);
        assert_eq!(parse_duration("30m").unwrap(), 1800);
    }

    #[test]
    fn parse_duration_hours() {
        assert_eq!(parse_duration("1h").unwrap(), 3600);
        assert_eq!(parse_duration("2h").unwrap(), 7200);
    }

    #[test]
    fn parse_duration_compound() {
        assert_eq!(parse_duration("1h30m").unwrap(), 5400);
        assert_eq!(parse_duration("2h15m30s").unwrap(), 8130);
        assert_eq!(parse_duration("1m30s").unwrap(), 90);
    }

    #[test]
    fn parse_duration_long_units() {
        assert_eq!(parse_duration("1min").unwrap(), 60);
        assert_eq!(parse_duration("1hour").unwrap(), 3600);
        assert_eq!(parse_duration("1minute").unwrap(), 60);
        assert_eq!(parse_duration("1second").unwrap(), 1);
    }

    #[test]
    fn parse_duration_rejects_zero() {
        assert!(parse_duration("0").is_err());
        assert!(parse_duration("0s").is_err());
    }

    #[test]
    fn parse_duration_rejects_invalid() {
        assert!(parse_duration("abc").is_err());
        assert!(parse_duration("6x").is_err());
        assert!(parse_duration("").is_err());
        assert!(parse_duration("6").is_ok()); // bare number is valid
    }

    #[test]
    fn parse_duration_no_max_cap() {
        // No maximum cap — user can specify very long durations
        assert_eq!(parse_duration("100h").unwrap(), 360000);
        assert_eq!(parse_duration("8784h").unwrap(), 31622400); // ~1 year
    }

    // ── parse_screen_size tests ─────────────────────────────────────────

    #[test]
    fn parse_screen_size_basic() {
        assert_eq!(parse_screen_size("120x40").unwrap(), (120, 40));
        assert_eq!(parse_screen_size("12x12").unwrap(), (12, 12));
        // 1x1 is now rejected (minimum is 4x4)
        assert!(parse_screen_size("1x1").is_err());
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
        // Minimum is 4x4 (MIN_TERMINAL_COLS x MIN_TERMINAL_LINES)
        assert!(parse_screen_size("1x1").is_err());
        assert!(parse_screen_size("3x3").is_err());
        assert!(parse_screen_size("12x1").is_err());
        assert!(parse_screen_size("12x2").is_err());
        assert!(parse_screen_size("12x3").is_err());
        assert!(parse_screen_size("3x12").is_err());
        // 4x4 is the minimum accepted
        assert!(parse_screen_size("4x4").is_ok());
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
}
