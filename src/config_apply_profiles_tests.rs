// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

use clap::{CommandFactory, FromArgMatches};

use crate::config::{Args, GlitchLevel};
use crate::config_apply::apply_config_and_runtime_defaults;
use crate::rain_style::RainStyle;
use crate::runtime::MonolithSize;

fn args_with_config_result(config: &str, cli: &[&str]) -> Result<Args, String> {
    let mut path = std::env::temp_dir();
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after unix epoch")
        .as_nanos();
    path.push(format!(
        "cosmostrix-profile-test-{}-{unique}.conf",
        std::process::id(),
    ));
    std::fs::write(&path, config).expect("write temp config");

    let path_string = path.to_string_lossy().into_owned();
    let mut argv = vec!["cosmostrix", "--config", path_string.as_str()];
    argv.extend_from_slice(cli);

    let cmd = Args::command();
    let matches = cmd.get_matches_from(argv);
    let mut args = Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    let result = apply_config_and_runtime_defaults(&matches, &mut args).map(|()| args);

    let _ = std::fs::remove_file(path);
    result
}

fn args_with_config(config: &str, cli: &[&str]) -> Args {
    args_with_config_result(config, cli).expect("apply profile config")
}

fn nightcore_config() -> &'static str {
    "profile.nightcore.base = monolith\n\
     profile.nightcore.color = purple\n\
     profile.nightcore.charset = binary\n\
     profile.nightcore.speed = 24\n\
     profile.nightcore.density = 0.70\n\
     profile.nightcore.glitch-level = subtle\n\
     profile.nightcore.monolith-size = large\n"
}

#[test]
fn cli_profile_loads_user_profile_from_config() {
    let args = args_with_config(nightcore_config(), &["--profile", "nightcore"]);
    assert_eq!(args.profile.as_deref(), Some("nightcore"));
    assert_eq!(args.scene.as_deref(), Some("monolith"));
    assert_eq!(args.color, "purple");
    assert_eq!(args.charset, "binary");
    assert_eq!(args.speed, 24.0);
    assert!((args.density - 0.70).abs() < f32::EPSILON);
    assert_eq!(args.glitch_level, GlitchLevel::Subtle);
    assert_eq!(args.monolith_size, MonolithSize::Large);
    assert_eq!(
        args.scene
            .as_deref()
            .and_then(crate::scene::rain_style_for_scene),
        Some(RainStyle::Monolith)
    );
}

#[test]
fn profile_base_monolith_applies_monolith_foundation() {
    let args = args_with_config(
        "profile.nightcore.base = monolith\n",
        &["--profile", "nightcore"],
    );
    assert_eq!(args.scene.as_deref(), Some("monolith"));
    assert_eq!(args.color, "cosmos");
    assert_eq!(args.speed, 20.0);
    assert_eq!(args.glitch_level, GlitchLevel::Subtle);
}

#[test]
fn explicit_cli_flags_override_profile_values() {
    let args = args_with_config(
        nightcore_config(),
        &[
            "--profile",
            "nightcore",
            "--speed",
            "30",
            "--color",
            "green",
        ],
    );
    assert_eq!(args.color, "green");
    assert_eq!(args.speed, 30.0);
    assert!((args.density - 0.70).abs() < f32::EPSILON);
    assert_eq!(args.monolith_size, MonolithSize::Large);
}

#[test]
fn config_profile_applies_after_config_scene() {
    let config = format!(
        "scene = signal\nprofile = nightcore\n{}",
        nightcore_config()
    );
    let args = args_with_config(&config, &[]);
    assert_eq!(args.scene.as_deref(), Some("monolith"));
    assert_eq!(args.color, "purple");
    assert_eq!(args.speed, 24.0);
}

#[test]
fn cli_profile_overrides_cli_scene_for_profile_foundation() {
    let args = args_with_config(
        nightcore_config(),
        &["--scene", "signal", "--profile", "nightcore"],
    );
    assert_eq!(args.scene.as_deref(), Some("monolith"));
    assert_eq!(args.color, "purple");
    assert_eq!(args.speed, 24.0);
}

#[test]
fn unknown_cli_profile_has_clear_error() {
    let err = args_with_config_result(nightcore_config(), &["--profile", "unknown"]).unwrap_err();
    assert!(err.contains("error: invalid profile: unknown"));
    assert!(err.contains("expected one of: nightcore"));
}

#[test]
fn invalid_profile_values_are_ignored_cleanly() {
    let config = "profile.bad.base = monolith\n\
                  profile.bad.color = not-a-color\n\
                  profile.bad.speed = 0\n\
                  profile.bad.density = nope\n";
    let args = args_with_config(config, &["--profile", "bad"]);
    assert_eq!(args.scene.as_deref(), Some("monolith"));
    assert_eq!(args.color, "cosmos");
    assert_eq!(args.speed, 20.0);
    assert_eq!(args.density, 0.75);
}

