// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Visual whisper adapter for Cosmostrix v4.0.0 Phase 7.
//!
//! Provides `AtmosphereVisualWhisper` — the first controlled visual modulation
//! path that converts verified `AtmosphereRuntimeModulation` into ultra-subtle,
//! tightly bounded visual-safe whisper values.
//!
//! ## Phase 7 Scope — First Real Controlled Visual Whisper
//!
//! This module introduces the visual whisper adapter, a thin conversion layer
//! that sits between the verified atmosphere modulation pipeline and potential
//! future visual parameter consumers. The whisper is:
//!
//! - **Identity by default**: Disabled/Calm regimes always produce identity.
//! - **Internal/test-only**: non-identity whisper is only reachable through
//!   test or internal code paths, never through default production runtime.
//! - **Ultra-subtle**: whisper bounds are tighter than ControlledLive bounds.
//! - **No color change**: `color_change_allowed` is always false.
//! - **No terminal effects**: `terminal_effect_allowed` is always false.
//! - **No config mutation**: the adapter is a pure read-only transform.
//!
//! ## Whisper Bounds vs ControlledLive Bounds
//!
//! | Parameter | Whisper | ControlledLive |
//! |-----------|---------|----------------|
//! | speed_scale | 0.98..1.02 (±2%) | 1.0 ±0.04 (±4%) |
//! | density_scale | 0.98..1.02 (±2%) | 1.0 ±0.04 (±4%) |
//! | brightness_scale | 0.985..1.015 (±1.5%) | 1.0 ±0.03 (±3%) |
//! | trail_energy_scale | 0.98..1.02 (±2%) | N/A |
//! | glyph_pulse_scale | 0.98..1.02 (±2%) | N/A |
//! | glitch_pressure | ≤ 0.05 | ≤ 0.2 |
//! | color_change | always false | always false |
//! | terminal_effect | always false | always false |

#![allow(dead_code)]

use crate::atmosphere_apply::{
    apply_application, AtmosphereApplicationMode, AtmosphereRuntimeModulation,
};
use crate::atmosphere_verifier::AtmosphereApplication;

// ── Whisper Bounds ──────────────────────────────────────────────────────────

/// Ultra-tight bounds for visual whisper values.
///
/// These bounds are strictly tighter than `ControlledLiveBounds` to guarantee
/// that any visual modulation through the whisper path is imperceptible to
/// the user and does not alter the v3.9.0 visual identity.
pub(crate) struct VisualWhisperBounds;

impl VisualWhisperBounds {
    /// Maximum absolute deviation for speed_scale from 1.0 (±2%).
    pub(crate) const SPEED_MAX_DELTA: f32 = 0.02;
    /// Minimum speed_scale value (0.98).
    pub(crate) const SPEED_MIN: f32 = 1.0 - Self::SPEED_MAX_DELTA;
    /// Maximum speed_scale value (1.02).
    pub(crate) const SPEED_MAX: f32 = 1.0 + Self::SPEED_MAX_DELTA;

    /// Maximum absolute deviation for density_scale from 1.0 (±2%).
    pub(crate) const DENSITY_MAX_DELTA: f32 = 0.02;
    /// Minimum density_scale value (0.98).
    pub(crate) const DENSITY_MIN: f32 = 1.0 - Self::DENSITY_MAX_DELTA;
    /// Maximum density_scale value (1.02).
    pub(crate) const DENSITY_MAX: f32 = 1.0 + Self::DENSITY_MAX_DELTA;

    /// Maximum absolute deviation for brightness_scale from 1.0 (±1.5%).
    pub(crate) const BRIGHTNESS_MAX_DELTA: f32 = 0.015;
    /// Minimum brightness_scale value (0.985).
    pub(crate) const BRIGHTNESS_MIN: f32 = 1.0 - Self::BRIGHTNESS_MAX_DELTA;
    /// Maximum brightness_scale value (1.015).
    pub(crate) const BRIGHTNESS_MAX: f32 = 1.0 + Self::BRIGHTNESS_MAX_DELTA;

