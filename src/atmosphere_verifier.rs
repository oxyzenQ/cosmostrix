// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Atmosphere verifier for Cosmostrix v4.0.0 Phase 3.
//!
//! The verifier layer ensures that atmosphere parameters and derived runtime
//! modulations are bounded before they reach the renderer. It provides a
//! deterministic, allocation-light safety gate that rejects or clamps unsafe
//! values.
//!
//! ## Phase 3 Scope
//!
//! - `AtmosphereBounds`: defines safe ranges for all modulation parameters.
//! - `AtmosphereSafetyBudget`: per-application safety constraints.
//! - `VerificationResult`: pass/reject outcome with clamped values.
//! - `verify_application()`: pure function that verifies an AtmosphereApplication.
//!
//! The verifier is intentionally conservative: color modification is forbidden
//! by default, terminal behavior is never affected, and transition pressure is
//! bounded.

// Phase 3: Module-level dead_code allow is required because verifier types are
// pub(crate) API contracts consumed in tests and future integration points —
// not yet wired into the hot render path.
#![allow(dead_code)]

// ── Atmosphere Bounds ─────────────────────────────────────────────────────

/// Safe ranges for atmosphere modulation parameters.
///
/// These bounds define the maximum safe deviation from identity (v3.9.0)
/// behavior. The verifier enforces these ranges before any application reaches
/// the renderer.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AtmosphereBounds {
    /// Minimum speed scale factor.
    pub speed_min: f32,
    /// Maximum speed scale factor.
    pub speed_max: f32,
    /// Minimum density scale factor.
    pub density_min: f32,
    /// Maximum density scale factor.
    pub density_max: f32,
    /// Minimum brightness scale factor.
    pub brightness_min: f32,
    /// Maximum brightness scale factor.
    pub brightness_max: f32,
    /// Maximum allowed glitch pressure (0.0 = no glitch, 1.0 = max).
    pub glitch_pressure_max: f32,
}

impl AtmosphereBounds {
    /// Conservative default bounds that keep atmosphere as a subtle effect.
    pub(crate) const fn conservative() -> Self {
        Self {
            speed_min: 0.5,
            speed_max: 2.0,
            density_min: 0.5,
            density_max: 1.5,
            brightness_min: 0.9,
            brightness_max: 1.1,
            glitch_pressure_max: 0.5,
        }
    }
}

impl Default for AtmosphereBounds {
    fn default() -> Self {
        Self::conservative()
    }
}

// ── Verification Result ────────────────────────────────────────────────────

/// Outcome of verifying an atmosphere application.
#[derive(Debug, Clone, Copy)]
pub(crate) struct VerificationResult {
    /// Whether the application passed verification without clamping.
    pub passed: bool,
    /// Whether any values were clamped during verification.
    pub clamped: bool,
}

impl VerificationResult {
    /// Full pass: all values within bounds, no clamping needed.
    pub(crate) const fn pass() -> Self {
        Self {
            passed: true,
            clamped: false,
        }
    }

    /// Values were out of bounds but were clamped to safe ranges.
    pub(crate) const fn clamped_pass() -> Self {
        Self {
            passed: true,
            clamped: true,
        }
    }
}

// ── Atmosphere Application ─────────────────────────────────────────────────

/// Verified, bounded modulation parameters ready for renderer consumption.
///
/// This struct represents the output of the verifier. All values are guaranteed
/// to be within safe bounds. For Calm regime, all values are identity/no-op.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AtmosphereApplication {
    /// Speed scale factor (1.0 = identity).
    pub speed_scale: f32,
    /// Density scale factor (1.0 = identity).
    pub density_scale: f32,
    /// Brightness scale factor (1.0 = identity).
    pub brightness_scale: f32,
    /// Glitch pressure (0.0 = default/no change).
    pub glitch_pressure: f32,
    /// Whether any color modification is requested. Always false by default.
    pub color_change: bool,
}

impl AtmosphereApplication {
    /// Identity application: no modulation. This is the Calm default.
    pub(crate) const fn identity() -> Self {
        Self {
            speed_scale: 1.0,
            density_scale: 1.0,
            brightness_scale: 1.0,
            glitch_pressure: 0.0,
            color_change: false,
        }
    }

