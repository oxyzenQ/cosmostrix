// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Small deterministic helpers for the Zactrix Core architecture seam.
//!
//! Zactrix Core is an internal discipline for turning renderer observations
//! into bounded, verifiable decisions. It is inspired by eBPF architecture
//! shapes (probe, map, filter, verifier, bounded history), but it is plain
//! stable Rust and has no Linux eBPF, root, kernel, or BPF dependency.

/// Classify measured frame-time jitter for benchmark reporting.
#[must_use]
pub(crate) fn classify_frame_jitter(jitter_std_ms: f64) -> &'static str {
    if jitter_std_ms < 0.5 {
        "low"
    } else if jitter_std_ms < 2.0 {
        "medium"
    } else {
        "high"
    }
}

/// Classify frame-time stability for benchmark reporting.
#[must_use]
pub(crate) fn classify_frame_time_stability(jitter_std_ms: f64) -> &'static str {
    if jitter_std_ms < 0.3 {
        "excellent"
    } else if jitter_std_ms < 0.5 {
        "good"
    } else if jitter_std_ms < 2.0 {
        "moderate"
    } else {
        "high"
    }
}

/// Compute the dirty-cell threshold used to estimate full terminal redraws.
#[must_use]
pub(crate) fn dirty_threshold_cells(total_cells: usize, threshold_divisor: usize) -> usize {
    if total_cells > 0 && threshold_divisor > 0 {
        total_cells / threshold_divisor
    } else {
        0
    }
}

/// Decide whether a frame crosses the estimated full-redraw threshold.
#[must_use]
pub(crate) fn estimates_full_redraw(
    total_cells: usize,
    dirty_cells: usize,
    dirty_all: bool,
    threshold_divisor: usize,
) -> bool {
    dirty_all
        || (total_cells > 0 && dirty_cells >= dirty_threshold_cells(total_cells, threshold_divisor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn benchmark_stability_classification_boundaries_match_existing_output() {
        assert_eq!(classify_frame_jitter(0.49), "low");
        assert_eq!(classify_frame_jitter(0.5), "medium");
        assert_eq!(classify_frame_jitter(2.0), "high");

        assert_eq!(classify_frame_time_stability(0.29), "excellent");
        assert_eq!(classify_frame_time_stability(0.3), "good");
        assert_eq!(classify_frame_time_stability(0.5), "moderate");
        assert_eq!(classify_frame_time_stability(2.0), "high");
    }

    #[test]
    fn dirty_redraw_estimator_is_bounded() {
        assert_eq!(dirty_threshold_cells(0, 3), 0);
        assert_eq!(dirty_threshold_cells(1200, 3), 400);
        assert!(!estimates_full_redraw(0, 1, false, 3));
        assert!(estimates_full_redraw(1200, 400, false, 3));
        assert!(estimates_full_redraw(1200, 0, true, 3));
    }
}
