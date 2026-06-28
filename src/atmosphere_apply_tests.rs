// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for atmosphere_apply module (Phase 4/5/6).
//!
//! Extracted from atmosphere_apply.rs to reduce file pressure below 800 LOC.

#[cfg(test)]
mod tests {
    use crate::atmosphere_apply::derive_effective_runtime;
    use crate::atmosphere_apply::AtmosphereApplicationMode;
    use crate::atmosphere_apply::AtmosphereRuntimeModulation;
    use crate::atmosphere_verifier::AtmosphereApplication;
    use crate::constants::{
        DENSITY_CLAMP_MAX, DENSITY_CLAMP_MIN, RUNTIME_SPEED_MAX, RUNTIME_SPEED_MIN,
    };

    // ── AtmosphereApplicationMode basics ──

    #[test]
    fn disabled_mode_is_default() {
        let mode = AtmosphereApplicationMode::default();
        assert_eq!(mode, AtmosphereApplicationMode::Disabled);
        assert!(!mode.allows_modulation());
        assert_eq!(mode.as_str(), "disabled");
    }

    #[test]
    fn internal_verified_mode_allows_modulation() {
        let mode = AtmosphereApplicationMode::InternalVerified;
        assert!(mode.allows_modulation());
        assert_eq!(mode.as_str(), "internal-verified");
    }

    #[test]
    fn test_only_mode_allows_modulation() {
        let mode = AtmosphereApplicationMode::TestOnly;
        assert!(mode.allows_modulation());
        assert_eq!(mode.as_str(), "test-only");
    }

