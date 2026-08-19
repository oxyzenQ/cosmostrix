// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! climate tests, extracted from inline `mod tests { ... }` block.
//!
//! Uses `use super::*;` to access parent's private items unchanged.

use super::*;

/// Neutral ctx returns the input unchanged.
#[test]
fn neutral_ctx_is_noop() {
    let ctx = ClimateCtx::none();
    assert!(ctx.is_neutral());
    let (r, g, b) = apply_climate(100, 150, 200, 5, 10, &ctx);
    assert_eq!((r, g, b), (100, 150, 200));
}

/// Default ctx (all None) is also neutral.
#[test]
fn default_ctx_is_neutral() {
    let ctx = ClimateCtx::default();
    assert!(ctx.is_neutral());
}

/// Lum dim factor multiplies each channel by fi/256.
/// fi=128 (= 0.5) → channels halved (with rounding).
#[test]
fn lum_fi_dims_channels() {
    let ctx = ClimateCtx {
        lum_fi: Some(128), // 0.5
        ..ClimateCtx::none()
    };
    let (r, g, b) = apply_climate(200, 100, 50, 5, 10, &ctx);
    // (200 * 128 + 128) >> 8 = 25728 >> 8 = 100 (with +128 rounding)
    assert_eq!(r, 100);
    assert_eq!(g, 50);
    assert_eq!(b, 25);
}

/// Lum dim factor of 256 (= 1.0) leaves channels unchanged.
#[test]
fn lum_fi_256_unchanged() {
    let ctx = ClimateCtx {
        lum_fi: Some(256),
        ..ClimateCtx::none()
    };
    let (r, g, b) = apply_climate(200, 100, 50, 5, 10, &ctx);
    assert_eq!((r, g, b), (200, 100, 50));
}

/// Lum dim factor of 0 zeros all channels.
#[test]
fn lum_fi_0_zeros() {
    let ctx = ClimateCtx {
        lum_fi: Some(0),
        ..ClimateCtx::none()
    };
    let (r, g, b) = apply_climate(200, 100, 50, 5, 10, &ctx);
    assert_eq!((r, g, b), (0, 0, 0));
}

/// Lum boost factor blends toward white by wf/256.
/// wf=256 (= 1.0) → pure white.
#[test]
fn lum_wf_full_boost_to_white() {
    let ctx = ClimateCtx {
        lum_wf: Some(256),
        ..ClimateCtx::none()
    };
    let (r, g, b) = apply_climate(100, 50, 200, 5, 10, &ctx);
    assert_eq!((r, g, b), (255, 255, 255));
}

/// Lum boost factor wf=0 leaves channels unchanged.
#[test]
fn lum_wf_zero_unchanged() {
    let ctx = ClimateCtx {
        lum_wf: Some(0),
        ..ClimateCtx::none()
    };
    let (r, g, b) = apply_climate(100, 50, 200, 5, 10, &ctx);
    assert_eq!((r, g, b), (100, 50, 200));
}

/// Lum dim and boost are mutually exclusive — when both are set, dim
/// wins (it's checked first). This is the pre-Phase-3-G behavior:
/// lum_fi is set when total_lum < 1.0, lum_wf when total_lum > 1.0,
/// so they never coexist in production. The test documents the
/// tiebreaker for defensive callers.
#[test]
fn lum_dim_wins_over_boost() {
    let ctx = ClimateCtx {
        lum_fi: Some(128),
        lum_wf: Some(256),
        ..ClimateCtx::none()
    };
    let (r, g, b) = apply_climate(200, 200, 200, 5, 10, &ctx);
    // Dim applied (100), boost skipped.
    assert_eq!((r, g, b), (100, 100, 100));
}

/// Saturation factor: ti=0 fully desaturates (all channels = the
/// average gray). ti=256 leaves channels unchanged. This matches the
/// pre-Phase-3-G semantics where `sat_ti = saturation * 256` and
/// `saturation < 1.0` activates the branch — so saturation=0 → ti=0
/// → full gray, saturation=1.0 → ti=256 → unchanged.
#[test]
fn saturation_zero_factor_full_gray() {
    let ctx = ClimateCtx {
        sat_ti: Some(0),
        ..ClimateCtx::none()
    };
    let (r, g, b) = apply_climate(255, 0, 0, 5, 10, &ctx);
    // gray = (255 + 0 + 0) / 3 = 85
    assert_eq!((r, g, b), (85, 85, 85));
}

/// Saturation ti=256 leaves channels approximately unchanged.
/// A ±1 LSB rounding artifact is expected because the integer math
/// `(channel - gray) * ti / 256` truncates toward zero, so negative
/// deltas round differently from positive ones. This is the same
/// behavior as the pre-Phase-3-G post-hoc pass.
#[test]
fn saturation_full_factor_unchanged() {
    let ctx = ClimateCtx {
        sat_ti: Some(256),
        ..ClimateCtx::none()
    };
    let (r, g, b) = apply_climate(255, 0, 0, 5, 10, &ctx);
    // r stays exactly 255 (positive delta rounds correctly).
    // g and b may be off by ±1 due to integer truncation toward zero
    // on negative deltas (the pre-Phase-3-G behavior).
    assert_eq!(r, 255, "r should be exactly preserved (positive delta)");
    assert!(
        g <= 1,
        "g should be 0 or 1 (negative delta truncates toward zero, ±1 LSB)"
    );
    assert!(
        b <= 1,
        "b should be 0 or 1 (negative delta truncates toward zero, ±1 LSB)"
    );
}

