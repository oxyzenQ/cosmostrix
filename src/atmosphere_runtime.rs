// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Effective runtime derivation for Cosmostrix v4.0.0 Phase 5.
//!
//! Provides `AtmosphereEffectiveRuntime` and `derive_effective_runtime()` which
//! compute final renderer parameters from base config + atmosphere modulation.
//! For Disabled modulation (the default), all values equal the base config
//! values exactly — zero visual change from v3.9.0.

#![allow(dead_code)]

use crate::atmosphere_apply::AtmosphereRuntimeModulation;
use crate::constants::{
    DENSITY_CLAMP_MAX, DENSITY_CLAMP_MIN, RUNTIME_SPEED_MAX, RUNTIME_SPEED_MIN,
};

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
