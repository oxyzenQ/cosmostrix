// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Clock Subsystem — Centralized Wall-Clock Helpers
//!
//! This module consolidates ALL time/clock code in cosmostrix into a single
//! `src/clock/` directory. Before this consolidation, time code was scattered
//! across 4 files (`clock.rs`, `posix_time.rs`, plus duplicated inline time
//! math in `ambient.rs` and `phase_predictor.rs`). Owner mandate 2026-08-19:
//! centralize for navigability + LTS stability.
//!
//! ## Module layout
//!
//! | File             | Role                                                                 |
//! |------------------|----------------------------------------------------------------------|
//! | `mod.rs`         | High-level helpers (Hinnant-style): `now_hhmm()`, `now_iso_utc()`. Consumes `posix_time` parsed structs. |
//! | `posix_time.rs`  | Low-level POSIX FFI: `libc::time(NULL)` + `localtime_r` / `gmtime_r`. Single place for `unsafe` time code. Returns `LocalTm` / `UtcTm` parsed structs. |
//!
//! ## Why this consolidation
//!
//! Before: 6 copy-pasted `MaybeUninit<libc::tm>` + `tzset()` OnceLock +
//! `assume_init()` blocks across 4 files. A bug in one site (e.g. missing
//! `tzset()`, wrong NULL check) had to be fixed 6 times.
//!
//! After: single verified path in `posix_time.rs`. Callers (clock/mod.rs,
//! `crystal_dragon_engine::ambient`, `central_control_power_dragon::
//! phase_predictor`) get the broken-out fields they need without touching
//! unsafe code.
//!
//! ## Platform coverage
//!
//! - Unix (Linux/macOS/BSD/Termux): `libc::time + localtime_r/gmtime_r`
//! - Non-Unix (Windows): `SystemTime::now()` UTC-based fallback (less
//!   accurate — no local timezone — but sufficient for scheduler + log-stamp)
//!
//! ## LTS stability
//!
//! - All `unsafe` FFI consolidated in `posix_time.rs` (single audit surface)
//! - `tzset()` invoked once per process via `OnceLock` (idempotent, µs-cost)
//! - All functions return `Option<T>` or `Default` — never panic on clock
//!   unavailable (degrades gracefully to `[--:--]` or `0`)
//! - No mutex/atomic (correct — single-threaded access from main thread)
//! - Howard Hinnant minimal abstraction: pure functions, no state

/// Get the current local time as `[HH:MM]` (24-hour, zero-padded).
///
/// Returns `[--:--]` if the system clock is unavailable (extremely rare —
/// only happens on platforms without a working localtime). This keeps
/// verbose output readable even in degraded environments.
#[must_use]
pub(crate) fn now_hhmm() -> String {
    match crate::posix_time::local_tm() {
        Some(tm) => format!("[{:02}:{:02}]", tm.hour, tm.minute),
        None => "[--:--]".to_string(),
    }
}

/// Current local wall-clock hour as f64 (with minute/second fraction).
///
/// Used by Crystal Dragon Engine sensor tests to verify wall-clock
/// hour math (the production sensor uses `SystemTime::now()` directly,
/// not this helper).
#[must_use]
#[cfg(test)]
pub(crate) fn current_local_hour() -> f64 {
    match crate::posix_time::local_tm() {
        Some(tm) => (tm.hour as f64) + (tm.minute as f64) / 60.0 + (tm.second as f64) / 3600.0,
        None => 0.0,
    }
}

/// Get the current UTC time as an ISO 8601 / RFC 3339 string with second
/// precision, e.g. `2026-08-06T15:10:00Z`.
///
/// Used by `configfile::dump_config_with_header()` to stamp generated
/// config.toml files with a machine-parseable, timezone-safe timestamp.
#[must_use]
pub(crate) fn now_iso_utc() -> String {
    let tm = crate::posix_time::utc_tm();
    if tm.year == 0 {
        return "0000-01-01T00:00:00Z".to_string();
    }
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        tm.year, tm.month, tm.day, tm.hour, tm.minute, tm.second
    )
}

