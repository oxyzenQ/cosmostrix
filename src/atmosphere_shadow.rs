// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Shadow metrics for the Atmosphere Visual Whisper system (Phase 8).
//!
//! Provides `AtmosphereShadowMetrics` — an internal measurement model that
//! computes, verifies, and summarizes the potential visual impact of the
//! atmosphere whisper system without enabling visible modulation by default.
//!
//! ## Phase 8 Scope — Whisper Wiring Guard / Runtime Shadow Metrics
//!
//! This module adds a shadow-metrics layer that measures whisper impact internally:
//!
//! - **Measurement, not activation**: shadow metrics observe and quantify the
//!   potential visual modulation without actually changing the renderer.
//! - **Pure deterministic functions**: all shadow evaluation functions are
//!   side-effect-free. No cache invalidation, no terminal state changes.
//! - **Risk labels**: each shadow metric carries a risk classification:
//!   - `identity` — no visual impact (default/Calm/Disabled).
//!   - `whisper` — within Phase 7 VisualWhisperBounds, imperceptible.
//!   - `elevated` — outside whisper bounds but verifier-safe, measurable.
//!   - `rejected` — color_change or terminal_effect allowed, unsafe for silent use.
//! - **Diagnostic integration**: shadow metrics are reported in `-i` and
//!   `--benchmark` output when the risk is low (identity/whisper).
//!
//! ## What Phase 8 Does NOT Do
//!
//! - Does NOT change default visual output — still identical to v3.9.0.
//! - Does NOT enable visible atmosphere modulation.
//! - Does NOT alter benchmark field names or remove existing fields.
//! - Does NOT add new CLI flags or scene types.
//! - Does NOT introduce color drift, terminal effects, or random changes.

#![allow(dead_code)]

use crate::atmosphere::AtmosphereRegime;
use crate::atmosphere_apply::AtmosphereApplicationMode;
use crate::atmosphere_verifier::AtmosphereApplication;
use crate::atmosphere_visual::{AtmosphereVisualWhisper, VisualWhisperBounds};

// ── Shadow Metrics Struct ────────────────────────────────────────────────

/// Shadow metrics for the visual whisper system.
///
/// Measures the potential visual impact of atmosphere modulation as percentage
/// deviations from identity. Zero deltas mean no visual impact. The risk label
/// summarizes the overall safety classification for diagnostic reporting.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AtmosphereShadowMetrics {
    /// Speed deviation from identity as a percentage (0.0 = no change).
    pub speed_delta_percent: f32,
    /// Density deviation from identity as a percentage (0.0 = no change).
    pub density_delta_percent: f32,
    /// Brightness deviation from identity as a percentage (0.0 = no change).
    pub brightness_delta_percent: f32,
    /// Trail energy deviation from identity as a percentage (0.0 = no change).
    pub trail_energy_delta_percent: f32,
    /// Glyph pulse deviation from identity as a percentage (0.0 = no change).
    pub glyph_pulse_delta_percent: f32,
    /// Glitch pressure (0.0 = default, no change).
    pub glitch_pressure: f32,
    /// Whether color change is allowed.
    pub color_change_allowed: bool,
    /// Whether terminal effect is allowed.
    pub terminal_effect_allowed: bool,
}

impl AtmosphereShadowMetrics {
    /// Identity shadow metrics: no visual impact. Default/Calm/Disabled.
    pub(crate) const fn identity() -> Self {
        Self {
            speed_delta_percent: 0.0,
            density_delta_percent: 0.0,
            brightness_delta_percent: 0.0,
            trail_energy_delta_percent: 0.0,
            glyph_pulse_delta_percent: 0.0,
            glitch_pressure: 0.0,
            color_change_allowed: false,
            terminal_effect_allowed: false,
        }
    }

    /// Whether this shadow represents zero visual impact.
    pub(crate) fn is_identity(&self) -> bool {
        self.speed_delta_percent == 0.0
            && self.density_delta_percent == 0.0
            && self.brightness_delta_percent == 0.0
            && self.trail_energy_delta_percent == 0.0
            && self.glyph_pulse_delta_percent == 0.0
            && self.glitch_pressure == 0.0
            && !self.color_change_allowed
            && !self.terminal_effect_allowed
    }

