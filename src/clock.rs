// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Centralized wall-clock helpers (Hinnant-style minimal abstraction).
//!
//! All POSIX FFI (`libc::time`, `localtime_r`, `gmtime_r`) is consolidated in
//! `crate::posix_time`. This module consumes the parsed structs and exposes
//! the public API used by the rest of cosmostrix.

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
/// Used by `SystemFeeling::tick()` for time-of-day state classification.
#[must_use]
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
