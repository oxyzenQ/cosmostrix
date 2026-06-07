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
//! - ControlledLive bounds and helpers live in `atmosphere_controlled_live.rs`.
//! - Calm regime always produces identity regardless of mode.
//! - Default production mode remains Disabled (identity, no visual change).
//!
//! ## Phase 5 Scope (Runtime Atmosphere Seam)
//!
//! - `AtmosphereEffectiveRuntime` and `derive_effective_runtime()` live in
//!   `atmosphere_runtime.rs`.
//! - Disabled modulation returns exact base values (identity).
//!
//! ## Phase 4 Scope
//!
//! - `AtmosphereApplicationMode`: controls whether modulation is active.
//! - `AtmosphereRuntimeModulation`: bounded modulation values for the renderer.
//! - `apply_application()`: converts a verified application into runtime modulation.
//! - Disabled mode always returns identity (no visual change from v3.9.0).
//! - Color change is always forbidden.
//! - Terminal behavior is never affected.

#![allow(dead_code)]

use crate::atmosphere_controlled_live::apply_controlled_live_modulation;
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

// ── Re-exports: items moved to split files, exposed via atmosphere_apply path ──

#[cfg(test)]
pub(crate) use crate::atmosphere_controlled_live::controlled_live_modulation_from_regime;
#[cfg(test)]
pub(crate) use crate::atmosphere_controlled_live::ControlledLiveBounds;
pub(crate) use crate::atmosphere_runtime::derive_effective_runtime;
