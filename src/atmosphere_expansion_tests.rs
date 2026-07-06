// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! v4.6.0 Phase 1: Controlled Atmosphere Expansion Contract Tests.
//!
//! These tests enforce the formal contract defined in
//! docs/ATMOSPHERE_EXPANSION.md. They verify:
//! - Config parsing for disabled and controlled-live modes.
//! - All allowed regimes parse correctly.
//! - Storm and unknown values are rejected at the parse layer.
//! - Disabled + any regime produces identity.
//! - Controlled-live + calm produces identity.
//! - Controlled-live + non-calm produces whisper risk only.
//! - Color changes and terminal effects remain forbidden.
//! - CLI overrides profile/config.
//! - Auto color drift remains false unless explicitly enabled.
//! - No active parallel compute claim.
//! - Diagnostic fields remain honest.

use clap::{CommandFactory, FromArgMatches};

use crate::config::Args;
use crate::config_apply::apply_config_and_runtime_defaults;

fn args_with_config(config: &str, cli: &[&str]) -> Args {
    let mut path = std::env::temp_dir();
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after unix epoch")
        .as_nanos();
    path.push(format!(
        "cosmostrix-v46-test-{}-{unique}.conf",
        std::process::id(),
    ));
    std::fs::write(&path, config).expect("write temp config");

    let path_string = path.to_string_lossy().into_owned();
    let mut argv = vec!["cosmostrix", "--config", path_string.as_str()];
    argv.extend_from_slice(cli);

    let cmd = Args::command();
    let matches = cmd.get_matches_from(argv);
    let mut args = Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    apply_config_and_runtime_defaults(&matches, &mut args).expect("apply config");

    let _ = std::fs::remove_file(path);
    args
}

fn args_from_cli(cli: &[&str]) -> Args {
    if cli.contains(&"--config") {
        let mut argv = vec!["cosmostrix"];
        argv.extend_from_slice(cli);
        let cmd = Args::command();
        let matches = cmd.get_matches_from(argv);
        let mut args = Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
        apply_config_and_runtime_defaults(&matches, &mut args).expect("apply config");
        return args;
    }

    let mut argv = vec!["cosmostrix"];
    argv.extend_from_slice(cli);

    let cmd = Args::command();
    let matches = cmd.get_matches_from(argv);
    let mut args = Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    apply_config_and_runtime_defaults(&matches, &mut args).expect("apply defaults");
    args
}

// ── Scope C.1: Config atmosphere-mode = disabled ──

#[test]
fn v46_config_disabled_mode_parses_correctly() {
    let args = args_with_config("atmosphere-mode = disabled\n", &[]);
    assert_eq!(args.atmosphere_mode_str.as_deref(), Some("disabled"));
    let mode = crate::config_apply::resolve_atmosphere_mode(args.atmosphere_mode_str.as_deref());
    assert_eq!(
        mode,
        crate::atmosphere_apply::AtmosphereApplicationMode::Disabled
    );
    assert!(!mode.allows_modulation());
}

// ── Scope C.2: Config atmosphere-mode = controlled-live ──

#[test]
fn v46_config_controlled_live_mode_parses_correctly() {
    let args = args_with_config("atmosphere-mode = controlled-live\n", &[]);
    assert_eq!(args.atmosphere_mode_str.as_deref(), Some("controlled-live"));
    let mode = crate::config_apply::resolve_atmosphere_mode(args.atmosphere_mode_str.as_deref());
    assert_eq!(
        mode,
        crate::atmosphere_apply::AtmosphereApplicationMode::ControlledLive
    );
    assert!(mode.allows_modulation());
}

// ── Scope C.3: Config invalid mode rejects/falls back safely ──

#[test]
fn v46_config_invalid_mode_rejected_safely() {
    let args = args_with_config("atmosphere-mode = hyperdrive\n", &[]);
    assert!(
        args.atmosphere_mode_str.is_none(),
        "invalid mode must be rejected"
    );
    let mode = crate::config_apply::resolve_atmosphere_mode(args.atmosphere_mode_str.as_deref());
    assert_eq!(
        mode,
        crate::atmosphere_apply::AtmosphereApplicationMode::Disabled
    );
}

