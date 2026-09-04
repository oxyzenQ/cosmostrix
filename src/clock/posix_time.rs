// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Centralized POSIX wall-clock helpers — single place for `libc::time` +
//! `libc::localtime_r` / `libc::gmtime_r` FFI.
//!
//! ## Why this exists
//!
//! Before this module, the exact same `libc::time(NULL) + localtime_r` dance
//! was copy-pasted across 6 call sites in 4 files:
//!   - `clock.rs::local_hms()`                     — (hour, minute, second)
//!   - `clock.rs::now_iso_utc()`                   — gmtime_r for ISO UTC
//!   - `ambient.rs::current_minute_of_day()`       — hour*60 + minute
//!   - `ambient.rs::current_second_of_minute()`    — second
//!   - `ambient.rs::current_yday()`                — day-of-year
//!   - `phase_predictor.rs::local_secs_since_midnight()` — total seconds
//!
//! Each duplication had its own `MaybeUninit<libc::tm>`, `tzset()` OnceLock,
//! and `assume_init()` — identical boilerplate repeated 6 times. A bug in one
//! site (e.g. missing `tzset()`, wrong NULL check) had to be fixed 6 times.
//!
//! This module consolidates all POSIX time FFI into one verified path. Callers
//! get the broken-out `libc::tm` fields they need without touching unsafe code.
//!
//! ## Platform coverage
//!
//! - Unix (Linux/macOS/BSD/Termux): `libc::time + localtime_r/gmtime_r`.
//! - Non-Unix (Windows): Falls back to `SystemTime::now()` UTC-based
//!   computation. Less accurate (no local timezone), but sufficient for the
//!   scheduler and log-stamp use cases.

// ── Unix path: libc::time + localtime_r / gmtime_r ─────────────────────

/// Result of a POSIX `localtime_r` call. Fields are `Option<T>` to gracefully
/// handle the (extremely rare) case where `localtime_r` returns NULL.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LocalTm {
    pub hour: i32,
    pub minute: i32,
    pub second: i32,
    pub yday: i32,
}

/// Result of a POSIX `gmtime_r` call (UTC).
#[derive(Debug, Clone, Copy)]
pub(crate) struct UtcTm {
    pub year: i32,  // full year (e.g. 2026)
    pub month: i32, // 1..=12
    pub day: i32,   // 1..=31
    pub hour: i32,
    pub minute: i32,
    pub second: i32,
}

/// Call `libc::time(NULL)` → `libc::localtime_r`, returning parsed fields.
///
/// Returns `None` if `time()` returns -1 or `localtime_r` returns NULL.
///
/// `tzset()` is invoked once per process via `OnceLock` so that timezone
/// changes mid-run are reflected.
#[cfg(unix)]
#[must_use]
pub(crate) fn local_tm() -> Option<LocalTm> {
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

    // SAFETY: libc::time(NULL) — writes nothing when pointer is NULL,
    // returns time_t or -1 on error. No preconditions.
    let now = unsafe { libc::time(std::ptr::null_mut()) };
    if now < 0 {
        return None;
    }

    let mut tm: MaybeUninit<libc::tm> = MaybeUninit::uninit();
    // SAFETY: localtime_r is the thread-safe POSIX variant. It reads `now`
    // (a valid time_t >= 0) and writes into our MaybeUninit<tm> buffer.
    // Returns NULL on failure (handled below).
    if unsafe { libc::localtime_r(&now, tm.as_mut_ptr()) }.is_null() {
        return None;
    }
    // SAFETY: localtime_r returned non-NULL → tm fully initialized.
    let tm = unsafe { tm.assume_init() };

    Some(LocalTm {
        hour: tm.tm_hour,
        minute: tm.tm_min,
        second: tm.tm_sec,
        yday: tm.tm_yday,
    })
}

/// Call `libc::time(NULL)` → `libc::gmtime_r`, returning parsed UTC fields.
///
/// Returns a zeroed `UtcTm` on any failure (clock unavailable, gmtime_r fails).
#[cfg(unix)]
#[must_use]
pub(crate) fn utc_tm() -> UtcTm {
    use std::mem::MaybeUninit;

    let now = unsafe { libc::time(std::ptr::null_mut()) };
    if now < 0 {
        return UtcTm::zero();
    }

    let mut tm: MaybeUninit<libc::tm> = MaybeUninit::uninit();
    if unsafe { libc::gmtime_r(&now, tm.as_mut_ptr()) }.is_null() {
        return UtcTm::zero();
    }
    let tm = unsafe { tm.assume_init() };

    UtcTm {
        year: tm.tm_year + 1900,
        month: tm.tm_mon + 1,
        day: tm.tm_mday,
        hour: tm.tm_hour,
        minute: tm.tm_min,
        second: tm.tm_sec,
    }
}

