// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! CLI parsing helpers for `--duration` and `--screen-size` flags.
//!
//! Extracted from config.rs to keep that file under its LOC guard.
//! These are pure functions — parse + validate, no side effects.
//!
//! ## `--duration` vs `--bench-duration`
//!
//! Both accept the SAME human duration syntax (see [`parse_secs_f64`]):
//!   - `--duration` is `Option<f64>` parsed by [`parse_secs_f64`] at clap
//!     level (bare float `5`, human `5s`/`30m`/`1h30m`). Interactive-mode
//!     only — sets the auto-exit deadline in `event_loop.rs`.
//!     NOOP in `--benchmark`/`--bench-frames`/`--bench-all` mode (warned).
//!   - `--bench-duration` is `Option<String>` parsed by `parse_duration`
//!     (integer seconds only — benchmark windows are whole seconds).
//!
//! `parse_duration` and `parse_secs_f64` share one unit table
//! ([`unit_secs`]) so every seconds-scale input in cosmostrix — CLI flags
//! (`--crystal-dragon-secs`, `--duration`, `--bench-duration`) and config
//! keys (`crystal-dragon-secs`, `ambient-snapback-secs`) — accepts the
//! identical `45` / `45.5` / `45s` / `1m` / `2h15m30s` vocabulary
//! (v80.0.0-alpha.2 owner contract: one human format everywhere).
//!
//! ## 24-hour hard ceiling (v80.0.0-alpha.1 S-master-HUNT-5)
//!
//! Owner security mandate (2026-09-03): every flag that accepts a
//! time-scale value is hard-capped at 24 hours ([`DURATION_MAX_SECS`]).
//! Rationale: cosmostrix is a courteous guest on the host OS — a
//! flag-requested run longer than a day would hold CPU + terminal
//! resources indefinitely (performance leakage). The ceiling is
//! enforced INSIDE both parsers (not at call sites), so no future flag
//! or config key can accidentally bypass it. Calendar units beyond a
//! week are rejected at the unit table — elapsed-time formatting is
//! the HUD's job (see `clock::format_uptime_tiered`), not the input
//! grammar's.

/// Minimum benchmark duration: 1 second.
const DURATION_MIN_SECS: u64 = 1;

/// Hard ceiling for EVERY seconds-scale input in cosmostrix: 24 hours
/// (86,400s). Owner security mandate 2026-09-03 — see module docs.
///
/// Enforced structurally inside `parse_secs_f64` (CLI flags + config
/// keys) and `validate_secs` (`--bench-duration` + the bench frames
/// watchdog), so the cap cannot be bypassed by any caller. Exactly
/// 86400 is valid ("1d", "24h", the existing dragon-knob max —
/// `86400 = poll once per 24h`); only values strictly ABOVE it are
/// rejected.
pub(crate) const DURATION_MAX_SECS: f64 = 86_400.0;

/// `DURATION_MAX_SECS` as integer seconds (the u64 parser grammar's
/// copy — same value, same policy).
const DURATION_MAX_SECS_U64: u64 = 86_400;

/// The 24h policy sentence shared by both parsers' rejection messages
/// (single source — the wording must not drift between the f64 and u64
/// grammar surfaces).
const DURATION_CEILING_POLICY: &str = "cosmostrix caps every time-scale input \
     at one day: unbounded runs would hold CPU and terminal resources \
     on your OS indefinitely — courteous-guest policy";

/// Compose the 24h-policy rejection message for a time-scale input.
///
/// The message carries the REASON (owner mandate: "reject error with
/// that reason"), not just the fact: unbounded runs hold OS resources.
/// Rendered by every parser surface (CLI prevalidator, clap
/// value_parser, `--bench-duration`, config-key validation) so the
/// policy explanation is identical everywhere.
fn duration_ceiling_error(input: &str, secs: f64) -> String {
    // Render the seconds count without a trailing ".0" for whole
    // numbers (799200, not 799200.0) — matches the integer grammar of
    // --bench-duration and reads cleaner in the f64 paths.
    let secs_str = if secs.fract() == 0.0 {
        format!("{}", secs as u64)
    } else {
        format!("{secs}")
    };
    format!(
        "'{input}' resolves to {secs_str} seconds — over the 24h (86400s) hard ceiling \
         ({DURATION_CEILING_POLICY})"
    )
}