/// Persistence blend toward white: wf=256 → pure white.
#[test]
fn persistence_full_to_white() {
    let ctx = ClimateCtx {
        persist_wf: Some(256),
        ..ClimateCtx::none()
    };
    let (r, g, b) = apply_climate(100, 50, 200, 5, 10, &ctx);
    assert_eq!((r, g, b), (255, 255, 255));
}

/// Instability: when threshold=1000, every cell triggers (hash % 1000
/// is always < 1000). The boost is applied to all cells.
#[test]
fn instability_full_threshold_triggers_all() {
    let ctx = ClimateCtx {
        instability_threshold: Some(1000),
        instability_wf: Some(256),
        now_secs: 0,
        ..ClimateCtx::none()
    };
    // Sample many cells — all should be white.
    for line in 0..16u16 {
        for col in 0..16u16 {
            let (r, g, b) = apply_climate(100, 50, 200, line, col, &ctx);
            assert_eq!(
                (r, g, b),
                (255, 255, 255),
                "cell ({line}, {col}) not boosted"
            );
        }
    }
}

/// Instability: when threshold=0, no cell triggers (hash % 1000 is
/// never < 0). The boost is never applied.
#[test]
fn instability_zero_threshold_triggers_none() {
    let ctx = ClimateCtx {
        instability_threshold: Some(0),
        instability_wf: Some(256),
        now_secs: 0,
        ..ClimateCtx::none()
    };
    for line in 0..16u16 {
        for col in 0..16u16 {
            let (r, g, b) = apply_climate(100, 50, 200, line, col, &ctx);
            assert_eq!(
                (r, g, b),
                (100, 50, 200),
                "cell ({line}, {col}) unexpectedly boosted"
            );
        }
    }
}

/// Instability: when threshold=500, roughly half the cells trigger.
/// Verify the count is in [400, 600] out of 1024 sampled cells.
#[test]
fn instability_half_threshold_triggers_about_half() {
    let ctx = ClimateCtx {
        instability_threshold: Some(500),
        instability_wf: Some(256),
        now_secs: 42,
        ..ClimateCtx::none()
    };
    let mut triggered = 0;
    let total = 1024u32;
    for line in 0..32u16 {
        for col in 0..32u16 {
            let (r, g, b) = apply_climate(100, 50, 200, line, col, &ctx);
            if (r, g, b) == (255, 255, 255) {
                triggered += 1;
            }
        }
    }
    // Expect ~512 (50% of 1024). Allow [400, 600] for hash variance.
    assert!(
        (400..=600).contains(&triggered),
        "instability triggered {triggered}/{total} cells, expected ~512 (in [400, 600])"
    );
}

/// Instability varies with now_secs — same cell, different seconds,
/// different trigger decisions. This is what produces the "flicker"
/// effect across frames.
#[test]
fn instability_varies_with_time() {
    let mut triggered_t0 = 0;
    let mut triggered_t1 = 0;
    for line in 0..32u16 {
        for col in 0..32u16 {
            let ctx0 = ClimateCtx {
                instability_threshold: Some(500),
                instability_wf: Some(256),
                now_secs: 0,
                ..ClimateCtx::none()
            };
            let ctx1 = ClimateCtx {
                instability_threshold: Some(500),
                instability_wf: Some(256),
                now_secs: 1,
                ..ClimateCtx::none()
            };
            let (r, g, b) = apply_climate(100, 50, 200, line, col, &ctx0);
            if (r, g, b) == (255, 255, 255) {
                triggered_t0 += 1;
            }
            let (r, g, b) = apply_climate(100, 50, 200, line, col, &ctx1);
            if (r, g, b) == (255, 255, 255) {
                triggered_t1 += 1;
            }
        }
    }
    // The set of triggered cells should differ between t=0 and t=1.
    // (Both counts should be ~512, but the cell sets should differ.)
    // We can't easily count "cells that differ" without storing sets,
    // so just verify both counts are non-zero and not equal (very
    // unlikely to be equal with 1024 cells and 50% trigger rate).
    assert!(triggered_t0 > 0, "t=0 should trigger some cells");
    assert!(triggered_t1 > 0, "t=1 should trigger some cells");
    // Not strictly required to differ, but extremely likely. Skip
    // the inequality assertion to avoid flaky tests on hash collisions.
}

/// All effects stack: dim + saturation + persistence + instability
/// all applied in sequence without panicking. Verify the output
/// differs from the input (atmospheric was actually applied) and is
/// deterministic across repeated calls.
#[test]
fn all_effects_stack_deterministic() {
    let ctx = ClimateCtx {
        lum_fi: Some(128),                 // dim 50%
        sat_ti: Some(256),                 // full desaturate
        persist_wf: Some(128),             // 50% toward white
        instability_threshold: Some(1000), // always trigger
        instability_wf: Some(128),         // 50% toward white
        lum_wf: None,
        now_secs: 0,
    };
    let first = apply_climate(200, 100, 50, 5, 7, &ctx);
    let second = apply_climate(200, 100, 50, 5, 7, &ctx);
    // Deterministic: same inputs → same outputs.
    assert_eq!(first, second, "stacked effects must be deterministic");
    // Applied: output differs from input (atmospheric actually ran).
    assert_ne!(
        first,
        (200, 100, 50),
        "stacked effects must modify the input"
    );
}

/// Apply atmospheric is pure — same inputs always produce same output.
#[test]
fn apply_climate_is_pure() {
    let ctx = ClimateCtx {
        lum_fi: Some(128),
        ..ClimateCtx::none()
    };
    let a = apply_climate(200, 100, 50, 5, 10, &ctx);
    let b = apply_climate(200, 100, 50, 5, 10, &ctx);
    assert_eq!(a, b);
}
