// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! v80.0.0-beta.1 adaptive density throttle (owner masterclass mandate, 2026-09-01).
//!
//! Banded spawn-scale curve with the user's configured density as the
//! CEILING — extracted from `central_control_rains/mod.rs` to keep that
//! file under the 800-LOC cap. Re-exported via
//! `pub(crate) use density_throttle::*;` (same pattern as atmosphere /
//! events / parallax), so `crate::central_control_rains::*` and
//! `crate::constants::*` consumers see no change.

// v80.0.0-beta.1 adaptive density throttle bands (owner masterclass mandate,
// 2026-09-01). Replaces the v50 linear curve `1 - factor*p`
// (PERF_PRESSURE_SPAWN_FACTOR 0.75/0.9 + scale floors 0.25/0.10), which
// cut the density nearly in half at moderate pressure (density 0.85 -> ~0.47
// at p ~0.6 on the owner's terminal). The banded design:
//
//   pressure        condition   target density (absolute space)
//   -------------   ---------   --------------------------------------
//   p <= 0.05       none        the configured density (dead zone)
//   0.05 < p < 0.30 low         0.84 -> 0.70
//   0.30 <= p < 0.60 medium     0.70 -> 0.50
//   0.60 <= p <= 1.0 high (rare) 0.50 -> 0.10
//
// The target is clamped to `[0.10, configured density]` — the user's
// density (CLI `-d`, config `density`, or the scene's builtin, e.g.
// monolith's 0.85) is the CEILING: the throttle only ever reduces below
// it. Cheap scenes (density below a band edge) are naturally untouched
// until the deep bands cross them — a 0.35-density scene only throttles
// in the high band, which is the self-harmonizing property of absolute
// band edges.

/// Pressure below which no density throttle applies (dead zone).
///
/// Light occasional overshoot decays fast (PERF_PRESSURE_DECAY 0.02/frame)
/// and does not need load shedding — the configured density renders at
/// full value. This also guarantees the HUD `dsty:` line shows the exact
/// user density at idle, matching the "power-dragon off = fixed density"
/// contract on the low end.
pub(crate) const DENSITY_THROTTLE_P_LOW: f32 = 0.05;

/// Pressure entering the medium band. Matches
/// `PERF_PRESSURE_CLASS_MEDIUM` (0.30) so the density band and the
/// post-run report classify "medium" at the same point.
pub(crate) const DENSITY_THROTTLE_P_MED: f32 = 0.30;

/// Pressure entering the high band. Sustained 60%+ frame overshoot is
/// rare on a healthy host (the owner's observed ~0.6 case was a slow
/// terminal write path) — only there may the throttle go below 0.50.
pub(crate) const DENSITY_THROTTLE_P_HIGH: f32 = 0.60;

/// Top of the low band: the first throttle step from the ceiling
/// (monolith 0.85 -> 0.84 -> 0.83 ... — owner's described stepping).
pub(crate) const DENSITY_THROTTLE_LOW_TOP: f32 = 0.84;

/// Bottom of the low band / top of the medium band.
pub(crate) const DENSITY_THROTTLE_LOW_BOTTOM: f32 = 0.70;

/// Bottom of the medium band / top of the high band.
pub(crate) const DENSITY_THROTTLE_MED_BOTTOM: f32 = 0.50;

/// Absolute floor of the high band (owner: "high condition but rare can
/// reach 0.50-0.10").
pub(crate) const DENSITY_THROTTLE_HIGH_BOTTOM: f32 = 0.10;

/// Aggressive-mode pressure shift. When the self-healer flags sustained
/// high CPU, the SAME band edges apply but the pressure is read 0.20
/// deeper — shedding more at the same raw pressure without a second
/// curve or a second constant set.
pub(crate) const DENSITY_THROTTLE_AGGRESSIVE_SHIFT: f32 = 0.20;

/// v50.0.0-beta.6 Option D + v80.0.0-beta.1 banded masterclass: shared
/// spawn-scale computation.
///
/// Single source of truth for the spawn-throttle formula. Called from:
/// - `cloud/rain.rs::rain_at()` — the actual render-path throttle
/// - `interactive/hud/mod.rs::update_metrics()` — the `dsty:` HUD display
///
/// This eliminates formula drift: if a constant changes, both the render
/// path and the HUD update automatically.
///
/// `pressure` is the effective pressure (0.0–1.0, clamped internally; NaN
/// maps to 0.0). `aggressive` selects the deeper-read variant used when
/// the self-healer has detected sustained high CPU. `density` is the
/// user's configured density (CLI `-d` > config `density` > scene builtin
/// — e.g. monolith 0.85); it is the throttle CEILING.
///
/// Returns the spawn-scale multiplier in `[min(0.10, density)/density, 1.0]`
/// such that `density * scale` lands on the banded target — never above
/// the configured density, never below the 0.10 absolute floor (or below
/// the configured density itself for cheap scenes).
#[must_use]
pub(crate) fn compute_spawn_scale(pressure: f32, aggressive: bool, density: f32) -> f32 {
    // NaN guard (CC2-03 pattern): f32::clamp propagates NaN — a future
    // sampler feeding NaN would poison the spawn path. Map to the dead
    // zone (full density) instead.
    let raw = if pressure.is_nan() { 0.0 } else { pressure };
    let p = if aggressive {
        (raw + DENSITY_THROTTLE_AGGRESSIVE_SHIFT).clamp(0.0, 1.0)
    } else {
        raw.clamp(0.0, 1.0)
    };
    // Band target in ABSOLUTE density space (the band edges are the
    // owner's literal numbers; lerp inside each band).
    let target = if p <= DENSITY_THROTTLE_P_LOW {
        // Dead zone: full configured density (scale 1.0).
        density
    } else if p < DENSITY_THROTTLE_P_MED {
        // Low band: 0.84 -> 0.70.
        let t = (p - DENSITY_THROTTLE_P_LOW) / (DENSITY_THROTTLE_P_MED - DENSITY_THROTTLE_P_LOW);
        DENSITY_THROTTLE_LOW_TOP + (DENSITY_THROTTLE_LOW_BOTTOM - DENSITY_THROTTLE_LOW_TOP) * t
    } else if p < DENSITY_THROTTLE_P_HIGH {
        // Medium band: 0.70 -> 0.50.
        let t = (p - DENSITY_THROTTLE_P_MED) / (DENSITY_THROTTLE_P_HIGH - DENSITY_THROTTLE_P_MED);
        DENSITY_THROTTLE_LOW_BOTTOM
            + (DENSITY_THROTTLE_MED_BOTTOM - DENSITY_THROTTLE_LOW_BOTTOM) * t
    } else {
        // High band (rare): 0.50 -> 0.10.
        let t = ((p - DENSITY_THROTTLE_P_HIGH) / (1.0 - DENSITY_THROTTLE_P_HIGH)).clamp(0.0, 1.0);
        DENSITY_THROTTLE_MED_BOTTOM
            + (DENSITY_THROTTLE_HIGH_BOTTOM - DENSITY_THROTTLE_MED_BOTTOM) * t
    };
    // Ceiling/floor: never above the configured density, never below the
    // absolute floor (or the configured density itself — the floor is
    // min(0.10, density), which also keeps clamp's min <= max invariant
    // for densities below 0.10).
    let d = density.max(1e-6);
    let floor = DENSITY_THROTTLE_HIGH_BOTTOM.min(d);
    let clamped = target.clamp(floor, d);
    clamped / d
}
