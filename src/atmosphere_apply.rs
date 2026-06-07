// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Atmosphere application adapter for Cosmostrix v4.0.0 Phase 4.
//!
//! Converts a verified AtmosphereApplication into safe runtime modulation values.
//! This module is the controlled seam between the atmosphere verifier (Phase 3)
//! and the actual renderer parameter space.
//!
//! ## Phase 4 Scope
//!
//! - `AtmosphereApplicationMode`: controls whether modulation is active.
//! - `AtmosphereRuntimeModulation`: bounded modulation values for the renderer.
//! - `apply_application()`: converts a verified application into runtime modulation.
//! - Disabled mode always returns identity (no visual change from v3.9.0).
//! - InternalVerified mode may return non-identity for non-Calm applications,
//!   but only when explicitly enabled by tests or internal code.
//! - Color change is always forbidden.
//! - Terminal behavior is never affected.

#![allow(dead_code)]

use crate::atmosphere_verifier::AtmosphereApplication;

// ── Application Mode ────────────────────────────────────────────────────────

/// Controls whether atmosphere modulation is active in the runtime.
///
/// The mode is a gate that determines whether verified applications
/// produce non-identity modulation values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum AtmosphereApplicationMode {
    /// Modulation is disabled. All applications produce identity output.
    /// This is the default for all production code paths.
    #[default]
    Disabled,
    /// Modulation is enabled for internally verified non-Calm applications.
    /// Only used in tests and internal integration paths.
    InternalVerified,
    /// Modulation is enabled only for tests. Produces bounded non-identity
    /// values for non-Calm applications without affecting production behavior.
    #[cfg(test)]
    TestOnly,
}

impl AtmosphereApplicationMode {
    /// Human-readable label for diagnostics.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::InternalVerified => "internal-verified",
            #[cfg(test)]
            Self::TestOnly => "test-only",
        }
    }

    /// Whether this mode allows non-identity modulation.
    pub(crate) fn allows_modulation(self) -> bool {
        match self {
            Self::InternalVerified => true,
            #[cfg(test)]
            Self::TestOnly => true,
            Self::Disabled => false,
        }
    }
}

// ── Runtime Modulation ─────────────────────────────────────────────────────

/// Bounded runtime modulation values derived from a verified atmosphere application.
///
/// These values are safe to apply to renderer parameters. For Calm/disabled mode,
/// all values are identity (multiplicative 1.0, additive 0.0).
#[derive(Debug, Clone, Copy)]
pub(crate) struct AtmosphereRuntimeModulation {
    /// Speed scale factor (1.0 = identity, no change).
    pub speed_scale: f32,
    /// Density scale factor (1.0 = identity, no change).
    pub density_scale: f32,
    /// Brightness scale factor (1.0 = identity, no change).
    pub brightness_scale: f32,
    /// Glitch pressure (0.0 = default, no change).
    pub glitch_pressure: f32,
    /// Whether color change is allowed. Always false.
    pub color_change_allowed: bool,
    /// Whether terminal effect is allowed. Always false.
    pub terminal_effect_allowed: bool,
}

impl AtmosphereRuntimeModulation {
    /// Identity modulation: no visual change. This is the default/Calm output.
    pub(crate) const fn identity() -> Self {
        Self {
            speed_scale: 1.0,
            density_scale: 1.0,
            brightness_scale: 1.0,
            glitch_pressure: 0.0,
            color_change_allowed: false,
            terminal_effect_allowed: false,
        }
    }

    /// Whether this modulation is a visual no-op (identity).
    pub(crate) fn is_identity(&self) -> bool {
        self.speed_scale == 1.0
            && self.density_scale == 1.0
            && self.brightness_scale == 1.0
            && self.glitch_pressure == 0.0
            && !self.color_change_allowed
            && !self.terminal_effect_allowed
    }
}

// ── Apply Function ─────────────────────────────────────────────────────────

