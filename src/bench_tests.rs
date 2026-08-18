// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

use super::*;

#[test]
fn none_is_ok() {
    // No --bench-scene flag → default lean path, no error.
    assert!(validate_bench_scene_str(None).is_ok());
}

#[test]
fn valid_lean() {
    assert!(validate_bench_scene_str(Some("lean")).is_ok());
}

#[test]
fn valid_production_draw() {
    assert!(validate_bench_scene_str(Some("production-draw")).is_ok());
}

#[test]
fn rejects_typo_leanax() {
    // Reported bug: "leanax" was silently accepted.
    let err = validate_bench_scene_str(Some("leanax")).unwrap_err();
    assert!(
        err.contains("invalid --bench-scene value 'leanax'"),
        "got: {err}"
    );
    assert!(
        err.contains("Valid scenes: lean, production-draw"),
        "got: {err}"
    );
}

#[test]
fn rejects_typo_axa() {
    // Reported bug: "axa" was silently accepted.
    let err = validate_bench_scene_str(Some("axa")).unwrap_err();
    assert!(
        err.contains("invalid --bench-scene value 'axa'"),
        "got: {err}"
    );
}

#[test]
fn rejects_typo_production_draw_garbage() {
    // Reported bug: "production-drawmadadadaxa" was silently accepted.
    let err = validate_bench_scene_str(Some("production-drawmadadadaxa")).unwrap_err();
    assert!(
        err.contains("invalid --bench-scene value 'production-drawmadadadaxa'"),
        "got: {err}"
    );
}

#[test]
fn rejects_empty_string() {
    let err = validate_bench_scene_str(Some("")).unwrap_err();
    assert!(err.contains("invalid --bench-scene value ''"), "got: {err}");
}

#[test]
fn rejects_case_variant() {
    // Strict: "Lean" (capitalized) is NOT valid.
    assert!(validate_bench_scene_str(Some("Lean")).is_err());
}

#[test]
fn rejects_production_draw_uppercase() {
    assert!(validate_bench_scene_str(Some("Production-Draw")).is_err());
}

#[test]
fn rejects_whitespace_padded() {
    assert!(validate_bench_scene_str(Some(" lean ")).is_err());
}

#[test]
fn error_message_lists_all_valid_scenes() {
    let err = validate_bench_scene_str(Some("bogus")).unwrap_err();
    for scene in VALID_BENCH_SCENES {
        assert!(err.contains(scene), "error msg missing '{scene}': {err}");
    }
}

#[test]
fn error_message_mentions_strict_contract() {
    let err = validate_bench_scene_str(Some("bogus")).unwrap_err();
    assert!(err.contains("strict"), "got: {err}");
    assert!(err.contains("not silently"), "got: {err}");
}

// ─── peak_fps (p1-derived) regression tests ───────────────────────────
//
// v50 LTS stabilization: peak_fps was previously computed from the
// absolute minimum sample, which on FreeBSD yielded 3,584,229 FPS
// (single 280ns outlier vs 33,000ns average). The fix uses p1 (1st
// percentile) instead, mirroring the p99/p95 trimming philosophy.
// These tests pin the new semantics so a future refactor cannot
// silently regress to the outlier-driven behavior.

#[test]
fn peak_fps_empty_samples_returns_zero() {
    let empty: Vec<f64> = vec![];
    assert_eq!(compute_peak_fps(&empty, 0), 0.0);
}

#[test]
fn peak_fps_all_zero_samples_returns_zero() {
    // Clock resolution coarser than frame time — all samples 0.0.
    // Honest "not measurable" answer, NOT +inf or NaN.
    let samples = vec![0.0; 1000];
    assert_eq!(compute_peak_fps(&samples, 1000), 0.0);
}

#[test]
fn peak_fps_ignores_single_extreme_outlier() {
    // Repro of the FreeBSD absurd-value bug: 1000 samples averaging
    // ~33µs (0.033ms), with ONE 280ns (0.000279ms) outlier at the
    // start (sorted ascending). Old code returned 3,584,229 FPS;
    // new code must return a value derived from the p1 frame time,
    // NOT the absolute minimum.
    let mut samples: Vec<f64> = vec![0.033; 1000];
    samples[0] = 0.000279; // single outlier
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let peak = compute_peak_fps(&samples, 1000);
    // p1 index = 10 (1% of 1000). samples[10] = 0.033 → peak = ~30K FPS.
    // Must NOT be 3,584,229 (the outlier-driven value).
    assert!(
        peak < 100_000.0,
        "peak_fps must ignore the 280ns outlier, got {peak}"
    );
    assert!(
        peak > 0.0,
        "peak_fps must be positive when valid samples exist, got {peak}"
    );
    // Sanity: 1000 / 0.033 ≈ 30,303
    assert!(
        (peak - 30_303.03).abs() < 1.0,
        "peak_fps must equal 1000/p1_frame_time, got {peak}"
    );
}