/// The ONE unit table for every duration input in cosmostrix (seconds).
///
/// Accepted unit spellings (long forms included):
///   - `s` / `sec` / `secs` / `second` / `seconds`  → 1
///   - `m` / `min` / `mins` / `minute` / `minutes`  → 60
///   - `h` / `hr` / `hrs` / `hour` / `hours`        → 3600
///   - `d` / `day` / `days`                          → 86400
///   - `w` / `week` / `weeks`                        → 604800
///
/// `d`/`w` (v80.0.0-alpha.1 S-master-HUNT-5): parsing day/week units lets
/// `--duration 2d` / `1w` fail with the 24h-policy message (the real
/// reason) instead of a misleading "unknown unit" — while sub-ceiling
/// day values stay expressible (`0.5d` = 12h, `1d` = exactly the cap).
/// A week can never be accepted (604800 > 86400) — the unit exists so
/// the rejection states the policy. Calendar units (`mo`/`y`) are
/// deliberately NOT in the table: their lengths are not fixed
/// elapsed-time units (see `clock::format_uptime_tiered` for the
/// display-side treatment).
///
/// Shared by `parse_duration` (u64, benchmark) and `parse_secs_f64`
/// (f64, CLI flags + config keys) so the two parsers can never drift.
fn unit_secs(unit: &str) -> Option<f64> {
    match unit {
        "s" | "sec" | "secs" | "second" | "seconds" => Some(1.0),
        "m" | "min" | "mins" | "minute" | "minutes" => Some(60.0),
        "h" | "hr" | "hrs" | "hour" | "hours" => Some(3600.0),
        "d" | "day" | "days" => Some(86_400.0),
        "w" | "week" | "weeks" => Some(604_800.0),
        _ => None,
    }
}

/// Parse a human-readable duration into fractional seconds (f64).
///
/// v80.0.0-alpha.2 (owner contract: one human format for every
/// seconds-scale input). Accepted forms:
///   - bare float  `45` / `45.5`          → 45.0 / 45.5 (backward compat)
///   - suffixed    `45s` / `30m` / `2h`   → 45 / 1800 / 7200
///   - compound    `1h30m` / `2h15m30s`   → 5400 / 8130
///   - fractional  `0.5s` / `1.5m`        → 0.5 / 90
///   - day/week    `0.5d` / `1d`          → 43200 / 86400 (S-master-HUNT-5)
///
/// Range is the CALLER's contract (the flag/config layer applies its own
/// min/max, e.g. 0.1..=86400.0 for `--duration`) — this fn rejects
/// negatives, non-finite values, malformed input, AND anything above
/// the 24h hard ceiling ([`DURATION_MAX_SECS`], enforced HERE so no
/// caller can bypass it). Callers: clap `value_parser` for
/// `--crystal-dragon-secs` + `--duration`, and `config_apply::
/// parse_secs_config` for the two `-secs` config keys.
///
/// # Errors
/// `Err(String)` (human-readable; the CLI/config layers add the flag/key
/// label) if the input is empty, has an unknown unit, a missing number or
/// unit, is negative, is non-finite, or resolves to more than 24 hours
/// (the message carries the policy reason).
pub(crate) fn parse_secs_f64(input: &str) -> Result<f64, String> {
    let input = input.trim();

    // Bare float → seconds (backward compat; rejects inf/NaN/negative).
    if let Ok(n) = input.parse::<f64>() {
        if !n.is_finite() {
            return Err(format!("'{input}' is not a finite number"));
        }
        if n < 0.0 {
            return Err(format!("'{input}' is negative (durations are >= 0)"));
        }
        if n > DURATION_MAX_SECS {
            return Err(duration_ceiling_error(input, n));
        }
        return Ok(n);
    }

    // Compound human format: <number><unit> pairs (whitespace allowed
    // between components), exactly the parse_duration grammar with f64
    // numbers so fractional seconds work (`0.5s`).
    let mut total_secs: f64 = 0.0;
    let mut chars = input.chars().peekable();
    let mut found_any = false;

    while chars.peek().is_some() {
        while matches!(chars.peek(), Some(c) if c.is_whitespace()) {
            chars.next();
        }
        if chars.peek().is_none() {
            break;
        }

        // Parse number (integer or fractional).
        let mut num_str = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() || c == '.' {
                num_str.push(c);
                chars.next();
            } else {
                break;
            }
        }
        if num_str.is_empty() {
            return Err(format!(
                "'{input}' has invalid format (expected number before unit)"
            ));
        }
        let num: f64 = num_str
            .parse()
            .map_err(|_| format!("'{input}' has invalid number '{num_str}'"))?;
        if !num.is_finite() || num < 0.0 {
            return Err(format!("'{input}' has invalid number '{num_str}'"));
        }

        // Parse unit.
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
                "'{input}' is missing a unit after {num_str} (use s/m/h/d/w — or a bare number for seconds)"
            ));
        }
        let multiplier = unit_secs(&unit_str)
            .ok_or_else(|| format!("'{input}' has unknown unit '{unit_str}' (use s/m/h/d/w)"))?;

        total_secs += num * multiplier;
        found_any = true;
    }

    if !found_any {
        return Err(format!(
            "'{input}' is empty or invalid (use a bare number, or 6s / 30m / 1h30m)"
        ));
    }
    if !total_secs.is_finite() || total_secs < 0.0 {
        return Err(format!("'{input}' resolves to an invalid duration"));
    }
    // 24h hard ceiling (S-master-HUNT-5): enforced here so no caller can
    // bypass it. `2d`/`1w`/`222h` reach this check with the parsed total.
    if total_secs > DURATION_MAX_SECS {
        return Err(duration_ceiling_error(input, total_secs));
    }
    Ok(total_secs)
}

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
/// Minimum: 1 second. Maximum: 24 hours — the hard ceiling
/// ([`DURATION_MAX_SECS`], v80.0.0-alpha.1 S-master-HUNT-5 owner
/// security mandate; previously "user responsibility", which let
/// `--bench-duration 222h` launch an unbounded benchmark run).
///
/// # Errors
/// Returns `Err(String)` with a human-readable error message if:
///   - Format is invalid (unrecognized unit, missing number)
///   - Value is zero or below minimum
pub(crate) fn parse_duration(flag_label: &str, input: &str) -> Result<u64, String> {
    let input = input.trim();

    // Bare number → seconds (backward compat with --bench-duration)
    if let Ok(n) = input.parse::<u64>() {
        return validate_secs(flag_label, n);
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
                "error: {flag_label} '{input}' has invalid format (expected number before unit)"
            ));
        }
        let num: u64 = num_str
            .parse()
            .map_err(|_| format!("error: {flag_label} '{input}' has number too large"))?;

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
                "error: {flag_label} '{input}' missing unit after {num_str} (use s/m/h/d/w)"
            ));
        }

        // Shared unit table (unit_secs) — cast to u64 for the integer
        // benchmark grammar. All multipliers are exact integers.
        let multiplier: u64 = unit_secs(&unit_str).map(|s| s as u64).ok_or_else(|| {
            format!("error: {flag_label} '{input}' has unknown unit '{unit_str}' (use s/m/h/d/w)")
        })?;

        total_secs = total_secs.saturating_add(num.saturating_mul(multiplier));
        found_any = true;
    }

    if !found_any {
        return Err(format!(
            "error: {flag_label} '{input}' is empty or invalid (use format like 6s, 30m, 1h30m)"
        ));
    }

    validate_secs(flag_label, total_secs)
}