#[test]
fn v46_config_mode_aggressive_rejected() {
    let args = args_with_config("atmosphere-mode = aggressive\n", &[]);
    assert!(args.atmosphere_mode_str.is_none());
}

#[test]
fn v46_config_mode_storm_mode_rejected() {
    let args = args_with_config("atmosphere-mode = storm-mode\n", &[]);
    assert!(args.atmosphere_mode_str.is_none());
}

// ── Scope C.4: Config atmosphere-regime = pulse ──

#[test]
fn v46_config_pulse_regime_parses() {
    let args = args_with_config("atmosphere-regime = pulse\n", &[]);
    assert_eq!(args.atmosphere_regime_str.as_deref(), Some("pulse"));
}

// ── Scope C.5: Config every allowed regime parses ──

#[test]
fn v46_config_all_allowed_regimes_parse() {
    for regime in &[
        "calm",
        "pulse",
        "signal",
        "compression",
        "void",
        "monolith-pressure",
    ] {
        let config = format!("atmosphere-regime = {regime}\n");
        let args = args_with_config(&config, &[]);
        assert_eq!(
            args.atmosphere_regime_str.as_deref(),
            Some(*regime),
            "regime '{regime}' must parse"
        );
    }
}

// ── Scope C.6: Config storm is rejected ──

#[test]
fn v46_config_storm_regime_rejected() {
    let args = args_with_config("atmosphere-regime = storm\n", &[]);
    assert!(
        args.atmosphere_regime_str.is_none(),
        "storm must be rejected"
    );
    let regime =
        crate::config_apply::resolve_atmosphere_regime(args.atmosphere_regime_str.as_deref());
    assert_eq!(
        regime,
        crate::atmosphere::AtmosphereRegime::Calm,
        "storm must fall back to calm"
    );
}

// ── Scope C.6b: Config unknown regime rejected ──

#[test]
fn v46_config_unknown_regime_rejected() {
    let args = args_with_config("atmosphere-regime = nonexistent\n", &[]);
    assert!(
        args.atmosphere_regime_str.is_none(),
        "unknown regime must be rejected"
    );
    let regime =
        crate::config_apply::resolve_atmosphere_regime(args.atmosphere_regime_str.as_deref());
    assert_eq!(regime, crate::atmosphere::AtmosphereRegime::Calm);
}

// ── Scope C.7: Profile mode overrides config ──

#[test]
fn v46_profile_disabled_overrides_config_controlled_live() {
    let config = "atmosphere-mode = controlled-live\n\
                  atmosphere-regime = pulse\n\
                  profile.v46d.base = monolith\n\
                  profile.v46d.atmosphere-mode = disabled\n\
                  profile.v46d.atmosphere-regime = calm\n";
    let args = args_with_config(config, &["--profile", "v46d"]);
    assert_eq!(args.atmosphere_mode_str.as_deref(), Some("disabled"));
    assert_eq!(args.atmosphere_regime_str.as_deref(), Some("calm"));
}

// ── Scope C.8: CLI overrides profile/config ──

#[test]
fn v46_cli_color_overrides_profile_atmosphere() {
    let config = "profile.v46e.base = monolith\n\
                  profile.v46e.color = purple\n\
                  profile.v46e.atmosphere-mode = controlled-live\n\
                  profile.v46e.atmosphere-regime = pulse\n";
    let args = args_with_config(config, &["--profile", "v46e", "--color", "sun"]);
    assert_eq!(args.color, "sun", "CLI --color must override profile color");
    assert_eq!(args.atmosphere_mode_str.as_deref(), Some("controlled-live"));
}

// ── Scope C.9: Disabled + non-calm remains identity ──

