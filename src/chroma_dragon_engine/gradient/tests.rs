// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! gradient tests, extracted from inline `mod tests { ... }` block.
//!
//! Uses `use super::*;` to access parent's private items unchanged.

use super::*;

/// Single stop → vec filled with that stop.
#[test]
fn single_stop_repeats() {
    let out = gradient_from_stops_oklab(&[(10, 20, 30)], 5);
    assert_eq!(out.len(), 5);
    for c in &out {
        assert_eq!(*c, (10, 20, 30));
    }
}

/// Empty stops → empty output.
#[test]
fn empty_stops_returns_empty() {
    let out = gradient_from_stops_oklab(&[], 5);
    assert!(out.is_empty());
}

/// Zero steps → empty output.
#[test]
fn zero_steps_returns_empty() {
    let out = gradient_from_stops_oklab(&[(0, 0, 0), (255, 255, 255)], 0);
    assert!(out.is_empty());
}

/// One step → first stop only.
#[test]
fn one_step_returns_first_stop() {
    let out = gradient_from_stops_oklab(&[(10, 20, 30), (200, 100, 50)], 1);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0], (10, 20, 30));
}

/// Endpoints are preserved exactly (t=0 → first, t=1 → last).
#[test]
fn endpoints_preserved() {
    let out = gradient_from_stops_oklab(&[(10, 20, 30), (200, 100, 50)], 9);
    assert_eq!(out.len(), 9);
    assert_eq!(out[0], (10, 20, 30));
    assert_eq!(out[8], (200, 100, 50));
}

/// Round-trip srgb → oklab → srgb preserves the original within ±1 unit.
/// Exhaustive check would be 16M iterations; sample a representative grid.
#[test]
fn round_trip_within_one_unit() {
    let mut max_err: i32 = 0;
    for r in (0..=255u8).step_by(17) {
        for g in (0..=255u8).step_by(17) {
            for b in (0..=255u8).step_by(17) {
                let (l, a, bb) = srgb_to_oklab(r, g, b);
                let (r2, g2, b2) = oklab_to_srgb(l, a, bb);
                let err = ((r as i32 - r2 as i32).abs())
                    .max((g as i32 - g2 as i32).abs())
                    .max((b as i32 - b2 as i32).abs());
                if err > max_err {
                    max_err = err;
                }
            }
        }
    }
    // The ±1 floor comes from f32 → u8 rounding in linear_to_srgb.
    // Anything > 1 would indicate a math bug.
    assert!(
        max_err <= 1,
        "OKLab round-trip max channel error = {max_err}, expected ≤ 1"
    );
}

/// Polar midpoint of red→green stays saturated (no muddy brown).
///
/// This is the canonical hue-crossing test. Polar interpolation rotates
/// hue through the chroma ring rather than the desaturated RGB cube
/// center, so the midpoint stays saturated.
#[test]
fn red_to_green_midpoint_is_not_muddy() {
    let out = gradient_from_stops_oklab(&[(255, 0, 0), (0, 255, 0)], 3);
    let (mr, mg, mb) = out[1];
    // Saturation proxy: max channel - min channel. Muddy colors have low
    // saturation (max ≈ min). A saturated yellow/orange has max >> min.
    let max_c = mr.max(mg).max(mb) as i32;
    let min_c = mr.min(mg).min(mb) as i32;
    let sat = max_c - min_c;
    // Expect a clearly saturated midpoint. Polar should produce sat ≥ 60
    // (typically ~140+).
    assert!(
        sat >= 60,
        "polar red→green midpoint ({mr},{mg},{mb}) saturation = {sat}, expected ≥ 60"
    );
}

/// Polar midpoint of blue→yellow stays saturated (no gray).
///
/// sRGB-linear midpoint of (0,0,255) → (255,255,0) is (143,143,143) — gray!
/// Cartesian OKLab also takes a shortcut through gray on this opposing-hue
/// gradient. Polar must produce a saturated midpoint.
#[test]
fn blue_to_yellow_midpoint_is_not_gray() {
    let out = gradient_from_stops_oklab(&[(0, 0, 255), (255, 255, 0)], 3);
    let (mr, mg, mb) = out[1];
    let max_c = mr.max(mg).max(mb) as i32;
    let min_c = mr.min(mg).min(mb) as i32;
    let sat = max_c - min_c;
    // Polar midpoint must be clearly saturated (not gray).
    assert!(
        sat >= 50,
        "polar blue→yellow midpoint ({mr},{mg},{mb}) saturation = {sat}, expected ≥ 50"
    );
}

