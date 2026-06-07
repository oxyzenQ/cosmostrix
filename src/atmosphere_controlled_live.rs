// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Controlled Live modulation for Cosmostrix v4.0.0 Phase 6.
//!
//! Provides the `ControlledLiveBounds` and `apply_controlled_live_modulation()`
//! function that applies an extra clamping layer on top of verified atmosphere
//! applications. This module is internal-only and NOT exposed via public CLI.
//!
//! - Calm regime always produces identity regardless of mode.
//! - Color change and terminal effects are always false.
//! - All bounds are subtle: speed ±4%, density ±4%, brightness ±3%, glitch ≤0.2.

#![allow(dead_code)]

use crate::atmosphere_apply::AtmosphereRuntimeModulation;
use crate::atmosphere_verifier::AtmosphereApplication;

// ── Controlled Live Bounds ──────────────────────────────────────────────

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