#[test]
fn existing_config_without_profiles_still_works() {
    let args = args_with_config("scene = signal\ncolor = cyan\n", &[]);
    assert_eq!(args.profile, None);
    assert_eq!(args.scene.as_deref(), Some("signal"));
    assert_eq!(args.color, "cyan");
}

#[test]
fn default_plain_runtime_profile_remains_monolith() {
    let args = args_with_config("", &[]);
    assert_eq!(args.scene.as_deref(), Some("monolith"));
    assert_eq!(args.color, "cosmos");
    assert_eq!(args.speed, 20.0);
}

// ---------------------------------------------------------------------------
// Color precedence vs auto-drift clarity tests
// ---------------------------------------------------------------------------

#[test]
fn config_color_overridden_by_config_preset_is_precedence_not_drift() {
    // When a config file sets color = sun AND preset = cinematic / scene = monolith,
    // the preset/scene color overrides per the documented precedence chain.
    // This is NOT auto-color-drift; auto_color_drift remains false.
    let args = args_with_config(
        "color = sun\npreset = cinematic\nscene = monolith\nauto-color-drift = false\n",
        &[],
    );
    // auto_color_drift must still be false — the color change is from precedence
    assert!(
        !args.auto_color_drift,
        "auto_color_drift must remain false; color change is from precedence, not drift"
    );
    // color = sun in config (step 2) is overridden by config preset (step 3)
    // or config scene (step 4), so the final color is NOT sun
    assert_ne!(
        args.color, "sun",
        "config color=sun must be overridden by config preset/scene per precedence rules"
    );
}

#[test]
fn profile_color_resolves_sun_after_preset_and_scene() {
    // Config profile color should override preset/scene color because
    // config profile (step 5) has higher precedence than config preset (step 3)
    // and config scene (step 4).
    let args = args_with_config(
        "preset = cinematic\n\
         scene = monolith\n\
         profile = nightcore\n\
         profile.nightcore.base = monolith\n\
         profile.nightcore.color = sun\n\
         profile.nightcore.charset = binary\n\
         profile.nightcore.speed = 24\n\
         profile.nightcore.density = 0.70\n\
         profile.nightcore.glitch-level = subtle\n\
         profile.nightcore.monolith-size = large\n",
        &[],
    );
    assert_eq!(
        args.color, "sun",
        "profile color must override preset/scene color per precedence"
    );
    assert!(
        !args.auto_color_drift,
        "auto_color_drift must default false"
    );
}

#[test]
fn cli_color_wins_over_config_preset_and_scene() {
    // CLI --color (step 10) is the highest precedence and always wins.
    let args = args_with_config(
        "preset = cinematic\nscene = monolith\n",
        &["--color", "sun"],
    );
    assert_eq!(
        args.color, "sun",
        "CLI --color must override config preset/scene"
    );
}

// ---------------------------------------------------------------------------
// Phase 10.5: Profile atmosphere smoke hardening tests
// ---------------------------------------------------------------------------

fn atmosphere_config_profile(name: &str, mode: &str, regime: &str) -> String {
    format!(
        "profile.{name}.base = monolith\n\
         profile.{name}.color = purple\n\
         profile.{name}.charset = binary\n\
         profile.{name}.speed = 24\n\
         profile.{name}.density = 0.70\n\
         profile.{name}.glitch-level = subtle\n\
         profile.{name}.monolith-size = large\n\
         profile.{name}.atmosphere-mode = {mode}\n\
         profile.{name}.atmosphere-regime = {regime}\n"
    )
}

#[test]
fn profile_controlled_live_pulse_works() {
    let config = atmosphere_config_profile("atmo1", "controlled-live", "pulse");
    let args = args_with_config(&config, &["--profile", "atmo1"]);
    assert_eq!(args.atmosphere_mode_str.as_deref(), Some("controlled-live"));
    assert_eq!(args.atmosphere_regime_str.as_deref(), Some("pulse"));
}

#[test]
fn profile_controlled_live_signal_works() {
    let config = atmosphere_config_profile("atmo2", "controlled-live", "signal");
    let args = args_with_config(&config, &["--profile", "atmo2"]);
    assert_eq!(args.atmosphere_mode_str.as_deref(), Some("controlled-live"));
    assert_eq!(args.atmosphere_regime_str.as_deref(), Some("signal"));
}

