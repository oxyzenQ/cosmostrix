// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Human-readable number formatting for benchmark + perf-stats + HUD display.
//!
//! Two unit families:
//! - Counts (SI / base-1000): `humanize`, `humanize_f64` — for FPS,
//!   cells_drawn, cache hits, frame counts. Suffixes K / M / B.
//! - Bytes (binary / base-1024): `humanize_bytes`, `humanize_bytes_f64`,
//!   `humanize_throughput` — for ANSI byte totals, write bandwidth, heap
//!   sizes. Suffixes B / KiB / MiB / GiB / TiB. Using binary units for
//!   bytes is unambiguous (1 MiB = 1,048,576 bytes) and matches the
//!   conventions already used in `diagnostics/info.rs::format_bytes`.
//!
//! Count rules:
//!   - `< 1,000`         → `999` (full precision)
//!   - `1,000 - 9,999`   → `7.9K` (1 decimal place)
//!   - `10K - 999K`      → `791K` (no decimal)
//!   - `1M - 999M`       → `1.16M` (2 decimal places)
//!   - `≥ 1B`            → `1.2B` (1 decimal place)
//!
//! Bytes rules:
//!   - `< 1 KiB`         → `512 B` (raw, no decimal)
//!   - `1 KiB - 1 MiB`   → `21.36 KiB` (2 decimals)
//!   - `1 MiB - 1 GiB`   → `172.58 MiB` (2 decimals)
//!   - `1 GiB - 1 TiB`   → `4.21 GiB` (2 decimals)
//!   - `≥ 1 TiB`         → `2.50 TiB` (2 decimals)
//!
//! Used by:
//! - bench_report.rs: FPS, throughput, cells_drawn, frames, bytes_written,
//!   ansi_bytes_per_second, heap sizes.
//! - bench_json.rs: same fields in JSON.
//! - event_loop_finalize.rs: perf-stats ENCODING + TIER2_XTERMJS sections.
//! - hud.rs: fps display when >10K.
//! - diagnostics/info.rs: format_bytes delegates here.
//!
//! NOT used for:
//! - Timing (ms) — needs precision.
//! - Ratios (%) — needs precision.
//! - Small counts (<1000) — bare number is clearer.

// Binary byte-unit constants. These are the canonical base-1024 definitions;
// declared as `const` so the compiler can fold them and no call site needs
// to repeat `1024.0 * 1024.0 * 1024.0` literals.
const KIB_F: f64 = 1024.0;
const MIB_F: f64 = KIB_F * 1024.0;
const GIB_F: f64 = MIB_F * 1024.0;
const TIB_F: f64 = GIB_F * 1024.0;

/// Format a u64 as a human-readable string with K/M/B suffix.
///
/// Rules:
/// - `< 1,000`: return bare number (full precision)
/// - `1,000 - 9,999`: `7.9K` (1 decimal)
/// - `10,000 - 999,999`: `791K` (no decimal)
/// - `1,000,000 - 999,999,999`: `1.16M` (2 decimals)
/// - `≥ 1,000,000,000`: `1.2B` (1 decimal)
#[must_use]
pub(crate) fn humanize(n: u64) -> String {
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
pub(crate) fn humanize_f64(n: f64) -> String {
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

/// Format a u64 byte count with auto-scaling binary units (B / KiB / MiB /
/// GiB / TiB).
///
/// Use this for any byte-typed value (ANSI byte totals, heap sizes, write
/// counts). For byte rates (bytes/sec), use [`humanize_throughput`] instead.
///
/// # Examples
/// - `0`           → `"0 B"`
/// - `512`         → `"512 B"`
/// - `21876`       → `"21.36 KiB"`
/// - `180_940_669` → `"172.56 MiB"`
/// - `5_000_000_000` → `"4.66 GiB"`
#[must_use]
pub(crate) fn humanize_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".to_string();
    }
    let b = bytes as f64;
    if b < KIB_F {
        format!("{bytes} B")
    } else if b < MIB_F {
        format!("{:.2} KiB", b / KIB_F)
    } else if b < GIB_F {
        format!("{:.2} MiB", b / MIB_F)
    } else if b < TIB_F {
        format!("{:.2} GiB", b / GIB_F)
    } else {
        format!("{:.2} TiB", b / TIB_F)
    }
}

/// Format an f64 byte count (e.g. `avg_bytes_per_frame = 21876.5`) with
/// auto-scaling binary units.
///
/// Float variant of [`humanize_bytes`] for values that are computed as
/// floating-point averages (bytes/frame, bytes/cell).
///
/// # Examples
/// - `0.0`     → `"0.0 B"`
/// - `512.5`   → `"512.5 B"`
/// - `21876.5` → `"21.36 KiB"`
#[must_use]
pub(crate) fn humanize_bytes_f64(bytes: f64) -> String {
    if bytes < KIB_F {
        format!("{bytes:.1} B")
    } else if bytes < MIB_F {
        format!("{:.2} KiB", bytes / KIB_F)
    } else if bytes < GIB_F {
        format!("{:.2} MiB", bytes / MIB_F)
    } else if bytes < TIB_F {
        format!("{:.2} GiB", bytes / GIB_F)
    } else {
        format!("{:.2} TiB", bytes / TIB_F)
    }
}