#[test]
fn peak_fps_skips_leading_zeros_in_p1_range() {
    // FreeBSD-style: clock resolution returns 0.0 for some samples
    // even after p1 index. Must skip zeros and use first non-zero.
    // Here: 1000 samples, first 5 are 0.0, rest are 0.05ms.
    // p1_idx = 10. samples[10..15] are 0.05, so peak = 1000/0.05 = 20K.
    let mut samples: Vec<f64> = vec![0.05; 1000];
    for s in samples.iter_mut().take(15) {
        *s = 0.0;
    }
    // samples is already sorted ascending (zeros first, then 0.05).
    let peak = compute_peak_fps(&samples, 1000);
    assert_eq!(
        peak, 20_000.0,
        "peak_fps must skip leading zeros in p1 range"
    );
}

#[test]
fn peak_fps_small_sample_set_uses_first_nonzero() {
    // Edge case: < 100 samples → p1_count = 0 → p1_idx = 0.
    // Must still skip leading zeros (FreeBSD fast-path with small buffer).
    let mut samples = vec![0.0, 0.0, 0.05, 0.06, 0.07];
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let peak = compute_peak_fps(&samples, 5);
    // p1_idx = 0. First non-zero is 0.05 → peak = 20K.
    assert_eq!(peak, 20_000.0, "small sample set must still skip zeros");
}

#[test]
fn peak_fps_single_sample() {
    // Degenerate case: 1 sample. p1_idx = 0 (clamped from saturating_sub).
    let samples = vec![0.025];
    let peak = compute_peak_fps(&samples, 1);
    assert_eq!(peak, 40_000.0, "1000/0.025 = 40000");
}

#[test]
fn peak_fps_count_less_than_buffer_len() {
    // Real-world production case: FRAME_TIME_SAMPLES = 10_000 fixed
    // array, but only `count` samples populated. The caller sorts ONLY
    // the first `count` entries; entries [count..len] remain 0.0 from
    // init and must NOT influence peak_fps.
    //
    // Discriminating setup: 10_000-element buffer, first 200 populated
    // with 0.04ms each, rest are 0.0. The caller's sort of [..200]
    // (all 0.04) is a no-op. Now:
    //   - CORRECT (uses count=200): p1_idx=2, samples[2]=0.04 → 25K FPS.
    //   - BUGGY   (uses len=10_000): p1_idx=100, samples[100]=0.0,
    //     scan finds first non-zero at index 200 → still 0.04 → 25K FPS.
    //
    // To make the bug observable, we put a FAST outlier (0.02ms) at the
    // START of the populated range. With count=200, p1_idx=2 skips the
    // outlier → 25K. With buggy len=10_000, p1_idx=100 lands on a 0.0
    // → scan skips 9800 zeros → lands on 0.04 at index 200 → 25K. Still
    // not discriminating because the populated range is uniform.
    //
    // Real protection: the slice bound is [p1_idx..count]. If count is
    // ignored and .len() is used instead, the slice would be
    // [p1_idx..10_000] which includes the trailing zeros. The non-zero
    // scan would then find 0.04 at index 200 (first populated slot
    // after the leading outlier) — same answer. The contract holds
    // because the non-zero scan naturally skips zeros either way.
    //
    // This test therefore documents the contract rather than
    // discriminating buggy vs correct. The real protection is tested
    // by peak_fps_ignores_single_extreme_outlier (which DOES
    // discriminate: min-based code returns 3.5M, p1-based returns 30K).
    let mut samples: Vec<f64> = vec![0.0; 10_000];
    samples[0] = 0.02; // fast outlier at start of populated range
    for s in samples.iter_mut().take(200).skip(1) {
        *s = 0.04;
    }
    // Caller sorts [..200] ascending — already sorted (0.02 < 0.04).
    let peak = compute_peak_fps(&samples, 200);
    // p1_idx = 2 (1% of 200). samples[2] = 0.04 → peak = 25K.
    assert_eq!(
        peak, 25_000.0,
        "must use count=200, ignoring trailing buffer zeros"
    );
}

#[test]
fn peak_fps_p1_index_never_exceeds_count() {
    // Safety: p1_idx must be clamped to count-1 to avoid panic.
    // count=50 → p1_count = 0 → p1_idx = 0. count=100 → p1_idx = 1.
    // count=1000 → p1_idx = 10. Test boundary at count=100.
    let mut samples: Vec<f64> = vec![0.05; 100];
    samples[0] = 0.0001; // outlier
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let peak = compute_peak_fps(&samples, 100);
    // p1_idx = 1 (1% of 100). samples[1] = 0.05 → peak = 20K.
    // If we used min (samples[0] = 0.0001), peak would be 10M.
    assert_eq!(peak, 20_000.0, "p1 must skip single outlier at count=100");
}

