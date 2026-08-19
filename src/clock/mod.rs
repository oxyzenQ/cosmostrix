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
//! | `mod.rs`         | High-level helpers (Hinnant-style): `now_hhmm()`, `now_iso_utc()`, `current_local_hour()`. Consumes `posix_time` parsed structs. |
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
/// Retained for Crystal Dragon Engine CLOCK fallback sensor.
#[must_use]
#[allow(dead_code)]
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