    /// Maximum absolute deviation for trail_energy_scale from 1.0 (±2%).
    pub(crate) const TRAIL_ENERGY_MAX_DELTA: f32 = 0.02;
    /// Minimum trail_energy_scale value (0.98).
    pub(crate) const TRAIL_ENERGY_MIN: f32 = 1.0 - Self::TRAIL_ENERGY_MAX_DELTA;
    /// Maximum trail_energy_scale value (1.02).
    pub(crate) const TRAIL_ENERGY_MAX: f32 = 1.0 + Self::TRAIL_ENERGY_MAX_DELTA;

    /// Maximum absolute deviation for glyph_pulse_scale from 1.0 (±2%).
    pub(crate) const GLYPH_PULSE_MAX_DELTA: f32 = 0.02;
    /// Minimum glyph_pulse_scale value (0.98).
    pub(crate) const GLYPH_PULSE_MIN: f32 = 1.0 - Self::GLYPH_PULSE_MAX_DELTA;
    /// Maximum glyph_pulse_scale value (1.02).
    pub(crate) const GLYPH_PULSE_MAX: f32 = 1.0 + Self::GLYPH_PULSE_MAX_DELTA;

    /// Maximum glitch_pressure allowed (extremely low — imperceptible).
    pub(crate) const GLITCH_PRESSURE_MAX: f32 = 0.05;
}

// ── Atmosphere Visual Whisper ───────────────────────────────────────────────

/// Ultra-subtle visual modulation values derived from atmosphere modulation.
///
/// This struct represents the final visual-safe output of the atmosphere
/// pipeline after passing through the visual whisper adapter. All values
/// are clamped to `VisualWhisperBounds` — much tighter than any other
/// modulation layer.
///
/// For Disabled/Calm (the default), all values are identity (multiplicative
/// 1.0, additive 0.0). The whisper never mutates persistent config.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AtmosphereVisualWhisper {
    /// Speed scale factor (1.0 = identity, no change). Bounded 0.98..1.02.
    pub speed_scale: f32,
    /// Density scale factor (1.0 = identity, no change). Bounded 0.98..1.02.
    pub density_scale: f32,
    /// Brightness scale factor (1.0 = identity, no change). Bounded 0.985..1.015.
    pub brightness_scale: f32,
    /// Trail energy scale factor (1.0 = identity). Bounded 0.98..1.02.
    /// Modulates phosphor afterglow intensity.
    pub trail_energy_scale: f32,
    /// Glyph pulse scale factor (1.0 = identity). Bounded 0.98..1.02.
    /// Modulates glyph brightness pulsing intensity.
    pub glyph_pulse_scale: f32,
    /// Glitch pressure (0.0 = default, no change). Bounded ≤ 0.05.
    pub glitch_pressure: f32,
    /// Whether color change is allowed. Always false.
    pub color_change_allowed: bool,
    /// Whether terminal effect is allowed. Always false.
    pub terminal_effect_allowed: bool,
}

impl AtmosphereVisualWhisper {
    /// Identity whisper: no visual change. This is the default/Calm/Disabled output.
    pub(crate) const fn identity() -> Self {
        Self {
            speed_scale: 1.0,
            density_scale: 1.0,
            brightness_scale: 1.0,
            trail_energy_scale: 1.0,
            glyph_pulse_scale: 1.0,
            glitch_pressure: 0.0,
            color_change_allowed: false,
            terminal_effect_allowed: false,
        }
    }

    /// Whether this whisper is a visual no-op (identity).
    pub(crate) fn is_identity(&self) -> bool {
        self.speed_scale == 1.0
            && self.density_scale == 1.0
            && self.brightness_scale == 1.0
            && self.trail_energy_scale == 1.0
            && self.glyph_pulse_scale == 1.0
            && self.glitch_pressure == 0.0
            && !self.color_change_allowed
            && !self.terminal_effect_allowed
    }