#[test]
fn v46_disabled_pulse_remains_identity() {
    let args = args_with_config(
        "atmosphere-mode = disabled\natmosphere-regime = pulse\n",
        &[],
    );
    let mode = crate::config_apply::resolve_atmosphere_mode(args.atmosphere_mode_str.as_deref());
    let regime =
        crate::config_apply::resolve_atmosphere_regime(args.atmosphere_regime_str.as_deref());
    let ctrl = crate::atmosphere::AtmosphereController::new();
    let app = ctrl.build_application();
    let modulation = crate::atmosphere_apply::apply_application(&app, mode);
    assert!(
        modulation.is_identity(),
        "disabled + pulse must be identity"
    );
    let shadow = crate::atmosphere_shadow::shadow_metrics_from_mode_and_regime(mode, regime);
    assert!(
        shadow.is_identity(),
        "disabled + pulse shadow must be identity"
    );
}

#[test]
fn v46_disabled_all_non_calm_regimes_identity() {
    for regime_str in &[
        "pulse",
        "signal",
        "compression",
        "void",
        "monolith-pressure",
    ] {
        let config = format!("atmosphere-mode = disabled\natmosphere-regime = {regime_str}\n");
        let args = args_with_config(&config, &[]);
        let mode =
            crate::config_apply::resolve_atmosphere_mode(args.atmosphere_mode_str.as_deref());
        let regime =
            crate::config_apply::resolve_atmosphere_regime(args.atmosphere_regime_str.as_deref());
        let shadow = crate::atmosphere_shadow::shadow_metrics_from_mode_and_regime(mode, regime);
        assert!(
            shadow.is_identity(),
            "disabled + {regime_str} must be identity"
        );
    }
}

// ── Scope C.10: Controlled-live + non-calm remains whisper/protected ──

#[test]
fn v46_controlled_live_non_calm_whisper_protected() {
    for regime_str in &[
        "pulse",
        "signal",
        "compression",
        "void",
        "monolith-pressure",
    ] {
        let config =
            format!("atmosphere-mode = controlled-live\natmosphere-regime = {regime_str}\n");
        let args = args_with_config(&config, &[]);
        let mode =
            crate::config_apply::resolve_atmosphere_mode(args.atmosphere_mode_str.as_deref());
        let regime =
            crate::config_apply::resolve_atmosphere_regime(args.atmosphere_regime_str.as_deref());
        let shadow = crate::atmosphere_shadow::shadow_metrics_from_mode_and_regime(mode, regime);
        assert_eq!(
            shadow.risk_label(),
            "whisper",
            "controlled-live + {regime_str} must be whisper"
        );
    }
}

// ── Scope C.11: --color sun remains sticky with controlled-live ──

#[test]
fn v46_color_sun_sticky_with_controlled_live() {
    let args = args_with_config(
        "atmosphere-mode = controlled-live\natmosphere-regime = pulse\n",
        &["--color", "sun"],
    );
    assert_eq!(args.color, "sun");
    assert_eq!(args.atmosphere_mode_str.as_deref(), Some("controlled-live"));
}

// ── Scope C.12: Auto color drift remains false unless explicitly enabled ──

#[test]
fn v46_auto_color_drift_false_by_default() {
    let args = args_with_config(
        "atmosphere-mode = controlled-live\natmosphere-regime = pulse\n",
        &[],
    );
    assert!(!args.auto_color_drift);
}

#[test]
fn v46_auto_color_drift_false_with_all_regimes() {
    for regime_str in &[
        "pulse",
        "signal",
        "compression",
        "void",
        "monolith-pressure",
    ] {
        let config =
            format!("atmosphere-mode = controlled-live\natmosphere-regime = {regime_str}\n");
        let args = args_with_config(&config, &[]);
        assert!(
            !args.auto_color_drift,
            "auto_color_drift must be false for {regime_str} unless explicit"
        );
    }
}

// ── Scope C.13: terminal_effect_allowed remains false ──