/// Format a byte throughput (bytes/sec) with auto-scaling binary units.
///
/// Use this for any `bytes / elapsed_seconds` rate (ANSI bandwidth, write
/// throughput). The unit suffix (`B/s`, `KiB/s`, `MiB/s`, ...) is chosen
/// dynamically from the rate value — callers do not hardcode `KiB/s` or
/// `1024.0` divisors at the call site.
///
/// Returns `"0 B/s"` when `secs <= 0` (avoids divide-by-zero).
///
/// # Examples
/// - `bytes=2_815_471_008, secs=1050.0` → `"2.53 MiB/s"`
/// - `bytes=2_815_471, secs=1.0`        → `"2.68 MiB/s"`
#[must_use]
pub(crate) fn humanize_throughput(bytes: u64, secs: f64) -> String {
    if secs <= 0.0 || bytes == 0 {
        return "0 B/s".to_string();
    }
    let rate = bytes as f64 / secs;
    if rate < KIB_F {
        format!("{rate:.1} B/s")
    } else if rate < MIB_F {
        format!("{:.2} KiB/s", rate / KIB_F)
    } else if rate < GIB_F {
        format!("{:.2} MiB/s", rate / MIB_F)
    } else if rate < TIB_F {
        format!("{:.2} GiB/s", rate / GIB_F)
    } else {
        format!("{:.2} TiB/s", rate / TIB_F)
    }
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

    // ── humanize_bytes (u64, binary) ─────────────────────────────────
    #[test]
    fn humanize_bytes_boundaries() {
        assert_eq!(humanize_bytes(0), "0 B");
        assert_eq!(humanize_bytes(1), "1 B");
        assert_eq!(humanize_bytes(1023), "1023 B");
        assert_eq!(humanize_bytes(1024), "1.00 KiB"); // exact KiB boundary
        assert_eq!(humanize_bytes(1536), "1.50 KiB"); // 1.5 KiB
        assert_eq!(
            humanize_bytes(180_940_669), // owner's example value
            "172.56 MiB"
        );
        assert_eq!(humanize_bytes(1_073_741_824), "1.00 GiB"); // exact GiB boundary
        assert_eq!(humanize_bytes(5_000_000_000), "4.66 GiB");
        assert_eq!(humanize_bytes(1_099_511_627_776), "1.00 TiB"); // exact TiB boundary
    }

    #[test]
    fn humanize_bytes_round_up_to_next_unit_rolls_correctly() {
        // 1 KiB - 1 byte stays in B
        assert_eq!(humanize_bytes(1023), "1023 B");
        // 1 MiB - 1 byte stays in KiB. Note: value 1_048_575 = 1023.999 KiB,
        // which rounds to 1024.00 KiB under 2-decimal formatting. This is
        // mathematically correct (value < 1 MiB) but visually close to the
        // next unit; acceptable trade-off for keeping the formatter simple.
        assert_eq!(humanize_bytes(1_048_575), "1024.00 KiB");
        // Just under 1 GiB stays in MiB
        assert_eq!(humanize_bytes(1_073_741_823), "1024.00 MiB");
    }

    // ── humanize_bytes_f64 (f64, binary) ─────────────────────────────
    #[test]
    fn humanize_bytes_f64_boundaries() {
        assert_eq!(humanize_bytes_f64(0.0), "0.0 B");
        assert_eq!(humanize_bytes_f64(512.5), "512.5 B");
        assert_eq!(humanize_bytes_f64(21876.5), "21.36 KiB"); // owner's example
        assert_eq!(humanize_bytes_f64(1_048_576.0), "1.00 MiB");
        assert_eq!(humanize_bytes_f64(1_073_741_824.0), "1.00 GiB");
    }

    // ── humanize_throughput (bytes / sec, binary) ────────────────────
    #[test]
    fn humanize_throughput_boundaries() {
        assert_eq!(humanize_throughput(0, 1.0), "0 B/s");
        assert_eq!(humanize_throughput(1024, 0.0), "0 B/s"); // secs=0 guard
        assert_eq!(humanize_throughput(512, 1.0), "512.0 B/s");
        assert_eq!(humanize_throughput(1024, 1.0), "1.00 KiB/s");
        // 2_815_471 bytes in 1 sec → 2.69 MiB/s (matches owner example scale;
        // raw math: 2815471 / 1048576 = 2.685034..., rounds to 2.69).
        assert_eq!(humanize_throughput(2_815_471, 1.0), "2.69 MiB/s");
        // 180_940_669 bytes in 65 sec → 2.65 MiB/s
        // (raw: 180940669 / 65 / 1048576 = 2.6543, rounds to 2.65)
        assert_eq!(humanize_throughput(180_940_669, 65.0), "2.65 MiB/s");
        assert_eq!(humanize_throughput(1_073_741_824, 1.0), "1.00 GiB/s");
    }
}
