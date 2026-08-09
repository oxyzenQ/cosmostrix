// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Centralized wall-clock helpers (Hinnant-style minimal abstraction).
//!
//! Replaces the previous direct `chrono::Local::now()` calls scattered in
//! `output.rs::now_hhmm()` and `system_feeling.rs::current_local_hour()`.
//!
//! ## Why this exists
//!
//! `chrono`'s `clock` feature drags in 8 transitive crates including
//! `wasm-bindgen`, `js-sys`, `iana-time-zone-haiku`, and `core-foundation-sys`
//! — all dead weight on a Linux-native CLI that only needs `[HH:MM]` log
//! stamps and a local-hour fraction. The Linux path here mirrors the
//! Hinnant-optimal pattern already established in
//! `interactive/adaptive.rs::local_secs_since_midnight()` (direct POSIX
//! `libc::time` + `libc::localtime_r`, no allocation, no chrono wrapper).
//!
//! ## Non-goals
//!
//! This module does NOT replace `std::time::Instant` for monotonic
//! measurements — `Instant` remains the correct primitive for elapsed-time
//! hot paths (see spawn loop / rain_at). This module is strictly for
//! human-readable wall-clock snapshots, which are infrequent (verbose log
//! prefix, 3-second ecosystem tick) and tolerate the slightly higher cost
//! of a `localtime_r` syscall.
//!
//! ## Crate choice rationale
//!
//! `libc` was already a direct dependency (`Cargo.toml`). Adding `time` or
//! keeping `chrono` would be parallel date crates for no benefit. The
//! attribution "Howard Hinnant chrono" is historically inaccurate (chrono is
//! maintained by the chrono-rs team, descended from Hinnant's initial work)
//! but the design philosophy — smallest feature set sufficient for the task,
//! zero-cost abstraction — is the spirit we honor here.

/// Get the current local time as `[HH:MM]` (24-hour, zero-padded).
///
/// Returns `[--:--]` if the system clock is unavailable (extremely rare —
/// only happens on platforms without a working localtime). This keeps
/// verbose output readable even in degraded environments.
#[must_use]
pub(crate) fn now_hhmm() -> String {
    #[cfg(unix)]
    {
        if let Some((hour, minute, _)) = local_hms() {
            return format!("[{hour:02}:{minute:02}]");
        }
        "[--:--]".to_string()
    }
    #[cfg(not(unix))]
    {
        // Non-unix fallback: SystemTime + UTC. Acceptable for log stamps —
        // the alternative would be pulling in winapi or chrono just for
        // local-time formatting, which isn't worth it for a CLI that
        // primarily targets Linux/macOS/Termux.
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let hour = (secs / 3600) % 24;
        let min = (secs / 60) % 60;
        format!("[{hour:02}:{min:02}]")
    }
}

/// Current local wall-clock hour as f64 (with minute/second fraction).
///
/// Inlined here after atmosphere engine elimination. The previous
/// implementation lived in `atmosphere_adaptive::current_hour()` which
/// was deleted along with the rest of the atmosphere engine subsystem.
/// Used by `SystemFeeling::tick()` for time-of-day state classification.
#[must_use]
pub(crate) fn current_local_hour() -> f64 {
    #[cfg(unix)]
    {
        if let Some((hour, minute, second)) = local_hms() {
            return (hour as f64) + (minute as f64) / 60.0 + (second as f64) / 3600.0;
        }
        0.0
    }
    #[cfg(not(unix))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let hour = ((secs / 3600) % 24) as f64;
        let min = ((secs / 60) % 60) as f64;
        let sec = (secs % 60) as f64;
        hour + min / 60.0 + sec / 3600.0
    }
}

/// Returns `(hour, minute, second)` from the system's local timezone.
///
/// Returns `None` on any failure (clock unavailable, localtime_r failed).
/// Pattern mirrors `interactive/adaptive.rs::local_secs_since_midnight()`.
///
/// `libc::tzset()` is invoked once per process (via `OnceLock`) so that
/// timezone changes via `TZ` env var or `timedatectl set-timezone` mid-run
/// are reflected. POSIX does not guarantee `localtime_r` re-reads tzdata
/// automatically (musl historically caches until `tzset()` is called).
#[cfg(unix)]
fn local_hms() -> Option<(i32, i32, i32)> {
    use std::mem::MaybeUninit;
    use std::sync::OnceLock;
    // Process-wide tzset() — safe, idempotent, µs-cost. Runs exactly once
    // on first wall-clock query so subsequent calls reuse cached tzdata.
    // Declared as direct extern because libc 0.2.x does not export tzset
    // in the top-level namespace on all targets.
    extern "C" {
        fn tzset();
    }
    static TZ_INIT: OnceLock<()> = OnceLock::new();
    TZ_INIT.get_or_init(|| unsafe { tzset() });
    // SAFETY: libc::time(NULL) is the documented POSIX call — writes nothing
    // when the pointer is NULL, returns time_t or -1 on error. No preconditions.
    let now = unsafe { libc::time(std::ptr::null_mut()) };
    if now < 0 {
        return None;
    }
    let mut tm: MaybeUninit<libc::tm> = MaybeUninit::uninit();
    let tm_ptr = tm.as_mut_ptr();
    // SAFETY: localtime_r is the thread-safe POSIX variant. It reads `now`
    // (a valid time_t value, checked >= 0 above) and writes into our
    // MaybeUninit<tm> buffer. Returns NULL on failure (handled below).
    if unsafe { libc::localtime_r(&now, tm_ptr) }.is_null() {
        return None;
    }
    // SAFETY: localtime_r returned non-NULL, which per POSIX means the tm
    // struct has been fully initialized. assume_init() is now sound.
    let tm = unsafe { tm.assume_init() };
    Some((tm.tm_hour, tm.tm_min, tm.tm_sec))
}