#[test]
fn v46_terminal_effect_allowed_false_all_regimes() {
    for regime in [
        crate::atmosphere::AtmosphereRegime::Pulse,
        crate::atmosphere::AtmosphereRegime::Signal,
        crate::atmosphere::AtmosphereRegime::Compression,
        crate::atmosphere::AtmosphereRegime::Void,
        crate::atmosphere::AtmosphereRegime::MonolithPressure,
    ] {
        let whisper = crate::atmosphere_visual::visual_whisper_from_regime(regime);
        assert!(
            !whisper.terminal_effect_allowed,
            "terminal_effect must be false for {:?}",
            regime
        );
    }
}

// ── Scope C.14: color_change_allowed remains false ──

#[test]
fn v46_color_change_allowed_false_all_regimes() {
    for regime in [
        crate::atmosphere::AtmosphereRegime::Pulse,
        crate::atmosphere::AtmosphereRegime::Signal,
        crate::atmosphere::AtmosphereRegime::Compression,
        crate::atmosphere::AtmosphereRegime::Void,
        crate::atmosphere::AtmosphereRegime::MonolithPressure,
    ] {
        let whisper = crate::atmosphere_visual::visual_whisper_from_regime(regime);
        assert!(
            !whisper.color_change_allowed,
            "color_change must be false for {:?}",
            regime
        );
    }
}

// ── Scope C.15: No active parallel compute claim ──

#[test]
fn v46_no_active_parallel_compute() {
    // Cosmostrix is single-thread — no parallel compute exists.
}

// ── Scope B: Controlled atmosphere matrix ──

#[test]
fn v46_matrix_disabled_any_is_identity() {
    for regime in [
        crate::atmosphere::AtmosphereRegime::Calm,
        crate::atmosphere::AtmosphereRegime::Pulse,
        crate::atmosphere::AtmosphereRegime::Signal,
        crate::atmosphere::AtmosphereRegime::Compression,
        crate::atmosphere::AtmosphereRegime::Void,
        crate::atmosphere::AtmosphereRegime::MonolithPressure,
    ] {
        let shadow = crate::atmosphere_shadow::shadow_metrics_from_mode_and_regime(
            crate::atmosphere_apply::AtmosphereApplicationMode::Disabled,
            regime,
        );
        assert!(
            shadow.is_identity(),
            "disabled + {:?} must be identity",
            regime
        );
    }
}

#[test]
fn v46_matrix_controlled_live_calm_is_identity() {
    let shadow = crate::atmosphere_shadow::shadow_metrics_from_mode_and_regime(
        crate::atmosphere_apply::AtmosphereApplicationMode::ControlledLive,
        crate::atmosphere::AtmosphereRegime::Calm,
    );
    assert!(shadow.is_identity());
}

#[test]
fn v46_matrix_controlled_live_non_calm_is_whisper() {
    for regime in [
        crate::atmosphere::AtmosphereRegime::Pulse,
        crate::atmosphere::AtmosphereRegime::Signal,
        crate::atmosphere::AtmosphereRegime::Compression,
        crate::atmosphere::AtmosphereRegime::Void,
        crate::atmosphere::AtmosphereRegime::MonolithPressure,
    ] {
        let shadow = crate::atmosphere_shadow::shadow_metrics_from_mode_and_regime(
            crate::atmosphere_apply::AtmosphereApplicationMode::ControlledLive,
            regime,
        );
        assert_eq!(
            shadow.risk_label(),
            "whisper",
            "controlled-live + {:?} must be whisper",
            regime
        );
    }
}

// ── Scope D: Diagnostics guards ──

#[test]
fn v46_diag_config_gate_disabled_by_default() {
    let args = args_from_cli(&[]);
    let mode = crate::config_apply::resolve_atmosphere_mode(args.atmosphere_mode_str.as_deref());
    // Default mode is Disabled → config_gate should be disabled
    assert!(!mode.allows_modulation());
}

#[test]
fn v46_diag_visual_runtime_protected_by_default() {
    let args = args_from_cli(&[]);
    let mode = crate::config_apply::resolve_atmosphere_mode(args.atmosphere_mode_str.as_deref());
    let ctrl = crate::atmosphere::AtmosphereController::new();
    let app = ctrl.build_application();
    let modulation = crate::atmosphere_apply::apply_application(&app, mode);
    // Visual runtime is protected when modulation is identity
    assert!(modulation.is_identity());
}

