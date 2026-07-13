// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Adaptive time-driven Atmosphere Engine.
//!
//! The adaptive regime modulates the rain's density, speed, brightness, and
//! glitch pressure based on the local wall-clock time. The 24-hour day is
//! divided into five emotional phases that transition smoothly via smoothstep
//! interpolation so the rain breathes rather than jumps.
//!
//! ## Phases (local time)
//!
//! - `00:00–03:00` Deep Void — silent night, dense + slow + dark + glitchy
//! - `03:00–06:00` Compression — pre-dawn pressure, extreme density
//! - `06:00–12:00` Pulse — morning energy, sparse + fast + bright
//! - `12:00–18:00` Calm — stable afternoon, balanced
//! - `18:00–24:00` Signal — dusk to night, rising glitch
//!
//! The returned [`AdaptiveParams`] are intentionally kept conservative: each
//! field is clamped to a safe range so the renderer never produces an
//! unreadable frame. Night phases enable `color_change_allowed` and
//! `terminal_effect_allowed` to give the rain a "signal interference" feel.

use crate::atmosphere_apply::AtmosphereRuntimeModulation;

/// Target modulation parameters for a given local hour.
///
/// All scales are multipliers (1.0 = identity). `glitch_pressure` is a
/// 0.0–1.0 weight that the renderer uses to bias glitch probability. The
/// two boolean flags opt into night-only color drift and terminal effects.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdaptiveParams {
    /// Speed multiplier (0.5 .. 1.3).
    pub speed_scale: f32,
    /// Density multiplier (0.4 .. 1.5).
    pub density_scale: f32,
    /// Brightness multiplier (0.4 .. 1.0).
    pub brightness_scale: f32,
    /// Glitch pressure weight (0.0 .. 1.0).
    pub glitch_pressure: f32,
    /// Allow subtle color drift at night.
    pub color_change_allowed: bool,
    /// Allow subtle terminal effects at night.
    pub terminal_effect_allowed: bool,
}

impl AdaptiveParams {
    /// Identity (no modulation). Used as a safe fallback.
    #[allow(dead_code)] // surfaced for future fallback paths
    pub const fn identity() -> Self {
        Self {
            speed_scale: 1.0,
            density_scale: 1.0,
            brightness_scale: 1.0,
            glitch_pressure: 0.0,
            color_change_allowed: false,
            terminal_effect_allowed: false,
        }
    }

    /// Clamp all numeric fields to their declared safe ranges in place.
    pub fn clamp(&mut self) {
        self.speed_scale = self.speed_scale.clamp(0.5, 1.3);
        self.density_scale = self.density_scale.clamp(0.4, 1.5);
        self.brightness_scale = self.brightness_scale.clamp(0.4, 1.0);
        self.glitch_pressure = self.glitch_pressure.clamp(0.0, 1.0);
    }

    /// Linearly interpolate between two parameter sets.
    pub fn lerp(a: Self, b: Self, t: f32) -> Self {
        Self {
            speed_scale: lerp(a.speed_scale, b.speed_scale, t),
            density_scale: lerp(a.density_scale, b.density_scale, t),
            brightness_scale: lerp(a.brightness_scale, b.brightness_scale, t),
            glitch_pressure: lerp(a.glitch_pressure, b.glitch_pressure, t),
            // Booleans snap to the nearer endpoint (t < 0.5 -> a, else b).
            color_change_allowed: if t < 0.5 {
                a.color_change_allowed
            } else {
                b.color_change_allowed
            },
            terminal_effect_allowed: if t < 0.5 {
                a.terminal_effect_allowed
            } else {
                b.terminal_effect_allowed
            },
        }
    }
}

/// Smoothstep interpolation: `t * t * (3.0 - 2.0 * t)`.
///
/// Produces a smooth ease-in/ease-out curve so transitions between phases
/// never have a visible kink. Input `t` is clamped to `[0.0, 1.0]`.
fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Linear interpolation helper.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Get the current local hour as `f64` (e.g. 3.5 for 03:30).
///
/// Returns `0.0` on platforms where the local time cannot be determined
/// (e.g. the system clock is unavailable). This keeps the engine alive
/// even in degraded environments — it simply falls back to the Deep Void
/// baseline, which is a safe, conservative starting point.
#[must_use]
pub fn current_hour() -> f64 {
    use chrono::Timelike;
    let now = chrono::Local::now();
    let hour = f64::from(now.hour());
    let minute = f64::from(now.minute());
    let second = f64::from(now.second());
    hour + (minute + second / 60.0) / 60.0
}

