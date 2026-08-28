// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Peak FPS computation helper — extracted from `bench/mod.rs` to keep
//! that file under the 1500-LOC cap.
//!
//! Owns the `compute_peak_fps()` function: a pure, stateless helper that
//! computes peak FPS from a sorted ascending slice of frame times in ms.
//!
//! Used by:
//! - `bench::run_premium_benchmark` — for the peak FPS metric
//! - `bench_tests` — for regression coverage of the p1 trim + clock-floor
//!
//! Re-exported from `bench/mod.rs` via `pub(crate) use` so all existing
//! `compute_peak_fps(...)` call sites (including `use super::*` glob in
//! bench_tests.rs) continue to resolve without changes.

/// Minimum frame time (in ms) considered a real frame rather than a clock
/// artifact. Anything below 1µs is physically impossible on real hardware
/// — see `compute_peak_fps` doc for the full rationale.
const PEAK_FPS_MIN_FRAME_MS: f64 = 0.001; // 1µs

/// Compute peak FPS from a sorted ascending slice of frame times (in ms).
///
/// "Peak FPS" is defined as `1000 / min_real_frame_time_ms`, where
/// `min_real_frame_time_ms` is the smallest sample that survives BOTH:
/// 1. p1 percentile trim (skip fastest 1% to absorb sparse outliers), AND
/// 2. absolute floor of `PEAK_FPS_MIN_FRAME_MS` (skip clock artifacts
///    that cluster densely enough to survive percentile trimming).
///
/// # Why both p1 AND a floor?
///
/// The original p1-only fix (commit 6b093f1) assumed outliers were sparse
/// (< 1% of samples). On FreeBSD's fast path this assumption fails: when
/// `Instant::elapsed()` returns sub-microsecond deltas for hundreds of
/// consecutive frames (TSC read hits the same cycle, or kernel clock
/// rounds down), the outlier cluster exceeds 1% and survives the p1 trim.
/// This produced absurd values like 3,584,229 FPS even AFTER the p1 fix.
///
/// The absolute floor catches what percentile trimming cannot: any sample
/// below 1µs is a clock artifact regardless of how many there are. This
/// is a physics-based filter, not a statistical one — it cannot over-trim
/// real frame times because no real frame completes in < 1µs.
///
/// # Zero-sample handling
///
/// Samples equal to `0.0` (clock resolution coarser than frame time) are
/// also skipped by the `> PEAK_FPS_MIN_FRAME_MS` filter. If ALL samples
/// are below the floor, we fall back to `0.0` (honest "not measurable"
/// answer for coarse-clock systems).
///
/// # Arguments
///
/// * `sorted_ft` - Frame times in ms, sorted ascending. Caller is
///   responsible for sorting (this fn does NOT re-sort, to avoid O(n log n)
///   on every call when the caller already has a sorted slice for p99/p95).
/// * `count` - Number of valid samples in `sorted_ft` (may be less than
///   `sorted_ft.len()` if the buffer is a fixed-size array partially filled).
///
/// # Returns
///
/// `peak_fps` in frames-per-second. `0.0` if no samples exceed the floor,
/// or if `count == 0`.
pub(crate) fn compute_peak_fps(sorted_ft: &[f64], count: usize) -> f64 {
    if count == 0 {
        return 0.0;
    }
    let p1_count = (count as f64 * 0.01) as usize;
    let p1_idx = p1_count.min(count.saturating_sub(1));
    // Scan p1+ range for the first sample above the absolute floor.
    // This skips BOTH zero samples AND sub-microsecond clock artifacts
    // that survive percentile trimming on FreeBSD's fast path.
    let min_ft = sorted_ft[p1_idx..count]
        .iter()
        .copied()
        .find(|&t| t > PEAK_FPS_MIN_FRAME_MS)
        .unwrap_or(0.0);
    if min_ft > 0.0 {
        1000.0 / min_ft
    } else {
        0.0
    }
}