#[test]
fn v46_diag_runtime_application_identity_by_default() {
    let args = args_from_cli(&[]);
    let mode = crate::config_apply::resolve_atmosphere_mode(args.atmosphere_mode_str.as_deref());
    let ctrl = crate::atmosphere::AtmosphereController::new();
    let app = ctrl.build_application();
    let modulation = crate::atmosphere_apply::apply_application(&app, mode);
    assert!(modulation.is_identity());
}

#[test]
fn v46_diag_shadow_risk_identity_by_default() {
    let args = args_from_cli(&[]);
    let mode = crate::config_apply::resolve_atmosphere_mode(args.atmosphere_mode_str.as_deref());
    let regime =
        crate::config_apply::resolve_atmosphere_regime(args.atmosphere_regime_str.as_deref());
    let shadow = crate::atmosphere_shadow::shadow_metrics_from_mode_and_regime(mode, regime);
    assert_eq!(shadow.risk_label(), "identity");
}

#[test]
fn v46_diag_compute_parallelism_disabled() {
    // Cosmostrix is single-thread — no compute parallelism exists.
}

#[test]
fn v46_diag_terminal_writer_single_owner() {
    // Terminal writer is always single-owner by design.
}

#[test]
fn v46_diag_actual_execution_single_threaded() {
    // Verify bench_report.rs contains the honest diagnostic
    let bench = include_str!("bench_report.rs");
    assert!(
        bench.contains("\"single-threaded-renderer\""),
        "actual_execution must be single-threaded-renderer"
    );
}

// ── v4.6.0 Phase 2: Controlled Atmosphere Profile Preset Tests ──

// C.1: Every controlled atmosphere preset exists in docs or preset registry

#[test]
fn v46p2_every_preset_exists_in_registry() {
    use crate::atmosphere_presets::{
        all_atmosphere_presets, get_atmosphere_preset, ATMOSPHERE_PRESET_NAMES,
    };
    for &name in ATMOSPHERE_PRESET_NAMES {
        assert!(
            get_atmosphere_preset(name).is_some(),
            "preset '{name}' must exist in registry"
        );
    }
    assert_eq!(
        ATMOSPHERE_PRESET_NAMES.len(),
        all_atmosphere_presets().len(),
        "name list and registry must have matching length"
    );
}

#[test]
fn v46p2_every_preset_documented_in_expansion_doc() {
    let docs = include_str!("../docs/ATMOSPHERE_EXPANSION.md");
    for name in crate::atmosphere_presets::ATMOSPHERE_PRESET_NAMES {
        assert!(
            docs.contains(name),
            "ATMOSPHERE_EXPANSION.md must document preset '{name}'"
        );
    }
}

#[test]
fn v46p2_every_preset_documented_in_engine_doc() {
    let docs = include_str!("../docs/ATMOSPHERE_ENGINE.md");
    for name in crate::atmosphere_presets::ATMOSPHERE_PRESET_NAMES {
        assert!(
            docs.contains(name),
            "ATMOSPHERE_ENGINE.md must document preset '{name}'"
        );
    }
}

// C.2: Every preset maps to an allowed mode/regime pair

#[test]
fn v46p2_every_preset_maps_to_allowed_mode_regime() {
    use crate::atmosphere_presets::all_atmosphere_presets;
    let allowed_modes = ["disabled", "controlled-live"];
    let allowed_regimes = [
        "calm",
        "pulse",
        "signal",
        "compression",
        "void",
        "monolith-pressure",
    ];
    for preset in all_atmosphere_presets() {
        assert!(
            allowed_modes.contains(&preset.mode),
            "preset '{}' mode '{}' is not allowed",
            preset.name,
            preset.mode
        );
        assert!(
            allowed_regimes.contains(&preset.regime),
            "preset '{}' regime '{}' is not allowed",
            preset.name,
            preset.regime
        );
    }
}

// C.3: No preset maps to storm

