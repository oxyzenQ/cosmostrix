// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Atmosphere regime state machine for Cosmostrix v4.0.0.
//!
//! The Atmosphere Engine models the overall visual climate of the terminal
//! render as a slow-moving regime that modulates rendering parameters
//! gradually over time. Phase 3 adds a verifier layer and controlled
//! internal application path while keeping Calm as the visual identity.
//!
//! ## Phase 6 Scope (Controlled Live Modulation)
//!
//! - `params_for_regime()`: maps each regime to specific bounded parameters.
//!   Calm returns identity. Non-Calm regimes return subtle, conservative
//!   modulation values.
//! - `effective_params()` now returns regime-specific params instead of
//!   always returning Calm identity.
//! - The ControlledLive application mode (atmosphere_apply.rs) gates
//!   whether regime modulation is applied to the renderer.
//! - Default behavior remains Disabled/identity — no visual change from v3.9.0.
//!
//! ## Phase 3 Scope
//!
//! - `build_application()`: converts current regime state into a verified
//!   AtmosphereApplication through the verifier layer.
//! - Calm application is always identity (no visual change from v3.9.0).
//! - Non-Calm applications are computed and verified but not applied to
//!   the renderer in Phase 3.
//! - Verifier rejects or clamps unsafe values deterministically.
//!
//! The renderer continues to behave exactly as v3.9.0 — Calm is the default
//! and only active regime. Non-Calm regimes are verified but not unleashed.

// Phase 3: Module-level dead_code allow is required because many types
// (AtmosphereState, AtmosphereController, RegimeProbe, build_application)
// are pub(crate) API contracts consumed in tests, diagnostics, and future
// integration points — not yet wired into the hot render path. As
// integration progresses, individual allows can replace this module-level one.
#![allow(dead_code)]

use crate::zactrix_cache::{CachePolicy, InvalidationEvent};

// ── Regime Enum ────────────────────────────────────────────────────────────

/// Visual regime for the Atmosphere Engine.
///
/// Each regime defines a bounded set of rendering parameter multipliers.
/// Regime transitions must be gradual and bounded. Color drift remains
/// opt-in only (`auto_color_drift = false` by default).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AtmosphereRegime {
    /// Default resting state. Closest to v3.9.0 behavior.
    Calm,
    /// Gradually increasing density and speed.
    Compression,
    /// Periodic intensity waves. Regular, bounded oscillation.
    Pulse,
    /// High activity, bounded. Faster streams, more glitches.
    Storm,
    /// Minimal activity. Sparse streams, slow speed.
    Void,
    /// Focused, directional. Streams converge.
    Signal,
    /// Enhanced Monolith Rain presence.
    MonolithPressure,
}

impl AtmosphereRegime {
    /// Human-readable label for diagnostics and logs.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Calm => "calm",
            Self::Compression => "compression",
            Self::Pulse => "pulse",
            Self::Storm => "storm",
            Self::Void => "void",
            Self::Signal => "signal",
            Self::MonolithPressure => "monolith-pressure",
        }
    }

    /// Total number of defined regimes.
    pub(crate) const COUNT: usize = 7;
}

// ── Regime Params ─────────────────────────────────────────────────────────

/// Bounded parameter multipliers for a regime.
///
/// All values are clamped to safe ranges. These define how a regime
/// modulates rendering parameters. In Phase 2, only Calm defaults are active.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RegimeParams {
    /// Speed multiplier (0.5 .. 2.0).
    pub speed_mult: f32,
    /// Density multiplier (0.5 .. 1.5).
    pub density_mult: f32,
    /// Glitch probability multiplier (0.0 .. 2.0).
    pub glitch_mult: f32,
    /// Brightness bias (-0.1 .. +0.1).
    pub brightness_bias: f32,
}

impl RegimeParams {
    /// Default params for Calm regime: no modulation (identity transform).
    pub(crate) const fn calm() -> Self {
        Self {
            speed_mult: 1.0,
            density_mult: 1.0,
            glitch_mult: 1.0,
            brightness_bias: 0.0,
        }
    }

    /// Clamp all parameters to their safe ranges.
    pub(crate) fn clamp(&mut self) {
        self.speed_mult = self.speed_mult.clamp(0.5, 2.0);
        self.density_mult = self.density_mult.clamp(0.5, 1.5);
        self.glitch_mult = self.glitch_mult.clamp(0.0, 2.0);
        self.brightness_bias = self.brightness_bias.clamp(-0.1, 0.1);
    }

    /// Create a clamped copy.
    pub(crate) fn clamped(&self) -> Self {
        let mut copy = *self;
        copy.clamp();
        copy
    }
}

// ── Regime-to-Parameter Mapping (Phase 6) ──────────────────────────────