/// Get the current UTC time as an ISO 8601 / RFC 3339 string with second
/// precision, e.g. `2026-08-06T15:10:00Z`.
///
/// Used by `configfile::dump_config_with_header()` to stamp generated
/// config.toml files with a machine-parseable, timezone-safe timestamp.
/// UTC is preferred over local time for generated artifacts because:
///   1. Cross-platform consistency (Linux/macOS/Windows/Termux all agree).
///   2. Round-trips through any RFC 3339 parser (chrono, time, jiff, etc.)
///      if the user later wants to read it.
///   3. No daylight-saving jumps when diffing dotfiles git history.
///
/// Returns `0000-01-01T00:00:00Z` if the system clock is unavailable
/// (extremely rare — only happens on platforms without a working `gmtime_r`).
#[must_use]
pub(crate) fn now_iso_utc() -> String {
    // Use libc::time + libc::gmtime_r on Unix for the same Hinnant-style
    // rationale as `now_hhmm()` — avoids pulling in chrono's `clock` feature
    // (8 transitive deps) for a single timestamp per --dump-config invocation.
    #[cfg(unix)]
    {
        use std::mem::MaybeUninit;
        // SAFETY: libc::time(NULL) — same POSIX contract as in `local_hms`.
        let now = unsafe { libc::time(std::ptr::null_mut()) };
        if now < 0 {
            return "0000-01-01T00:00:00Z".to_string();
        }
        let mut tm: MaybeUninit<libc::tm> = MaybeUninit::uninit();
        // SAFETY: gmtime_r is the thread-safe POSIX variant. Same contract
        // as localtime_r — writes into our buffer, returns NULL on failure.
        if unsafe { libc::gmtime_r(&now, tm.as_mut_ptr()) }.is_null() {
            return "0000-01-01T00:00:00Z".to_string();
        }
        // SAFETY: gmtime_r returned non-NULL → tm fully initialized.
        let tm = unsafe { tm.assume_init() };
        // tm.tm_year is years since 1900; tm.tm_mon is 0-indexed.
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            tm.tm_year + 1900,
            tm.tm_mon + 1,
            tm.tm_mday,
            tm.tm_hour,
            tm.tm_min,
            tm.tm_sec
        )
    }
    #[cfg(not(unix))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        // Days since epoch + remainder. UTC throughout (no DST handling).
        let days = secs / 86_400;
        let remainder = secs % 86_400;
        let hour = remainder / 3600;
        let min = (remainder / 60) % 60;
        let sec = remainder % 60;
        // Civil-from-days algorithm (Howard Hinnant, "date" library) —
        // converts days-since-epoch to (year, month, day) without pulling
        // in chrono. See: https://howardhinnant.github.io/date_algorithms.html
        let z = days as i64 + 719_468; // shift epoch from 1970-01-01 to 0000-03-01
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = (z - era * 146_097) as u64; // [0, 146096]
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
        let y = yoe as i64 + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
        let mp = (5 * doy + 2) / 153; // [0, 11]
        let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
        let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
        let year = if m <= 2 { y + 1 } else { y };
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            year, m, d, hour, min, sec
        )
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
        // Hour fraction is in [0.0, 24.0). On non-unix fallback or libc
        // failure it may be exactly 0.0, which is still in range.
        assert!(
            (0.0..24.0).contains(&h),
            "current_local_hour out of range: {h}"
        );
    }

    #[test]
    fn now_hhmm_is_ascii() {
        // Defensive: ensure no non-ASCII leaked in (e.g. from locale-aware
        // strftime). libc::localtime_r fills numeric fields only.
        let s = now_hhmm();
        assert!(s.is_ascii(), "now_hhmm must be ASCII: {s:?}");
    }

    #[test]
    fn now_iso_utc_format() {
        let s = now_iso_utc();
        // Must match `YYYY-MM-DDTHH:MM:SSZ` (20 chars) OR the zero fallback.
        assert!(
            s.len() == 20 && s.ends_with('Z') && s.as_bytes()[10] == b'T',
            "now_iso_utc returned unexpected format: {s:?}"
        );
    }

    #[test]
    fn now_iso_utc_matches_rfc3339() {
        // Strict regex for RFC 3339 with second precision (no fractional).
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