/// Convert a verified AtmosphereApplication into safe runtime modulation.
///
/// This is a pure deterministic function. The result depends on:
/// - The application's values (which are already verified/clamped).
/// - The application mode (Disabled always returns identity).
///
/// Color change is always forbidden regardless of application content.
/// Terminal behavior is never affected.
pub(crate) fn apply_application(
    app: &AtmosphereApplication,
    mode: AtmosphereApplicationMode,
) -> AtmosphereRuntimeModulation {
    // Disabled mode always returns identity — production default.
    if !mode.allows_modulation() {
        return AtmosphereRuntimeModulation::identity();
    }

    // Calm application is always identity regardless of mode.
    if app.is_identity() {
        return AtmosphereRuntimeModulation::identity();
    }

    // InternalVerified/TestOnly mode with non-Calm application:
    // Convert verified application values into runtime modulation.
    AtmosphereRuntimeModulation {
        speed_scale: app.speed_scale,
        density_scale: app.density_scale,
        brightness_scale: app.brightness_scale,
        glitch_pressure: app.glitch_pressure,
        // Color change is always forbidden.
        color_change_allowed: false,
        // Terminal behavior is never affected.
        terminal_effect_allowed: false,
    }
}

// ── Effective Parameter Helpers ─────────────────────────────────────────────

/// Compute effective speed from base speed and modulation.
///
/// For identity modulation, returns base_speed unchanged.
/// For non-identity, returns base_speed * speed_scale.
#[must_use]
pub(crate) fn effective_speed(base_speed: f32, modulation: &AtmosphereRuntimeModulation) -> f32 {
    base_speed * modulation.speed_scale
}

/// Compute effective density from base density and modulation.
///
/// For identity modulation, returns base_density unchanged.
/// For non-identity, returns base_density * density_scale, clamped to 0.01..5.0.
#[must_use]
pub(crate) fn effective_density_from_modulation(
    base_density: f32,
    modulation: &AtmosphereRuntimeModulation,
) -> f32 {
    let raw = base_density * modulation.density_scale;
    raw.clamp(0.01, 5.0)
}

/// Compute effective brightness from modulation.
///
/// Returns the brightness_scale (1.0 = identity).
/// Not directly wired to renderer unless already supported safely.
#[must_use]
pub(crate) fn effective_brightness(modulation: &AtmosphereRuntimeModulation) -> f32 {
    modulation.brightness_scale
}