/// Map a regime to its specific bounded rendering parameters.
///
/// Each regime defines subtle, conservative modulation that preserves the
/// v3.9.0 visual identity while allowing controlled atmospheric variation.
/// All values are within the RegimeParams safe ranges (no clamping needed).
///
/// Calm returns identity. Non-Calm regimes return tiny deviations:
/// - Pulse: tiny speed/brightness lift, no density spike
/// - Compression: tiny density tightening, low brightness change
/// - Void: tiny density/speed reduction, no brightness change
/// - Storm: verified but heavily clamped speed/density/glitch
/// - Signal: tiny structured speed/brightness change
/// - MonolithPressure: tiny depth/brightness pressure, low glitch
pub(crate) const fn params_for_regime(regime: AtmosphereRegime) -> RegimeParams {
    match regime {
        AtmosphereRegime::Calm => RegimeParams::calm(),
        AtmosphereRegime::Pulse => RegimeParams {
            speed_mult: 1.04,
            density_mult: 1.0, // no density spike
            glitch_mult: 1.0,
            brightness_bias: 0.02,
        },
        AtmosphereRegime::Compression => RegimeParams {
            speed_mult: 1.0,
            density_mult: 1.04, // tiny density tightening
            glitch_mult: 1.0,
            brightness_bias: 0.01,
        },
        AtmosphereRegime::Storm => RegimeParams {
            speed_mult: 1.08,   // heavily clamped
            density_mult: 1.06, // heavily clamped
            glitch_mult: 1.2,   // elevated but bounded
            brightness_bias: 0.03,
        },
        AtmosphereRegime::Void => RegimeParams {
            speed_mult: 0.97,     // slow down
            density_mult: 0.96,   // sparse
            glitch_mult: 0.6,     // reduced
            brightness_bias: 0.0, // no brightness change
        },
        AtmosphereRegime::Signal => RegimeParams {
            speed_mult: 1.03, // tiny structured lift
            density_mult: 1.0,
            glitch_mult: 1.0,
            brightness_bias: 0.01,
        },
        AtmosphereRegime::MonolithPressure => RegimeParams {
            speed_mult: 0.98,      // subtle slow
            density_mult: 1.0,     // no density change
            glitch_mult: 0.8,      // reduced glitch
            brightness_bias: 0.02, // subtle depth
        },
    }
}

/// Minimum dwell time in seconds between regime transitions.
pub(crate) const REGIME_MIN_DWELL_SECS: f64 = 5.0;

/// Minimum transition ramp duration in seconds.
pub(crate) const REGIME_TRANSITION_RAMP_SECS: f64 = 1.0;

// ── Atmosphere State ───────────────────────────────────────────────────────

/// Internal regime state for the Atmosphere Engine.
///
/// Tracks current regime, target regime, transition progress, and timing.
/// In Phase 2, this state is maintained internally but does not affect
/// rendering. The state is deterministic and testable.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AtmosphereState {
    /// Currently active regime. Always Calm in Phase 2 unless test-invoked.
    pub current_regime: AtmosphereRegime,
    /// Target regime for transition. Equal to current when stable.
    pub target_regime: AtmosphereRegime,
    /// Transition progress (0.0 = at current, 1.0 = at target).
    /// Always 0.0 when current == target (stable).
    pub transition_progress: f32,
    /// Simulated elapsed seconds since last regime evaluation.
    pub last_update_secs: f64,
}

impl AtmosphereState {
    /// Create the default atmosphere state: Calm, stable, no transition.
    pub(crate) const fn default() -> Self {
        Self {
            current_regime: AtmosphereRegime::Calm,
            target_regime: AtmosphereRegime::Calm,
            transition_progress: 0.0,
            last_update_secs: 0.0,
        }
    }

    /// Whether the state is stable (no transition in progress).
    pub(crate) fn is_stable(&self) -> bool {
        self.current_regime == self.target_regime && self.transition_progress == 0.0
    }

    /// Whether the effective regime is Calm (near no-op for visuals).
    pub(crate) fn is_calm(&self) -> bool {
        self.current_regime == AtmosphereRegime::Calm
    }

    /// Effective rendering parameters for the current state.
    ///
    /// In Phase 6, returns regime-specific params. For Calm, returns
    /// identity params. For non-Calm regimes, returns subtle bounded
    /// modulation values. These values are further verified by the
    /// verifier and clamped by the application adapter.
    pub(crate) fn effective_params(&self) -> RegimeParams {
        params_for_regime(self.current_regime)
    }

    /// Build a verified atmosphere application from the current state.
    ///
    /// Converts the current regime's parameters into an AtmosphereApplication
    /// through the verifier layer. For Calm (the default), this returns
    /// identity (no-op). Non-Calm regimes produce modulated applications
    /// that are verified but not applied to the renderer in Phase 3.
    pub(crate) fn build_application(&self) -> crate::atmosphere_verifier::AtmosphereApplication {
        let params = self.effective_params();
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
        app
    }
}

// ── Atmosphere Controller ──────────────────────────────────────────────────