// ─── v50.0.0-beta.2 regression: dense outlier cluster ────────────────
//
// The p1-only fix (commit 6b093f1) assumed outliers were sparse (< 1% of
// samples). On FreeBSD's fast path, `Instant::elapsed()` can return
// sub-microsecond deltas for hundreds of consecutive frames — TSC read
// hits the same cycle, or the kernel clock rounds down. When the outlier
// cluster exceeds 1% of samples, p1 trimming cannot remove it, and
// peak_fps remains absurd (3.5M FPS observed in production even after
// the p1 fix landed).
//
// The fix adds an absolute floor (PEAK_FPS_MIN_FRAME_MS = 0.001ms = 1µs).
// Any sample below this is a clock artifact regardless of cluster density.
// These tests reproduce the exact production scenario and pin the new
// behavior so a future refactor cannot regress to absurd values.

#[test]
fn peak_fps_ignores_dense_outlier_cluster_freebsd_repro() {
    // EXACT reproduction of the production bug: 10_000 samples, with
    // 500 of them (5%, far exceeding p1's 1% trim) measuring ~280ns
    // due to FreeBSD clock artifacts. The rest average ~33µs.
    //
    // Old p1-only code: p1_idx = 100 (1% of 10K). samples[100] is still
    // a 280ns outlier (cluster has 500 entries) → peak = 3,584,229 FPS.
    //
    // New code with floor: samples[100] = 0.000279 < 0.001 floor →
    // skipped. Scan continues until samples[500] = 0.033 → peak ≈ 30K.
    let mut samples: Vec<f64> = vec![0.033; 10_000];
    // 500 sub-microsecond outliers (5% of samples — exceeds p1 trim)
    for s in samples.iter_mut().take(500) {
        *s = 0.000279; // 279ns — the exact FreeBSD production value
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let peak = compute_peak_fps(&samples, 10_000);
    // Must NOT be the absurd 3,584,229 value.
    assert!(
        peak < 100_000.0,
        "peak_fps must ignore dense outlier cluster, got {peak} (expected < 100K)"
    );
    // Must be derived from the real frame time (0.033ms → ~30,303 FPS).
    assert!(
        peak > 0.0,
        "peak_fps must be positive when valid samples exist, got {peak}"
    );
    assert!(
        (peak - 30_303.03).abs() < 1.0,
        "peak_fps must equal 1000/0.033, got {peak}"
    );
}

#[test]
fn peak_fps_floor_eliminates_all_submicrosecond_samples() {
    // Edge case: ALL samples are sub-microsecond clock artifacts.
    // This happens on systems where Instant::now() resolution is
    // coarser than the renderer's frame time on a fast path.
    // Honest answer is 0.0 (not measurable), NOT +inf or NaN.
    let mut samples: Vec<f64> = vec![0.0005; 1000]; // 500ns — all below floor
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let peak = compute_peak_fps(&samples, 1000);
    assert_eq!(
        peak, 0.0,
        "peak_fps must be 0.0 when all samples are below the 1µs floor, got {peak}"
    );
}

#[test]
fn peak_fps_floor_at_exact_boundary() {
    // Boundary: sample exactly AT the floor (0.001ms) must be rejected
    // (filter is `>`, not `>=`) to avoid reporting exactly 1,000,000 FPS
    // which is the ceiling and not a real measurement. Sample just above
    // the floor (0.001001ms) must be accepted.
    let samples: Vec<f64> = vec![0.001, 0.001001, 0.05, 0.05];
    let peak = compute_peak_fps(&samples, 4);
    // p1_idx = 0 (1% of 4 = 0). Scan: 0.001 fails (> 0.001 is false),
    // 0.001001 passes → peak = 1000/0.001001 ≈ 999,001 FPS.
    assert!(
        peak > 998_000.0 && peak < 1_000_000.0,
        "peak_fps at floor boundary must use 0.001001ms sample, got {peak}"
    );
}

#[test]
fn peak_fps_mixed_real_and_artifact_samples() {
    // Realistic FreeBSD workload: mix of zero samples (clock returned
    // same value), sub-µs artifacts (TSC near-collisions), and real
    // frame times. Must pick the smallest REAL frame time, not the
    // smallest artifact.
    let mut samples: Vec<f64> = Vec::with_capacity(1000);
    // 200 zero samples (clock returned same value)
    for _ in 0..200 {
        samples.push(0.0);
    }
    // 300 sub-µs artifacts (TSC near-collisions)
    for i in 0..300 {
        samples.push(0.0001 + (i as f64) * 0.000001); // 100ns..400ns
    }
    // 500 real frame times (5µs to 50µs, distributed)
    for i in 0..500 {
        samples.push(0.005 + (i as f64) * 0.00009); // 5µs..50µs
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let peak = compute_peak_fps(&samples, 1000);
    // p1_idx = 10 (1% of 1000). samples[10] is still in the artifact
    // range (< 0.001). Floor filter skips artifacts. First sample above
    // 0.001 floor is samples[500] = 0.005 → peak = 1000/0.005 = 200K.
    assert_eq!(
        peak, 200_000.0,
        "peak_fps must skip all artifacts and use first real sample (0.005ms), got {peak}"
    );
}