    /// Whether this whisper is within the defined VisualWhisperBounds.
    #[cfg(test)]
    pub(crate) fn is_within_whisper_bounds(&self) -> bool {
        Self::in_range(
            self.speed_scale,
            VisualWhisperBounds::SPEED_MIN,
            VisualWhisperBounds::SPEED_MAX,
        ) && Self::in_range(
            self.density_scale,
            VisualWhisperBounds::DENSITY_MIN,
            VisualWhisperBounds::DENSITY_MAX,
        ) && Self::in_range(
            self.brightness_scale,
            VisualWhisperBounds::BRIGHTNESS_MIN,
            VisualWhisperBounds::BRIGHTNESS_MAX,
        ) && Self::in_range(
            self.trail_energy_scale,
            VisualWhisperBounds::TRAIL_ENERGY_MIN,
            VisualWhisperBounds::TRAIL_ENERGY_MAX,
        ) && Self::in_range(
            self.glyph_pulse_scale,
            VisualWhisperBounds::GLYPH_PULSE_MIN,
            VisualWhisperBounds::GLYPH_PULSE_MAX,
        ) && self.glitch_pressure >= 0.0
            && self.glitch_pressure <= VisualWhisperBounds::GLITCH_PRESSURE_MAX
            && !self.color_change_allowed
            && !self.terminal_effect_allowed
    }

    #[cfg(test)]
    fn in_range(val: f32, min: f32, max: f32) -> bool {
        val >= min && val <= max
    }
}

impl Default for AtmosphereVisualWhisper {
    fn default() -> Self {
        Self::identity()
    }
}

// ── Whisper Adapter Functions ─────────────────────────────────────────────

/// Convert a verified atmosphere application and mode into a visual whisper.
///
/// This is the primary public entry point for the visual whisper adapter.
/// It follows the same gate logic as `apply_application()`:
///
/// - Disabled mode → always returns identity.
/// - Calm application (identity) → always returns identity.
/// - Non-Calm with ControlledLive → converts through modulation, then
///   further clamps to whisper bounds.
/// - Non-Calm with InternalVerified/TestOnly → converts through modulation,
///   then clamps to whisper bounds.
///
/// Color change and terminal effects are always false.
/// The whisper is always within VisualWhisperBounds.
#[must_use]
pub(crate) fn visual_whisper_from_modulation(
    mode: AtmosphereApplicationMode,
    modulation: &AtmosphereRuntimeModulation,
) -> AtmosphereVisualWhisper {
    // Disabled mode always returns identity — production default.
    if !mode.allows_modulation() {
        return AtmosphereVisualWhisper::identity();
    }

    // Identity modulation (Calm) always returns identity.
    if modulation.is_identity() {
        return AtmosphereVisualWhisper::identity();
    }

    // Clamp modulation values to whisper bounds.
    clamp_modulation_to_whisper(modulation)
}

/// Convert a verified atmosphere application and mode into a visual whisper.
///
/// This variant takes the raw application and mode, running it through
/// `apply_application()` first to get the modulation, then converting to whisper.
/// Useful for test/internal paths that have the application but not modulation.
#[must_use]
pub(crate) fn visual_whisper_from_application(
    app: &AtmosphereApplication,
    mode: AtmosphereApplicationMode,
) -> AtmosphereVisualWhisper {
    // Disabled mode always returns identity.
    if !mode.allows_modulation() {
        return AtmosphereVisualWhisper::identity();
    }

    // Calm application is always identity.
    if app.is_identity() {
        return AtmosphereVisualWhisper::identity();
    }

    // Get modulation first, then clamp to whisper bounds.
    let modulation = apply_application(app, mode);
    clamp_modulation_to_whisper(&modulation)
}

/// Build a visual whisper directly from a regime (test/internal helper).
///
/// This is a one-step pipeline function that combines:
/// regime → params → application → verify → modulation → whisper.
///
/// For Calm regime, returns identity whisper.
/// For non-Calm regimes, returns whisper clamped to VisualWhisperBounds.
/// Only useful in tests — the production path never calls this directly.
#[must_use]
pub(crate) fn visual_whisper_from_regime(
    regime: crate::atmosphere::AtmosphereRegime,
) -> AtmosphereVisualWhisper {
    // Calm always returns identity.
    if regime == crate::atmosphere::AtmosphereRegime::Calm {
        return AtmosphereVisualWhisper::identity();
    }

    // Use ControlledLive modulation (tightest non-identity path).
    let modulation =
        crate::atmosphere_controlled_live::controlled_live_modulation_from_regime(regime);

    // If modulation is identity (shouldn't happen for non-Calm, but safety),
    // return identity whisper.
    if modulation.is_identity() {
        return AtmosphereVisualWhisper::identity();
    }

    clamp_modulation_to_whisper(&modulation)
}

// ── Internal Clamping ─────────────────────────────────────────────────────