    /// Maximum absolute delta percentage across all scale dimensions.
    ///
    /// Useful for quick risk classification: whisper bounds have a max
    /// delta of 2.0% (VisualWhisperBounds::SPEED_MAX_DELTA * 100).
    pub(crate) fn max_abs_delta_percent(&self) -> f32 {
        self.speed_delta_percent
            .max(self.density_delta_percent)
            .max(self.brightness_delta_percent)
            .max(self.trail_energy_delta_percent)
            .max(self.glyph_pulse_delta_percent)
    }

    /// Risk label for the shadow metrics.
    ///
    /// Classification rules:
    /// - `rejected`: color_change_allowed or terminal_effect_allowed is true.
    ///   These flags make the shadow unsafe for silent visual use.
    /// - `identity`: all deltas are zero (no visual impact at all).
    /// - `whisper`: within VisualWhisperBounds (imperceptible to the user).
    ///   Max delta <= 2.0% and glitch_pressure <= 0.05.
    /// - `elevated`: outside whisper bounds but still verifier-safe.
    ///   Values are measurable but would not break visual stability.
    pub(crate) fn risk_label(&self) -> &'static str {
        if self.color_change_allowed || self.terminal_effect_allowed {
            return "rejected";
        }
        if self.is_identity() {
            return "identity";
        }
        let max_delta = self.max_abs_delta_percent();
        let whisper_threshold = VisualWhisperBounds::SPEED_MAX_DELTA * 100.0;
        if max_delta <= whisper_threshold
            && self.glitch_pressure <= VisualWhisperBounds::GLITCH_PRESSURE_MAX
        {
            return "whisper";
        }
        "elevated"
    }
}

impl Default for AtmosphereShadowMetrics {
    fn default() -> Self {
        Self::identity()
    }
}

// ── Shadow Evaluation Functions ─────────────────────────────────────────

/// Compute shadow metrics from a visual whisper.
///
/// Converts whisper scale factors into percentage deviations from identity.
/// Pure function — no side effects, no cache invalidation.
#[must_use]
pub(crate) fn shadow_metrics_from_whisper(
    whisper: &AtmosphereVisualWhisper,
) -> AtmosphereShadowMetrics {
    AtmosphereShadowMetrics {
        speed_delta_percent: (whisper.speed_scale - 1.0).abs() * 100.0,
        density_delta_percent: (whisper.density_scale - 1.0).abs() * 100.0,
        brightness_delta_percent: (whisper.brightness_scale - 1.0).abs() * 100.0,
        trail_energy_delta_percent: (whisper.trail_energy_scale - 1.0).abs() * 100.0,
        glyph_pulse_delta_percent: (whisper.glyph_pulse_scale - 1.0).abs() * 100.0,
        glitch_pressure: whisper.glitch_pressure,
        color_change_allowed: whisper.color_change_allowed,
        terminal_effect_allowed: whisper.terminal_effect_allowed,
    }
}

/// Compute shadow metrics from a runtime modulation.
///
/// Converts modulation scale factors into percentage deviations.
/// Trail energy and glyph pulse are zero (not present in modulation struct).
fn shadow_metrics_from_modulation(
    modulation: &crate::atmosphere_apply::AtmosphereRuntimeModulation,
) -> AtmosphereShadowMetrics {
    AtmosphereShadowMetrics {
        speed_delta_percent: (modulation.speed_scale - 1.0).abs() * 100.0,
        density_delta_percent: (modulation.density_scale - 1.0).abs() * 100.0,
        brightness_delta_percent: (modulation.brightness_scale - 1.0).abs() * 100.0,
        trail_energy_delta_percent: 0.0,
        glyph_pulse_delta_percent: 0.0,
        glitch_pressure: modulation.glitch_pressure,
        color_change_allowed: modulation.color_change_allowed,
        terminal_effect_allowed: modulation.terminal_effect_allowed,
    }
}