    /// Whether this application is a visual no-op (identity).
    pub(crate) fn is_identity(&self) -> bool {
        self.speed_scale == 1.0
            && self.density_scale == 1.0
            && self.brightness_scale == 1.0
            && self.glitch_pressure == 0.0
            && !self.color_change
    }
}

// ── Verify Function ────────────────────────────────────────────────────────

/// Verify and clamp an atmosphere application against the given bounds.
///
/// This is a pure deterministic function. It returns the verification result
/// (pass/clamped) and clamps all values in place on the application.
///
/// Color modification is always rejected (set to false) unless explicitly
/// allowed by a future opt-in mechanism that does not exist yet.
pub(crate) fn verify_application(
    app: &mut AtmosphereApplication,
    bounds: &AtmosphereBounds,
) -> VerificationResult {
    let mut was_clamped = false;

    if app.speed_scale < bounds.speed_min {
        app.speed_scale = bounds.speed_min;
        was_clamped = true;
    } else if app.speed_scale > bounds.speed_max {
        app.speed_scale = bounds.speed_max;
        was_clamped = true;
    }

    if app.density_scale < bounds.density_min {
        app.density_scale = bounds.density_min;
        was_clamped = true;
    } else if app.density_scale > bounds.density_max {
        app.density_scale = bounds.density_max;
        was_clamped = true;
    }

    if app.brightness_scale < bounds.brightness_min {
        app.brightness_scale = bounds.brightness_min;
        was_clamped = true;
    } else if app.brightness_scale > bounds.brightness_max {
        app.brightness_scale = bounds.brightness_max;
        was_clamped = true;
    }

    if app.glitch_pressure < 0.0 {
        app.glitch_pressure = 0.0;
        was_clamped = true;
    } else if app.glitch_pressure > bounds.glitch_pressure_max {
        app.glitch_pressure = bounds.glitch_pressure_max;
        was_clamped = true;
    }

    // Color modification is always forbidden by default.
    if app.color_change {
        app.color_change = false;
        was_clamped = true;
    }

    if was_clamped {
        VerificationResult::clamped_pass()
    } else {
        VerificationResult::pass()
    }
}

// ── Build Application from Regime Params ──────────────────────────────────

