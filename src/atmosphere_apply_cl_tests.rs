// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Phase 6 ControlledLive modulation tests.

#[cfg(test)]
mod tests {
    use crate::atmosphere_apply::*;

    #[test]
    fn controlled_live_mode_allows_modulation() {
        let mode = crate::atmosphere_apply::AtmosphereApplicationMode::ControlledLive;
        assert!(mode.allows_modulation());
        assert!(mode.is_controlled_live());
        assert_eq!(mode.as_str(), "controlled-live");
    }

    #[test]
    fn controlled_live_calm_returns_identity() {
        let app = crate::atmosphere_verifier::AtmosphereApplication::identity();
        let result = crate::atmosphere_apply::apply_application(
            &app,
            crate::atmosphere_apply::AtmosphereApplicationMode::ControlledLive,
        );
        assert!(result.is_identity());
    }

    #[test]
    fn controlled_live_non_calm_clamped_to_tight_bounds() {
        let mut app = crate::atmosphere_verifier::AtmosphereApplication {
            speed_scale: 2.0,
            density_scale: 1.5,
            brightness_scale: 1.1,
            glitch_pressure: 0.5,
            color_change: false,
        };
        let _ = crate::atmosphere_verifier::verify_application(
            &mut app,
            &crate::atmosphere_verifier::AtmosphereBounds::conservative(),
        );
        let result = crate::atmosphere_apply::apply_application(
            &app,
            crate::atmosphere_apply::AtmosphereApplicationMode::ControlledLive,
        );
        assert!(result.speed_scale <= 1.0 + ControlledLiveBounds::SPEED_MAX_DELTA);
        assert!(result.speed_scale >= 1.0 - ControlledLiveBounds::SPEED_MAX_DELTA);
        assert!(result.density_scale <= 1.0 + ControlledLiveBounds::DENSITY_MAX_DELTA);
        assert!(result.density_scale >= 1.0 - ControlledLiveBounds::DENSITY_MAX_DELTA);
        assert!(result.brightness_scale <= 1.0 + ControlledLiveBounds::BRIGHTNESS_MAX_DELTA);
        assert!(result.brightness_scale >= 1.0 - ControlledLiveBounds::BRIGHTNESS_MAX_DELTA);
        assert!(result.glitch_pressure <= ControlledLiveBounds::GLITCH_PRESSURE_MAX);
        assert!(!result.color_change_allowed);
        assert!(!result.terminal_effect_allowed);
    }

    #[test]
    fn controlled_live_never_allows_color_change() {
        let mut app = crate::atmosphere_verifier::AtmosphereApplication {
            speed_scale: 1.5,
            density_scale: 1.2,
            brightness_scale: 1.05,
            glitch_pressure: 0.3,
            color_change: true,
        };
        let _ = crate::atmosphere_verifier::verify_application(
            &mut app,
            &crate::atmosphere_verifier::AtmosphereBounds::conservative(),
        );
        let result = crate::atmosphere_apply::apply_application(
            &app,
            crate::atmosphere_apply::AtmosphereApplicationMode::ControlledLive,
        );
        assert!(!result.color_change_allowed);
        assert!(!result.terminal_effect_allowed);
    }

    #[test]
    fn controlled_live_effective_runtime_is_close_to_base() {
        let mut app = crate::atmosphere_verifier::AtmosphereApplication {
            speed_scale: 1.08,
            density_scale: 1.06,
            brightness_scale: 1.03,
            glitch_pressure: 0.2,
            color_change: false,
        };
        let _ = crate::atmosphere_verifier::verify_application(
            &mut app,
            &crate::atmosphere_verifier::AtmosphereBounds::conservative(),
        );
        let modulation = crate::atmosphere_apply::apply_application(
            &app,
            crate::atmosphere_apply::AtmosphereApplicationMode::ControlledLive,
        );
        let eff = crate::atmosphere_apply::derive_effective_runtime(20.0, 0.75, &modulation);
        let speed_delta = (eff.speed - 20.0).abs() / 20.0;
        assert!(speed_delta <= ControlledLiveBounds::SPEED_MAX_DELTA + 0.001);
        let density_delta = (eff.density - 0.75).abs() / 0.75;
        assert!(density_delta <= ControlledLiveBounds::DENSITY_MAX_DELTA + 0.001);
        assert!(!eff.color_change_allowed);
        assert!(!eff.terminal_effect_allowed);
    }

    #[test]
    fn controlled_live_is_more_restrictive_than_internal_verified() {
        let mut app = crate::atmosphere_verifier::AtmosphereApplication {
            speed_scale: 1.9,
            density_scale: 1.4,
            brightness_scale: 1.09,
            glitch_pressure: 0.45,
            color_change: false,
        };
        let _ = crate::atmosphere_verifier::verify_application(
            &mut app,
            &crate::atmosphere_verifier::AtmosphereBounds::conservative(),
        );
        let iv = crate::atmosphere_apply::apply_application(
            &app,
            crate::atmosphere_apply::AtmosphereApplicationMode::InternalVerified,
        );
        let cl = crate::atmosphere_apply::apply_application(
            &app,
            crate::atmosphere_apply::AtmosphereApplicationMode::ControlledLive,
        );
        assert!((cl.speed_scale - 1.0).abs() <= (iv.speed_scale - 1.0).abs());
        assert!((cl.density_scale - 1.0).abs() <= (iv.density_scale - 1.0).abs());
        assert!(cl.glitch_pressure <= iv.glitch_pressure);
    }