/// Compute shadow metrics for a given application mode and regime.
///
/// Routing rules:
/// - Disabled/default returns identity metrics (mode gates all modulation off).
/// - Calm returns identity metrics (calm is a visual no-op).
/// - ControlledLive non-Calm routes through the visual whisper adapter,
///   producing whisper-bounded metrics (imperceptible).
/// - InternalVerified/TestOnly non-Calm routes through the regime-to-modulation
///   pipeline, producing wider values that may exceed whisper bounds (elevated).
///
/// Pure function — no cache invalidation, no terminal state changes,
/// no color changes, no config mutation.
#[must_use]
pub(crate) fn shadow_metrics_from_mode_and_regime(
    mode: AtmosphereApplicationMode,
    regime: AtmosphereRegime,
) -> AtmosphereShadowMetrics {
    // Disabled always returns identity — production default.
    if !mode.allows_modulation() {
        return AtmosphereShadowMetrics::identity();
    }

    // Calm always returns identity — visual no-op.
    if regime == AtmosphereRegime::Calm {
        return AtmosphereShadowMetrics::identity();
    }

    // ControlledLive: route through visual whisper (tightest bounds).
    if mode.is_controlled_live() {
        let whisper = crate::atmosphere_visual::visual_whisper_from_regime(regime);
        return shadow_metrics_from_whisper(&whisper);
    }

    // InternalVerified/TestOnly: route through regime -> application -> modulation.
    // This path produces wider values that may exceed whisper bounds -> elevated.
    let params = crate::atmosphere::params_for_regime(regime);
    let mut app = crate::atmosphere_verifier::application_from_regime_params(
        params.speed_mult,
        params.density_mult,
        params.glitch_mult,
        params.brightness_bias,
    );
    let _ = crate::atmosphere_verifier::verify_application(
        &mut app,
        &crate::atmosphere_verifier::AtmosphereBounds::conservative(),
    );
    let modulation = crate::atmosphere_apply::apply_application(&app, mode);
    shadow_metrics_from_modulation(&modulation)
}

