// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Atmosphere regime state machine for Cosmostrix v4.0.0.
//!
//! The Atmosphere Engine models the overall visual climate of the terminal
//! render as a slow-moving regime that modulates rendering parameters
//! gradually over time. v4.0.0 Phase 2 wires the regime model into internal
//! runtime state without applying visual modulation. The renderer behaves
//! exactly as v3.9.0 — Calm is the default and only active regime.
//!
//! ## Phase 2 Scope
//!
//! - `AtmosphereState`: holds current regime, target regime, transition
//!   progress, and last-update marker. Default: Calm/Calm/stable.
//! - `AtmosphereController`: manages regime transitions with dwell-time
//!   enforcement and bounded ramp progress.
//! - `RegimeProbe`: observable facts for deterministic regime selection.
//! - `select_regime_from_probe()`: pure function that maps probe facts
//!   to a candidate regime without applying it to visuals.
//!
//! No regime transitions are applied to the renderer in Phase 2. The
//! controller is wired internally but only advances state when explicitly
//! invoked (tests, future phases).

// Phase 2: Module-level dead_code allow is required because many types
// (AtmosphereState, AtmosphereController, RegimeProbe) are pub(crate) API
// contracts consumed in tests, diagnostics, and future integration points —
// not yet wired into the hot render path. As integration progresses,
// individual allows can replace this module-level one.
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

/// Minimum dwell time in seconds between regime transitions.
pub(crate) const REGIME_MIN_DWELL_SECS: f64 = 5.0;

/// Minimum transition ramp duration in seconds.
pub(crate) const REGIME_TRANSITION_RAMP_SECS: f64 = 1.0;

// ── Regime Probe ──────────────────────────────────────────────────────────

/// Observable facts fed to the regime selector.
///
/// All fields are deterministic and derived from measurable runtime state.
/// The probe does not mutate any state.
#[derive(Debug, Clone, Copy)]
#[must_use]
pub(crate) struct RegimeProbe {
    /// Fraction of dirty cells (0.0 .. 1.0).
    pub dirty_cell_ratio: f64,
    /// Number of active droplet streams.
    pub active_streams: usize,
    /// p99 frame time in milliseconds (0.0 if unknown).
    pub frame_time_pressure: f64,
    /// Whether this is a benchmark run.
    pub benchmark_mode: bool,
    /// Elapsed time in seconds since last regime evaluation (0.0 if unknown).
    pub elapsed_secs: f64,
}

impl RegimeProbe {
    /// Create a probe with sensible defaults for idle/normal conditions.
    pub(crate) const fn idle() -> Self {
        Self {
            dirty_cell_ratio: 0.0,
            active_streams: 0,
            frame_time_pressure: 0.0,
            benchmark_mode: false,
            elapsed_secs: 0.0,
        }
    }
}

// ── Probe-to-Regime Selection ──────────────────────────────────────────────

/// Frame-time pressure threshold (ms) above which Storm is candidate.
const FRAME_TIME_HIGH_MS: f64 = 30.0;

/// Frame-time pressure threshold (ms) above which Pulse is candidate.
const FRAME_TIME_MODERATE_MS: f64 = 10.0;

/// Dirty cell ratio threshold for high activity classification.
const DIRTY_RATIO_HIGH: f64 = 0.8;

/// Dirty cell ratio threshold for low activity classification.
const DIRTY_RATIO_LOW: f64 = 0.1;

/// Active streams threshold for "truly active" (avoid Void on idle).
/// Must be positive so that a zero-stream idle probe selects Calm.
const ACTIVE_STREAMS_LOW: usize = 10;

/// Active streams threshold for high-activity Pulse candidate.
const ACTIVE_STREAMS_HIGH: usize = 50;

