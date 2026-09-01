// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! v80.0.0-beta.1 adaptive density throttle band tests (owner masterclass
//! mandate, 2026-09-01).
//!
//! The contract under test (owner's literal numbers, ceiling = the
//! configured density — e.g. monolith's builtin 0.85):
//!
//! | pressure       | condition    | target density          |
//! | -------------- | ------------ | ----------------------- |
//! | p <= 0.05      | none         | the configured density  |
//! | 0.05 < p < 0.3 | low          | 0.84 -> 0.70            |
//! | 0.3 <= p < 0.6 | medium       | 0.70 -> 0.50            |
//! | 0.6 <= p <= 1  | high (rare)  | 0.50 -> 0.10            |
//!
//! plus: aggressive reads the pressure 0.20 deeper (same band edges),
//! NaN pressure maps to the dead zone, and the configured density is
//! always the ceiling (never amplified, cheap scenes untouched until
//! the deep bands cross them).

use super::*;

/// Effective density = density * scale — the number the HUD `dsty:`
/// line shows and the spawn path uses.
fn effective(pressure: f32, aggressive: bool, density: f32) -> f32 {
    density * compute_spawn_scale(pressure, aggressive, density)
}

// ── dead zone + ceiling ─────────────────────────────────────────────

#[test]
fn dead_zone_zero_and_low_pressure_full_density() {
    for p in [0.0f32, 0.02, 0.049, 0.05] {
        let e = effective(p, false, 0.85);
        assert_eq!(
            e, 0.85,
            "p={p} is at/below the 0.05 dead zone — full monolith density expected"
        );
    }
}

#[test]
fn zero_pressure_aggressive_stays_in_low_band() {
    // aggressive at p=0: the +0.20 shift lands at 0.20 → low-band
    // t=0.6 → target 0.756. This state is transient in production (the
    // self-healer engages aggressive only under sustained HIGH pressure
    // and releases it on recovery) — the assert locks the recovery
    // hysteresis: even with a stale aggressive flag at zero pressure the
    // throttle stays inside the LOW band, never below its 0.70 floor,
    // and never above the configured ceiling.
    let e = effective(0.0, true, 0.85);
    assert!(
        (e - 0.756).abs() < 1e-3,
        "aggressive + zero pressure stays at the low-band midpoint, got {e}"
    );
    assert!(e >= 0.70 - 1e-6, "never below the low-band floor");
    assert!(e <= 0.85 + 1e-6, "never above the configured ceiling");
}

#[test]
fn configured_density_is_never_exceeded() {
    for p in [0.0f32, 0.1, 0.3, 0.6, 1.0] {
        for d in [0.35f32, 0.72, 0.85, 1.0, 1.10] {
            let e = effective(p, false, d);
            assert!(
                e <= d + 1e-6,
                "p={p} d={d}: effective {e} exceeds the configured ceiling"
            );
            let e_agg = effective(p, true, d);
            assert!(
                e_agg <= d + 1e-6,
                "aggressive p={p} d={d}: effective {e_agg} exceeds the ceiling"
            );
        }
    }
}

// ── band edges (the owner's literal numbers) ────────────────────────

#[test]
fn monolith_085_low_band_lands_084_on_first_step() {
    // Owner example: monolith builtin 0.85; "reduce density to 0.84, 0.83".
    // p just above the dead zone (0.051) → t≈0.004 → target ≈ 0.8394.
    let e = effective(0.051, false, 0.85);
    assert!(
        (0.834..=0.84).contains(&e),
        "first low-band step from 0.85 must land at ~0.84, got {e}"
    );
}

#[test]
fn low_band_bottom_at_medium_entry() {
    // p=0.30 (medium entry) → low band bottom 0.70.
    let e = effective(0.30, false, 0.85);
    assert!(
        (e - 0.70).abs() < 1e-4,
        "p=0.30 must land at the 0.70 band boundary, got {e}"
    );
}

#[test]
fn medium_band_bottom_at_high_entry() {
    // p=0.60 (high entry) → 0.50. This is the owner's observed regime:
    // the old linear curve showed ~0.47 here; the new band floor is 0.50.
    let e = effective(0.60, false, 0.85);
    assert!(
        (e - 0.50).abs() < 1e-4,
        "p=0.60 must land at the 0.50 boundary (owner's observed regime), got {e}"
    );
}