/// Multi-segment gradient (3 stops, 9 steps) hits all 3 stops exactly.
#[test]
fn multi_segment_preserves_anchor_stops() {
    let stops = &[(0, 0, 0), (128, 64, 200), (255, 255, 255)];
    let out = gradient_from_stops_oklab(stops, 9);
    assert_eq!(out.len(), 9);
    // t=0   → stops[0]
    assert_eq!(out[0], (0, 0, 0));
    // t=0.5 → stops[1]
    assert_eq!(out[4], (128, 64, 200));
    // t=1   → stops[2]
    assert_eq!(out[8], (255, 255, 255));
}

/// Red↔cyan midpoint stays saturated.
///
/// Red (255,0,0) and cyan (0,255,255) are roughly opposite on the OKLab
/// chroma ring. Polar rotates through the chroma ring, keeping saturation
/// high. This is the canonical case where polar outperforms the (now
/// removed) Cartesian variant.
#[test]
fn red_to_cyan_midpoint_is_saturated() {
    let pol = gradient_from_stops_oklab(&[(255, 0, 0), (0, 255, 255)], 3);
    // Endpoints preserved.
    assert_eq!(pol[0], (255, 0, 0));
    assert_eq!(pol[2], (0, 255, 255));

    // Saturation proxy: max - min channel.
    let (mr, mg, mb) = pol[1];
    let max_c = mr.max(mg).max(mb) as i32;
    let min_c = mr.min(mg).min(mb) as i32;
    let sat_pol = max_c - min_c;
    // Polar midpoint should be clearly saturated (not gray).
    // Cartesian OKLab on red→cyan typically produces sat ≈ 30-60; polar
    // should produce sat ≥ 80 (typically 150+).
    assert!(
        sat_pol >= 80,
        "polar red→cyan midpoint {:?} saturation = {sat_pol}, expected ≥ 80",
        pol[1]
    );
}

/// When one endpoint is grayscale, polar falls back to Cartesian lerp
/// (chroma magnitude drops linearly to the colored endpoint's value
/// scaled by `t`). The grayscale fallback is the visually correct
/// answer because hue rotation from "no hue" to any hue is meaningless.
#[test]
fn grayscale_endpoint_falls_back_to_cartesian() {
    // gray → red. Gray has OKLab chroma 0; red has chroma ~0.258.
    let out = gradient_from_stops_oklab(&[(128, 128, 128), (255, 0, 0)], 3);
    // Endpoints preserved.
    assert_eq!(out[0], (128, 128, 128));
    assert_eq!(out[2], (255, 0, 0));

    // Midpoint OKLab chroma must equal (c0 + c1) / 2 = c1 / 2 (since
    // c0=0 for gray). This is the Cartesian-fallback contract: linear
    // chroma interpolation between the endpoints' chroma magnitudes.
    let (_, a0, b0) = srgb_to_oklab(128, 128, 128);
    let (_, a1, b1) = srgb_to_oklab(255, 0, 0);
    let c0 = (a0 * a0 + b0 * b0).sqrt();
    let c1 = (a1 * a1 + b1 * b1).sqrt();
    assert!(c0 < 1e-6, "gray endpoint must have ~0 chroma");

    let (mr, mg, mb) = out[1];
    let (_, a_mid, b_mid) = srgb_to_oklab(mr, mg, mb);
    let c_mid = (a_mid * a_mid + b_mid * b_mid).sqrt();

    let expected_mid_chroma = (c0 + c1) / 2.0;
    assert!(
            (c_mid - expected_mid_chroma).abs() < 0.01,
            "grayscale fallback midpoint chroma {c_mid:.4} should equal linear average {expected_mid_chroma:.4} \
             (Cartesian fallback contract)"
        );

    // Sanity: midpoint should be a desaturated red (R > G, R > B), not
    // pure gray and not a hue-rotated saturated color.
    assert!(mr > mg, "R must dominate over G at the gray→red midpoint");
    assert!(mr > mb, "R must dominate over B at the gray→red midpoint");
}