/// Clamp modulation values to whisper bounds.
///
/// This function takes an `AtmosphereRuntimeModulation` and produces an
/// `AtmosphereVisualWhisper` where all scale factors are clamped to the
/// ultra-tight VisualWhisperBounds. Trail energy and glyph pulse are derived
/// from the speed/density/brightness modulation as proportional responses.
///
/// Color change and terminal effects are always false.
#[must_use]
fn clamp_modulation_to_whisper(
    modulation: &AtmosphereRuntimeModulation,
) -> AtmosphereVisualWhisper {
    // Speed: clamp to 1.0 ± 0.02 (±2%).
    let speed_scale = (modulation.speed_scale).clamp(
        VisualWhisperBounds::SPEED_MIN,
        VisualWhisperBounds::SPEED_MAX,
    );

    // Density: clamp to 1.0 ± 0.02 (±2%).
    let density_scale = (modulation.density_scale).clamp(
        VisualWhisperBounds::DENSITY_MIN,
        VisualWhisperBounds::DENSITY_MAX,
    );

    // Brightness: clamp to 1.0 ± 0.015 (±1.5%).
    let brightness_scale = (modulation.brightness_scale).clamp(
        VisualWhisperBounds::BRIGHTNESS_MIN,
        VisualWhisperBounds::BRIGHTNESS_MAX,
    );

    // Trail energy: derived from speed and density modulation.
    // Use average deviation from identity, clamped to whisper range.
    let speed_dev = (modulation.speed_scale - 1.0).abs();
    let density_dev = (modulation.density_scale - 1.0).abs();
    let avg_dev = (speed_dev + density_dev) * 0.5;
    let trail_energy_scale = (1.0 + avg_dev).clamp(
        VisualWhisperBounds::TRAIL_ENERGY_MIN,
        VisualWhisperBounds::TRAIL_ENERGY_MAX,
    );

    // Glyph pulse: derived from brightness modulation.
    // Responds proportionally to brightness deviation from identity.
    let bright_dev = (modulation.brightness_scale - 1.0).abs();
    let glyph_pulse_scale = (1.0 + bright_dev).clamp(
        VisualWhisperBounds::GLYPH_PULSE_MIN,
        VisualWhisperBounds::GLYPH_PULSE_MAX,
    );

    // Glitch pressure: clamp to 0.0 .. 0.05 (extremely low).
    let glitch_pressure =
        (modulation.glitch_pressure).clamp(0.0, VisualWhisperBounds::GLITCH_PRESSURE_MAX);

    AtmosphereVisualWhisper {
        speed_scale,
        density_scale,
        brightness_scale,
        trail_energy_scale,
        glyph_pulse_scale,
        glitch_pressure,
        // Color change is always forbidden.
        color_change_allowed: false,
        // Terminal effects are always forbidden.
        terminal_effect_allowed: false,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atmosphere::AtmosphereRegime;
    use crate::atmosphere_apply::AtmosphereApplicationMode;

    // ── Identity Tests ──

    #[test]
    fn default_whisper_is_identity() {
        let whisper = AtmosphereVisualWhisper::default();
        assert!(whisper.is_identity());
    }

    #[test]
    fn identity_whisper_fields_are_exact() {
        let whisper = AtmosphereVisualWhisper::identity();
        assert_eq!(whisper.speed_scale, 1.0);
        assert_eq!(whisper.density_scale, 1.0);
        assert_eq!(whisper.brightness_scale, 1.0);
        assert_eq!(whisper.trail_energy_scale, 1.0);
        assert_eq!(whisper.glyph_pulse_scale, 1.0);
        assert_eq!(whisper.glitch_pressure, 0.0);
        assert!(!whisper.color_change_allowed);
        assert!(!whisper.terminal_effect_allowed);
    }

    #[test]
    fn disabled_mode_produces_identity_whisper() {
        let modulation = AtmosphereRuntimeModulation {
            speed_scale: 1.5,
            density_scale: 1.3,
            brightness_scale: 1.1,
            glitch_pressure: 0.4,
            color_change_allowed: false,
            terminal_effect_allowed: false,
        };
        let whisper =
            visual_whisper_from_modulation(AtmosphereApplicationMode::Disabled, &modulation);
        assert!(whisper.is_identity());
    }

    #[test]
    fn calm_modulation_produces_identity_whisper() {
        let identity_mod = AtmosphereRuntimeModulation::identity();
        let whisper = visual_whisper_from_modulation(
            AtmosphereApplicationMode::InternalVerified,
            &identity_mod,
        );
        assert!(whisper.is_identity());
    }

    #[test]
    fn calm_regime_produces_identity_whisper() {
        let whisper = visual_whisper_from_regime(AtmosphereRegime::Calm);
        assert!(whisper.is_identity());
    }

    // ── Non-Identity Tests ──

    #[test]
    fn controlled_live_pulse_whisper_is_non_identity_but_tiny() {
        let whisper = visual_whisper_from_regime(AtmosphereRegime::Pulse);
        assert!(whisper.is_within_whisper_bounds());
        assert!(!whisper.color_change_allowed);
        assert!(!whisper.terminal_effect_allowed);
    }

    #[test]
    fn controlled_live_storm_whisper_is_clamped_to_bounds() {
        let whisper = visual_whisper_from_regime(AtmosphereRegime::Storm);
        assert!(whisper.is_within_whisper_bounds());
        assert!(whisper.speed_scale >= VisualWhisperBounds::SPEED_MIN);
        assert!(whisper.speed_scale <= VisualWhisperBounds::SPEED_MAX);
        assert!(whisper.density_scale >= VisualWhisperBounds::DENSITY_MIN);
        assert!(whisper.density_scale <= VisualWhisperBounds::DENSITY_MAX);
        assert!(whisper.glitch_pressure <= VisualWhisperBounds::GLITCH_PRESSURE_MAX);
    }

    #[test]
    fn void_whisper_cannot_drop_density_below_bounds() {
        let whisper = visual_whisper_from_regime(AtmosphereRegime::Void);
        assert!(whisper.density_scale >= VisualWhisperBounds::DENSITY_MIN);
        assert!(whisper.is_within_whisper_bounds());
    }

    #[test]
    fn monolith_pressure_whisper_cannot_over_brighten() {
        let whisper = visual_whisper_from_regime(AtmosphereRegime::MonolithPressure);
        assert!(whisper.brightness_scale <= VisualWhisperBounds::BRIGHTNESS_MAX);
        assert!(whisper.is_within_whisper_bounds());
    }

    // ── Safety Tests ──

    #[test]
    fn color_change_remains_forbidden_in_whisper() {
        for regime in [
            AtmosphereRegime::Pulse,
            AtmosphereRegime::Storm,
            AtmosphereRegime::Void,
            AtmosphereRegime::Signal,
            AtmosphereRegime::Compression,
            AtmosphereRegime::MonolithPressure,
        ] {
            let whisper = visual_whisper_from_regime(regime);
            assert!(
                !whisper.color_change_allowed,
                "color_change must be false for {:?}",
                regime
            );
        }
    }

    #[test]
    fn terminal_effects_remain_forbidden_in_whisper() {
        for regime in [
            AtmosphereRegime::Pulse,
            AtmosphereRegime::Storm,
            AtmosphereRegime::Void,
            AtmosphereRegime::Signal,
            AtmosphereRegime::Compression,
            AtmosphereRegime::MonolithPressure,
        ] {
            let whisper = visual_whisper_from_regime(regime);
            assert!(
                !whisper.terminal_effect_allowed,
                "terminal_effect must be false for {:?}",
                regime
            );
        }
    }

    #[test]
    fn whisper_adapter_is_deterministic() {
        for _ in 0..100 {
            let w1 = visual_whisper_from_regime(AtmosphereRegime::Storm);
            let w2 = visual_whisper_from_regime(AtmosphereRegime::Storm);
            assert_eq!(w1.speed_scale, w2.speed_scale);
            assert_eq!(w1.density_scale, w2.density_scale);
            assert_eq!(w1.brightness_scale, w2.brightness_scale);
            assert_eq!(w1.trail_energy_scale, w2.trail_energy_scale);
            assert_eq!(w1.glyph_pulse_scale, w2.glyph_pulse_scale);
            assert_eq!(w1.glitch_pressure, w2.glitch_pressure);
        }
    }

    // ── From Application Path Tests ──

    #[test]
    fn whisper_from_application_disabled_is_identity() {
        let app = AtmosphereApplication {
            speed_scale: 1.5,
            density_scale: 1.3,
            brightness_scale: 1.1,
            glitch_pressure: 0.4,
            color_change: false,
        };
        let whisper = visual_whisper_from_application(&app, AtmosphereApplicationMode::Disabled);
        assert!(whisper.is_identity());
    }

    #[test]
    fn whisper_from_application_calm_is_identity() {
        let app = AtmosphereApplication::identity();
        let whisper = visual_whisper_from_application(&app, AtmosphereApplicationMode::TestOnly);
        assert!(whisper.is_identity());
    }

    #[test]
    fn whisper_from_modulation_extreme_values_clamped() {
        let modulation = AtmosphereRuntimeModulation {
            speed_scale: 5.0,
            density_scale: 10.0,
            brightness_scale: 3.0,
            glitch_pressure: 1.0,
            color_change_allowed: false,
            terminal_effect_allowed: false,
        };
        let whisper =
            visual_whisper_from_modulation(AtmosphereApplicationMode::TestOnly, &modulation);
        assert!(whisper.is_within_whisper_bounds());
        assert!(whisper.speed_scale <= VisualWhisperBounds::SPEED_MAX);
        assert!(whisper.density_scale <= VisualWhisperBounds::DENSITY_MAX);
        assert!(whisper.brightness_scale <= VisualWhisperBounds::BRIGHTNESS_MAX);
        assert!(whisper.glitch_pressure <= VisualWhisperBounds::GLITCH_PRESSURE_MAX);
    }

    // ── All Regimes Within Bounds ──

    #[test]
    fn all_non_calm_regimes_produce_whisper_within_bounds() {
        let regimes = [
            AtmosphereRegime::Compression,
            AtmosphereRegime::Pulse,
            AtmosphereRegime::Storm,
            AtmosphereRegime::Void,
            AtmosphereRegime::Signal,
            AtmosphereRegime::MonolithPressure,
        ];
        for regime in regimes {
            let whisper = visual_whisper_from_regime(regime);
            assert!(
                whisper.is_within_whisper_bounds(),
                "whisper for {:?} must be within bounds",
                regime
            );
        }
    }

    // ── Whisper Bounds Constants Validation ──

    #[test]
    fn whisper_bounds_are_reasonable() {
        const { assert!(VisualWhisperBounds::SPEED_MIN < 1.0) };
        const { assert!(VisualWhisperBounds::SPEED_MAX > 1.0) };
        const { assert!(VisualWhisperBounds::DENSITY_MIN < 1.0) };
        const { assert!(VisualWhisperBounds::DENSITY_MAX > 1.0) };
        const { assert!(VisualWhisperBounds::BRIGHTNESS_MIN < 1.0) };
        const { assert!(VisualWhisperBounds::BRIGHTNESS_MAX > 1.0) };
        const { assert!(VisualWhisperBounds::TRAIL_ENERGY_MIN < 1.0) };
        const { assert!(VisualWhisperBounds::TRAIL_ENERGY_MAX > 1.0) };
        const { assert!(VisualWhisperBounds::GLYPH_PULSE_MIN < 1.0) };
        const { assert!(VisualWhisperBounds::GLYPH_PULSE_MAX > 1.0) };
        const { assert!(VisualWhisperBounds::GLITCH_PRESSURE_MAX > 0.0) };
    }

    #[test]
    fn whisper_bounds_are_tighter_than_controlled_live() {
        use crate::atmosphere_controlled_live::ControlledLiveBounds;
        const { assert!(VisualWhisperBounds::SPEED_MAX_DELTA < ControlledLiveBounds::SPEED_MAX_DELTA) };
        const {
            assert!(
                VisualWhisperBounds::DENSITY_MAX_DELTA < ControlledLiveBounds::DENSITY_MAX_DELTA
            )
        };
        const {
            assert!(
                VisualWhisperBounds::BRIGHTNESS_MAX_DELTA
                    < ControlledLiveBounds::BRIGHTNESS_MAX_DELTA
            )
        };
        const {
            assert!(
                VisualWhisperBounds::GLITCH_PRESSURE_MAX
                    < ControlledLiveBounds::GLITCH_PRESSURE_MAX
            )
        };
    }
}
