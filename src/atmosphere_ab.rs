// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Internal A/B smoke model for Atmosphere Visual Whisper (Phase 9).
//!
//! Provides deterministic, test-only comparisons between the baseline identity
//! visual path and controlled whisper behavior. The A/B smoke layer proves that
//! the whisper path is bounded, clean, and safe before any public activation.
//!
//! ## Phase 9 Scope — Internal Atmosphere Visual A/B Smoke
//!
//! This module adds an internal test-only A/B smoke validation layer:
//!
//! - **Baseline vs Candidate**: each smoke sample captures both the identity
//!   baseline (Calm/Disabled) and a controlled candidate (whisper from a
//!   specific regime under ControlledLive mode).
//! - **Verdict system**: each comparison produces a pass/reject verdict with
//!   a human-readable reason.
//! - **Pure deterministic functions**: no side effects, no cache invalidation,
//!   no terminal state changes, no color mutations.
//! - **Test-only**: the A/B smoke functions are only called from tests.
//!   No public CLI flag, no config key, no runtime default change.
//!
//! ## Safety Checks
//!
//! The A/B smoke verifies:
//! - No color change is allowed in the candidate.
//! - No terminal effect is allowed in the candidate.
//! - Density does not collapse (remains >= 0.98).
//! - Brightness does not spike (remains <= 1.015).
//! - Glitch pressure stays at or below the whisper cap (0.05).
//! - Maximum absolute delta percent remains within whisper bounds.
//! - Candidate risk is identity or whisper for normal regimes.
//! - Default production mode remains disabled/identity.
//!
//! ## What Phase 9 Does NOT Do
//!
//! - Does NOT change default visual output — still identical to v3.9.0.
//! - Does NOT expose public atmosphere controls.
//! - Does NOT add CLI flags, config keys, or benchmark field renames.
//! - Does NOT alter runtime default behavior.

use crate::atmosphere::AtmosphereRegime;
use crate::atmosphere_apply::AtmosphereApplicationMode;
use crate::atmosphere_shadow::{
    shadow_metrics_from_mode_and_regime, shadow_metrics_from_whisper, AtmosphereShadowMetrics,
};
use crate::atmosphere_visual::{visual_whisper_from_regime, AtmosphereVisualWhisper};

// ── A/B Sample ──────────────────────────────────────────────────────────────

/// A single A/B smoke sample comparing baseline identity against a candidate.
///
/// Captures both the whisper values and the shadow metrics for the baseline
/// (Calm/Disabled identity) and the candidate (regime under ControlledLive).
/// All delta fields measure the difference between candidate and baseline.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AtmosphereAbSample {
    /// Baseline visual whisper (always identity).
    pub baseline_whisper: AtmosphereVisualWhisper,
    /// Candidate visual whisper (regime under ControlledLive).
    pub candidate_whisper: AtmosphereVisualWhisper,
    /// Baseline shadow metrics (always identity).
    pub baseline_shadow: AtmosphereShadowMetrics,
    /// Candidate shadow metrics (regime under ControlledLive).
    pub candidate_shadow: AtmosphereShadowMetrics,
    /// Speed delta as a percentage (candidate - baseline).
    pub speed_delta_percent: f32,
    /// Density delta as a percentage.
    pub density_delta_percent: f32,
    /// Brightness delta as a percentage.
    pub brightness_delta_percent: f32,
    /// Trail energy delta as a percentage.
    pub trail_energy_delta_percent: f32,
    /// Glyph pulse delta as a percentage.
    pub glyph_pulse_delta_percent: f32,
    /// Glitch pressure delta (candidate - baseline).
    pub glitch_delta: f32,
    /// Risk label for the candidate shadow metrics.
    pub risk_label: &'static str,
    /// Whether the A/B comparison passed.
    pub passed: bool,
}

// ── A/B Verdict ─────────────────────────────────────────────────────────────

/// Outcome of an A/B smoke comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AtmosphereAbVerdict {
    /// Whether the comparison passed all safety checks.
    pub pass: bool,
    /// Human-readable reason for the verdict.
    pub reason: &'static str,
}

impl AtmosphereAbVerdict {
    /// Create a passing verdict with a reason.
    pub(crate) const fn pass(reason: &'static str) -> Self {
        Self { pass: true, reason }
    }

    /// Create a failing verdict with a reason.
    pub(crate) const fn reject(reason: &'static str) -> Self {
        Self {
            pass: false,
            reason,
        }
    }
}

// ── A/B Safety Thresholds ─────────────────────────────────────────────────

/// Internal thresholds for A/B smoke safety checks.
pub(crate) struct AbSafetyThresholds;