/// When both endpoints share a hue (differ only in lightness/saturation),
/// polar interpolation preserves that hue — the midpoint has the same
/// hue angle as both endpoints (no rotation introduced).
///
/// For two pure sRGB reds (G=B=0), the OKLab hue is identical at any
/// intensity, and the polar midpoint also stays pure red (G=B=0) because
/// the OKLab ray for pure red is collinear with the chroma ring's hue
/// direction.
#[test]
fn same_hue_endpoints_preserve_hue() {
    // Two reds with different lightness.
    let out = gradient_from_stops_oklab(&[(50, 0, 0), (255, 0, 0)], 3);
    // Endpoints preserved.
    assert_eq!(out[0], (50, 0, 0));
    assert_eq!(out[2], (255, 0, 0));

    // Midpoint: pure red stays pure red — polar must not introduce
    // green or blue channels when both endpoints have G=B=0 in sRGB.
    // This is because pure sRGB reds at any intensity are collinear
    // with the OKLab hue direction (a, b scales linearly with L for
    // pure sRGB primaries), so polar stays on the same ray.
    let (mr, mg, mb) = out[1];
    assert_eq!(
        mg, 0,
        "midpoint G must be 0 (pure red preserved — no green introduced)"
    );
    assert_eq!(
        mb, 0,
        "midpoint B must be 0 (pure red preserved — no blue introduced)"
    );
    // Sanity: midpoint R should be roughly between 50 and 255.
    assert!(
        (50..=255).contains(&mr),
        "midpoint R = {mr}, should be in [50, 255]"
    );

    // Verify the hue-preservation property directly: midpoint OKLab
    // hue must equal both endpoints' hue (no rotation).
    let (_, a0, b0) = srgb_to_oklab(50, 0, 0);
    let (_, a1, b1) = srgb_to_oklab(255, 0, 0);
    let (_, a_mid, b_mid) = srgb_to_oklab(mr, mg, mb);
    let h0 = b0.atan2(a0);
    let h1 = b1.atan2(a1);
    let h_mid = b_mid.atan2(a_mid);
    assert!(
        (h_mid - h0).abs() < 1e-4 && (h_mid - h1).abs() < 1e-4,
        "midpoint hue {h_mid:.4} must equal endpoint hues ({h0:.4}, {h1:.4}) — polar preserves hue"
    );
}

/// `polar_chroma_lerp` unit test: t=0 returns start, t=1 returns end
/// (within floating-point precision).
#[test]
fn polar_chroma_lerp_endpoints() {
    let (a, b) = polar_chroma_lerp(0.5, 0.3, -0.4, 0.2, 0.0);
    assert!((a - 0.5).abs() < 1e-5 && (b - 0.3).abs() < 1e-5);

    let (a, b) = polar_chroma_lerp(0.5, 0.3, -0.4, 0.2, 1.0);
    assert!((a - -0.4).abs() < 1e-5 && (b - 0.2).abs() < 1e-5);
}

/// `polar_chroma_lerp` unit test: midpoint chroma magnitude is the
/// average of the endpoint chromas (linear chroma interpolation).
#[test]
fn polar_chroma_lerp_midpoint_chroma_is_average() {
    let a0 = 0.6_f32;
    let b0 = 0.0_f32;
    let a1 = -0.6_f32;
    let b1 = 0.0_f32;
    let c0 = (a0 * a0 + b0 * b0).sqrt();
    let c1 = (a1 * a1 + b1 * b1).sqrt();
    let (am, bm) = polar_chroma_lerp(a0, b0, a1, b1, 0.5);
    let cm = (am * am + bm * bm).sqrt();
    let expected = (c0 + c1) / 2.0;
    assert!(
        (cm - expected).abs() < 1e-5,
        "midpoint chroma {cm} should be average {expected}"
    );
}