/// Get the current UTC wall-clock as `YYYY-MM-DD HH:MM:SSZ`.
///
/// v50.0.0-beta.6: used by the verbose exit summary so the user can see
/// the exact UTC time cosmostrix exited at, alongside the total run
/// duration. Switched from local-time + offset (rc.1) to plain UTC for
/// LTS stability: UTC has no DST transitions, no timezone-database drift,
/// and is consistent across environments. The `Z` suffix (ISO 8601 UTC
/// designator) is universally recognized and machine-parseable.
///
/// Falls back to `0000-01-01 00:00:00Z` if the system clock is
/// unavailable (extremely rare — only on platforms without a working
/// gmtime). Reuses the existing `utc_tm()` FFI path (single POSIX
/// `gmtime_r` call) — no new FFI, no timezone lookup.
///
/// Format rationale: space separator (not `T`) keeps the original
/// human-readable feel the owner specified in rc.1; the `Z` suffix
/// replaces the `±HH:MM` offset and is shorter + unambiguous.
#[must_use]
pub(crate) fn now_utc_datetime() -> String {
    let tm = crate::posix_time::utc_tm();
    if tm.year == 0 {
        return "0000-01-01 00:00:00Z".to_string();
    }
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}Z",
        tm.year, tm.month, tm.day, tm.hour, tm.minute, tm.second
    )
}

/// Maximum width (chars) of the value part produced by
/// [`format_uptime_tiered`] — the tiered uptime formatter's width budget.
///
/// Derivation: the HUD's widest line budget is `HUD_MAX_WIDTH` = 24
/// columns (`src/interactive/hud/mod.rs`) and the `up:` line prefix
/// `" up: "` occupies 5 of them, leaving 19 for the value. The
/// degradation rule in `format_uptime_tiered` drops the
/// least-significant unit while the composed value exceeds this
/// budget. Mathematical guarantee (see the formatter's docs): the
/// 2-unit floor always fits — even `u64::MAX` seconds (≈ 584 billion
/// years, 12-digit year count) composes to an 18-char value — so the
/// budget can NEVER be exceeded, only approached.
const UPTIME_VALUE_MAX_CHARS: usize = 19;

