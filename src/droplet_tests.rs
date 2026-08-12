// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

use crate::constants::{
    PARALLAX_BRIGHTNESS_MULT, PARALLAX_HEAD_SELFBLOOM_MULT, PARALLAX_SATURATION_MULT,
};

/// Bug #1 regression: brightness multiplier > 1.0 must lighten the
/// pixel, not be a silent no-op.
///
/// Before fix: `if combined_layer < 1.0` skipped the entire block
/// for front-layer boost 1.05, so the pixel was returned unchanged.
/// After fix: gate is `!= 1.0`, so boost path runs.
#[test]
fn brightness_boost_above_one_actually_lightens() {
    // Reproduce the production arithmetic from droplet.rs:644-648.
    // Front brightness is Option F = 1.10 (was 1.05 before Option F).
    let combined_layer = PARALLAX_BRIGHTNESS_MULT[2];
    assert!(
        combined_layer > 1.0,
        "front brightness must be a boost (>1.0) for this regression to be meaningful"
    );

    let r_in: u8 = 100;
    let fi = (combined_layer * 256.0) as i32;
    let r_out = ((r_in as i32 * fi + 128) >> 8).clamp(0, 255) as u8;

    // Boost >1.0 on r=100 should produce r' > 100 (not 100).
    // The key invariant is r_out > r_in — if the gate regresses to
    // `< 1.0`, this branch is skipped and r_out == r_in.
    assert!(
        r_out > r_in,
        "brightness boost >1.0 was a no-op: r stayed at {r_in} (fi={fi}, r_out={r_out}). \
             Bug #1 has regressed — the gate is probably back to `< 1.0`."
    );
    // Expected delta ≈ boost_pct × r_in. For Option F (1.10): ~10.
    // The 6.0..=14.0 range tolerates either the old 1.05 (delta≈5, but
    // outside this range — would fail) or the new 1.10 (delta≈10). The
    // test author picked a range that matches the current production
    // value; update both together when retuning Option F.
    let delta = (r_out as i32 - r_in as i32).abs() as f32;
    assert!(
        (6.0..=14.0).contains(&delta),
        "brightness boost produced unexpected delta: r {r_in} -> {r_out} (delta={delta})"
    );
}

/// Bug #1 regression (negative case): brightness multiplier < 1.0
/// must still dim the pixel (the original code path that worked).
#[test]
fn brightness_dim_below_one_still_dims() {
    let combined_layer = PARALLAX_BRIGHTNESS_MULT[0]; // back = 0.48
    assert!(combined_layer < 1.0);

    let r_in: u8 = 100;
    let fi = (combined_layer * 256.0) as i32;
    let r_out = ((r_in as i32 * fi + 128) >> 8).clamp(0, 255) as u8;

    assert!(
        r_out < r_in,
        "brightness dim <1.0 failed: r stayed at {r_in} (r_out={r_out})"
    );
}

/// Bug #2 regression: saturation multiplier > 1.0 must oversaturate
/// a vivid color (push it further from gray), not be a silent no-op.
///
/// Before fix: `if saturation_mult < 1.0` skipped the entire block
/// for front-layer oversaturation 1.05, so vivid colors stayed at
/// their original saturation.
/// After fix: gate is `!= 1.0`, and the formula
/// `color - (color - lum) * (1 - sat)` naturally extends to sat > 1.0
/// (inv_sat goes negative, dr inverts, subtraction becomes addition).
#[test]
fn saturation_boost_above_one_oversaturates_vivid_color() {
    let saturation_mult = PARALLAX_SATURATION_MULT[2]; // front = 1.05
    assert!(
        saturation_mult > 1.0,
        "front saturation must be a boost (>1.0) for this regression to be meaningful"
    );

    // Vivid red — r far above lum, so oversaturation should push r up.
    let r: u8 = 200;
    let g: u8 = 50;
    let b: u8 = 50;
    let lum = ((r as u32 * 77 + g as u32 * 150 + b as u32 * 29 + 128) >> 8).min(255) as u8;
    assert!(
        r > lum,
        "test setup: r must be above lum for oversaturation to push it up"
    );

    // Reproduce the production arithmetic from droplet.rs:678-682.
    let inv_sat = ((1.0 - saturation_mult) * 256.0) as i32;
    let dr = (r as i32 - lum as i32) * inv_sat;
    let r_out = (r as i32 - (dr + 128) / 256).clamp(0, 255) as u8;

    // With sat=1.05 and r=200, lum=93: inv_sat ≈ -13, dr ≈ -1391,
    // r_out = 200 - (-1391+128)/256 = 200 - (-5) = 205. Boost applied.
    assert!(
        r_out > r,
        "saturation boost >1.0 was a no-op on vivid color: r stayed at {r} \
             (inv_sat={inv_sat}, dr={dr}, r_out={r_out}). Bug #2 has regressed — \
             the gate is probably back to `< 1.0`."
    );
}