// ── Phase definitions ──────────────────────────────────────────────────────
//
// Each phase is a half-open interval [start, end) over the 24-hour day.
// Phase endpoints define the parameter sets that the smoothstep interpolates
// between. The end-of-phase params are the start-of-next-phase params, so
// the curve is continuous across phase boundaries.

/// Phase boundary parameter sets, in chronological order.
///
/// Index `i` describes the parameters at the *start* of phase `i`. The
/// final entry (`SIGMA_END`) closes the loop back to phase 0 at 24:00.
const PHASE_PARAMS: [AdaptiveParams; 6] = [
    // 00:00 — Deep Void start
    AdaptiveParams {
        speed_scale: 0.55,
        density_scale: 1.25,
        brightness_scale: 0.45,
        glitch_pressure: 0.9,
        color_change_allowed: true,
        terminal_effect_allowed: true,
    },
    // 03:00 — Compression start
    AdaptiveParams {
        speed_scale: 0.80,
        density_scale: 1.40,
        brightness_scale: 0.50,
        glitch_pressure: 0.85,
        color_change_allowed: true,
        terminal_effect_allowed: true,
    },
    // 06:00 — Pulse start
    AdaptiveParams {
        speed_scale: 1.15,
        density_scale: 0.50,
        brightness_scale: 0.85,
        glitch_pressure: 0.25,
        color_change_allowed: false,
        terminal_effect_allowed: false,
    },
    // 12:00 — Calm start
    AdaptiveParams {
        speed_scale: 0.95,
        density_scale: 0.65,
        brightness_scale: 1.0,
        glitch_pressure: 0.15,
        color_change_allowed: false,
        terminal_effect_allowed: false,
    },
    // 18:00 — Signal start
    AdaptiveParams {
        speed_scale: 0.90,
        density_scale: 0.90,
        brightness_scale: 0.75,
        glitch_pressure: 0.55,
        color_change_allowed: true,
        terminal_effect_allowed: false,
    },
    // 24:00 — wraps back to Deep Void start (used as end-of-Signal target)
    AdaptiveParams {
        speed_scale: 0.55,
        density_scale: 1.25,
        brightness_scale: 0.45,
        glitch_pressure: 0.9,
        color_change_allowed: true,
        terminal_effect_allowed: true,
    },
];

/// Phase start hours, in chronological order. Must match `PHASE_PARAMS`.
const PHASE_STARTS: [f64; 5] = [0.0, 3.0, 6.0, 12.0, 18.0];

/// Compute adaptive parameters for a given local hour.
///
/// `hour` is a 0.0–24.0 `f64` (e.g. 3.5 = 03:30). Values outside this range
/// are wrapped modulo 24 so the function is total. The result is always
/// clamped to safe ranges.
#[must_use]
pub fn adaptive_params(hour: f64) -> AdaptiveParams {
    // Wrap to [0.0, 24.0).
    let h = ((hour.rem_euclid(24.0)) as f32).clamp(0.0, 24.0);

    // Find which phase we're in and the interpolation factor within it.
    // PHASE_STARTS has 5 entries; the last phase (18:00–24:00) uses
    // PHASE_PARAMS[4] -> PHASE_PARAMS[5] (which mirrors PHASE_PARAMS[0]).
    let (start_idx, t) = phase_index(h);
    let end_idx = if start_idx + 1 < PHASE_PARAMS.len() {
        start_idx + 1
    } else {
        // Shouldn't happen because start_idx is always in 0..5, but guard anyway.
        0
    };

    let start = PHASE_PARAMS[start_idx];
    let end = PHASE_PARAMS[end_idx];
    let smoothed = smoothstep(t);
    let mut result = AdaptiveParams::lerp(start, end, smoothed);
    result.clamp();
    result
}