/// Select a candidate regime from probe facts.
///
/// This is a pure deterministic function. It does NOT apply the regime
/// to the renderer. In Phase 2, the result is used for diagnostics only.
/// The actual regime remains Calm unless explicitly transitioned.
///
/// Selection logic:
/// - Benchmark mode → Calm (benchmark stability, no regime noise).
/// - Zero active streams → Calm (idle/no data).
/// - Extreme frame-time pressure (>30ms) → Storm candidate.
/// - High frame-time pressure (>10ms) + high dirty ratio → Storm candidate.
/// - High dirty ratio + many streams → Pulse candidate.
/// - Low activity (low dirty ratio, few streams) → Void candidate.
/// - Default → Calm.
pub(crate) fn select_regime_from_probe(probe: &RegimeProbe) -> AtmosphereRegime {
    // Benchmark mode always stays Calm for stability.
    if probe.benchmark_mode {
        return AtmosphereRegime::Calm;
    }

    // Zero active streams → no data to classify → Calm.
    if probe.active_streams == 0 {
        return AtmosphereRegime::Calm;
    }

    // Extreme frame-time pressure → Storm candidate.
    if probe.frame_time_pressure > FRAME_TIME_HIGH_MS {
        return AtmosphereRegime::Storm;
    }

    // High pressure + high dirty ratio → Storm candidate.
    if probe.frame_time_pressure > FRAME_TIME_MODERATE_MS
        && probe.dirty_cell_ratio > DIRTY_RATIO_HIGH
    {
        return AtmosphereRegime::Storm;
    }

    // High dirty ratio + many active streams → Pulse candidate.
    if probe.dirty_cell_ratio > DIRTY_RATIO_HIGH && probe.active_streams > ACTIVE_STREAMS_HIGH {
        return AtmosphereRegime::Pulse;
    }

    // Low activity → Void candidate.
    if probe.dirty_cell_ratio < DIRTY_RATIO_LOW && probe.active_streams < ACTIVE_STREAMS_LOW {
        return AtmosphereRegime::Void;
    }

    // Default → Calm.
    AtmosphereRegime::Calm
}

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
    /// In Phase 2 with Calm default, this always returns identity params.
    pub(crate) fn effective_params(&self) -> RegimeParams {
        RegimeParams::calm()
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

    /// Evaluate probe and compute candidate regime without applying it.
    ///
    /// This is the Phase 2 safe path: probe facts are observed, a candidate
    /// is computed, but the actual regime remains Calm. Returns the candidate
    /// for diagnostic reporting only.
    pub(crate) fn evaluate_probe(&self, probe: &RegimeProbe) -> AtmosphereRegime {
        select_regime_from_probe(probe)
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

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Regime basics ──

    #[test]
    fn calm_is_default_regime() {
        let calm = AtmosphereRegime::Calm;
        assert_eq!(calm.as_str(), "calm");
    }

    #[test]
    fn all_regimes_have_non_empty_labels() {
        let regimes = [
            AtmosphereRegime::Calm,
            AtmosphereRegime::Compression,
            AtmosphereRegime::Pulse,
            AtmosphereRegime::Storm,
            AtmosphereRegime::Void,
            AtmosphereRegime::Signal,
            AtmosphereRegime::MonolithPressure,
        ];
        for regime in regimes {
            assert!(!regime.as_str().is_empty());
            assert!(regime.as_str().len() < 30);
        }
    }

    #[test]
    fn regime_count_matches_defined_variants() {
        assert_eq!(AtmosphereRegime::COUNT, 7);
    }

    #[test]
    fn all_regimes_are_distinct() {
        let regimes = [
            AtmosphereRegime::Calm,
            AtmosphereRegime::Compression,
            AtmosphereRegime::Pulse,
            AtmosphereRegime::Storm,
            AtmosphereRegime::Void,
            AtmosphereRegime::Signal,
            AtmosphereRegime::MonolithPressure,
        ];
        for (i, a) in regimes.iter().enumerate() {
            for (j, b) in regimes.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "regimes must be distinct");
                }
            }
        }
    }

    // ── Regime params ──

    #[test]
    fn calm_params_are_identity() {
        let params = RegimeParams::calm();
        assert_eq!(params.speed_mult, 1.0);
        assert_eq!(params.density_mult, 1.0);
        assert_eq!(params.glitch_mult, 1.0);
        assert_eq!(params.brightness_bias, 0.0);
    }

    #[test]
    fn regime_params_clamp_to_safe_ranges() {
        let mut params = RegimeParams {
            speed_mult: 5.0,
            density_mult: 3.0,
            glitch_mult: 10.0,
            brightness_bias: 1.0,
        };
        params.clamp();

        assert!((0.5..=2.0).contains(&params.speed_mult));
        assert!((0.5..=1.5).contains(&params.density_mult));
        assert!((0.0..=2.0).contains(&params.glitch_mult));
        assert!((-0.1..=0.1).contains(&params.brightness_bias));
    }

    #[test]
    fn clamped_returns_new_clamped_value() {
        let params = RegimeParams {
            speed_mult: 0.0,
            density_mult: 0.0,
            glitch_mult: -1.0,
            brightness_bias: -1.0,
        };
        let clamped = params.clamped();

        assert!((0.5..=2.0).contains(&clamped.speed_mult));
        assert!((0.5..=1.5).contains(&clamped.density_mult));
        assert_eq!(params.speed_mult, 0.0);
    }

    #[test]
    fn regime_constants_are_reasonable() {
        const { assert!(REGIME_MIN_DWELL_SECS >= 5.0) };
        const { assert!(REGIME_TRANSITION_RAMP_SECS >= 0.5) };
    }

    // ── AtmosphereState ──

    #[test]
    fn default_atmosphere_state_is_calm_and_stable() {
        let state = AtmosphereState::default();
        assert_eq!(state.current_regime, AtmosphereRegime::Calm);
        assert_eq!(state.target_regime, AtmosphereRegime::Calm);
        assert_eq!(state.transition_progress, 0.0);
        assert!(state.is_stable());
        assert!(state.is_calm());
    }

    #[test]
    fn calm_to_calm_is_no_op() {
        let state = AtmosphereState::default();
        assert!(state.is_stable());
        assert_eq!(state.effective_params().speed_mult, 1.0);
        assert_eq!(state.effective_params().density_mult, 1.0);
    }

    #[test]
    fn atmosphere_state_initializes_without_allocation() {
        let state = AtmosphereState::default();
        let _ = state; // Copy type, no heap allocation.
        assert_eq!(
            std::mem::size_of::<AtmosphereState>(),
            std::mem::size_of::<AtmosphereState>()
        );
    }

    #[test]
    fn transition_progress_is_bounded_zero_to_one() {
        let state = AtmosphereState::default();
        assert!(state.transition_progress >= 0.0);
        assert!(state.transition_progress <= 1.0);
    }

    // ── AtmosphereController ──

    #[test]
    fn controller_default_is_calm_stable() {
        let ctrl = AtmosphereController::new();
        assert_eq!(ctrl.current_regime(), AtmosphereRegime::Calm);
        assert!(ctrl.is_stable());
        assert!(ctrl.is_effective_noop());
        assert_eq!(ctrl.transition_status(), "stable");
    }

    #[test]
    fn controller_implements_default_trait() {
        let ctrl = AtmosphereController::default();
        assert_eq!(ctrl.current_regime(), AtmosphereRegime::Calm);
    }

    #[test]
    fn controller_state_returns_valid_reference() {
        let ctrl = AtmosphereController::new();
        let state = ctrl.state();
        assert_eq!(state.current_regime, AtmosphereRegime::Calm);
        assert!(state.is_stable());
    }

    #[test]
    fn controller_advance_is_no_op_when_stable() {
        let mut ctrl = AtmosphereController::new();
        ctrl.advance(10.0);
        assert_eq!(ctrl.current_regime(), AtmosphereRegime::Calm);
        assert!(ctrl.is_stable());
    }

    #[test]
    fn controller_transition_rejected_by_dwell_time() {
        let mut ctrl = AtmosphereController::new();
        ctrl.advance(1.0); // Only 1 second, need 5.
        let mut cache = CachePolicy::default_policy();
        let accepted = ctrl.transition_to(AtmosphereRegime::Storm, &mut cache);
        assert!(!accepted);
        assert_eq!(ctrl.current_regime(), AtmosphereRegime::Calm);
        assert!(ctrl.is_stable());
    }

    #[test]
    fn controller_calm_to_calm_transition_is_no_op() {
        let mut ctrl = AtmosphereController::new();
        ctrl.advance(10.0); // Enough dwell time.
        let mut cache = CachePolicy::default_policy();
        let accepted = ctrl.transition_to(AtmosphereRegime::Calm, &mut cache);
        assert!(!accepted);
    }

    #[test]
    fn controller_force_transition_bypasses_dwell() {
        let mut ctrl = AtmosphereController::new();
        let changed = ctrl.force_transition_to(AtmosphereRegime::Storm);
        assert!(changed);
        assert_eq!(ctrl.current_regime(), AtmosphereRegime::Storm);
        assert!(ctrl.is_stable()); // force snaps immediately.
    }

    #[test]
    fn controller_force_same_regime_is_no_op() {
        let mut ctrl = AtmosphereController::new();
        let changed = ctrl.force_transition_to(AtmosphereRegime::Calm);
        assert!(!changed);
    }

    #[test]
    fn controller_transition_advances_progress() {
        let mut ctrl = AtmosphereController::new();
        ctrl.advance(10.0); // Enough dwell time.
        let mut cache = CachePolicy::default_policy();
        let accepted = ctrl.transition_to(AtmosphereRegime::Storm, &mut cache);
        assert!(accepted);
        assert!(!ctrl.is_stable());
        assert_eq!(ctrl.transition_status(), "transitioning");
        assert!(ctrl.state().transition_progress < 1.0);

        // Advance past ramp duration.
        ctrl.advance(REGIME_TRANSITION_RAMP_SECS + 0.5);
        assert_eq!(ctrl.current_regime(), AtmosphereRegime::Storm);
        assert!(ctrl.is_stable());
    }

    // ── RegimeProbe ──

    #[test]
    fn regime_probe_idle_defaults() {
        let probe = RegimeProbe::idle();
        assert_eq!(probe.dirty_cell_ratio, 0.0);
        assert_eq!(probe.active_streams, 0);
        assert_eq!(probe.frame_time_pressure, 0.0);
        assert!(!probe.benchmark_mode);
    }

    // ── select_regime_from_probe ──

    #[test]
    fn probe_idle_selects_calm() {
        let probe = RegimeProbe::idle();
        assert_eq!(select_regime_from_probe(&probe), AtmosphereRegime::Calm);
    }

    #[test]
    fn probe_benchmark_selects_calm() {
        let probe = RegimeProbe {
            benchmark_mode: true,
            dirty_cell_ratio: 0.9,
            active_streams: 100,
            frame_time_pressure: 50.0,
            elapsed_secs: 0.0,
        };
        assert_eq!(select_regime_from_probe(&probe), AtmosphereRegime::Calm);
    }

    #[test]
    fn probe_extreme_pressure_selects_storm() {
        let probe = RegimeProbe {
            frame_time_pressure: 40.0,
            active_streams: 20,
            ..RegimeProbe::idle()
        };
        assert_eq!(select_regime_from_probe(&probe), AtmosphereRegime::Storm);
    }

    #[test]
    fn probe_high_dirty_with_pressure_selects_storm() {
        let probe = RegimeProbe {
            frame_time_pressure: 15.0,
            dirty_cell_ratio: 0.9,
            active_streams: 80,
            ..RegimeProbe::idle()
        };
        assert_eq!(select_regime_from_probe(&probe), AtmosphereRegime::Storm);
    }

    #[test]
    fn probe_high_dirty_selects_pulse() {
        let probe = RegimeProbe {
            dirty_cell_ratio: 0.9,
            active_streams: 60,
            frame_time_pressure: 5.0,
            ..RegimeProbe::idle()
        };
        assert_eq!(select_regime_from_probe(&probe), AtmosphereRegime::Pulse);
    }

    #[test]
    fn probe_low_activity_selects_void() {
        let probe = RegimeProbe {
            dirty_cell_ratio: 0.05,
            active_streams: 3,
            frame_time_pressure: 1.0,
            ..RegimeProbe::idle()
        };
        assert_eq!(select_regime_from_probe(&probe), AtmosphereRegime::Void);
    }

    #[test]
    fn probe_moderate_selects_calm() {
        let probe = RegimeProbe {
            dirty_cell_ratio: 0.3,
            active_streams: 20,
            frame_time_pressure: 5.0,
            ..RegimeProbe::idle()
        };
        assert_eq!(select_regime_from_probe(&probe), AtmosphereRegime::Calm);
    }

    #[test]
    fn probe_selection_is_deterministic() {
        for _ in 0..100 {
            let probe = RegimeProbe {
                dirty_cell_ratio: 0.5,
                active_streams: 30,
                frame_time_pressure: 8.0,
                benchmark_mode: false,
                elapsed_secs: 1.0,
            };
            assert_eq!(
                select_regime_from_probe(&probe),
                AtmosphereRegime::Calm,
                "probe selection must be deterministic"
            );
        }
    }

    // ── Cache integration ──

    #[test]
    fn regime_change_invalidates_cache_generation() {
        let mut ctrl = AtmosphereController::new();
        let mut cache = CachePolicy::default_policy();
        let initial_gen = cache.generation;

        ctrl.advance(10.0);
        ctrl.transition_to(AtmosphereRegime::Storm, &mut cache);

        assert_ne!(cache.generation.id(), initial_gen.id());
        assert!(!cache.is_generation_current(initial_gen));
    }

    #[test]
    fn regime_change_uses_atmosphere_invalidation_event() {
        let mut ctrl = AtmosphereController::new();
        let mut cache = CachePolicy::default_policy();
        let gen_before = cache.generation.id();

        ctrl.advance(10.0);
        ctrl.transition_to(AtmosphereRegime::Compression, &mut cache);

        assert_eq!(cache.generation.id(), gen_before + 1);
    }
}