/// Format session uptime (whole seconds) as a tiered compound string.
///
/// v80.0.0-alpha.1 S-master-HUNT-5 (owner task 2026-09-03): the HUD
/// `up:` line previously collapsed everything ≥ 1 day to `Xd:YYh`,
/// losing minute precision, and had no month/year scale — while the
/// owner runs multi-day sessions and expects server-class uptimes
/// (`up: 1mo:1d:22h:10m`) to stay readable. The owner's examples were
/// explicitly "just reference for simplify" — this is the masterclass
/// ladder that supersedes them.
///
/// ## Tier ladder
///
/// | Condition | Format | Example |
/// |-----------|--------|---------|
/// | < 1h | `MM:SS` | `59:03` |
/// | < 1d | `Xh:MMm` | `8h:01m` |
/// | < 30d | `Xd:HHh:MMm` | `1d:07h:22m` |
/// | < 365d | `Xmo:DDd:HHh:MMm` | `1mo:01d:22h:10m` |
/// | ≥ 365d | `Xy:MOmo:DDd:HHh:MMm` | `1y:02mo:03d:22h:10m` |
///
/// Precision degrades gracefully with magnitude: seconds are shown
/// below 1h, minutes from 1h up — mirroring how a viewer's caring
/// granularity scales with session length.
///
/// ## Design decisions (deliberate, owner-delegated)
///
/// - Fixed elapsed-time units: 1mo = 30d, 1y = 365d. Uptime is
///   elapsed duration, not a calendar date — calendar months (28-31d)
///   and leap years would make the display non-deterministic across
///   timezones/eras and untestable. Fixed units keep every boundary
///   exact (`86_399s` = `23h:59m`, `86_400s` = `1d:00h:00m`).
/// - Explicit unit suffixes on every component ≥ 1h (`8h:01m`, not
///   `8h:01`): self-describing at every scale once days join in, and
///   matches the owner's reference spelling.
/// - Zero-padded non-leading units (`1d:07h:22m`, not `1d:7h:22m`):
///   width stability. Unpadded values jitter the line width at rollover
///   (`7h` → `10h`), resizing the dynamic-width HUD box every cycle;
///   padding pins the tier width so the HUD frame and its chroma
///   border (`draw_border`) stay visually still between tier crossings.
/// - Full unit chain, no zero-trimming (`1mo:00d:00h:10m`, not
///   `1mo:10m`): stable per-tier width + unambiguous reading (trimmed
///   `1mo:10m` invites misreading the minutes as months).
/// - Budget-aware degradation: while the value exceeds 19 chars
///   (the HUD budget, see [`UPTIME_VALUE_MAX_CHARS`]) the
///   least-significant unit is dropped. In practice this only fires at
///   year ≥ 10 (`10y:11mo:28d:23h` — a decade-scale run where nobody
///   reads minutes). The 2-unit floor always fits within the budget
///   (12-digit year + `:11mo` = 18 chars), so overflow is impossible.
/// - ASCII only: every component is plain ASCII (`d`, `h`, `m`,
///   `mo`, `y` + `:` + digits) — complies with the project-wide
///   symbol-only-output policy (see `scripts/check-symbol-only-output.sh`).
///
/// ## Callers
///
/// - HUD `up:` line (`interactive/hud/metrics.rs`) — the owner-facing
///   surface for this task.
/// - Unit tests in this module lock every tier boundary + the
///   degradation ladder + the `u64::MAX` guarantee.
///
/// Distinct from [`format_duration_compact`]: that one is the prose
/// style (`1h 5m 3s`) used by the verbose exit summary; this one is
/// the compact HUD style with unit suffixes and the width budget.
#[must_use]
pub(crate) fn format_uptime_tiered(secs: u64) -> String {
    const HOUR: u64 = 3_600;
    const DAY: u64 = 86_400;
    const MONTH: u64 = 30 * DAY; // fixed 30-day month (elapsed-time unit)
    const YEAR: u64 = 365 * DAY; // fixed 365-day year (elapsed-time unit)

    // Tier 0 — stopwatch style, seconds precision (locked since v80.0.0-beta.1;
    // existing HUD tests assert this surface, unchanged by design).
    if secs < HOUR {
        return format!("{:02}:{:02}", secs / 60, secs % 60);
    }

    // Build the unit chain (value, label), most-significant first.
    // Each tier includes every smaller unit so the chain is complete.
    let years = secs / YEAR;
    let mut chain: Vec<(u64, &str)> = Vec::with_capacity(5);
    if years > 0 {
        let rem = secs % YEAR;
        chain.push((years, "y"));
        chain.push((rem / MONTH, "mo"));
        chain.push((rem % MONTH / DAY, "d"));
        chain.push((rem % DAY / HOUR, "h"));
        chain.push((rem % HOUR / 60, "m"));
    } else if secs >= MONTH {
        let rem = secs % MONTH;
        chain.push((secs / MONTH, "mo"));
        chain.push((rem / DAY, "d"));
        chain.push((rem % DAY / HOUR, "h"));
        chain.push((rem % HOUR / 60, "m"));
    } else if secs >= DAY {
        let rem = secs % DAY;
        chain.push((secs / DAY, "d"));
        chain.push((rem / HOUR, "h"));
        chain.push((rem % HOUR / 60, "m"));
    } else {
        chain.push((secs / HOUR, "h"));
        chain.push((secs % HOUR / 60, "m"));
    }

    // Compose + degrade: drop the least-significant unit while the
    // value exceeds the HUD budget. The 2-unit floor provably fits
    // (u64::MAX → 12-digit year + ":11mo" = 18 chars ≤ 19), so the
    // loop always terminates within the budget.
    let mut value = compose_uptime_chain(&chain);
    while value.chars().count() > UPTIME_VALUE_MAX_CHARS && chain.len() > 2 {
        chain.pop();
        value = compose_uptime_chain(&chain);
    }
    value
}

/// Compose a `(value, label)` chain into `V:VV:VV` form — leading unit
/// at natural width, every following unit zero-padded to 2 digits
/// (width stability, see [`format_uptime_tiered`] docs).
fn compose_uptime_chain(chain: &[(u64, &str)]) -> String {
    let mut out = String::with_capacity(chain.len() * 4);
    for (i, (v, label)) in chain.iter().enumerate() {
        if i > 0 {
            out.push(':');
        }
        if i == 0 {
            out.push_str(&v.to_string());
        } else {
            out.push_str(&format!("{v:02}"));
        }
        out.push_str(label);
    }
    out
}