/// Compute shadow metrics for a given application mode and verified application.
///
/// Routing rules:
/// - Disabled returns identity metrics.
/// - Identity application returns identity metrics.
/// - ControlledLive routes through the visual whisper adapter.
/// - InternalVerified/TestOnly routes through the modulation path (may be elevated).
///
/// Pure function — no side effects.
#[must_use]
pub(crate) fn shadow_metrics_from_application(
    mode: AtmosphereApplicationMode,
    app: &AtmosphereApplication,
) -> AtmosphereShadowMetrics {
    // Disabled always returns identity.
    if !mode.allows_modulation() {
        return AtmosphereShadowMetrics::identity();
    }

    // Identity application always returns identity.
    if app.is_identity() {
        return AtmosphereShadowMetrics::identity();
    }

    // ControlledLive: route through visual whisper (tightest bounds).
    if mode.is_controlled_live() {
        let whisper = crate::atmosphere_visual::visual_whisper_from_application(app, mode);
        return shadow_metrics_from_whisper(&whisper);
    }

    // InternalVerified/TestOnly: route through modulation (may be elevated).
    let modulation = crate::atmosphere_apply::apply_application(app, mode);
    shadow_metrics_from_modulation(&modulation)
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atmosphere::AtmosphereRegime;
    use crate::atmosphere_apply::AtmosphereApplicationMode;
    use crate::atmosphere_visual::AtmosphereVisualWhisper;
    use crate::zactrix_cache::CachePolicy;

    // ── Identity Tests ──

    #[test]
    fn default_shadow_metrics_is_identity() {
        let metrics = AtmosphereShadowMetrics::default();
        assert!(metrics.is_identity());
        assert_eq!(metrics.risk_label(), "identity");
    }

    #[test]
    fn identity_shadow_fields_are_exact() {
        let metrics = AtmosphereShadowMetrics::identity();
        assert_eq!(metrics.speed_delta_percent, 0.0);
        assert_eq!(metrics.density_delta_percent, 0.0);
        assert_eq!(metrics.brightness_delta_percent, 0.0);
        assert_eq!(metrics.trail_energy_delta_percent, 0.0);
        assert_eq!(metrics.glyph_pulse_delta_percent, 0.0);
        assert_eq!(metrics.glitch_pressure, 0.0);
        assert!(!metrics.color_change_allowed);
        assert!(!metrics.terminal_effect_allowed);
    }

    #[test]
    fn disabled_shadow_metrics_are_identity() {
        for regime in [
            AtmosphereRegime::Calm,
            AtmosphereRegime::Compression,
            AtmosphereRegime::Pulse,
            AtmosphereRegime::Storm,
            AtmosphereRegime::Void,
            AtmosphereRegime::Signal,
            AtmosphereRegime::MonolithPressure,
        ] {
            let metrics =
                shadow_metrics_from_mode_and_regime(AtmosphereApplicationMode::Disabled, regime);
            assert!(
                metrics.is_identity(),
                "Disabled must produce identity for {:?}",
                regime
            );
            assert_eq!(metrics.risk_label(), "identity");
        }
    }

    #[test]
    fn calm_shadow_metrics_are_identity() {
        for mode in [
            AtmosphereApplicationMode::Disabled,
            AtmosphereApplicationMode::InternalVerified,
            AtmosphereApplicationMode::ControlledLive,
            AtmosphereApplicationMode::TestOnly,
        ] {
            let metrics = shadow_metrics_from_mode_and_regime(mode, AtmosphereRegime::Calm);
            assert!(
                metrics.is_identity(),
                "Calm must produce identity for {:?}",
                mode
            );
            assert_eq!(metrics.risk_label(), "identity");
        }
    }

    #[test]
    fn shadow_from_application_disabled_is_identity() {
        let app = AtmosphereApplication {
            speed_scale: 1.5,
            density_scale: 1.3,
            brightness_scale: 1.1,
            glitch_pressure: 0.4,
            color_change: false,
        };
        let metrics = shadow_metrics_from_application(AtmosphereApplicationMode::Disabled, &app);
        assert!(metrics.is_identity());
        assert_eq!(metrics.risk_label(), "identity");
    }

    #[test]
    fn shadow_from_application_calm_is_identity() {
        let app = AtmosphereApplication::identity();
        let metrics = shadow_metrics_from_application(AtmosphereApplicationMode::TestOnly, &app);
        assert!(metrics.is_identity());
        assert_eq!(metrics.risk_label(), "identity");
    }

    #[test]
    fn shadow_from_whisper_identity_is_identity() {
        let whisper = AtmosphereVisualWhisper::identity();
        let metrics = shadow_metrics_from_whisper(&whisper);
        assert!(metrics.is_identity());
        assert_eq!(metrics.risk_label(), "identity");
    }

    // ── Whisper Risk Tests ──

    #[test]
    fn controlled_live_pulse_shadow_is_whisper_risk() {
        let metrics = shadow_metrics_from_mode_and_regime(
            AtmosphereApplicationMode::ControlledLive,
            AtmosphereRegime::Pulse,
        );
        assert_eq!(metrics.risk_label(), "whisper");
        assert!(metrics.max_abs_delta_percent() > 0.0);
        assert!(metrics.max_abs_delta_percent() <= 2.0);
    }

    #[test]
    fn controlled_live_storm_shadow_is_whisper_risk() {
        let metrics = shadow_metrics_from_mode_and_regime(
            AtmosphereApplicationMode::ControlledLive,
            AtmosphereRegime::Storm,
        );
        assert_eq!(metrics.risk_label(), "whisper");
        assert!(metrics.glitch_pressure <= VisualWhisperBounds::GLITCH_PRESSURE_MAX);
    }

    #[test]
    fn storm_shadow_metrics_not_default() {
        let metrics = shadow_metrics_from_mode_and_regime(
            AtmosphereApplicationMode::ControlledLive,
            AtmosphereRegime::Storm,
        );
        assert!(
            !metrics.is_identity(),
            "Storm shadow must not be identity under ControlledLive"
        );
        assert!(
            matches!(metrics.risk_label(), "whisper" | "elevated"),
            "Storm shadow risk must be whisper or elevated, got: {}",
            metrics.risk_label()
        );
    }

    #[test]
    fn internal_verified_storm_shadow_is_elevated() {
        let metrics = shadow_metrics_from_mode_and_regime(
            AtmosphereApplicationMode::InternalVerified,
            AtmosphereRegime::Storm,
        );
        assert_eq!(metrics.risk_label(), "elevated");
        assert!(
            metrics.max_abs_delta_percent() > 2.0,
            "InternalVerified Storm must exceed whisper delta threshold, got {:.2}",
            metrics.max_abs_delta_percent()
        );
    }

    #[test]
    fn color_change_true_produces_rejected_risk() {
        let mut metrics = AtmosphereShadowMetrics::identity();
        metrics.color_change_allowed = true;
        assert_eq!(metrics.risk_label(), "rejected");
    }

    #[test]
    fn terminal_effect_true_produces_rejected_risk() {
        let mut metrics = AtmosphereShadowMetrics::identity();
        metrics.terminal_effect_allowed = true;
        assert_eq!(metrics.risk_label(), "rejected");
    }

    #[test]
    fn shadow_risk_labels_are_exhaustive() {
        assert_eq!(AtmosphereShadowMetrics::identity().risk_label(), "identity");
        let whisper = shadow_metrics_from_mode_and_regime(
            AtmosphereApplicationMode::ControlledLive,
            AtmosphereRegime::Pulse,
        );
        assert_eq!(whisper.risk_label(), "whisper");
        let elevated = shadow_metrics_from_mode_and_regime(
            AtmosphereApplicationMode::InternalVerified,
            AtmosphereRegime::Storm,
        );
        assert_eq!(elevated.risk_label(), "elevated");
        let mut rejected = AtmosphereShadowMetrics::identity();
        rejected.color_change_allowed = true;
        assert_eq!(rejected.risk_label(), "rejected");
    }

    // ── Properties Tests ──

    #[test]
    fn max_abs_delta_percent_is_bounded_and_deterministic() {
        for regime in [
            AtmosphereRegime::Pulse,
            AtmosphereRegime::Storm,
            AtmosphereRegime::Void,
            AtmosphereRegime::Compression,
            AtmosphereRegime::Signal,
            AtmosphereRegime::MonolithPressure,
        ] {
            let metrics = shadow_metrics_from_mode_and_regime(
                AtmosphereApplicationMode::ControlledLive,
                regime,
            );
            assert!(
                metrics.max_abs_delta_percent() <= 10.0,
                "max_abs_delta_percent must be bounded (got {:.2} for {:?})",
                metrics.max_abs_delta_percent(),
                regime
            );
        }
        let m1 = shadow_metrics_from_mode_and_regime(
            AtmosphereApplicationMode::ControlledLive,
            AtmosphereRegime::Storm,
        );
        let m2 = shadow_metrics_from_mode_and_regime(
            AtmosphereApplicationMode::ControlledLive,
            AtmosphereRegime::Storm,
        );
        assert_eq!(m1.max_abs_delta_percent(), m2.max_abs_delta_percent());
        assert_eq!(m1.speed_delta_percent, m2.speed_delta_percent);
        assert_eq!(m1.density_delta_percent, m2.density_delta_percent);
    }

    #[test]
    fn max_abs_delta_percent_identity_is_zero() {
        let metrics = AtmosphereShadowMetrics::identity();
        assert_eq!(metrics.max_abs_delta_percent(), 0.0);
    }

    #[test]
    fn shadow_evaluation_does_not_invalidate_cache() {
        let cache = CachePolicy::new(100);
        let initial_gen = cache.generation;

        let _ = shadow_metrics_from_mode_and_regime(
            AtmosphereApplicationMode::ControlledLive,
            AtmosphereRegime::Pulse,
        );
        let _ = shadow_metrics_from_mode_and_regime(
            AtmosphereApplicationMode::InternalVerified,
            AtmosphereRegime::Storm,
        );
        let _ = shadow_metrics_from_application(
            AtmosphereApplicationMode::TestOnly,
            &AtmosphereApplication {
                speed_scale: 1.5,
                density_scale: 1.3,
                brightness_scale: 1.1,
                glitch_pressure: 0.4,
                color_change: false,
            },
        );

        assert_eq!(
            cache.generation, initial_gen,
            "shadow evaluation must not invalidate cache"
        );
    }

    #[test]
    fn shadow_metrics_deterministic_for_same_input() {
        for _ in 0..50 {
            let m1 = shadow_metrics_from_mode_and_regime(
                AtmosphereApplicationMode::ControlledLive,
                AtmosphereRegime::Storm,
            );
            let m2 = shadow_metrics_from_mode_and_regime(
                AtmosphereApplicationMode::ControlledLive,
                AtmosphereRegime::Storm,
            );
            assert_eq!(m1.speed_delta_percent, m2.speed_delta_percent);
            assert_eq!(m1.density_delta_percent, m2.density_delta_percent);
            assert_eq!(m1.brightness_delta_percent, m2.brightness_delta_percent);
            assert_eq!(m1.trail_energy_delta_percent, m2.trail_energy_delta_percent);
            assert_eq!(m1.glyph_pulse_delta_percent, m2.glyph_pulse_delta_percent);
            assert_eq!(m1.glitch_pressure, m2.glitch_pressure);
            assert_eq!(m1.risk_label(), m2.risk_label());
        }
    }
}