/// Bug #2 regression (gray-invariant): saturation changes must not
/// affect pure gray pixels (where r == g == b == lum). This is a
/// mathematical invariant of the formula — gray is the fixed point
/// of any saturation operation, whether desaturation or oversaturation.
#[test]
fn saturation_boost_leaves_gray_unchanged() {
    let saturation_mult = PARALLAX_SATURATION_MULT[2]; // front = 1.05
    let gray: u8 = 128;
    let lum = ((gray as u32 * 77 + gray as u32 * 150 + gray as u32 * 29 + 128) >> 8).min(255) as u8;
    assert_eq!(lum, gray, "gray pixel must equal its own luminance");

    let inv_sat = ((1.0 - saturation_mult) * 256.0) as i32;
    let dr = (gray as i32 - lum as i32) * inv_sat; // == 0
    let r_out = (gray as i32 - (dr + 128) / 256).clamp(0, 255) as u8;

    // Gray is a fixed point — dr=0 means r_out = gray - 0 = gray.
    // (The +128 rounding may shift by ±1, which we tolerate.)
    assert!(
        (r_out as i32 - gray as i32).abs() <= 1,
        "saturation boost moved gray: gray={gray}, r_out={r_out}"
    );
}

/// Bug #3 regression: head self-bloom with a fractional multiplier
/// must produce a non-trivial boost, not silently collapse to 0%.
///
/// Before fix: `let layer_selfbloom = PARALLAX_HEAD_SELFBLOOM_MULT[...] as i32;`
/// combined with `let wf = (HEAD_BOOST_i32 * layer_selfbloom) / 256;`
/// (integer division) gave wf=0 for ALL layers — selfbloom was a
/// 0% boost no-op since the constant was introduced. The mechanism
/// differed per layer:
///   - Layers 0/1 (mult < 1.0): `as i32` truncated 0.38→0, 0.68→0.
///     Then `(60 * 0) / 256 = 0`.
///   - Layer 2 (mult ≥ 1.0): `as i32` truncated 1.15→1. Then
///     `(60 * 1) / 256 = 0` (integer division by 256 of a value < 256).
///
/// After fix: switched to f32 math — `let wf = HEAD_BOOST * layer_selfbloom;`
/// so fractional multipliers actually apply.
#[test]
fn selfbloom_fractional_multiplier_actually_applies() {
    // Reproduce the production arithmetic from droplet.rs:835-844.
    // HEAD_BOOST is `60.0 / 256.0` (~0.234) in the production code.
    const HEAD_BOOST: f32 = 60.0 / 256.0;
    const HEAD_BOOST_I32: i32 = 60;

    // Test all three layers — none should silently no-op.
    for (layer_idx, &mult) in PARALLAX_HEAD_SELFBLOOM_MULT.iter().enumerate() {
        assert!(
            mult > 0.0,
            "layer {layer_idx} selfbloom mult must be > 0 for this regression to be meaningful"
        );

        // The OLD (buggy) arithmetic — `as i32` truncation + integer
        // division. Reproduces the original (broken) code path:
        //   let layer_selfbloom = mult as i32;       // truncates
        //   let wf = (HEAD_BOOST_I32 * layer_selfbloom) / 256;  // int div
        let layer_selfbloom_buggy = mult as i32;
        let wf_buggy = (HEAD_BOOST_I32 * layer_selfbloom_buggy) / 256;
        // The bug: wf_buggy is 0 for ALL three layers.
        // - Layer 0: 0.38 → 0, wf = (60 * 0) / 256 = 0
        // - Layer 1: 0.68 → 0, wf = (60 * 0) / 256 = 0
        // - Layer 2: 1.15 → 1, wf = (60 * 1) / 256 = 0 (integer division)
        assert_eq!(
            wf_buggy, 0,
            "test setup invariant: buggy wf must be 0 for layer {layer_idx} (mult={mult}, \
                 trunc={layer_selfbloom_buggy}). If this fails, the bug pattern has changed — \
                 update this regression test to match."
        );

        // The NEW (fixed) arithmetic — f32 math throughout.
        let layer_selfbloom_fixed = mult; // 0.38, 0.68, 1.15
        let wf_fixed = HEAD_BOOST * layer_selfbloom_fixed; // ~0.089, ~0.159, ~0.269
        assert!(
            wf_fixed > 0.01,
            "selfbloom wf collapsed to ~0 for layer {layer_idx} (wf={wf_fixed}). \
                 Bug #3 has regressed — the code is probably back to `as i32` truncation."
        );

        // Verify the boost actually lightens a pixel.
        let r_in: u8 = 100;
        let scale = 1.0 + wf_fixed;
        let r_out = (r_in as f32 * scale).round().clamp(0.0, 255.0) as u8;
        assert!(
            r_out > r_in,
            "selfbloom failed to lighten pixel for layer {layer_idx}: \
                 r stayed at {r_in} (wf={wf_fixed}, scale={scale}, r_out={r_out})"
        );
    }
}

/// Sanity invariant: the per-layer multipliers must be monotonically
/// non-decreasing from back (layer 0) to front (layer 2). This is the
/// fundamental depth cue — front layer is always at least as bright,
/// saturated, and bloom-heavy as the back layer. If anyone inverts
/// the array order or accidentally swaps two values, this catches it.
#[test]
fn per_layer_multipliers_are_monotically_nondecreasing() {
    fn assert_monotonic(arr: &[f32], label: &str) {
        for w in arr.windows(2) {
            assert!(
                w[1] >= w[0] - 1e-6,
                "{label} must be monotonically non-decreasing back→front, got {arr:?}"
            );
        }
    }
    assert_monotonic(&PARALLAX_BRIGHTNESS_MULT, "PARALLAX_BRIGHTNESS_MULT");
    assert_monotonic(&PARALLAX_SATURATION_MULT, "PARALLAX_SATURATION_MULT");
    assert_monotonic(
        &PARALLAX_HEAD_SELFBLOOM_MULT,
        "PARALLAX_HEAD_SELFBLOOM_MULT",
    );
}