/// Format a `Duration` as a compact human-readable string `Xm Ys` or `Ys`.
///
/// v50.0.0-rc.1: used by the verbose exit summary to show how long
/// cosmostrix ran. Examples: `1m 52s`, `0s`, `45s`, `1h 5m 3s`. Hours are
/// only shown for runs ≥ 1h. Sub-second precision is dropped (verbose
/// summary is for humans, not benchmark reports — use `--benchmark` for
/// sub-millisecond timing).
///
/// v80.0.0-alpha.1 S-master-HUNT-5: day/month/year tiers added so a
/// multi-day interactive session (the same class the HUD `up:` line
/// now handles via [`format_uptime_tiered`]) does not collapse to
/// hour-counting prose (`72h 0m 0s`). Calendar-fixed units (1mo = 30d,
/// 1y = 365d) — same rationale as the tiered formatter. Prose style
/// (space-separated, sub-minute seconds) is preserved for the
/// short-run summary context this function serves.
#[must_use]
pub(crate) fn format_duration_compact(d: std::time::Duration) -> String {
    const DAY: u64 = 86_400;
    const MONTH: u64 = 30 * DAY;
    const YEAR: u64 = 365 * DAY;

    let total_secs = d.as_secs();
    // Chained remainders (each unit derives from what is left after the
    // larger units) — a flat `total % UNIT` per unit would double-count
    // the year's 5 leftover days as both "1y" and "5d".
    let years = total_secs / YEAR;
    let rem_y = total_secs % YEAR;
    let months = rem_y / MONTH;
    let rem_m = rem_y % MONTH;
    let days = rem_m / DAY;
    let rem_d = rem_m % DAY;
    let hours = rem_d / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    if years > 0 {
        format!("{years}y {months}mo {days}d {hours}h {mins}m {secs}s")
    } else if months > 0 {
        format!("{months}mo {days}d {hours}h {mins}m {secs}s")
    } else if days > 0 {
        format!("{days}d {hours}h {mins}m {secs}s")
    } else if hours > 0 {
        format!("{hours}h {mins}m {secs}s")
    } else if mins > 0 {
        format!("{mins}m {secs}s")
    } else {
        format!("{secs}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_hhmm_format() {
        let s = now_hhmm();
        // Should match `[HH:MM]` (5 chars + brackets = 7) OR `[--:--]` on failure.
        assert!(
            (s.len() == 7 && s.starts_with('[') && s.ends_with(']')) || s == "[--:--]",
            "now_hhmm returned unexpected format: {s:?}"
        );
    }

    #[test]
    fn current_local_hour_bounded() {
        let h = current_local_hour();
        assert!(
            (0.0..24.0).contains(&h),
            "current_local_hour out of range: {h}"
        );
    }

    #[test]
    fn now_hhmm_is_ascii() {
        let s = now_hhmm();
        assert!(s.is_ascii(), "now_hhmm must be ASCII: {s:?}");
    }

    #[test]
    fn now_iso_utc_format() {
        let s = now_iso_utc();
        assert!(
            s.len() == 20 && s.ends_with('Z') && s.as_bytes()[10] == b'T',
            "now_iso_utc returned unexpected format: {s:?}"
        );
    }

    #[test]
    fn now_iso_utc_matches_rfc3339() {
        let s = now_iso_utc();
        let re = regex_lite(s.as_str());
        assert!(re.is_some(), "now_iso_utc not RFC 3339: {s:?}");
    }

    // v50.0.0-beta.6: tests for the verbose exit-stamp helpers.

    #[test]
    fn now_utc_datetime_format() {
        let s = now_utc_datetime();
        // Expected: "YYYY-MM-DD HH:MM:SSZ" (20 chars).
        // Fallback "0000-01-01 00:00:00Z" is also 20 chars.
        assert_eq!(s.len(), 20, "now_utc_datetime wrong length: {s:?}");
        let b = s.as_bytes();
        assert_eq!(b[4] as char, '-', "date separator wrong: {s:?}");
        assert_eq!(b[7] as char, '-', "date separator wrong: {s:?}");
        assert_eq!(b[10] as char, ' ', "date/time separator wrong: {s:?}");
        assert_eq!(b[13] as char, ':', "time separator wrong: {s:?}");
        assert_eq!(b[16] as char, ':', "time separator wrong: {s:?}");
        // Trailing Z (ISO 8601 UTC designator) at position 19.
        assert_eq!(b[19] as char, 'Z', "UTC designator wrong: {s:?}");
    }

    #[test]
    fn now_utc_datetime_is_ascii() {
        let s = now_utc_datetime();
        assert!(s.is_ascii(), "now_utc_datetime must be ASCII: {s:?}");
    }

    #[test]
    fn now_utc_datetime_matches_now_iso_utc() {
        // Both now_utc_datetime() and now_iso_utc() read from the same
        // utc_tm() FFI path. The only difference is the separator (space
        // vs 'T') and the offset ('Z' is present in both). Cross-check
        // that the date + time digits agree (within a 2-second window to
        // avoid rare boundary-crossing false positives).
        let local = now_utc_datetime();
        let iso = now_iso_utc();
        // Both are 20 chars; digits at positions 0-9 (date) and 11-18 (time).
        let local_digits: String = local.chars().filter(|c| c.is_ascii_digit()).collect();
        let iso_digits: String = iso.chars().filter(|c| c.is_ascii_digit()).collect();
        assert_eq!(
            local_digits.len(),
            14,
            "now_utc_datetime must have 14 digits: {local:?}"
        );
        assert_eq!(
            iso_digits.len(),
            14,
            "now_iso_utc must have 14 digits: {iso:?}"
        );
        // Allow last digit (seconds) to differ by 1 (boundary crossing).
        let local_secs: i32 = local_digits[12..14].parse().unwrap_or(-1);
        let iso_secs: i32 = iso_digits[12..14].parse().unwrap_or(-2);
        let diff = (local_secs - iso_secs).abs();
        assert!(
            diff <= 2,
            "now_utc_datetime vs now_iso_utc second diff too large: {diff}"
        );
    }

    #[test]
    fn format_duration_compact_canonical_cases() {
        use std::time::Duration;
        // 0s.
        assert_eq!(format_duration_compact(Duration::from_secs(0)), "0s");
        // 45s.
        assert_eq!(format_duration_compact(Duration::from_secs(45)), "45s");
        // 1m 52s (the user's example output).
        assert_eq!(format_duration_compact(Duration::from_secs(112)), "1m 52s");
        // 59m 59s.
        assert_eq!(
            format_duration_compact(Duration::from_secs(3599)),
            "59m 59s"
        );
        // 1h 0m 0s.
        assert_eq!(
            format_duration_compact(Duration::from_secs(3600)),
            "1h 0m 0s"
        );
        // 1h 5m 3s.
        assert_eq!(
            format_duration_compact(Duration::from_secs(3903)),
            "1h 5m 3s"
        );
    }

    #[test]
    fn format_duration_compact_drops_subsecond() {
        use std::time::Duration;
        // 1m 52s + 750ms still renders as "1m 52s" (sub-second dropped).
        assert_eq!(
            format_duration_compact(Duration::from_millis(112_750)),
            "1m 52s"
        );
    }

    // ── format_uptime_tiered tests (v80.0.0-alpha.1 S-master-HUNT-5) ──

    #[test]
    fn uptime_tier0_minutes_seconds_under_1h() {
        // Tier 0 unchanged from the v80.0.0-beta.1 surface (HUD tests
        // lock " up: 02:00" — this tier must keep the MM:SS stopwatch form).
        assert_eq!(format_uptime_tiered(0), "00:00");
        assert_eq!(format_uptime_tiered(45), "00:45");
        assert_eq!(format_uptime_tiered(120), "02:00");
        assert_eq!(format_uptime_tiered(3543), "59:03");
        assert_eq!(format_uptime_tiered(3599), "59:59");
    }

    #[test]
    fn uptime_tier1_hours_with_minute_suffix() {
        // 1h boundary: seconds drop, explicit m suffix joins (owner
        // reference "up: 8h:01m").
        assert_eq!(format_uptime_tiered(3600), "1h:00m");
        assert_eq!(format_uptime_tiered(3661), "1h:01m");
        assert_eq!(format_uptime_tiered(8 * 3600 + 60), "8h:01m");
        assert_eq!(format_uptime_tiered(23 * 3600 + 59 * 60 + 59), "23h:59m");
        // 1h + 1s: the seconds are folded into the minute display only
        // at whole minutes — 3601s renders 1h:00m (sub-minute dropped,
        // same truncation semantics as every prior tier).
        assert_eq!(format_uptime_tiered(3601), "1h:00m");
    }

    #[test]
    fn uptime_tier2_days_keep_minutes() {
        // 1d boundary: minutes SURVIVE past the day crossing (the owner's
        // core complaint — old format lost them: "2d:03h").
        assert_eq!(format_uptime_tiered(86_400), "1d:00h:00m");
        assert_eq!(
            format_uptime_tiered(86_400 + 7 * 3600 + 22 * 60),
            "1d:07h:22m"
        );
        // Owner reference: 1d:7h:22m — rendered with the zero-padded
        // middle unit (width stability, see formatter docs).
        assert_eq!(
            format_uptime_tiered(86_400 + 7 * 3600 + 22 * 60),
            "1d:07h:22m"
        );
        assert_eq!(
            format_uptime_tiered(29 * 86_400 + 23 * 3600 + 59 * 60),
            "29d:23h:59m"
        );
        // The 7-hour owner case: "up: 8h:01m" after 8h01m.
        assert_eq!(format_uptime_tiered(28_860), "8h:01m");
    }

    #[test]
    fn uptime_tier3_months() {
        // 30d boundary (fixed 30-day month): the mo unit joins.
        assert_eq!(format_uptime_tiered(30 * 86_400), "1mo:00d:00h:00m");
        // Owner reference: 1mo:1d:22h:10m.
        let secs = 30 * 86_400 + 86_400 + 22 * 3600 + 10 * 60;
        assert_eq!(format_uptime_tiered(secs), "1mo:01d:22h:10m");
        // Full month-band max: 11mo:29d:23h:59m (16 chars — fits budget).
        assert_eq!(
            format_uptime_tiered(11 * 30 * 86_400 + 29 * 86_400 + 86_399),
            "11mo:29d:23h:59m"
        );
    }

    #[test]
    fn uptime_tier4_years() {
        // 365d boundary (fixed 365-day year): the y unit joins.
        assert_eq!(format_uptime_tiered(365 * 86_400), "1y:00mo:00d:00h:00m");
        // 1y + 2mo + 3d + 22h + 10m.
        let secs = 365 * 86_400 + 2 * 30 * 86_400 + 3 * 86_400 + 22 * 3600 + 600;
        assert_eq!(format_uptime_tiered(secs), "1y:02mo:03d:22h:10m");
        // 9y:11mo:29d:23h:59m = 19 chars — the widest in-budget value.
        let wide = 9 * 365 * 86_400 + 11 * 30 * 86_400 + 29 * 86_400 + 86_399;
        let s = format_uptime_tiered(wide);
        assert_eq!(s, "9y:11mo:29d:23h:59m");
        assert_eq!(
            s.chars().count(),
            19,
            "widest in-budget value must be exactly 19 chars"
        );
    }

    #[test]
    fn uptime_degradation_drops_minutes_at_decade_scale() {
        // 10y+: minutes would push the value past the 19-char HUD budget
        // → the least-significant unit drops (decade-scale viewers do
        // not read minutes).
        let decade = 10 * 365 * 86_400 + 11 * 30 * 86_400 + 28 * 86_400 + 23 * 3600;
        assert_eq!(format_uptime_tiered(decade), "10y:11mo:28d:23h");
        // A century: hours survive (17 chars, in budget).
        let century = 100 * 365 * 86_400 + 11 * 30 * 86_400 + 28 * 86_400 + 23 * 3600;
        assert_eq!(format_uptime_tiered(century), "100y:11mo:28d:23h");
    }

    #[test]
    fn uptime_budget_never_exceeded_even_at_u64_max() {
        // The mathematical guarantee: u64::MAX seconds ≈ 584 billion
        // years → 12-digit year + ":11mo" = 18 chars ≤ 19. Every output
        // the formatter can ever produce respects the HUD budget.
        for secs in [u64::MAX, u64::MAX - 1, 10u64.pow(18), 10u64.pow(15)] {
            let s = format_uptime_tiered(secs);
            assert!(
                s.chars().count() <= 19,
                "budget violated at {secs}: {s:?} ({} chars)",
                s.chars().count()
            );
        }
        // u64::MAX / 365d = 584,942,417,355 years (12 digits).
        assert!(format_uptime_tiered(u64::MAX).starts_with("584942417355y:"));
    }

    #[test]
    fn uptime_output_is_ascii() {
        // Symbol-only-output policy: every tier must be pure ASCII.
        for secs in [0, 3599, 3600, 86_400, 30 * 86_400, 365 * 86_400, u64::MAX] {
            let s = format_uptime_tiered(secs);
            assert!(s.is_ascii(), "non-ASCII uptime output at {secs}: {s:?}");
        }
    }

    #[test]
    fn uptime_tier_boundaries_exact() {
        // 1h - 1s → tier 0; 1h → tier 1.
        assert_eq!(format_uptime_tiered(3599), "59:59");
        assert_eq!(format_uptime_tiered(3600), "1h:00m");
        // 1d - 1s → tier 1 max; 1d → tier 2.
        assert_eq!(format_uptime_tiered(86_399), "23h:59m");
        assert_eq!(format_uptime_tiered(86_400), "1d:00h:00m");
        // 30d - 1s → tier 2 max; 30d → tier 3.
        assert_eq!(format_uptime_tiered(30 * 86_400 - 1), "29d:23h:59m");
        assert_eq!(format_uptime_tiered(30 * 86_400), "1mo:00d:00h:00m");
        // 365d - 1s → tier 3 max (12 full months + 4 days — 364d = 12×30d
        // + 4d under fixed units); 365d → tier 4.
        assert_eq!(format_uptime_tiered(365 * 86_400 - 1), "12mo:04d:23h:59m");
        assert_eq!(format_uptime_tiered(365 * 86_400), "1y:00mo:00d:00h:00m");
    }

    #[test]
    fn duration_compact_day_month_year_tiers() {
        use std::time::Duration;
        // S-master-HUNT-5: the verbose-exit prose formatter gains the
        // same day/month/year ladder (72h stays readable as 3d).
        assert_eq!(
            format_duration_compact(Duration::from_secs(86_400)),
            "1d 0h 0m 0s"
        );
        assert_eq!(
            format_duration_compact(Duration::from_secs(3 * 86_400 + 5 * 3600)),
            "3d 5h 0m 0s"
        );
        assert_eq!(
            format_duration_compact(Duration::from_secs(30 * 86_400 + 86_400)),
            "1mo 1d 0h 0m 0s"
        );
        assert_eq!(
            format_duration_compact(Duration::from_secs(365 * 86_400)),
            "1y 0mo 0d 0h 0m 0s"
        );
    }

    /// Tiny inline RFC 3339 validator (avoids pulling in the `regex` crate
    /// just for one test). Returns Some(()) on match, None otherwise.
    fn regex_lite(s: &str) -> Option<()> {
        let b = s.as_bytes();
        if b.len() != 20
            || b[4] != b'-'
            || b[7] != b'-'
            || b[10] != b'T'
            || b[13] != b':'
            || b[16] != b':'
            || b[19] != b'Z'
        {
            return None;
        }
        for &c in b
            .iter()
            .filter(|&&c| c != b'-' && c != b'T' && c != b':' && c != b'Z')
        {
            if !c.is_ascii_digit() {
                return None;
            }
        }
        Some(())
    }
}

// ── POSIX FFI submodule ─────────────────────────────────────────────────
//
// Low-level FFI for libc::time + localtime_r / gmtime_r. Re-exported at
// the crate root via `pub(crate) use clock::posix_time;` in main.rs so
// all existing `crate::posix_time::Foo` call sites continue to resolve.
pub(crate) mod posix_time;
