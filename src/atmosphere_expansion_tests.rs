// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

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
    use crate::zactrix_engine::{ComputeParallelism, ZactrixSystemConfig};
    let sys = ZactrixSystemConfig::default();
    assert_eq!(sys.compute_parallelism, ComputeParallelism::Disabled);
    assert_eq!(sys.compute_parallelism.as_str(), "disabled");
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
    use crate::zactrix_engine::{ComputeParallelism, ZactrixSystemConfig};
    let sys = ZactrixSystemConfig::default();
    assert_eq!(sys.compute_parallelism, ComputeParallelism::Disabled);
}

#[test]
fn v46_diag_terminal_writer_single_owner() {
    use crate::zactrix_engine::{RenderPlan, TerminalWriterPolicy};
    let render = RenderPlan::default();
    assert_eq!(render.writer_policy, TerminalWriterPolicy::SingleOwner);
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