fn validate_secs(flag_label: &str, secs: u64) -> Result<u64, String> {
    if secs < DURATION_MIN_SECS {
        return Err(format!(
            "error: {flag_label} {secs}s is below the {DURATION_MIN_SECS}-second minimum"
        ));
    }
    // 24h hard ceiling (S-master-HUNT-5): same policy as parse_secs_f64,
    // integer grammar. The error carries the reason so the user knows
    // this is a deliberate OS-protection cap, not an arbitrary limit.
    if secs > DURATION_MAX_SECS_U64 {
        return Err(format!(
            "error: {flag_label} {secs}s is over the 24h (86400s) hard ceiling \
             ({DURATION_CEILING_POLICY})"
        ));
    }
    Ok(secs)
}

/// Parsed screen size: (width, height).
pub(crate) type ScreenSize = (u16, u16);

/// Parse a screen size string `WxH` into `(width, height)`.
///
/// Accepted formats:
///   - `120x40` → (120, 40)
///   - `12x12` → (12, 12)
///   - `1x1` → (1, 1) (minimum, enforced by MIN_TERMINAL_COLS/LINES)
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
///   - Value is below minimum (0x0, 0x10, 10x0, or below 1x1)
pub(crate) fn parse_screen_size(input: &str) -> Result<ScreenSize, String> {
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
pub(crate) fn parse_screen_size_optional(
    input: &Option<String>,
) -> Result<Option<ScreenSize>, String> {
    match input {
        None => Ok(None),
        Some(s) => parse_screen_size(s).map(Some),
    }
}

#[cfg(test)]
#[path = "cli_parse_tests.rs"]
mod tests;
