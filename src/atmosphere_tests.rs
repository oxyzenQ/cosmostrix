// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Tests for atmosphere module (regime enum, params, state, controller).
//!
//! Extracted from atmosphere.rs to reduce file pressure.

#[cfg(test)]
mod tests {
    use crate::atmosphere::AtmosphereController;
    use crate::atmosphere::AtmosphereRegime;
    use crate::atmosphere::AtmosphereState;
    use crate::zactrix_cache::CachePolicy;

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
        let params = crate::atmosphere::RegimeParams::calm();
        assert_eq!(params.speed_mult, 1.0);
        assert_eq!(params.density_mult, 1.0);
        assert_eq!(params.glitch_mult, 1.0);
        assert_eq!(params.brightness_bias, 0.0);
    }

    #[test]
    fn regime_params_clamp_to_safe_ranges() {
        let mut params = crate::atmosphere::RegimeParams {
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
        let params = crate::atmosphere::RegimeParams {
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
        const { assert!(crate::atmosphere::REGIME_MIN_DWELL_SECS >= 5.0) };
        const { assert!(crate::atmosphere::REGIME_TRANSITION_RAMP_SECS >= 0.5) };
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
    fn params_for_regime_calm_is_identity() {
        let params = crate::atmosphere::params_for_regime(AtmosphereRegime::Calm);
        assert_eq!(params.speed_mult, 1.0);
        assert_eq!(params.density_mult, 1.0);
        assert_eq!(params.glitch_mult, 1.0);
        assert_eq!(params.brightness_bias, 0.0);
    }

    #[test]
    fn params_for_regime_pulse_has_subtle_speed_and_brightness_lift() {
        let params = crate::atmosphere::params_for_regime(AtmosphereRegime::Pulse);
        assert!(params.speed_mult > 1.0);
        assert!(params.speed_mult <= 1.06);
        assert_eq!(params.density_mult, 1.0);
        assert!(params.brightness_bias > 0.0);
        assert!(params.brightness_bias <= 0.03);
    }

    #[test]
    fn params_for_regime_void_has_density_reduction() {
        let params = crate::atmosphere::params_for_regime(AtmosphereRegime::Void);
        assert!(params.density_mult < 1.0);
        assert!(params.density_mult >= 0.95);
        assert!(params.speed_mult < 1.0);
        assert_eq!(params.brightness_bias, 0.0);
    }

    #[test]
    fn params_for_regime_storm_is_tightly_bounded() {
        let params = crate::atmosphere::params_for_regime(AtmosphereRegime::Storm);
        assert!(params.speed_mult > 1.0);
        assert!(params.speed_mult <= 1.10);
        assert!(params.density_mult > 1.0);
        assert!(params.density_mult <= 1.08);
        assert!(params.glitch_mult >= 1.0);
        assert!(params.glitch_mult <= 1.3);
    }

    #[test]
    fn params_for_regime_all_non_calm_bounded_within_safe_ranges() {
        let non_calm = [
            AtmosphereRegime::Pulse,
            AtmosphereRegime::Compression,
            AtmosphereRegime::Storm,
            AtmosphereRegime::Void,
            AtmosphereRegime::Signal,
            AtmosphereRegime::MonolithPressure,
        ];
        for regime in non_calm {
            let params = crate::atmosphere::params_for_regime(regime);
            let clamped = params.clamped();
            assert_eq!(
                clamped.speed_mult, params.speed_mult,
                "{:?} speed_mult must be in safe range",
                regime
            );
            assert_eq!(
                clamped.density_mult, params.density_mult,
                "{:?} density_mult must be in safe range",
                regime
            );
            assert_eq!(
                clamped.glitch_mult, params.glitch_mult,
                "{:?} glitch_mult must be in safe range",
                regime
            );
            assert_eq!(
                clamped.brightness_bias, params.brightness_bias,
                "{:?} brightness_bias must be in safe range",
                regime
            );
        }
    }

    #[test]
    fn transition_progress_is_bounded_zero_to_one() {
        let state = AtmosphereState::default();
        assert!(state.transition_progress >= 0.0);
        assert!(state.transition_progress <= 1.0);
    }

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
        ctrl.advance(1.0);
        let mut cache = CachePolicy::default_policy();
        let accepted = ctrl.transition_to(AtmosphereRegime::Storm, &mut cache);
        assert!(!accepted);
        assert_eq!(ctrl.current_regime(), AtmosphereRegime::Calm);
        assert!(ctrl.is_stable());
    }

    #[test]
    fn controller_calm_to_calm_transition_is_no_op() {
        let mut ctrl = AtmosphereController::new();
        ctrl.advance(10.0);
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
        assert!(ctrl.is_stable());
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
        ctrl.advance(10.0);
        let mut cache = CachePolicy::default_policy();
        let accepted = ctrl.transition_to(AtmosphereRegime::Storm, &mut cache);
        assert!(accepted);
        assert!(!ctrl.is_stable());
        assert_eq!(ctrl.transition_status(), "transitioning");
        assert!(ctrl.state().transition_progress < 1.0);

        ctrl.advance(crate::atmosphere::REGIME_TRANSITION_RAMP_SECS + 0.5);
        assert_eq!(ctrl.current_regime(), AtmosphereRegime::Storm);
        assert!(ctrl.is_stable());
    }

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

    #[test]
    fn calm_controller_builds_identity_application() {
        let ctrl = AtmosphereController::new();
        let app = ctrl.build_application();
        assert!(app.is_identity());
    }

    #[test]
    fn calm_state_builds_identity_application() {
        let state = AtmosphereState::default();
        let app = state.build_application();
        assert!(app.is_identity());
        assert_eq!(app.speed_scale, 1.0);
        assert_eq!(app.density_scale, 1.0);
        assert_eq!(app.brightness_scale, 1.0);
        assert_eq!(app.glitch_pressure, 0.0);
        assert!(!app.color_change);
    }

    #[test]
    fn build_application_is_deterministic() {
        let ctrl = AtmosphereController::new();
        for _ in 0..50 {
            let app_a = ctrl.build_application();
            let app_b = ctrl.build_application();
            assert_eq!(app_a.speed_scale, app_b.speed_scale);
            assert_eq!(app_a.density_scale, app_b.density_scale);
            assert_eq!(app_a.brightness_scale, app_b.brightness_scale);
            assert_eq!(app_a.glitch_pressure, app_b.glitch_pressure);
            assert_eq!(app_a.color_change, app_b.color_change);
        }
    }

    #[test]
    fn transition_progress_remains_bounded_after_build() {
        let ctrl = AtmosphereController::new();
        let state = ctrl.state();
        let _app = ctrl.build_application();
        assert!(state.transition_progress >= 0.0);
        assert!(state.transition_progress <= 1.0);
    }
}