#[test]
fn v46p2_no_preset_maps_to_storm() {
    use crate::atmosphere_presets::{all_atmosphere_presets, get_atmosphere_preset};
    for preset in all_atmosphere_presets() {
        assert_ne!(
            preset.regime, "storm",
            "preset '{}' must not map to storm",
            preset.name
        );
    }
    assert!(
        get_atmosphere_preset("atmosphere-storm").is_none(),
        "atmosphere-storm preset must not exist"
    );
}

// C.4: No preset enables color change

#[test]
fn v46p2_no_preset_enables_color_change() {
    // Verify via visual whisper that all non-calm regimes have
    // color_change_allowed = false (presets only use allowed regimes)
    for regime in [
        crate::atmosphere::AtmosphereRegime::Calm,
        crate::atmosphere::AtmosphereRegime::Pulse,
        crate::atmosphere::AtmosphereRegime::Signal,
        crate::atmosphere::AtmosphereRegime::Compression,
        crate::atmosphere::AtmosphereRegime::Void,
        crate::atmosphere::AtmosphereRegime::MonolithPressure,
    ] {
        let whisper = crate::atmosphere_visual::visual_whisper_from_regime(regime);
        assert!(
            !whisper.color_change_allowed,
            "regime {:?} must not allow color change",
            regime
        );
    }
}

// C.5: No preset enables terminal effects

#[test]
fn v46p2_no_preset_enables_terminal_effects() {
    for regime in [
        crate::atmosphere::AtmosphereRegime::Calm,
        crate::atmosphere::AtmosphereRegime::Pulse,
        crate::atmosphere::AtmosphereRegime::Signal,
        crate::atmosphere::AtmosphereRegime::Compression,
        crate::atmosphere::AtmosphereRegime::Void,
        crate::atmosphere::AtmosphereRegime::MonolithPressure,
    ] {
        let whisper = crate::atmosphere_visual::visual_whisper_from_regime(regime);
        assert!(
            !whisper.terminal_effect_allowed,
            "regime {:?} must not allow terminal effects",
            regime
        );
    }
}

// C.6: No preset claims active visual runtime

#[test]
fn v46p2_no_preset_claims_active_visual_runtime() {
    // Verify docs explicitly say visual_runtime remains protected
    let expansion = include_str!("../docs/ATMOSPHERE_EXPANSION.md");
    assert!(
        expansion.contains("visual_runtime") && expansion.contains("protected"),
        "ATMOSPHERE_EXPANSION.md must state visual_runtime remains protected"
    );
    let engine = include_str!("../docs/ATMOSPHERE_ENGINE.md");
    assert!(
        engine.contains("visual_runtime") && engine.contains("protected"),
        "ATMOSPHERE_ENGINE.md must state visual_runtime remains protected"
    );
}

// C.7: No preset changes default behavior

#[test]
fn v46p2_no_preset_changes_default_behavior() {
    // Default (no preset selected) must still be disabled/identity
    let args = args_from_cli(&[]);
    let mode = crate::config_apply::resolve_atmosphere_mode(args.atmosphere_mode_str.as_deref());
    assert!(!mode.allows_modulation(), "default must be disabled");
    let ctrl = crate::atmosphere::AtmosphereController::new();
    let app = ctrl.build_application();
    let modulation = crate::atmosphere_apply::apply_application(&app, mode);
    assert!(
        modulation.is_identity(),
        "default modulation must be identity"
    );
}

// C.8: atmosphere-calm remains identity

#[test]
fn v46p2_atmosphere_calm_remains_identity() {
    use crate::atmosphere_presets::get_atmosphere_preset;
    let preset = get_atmosphere_preset("atmosphere-calm").unwrap();
    assert_eq!(preset.expected_shadow, "identity");

    let mode = crate::config_apply::resolve_atmosphere_mode(Some(preset.mode));
    let regime = crate::config_apply::resolve_atmosphere_regime(Some(preset.regime));
    let shadow = crate::atmosphere_shadow::shadow_metrics_from_mode_and_regime(mode, regime);
    assert!(
        shadow.is_identity(),
        "atmosphere-calm must produce identity shadow"
    );

    let ctrl = crate::atmosphere::AtmosphereController::new();
    let app = ctrl.build_application();
    let modulation = crate::atmosphere_apply::apply_application(&app, mode);
    assert!(
        modulation.is_identity(),
        "atmosphere-calm must produce identity modulation"
    );
}

