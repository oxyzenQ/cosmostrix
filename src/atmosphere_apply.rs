// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Atmosphere application adapter for Cosmostrix v4.0.0.
//!
//! Converts a verified AtmosphereApplication into safe runtime modulation values.
//! This module is the controlled seam between the atmosphere verifier (Phase 3)
//! and the actual renderer parameter space.
//!
//! ## Phase 6 Scope (Controlled Live Modulation)
//!
//! - `ControlledLive` application mode: internal-only mode that applies very
//!   subtle verified modulation through an extra clamping layer.
//! - `ControlledLiveBounds`: tighter bounds than conservative — speed ±4%,
//!   density ±4%, brightness ±3%, glitch_pressure ≤ 0.2.
//! - `apply_controlled_live_modulation()`: deterministic function that builds
//!   modulation from regime params through ControlledLive-specific bounds.
//! - Calm regime always produces identity regardless of mode.
//! - Default production mode remains Disabled (identity, no visual change).
//! - ControlledLive is NOT exposed via public CLI; only internal/test paths.
//!
//! ## Phase 5 Scope (Runtime Atmosphere Seam)
//!
//! - `AtmosphereEffectiveRuntime`: derives effective runtime values from base
//!   config + AtmosphereRuntimeModulation. Disabled modulation returns exact
//!   base values (identity).
//! - `derive_effective_runtime()`: computes effective speed, density,
//!   brightness_scale, glitch_pressure from base values and modulation.
//! - Values are clamped to existing safe runtime ranges.
//! - Color and terminal effects remain permanently false.
//! - No permanent mutation of config/profile/CLI args.
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
use crate::constants::{
    DENSITY_CLAMP_MAX, DENSITY_CLAMP_MIN, RUNTIME_SPEED_MAX, RUNTIME_SPEED_MIN,
};

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
    /// Internal-only controlled live modulation mode (Phase 6).
    /// Applies very subtle verified modulation through an extra clamping
    /// layer (ControlledLiveBounds). NOT exposed via public CLI.
    /// Only reachable through internal/test code paths.
    ControlledLive,
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
            Self::ControlledLive => "controlled-live",
            #[cfg(test)]
            Self::TestOnly => "test-only",
        }
    }

    /// Whether this mode allows non-identity modulation.
    pub(crate) fn allows_modulation(self) -> bool {
        match self {
            Self::InternalVerified => true,
            Self::ControlledLive => true,
            #[cfg(test)]
            Self::TestOnly => true,
            Self::Disabled => false,
        }
    }

    /// Whether this mode uses the controlled live modulation path.
    /// ControlledLive applies extra clamping via ControlledLiveBounds.
    pub(crate) fn is_controlled_live(self) -> bool {
        matches!(self, Self::ControlledLive)
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
/// For ControlledLive mode, an extra clamping layer (ControlledLiveBounds)
/// is applied to ensure modulation is extremely subtle.
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

    // ControlledLive mode: apply extra clamping layer.
    if mode.is_controlled_live() {
        return apply_controlled_live_modulation(app);
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

// ── Effective Runtime (Phase 5) ──────────────────────────────────────────

/// Derived effective runtime values from base config + atmosphere modulation.
///
/// This is the final output of the atmosphere pipeline before reaching the
/// renderer. For Disabled modulation (the default), all values equal the base
/// config values exactly — zero visual change from v3.9.0.
///
/// Speed and density are clamped to existing safe runtime ranges.
/// Color and terminal effects are always false.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AtmosphereEffectiveRuntime {
    /// Effective speed (chars/sec). Equals base_speed when modulation is identity.
    pub speed: f32,
    /// Effective density multiplier. Equals base_density when modulation is identity.
    pub density: f32,
    /// Effective brightness scale (1.0 = identity). Always 1.0 when modulation is identity.
    pub brightness_scale: f32,
    /// Effective glitch pressure (0.0 = default). Always 0.0 when modulation is identity.
    pub glitch_pressure: f32,
    /// Whether color change is allowed. Always false.
    pub color_change_allowed: bool,
    /// Whether terminal effect is allowed. Always false.
    pub terminal_effect_allowed: bool,
}

impl AtmosphereEffectiveRuntime {
    /// Whether this effective runtime is identity (no modulation applied).
    pub(crate) fn is_identity(&self) -> bool {
        !self.color_change_allowed && !self.terminal_effect_allowed
    }
}

/// Derive effective runtime values from base speed, base density, and modulation.
///
/// This is a pure deterministic function that computes the final renderer
/// parameters after applying atmosphere modulation. For identity modulation
/// (the default), returns exact base values unmodified.
///
/// Values are clamped to existing safe runtime ranges:
/// - Speed: RUNTIME_SPEED_MIN (1.0) .. RUNTIME_SPEED_MAX (100.0)
/// - Density: DENSITY_CLAMP_MIN (0.01) .. DENSITY_CLAMP_MAX (5.0)
/// - Brightness: passthrough from modulation (1.0 = identity)
/// - Glitch pressure: passthrough from modulation (0.0 = default)
#[must_use]
pub(crate) fn derive_effective_runtime(
    base_speed: f32,
    base_density: f32,
    modulation: &AtmosphereRuntimeModulation,
) -> AtmosphereEffectiveRuntime {
    // Speed: base * scale, clamped to safe range.
    let raw_speed = base_speed * modulation.speed_scale;
    let speed = raw_speed.clamp(RUNTIME_SPEED_MIN, RUNTIME_SPEED_MAX);

    // Density: base * scale, clamped to safe range.
    let raw_density = base_density * modulation.density_scale;
    let density = raw_density.clamp(DENSITY_CLAMP_MIN, DENSITY_CLAMP_MAX);

    AtmosphereEffectiveRuntime {
        speed,
        density,
        brightness_scale: modulation.brightness_scale,
        glitch_pressure: modulation.glitch_pressure,
        color_change_allowed: false,
        terminal_effect_allowed: false,
    }
}

// ── Controlled Live Bounds (Phase 6) ──────────────────────────────────────

/// Tighter bounds for ControlledLive modulation — much more restrictive than
/// conservative bounds to ensure the modulation is subtle and safe.
///
/// These bounds define the maximum deviation from identity (v3.9.0 behavior)
/// that ControlledLive mode allows. They are stricter than the verifier's
/// conservative bounds to guarantee the visual identity is preserved.
pub(crate) struct ControlledLiveBounds;

impl ControlledLiveBounds {
    /// Maximum absolute deviation for speed_scale from 1.0 (±4%).
    pub(crate) const SPEED_MAX_DELTA: f32 = 0.04;
    /// Maximum absolute deviation for density_scale from 1.0 (±4%).
    pub(crate) const DENSITY_MAX_DELTA: f32 = 0.04;
    /// Maximum absolute deviation for brightness_scale from 1.0 (±3%).
    pub(crate) const BRIGHTNESS_MAX_DELTA: f32 = 0.03;
    /// Maximum glitch_pressure (extremely low — barely perceptible).
    pub(crate) const GLITCH_PRESSURE_MAX: f32 = 0.2;
}

/// Apply ControlledLive modulation with extra clamping.
///
/// This function takes a verified application and applies ControlledLive-specific
/// bounds. The output is guaranteed to be within ControlledLiveBounds regardless
/// of the input application values.
///
/// Calm applications always return identity.
/// Color change and terminal effects are always false.
#[must_use]
pub(crate) fn apply_controlled_live_modulation(
    app: &AtmosphereApplication,
) -> AtmosphereRuntimeModulation {
    // Calm application is always identity regardless of mode.
    if app.is_identity() {
        return AtmosphereRuntimeModulation::identity();
    }

    // Clamp speed_scale to 1.0 ± SPEED_MAX_DELTA.
    let speed_scale = (app.speed_scale).clamp(
        1.0 - ControlledLiveBounds::SPEED_MAX_DELTA,
        1.0 + ControlledLiveBounds::SPEED_MAX_DELTA,
    );

    // Clamp density_scale to 1.0 ± DENSITY_MAX_DELTA.
    let density_scale = (app.density_scale).clamp(
        1.0 - ControlledLiveBounds::DENSITY_MAX_DELTA,
        1.0 + ControlledLiveBounds::DENSITY_MAX_DELTA,
    );

    // Clamp brightness_scale to 1.0 ± BRIGHTNESS_MAX_DELTA.
    let brightness_scale = (app.brightness_scale).clamp(
        1.0 - ControlledLiveBounds::BRIGHTNESS_MAX_DELTA,
        1.0 + ControlledLiveBounds::BRIGHTNESS_MAX_DELTA,
    );

    // Clamp glitch_pressure to 0.0 .. GLITCH_PRESSURE_MAX.
    let glitch_pressure =
        (app.glitch_pressure).clamp(0.0, ControlledLiveBounds::GLITCH_PRESSURE_MAX);

    AtmosphereRuntimeModulation {
        speed_scale,
        density_scale,
        brightness_scale,
        glitch_pressure,
        color_change_allowed: false,
        terminal_effect_allowed: false,
    }
}

/// Build ControlledLive modulation directly from regime parameters.
///
/// This is a deterministic function that combines the regime→application
/// conversion, conservative verification, and ControlledLive extra clamping
/// into a single step. Useful for internal/test paths.
///
/// For Calm regime, returns identity modulation.
#[must_use]
pub(crate) fn controlled_live_modulation_from_regime(
    regime: crate::atmosphere::AtmosphereRegime,
) -> AtmosphereRuntimeModulation {
    // Get regime params.
    let params = crate::atmosphere::params_for_regime(regime);

    // Convert to application.
    let mut app = crate::atmosphere_verifier::application_from_regime_params(
        params.speed_mult,
        params.density_mult,
        params.glitch_mult,
        params.brightness_bias,
    );

    // Verify with conservative bounds.
    let _ = crate::atmosphere_verifier::verify_application(
        &mut app,
        &crate::atmosphere_verifier::AtmosphereBounds::conservative(),
    );

    // Apply ControlledLive extra clamping.
    apply_controlled_live_modulation(&app)
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
            AtmosphereApplicationMode::ControlledLive,
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
    fn disabled_mode_never_allows_color_or_terminal() {
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
        assert!(!result.terminal_effect_allowed);
    }

    // ── apply_application: Calm application ──

    #[test]
    fn calm_application_returns_identity_in_all_modes() {
        let app = AtmosphereApplication::identity();
        for mode in [
            AtmosphereApplicationMode::Disabled,
            AtmosphereApplicationMode::InternalVerified,
            AtmosphereApplicationMode::ControlledLive,
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

    // ── Phase 5: AtmosphereEffectiveRuntime ──

    #[test]
    fn default_effective_runtime_equals_base_speed_and_density() {
        let modulation = AtmosphereRuntimeModulation::identity();
        let eff = derive_effective_runtime(20.0, 0.75, &modulation);
        assert_eq!(eff.speed, 20.0);
        assert_eq!(eff.density, 0.75);
        assert_eq!(eff.brightness_scale, 1.0);
        assert_eq!(eff.glitch_pressure, 0.0);
        assert!(!eff.color_change_allowed);
        assert!(!eff.terminal_effect_allowed);
    }

    #[test]
    fn disabled_modulation_effective_equals_base() {
        let app = AtmosphereApplication::identity();
        let modulation = apply_application(&app, AtmosphereApplicationMode::Disabled);
        let eff = derive_effective_runtime(15.0, 1.0, &modulation);
        assert_eq!(eff.speed, 15.0);
        assert_eq!(eff.density, 1.0);
    }

    #[test]
    fn calm_modulation_effective_equals_base() {
        let app = AtmosphereApplication::identity();
        let modulation = apply_application(&app, AtmosphereApplicationMode::InternalVerified);
        let eff = derive_effective_runtime(8.0, 0.5, &modulation);
        assert_eq!(eff.speed, 8.0);
        assert_eq!(eff.density, 0.5);
    }

    #[test]
    fn internal_verified_non_calm_derives_bounded_effective() {
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
        let modulation = apply_application(&app, AtmosphereApplicationMode::InternalVerified);
        let eff = derive_effective_runtime(20.0, 0.75, &modulation);
        assert!((eff.speed - 30.0).abs() < 0.01); // 20.0 * 1.5
        assert!((eff.density - 0.9).abs() < 0.01); // 0.75 * 1.2
        assert_eq!(eff.brightness_scale, 1.05);
        assert_eq!(eff.glitch_pressure, 0.3);
        assert!(!eff.color_change_allowed);
        assert!(!eff.terminal_effect_allowed);
    }

    #[test]
    fn effective_speed_is_clamped_to_safe_range() {
        let extreme = AtmosphereRuntimeModulation {
            speed_scale: 100.0,
            density_scale: 1.0,
            brightness_scale: 1.0,
            glitch_pressure: 0.0,
            color_change_allowed: false,
            terminal_effect_allowed: false,
        };
        let eff = derive_effective_runtime(50.0, 1.0, &extreme);
        assert_eq!(eff.speed, RUNTIME_SPEED_MAX); // clamped
    }

    #[test]
    fn effective_density_is_clamped_to_safe_range() {
        let extreme = AtmosphereRuntimeModulation {
            speed_scale: 1.0,
            density_scale: 100.0,
            brightness_scale: 1.0,
            glitch_pressure: 0.0,
            color_change_allowed: false,
            terminal_effect_allowed: false,
        };
        let eff = derive_effective_runtime(10.0, 0.1, &extreme);
        assert_eq!(eff.density, DENSITY_CLAMP_MAX); // clamped
    }

    #[test]
    fn effective_runtime_never_allows_color_change() {
        let mod_identity = AtmosphereRuntimeModulation::identity();
        let eff = derive_effective_runtime(10.0, 1.0, &mod_identity);
        assert!(!eff.color_change_allowed);

        let extreme = AtmosphereRuntimeModulation {
            speed_scale: 1.0,
            density_scale: 1.0,
            brightness_scale: 1.0,
            glitch_pressure: 0.0,
            color_change_allowed: true,
            terminal_effect_allowed: true,
        };
        let eff = derive_effective_runtime(10.0, 1.0, &extreme);
        // derive_effective_runtime always sets these to false
        assert!(!eff.color_change_allowed);
    }

    #[test]
    fn effective_runtime_never_allows_terminal_effects() {
        let extreme = AtmosphereRuntimeModulation {
            speed_scale: 1.0,
            density_scale: 1.0,
            brightness_scale: 1.0,
            glitch_pressure: 0.0,
            color_change_allowed: true,
            terminal_effect_allowed: true,
        };
        let eff = derive_effective_runtime(10.0, 1.0, &extreme);
        assert!(!eff.terminal_effect_allowed);
    }

    #[test]
    fn derive_effective_runtime_is_deterministic() {
        let modulation = AtmosphereRuntimeModulation::identity();
        for _ in 0..50 {
            let a = derive_effective_runtime(20.0, 0.75, &modulation);
            let b = derive_effective_runtime(20.0, 0.75, &modulation);
            assert_eq!(a.speed, b.speed);
            assert_eq!(a.density, b.density);
            assert_eq!(a.brightness_scale, b.brightness_scale);
        }
    }

    #[test]
    fn effective_runtime_speed_clamped_to_minimum() {
        // Speed scale near zero should clamp to RUNTIME_SPEED_MIN (1.0).
        let near_zero = AtmosphereRuntimeModulation {
            speed_scale: 0.001,
            density_scale: 1.0,
            brightness_scale: 1.0,
            glitch_pressure: 0.0,
            color_change_allowed: false,
            terminal_effect_allowed: false,
        };
        let eff = derive_effective_runtime(50.0, 1.0, &near_zero);
        assert_eq!(eff.speed, RUNTIME_SPEED_MIN);
    }

    #[test]
    fn effective_runtime_density_clamped_to_minimum() {
        // Density scale near zero should clamp to DENSITY_CLAMP_MIN (0.01).
        let near_zero = AtmosphereRuntimeModulation {
            speed_scale: 1.0,
            density_scale: 0.001,
            brightness_scale: 1.0,
            glitch_pressure: 0.0,
            color_change_allowed: false,
            terminal_effect_allowed: false,
        };
        let eff = derive_effective_runtime(10.0, 1.0, &near_zero);
        assert_eq!(eff.density, DENSITY_CLAMP_MIN);
    }

    #[test]
    fn effective_runtime_identity_check() {
        let mod_identity = AtmosphereRuntimeModulation::identity();
        let eff = derive_effective_runtime(20.0, 1.0, &mod_identity);
        // Identity effective runtime has both flags false.
        assert!(eff.is_identity());
    }

    #[test]
    fn derive_effective_runtime_combined_modulation() {
        let combined = AtmosphereRuntimeModulation {
            speed_scale: 1.3,
            density_scale: 0.8,
            brightness_scale: 1.05,
            glitch_pressure: 0.15,
            color_change_allowed: false,
            terminal_effect_allowed: false,
        };
        let eff = derive_effective_runtime(25.0, 1.5, &combined);
        assert!((eff.speed - 32.5).abs() < 0.01); // 25.0 * 1.3
        assert!((eff.density - 1.2).abs() < 0.01); // 1.5 * 0.8
        assert_eq!(eff.brightness_scale, 1.05);
        assert_eq!(eff.glitch_pressure, 0.15);
        assert!(!eff.color_change_allowed);
        assert!(!eff.terminal_effect_allowed);
    }

    #[test]
    fn derive_effective_runtime_with_test_only_mode() {
        let mut app = AtmosphereApplication {
            speed_scale: 1.6,
            density_scale: 1.1,
            brightness_scale: 1.02,
            glitch_pressure: 0.2,
            color_change: false,
        };
        let _ = crate::atmosphere_verifier::verify_application(
            &mut app,
            &crate::atmosphere_verifier::AtmosphereBounds::conservative(),
        );
        let modulation = apply_application(&app, AtmosphereApplicationMode::TestOnly);
        let eff = derive_effective_runtime(12.0, 0.8, &modulation);
        assert!((eff.speed - 19.2).abs() < 0.01); // 12.0 * 1.6
        assert!((eff.density - 0.88).abs() < 0.01); // 0.8 * 1.1
    }

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
