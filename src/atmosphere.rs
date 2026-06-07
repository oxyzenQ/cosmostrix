// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Future atmosphere regime types for Cosmostrix v4.0.0.
//!
//! The Atmosphere Engine is a future visual layer that models the overall
//! visual climate of the terminal render. v4.0.0 Phase 1 defines the regime
//! types and design contracts but does not enable any regime transitions or
//! visual changes. The renderer behaves exactly as v3.9.0.

// Phase 1: Atmosphere types are design contracts tested through unit tests.
// Not yet wired into the rendering path.
#![allow(dead_code)]

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

/// Bounded parameter multipliers for a regime.
///
/// All values are clamped to safe ranges. These define how a regime
/// modulates rendering parameters. In v4.0.0 Phase 1, only Calm defaults
/// are active.
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
    /// Default params for Calm regime: no modulation.
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

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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
        // Original unchanged
        assert_eq!(params.speed_mult, 0.0);
    }

    #[test]
    fn regime_constants_are_reasonable() {
        const { assert!(REGIME_MIN_DWELL_SECS >= 5.0) };
        const { assert!(REGIME_TRANSITION_RAMP_SECS >= 0.5) };
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
}
