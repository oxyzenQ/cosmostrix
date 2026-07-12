// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Human-readable number formatting for benchmark + HUD display.
//!
//! Converts large numbers to compact K/M/B suffix form:
//!   - `< 1,000`         → `999` (full precision)
//!   - `1,000 - 9,999`   → `7.9K` (1 decimal place)
//!   - `10K - 999K`      → `791K` (no decimal)
//!   - `1M - 999M`       → `1.16M` (2 decimal places)
//!   - `≥ 1B`            → `1.2B` (1 decimal place)
//!
//! Used by:
//! - bench_report.rs: FPS, throughput, cells_drawn, frames
//! - bench_json.rs: same fields in JSON
//! - hud.rs: fps display when >10K
//!
//! NOT used for:
//! - Timing (ms) — needs precision
//! - Ratios (%) — needs precision
//! - RSS (already MiB)
//! - Small numbers (<1000)

/// Format a u64 as a human-readable string with K/M/B suffix.
///
/// Rules:
/// - `< 1,000`: return bare number (full precision)
/// - `1,000 - 9,999`: `7.9K` (1 decimal)
/// - `10,000 - 999,999`: `791K` (no decimal)
/// - `1,000,000 - 999,999,999`: `1.16M` (2 decimals)
/// - `≥ 1,000,000,000`: `1.2B` (1 decimal)
#[must_use]
pub fn humanize(n: u64) -> String {
    if n < 1_000 {
        return n.to_string();
    }
    if n < 10_000 {
        // 1K - 9.9K: 1 decimal place
        let k = n as f64 / 1_000.0;
        return format!("{k:.1}K");
    }
    if n < 1_000_000 {
        // 10K - 999K: no decimal. Use round to handle 999,999 → 1000K edge.
        let k = (n as f64 / 1_000.0).round() as u64;
        if k >= 1000 {
            // Rolled to 1M
            let m = n as f64 / 1_000_000.0;
            return format!("{m:.2}M");
        }
        return format!("{k}K");
    }
    if n < 1_000_000_000 {
        // 1M - 999M: 2 decimal places
        let m = n as f64 / 1_000_000.0;
        return format!("{m:.2}M");
    }
    // 1B+: 1 decimal place
    let b = n as f64 / 1_000_000_000.0;
    format!("{b:.1}B")
}

/// Format an f64 as a human-readable string with K/M/B suffix.
///
/// Same rules as `humanize()` but for float values (e.g. avg_fps = 38143.3).
/// Rounds to integer before applying suffix logic.
#[must_use]
pub fn humanize_f64(n: f64) -> String {
    if n < 1_000.0 {
        return format!("{n:.0}");
    }
    if n < 10_000.0 {
        let k = n / 1_000.0;
        return format!("{k:.1}K");
    }
    if n < 1_000_000.0 {
        let k = (n / 1_000.0).round() as u64;
        return format!("{k}K");
    }
    if n < 1_000_000_000.0 {
        let m = n / 1_000_000.0;
        return format!("{m:.2}M");
    }
    let b = n / 1_000_000_000.0;
    format!("{b:.1}B")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humanize_small_numbers_no_suffix() {
        assert_eq!(humanize(0), "0");
        assert_eq!(humanize(1), "1");
        assert_eq!(humanize(999), "999");
    }

    #[test]
    fn humanize_thousands_1_decimal() {
        assert_eq!(humanize(1_000), "1.0K");
        assert_eq!(humanize(7_900), "7.9K");
        assert_eq!(humanize(9_999), "10.0K");
    }

    #[test]
    fn humanize_thousands_no_decimal() {
        assert_eq!(humanize(10_000), "10K");
        assert_eq!(humanize(791_038), "791K");
        assert_eq!(humanize(999_999), "1.00M"); // rolls to M
    }

    #[test]
    fn humanize_millions_2_decimals() {
        assert_eq!(humanize(1_000_000), "1.00M");
        assert_eq!(humanize(1_161_440), "1.16M");
        assert_eq!(humanize(189_403_992), "189.40M");
    }

    #[test]
    fn humanize_billions_1_decimal() {
        assert_eq!(humanize(1_000_000_000), "1.0B");
        assert_eq!(humanize(1_200_000_000), "1.2B");
        assert_eq!(humanize(854_006_868), "854.01M"); // still M
    }

    // f64 versions
    #[test]
    fn humanize_f64_small() {
        assert_eq!(humanize_f64(0.0), "0");
        assert_eq!(humanize_f64(60.0), "60");
        assert_eq!(humanize_f64(999.9), "1000");
    }

    #[test]
    fn humanize_f64_thousands() {
        assert_eq!(humanize_f64(7_900.0), "7.9K");
        assert_eq!(humanize_f64(38_143.0), "38K");
        assert_eq!(humanize_f64(791_038.0), "791K");
    }

    #[test]
    fn humanize_f64_millions() {
        assert_eq!(humanize_f64(1_161_440.0), "1.16M");
        assert_eq!(humanize_f64(189_403_992.0), "189.40M");
    }

    #[test]
    fn humanize_f64_billions() {
        assert_eq!(humanize_f64(1_200_000_000.0), "1.2B");
    }
}