    #[test]
    fn controlled_live_modulation_from_regime_calm_is_identity() {
        let mod_cl =
            controlled_live_modulation_from_regime(crate::atmosphere::AtmosphereRegime::Calm);
        assert!(mod_cl.is_identity());
    }

    #[test]
    fn controlled_live_modulation_from_regime_pulse_is_subtle() {
        let mod_cl =
            controlled_live_modulation_from_regime(crate::atmosphere::AtmosphereRegime::Pulse);
        assert!(mod_cl.speed_scale > 1.0);
        assert!(mod_cl.speed_scale <= 1.0 + ControlledLiveBounds::SPEED_MAX_DELTA);
        assert!(!mod_cl.color_change_allowed);
        assert!(!mod_cl.terminal_effect_allowed);
    }

    #[test]
    fn controlled_live_modulation_from_regime_storm_is_tightly_clamped() {
        let mod_cl =
            controlled_live_modulation_from_regime(crate::atmosphere::AtmosphereRegime::Storm);
        assert!(mod_cl.speed_scale <= 1.0 + ControlledLiveBounds::SPEED_MAX_DELTA);
        assert!(mod_cl.density_scale <= 1.0 + ControlledLiveBounds::DENSITY_MAX_DELTA);
        assert!(mod_cl.glitch_pressure <= ControlledLiveBounds::GLITCH_PRESSURE_MAX);
    }

    #[test]
    fn controlled_live_modulation_from_regime_void_is_subtle() {
        let mod_cl =
            controlled_live_modulation_from_regime(crate::atmosphere::AtmosphereRegime::Void);
        assert!(mod_cl.speed_scale >= 1.0 - ControlledLiveBounds::SPEED_MAX_DELTA);
        assert!(mod_cl.density_scale >= 1.0 - ControlledLiveBounds::DENSITY_MAX_DELTA);
        assert!(mod_cl.brightness_scale >= 1.0 - ControlledLiveBounds::BRIGHTNESS_MAX_DELTA);
        assert!(mod_cl.brightness_scale <= 1.0 + ControlledLiveBounds::BRIGHTNESS_MAX_DELTA);
    }

    #[test]
    fn controlled_live_all_non_calm_regimes_within_bounds() {
        let regimes = [
            crate::atmosphere::AtmosphereRegime::Pulse,
            crate::atmosphere::AtmosphereRegime::Compression,
            crate::atmosphere::AtmosphereRegime::Storm,
            crate::atmosphere::AtmosphereRegime::Void,
            crate::atmosphere::AtmosphereRegime::Signal,
            crate::atmosphere::AtmosphereRegime::MonolithPressure,
        ];
        for regime in regimes {
            let mod_cl = controlled_live_modulation_from_regime(regime);
            assert!(
                mod_cl.speed_scale >= 1.0 - ControlledLiveBounds::SPEED_MAX_DELTA,
                "{:?} speed_scale below CL bounds",
                regime
            );
            assert!(
                mod_cl.speed_scale <= 1.0 + ControlledLiveBounds::SPEED_MAX_DELTA,
                "{:?} speed_scale above CL bounds",
                regime
            );
            assert!(
                mod_cl.density_scale >= 1.0 - ControlledLiveBounds::DENSITY_MAX_DELTA,
                "{:?} density_scale below CL bounds",
                regime
            );
            assert!(
                mod_cl.density_scale <= 1.0 + ControlledLiveBounds::DENSITY_MAX_DELTA,
                "{:?} density_scale above CL bounds",
                regime
            );
            assert!(
                mod_cl.glitch_pressure <= ControlledLiveBounds::GLITCH_PRESSURE_MAX,
                "{:?} glitch_pressure above CL bounds",
                regime
            );
            assert!(!mod_cl.color_change_allowed);
            assert!(!mod_cl.terminal_effect_allowed);
        }
    }

    #[test]
    fn controlled_live_modulation_is_deterministic() {
        for _ in 0..50 {
            let a =
                controlled_live_modulation_from_regime(crate::atmosphere::AtmosphereRegime::Pulse);
            let b =
                controlled_live_modulation_from_regime(crate::atmosphere::AtmosphereRegime::Pulse);
            assert_eq!(a.speed_scale, b.speed_scale);
            assert_eq!(a.density_scale, b.density_scale);
            assert_eq!(a.brightness_scale, b.brightness_scale);
            assert_eq!(a.glitch_pressure, b.glitch_pressure);
        }
    }

    #[test]
    fn controlled_live_bounds_constants_are_subtle() {
        const { assert!(ControlledLiveBounds::SPEED_MAX_DELTA <= 0.05) };
        const { assert!(ControlledLiveBounds::DENSITY_MAX_DELTA <= 0.05) };
        const { assert!(ControlledLiveBounds::BRIGHTNESS_MAX_DELTA <= 0.04) };
        const { assert!(ControlledLiveBounds::GLITCH_PRESSURE_MAX <= 0.25) };
    }

    #[test]
    fn controlled_live_disabled_mode_is_default() {
        assert_eq!(
            crate::atmosphere_apply::AtmosphereApplicationMode::default(),
            crate::atmosphere_apply::AtmosphereApplicationMode::Disabled
        );
    }
}