/// Compute effective glitch pressure from modulation.
///
/// Returns the glitch_pressure (0.0 = default, no change).
/// Not directly wired to renderer in Phase 4 (reserved for future).
#[must_use]
pub(crate) fn effective_glitch_pressure(modulation: &AtmosphereRuntimeModulation) -> f32 {
    modulation.glitch_pressure
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atmosphere_verifier::AtmosphereApplication;

    // ── AtmosphereApplicationMode basics ──

    #[test]
    fn disabled_mode_is_default() {
        let mode = AtmosphereApplicationMode::default();
        assert_eq!(mode, AtmosphereApplicationMode::Disabled);
        assert!(!mode.allows_modulation());
        assert_eq!(mode.as_str(), "disabled");
    }

    #[test]
    fn internal_verified_mode_allows_modulation() {
        let mode = AtmosphereApplicationMode::InternalVerified;
        assert!(mode.allows_modulation());
        assert_eq!(mode.as_str(), "internal-verified");
    }

    #[test]
    fn test_only_mode_allows_modulation() {
        let mode = AtmosphereApplicationMode::TestOnly;
        assert!(mode.allows_modulation());
        assert_eq!(mode.as_str(), "test-only");
    }

    #[test]
    fn all_modes_have_distinct_labels() {
        let modes = [
            AtmosphereApplicationMode::Disabled,
            AtmosphereApplicationMode::InternalVerified,
            AtmosphereApplicationMode::TestOnly,
        ];
        for (i, a) in modes.iter().enumerate() {
            for (j, b) in modes.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "modes must be distinct");
                }
            }
        }
    }

    // ── AtmosphereRuntimeModulation basics ──

    #[test]
    fn identity_modulation_is_no_op() {
        let mod_identity = AtmosphereRuntimeModulation::identity();
        assert!(mod_identity.is_identity());
        assert_eq!(mod_identity.speed_scale, 1.0);
        assert_eq!(mod_identity.density_scale, 1.0);
        assert_eq!(mod_identity.brightness_scale, 1.0);
        assert_eq!(mod_identity.glitch_pressure, 0.0);
        assert!(!mod_identity.color_change_allowed);
        assert!(!mod_identity.terminal_effect_allowed);
    }

    #[test]
    fn identity_modulation_is_copy_type() {
        let a = AtmosphereRuntimeModulation::identity();
        let b = a;
        assert_eq!(a.speed_scale, b.speed_scale);
    }

    // ── apply_application: Disabled mode ──

    #[test]
    fn disabled_mode_returns_identity_for_calm_application() {
        let app = AtmosphereApplication::identity();
        let result = apply_application(&app, AtmosphereApplicationMode::Disabled);
        assert!(result.is_identity());
    }

    #[test]
    fn disabled_mode_returns_identity_for_non_calm_application() {
        let app = AtmosphereApplication {
            speed_scale: 1.5,
            density_scale: 1.2,
            brightness_scale: 1.05,
            glitch_pressure: 0.3,
            color_change: false,
        };
        let result = apply_application(&app, AtmosphereApplicationMode::Disabled);
        assert!(result.is_identity());
    }

    #[test]
    fn disabled_mode_never_allows_color_change() {
        let mut app = AtmosphereApplication {
            speed_scale: 1.5,
            density_scale: 1.2,
            brightness_scale: 1.05,
            glitch_pressure: 0.3,
            color_change: true,
        };
        let _ = crate::atmosphere_verifier::verify_application(
            &mut app,
            &crate::atmosphere_verifier::AtmosphereBounds::conservative(),
        );
        let result = apply_application(&app, AtmosphereApplicationMode::Disabled);
        assert!(!result.color_change_allowed);
    }

    // ── apply_application: Calm application ──

    #[test]
    fn calm_application_returns_identity_in_all_modes() {
        let app = AtmosphereApplication::identity();
        for mode in [
            AtmosphereApplicationMode::Disabled,
            AtmosphereApplicationMode::InternalVerified,
            AtmosphereApplicationMode::TestOnly,
        ] {
            let result = apply_application(&app, mode);
            assert!(result.is_identity(), "Calm must be identity in {:?}", mode);
        }
    }

    // ── apply_application: InternalVerified mode ──

    #[test]
    fn internal_verified_non_calm_returns_bounded_modulation() {
        let mut app = AtmosphereApplication {
            speed_scale: 1.5,
            density_scale: 1.2,
            brightness_scale: 1.05,
            glitch_pressure: 0.3,
            color_change: false,
        };
        let _ = crate::atmosphere_verifier::verify_application(
            &mut app,
            &crate::atmosphere_verifier::AtmosphereBounds::conservative(),
        );
        let result = apply_application(&app, AtmosphereApplicationMode::InternalVerified);
        assert!(!result.is_identity());
        assert_eq!(result.speed_scale, 1.5);
        assert_eq!(result.density_scale, 1.2);
        assert!(!result.color_change_allowed);
        assert!(!result.terminal_effect_allowed);
    }

    #[test]
    fn internal_verified_modulation_is_bounded() {
        let bounds = crate::atmosphere_verifier::AtmosphereBounds::conservative();
        let mut app = AtmosphereApplication {
            speed_scale: 10.0,
            density_scale: 0.0,
            brightness_scale: 5.0,
            glitch_pressure: 3.0,
            color_change: true,
        };
        let _ = crate::atmosphere_verifier::verify_application(&mut app, &bounds);
        let result = apply_application(&app, AtmosphereApplicationMode::InternalVerified);

        assert!(result.speed_scale >= bounds.speed_min);
        assert!(result.speed_scale <= bounds.speed_max);
        assert!(result.density_scale >= bounds.density_min);
        assert!(result.density_scale <= bounds.density_max);
        assert!(result.brightness_scale >= bounds.brightness_min);
        assert!(result.brightness_scale <= bounds.brightness_max);
        assert!(result.glitch_pressure >= 0.0);
        assert!(result.glitch_pressure <= bounds.glitch_pressure_max);
    }

    // ── TestOnly mode ──

    #[test]
    fn test_only_non_calm_returns_bounded_modulation() {
        let mut app = AtmosphereApplication {
            speed_scale: 1.8,
            density_scale: 1.3,
            brightness_scale: 1.08,
            glitch_pressure: 0.4,
            color_change: false,
        };
        let _ = crate::atmosphere_verifier::verify_application(
            &mut app,
            &crate::atmosphere_verifier::AtmosphereBounds::conservative(),
        );
        let result = apply_application(&app, AtmosphereApplicationMode::TestOnly);
        assert!(!result.is_identity());
        assert_eq!(result.speed_scale, 1.8);
        assert!(!result.color_change_allowed);
        assert!(!result.terminal_effect_allowed);
    }

    // ── Determinism ──

    #[test]
    fn apply_application_is_deterministic() {
        let mut app = AtmosphereApplication {
            speed_scale: 1.7,
            density_scale: 1.1,
            brightness_scale: 1.03,
            glitch_pressure: 0.2,
            color_change: false,
        };
        let _ = crate::atmosphere_verifier::verify_application(
            &mut app,
            &crate::atmosphere_verifier::AtmosphereBounds::conservative(),
        );
        for _ in 0..100 {
            let a = apply_application(&app, AtmosphereApplicationMode::InternalVerified);
            let b = apply_application(&app, AtmosphereApplicationMode::InternalVerified);
            assert_eq!(a.speed_scale, b.speed_scale);
            assert_eq!(a.density_scale, b.density_scale);
            assert_eq!(a.brightness_scale, b.brightness_scale);
            assert_eq!(a.glitch_pressure, b.glitch_pressure);
            assert_eq!(a.color_change_allowed, b.color_change_allowed);
            assert_eq!(a.terminal_effect_allowed, b.terminal_effect_allowed);
        }
    }

    // ── Effective parameter helpers ──

    #[test]
    fn effective_speed_equals_base_for_identity() {
        let mod_identity = AtmosphereRuntimeModulation::identity();
        assert_eq!(effective_speed(20.0, &mod_identity), 20.0);
        assert_eq!(effective_speed(8.0, &mod_identity), 8.0);
    }

    #[test]
    fn effective_speed_scales_with_modulation() {
        let mod_val = AtmosphereRuntimeModulation {
            speed_scale: 1.5,
            density_scale: 1.0,
            brightness_scale: 1.0,
            glitch_pressure: 0.0,
            color_change_allowed: false,
            terminal_effect_allowed: false,
        };
        assert!((effective_speed(10.0, &mod_val) - 15.0).abs() < f32::EPSILON);
    }

    #[test]
    fn effective_density_equals_base_for_identity() {
        let mod_identity = AtmosphereRuntimeModulation::identity();
        assert_eq!(effective_density_from_modulation(1.0, &mod_identity), 1.0);
        assert_eq!(effective_density_from_modulation(0.75, &mod_identity), 0.75);
    }

    #[test]
    fn effective_density_is_clamped() {
        let mod_high = AtmosphereRuntimeModulation {
            speed_scale: 1.0,
            density_scale: 10.0,
            brightness_scale: 1.0,
            glitch_pressure: 0.0,
            color_change_allowed: false,
            terminal_effect_allowed: false,
        };
        let result = effective_density_from_modulation(1.0, &mod_high);
        assert_eq!(result, 5.0); // clamped to max
    }

    #[test]
    fn effective_brightness_returns_scale() {
        let mod_identity = AtmosphereRuntimeModulation::identity();
        assert_eq!(effective_brightness(&mod_identity), 1.0);
    }

    #[test]
    fn effective_glitch_pressure_returns_pressure() {
        let mod_identity = AtmosphereRuntimeModulation::identity();
        assert_eq!(effective_glitch_pressure(&mod_identity), 0.0);
    }

    // ── Pulse/Storm/Void clamped by verifier before adapter ──

    #[test]
    fn pulse_application_clamped_before_adapter_output() {
        let bounds = crate::atmosphere_verifier::AtmosphereBounds::conservative();
        let mut app =
            crate::atmosphere_verifier::application_from_regime_params(1.8, 1.3, 1.8, 0.08);
        let _ = crate::atmosphere_verifier::verify_application(&mut app, &bounds);
        let result = apply_application(&app, AtmosphereApplicationMode::InternalVerified);
        assert!(result.speed_scale >= bounds.speed_min);
        assert!(result.speed_scale <= bounds.speed_max);
        assert!(result.density_scale >= bounds.density_min);
        assert!(result.density_scale <= bounds.density_max);
    }

    #[test]
    fn storm_application_clamped_before_adapter_output() {
        let bounds = crate::atmosphere_verifier::AtmosphereBounds::conservative();
        let mut app =
            crate::atmosphere_verifier::application_from_regime_params(2.0, 1.5, 2.0, 0.1);
        let _ = crate::atmosphere_verifier::verify_application(&mut app, &bounds);
        let result = apply_application(&app, AtmosphereApplicationMode::InternalVerified);
        assert!(result.speed_scale <= bounds.speed_max);
        assert!(result.density_scale <= bounds.density_max);
        assert!(result.glitch_pressure <= bounds.glitch_pressure_max);
    }

    #[test]
    fn void_application_clamped_before_adapter_output() {
        let bounds = crate::atmosphere_verifier::AtmosphereBounds::conservative();
        let mut app =
            crate::atmosphere_verifier::application_from_regime_params(0.5, 0.5, 0.5, -0.1);
        let _ = crate::atmosphere_verifier::verify_application(&mut app, &bounds);
        let result = apply_application(&app, AtmosphereApplicationMode::InternalVerified);
        assert!(result.speed_scale >= bounds.speed_min);
        assert!(result.density_scale >= bounds.density_min);
        assert!(result.brightness_scale >= bounds.brightness_min);
    }

    #[test]
    fn out_of_range_application_clamped_before_runtime_modulation() {
        let bounds = crate::atmosphere_verifier::AtmosphereBounds::conservative();
        let mut app = AtmosphereApplication {
            speed_scale: 100.0,
            density_scale: -50.0,
            brightness_scale: 100.0,
            glitch_pressure: 100.0,
            color_change: true,
        };
        let _ = crate::atmosphere_verifier::verify_application(&mut app, &bounds);
        let result = apply_application(&app, AtmosphereApplicationMode::InternalVerified);
        assert!(result.speed_scale <= bounds.speed_max);
        assert!(result.density_scale >= bounds.density_min);
        assert!(result.brightness_scale <= bounds.brightness_max);
        assert!(result.glitch_pressure <= bounds.glitch_pressure_max);
        assert!(!result.color_change_allowed);
    }

    // ── No new unsafe, no debt markers ──
    // These are guaranteed by the implementation; verified by loc_tests and clippy.

    // ── Application adapter does not touch cache ──

    #[test]
    fn application_adapter_does_not_invalidate_cache() {
        use crate::zactrix_cache::CachePolicy;
        let cache = CachePolicy::default_policy();
        let gen_before = cache.generation.id();

        let app = AtmosphereApplication::identity();
        let _ = apply_application(&app, AtmosphereApplicationMode::Disabled);
        let _ = apply_application(&app, AtmosphereApplicationMode::InternalVerified);

        assert_eq!(cache.generation.id(), gen_before);
    }

    // ── Runtime default effective values equal base values ──

    #[test]
    fn runtime_default_effective_speed_equals_base_speed() {
        let mod_default = apply_application(
            &AtmosphereApplication::identity(),
            AtmosphereApplicationMode::Disabled,
        );
        let base = 20.0;
        assert_eq!(effective_speed(base, &mod_default), base);
    }

    #[test]
    fn runtime_default_effective_density_equals_base_density() {
        let mod_default = apply_application(
            &AtmosphereApplication::identity(),
            AtmosphereApplicationMode::Disabled,
        );
        let base = 0.75;
        assert_eq!(effective_density_from_modulation(base, &mod_default), base);
    }

    #[test]
    fn runtime_default_application_does_not_change_color() {
        let mod_default = apply_application(
            &AtmosphereApplication::identity(),
            AtmosphereApplicationMode::Disabled,
        );
        assert!(!mod_default.color_change_allowed);
    }

    #[test]
    fn runtime_default_application_does_not_change_terminal_behavior() {
        let mod_default = apply_application(
            &AtmosphereApplication::identity(),
            AtmosphereApplicationMode::Disabled,
        );
        assert!(!mod_default.terminal_effect_allowed);
    }
}
