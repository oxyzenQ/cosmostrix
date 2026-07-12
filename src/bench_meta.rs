// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Metric meaning constants + formatting helpers for the benchmark report.
//!
//! Extracted from `bench_report.rs` to keep that file under its 1000-LOC
//! guard after the v11.1.0 SYSTEM + RESOURCE section expansions. Contains
//! the documentation strings that explain what each metric measures, plus
//! the `format_rss_kb` and `cpu_model_label` helpers.

// ── Metric meaning constants ──────────────────────────────────────────────
//
// These document what each benchmark metric measures. They appear in the
// premium benchmark output and are referenced by tests to prevent
// accidental removal or misleading wording changes.

#[allow(dead_code)]
pub(crate) const DRAW_RATIO_MEANING: &str =
    "legacy compatibility: percentage of frames with >=1 dirty cell";
pub(crate) const ACTIVE_FRAME_RATIO_MEANING: &str =
    "frames that produced at least one dirty cell during measurement";
pub(crate) const AVG_DIRTY_CELL_RATIO_MEANING: &str =
    "average dirty-cell coverage across all measured frames";
#[allow(dead_code)]
pub(crate) const DIRTY_ALL_FRAMES_MEANING: &str =
    "logical frames where every cell was dirty; distinct from terminal redraw estimate";
#[allow(dead_code)]
pub(crate) const ESTIMATED_FULL_REDRAW_MEANING: &str =
    "threshold estimate of frames likely to use Terminal::draw full-redraw path";

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Format a KiB RSS value as a human-readable string with binary suffix.
///
/// Examples: 512 → "512 KiB", 2048 → "2.0 MiB", 1572864 → "1.5 GiB".
pub(crate) fn format_rss_kb(kib: u64) -> String {
    const MIB: u64 = 1024;
    const GIB: u64 = 1024 * 1024;
    if kib >= GIB {
        format!("{:.2} GiB", kib as f64 / GIB as f64)
    } else if kib >= MIB {
        format!("{:.1} MiB", kib as f64 / MIB as f64)
    } else {
        format!("{kib} KiB")
    }
}

/// Return the CPU model string for the SYSTEM section, or "unknown" if
/// detection is unavailable on this platform.
pub(crate) fn cpu_model_label() -> String {
    crate::diagnostics::cpu_model_string().unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_rss_kb_renders_human_readable_suffixes() {
        assert_eq!(format_rss_kb(0), "0 KiB");
        assert_eq!(format_rss_kb(512), "512 KiB");
        assert_eq!(format_rss_kb(1023), "1023 KiB");
        assert_eq!(format_rss_kb(1024), "1.0 MiB");
        assert_eq!(format_rss_kb(2048), "2.0 MiB");
        assert_eq!(format_rss_kb(1_572_864), "1.50 GiB");
        assert!(format_rss_kb(1_048_576).ends_with("GiB"));
    }

    #[test]
    fn cpu_model_label_returns_unknown_or_real_string() {
        let label = cpu_model_label();
        // On Linux/macOS this should return a real CPU model string
        // (non-empty, non-"unknown"). On other platforms it returns
        // "unknown". Either way it must be non-empty.
        assert!(!label.is_empty(), "cpu_model_label must return non-empty");
    }

    #[test]
    fn metric_meaning_constants_are_non_empty() {
        assert!(!DRAW_RATIO_MEANING.is_empty());
        assert!(!ACTIVE_FRAME_RATIO_MEANING.is_empty());
        assert!(!AVG_DIRTY_CELL_RATIO_MEANING.is_empty());
        assert!(!DIRTY_ALL_FRAMES_MEANING.is_empty());
        assert!(!ESTIMATED_FULL_REDRAW_MEANING.is_empty());
    }
}
