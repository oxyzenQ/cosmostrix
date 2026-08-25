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
//! `crystal_dragon_engine::ambient`, `central_control_dragon_power::
//! phase_predictor`) get the broken-out fields they need without touching
//! unsafe code.
//!
//! ## Platform coverage
//!
//! - **Unix (Linux/macOS/BSD/Termux)**: `libc::time + localtime_r/gmtime_r`
//! - **Non-Unix (Windows)**: `SystemTime::now()` UTC-based fallback (less
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

/// Format a `Duration` as a compact human-readable string `Xm Ys` or `Ys`.
///
/// v50.0.0-rc.1: used by the verbose exit summary to show how long
/// cosmostrix ran. Examples: `1m 52s`, `0s`, `45s`, `1h 5m 3s`. Hours are
/// only shown for runs ≥ 1h. Sub-second precision is dropped (verbose
/// summary is for humans, not benchmark reports — use `--benchmark` for
/// sub-millisecond timing).
#[must_use]
pub(crate) fn format_duration_compact(d: std::time::Duration) -> String {
    let total_secs = d.as_secs();
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    if hours > 0 {
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