/// `polar_chroma_lerp` unit test: grayscale endpoint falls back to
/// Cartesian lerp (chroma magnitude drops linearly to 0).
#[test]
fn polar_chroma_lerp_grayscale_falls_back_to_cartesian() {
    // Start: saturated red (a=0.5, b=0). End: gray (a=0, b=0).
    let (a, b) = polar_chroma_lerp(0.5, 0.0, 0.0, 0.0, 0.5);
    // Cartesian would give a=0.25, b=0. Polar fallback should match.
    assert!((a - 0.25).abs() < 1e-5 && b.abs() < 1e-5);
}

/// `polar_chroma_lerp` on opposing hues produces higher chroma than
/// Cartesian at the midpoint (the polar path's defining property).
#[test]
fn polar_chroma_lerp_opposing_hues_stay_saturated() {
    // Red (a=+0.45) ↔ Cyan (a=-0.45). Cartesian midpoint = (0, 0) = gray.
    // Polar midpoint stays on the chroma ring.
    let (a0, b0) = (0.45_f32, 0.20_f32);
    let (a1, b1) = (-0.45_f32, -0.05_f32);

    // Cartesian midpoint at t=0.5
    let cart_chroma = {
        let ca = a0 + (a1 - a0) * 0.5;
        let cb = b0 + (b1 - b0) * 0.5;
        (ca * ca + cb * cb).sqrt()
    };

    // Polar midpoint at t=0.5
    let (pa, pb) = polar_chroma_lerp(a0, b0, a1, b1, 0.5);
    let pol_chroma = (pa * pa + pb * pb).sqrt();

    assert!(
            pol_chroma > cart_chroma,
            "Polar midpoint chroma {pol_chroma:.4} should exceed Cartesian midpoint chroma {cart_chroma:.4} \
             for opposing hues — polar must stay saturated"
        );
}

/// Peak optimization: precomputed `PolarSegment` produces identical
/// output to the un-precomputed `polar_chroma_lerp` path. This guards
/// against future regressions in the PolarSegment struct.
#[test]
fn polar_segment_matches_polar_chroma_lerp() {
    // Arbitrary non-gray endpoints.
    let (l0, a0, b0) = (0.5_f32, 0.3_f32, 0.2_f32);
    let (l1, a1, b1) = (0.7_f32, -0.2_f32, 0.4_f32);

    let seg = PolarSegment::new(l0, a0, b0, l1, a1, b1);

    for i in 0..=100 {
        let t = i as f32 / 100.0;
        let (pl, pa, pb) = seg.sample(t);
        let expected_l = l0 + (l1 - l0) * t;
        let (expected_a, expected_b) = polar_chroma_lerp(a0, b0, a1, b1, t);

        assert!((pl - expected_l).abs() < 1e-5, "L mismatch at t={t}");
        assert!((pa - expected_a).abs() < 1e-5, "a mismatch at t={t}");
        assert!((pb - expected_b).abs() < 1e-5, "b mismatch at t={t}");
    }
}

/// Peak optimization: `PolarSegment` grayscale fallback matches
/// `polar_chroma_lerp` grayscale fallback (which is Cartesian lerp).
#[test]
fn polar_segment_grayscale_fallback_matches_polar_chroma_lerp() {
    // Start: gray (a=0, b=0). End: red (a=0.5, b=0).
    let (l0, a0, b0) = (0.5_f32, 0.0_f32, 0.0_f32);
    let (l1, a1, b1) = (0.6_f32, 0.5_f32, 0.0_f32);

    let seg = PolarSegment::new(l0, a0, b0, l1, a1, b1);
    assert!(
        seg.is_gray,
        "segment with gray endpoint must be flagged is_gray"
    );

    for i in 0..=100 {
        let t = i as f32 / 100.0;
        let (pl, pa, pb) = seg.sample(t);
        let expected_l = l0 + (l1 - l0) * t;
        let (expected_a, expected_b) = polar_chroma_lerp(a0, b0, a1, b1, t);

        assert!((pl - expected_l).abs() < 1e-5, "L mismatch at t={t}");
        assert!((pa - expected_a).abs() < 1e-5, "a mismatch at t={t}");
        assert!((pb - expected_b).abs() < 1e-5, "b mismatch at t={t}");
    }
}