impl AbSafetyThresholds {
    /// Maximum allowed brightness scale factor (1.5% above identity).
    pub(crate) const MAX_BRIGHTNESS_SCALE: f32 = 1.015;
    /// Minimum allowed density scale factor (2% below identity).
    pub(crate) const MIN_DENSITY_SCALE: f32 = 0.98;
    /// Maximum glitch pressure allowed in A/B candidate (whisper cap).
    pub(crate) const MAX_GLITCH_PRESSURE: f32 = 0.05;
    /// Maximum absolute delta percent across all dimensions.
    pub(crate) const MAX_DELTA_PERCENT: f32 = 2.0;
}

// ── A/B Comparison Functions ───────────────────────────────────────────────

/// Compare identity baseline against a specific regime under ControlledLive.
///
/// Builds both the baseline (identity/Calm) and candidate (regime) whisper and
/// shadow metrics, computes deltas, and returns the A/B sample with a pass/reject
/// verdict based on safety thresholds.
#[must_use]
pub(crate) fn compare_identity_vs_regime(regime: AtmosphereRegime) -> AtmosphereAbSample {
    let baseline_whisper = AtmosphereVisualWhisper::identity();
    let baseline_shadow = AtmosphereShadowMetrics::identity();

    let candidate_whisper = visual_whisper_from_regime(regime);
    let candidate_shadow =
        shadow_metrics_from_mode_and_regime(AtmosphereApplicationMode::ControlledLive, regime);

    let speed_delta = candidate_shadow.speed_delta_percent - baseline_shadow.speed_delta_percent;
    let density_delta =
        candidate_shadow.density_delta_percent - baseline_shadow.density_delta_percent;
    let brightness_delta =
        candidate_shadow.brightness_delta_percent - baseline_shadow.brightness_delta_percent;
    let trail_delta =
        candidate_shadow.trail_energy_delta_percent - baseline_shadow.trail_energy_delta_percent;
    let glyph_delta =
        candidate_shadow.glyph_pulse_delta_percent - baseline_shadow.glyph_pulse_delta_percent;
    let glitch_delta = candidate_shadow.glitch_pressure - baseline_shadow.glitch_pressure;

    let risk_label = candidate_shadow.risk_label();

    let passed = evaluate_ab_safety(regime, &candidate_whisper, &candidate_shadow, risk_label);

    AtmosphereAbSample {
        baseline_whisper,
        candidate_whisper,
        baseline_shadow,
        candidate_shadow,
        speed_delta_percent: speed_delta,
        density_delta_percent: density_delta,
        brightness_delta_percent: brightness_delta,
        trail_energy_delta_percent: trail_delta,
        glyph_pulse_delta_percent: glyph_delta,
        glitch_delta,
        risk_label,
        passed,
    }
}

/// Compare identity baseline against an arbitrary candidate whisper.
///
/// This is a lower-level A/B function that takes a pre-built whisper and
/// evaluates it against the identity baseline.
#[must_use]
pub(crate) fn compare_identity_vs_whisper(whisper: AtmosphereVisualWhisper) -> AtmosphereAbSample {
    let baseline_whisper = AtmosphereVisualWhisper::identity();
    let baseline_shadow = AtmosphereShadowMetrics::identity();

    let candidate_shadow = shadow_metrics_from_whisper(&whisper);

    let speed_delta = candidate_shadow.speed_delta_percent - baseline_shadow.speed_delta_percent;
    let density_delta =
        candidate_shadow.density_delta_percent - baseline_shadow.density_delta_percent;
    let brightness_delta =
        candidate_shadow.brightness_delta_percent - baseline_shadow.brightness_delta_percent;
    let trail_delta =
        candidate_shadow.trail_energy_delta_percent - baseline_shadow.trail_energy_delta_percent;
    let glyph_delta =
        candidate_shadow.glyph_pulse_delta_percent - baseline_shadow.glyph_pulse_delta_percent;
    let glitch_delta = candidate_shadow.glitch_pressure - baseline_shadow.glitch_pressure;

    let risk_label = candidate_shadow.risk_label();

    let passed = !whisper.color_change_allowed
        && !whisper.terminal_effect_allowed
        && whisper.density_scale >= AbSafetyThresholds::MIN_DENSITY_SCALE
        && whisper.brightness_scale <= AbSafetyThresholds::MAX_BRIGHTNESS_SCALE
        && whisper.glitch_pressure <= AbSafetyThresholds::MAX_GLITCH_PRESSURE
        && candidate_shadow.max_abs_delta_percent() <= AbSafetyThresholds::MAX_DELTA_PERCENT;

    AtmosphereAbSample {
        baseline_whisper,
        candidate_whisper: whisper,
        baseline_shadow,
        candidate_shadow,
        speed_delta_percent: speed_delta,
        density_delta_percent: density_delta,
        brightness_delta_percent: brightness_delta,
        trail_energy_delta_percent: trail_delta,
        glyph_pulse_delta_percent: glyph_delta,
        glitch_delta,
        risk_label,
        passed,
    }
}

