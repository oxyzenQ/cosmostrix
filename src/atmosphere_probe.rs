// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Regime probe and selection for Cosmostrix v4.0.0.
//!
//! Contains `RegimeProbe` (observable runtime facts) and `select_regime_from_probe()`
//! (deterministic pure function that maps probe facts to candidate regimes).
//! In production, the actual regime remains Calm unless explicitly transitioned.

#![allow(dead_code)]

use crate::atmosphere::AtmosphereRegime;

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

// ── Probe Thresholds ──────────────────────────────────────────────────────

/// Frame-time pressure threshold (ms) above which Storm is candidate.
pub(crate) const FRAME_TIME_HIGH_MS: f64 = 30.0;

/// Frame-time pressure threshold (ms) above which Pulse is candidate.
pub(crate) const FRAME_TIME_MODERATE_MS: f64 = 10.0;

/// Dirty cell ratio threshold for high activity classification.
pub(crate) const DIRTY_RATIO_HIGH: f64 = 0.8;

/// Dirty cell ratio threshold for low activity classification.
pub(crate) const DIRTY_RATIO_LOW: f64 = 0.1;

/// Active streams threshold for "truly active" (avoid Void on idle).
pub(crate) const ACTIVE_STREAMS_LOW: usize = 10;

/// Active streams threshold for high-activity Pulse candidate.
pub(crate) const ACTIVE_STREAMS_HIGH: usize = 50;

// ── Probe-to-Regime Selection ──────────────────────────────────────────

/// Select a candidate regime from probe facts.
///
/// This is a pure deterministic function. It does NOT apply the regime
/// to the renderer. In Phase 2, the result is used for diagnostics only.
/// The actual regime remains Calm unless explicitly transitioned.
pub(crate) fn select_regime_from_probe(probe: &RegimeProbe) -> AtmosphereRegime {
    // Benchmark mode always stays Calm for stability.
    if probe.benchmark_mode {
        return AtmosphereRegime::Calm;
    }

    // Zero active streams -> no data to classify -> Calm.
    if probe.active_streams == 0 {
        return AtmosphereRegime::Calm;
    }

    // Extreme frame-time pressure -> Storm candidate.
    if probe.frame_time_pressure > FRAME_TIME_HIGH_MS {
        return AtmosphereRegime::Storm;
    }

    // High pressure + high dirty ratio -> Storm candidate.
    if probe.frame_time_pressure > FRAME_TIME_MODERATE_MS
        && probe.dirty_cell_ratio > DIRTY_RATIO_HIGH
    {
        return AtmosphereRegime::Storm;
    }

    // High dirty ratio + many active streams -> Pulse candidate.
    if probe.dirty_cell_ratio > DIRTY_RATIO_HIGH && probe.active_streams > ACTIVE_STREAMS_HIGH {
        return AtmosphereRegime::Pulse;
    }

    // Low activity -> Void candidate.
    if probe.dirty_cell_ratio < DIRTY_RATIO_LOW && probe.active_streams < ACTIVE_STREAMS_LOW {
        return AtmosphereRegime::Void;
    }

    // Default -> Calm.
    AtmosphereRegime::Calm
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_idle_defaults() {
        let probe = RegimeProbe::idle();
        assert_eq!(probe.dirty_cell_ratio, 0.0);
        assert_eq!(probe.active_streams, 0);
        assert_eq!(probe.frame_time_pressure, 0.0);
        assert!(!probe.benchmark_mode);
    }

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

    #[test]
    fn probe_constants_are_reasonable() {
        const { assert!(FRAME_TIME_HIGH_MS > FRAME_TIME_MODERATE_MS) };
        const { assert!(DIRTY_RATIO_HIGH > DIRTY_RATIO_LOW) };
        const { assert!(ACTIVE_STREAMS_HIGH > ACTIVE_STREAMS_LOW) };
    }
}