/// Convert regime params into an atmosphere application.
///
/// Maps the bounded regime parameter multipliers into the application format
/// used by the verifier. This is a deterministic pure function.
///
/// The color_change field is always set to false (color drift is opt-in only,
/// controlled by auto_color_drift, never by atmosphere).
pub(crate) fn application_from_regime_params(
    speed_mult: f32,
    density_mult: f32,
    glitch_mult: f32,
    brightness_bias: f32,
) -> AtmosphereApplication {
    AtmosphereApplication {
        // Speed multiplier maps directly to speed_scale.
        speed_scale: speed_mult,
        // Density multiplier maps directly to density_scale.
        density_scale: density_mult,
        // Brightness bias (-0.1 .. +0.1) maps to a brightness scale
        // centered at 1.0: brightness_scale = 1.0 + bias.
        brightness_scale: 1.0 + brightness_bias,
        // Glitch multiplier > 1.0 means increased glitch pressure.
        // Map to 0.0..1.0 range: pressure = max(0, (mult - 1.0) / 1.0).
        glitch_pressure: if glitch_mult > 1.0 {
            ((glitch_mult - 1.0) / 1.0).clamp(0.0, 1.0)
        } else {
            0.0
        },
        // Color change is always forbidden by default.
        color_change: false,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── AtmosphereBounds ──

    #[test]
    fn conservative_bounds_are_reasonable() {
        let bounds = AtmosphereBounds::conservative();
        assert!(bounds.speed_min < bounds.speed_max);
        assert!(bounds.density_min < bounds.density_max);
        assert!(bounds.brightness_min < bounds.brightness_max);
        assert!(bounds.glitch_pressure_max >= 0.0);
        assert!(bounds.speed_min <= 1.0);
        assert!(bounds.speed_max >= 1.0);
    }

    #[test]
    fn default_bounds_match_conservative() {
        let default = AtmosphereBounds::default();
        let conservative = AtmosphereBounds::conservative();
        assert_eq!(default.speed_min, conservative.speed_min);
        assert_eq!(default.speed_max, conservative.speed_max);
        assert_eq!(default.density_min, conservative.density_min);
        assert_eq!(default.density_max, conservative.density_max);
    }

    // ── AtmosphereApplication identity ──

    #[test]
    fn identity_application_is_no_op() {
        let app = AtmosphereApplication::identity();
        assert!(app.is_identity());
        assert_eq!(app.speed_scale, 1.0);
        assert_eq!(app.density_scale, 1.0);
        assert_eq!(app.brightness_scale, 1.0);
        assert_eq!(app.glitch_pressure, 0.0);
        assert!(!app.color_change);
    }

    #[test]
    fn identity_application_passes_verification() {
        let mut app = AtmosphereApplication::identity();
        let bounds = AtmosphereBounds::conservative();
        let result = verify_application(&mut app, &bounds);
        assert!(result.passed);
        assert!(!result.clamped);
        assert!(app.is_identity());
    }

    // ── Verification: accept ──

    #[test]
    fn verifier_accepts_values_within_bounds() {
        let mut app = AtmosphereApplication {
            speed_scale: 1.2,
            density_scale: 1.1,
            brightness_scale: 1.05,
            glitch_pressure: 0.3,
            color_change: false,
        };
        let bounds = AtmosphereBounds::conservative();
        let result = verify_application(&mut app, &bounds);
        assert!(result.passed);
        assert!(!result.clamped);
    }

    // ── Verification: clamp speed ──

    #[test]
    fn verifier_clamps_excessive_speed() {
        let mut app = AtmosphereApplication {
            speed_scale: 5.0,
            density_scale: 1.0,
            brightness_scale: 1.0,
            glitch_pressure: 0.0,
            color_change: false,
        };
        let bounds = AtmosphereBounds::conservative();
        let result = verify_application(&mut app, &bounds);
        assert!(result.passed);
        assert!(result.clamped);
        assert_eq!(app.speed_scale, bounds.speed_max);
    }

    #[test]
    fn verifier_clamps_negative_speed() {
        let mut app = AtmosphereApplication {
            speed_scale: -1.0,
            density_scale: 1.0,
            brightness_scale: 1.0,
            glitch_pressure: 0.0,
            color_change: false,
        };
        let bounds = AtmosphereBounds::conservative();
        let result = verify_application(&mut app, &bounds);
        assert!(result.passed);
        assert!(result.clamped);
        assert_eq!(app.speed_scale, bounds.speed_min);
    }

    // ── Verification: clamp density ──

    #[test]
    fn verifier_clamps_excessive_density() {
        let mut app = AtmosphereApplication {
            speed_scale: 1.0,
            density_scale: 3.0,
            brightness_scale: 1.0,
            glitch_pressure: 0.0,
            color_change: false,
        };
        let bounds = AtmosphereBounds::conservative();
        let result = verify_application(&mut app, &bounds);
        assert!(result.passed);
        assert!(result.clamped);
        assert_eq!(app.density_scale, bounds.density_max);
    }

    // ── Verification: clamp brightness ──

    #[test]
    fn verifier_clamps_excessive_brightness() {
        let mut app = AtmosphereApplication {
            speed_scale: 1.0,
            density_scale: 1.0,
            brightness_scale: 2.0,
            glitch_pressure: 0.0,
            color_change: false,
        };
        let bounds = AtmosphereBounds::conservative();
        let result = verify_application(&mut app, &bounds);
        assert!(result.passed);
        assert!(result.clamped);
        assert_eq!(app.brightness_scale, bounds.brightness_max);
    }

    // ── Verification: clamp glitch ──

    #[test]
    fn verifier_clamps_excessive_glitch() {
        let mut app = AtmosphereApplication {
            speed_scale: 1.0,
            density_scale: 1.0,
            brightness_scale: 1.0,
            glitch_pressure: 1.5,
            color_change: false,
        };
        let bounds = AtmosphereBounds::conservative();
        let result = verify_application(&mut app, &bounds);
        assert!(result.passed);
        assert!(result.clamped);
        assert_eq!(app.glitch_pressure, bounds.glitch_pressure_max);
    }

    #[test]
    fn verifier_clamps_negative_glitch() {
        let mut app = AtmosphereApplication {
            speed_scale: 1.0,
            density_scale: 1.0,
            brightness_scale: 1.0,
            glitch_pressure: -0.5,
            color_change: false,
        };
        let bounds = AtmosphereBounds::conservative();
        let result = verify_application(&mut app, &bounds);
        assert!(result.passed);
        assert!(result.clamped);
        assert_eq!(app.glitch_pressure, 0.0);
    }

    // ── Verification: reject color change ──

    #[test]
    fn verifier_rejects_color_change() {
        let mut app = AtmosphereApplication {
            speed_scale: 1.0,
            density_scale: 1.0,
            brightness_scale: 1.0,
            glitch_pressure: 0.0,
            color_change: true,
        };
        let bounds = AtmosphereBounds::conservative();
        let result = verify_application(&mut app, &bounds);
        assert!(result.passed);
        assert!(result.clamped);
        assert!(!app.color_change);
    }

    // ── Verification: multi-violation ──

    #[test]
    fn verifier_clamps_multiple_violations() {
        let mut app = AtmosphereApplication {
            speed_scale: 10.0,
            density_scale: 0.0,
            brightness_scale: 5.0,
            glitch_pressure: 3.0,
            color_change: true,
        };
        let bounds = AtmosphereBounds::conservative();
        let result = verify_application(&mut app, &bounds);
        assert!(result.passed);
        assert!(result.clamped);
        assert!(!app.color_change);
        assert_eq!(app.speed_scale, bounds.speed_max);
        assert_eq!(app.density_scale, bounds.density_min);
        assert_eq!(app.brightness_scale, bounds.brightness_max);
        assert_eq!(app.glitch_pressure, bounds.glitch_pressure_max);
    }

    // ── Verifier determinism ──

    #[test]
    fn verifier_is_deterministic() {
        for _ in 0..100 {
            let mut app_a = AtmosphereApplication {
                speed_scale: 1.7,
                density_scale: 0.6,
                brightness_scale: 1.08,
                glitch_pressure: 0.4,
                color_change: false,
            };
            let mut app_b = AtmosphereApplication {
                speed_scale: 1.7,
                density_scale: 0.6,
                brightness_scale: 1.08,
                glitch_pressure: 0.4,
                color_change: false,
            };
            let bounds = AtmosphereBounds::conservative();
            let r_a = verify_application(&mut app_a, &bounds);
            let r_b = verify_application(&mut app_b, &bounds);
            assert_eq!(app_a.speed_scale, app_b.speed_scale);
            assert_eq!(app_a.density_scale, app_b.density_scale);
            assert_eq!(app_a.brightness_scale, app_b.brightness_scale);
            assert_eq!(app_a.glitch_pressure, app_b.glitch_pressure);
            assert_eq!(r_a.passed, r_b.passed);
            assert_eq!(r_a.clamped, r_b.clamped);
        }
    }

    // ── application_from_regime_params ──

    #[test]
    fn calm_regime_params_produce_identity_application() {
        let app = application_from_regime_params(1.0, 1.0, 1.0, 0.0);
        assert!(app.is_identity());
        assert!(!app.color_change);
    }

    #[test]
    fn regime_params_map_to_application_correctly() {
        let app = application_from_regime_params(1.5, 1.2, 1.5, 0.05);
        assert_eq!(app.speed_scale, 1.5);
        assert_eq!(app.density_scale, 1.2);
        assert_eq!(app.brightness_scale, 1.05);
        // glitch_mult 1.5 → pressure = (1.5 - 1.0) / 1.0 = 0.5
        assert_eq!(app.glitch_pressure, 0.5);
        assert!(!app.color_change);
    }

    #[test]
    fn regime_params_never_produce_color_change() {
        let app = application_from_regime_params(2.0, 1.5, 2.0, 0.1);
        assert!(!app.color_change);
    }

    #[test]
    fn non_calm_application_is_bounded_after_verification() {
        let mut app = application_from_regime_params(1.8, 1.3, 1.8, 0.08);
        let bounds = AtmosphereBounds::conservative();
        let result = verify_application(&mut app, &bounds);
        assert!(result.passed);
        assert!(!app.color_change);
        assert!(app.speed_scale <= bounds.speed_max);
        assert!(app.speed_scale >= bounds.speed_min);
        assert!(app.density_scale <= bounds.density_max);
        assert!(app.density_scale >= bounds.density_min);
        assert!(app.brightness_scale <= bounds.brightness_max);
        assert!(app.brightness_scale >= bounds.brightness_min);
        assert!(app.glitch_pressure <= bounds.glitch_pressure_max);
        assert!(app.glitch_pressure >= 0.0);
    }
}