#[test]
fn high_band_floor_at_max_pressure() {
    let e = effective(1.0, false, 0.85);
    assert!(
        (e - 0.10).abs() < 1e-4,
        "p=1.0 must reach the 0.10 absolute floor, got {e}"
    );
}

#[test]
fn medium_band_is_monotone_and_gentler_than_v50_linear() {
    // Sweep the medium band: monotone non-increasing, and always >= the
    // v50 linear curve at the same pressure (the owner's "should not
    // extreme throttle" mandate — gentler everywhere below the high band).
    let mut prev = f32::MAX;
    for i in 0..=30 {
        let p = 0.30 + (i as f32) * (0.30 / 30.0);
        let e = effective(p, false, 0.85);
        assert!(
            e <= prev + 1e-6,
            "medium band must be monotone non-increasing (p={p}, e={e})"
        );
        prev = e;
        let v50 = 0.85 * (1.0 - 0.75 * p).clamp(0.25, 1.0);
        assert!(
            e >= v50 - 1e-6,
            "p={p}: banded {e} must be >= v50 linear {v50} (gentler below the high band)"
        );
    }
}

// ── cheap scenes self-harmonize ─────────────────────────────────────

#[test]
fn cheap_scene_untouched_until_high_band_crosses_it() {
    // density 0.30: low/medium band targets (0.84..0.50) all exceed the
    // ceiling → no throttle. The high band crosses 0.30 at
    // t=(0.50-0.30)/0.40=0.5 → p=0.60+0.5*0.40=0.80.
    for p in [0.1f32, 0.3, 0.5, 0.7, 0.79] {
        let e = effective(p, false, 0.30);
        assert_eq!(e, 0.30, "p={p} must not throttle a 0.30-density scene");
    }
    let e = effective(0.81, false, 0.30);
    assert!(
        e < 0.30,
        "p=0.81 (high band below 0.30) must start throttling, got {e}"
    );
}

#[test]
fn tiny_density_never_clamps_inverted_or_divides_by_zero() {
    // Guard: clamp(floor, density) with density < 0.10 must not panic
    // (floor = min(0.10, density)) and must not amplify.
    let e = effective(1.0, false, 0.01);
    assert_eq!(e, 0.01, "density below the band floor stays fixed");
    let e0 = effective(0.5, false, 0.0);
    assert_eq!(e0, 0.0, "zero density must not divide by zero");
}

// ── aggressive reads deeper, never wider than the bands ─────────────

#[test]
fn aggressive_shifts_one_band_deeper() {
    // p=0.5 normal → medium (0.5667); aggressive → reads 0.70 → high band
    // t=0.25 → 0.40. Deeper at the same raw pressure, same band edges.
    let e_normal = effective(0.5, false, 0.85);
    let e_aggr = effective(0.5, true, 0.85);
    assert!((e_normal - 0.56667).abs() < 1e-3, "normal got {e_normal}");
    assert!((e_aggr - 0.40).abs() < 1e-3, "aggressive got {e_aggr}");
    assert!(
        e_aggr < e_normal,
        "aggressive must shed more than normal at the same pressure"
    );
}

#[test]
fn aggressive_at_max_pressure_matches_high_floor() {
    // The +0.20 shift clamps at 1.0 — same absolute floor as normal.
    let e = effective(1.0, true, 0.85);
    assert!((e - 0.10).abs() < 1e-4, "got {e}");
}

// ── NaN guard (CC2-03 pattern) ──────────────────────────────────────

#[test]
fn nan_pressure_maps_to_dead_zone() {
    let scale = compute_spawn_scale(f32::NAN, false, 0.85);
    assert_eq!(
        scale, 1.0,
        "NaN pressure must map to the dead zone (full density)"
    );
    let scale_agg = compute_spawn_scale(f32::NAN, true, 0.85);
    assert!(scale_agg.is_finite(), "NaN + aggressive must stay finite");
}

// ── pressure clamping ───────────────────────────────────────────────

#[test]
fn pressure_above_one_clamps() {
    let e = effective(7.5, false, 0.85);
    assert!(
        (e - 0.10).abs() < 1e-4,
        "p>1 clamps to the high-band floor, got {e}"
    );
}