#[test]
fn profile_atmosphere_mode_overrides_base_config_mode() {
    // Base config sets controlled-live, profile overrides to disabled
    let config = format!(
        "atmosphere-mode = controlled-live\n\
         atmosphere-regime = pulse\n\
         {}",
        atmosphere_config_profile("atmo3", "disabled", "calm")
    );
    let args = args_with_config(&config, &["--profile", "atmo3"]);
    assert_eq!(args.atmosphere_mode_str.as_deref(), Some("disabled"));
    assert_eq!(args.atmosphere_regime_str.as_deref(), Some("calm"));
}

#[test]
fn profile_atmosphere_regime_overrides_base_config_regime() {
    // Base config sets pulse, profile overrides to signal
    let config = format!(
        "atmosphere-mode = controlled-live\n\
         atmosphere-regime = pulse\n\
         {}",
        atmosphere_config_profile("atmo4", "controlled-live", "signal")
    );
    let args = args_with_config(&config, &["--profile", "atmo4"]);
    assert_eq!(args.atmosphere_regime_str.as_deref(), Some("signal"));
}

#[test]
fn profile_disabled_overrides_base_controlled_live() {
    // Base config sets controlled-live, profile overrides to disabled
    let config = format!(
        "atmosphere-mode = controlled-live\n\
         atmosphere-regime = pulse\n\
         {}",
        atmosphere_config_profile("atmo5", "disabled", "calm")
    );
    let args = args_with_config(&config, &["--profile", "atmo5"]);
    assert_eq!(args.atmosphere_mode_str.as_deref(), Some("disabled"));
}

#[test]
fn profile_calm_results_in_identity_modulation() {
    let config = atmosphere_config_profile("atmo6", "controlled-live", "calm");
    let args = args_with_config(&config, &["--profile", "atmo6"]);
    let mode = crate::config_apply::resolve_atmosphere_mode(args.atmosphere_mode_str.as_deref());
    let regime =
        crate::config_apply::resolve_atmosphere_regime(args.atmosphere_regime_str.as_deref());
    let ctrl = crate::atmosphere::AtmosphereController::new();
    let app = ctrl.build_application();
    let modulation = crate::atmosphere_apply::apply_application(&app, mode);
    let shadow = crate::atmosphere_shadow::shadow_metrics_from_mode_and_regime(mode, regime);
    // Calm always produces identity regardless of mode
    assert!(
        modulation.is_identity(),
        "calm must produce identity modulation"
    );
    assert!(shadow.is_identity(), "calm must produce identity shadow");
}

#[test]
fn profile_storm_is_rejected_or_ignored_cleanly() {
    // Storm is not config-safe — should be rejected at profile parse layer
    let config = atmosphere_config_profile("atmo7", "controlled-live", "storm");
    let args = args_with_config(&config, &["--profile", "atmo7"]);
    // storm should be rejected, so regime_str remains None or calm
    assert_eq!(
        args.atmosphere_regime_str.as_deref(),
        None,
        "storm must be rejected in profile"
    );
}

#[test]
fn profile_controlled_live_signal_produces_shadow_risk_whisper() {
    let config = atmosphere_config_profile("atmo8", "controlled-live", "signal");
    let args = args_with_config(&config, &["--profile", "atmo8"]);
    let mode = crate::config_apply::resolve_atmosphere_mode(args.atmosphere_mode_str.as_deref());
    let regime =
        crate::config_apply::resolve_atmosphere_regime(args.atmosphere_regime_str.as_deref());
    let shadow = crate::atmosphere_shadow::shadow_metrics_from_mode_and_regime(mode, regime);
    assert_eq!(shadow.risk_label(), "whisper");
}

#[test]
fn profile_controlled_live_monolith_pressure_produces_shadow_risk_whisper() {
    let config = atmosphere_config_profile("atmo9", "controlled-live", "monolith-pressure");
    let args = args_with_config(&config, &["--profile", "atmo9"]);
    let mode = crate::config_apply::resolve_atmosphere_mode(args.atmosphere_mode_str.as_deref());
    let regime =
        crate::config_apply::resolve_atmosphere_regime(args.atmosphere_regime_str.as_deref());
    let shadow = crate::atmosphere_shadow::shadow_metrics_from_mode_and_regime(mode, regime);
    assert_eq!(shadow.risk_label(), "whisper");
}

#[test]
fn cli_color_sun_wins_even_when_profile_sets_scene_atmosphere() {
    // Profile sets controlled-live + pulse, but CLI --color sun still wins
    let config = atmosphere_config_profile("atmo10", "controlled-live", "pulse");
    let args = args_with_config(&config, &["--profile", "atmo10", "--color", "sun"]);
    assert_eq!(args.color, "sun", "CLI --color must win over profile");
    assert_eq!(args.atmosphere_mode_str.as_deref(), Some("controlled-live"));
}