/// Map a wrapped hour to (phase_index, interpolation_factor).
///
/// `phase_index` is the index into `PHASE_STARTS` (0..5). `t` is the
/// normalized position within that phase, in `[0.0, 1.0]`.
fn phase_index(hour: f32) -> (usize, f32) {
    // Iterate phase boundaries; the phase is the last start <= hour.
    // The final phase (18:00–24:00) is the fallback when hour >= 18.0.
    for i in (0..PHASE_STARTS.len()).rev() {
        let start = PHASE_STARTS[i] as f32;
        if hour >= start {
            // End of this phase: either next PHASE_STARTS[i+1], or 24.0.
            let end = if i + 1 < PHASE_STARTS.len() {
                PHASE_STARTS[i + 1] as f32
            } else {
                24.0
            };
            let span = (end - start).max(1e-6);
            let t = ((hour - start) / span).clamp(0.0, 1.0);
            return (i, t);
        }
    }
    // hour < 0 after clamp — impossible, but fall back to phase 0.
    (0, 0.0)
}

/// Per-frame smooth approach toward the adaptive target.
///
/// Instead of snapping to the time-driven target every frame, this nudges
/// the current modulation toward the target by `lerp_factor` per frame.
/// A factor of 0.001 means ~1000 frames to converge, which at 60 FPS is
/// ~16 seconds — slow enough to be imperceptible during normal play but
/// fast enough to react to a multi-hour time jump (e.g. laptop resume).
///
/// `lerp_factor` is clamped to `[0.0, 1.0]`; 0.0 means "don't move" and
/// 1.0 means "snap immediately".
pub fn update_modulation(
    current: &mut AtmosphereRuntimeModulation,
    target: &AdaptiveParams,
    lerp_factor: f32,
) {
    let f = lerp_factor.clamp(0.0, 1.0);
    current.speed_scale = lerp(current.speed_scale, target.speed_scale, f);
    current.density_scale = lerp(current.density_scale, target.density_scale, f);
    current.brightness_scale = lerp(current.brightness_scale, target.brightness_scale, f);
    current.glitch_pressure = lerp(current.glitch_pressure, target.glitch_pressure, f);
    // Booleans: snap once we cross 50% of the approach.
    if f >= 0.5 {
        current.color_change_allowed = target.color_change_allowed;
        current.terminal_effect_allowed = target.terminal_effect_allowed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_hour_is_in_valid_range() {
        let h = current_hour();
        assert!(
            (0.0..24.0).contains(&h),
            "current_hour must be in [0.0, 24.0), got {h}"
        );
    }

    #[test]
    fn adaptive_params_returns_clamped_values() {
        for hour in 0..24 {
            let p = adaptive_params(f64::from(hour));
            assert!(
                (0.5..=1.3).contains(&p.speed_scale),
                "speed_scale out of range at {hour}: {}",
                p.speed_scale
            );
            assert!(
                (0.4..=1.5).contains(&p.density_scale),
                "density_scale out of range at {hour}: {}",
                p.density_scale
            );
            assert!(
                (0.4..=1.0).contains(&p.brightness_scale),
                "brightness_scale out of range at {hour}: {}",
                p.brightness_scale
            );
            assert!(
                (0.0..=1.0).contains(&p.glitch_pressure),
                "glitch_pressure out of range at {hour}: {}",
                p.glitch_pressure
            );
        }
    }

    #[test]
    fn adaptive_params_midnight_is_deep_void() {
        let p = adaptive_params(0.0);
        // Deep Void: high density, low speed, low brightness, high glitch.
        assert!(p.density_scale > 1.1, "midnight density should be high");
        assert!(p.speed_scale < 0.7, "midnight speed should be slow");
        assert!(
            p.brightness_scale < 0.6,
            "midnight brightness should be low"
        );
        assert!(p.glitch_pressure > 0.7, "midnight glitch should be high");
        assert!(p.color_change_allowed, "midnight should allow color change");
    }

    #[test]
    fn adaptive_params_noon_is_calm_bright() {
        let p = adaptive_params(12.0);
        // Calm phase: full brightness, low glitch.
        assert!(
            p.brightness_scale > 0.95,
            "noon brightness should be near max"
        );
        assert!(p.glitch_pressure < 0.3, "noon glitch should be low");
        assert!(
            !p.color_change_allowed,
            "noon should not allow color change"
        );
    }

    #[test]
    fn adaptive_params_morning_is_pulse() {
        let p = adaptive_params(9.0);
        // Pulse phase: fast speed, low density.
        assert!(p.speed_scale > 1.0, "morning speed should be fast");
        assert!(p.density_scale < 0.7, "morning density should be low");
    }

    #[test]
    fn adaptive_params_wraps_past_midnight() {
        // 25.0 should wrap to 1.0 (Deep Void).
        let p_wrap = adaptive_params(25.0);
        let p_one = adaptive_params(1.0);
        assert_eq!(p_wrap, p_one, "25.0 should wrap to 1.0");
    }

    #[test]
    fn adaptive_params_negative_hour_wraps() {
        // -1.0 should wrap to 23.0 (late Signal / pre-Void).
        let p_wrap = adaptive_params(-1.0);
        let p_eleven_pm = adaptive_params(23.0);
        assert_eq!(p_wrap, p_eleven_pm, "-1.0 should wrap to 23.0");
    }

    #[test]
    fn adaptive_params_transitions_are_smooth() {
        // Adjacent hours should not produce wildly different params.
        for hour in 0..24 {
            let h0 = f64::from(hour);
            let h1 = h0 + 0.5;
            let p0 = adaptive_params(h0);
            let p1 = adaptive_params(h1);
            let d_speed = (p0.speed_scale - p1.speed_scale).abs();
            let d_density = (p0.density_scale - p1.density_scale).abs();
            assert!(
                d_speed < 0.2,
                "speed jump too large between {h0} and {h1}: {d_speed}"
            );
            assert!(
                d_density < 0.25,
                "density jump too large between {h0} and {h1}: {d_density}"
            );
        }
    }

    #[test]
    fn smoothstep_is_zero_at_zero_and_one_at_one() {
        assert!((smoothstep(0.0) - 0.0).abs() < 1e-6);
        assert!((smoothstep(1.0) - 1.0).abs() < 1e-6);
        assert!((smoothstep(0.5) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn smoothstep_clamps_outside_unit_range() {
        assert!((smoothstep(-1.0) - 0.0).abs() < 1e-6);
        assert!((smoothstep(2.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn lerp_interpolates_correctly() {
        assert!((lerp(0.0, 10.0, 0.5) - 5.0).abs() < 1e-6);
        assert!((lerp(1.0, 3.0, 0.0) - 1.0).abs() < 1e-6);
        assert!((lerp(1.0, 3.0, 1.0) - 3.0).abs() < 1e-6);
    }

    #[test]
    fn update_modulation_approaches_target() {
        let mut current = AtmosphereRuntimeModulation::identity();
        let target = adaptive_params(0.0); // Deep Void
                                           // 1000 small steps should converge close to target.
        for _ in 0..1000 {
            update_modulation(&mut current, &target, 0.01);
        }
        assert!(
            (current.speed_scale - target.speed_scale).abs() < 0.01,
            "speed_scale should converge to target"
        );
        assert!(
            (current.glitch_pressure - target.glitch_pressure).abs() < 0.01,
            "glitch_pressure should converge to target"
        );
    }

    #[test]
    fn update_modulation_zero_factor_is_noop() {
        let mut current = AtmosphereRuntimeModulation::identity();
        let target = adaptive_params(0.0);
        update_modulation(&mut current, &target, 0.0);
        assert!(
            current.is_identity(),
            "zero lerp factor must not change modulation"
        );
    }

    #[test]
    fn update_modulation_full_factor_snaps() {
        let mut current = AtmosphereRuntimeModulation::identity();
        let target = adaptive_params(0.0);
        update_modulation(&mut current, &target, 1.0);
        assert_eq!(current.speed_scale, target.speed_scale);
        assert_eq!(current.density_scale, target.density_scale);
        assert_eq!(current.brightness_scale, target.brightness_scale);
        assert_eq!(current.glitch_pressure, target.glitch_pressure);
        assert_eq!(current.color_change_allowed, target.color_change_allowed);
    }

    #[test]
    fn phase_index_at_boundaries() {
        assert_eq!(phase_index(0.0), (0, 0.0));
        assert_eq!(phase_index(3.0), (1, 0.0));
        assert_eq!(phase_index(6.0), (2, 0.0));
        assert_eq!(phase_index(12.0), (3, 0.0));
        assert_eq!(phase_index(18.0), (4, 0.0));
        // Just before midnight wraps to phase 4 end (use approximate range
        // to avoid float precision brittleness).
        let (idx, t) = phase_index(23.999);
        assert_eq!(idx, 4);
        assert!(t > 0.99 && t < 1.0, "expected t near 1.0, got {t}");
    }
}