#[cfg(unix)]
impl UtcTm {
    fn zero() -> Self {
        Self {
            year: 0,
            month: 1,
            day: 1,
            hour: 0,
            minute: 0,
            second: 0,
        }
    }
}

// ── Non-Unix fallback (Windows / unknown) ──────────────────────────────

#[cfg(not(unix))]
pub(crate) fn local_tm() -> Option<LocalTm> {
    // Non-unix: UTC-based approximation. No local timezone available
    // without pulling in winapi or chrono. Sufficient for scheduler.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let hour = ((secs / 3600) % 24) as i32;
    let min = ((secs / 60) % 60) as i32;
    let sec = (secs % 60) as i32;
    // Security audit SV-02 (2026-08-23): the previous `yday` computation
    // `(secs / 86_400) % 366` returned days-since-epoch mod 366, NOT the
    // day of year. Both change once per day, so the ambient scheduler's
    // day-boundary refire still worked, but the boundary landed at UTC
    // midnight with a wrong (offset by days-since-epoch-mod) value instead
    // of a real calendar day index. Derive the true day-of-year from the
    // same civil-from-days (Howard Hinnant) algorithm used by utc_tm()
    // below: day-of-year = day-of-era minus the era's Jan 1 offset.
    let days = secs / 86_400;
    let z = days as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    Some(LocalTm {
        hour,
        minute: min,
        second: sec,
        yday: doy as i32,
    })
}

#[cfg(not(unix))]
pub(crate) fn utc_tm() -> UtcTm {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86_400;
    let remainder = secs % 86_400;
    let hour = (remainder / 3600) as i32;
    let min = ((remainder / 60) % 60) as i32;
    let sec = (remainder % 60) as i32;
    // Civil-from-days algorithm (Howard Hinnant)
    let z = days as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    UtcTm {
        year: year as i32,
        month: m as i32,
        day: d as i32,
        hour,
        minute: min,
        second: sec,
    }
}

// ── Convenience helpers ─────────────────────────────────────────────────

impl LocalTm {
    /// Seconds since midnight: `hour3600 + min60 + sec`.
    #[must_use]
    pub(crate) fn secs_since_midnight(self) -> f64 {
        (self.hour as f64 * 3600.0) + (self.minute as f64 * 60.0) + self.second as f64
    }

    /// Minute of day: `hour*60 + min` (0..=1439).
    #[must_use]
    pub(crate) fn minute_of_day(self) -> u32 {
        (self.hour as u32) * 60 + (self.minute as u32)
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_tm_fields_bounded() {
        let tm = local_tm().expect("local_tm should succeed on this platform");
        assert!((0..24).contains(&tm.hour), "hour out of range: {}", tm.hour);
        assert!(
            (0..60).contains(&tm.minute),
            "minute out of range: {}",
            tm.minute
        );
        assert!(
            (0..60).contains(&tm.second),
            "second out of range: {}",
            tm.second
        );
        assert!(
            (0..=365).contains(&tm.yday),
            "yday out of range: {}",
            tm.yday
        );
    }

    #[test]
    fn utc_tm_fields_bounded() {
        let tm = utc_tm();
        assert!(tm.year >= 2025, "year too old: {}", tm.year);
        assert!(
            (1..=12).contains(&tm.month),
            "month out of range: {}",
            tm.month
        );
        assert!((1..=31).contains(&tm.day), "day out of range: {}", tm.day);
        assert!((0..24).contains(&tm.hour), "hour out of range: {}", tm.hour);
    }

    #[test]
    fn secs_since_midnight_bounded() {
        let tm = local_tm().expect("local_tm should succeed");
        let secs = tm.secs_since_midnight();
        assert!(
            (0.0..86_400.0).contains(&secs),
            "secs_since_midnight out of range: {}",
            secs
        );
    }

    #[test]
    fn minute_of_day_bounded() {
        let tm = local_tm().expect("local_tm should succeed");
        let mod_ = tm.minute_of_day();
        assert!(mod_ <= 1439, "minute_of_day out of range: {}", mod_);
    }

    #[test]
    fn local_tm_consistent_with_utc() {
        // Both should return roughly the same second (within 2s to avoid
        // rare boundary-crossing false positives).
        let local = local_tm().expect("local_tm should succeed");
        let utc = utc_tm();
        let diff = (local.second - utc.second).abs();
        assert!(diff <= 2, "local vs UTC second diff too large: {}", diff);
    }
}