/// Manages regime transitions with dwell-time and ramp enforcement.
///
/// The controller ensures:
/// - Minimum dwell time between transitions (REGIME_MIN_DWELL_SECS).
/// - Transition progress bounded between 0.0 and 1.0.
/// - Calm-to-Calm is always a no-op.
/// - Regime changes invalidate the Zactrix Cache generation.
///
/// In Phase 2, the controller is available for internal use but the
/// actual regime is never changed from Calm in production code paths.
#[derive(Debug)]
pub(crate) struct AtmosphereController {
    /// The atmosphere state being managed.
    state: AtmosphereState,
    /// Simulated elapsed time since last regime change (for dwell enforcement).
    time_since_last_change: f64,
}

impl AtmosphereController {
    /// Create a new controller with default Calm state.
    pub(crate) fn new() -> Self {
        Self {
            state: AtmosphereState::default(),
            time_since_last_change: 0.0,
        }
    }

    /// Get a reference to the current atmosphere state.
    pub(crate) const fn state(&self) -> &AtmosphereState {
        &self.state
    }

    /// Get the current regime.
    pub(crate) const fn current_regime(&self) -> AtmosphereRegime {
        self.state.current_regime
    }

    /// Whether the state is stable (no transition in progress).
    pub(crate) fn is_stable(&self) -> bool {
        self.state.is_stable()
    }

    /// Transition progress as a diagnostic string.
    pub(crate) fn transition_status(&self) -> &'static str {
        if self.state.is_stable() {
            "stable"
        } else {
            "transitioning"
        }
    }

    /// Whether the effective regime is a visual no-op (Calm).
    pub(crate) fn is_effective_noop(&self) -> bool {
        self.state.is_calm()
    }

    /// Build a verified atmosphere application for the current state.
    ///
    /// Delegates to AtmosphereState::build_application after computing
    /// the current effective parameters.
    pub(crate) fn build_application(&self) -> crate::atmosphere_verifier::AtmosphereApplication {
        self.state.build_application()
    }

    /// Evaluate probe and compute candidate regime without applying it.
    ///
    /// This is the Phase 2 safe path: probe facts are observed, a candidate
    /// is computed, but the actual regime remains Calm. Returns the candidate
    /// for diagnostic reporting only.
    pub(crate) fn evaluate_probe(
        &self,
        probe: &crate::atmosphere_probe::RegimeProbe,
    ) -> AtmosphereRegime {
        crate::atmosphere_probe::select_regime_from_probe(probe)
    }

    /// Advance the controller's internal clock by the given delta seconds.
    ///
    /// In Phase 2 this is a no-op because no transitions are in progress.
    /// Future phases will use this to drive transition ramp progress.
    pub(crate) fn advance(&mut self, delta_secs: f64) {
        self.time_since_last_change += delta_secs;
        self.state.last_update_secs += delta_secs;

        // If a transition is in progress, advance progress.
        if self.state.current_regime != self.state.target_regime {
            let ramp_duration = REGIME_TRANSITION_RAMP_SECS.max(0.1);
            let increment = (delta_secs / ramp_duration) as f32;
            self.state.transition_progress = (self.state.transition_progress + increment).min(1.0);

            // Transition complete: snap to target.
            if self.state.transition_progress >= 1.0 {
                self.state.current_regime = self.state.target_regime;
                self.state.transition_progress = 0.0;
            }
        }
    }

    /// Transition to a target regime, enforcing dwell-time constraints.
    ///
    /// Returns `true` if the transition was accepted, `false` if rejected
    /// due to dwell-time constraint or same-regime no-op.
    ///
    /// When accepted, invalidates the given cache policy with
    /// AtmosphereRegimeChange.
    pub(crate) fn transition_to(
        &mut self,
        target: AtmosphereRegime,
        cache: &mut CachePolicy,
    ) -> bool {
        // Calm-to-Calm is always a no-op.
        if target == self.state.current_regime && self.state.target_regime == target {
            return false;
        }

        // Enforce minimum dwell time.
        if self.time_since_last_change < REGIME_MIN_DWELL_SECS {
            return false;
        }

        self.state.target_regime = target;
        self.state.transition_progress = 0.0;
        self.time_since_last_change = 0.0;

        // Invalidate cache generation for atmosphere regime change.
        cache.invalidate(InvalidationEvent::AtmosphereRegimeChange);

        true
    }

    /// Force an immediate transition, bypassing dwell-time (for tests only).
    ///
    /// Returns `true` if the transition changed the regime.
    #[cfg(test)]
    pub(crate) fn force_transition_to(&mut self, target: AtmosphereRegime) -> bool {
        if target == self.state.current_regime {
            return false;
        }
        self.state.target_regime = target;
        self.state.transition_progress = 1.0;
        self.state.current_regime = target;
        self.state.transition_progress = 0.0;
        self.time_since_last_change = 0.0;
        true
    }
}

impl Default for AtmosphereController {
    fn default() -> Self {
        Self::new()
    }
}