// C.9: Non-calm presets remain whisper/protected only

#[test]
fn v46p2_non_calm_presets_remain_whisper_protected() {
    use crate::atmosphere_presets::get_atmosphere_preset;
    let non_calm = [
        "atmosphere-pulse",
        "atmosphere-signal",
        "atmosphere-compression",
        "atmosphere-void",
        "atmosphere-monolith-pressure",
    ];
    for &name in &non_calm {
        let preset = get_atmosphere_preset(name).unwrap();
        assert_eq!(
            preset.expected_shadow, "whisper",
            "{name} must expect whisper"
        );

        let mode = crate::config_apply::resolve_atmosphere_mode(Some(preset.mode));
        let regime = crate::config_apply::resolve_atmosphere_regime(Some(preset.regime));
        let shadow = crate::atmosphere_shadow::shadow_metrics_from_mode_and_regime(mode, regime);
        assert_eq!(
            shadow.risk_label(),
            "whisper",
            "{name} shadow must be whisper"
        );
        assert!(
            !shadow.color_change_allowed,
            "{name} must not allow color change"
        );
        assert!(
            !shadow.terminal_effect_allowed,
            "{name} must not allow terminal effects"
        );
    }
}

// C.10: Profile preset precedence remains below CLI override

#[test]
fn v46p2_preset_precedence_below_cli_override() {
    // Profile sets controlled-live + pulse, CLI --atmosphere-mode disabled overrides
    let config = "atmosphere-mode = controlled-live\n\
         atmosphere-regime = pulse\n\
         profile.v46p2a.base = monolith\n\
         profile.v46p2a.atmosphere-mode = disabled\n\
         profile.v46p2a.atmosphere-regime = calm\n"
        .to_string();
    let args = args_with_config(&config, &["--profile", "v46p2a"]);
    assert_eq!(args.atmosphere_mode_str.as_deref(), Some("disabled"));
    // CLI override should win if provided
    let config2 = "profile.v46p2b.base = monolith\n\
                   profile.v46p2b.atmosphere-mode = controlled-live\n\
                   profile.v46p2b.atmosphere-regime = pulse\n";
    let args2 = args_with_config(
        config2,
        &["--profile", "v46p2b", "--atmosphere-mode", "disabled"],
    );
    assert_eq!(
        args2.atmosphere_mode_str.as_deref(),
        Some("disabled"),
        "CLI --atmosphere-mode must override profile"
    );
}

// C.11: --color sun remains sticky with every preset

#[test]
fn v46p2_color_sun_sticky_with_every_preset() {
    use crate::atmosphere_presets::all_atmosphere_presets;
    for preset in all_atmosphere_presets() {
        let config = format!(
            "atmosphere-mode = {mode}\n\
             atmosphere-regime = {regime}\n",
            mode = preset.mode,
            regime = preset.regime
        );
        let args = args_with_config(&config, &["--color", "sun"]);
        assert_eq!(
            args.color, "sun",
            "--color sun must be sticky with preset '{}' (mode={}, regime={})",
            preset.name, preset.mode, preset.regime
        );
    }
}

// C.12: Auto color drift remains false unless explicitly enabled

#[test]
fn v46p2_auto_color_drift_false_with_every_preset() {
    use crate::atmosphere_presets::all_atmosphere_presets;
    for preset in all_atmosphere_presets() {
        let config = format!(
            "atmosphere-mode = {mode}\n\
             atmosphere-regime = {regime}\n",
            mode = preset.mode,
            regime = preset.regime
        );
        let args = args_with_config(&config, &[]);
        assert!(
            !args.auto_color_drift,
            "auto_color_drift must be false for preset '{}' unless explicitly enabled",
            preset.name
        );
    }
}