    #[test]
    fn all_modes_have_distinct_labels() {
        let modes = [
            AtmosphereApplicationMode::Disabled,
            AtmosphereApplicationMode::InternalVerified,
            AtmosphereApplicationMode::ControlledLive,
            AtmosphereApplicationMode::TestOnly,
        ];
        for (i, a) in modes.iter().enumerate() {
            for (j, b) in modes.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "modes must be distinct");
                }
            }
        }
    }

    // ── AtmosphereRuntimeModulation basics ──

    #[test]
    fn identity_modulation_is_no_op() {
        let mod_identity = AtmosphereRuntimeModulation::identity();
        assert!(mod_identity.is_identity());
        assert_eq!(mod_identity.speed_scale, 1.0);
        assert_eq!(mod_identity.density_scale, 1.0);
        assert_eq!(mod_identity.brightness_scale, 1.0);
        assert_eq!(mod_identity.glitch_pressure, 0.0);
        assert!(!mod_identity.color_change_allowed);
        assert!(!mod_identity.terminal_effect_allowed);
    }

    #[test]
    fn identity_modulation_is_copy_type() {
        let a = AtmosphereRuntimeModulation::identity();
        let b = a;
        assert_eq!(a.speed_scale, b.speed_scale);
    }

    // ── apply_application: Disabled mode ──

    #[test]
    fn disabled_mode_returns_identity_for_calm_application() {
        let app = AtmosphereApplication::identity();
        let result =
            crate::atmosphere_apply::apply_application(&app, AtmosphereApplicationMode::Disabled);
        assert!(result.is_identity());
    }

    #[test]
    fn disabled_mode_returns_identity_for_non_calm_application() {
        let app = AtmosphereApplication {
            speed_scale: 1.5,
            density_scale: 1.2,
            brightness_scale: 1.05,
            glitch_pressure: 0.3,
            color_change: false,
        };
        let result =
            crate::atmosphere_apply::apply_application(&app, AtmosphereApplicationMode::Disabled);
        assert!(result.is_identity());
    }

    #[test]
    fn disabled_mode_never_allows_color_or_terminal() {
        let mut app = AtmosphereApplication {
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
        let result =
            crate::atmosphere_apply::apply_application(&app, AtmosphereApplicationMode::Disabled);
        assert!(!result.color_change_allowed);
        assert!(!result.terminal_effect_allowed);
    }

    // ── apply_application: Calm application ──

    #[test]
    fn calm_application_returns_identity_in_all_modes() {
        let app = AtmosphereApplication::identity();
        for mode in [
            AtmosphereApplicationMode::Disabled,
            AtmosphereApplicationMode::InternalVerified,
            AtmosphereApplicationMode::ControlledLive,
            AtmosphereApplicationMode::TestOnly,
        ] {
            let result = crate::atmosphere_apply::apply_application(&app, mode);
            assert!(result.is_identity(), "Calm must be identity in {:?}", mode);
        }
    }

    // ── apply_application: InternalVerified mode ──

    #[test]
    fn internal_verified_non_calm_returns_bounded_modulation() {
        let mut app = AtmosphereApplication {
            speed_scale: 1.5,
            density_scale: 1.2,
            brightness_scale: 1.05,
            glitch_pressure: 0.3,
            color_change: false,
        };
        let _ = crate::atmosphere_verifier::verify_application(
            &mut app,
            &crate::atmosphere_verifier::AtmosphereBounds::conservative(),
        );
        let result = crate::atmosphere_apply::apply_application(
            &app,
            AtmosphereApplicationMode::InternalVerified,
        );
        assert!(!result.is_identity());
        assert_eq!(result.speed_scale, 1.5);
        assert_eq!(result.density_scale, 1.2);
        assert!(!result.color_change_allowed);
        assert!(!result.terminal_effect_allowed);
    }

    #[test]
    fn internal_verified_modulation_is_bounded() {
        let bounds = crate::atmosphere_verifier::AtmosphereBounds::conservative();
        let mut app = AtmosphereApplication {
            speed_scale: 10.0,
            density_scale: 0.0,
            brightness_scale: 5.0,
            glitch_pressure: 3.0,
            color_change: true,
        };
        let _ = crate::atmosphere_verifier::verify_application(&mut app, &bounds);
        let result = crate::atmosphere_apply::apply_application(
            &app,
            AtmosphereApplicationMode::InternalVerified,
        );

        assert!(result.speed_scale >= bounds.speed_min);
        assert!(result.speed_scale <= bounds.speed_max);
        assert!(result.density_scale >= bounds.density_min);
        assert!(result.density_scale <= bounds.density_max);
        assert!(result.brightness_scale >= bounds.brightness_min);
        assert!(result.brightness_scale <= bounds.brightness_max);
        assert!(result.glitch_pressure >= 0.0);
        assert!(result.glitch_pressure <= bounds.glitch_pressure_max);
    }

    // ── TestOnly mode ──

    #[test]
    fn test_only_non_calm_returns_bounded_modulation() {
        let mut app = AtmosphereApplication {
            speed_scale: 1.8,
            density_scale: 1.3,
            brightness_scale: 1.08,
            glitch_pressure: 0.4,
            color_change: false,
        };
        let _ = crate::atmosphere_verifier::verify_application(
            &mut app,
            &crate::atmosphere_verifier::AtmosphereBounds::conservative(),
        );
        let result =
            crate::atmosphere_apply::apply_application(&app, AtmosphereApplicationMode::TestOnly);
        assert!(!result.is_identity());
        assert_eq!(result.speed_scale, 1.8);
        assert!(!result.color_change_allowed);
        assert!(!result.terminal_effect_allowed);
    }

    // ── Determinism ──

    #[test]
    fn apply_application_is_deterministic() {
        let mut app = AtmosphereApplication {
            speed_scale: 1.7,
            density_scale: 1.1,
            brightness_scale: 1.03,
            glitch_pressure: 0.2,
            color_change: false,
        };
        let _ = crate::atmosphere_verifier::verify_application(
            &mut app,
            &crate::atmosphere_verifier::AtmosphereBounds::conservative(),
        );
        for _ in 0..100 {
            let a = crate::atmosphere_apply::apply_application(
                &app,
                AtmosphereApplicationMode::InternalVerified,
            );
            let b = crate::atmosphere_apply::apply_application(
                &app,
                AtmosphereApplicationMode::InternalVerified,
            );
            assert_eq!(a.speed_scale, b.speed_scale);
            assert_eq!(a.density_scale, b.density_scale);
            assert_eq!(a.brightness_scale, b.brightness_scale);
            assert_eq!(a.glitch_pressure, b.glitch_pressure);
            assert_eq!(a.color_change_allowed, b.color_change_allowed);
            assert_eq!(a.terminal_effect_allowed, b.terminal_effect_allowed);
        }
    }

    // ── Effective parameter helpers ──

    #[test]
    fn effective_speed_equals_base_for_identity() {
        let mod_identity = AtmosphereRuntimeModulation::identity();
        assert_eq!(
            crate::atmosphere_apply::effective_speed(20.0, &mod_identity),
            20.0
        );
        assert_eq!(
            crate::atmosphere_apply::effective_speed(8.0, &mod_identity),
            8.0
        );
    }

    #[test]
    fn effective_speed_scales_with_modulation() {
        let mod_val = AtmosphereRuntimeModulation {
            speed_scale: 1.5,
            density_scale: 1.0,
            brightness_scale: 1.0,
            glitch_pressure: 0.0,
            color_change_allowed: false,
            terminal_effect_allowed: false,
        };
        assert!(
            (crate::atmosphere_apply::effective_speed(10.0, &mod_val) - 15.0).abs() < f32::EPSILON
        );
    }

    #[test]
    fn effective_density_equals_base_for_identity() {
        let mod_identity = AtmosphereRuntimeModulation::identity();
        assert_eq!(
            crate::atmosphere_apply::effective_density_from_modulation(1.0, &mod_identity),
            1.0
        );
        assert_eq!(
            crate::atmosphere_apply::effective_density_from_modulation(0.75, &mod_identity),
            0.75
        );
    }

    #[test]
    fn effective_density_is_clamped() {
        let mod_high = AtmosphereRuntimeModulation {
            speed_scale: 1.0,
            density_scale: 10.0,
            brightness_scale: 1.0,
            glitch_pressure: 0.0,
            color_change_allowed: false,
            terminal_effect_allowed: false,
        };
        let result = crate::atmosphere_apply::effective_density_from_modulation(1.0, &mod_high);
        assert_eq!(result, 5.0); // clamped to max
    }

    #[test]
    fn effective_brightness_returns_scale() {
        let mod_identity = AtmosphereRuntimeModulation::identity();
        assert_eq!(
            crate::atmosphere_apply::effective_brightness(&mod_identity),
            1.0
        );
    }

    #[test]
    fn effective_glitch_pressure_returns_pressure() {
        let mod_identity = AtmosphereRuntimeModulation::identity();
        assert_eq!(
            crate::atmosphere_apply::effective_glitch_pressure(&mod_identity),
            0.0
        );
    }

    // ── Pulse/Storm/Void clamped by verifier before adapter ──

    #[test]
    fn pulse_application_clamped_before_adapter_output() {
        let bounds = crate::atmosphere_verifier::AtmosphereBounds::conservative();
        let mut app =
            crate::atmosphere_verifier::application_from_regime_params(1.8, 1.3, 1.8, 0.08);
        let _ = crate::atmosphere_verifier::verify_application(&mut app, &bounds);
        let result = crate::atmosphere_apply::apply_application(
            &app,
            AtmosphereApplicationMode::InternalVerified,
        );
        assert!(result.speed_scale >= bounds.speed_min);
        assert!(result.speed_scale <= bounds.speed_max);
        assert!(result.density_scale >= bounds.density_min);
        assert!(result.density_scale <= bounds.density_max);
    }

    #[test]
    fn storm_application_clamped_before_adapter_output() {
        let bounds = crate::atmosphere_verifier::AtmosphereBounds::conservative();
        let mut app =
            crate::atmosphere_verifier::application_from_regime_params(2.0, 1.5, 2.0, 0.1);
        let _ = crate::atmosphere_verifier::verify_application(&mut app, &bounds);
        let result = crate::atmosphere_apply::apply_application(
            &app,
            AtmosphereApplicationMode::InternalVerified,
        );
        assert!(result.speed_scale <= bounds.speed_max);
        assert!(result.density_scale <= bounds.density_max);
        assert!(result.glitch_pressure <= bounds.glitch_pressure_max);
    }

    #[test]
    fn void_application_clamped_before_adapter_output() {
        let bounds = crate::atmosphere_verifier::AtmosphereBounds::conservative();
        let mut app =
            crate::atmosphere_verifier::application_from_regime_params(0.5, 0.5, 0.5, -0.1);
        let _ = crate::atmosphere_verifier::verify_application(&mut app, &bounds);
        let result = crate::atmosphere_apply::apply_application(
            &app,
            AtmosphereApplicationMode::InternalVerified,
        );
        assert!(result.speed_scale >= bounds.speed_min);
        assert!(result.density_scale >= bounds.density_min);
        assert!(result.brightness_scale >= bounds.brightness_min);
    }

    #[test]
    fn out_of_range_application_clamped_before_runtime_modulation() {
        let bounds = crate::atmosphere_verifier::AtmosphereBounds::conservative();
        let mut app = AtmosphereApplication {
            speed_scale: 100.0,
            density_scale: -50.0,
            brightness_scale: 100.0,
            glitch_pressure: 100.0,
            color_change: true,
        };
        let _ = crate::atmosphere_verifier::verify_application(&mut app, &bounds);
        let result = crate::atmosphere_apply::apply_application(
            &app,
            AtmosphereApplicationMode::InternalVerified,
        );
        assert!(result.speed_scale <= bounds.speed_max);
        assert!(result.density_scale >= bounds.density_min);
        assert!(result.brightness_scale <= bounds.brightness_max);
        assert!(result.glitch_pressure <= bounds.glitch_pressure_max);
        assert!(!result.color_change_allowed);
    }

    // ── Phase 5: AtmosphereEffectiveRuntime ──

    #[test]
    fn default_effective_runtime_equals_base_speed_and_density() {
        let modulation = AtmosphereRuntimeModulation::identity();
        let eff = derive_effective_runtime(20.0, 0.75, &modulation);
        assert_eq!(eff.speed, 20.0);
        assert_eq!(eff.density, 0.75);
        assert_eq!(eff.brightness_scale, 1.0);
        assert_eq!(eff.glitch_pressure, 0.0);
        assert!(!eff.color_change_allowed);
        assert!(!eff.terminal_effect_allowed);
    }

    #[test]
    fn disabled_modulation_effective_equals_base() {
        let app = AtmosphereApplication::identity();
        let modulation =
            crate::atmosphere_apply::apply_application(&app, AtmosphereApplicationMode::Disabled);
        let eff = derive_effective_runtime(15.0, 1.0, &modulation);
        assert_eq!(eff.speed, 15.0);
        assert_eq!(eff.density, 1.0);
    }

    #[test]
    fn calm_modulation_effective_equals_base() {
        let app = AtmosphereApplication::identity();
        let modulation = crate::atmosphere_apply::apply_application(
            &app,
            AtmosphereApplicationMode::InternalVerified,
        );
        let eff = derive_effective_runtime(8.0, 0.5, &modulation);
        assert_eq!(eff.speed, 8.0);
        assert_eq!(eff.density, 0.5);
    }

    #[test]
    fn internal_verified_non_calm_derives_bounded_effective() {
        let mut app = AtmosphereApplication {
            speed_scale: 1.5,
            density_scale: 1.2,
            brightness_scale: 1.05,
            glitch_pressure: 0.3,
            color_change: false,
        };
        let _ = crate::atmosphere_verifier::verify_application(
            &mut app,
            &crate::atmosphere_verifier::AtmosphereBounds::conservative(),
        );
        let modulation = crate::atmosphere_apply::apply_application(
            &app,
            AtmosphereApplicationMode::InternalVerified,
        );
        let eff = derive_effective_runtime(20.0, 0.75, &modulation);
        assert!((eff.speed - 30.0).abs() < 0.01); // 20.0 * 1.5
        assert!((eff.density - 0.9).abs() < 0.01); // 0.75 * 1.2
        assert_eq!(eff.brightness_scale, 1.05);
        assert_eq!(eff.glitch_pressure, 0.3);
        assert!(!eff.color_change_allowed);
        assert!(!eff.terminal_effect_allowed);
    }

    #[test]
    fn effective_speed_is_clamped_to_safe_range() {
        let extreme = AtmosphereRuntimeModulation {
            speed_scale: 100.0,
            density_scale: 1.0,
            brightness_scale: 1.0,
            glitch_pressure: 0.0,
            color_change_allowed: false,
            terminal_effect_allowed: false,
        };
        let eff = derive_effective_runtime(50.0, 1.0, &extreme);
        assert_eq!(eff.speed, RUNTIME_SPEED_MAX); // clamped
    }

    #[test]
    fn effective_density_is_clamped_to_safe_range() {
        let extreme = AtmosphereRuntimeModulation {
            speed_scale: 1.0,
            density_scale: 100.0,
            brightness_scale: 1.0,
            glitch_pressure: 0.0,
            color_change_allowed: false,
            terminal_effect_allowed: false,
        };
        let eff = derive_effective_runtime(10.0, 0.1, &extreme);
        assert_eq!(eff.density, DENSITY_CLAMP_MAX); // clamped
    }

    #[test]
    fn effective_runtime_never_allows_color_change() {
        let mod_identity = AtmosphereRuntimeModulation::identity();
        let eff = derive_effective_runtime(10.0, 1.0, &mod_identity);
        assert!(!eff.color_change_allowed);

        let extreme = AtmosphereRuntimeModulation {
            speed_scale: 1.0,
            density_scale: 1.0,
            brightness_scale: 1.0,
            glitch_pressure: 0.0,
            color_change_allowed: true,
            terminal_effect_allowed: true,
        };
        let eff = derive_effective_runtime(10.0, 1.0, &extreme);
        assert!(!eff.color_change_allowed);
    }

    #[test]
    fn effective_runtime_never_allows_terminal_effects() {
        let extreme = AtmosphereRuntimeModulation {
            speed_scale: 1.0,
            density_scale: 1.0,
            brightness_scale: 1.0,
            glitch_pressure: 0.0,
            color_change_allowed: true,
            terminal_effect_allowed: true,
        };
        let eff = derive_effective_runtime(10.0, 1.0, &extreme);
        assert!(!eff.terminal_effect_allowed);
    }

    #[test]
    fn derive_effective_runtime_is_deterministic() {
        let modulation = AtmosphereRuntimeModulation::identity();
        for _ in 0..50 {
            let a = derive_effective_runtime(20.0, 0.75, &modulation);
            let b = derive_effective_runtime(20.0, 0.75, &modulation);
            assert_eq!(a.speed, b.speed);
            assert_eq!(a.density, b.density);
            assert_eq!(a.brightness_scale, b.brightness_scale);
        }
    }

    #[test]
    fn effective_runtime_speed_clamped_to_minimum() {
        let near_zero = AtmosphereRuntimeModulation {
            speed_scale: 0.001,
            density_scale: 1.0,
            brightness_scale: 1.0,
            glitch_pressure: 0.0,
            color_change_allowed: false,
            terminal_effect_allowed: false,
        };
        let eff = derive_effective_runtime(50.0, 1.0, &near_zero);
        assert_eq!(eff.speed, RUNTIME_SPEED_MIN);
    }

    #[test]
    fn effective_runtime_density_clamped_to_minimum() {
        let near_zero = AtmosphereRuntimeModulation {
            speed_scale: 1.0,
            density_scale: 0.001,
            brightness_scale: 1.0,
            glitch_pressure: 0.0,
            color_change_allowed: false,
            terminal_effect_allowed: false,
        };
        let eff = derive_effective_runtime(10.0, 1.0, &near_zero);
        assert_eq!(eff.density, DENSITY_CLAMP_MIN);
    }

    #[test]
    fn effective_runtime_identity_check() {
        let mod_identity = AtmosphereRuntimeModulation::identity();
        let eff = derive_effective_runtime(20.0, 1.0, &mod_identity);
        assert!(eff.is_identity());
    }

    #[test]
    fn derive_effective_runtime_combined_modulation() {
        let combined = AtmosphereRuntimeModulation {
            speed_scale: 1.3,
            density_scale: 0.8,
            brightness_scale: 1.05,
            glitch_pressure: 0.15,
            color_change_allowed: false,
            terminal_effect_allowed: false,
        };
        let eff = derive_effective_runtime(25.0, 1.5, &combined);
        assert!((eff.speed - 32.5).abs() < 0.01); // 25.0 * 1.3
        assert!((eff.density - 1.2).abs() < 0.01); // 1.5 * 0.8
        assert_eq!(eff.brightness_scale, 1.05);
        assert_eq!(eff.glitch_pressure, 0.15);
        assert!(!eff.color_change_allowed);
        assert!(!eff.terminal_effect_allowed);
    }

    #[test]
    fn derive_effective_runtime_with_test_only_mode() {
        let mut app = AtmosphereApplication {
            speed_scale: 1.6,
            density_scale: 1.1,
            brightness_scale: 1.02,
            glitch_pressure: 0.2,
            color_change: false,
        };
        let _ = crate::atmosphere_verifier::verify_application(
            &mut app,
            &crate::atmosphere_verifier::AtmosphereBounds::conservative(),
        );
        let modulation =
            crate::atmosphere_apply::apply_application(&app, AtmosphereApplicationMode::TestOnly);
        let eff = derive_effective_runtime(12.0, 0.8, &modulation);
        assert!((eff.speed - 19.2).abs() < 0.01); // 12.0 * 1.6
        assert!((eff.density - 0.88).abs() < 0.01); // 0.8 * 1.1
    }

    // ── Application adapter does not touch cache ──

    #[test]
    fn application_adapter_does_not_invalidate_cache() {
        use crate::zactrix_cache::CachePolicy;
        let cache = CachePolicy::default_policy();
        let gen_before = cache.generation.id();

        let app = AtmosphereApplication::identity();
        let _ =
            crate::atmosphere_apply::apply_application(&app, AtmosphereApplicationMode::Disabled);
        let _ = crate::atmosphere_apply::apply_application(
            &app,
            AtmosphereApplicationMode::InternalVerified,
        );

        assert_eq!(cache.generation.id(), gen_before);
    }

    #[test]
    fn runtime_default_effective_speed_equals_base_speed() {
        let mod_default = crate::atmosphere_apply::apply_application(
            &AtmosphereApplication::identity(),
            AtmosphereApplicationMode::Disabled,
        );
        let base = 20.0;
        assert_eq!(
            crate::atmosphere_apply::effective_speed(base, &mod_default),
            base
        );
    }

    #[test]
    fn runtime_default_effective_density_equals_base_density() {
        let mod_default = crate::atmosphere_apply::apply_application(
            &AtmosphereApplication::identity(),
            AtmosphereApplicationMode::Disabled,
        );
        let base = 0.75;
        assert_eq!(
            crate::atmosphere_apply::effective_density_from_modulation(base, &mod_default),
            base
        );
    }

    #[test]
    fn runtime_default_application_does_not_change_color() {
        let mod_default = crate::atmosphere_apply::apply_application(
            &AtmosphereApplication::identity(),
            AtmosphereApplicationMode::Disabled,
        );
        assert!(!mod_default.color_change_allowed);
    }

    #[test]
    fn runtime_default_application_does_not_change_terminal_behavior() {
        let mod_default = crate::atmosphere_apply::apply_application(
            &AtmosphereApplication::identity(),
            AtmosphereApplicationMode::Disabled,
        );
        assert!(!mod_default.terminal_effect_allowed);
    }
}