/// Run A/B smoke for a single regime under ControlledLive mode.
///
/// Returns both the sample and a structured verdict.
#[must_use]
pub(crate) fn smoke_regime_under_controlled_live(
    regime: AtmosphereRegime,
) -> (AtmosphereAbSample, AtmosphereAbVerdict) {
    let sample = compare_identity_vs_regime(regime);

    if regime == AtmosphereRegime::Calm {
        if sample.candidate_whisper.is_identity() && sample.risk_label == "identity" {
            return (sample, AtmosphereAbVerdict::pass("calm is identity"));
        }
        return (sample, AtmosphereAbVerdict::reject("calm must be identity"));
    }

    if sample.candidate_whisper.color_change_allowed {
        return (
            sample,
            AtmosphereAbVerdict::reject("color_change_allowed must be false"),
        );
    }

    if sample.candidate_whisper.terminal_effect_allowed {
        return (
            sample,
            AtmosphereAbVerdict::reject("terminal_effect_allowed must be false"),
        );
    }

    if sample.candidate_whisper.density_scale < AbSafetyThresholds::MIN_DENSITY_SCALE {
        return (
            sample,
            AtmosphereAbVerdict::reject("density collapsed below safe threshold"),
        );
    }

    if sample.candidate_whisper.brightness_scale > AbSafetyThresholds::MAX_BRIGHTNESS_SCALE {
        return (
            sample,
            AtmosphereAbVerdict::reject("brightness exceeded safe threshold"),
        );
    }

    if sample.candidate_whisper.glitch_pressure > AbSafetyThresholds::MAX_GLITCH_PRESSURE {
        return (
            sample,
            AtmosphereAbVerdict::reject("glitch pressure above whisper cap"),
        );
    }

    if sample.candidate_shadow.max_abs_delta_percent() > AbSafetyThresholds::MAX_DELTA_PERCENT {
        return (
            sample,
            AtmosphereAbVerdict::reject("max delta percent exceeds whisper bounds"),
        );
    }

    if matches!(sample.risk_label, "identity" | "whisper") {
        return (
            sample,
            AtmosphereAbVerdict::pass("candidate within whisper risk bounds"),
        );
    }

    if regime == AtmosphereRegime::Storm && sample.risk_label == "elevated" {
        if sample.candidate_whisper.is_within_whisper_bounds() {
            return (
                sample,
                AtmosphereAbVerdict::pass(
                    "storm clamped to whisper bounds (elevated shadow but safe whisper)",
                ),
            );
        }
        return (
            sample,
            AtmosphereAbVerdict::reject("storm elevated and not within whisper bounds"),
        );
    }

    (sample, AtmosphereAbVerdict::reject("unexpected risk label"))
}

/// Run A/B smoke for all regimes under ControlledLive mode.
///
/// Returns a vector of (regime, sample, verdict) tuples.
#[must_use]
pub(crate) fn smoke_all_regimes_under_controlled_live(
) -> Vec<(AtmosphereRegime, AtmosphereAbSample, AtmosphereAbVerdict)> {
    let regimes = [
        AtmosphereRegime::Calm,
        AtmosphereRegime::Compression,
        AtmosphereRegime::Pulse,
        AtmosphereRegime::Storm,
        AtmosphereRegime::Void,
        AtmosphereRegime::Signal,
        AtmosphereRegime::MonolithPressure,
        AtmosphereRegime::Adaptive,
    ];

    regimes
        .into_iter()
        .map(|regime| {
            let (sample, verdict) = smoke_regime_under_controlled_live(regime);
            (regime, sample, verdict)
        })
        .collect()
}

// ── Internal Safety Evaluation ─────────────────────────────────────────────

/// Evaluate whether a candidate passes all A/B safety checks.
fn evaluate_ab_safety(
    regime: AtmosphereRegime,
    candidate_whisper: &AtmosphereVisualWhisper,
    candidate_shadow: &AtmosphereShadowMetrics,
    risk_label: &str,
) -> bool {
    if regime == AtmosphereRegime::Calm {
        return candidate_whisper.is_identity() && risk_label == "identity";
    }

    if candidate_whisper.color_change_allowed {
        return false;
    }

    if candidate_whisper.terminal_effect_allowed {
        return false;
    }

    if candidate_whisper.density_scale < AbSafetyThresholds::MIN_DENSITY_SCALE {
        return false;
    }

    if candidate_whisper.brightness_scale > AbSafetyThresholds::MAX_BRIGHTNESS_SCALE {
        return false;
    }

    if candidate_whisper.glitch_pressure > AbSafetyThresholds::MAX_GLITCH_PRESSURE {
        return false;
    }

    if candidate_shadow.max_abs_delta_percent() > AbSafetyThresholds::MAX_DELTA_PERCENT {
        return false;
    }

    matches!(risk_label, "identity" | "whisper")
}
