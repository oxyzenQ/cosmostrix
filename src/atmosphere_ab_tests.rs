// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the internal A/B smoke model (Phase 9 / Phase 9.5).

use crate::atmosphere::AtmosphereRegime;
use crate::atmosphere_ab::*;
use crate::atmosphere_apply::AtmosphereApplicationMode;
use crate::atmosphere_shadow::shadow_metrics_from_mode_and_regime;
use crate::atmosphere_visual::AtmosphereVisualWhisper;

// ── Core A/B Tests ────────────────────────────────────────────────────────

#[test]
fn identity_vs_calm_ab_passes_as_identity() {
    let sample = compare_identity_vs_regime(AtmosphereRegime::Calm);
    assert!(sample.passed);
    assert_eq!(sample.risk_label, "identity");
    assert!(sample.baseline_whisper.is_identity());
    assert!(sample.candidate_whisper.is_identity());
    assert!(sample.baseline_shadow.is_identity());
    assert!(sample.candidate_shadow.is_identity());
}

#[test]
fn identity_vs_pulse_controlled_live_passes_as_whisper() {
    let (sample, verdict) = smoke_regime_under_controlled_live(AtmosphereRegime::Pulse);
    assert!(verdict.pass);
    assert_eq!(sample.risk_label, "whisper");
    assert!(!sample.candidate_whisper.color_change_allowed);
    assert!(!sample.candidate_whisper.terminal_effect_allowed);
}

#[test]
fn identity_vs_signal_controlled_live_passes_as_whisper() {
    let (sample, verdict) = smoke_regime_under_controlled_live(AtmosphereRegime::Signal);
    assert!(verdict.pass);
    assert_eq!(sample.risk_label, "whisper");
    assert!(sample.candidate_shadow.max_abs_delta_percent() <= 2.0);
}

#[test]
fn identity_vs_monolith_pressure_controlled_live_passes_as_whisper() {
    let (sample, verdict) = smoke_regime_under_controlled_live(AtmosphereRegime::MonolithPressure);
    assert!(verdict.pass);
    assert!(
        matches!(sample.risk_label, "identity" | "whisper"),
        "expected identity or whisper, got: {}",
        sample.risk_label
    );
}

#[test]
fn storm_ab_is_clamped_and_not_default() {
    let (sample, verdict) = smoke_regime_under_controlled_live(AtmosphereRegime::Storm);
    assert!(verdict.pass, "Storm A/B must pass: {}", verdict.reason);
    assert!(sample.candidate_whisper.is_within_whisper_bounds());
    assert!(!sample.candidate_whisper.terminal_effect_allowed);
}

#[test]
fn void_ab_does_not_collapse_density() {
    let (sample, verdict) = smoke_regime_under_controlled_live(AtmosphereRegime::Void);
    assert!(verdict.pass, "Void A/B must pass: {}", verdict.reason);
    assert!(
        sample.candidate_whisper.density_scale >= AbSafetyThresholds::MIN_DENSITY_SCALE,
        "Void density must not collapse: got {}",
        sample.candidate_whisper.density_scale
    );
}

#[test]
fn color_change_candidate_rejects() {
    let mut whisper = AtmosphereVisualWhisper::identity();
    whisper.color_change_allowed = true;
    let sample = compare_identity_vs_whisper(whisper);
    assert!(!sample.passed, "color_change_allowed must cause rejection");
    assert_eq!(sample.risk_label, "rejected");
}

#[test]
fn terminal_effect_candidate_rejects() {
    let mut whisper = AtmosphereVisualWhisper::identity();
    whisper.terminal_effect_allowed = true;
    let sample = compare_identity_vs_whisper(whisper);
    assert!(
        !sample.passed,
        "terminal_effect_allowed must cause rejection"
    );
    assert_eq!(sample.risk_label, "rejected");
}

#[test]
fn over_bright_candidate_rejects() {
    let mut whisper = AtmosphereVisualWhisper::identity();
    whisper.brightness_scale = 1.05;
    let sample = compare_identity_vs_whisper(whisper);
    assert!(!sample.passed, "over-bright candidate must reject");
}

#[test]
fn glitch_above_whisper_cap_rejects() {
    let mut whisper = AtmosphereVisualWhisper::identity();
    whisper.glitch_pressure = 0.10;
    let sample = compare_identity_vs_whisper(whisper);
    assert!(!sample.passed, "glitch above whisper cap must reject");
}

#[test]
fn ab_smoke_is_deterministic() {
    for _ in 0..50 {
        let s1 = compare_identity_vs_regime(AtmosphereRegime::Storm);
        let s2 = compare_identity_vs_regime(AtmosphereRegime::Storm);
        assert_eq!(s1.speed_delta_percent, s2.speed_delta_percent);
        assert_eq!(s1.density_delta_percent, s2.density_delta_percent);
        assert_eq!(s1.brightness_delta_percent, s2.brightness_delta_percent);
        assert_eq!(s1.trail_energy_delta_percent, s2.trail_energy_delta_percent);
        assert_eq!(s1.glyph_pulse_delta_percent, s2.glyph_pulse_delta_percent);
        assert_eq!(s1.glitch_delta, s2.glitch_delta);
        assert_eq!(s1.risk_label, s2.risk_label);
        assert_eq!(s1.passed, s2.passed);
    }
}

#[test]
fn all_regimes_evaluated_without_panic() {
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
        let _ = compare_identity_vs_regime(regime);
        let _ = smoke_regime_under_controlled_live(regime);
    }
    let results = smoke_all_regimes_under_controlled_live();
    assert_eq!(results.len(), AtmosphereRegime::COUNT);
}

#[test]
fn all_regimes_batch_smoke_passes() {
    let results = smoke_all_regimes_under_controlled_live();
    let failures: Vec<_> = results
        .iter()
        .filter(|(_, _, verdict)| !verdict.pass)
        .collect();
    assert!(
        failures.is_empty(),
        "A/B smoke failures: {:?}",
        failures
            .iter()
            .map(|(r, _, v)| format!("{:?}: {}", r, v.reason))
            .collect::<Vec<_>>()
    );
}

#[test]
fn compression_controlled_live_ab_passes() {
    let (sample, verdict) = smoke_regime_under_controlled_live(AtmosphereRegime::Compression);
    assert!(
        verdict.pass,
        "Compression A/B must pass: {}",
        verdict.reason
    );
    assert!(
        matches!(sample.risk_label, "identity" | "whisper"),
        "Compression risk must be identity or whisper, got: {}",
        sample.risk_label
    );
}

#[test]
fn identity_whisper_ab_passes() {
    let sample = compare_identity_vs_whisper(AtmosphereVisualWhisper::identity());
    assert!(sample.passed);
    assert_eq!(sample.risk_label, "identity");
}

#[test]
fn ab_verdict_struct_correctness() {
    let pass = AtmosphereAbVerdict::pass("test pass");
    assert!(pass.pass);
    assert_eq!(pass.reason, "test pass");
    let reject = AtmosphereAbVerdict::reject("test reject");
    assert!(!reject.pass);
    assert_eq!(reject.reason, "test reject");
    assert_eq!(pass, AtmosphereAbVerdict::pass("test pass"));
    assert_ne!(pass, reject);
}

#[test]
fn identity_vs_identity_deltas_are_zero() {
    let sample = compare_identity_vs_regime(AtmosphereRegime::Calm);
    assert_eq!(sample.speed_delta_percent, 0.0);
    assert_eq!(sample.density_delta_percent, 0.0);
    assert_eq!(sample.brightness_delta_percent, 0.0);
    assert_eq!(sample.trail_energy_delta_percent, 0.0);
    assert_eq!(sample.glyph_pulse_delta_percent, 0.0);
    assert_eq!(sample.glitch_delta, 0.0);
}

#[test]
fn baseline_whisper_always_identity() {
    for regime in [
        AtmosphereRegime::Calm,
        AtmosphereRegime::Pulse,
        AtmosphereRegime::Storm,
        AtmosphereRegime::Void,
        AtmosphereRegime::Signal,
        AtmosphereRegime::Compression,
        AtmosphereRegime::MonolithPressure,
    ] {
        let sample = compare_identity_vs_regime(regime);
        assert!(
            sample.baseline_whisper.is_identity(),
            "baseline whisper must be identity for {:?}",
            regime
        );
        assert!(
            sample.baseline_shadow.is_identity(),
            "baseline shadow must be identity for {:?}",
            regime
        );
    }
}

// ── Default Runtime Guards ────────────────────────────────────────────────

#[test]
fn default_runtime_application_mode_disabled() {
    let shadow = shadow_metrics_from_mode_and_regime(
        AtmosphereApplicationMode::Disabled,
        AtmosphereRegime::Storm,
    );
    assert!(shadow.is_identity());
    assert_eq!(shadow.risk_label(), "identity");
}

#[test]
fn default_effective_runtime_remains_identity() {
    let modulation = crate::atmosphere_apply::apply_application(
        &crate::atmosphere_verifier::AtmosphereApplication::identity(),
        AtmosphereApplicationMode::Disabled,
    );
    assert!(modulation.is_identity());
    let eff = crate::atmosphere_apply::derive_effective_runtime(1.0, 1.0, &modulation);
    assert_eq!(eff.speed, 1.0);
    assert_eq!(eff.density, 1.0);
}

#[test]
fn default_shadow_metrics_remains_identity() {
    let shadow = shadow_metrics_from_mode_and_regime(
        AtmosphereApplicationMode::Disabled,
        AtmosphereRegime::Calm,
    );
    assert!(shadow.is_identity());
    assert_eq!(shadow.risk_label(), "identity");
    assert_eq!(shadow.max_abs_delta_percent(), 0.0);
}

#[test]
fn color_sun_remains_sticky() {
    let canonical = crate::theme::canonical_name_for_input("sun");
    assert_eq!(canonical, Some("sun"));
}

#[test]
fn auto_color_drift_default_false() {
    let cfg = crate::configfile::load_config_file(None);
    let drift_val = cfg
        .get("auto_color_drift")
        .map(|s| s.as_str())
        .unwrap_or("false");
    assert_ne!(
        drift_val, "true",
        "auto_color_drift must not default to true"
    );
}

#[test]
fn benchmark_fields_unchanged() {
    // Verify that benchmark struct layout is stable by checking
    // that the CloudConfig struct still has the expected fields.
    let cfg = crate::app::CloudConfig {
        color_mode: crate::runtime::ColorMode::TrueColor,
        fullwidth: false,
        shading_mode: crate::runtime::ShadingMode::Random,
        bold_mode: crate::runtime::BoldMode::Random,
        async_mode: false,
        default_bg: true,
        color_scheme: crate::runtime::ColorScheme::Green,
        rain_style: crate::rain_style::RainStyle::Glyph,
        noglitch: false,
        glitch_pct: 0.0,
        glitch_low: 100,
        glitch_high: 1000,
        linger_low: 2000,
        linger_high: 8000,
        short_pct: 0.0,
        die_early_pct: 0.0,
        max_dpc: 1,
        density: 1.0,
        speed: 1.0,
        monolith_size: crate::runtime::MonolithSize::Normal,
        chars: crate::charset::build_chars(crate::charset::Charset::ASCII_SAFE, &[], true),
        message: None,
        message_border: false,
        target_fps: 60.0,
        duration: None,
        duration_s: None,
        bench_frames: None,
        benchmark: false,
        density_auto: false,
        base_density: 1.0,
        perf_stats: false,
        screensaver: false,
        mouse: false,
        charset_preset: "ascii".to_string(),
        user_ranges: vec![],
        def_ascii: true,
        auto_color_drift: false,
        atmosphere_modulation: crate::atmosphere_apply::AtmosphereRuntimeModulation::identity(),
        atmosphere_mode: crate::atmosphere_apply::AtmosphereApplicationMode::Disabled,
    };
    assert!(!cfg.benchmark);
    assert!(!cfg.auto_color_drift);
}

#[test]
fn actual_execution_single_threaded_renderer() {
    // Verify that the default config does not enable async mode.
    let cfg = crate::configfile::load_config_file(None);
    let async_val = cfg.get("async_mode").map(|s| s.as_str()).unwrap_or("false");
    assert_ne!(async_val, "true", "async_mode must not default to true");
}

// ── Identity / Metadata Guards ─────────────────────────────────────────────

#[test]
fn no_lowercase_repo_owner_in_source_or_toml() {
    let bad_pattern = concat!("oxyzen", "q");
    let source_dir = std::path::Path::new("src");
    let toml_path = std::path::Path::new("Cargo.toml");
    let hits = scan_dir_for_lowercase_owner(source_dir, bad_pattern)
        + scan_file_for_literal(toml_path, bad_pattern);
    assert_eq!(hits, 0, "found {hits} bad hits for wrong-casing pattern");
}

fn scan_dir_for_lowercase_owner(dir: &std::path::Path, pattern: &str) -> usize {
    let mut hits = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                hits += scan_dir_for_lowercase_owner(&path, pattern);
            } else if let Some(ext) = path.extension() {
                if ext == "rs" {
                    hits += scan_file_for_literal(&path, pattern);
                }
            }
        }
    }
    hits
}

fn scan_file_for_literal(path: &std::path::Path, pattern: &str) -> usize {
    if let Ok(content) = std::fs::read_to_string(path) {
        content.matches(pattern).count()
    } else {
        0
    }
}

#[test]
fn all_rust_files_under_1000_loc() {
    let src_dir = std::path::Path::new("src");
    let mut violations = Vec::new();
    check_loc_limit(src_dir, &mut violations);
    assert!(
        violations.is_empty(),
        "Rust files over 1000 LOC: {:?}",
        violations
    );
}

fn check_loc_limit(dir: &std::path::Path, violations: &mut Vec<String>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                check_loc_limit(&path, violations);
            } else if let Some(ext) = path.extension() {
                if ext == "rs" {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let loc = content.lines().count();
                        if loc > 1000 {
                            violations.push(format!("{} ({} LOC)", path.display(), loc));
                        }
                    }
                }
            }
        }
    }
}

// ── File Integrity Guards ──────────────────────────────────────────────────

#[test]
fn no_unsafe_in_atmosphere_ab_files() {
    let pattern = concat!("un", "safe ");
    for path in &["src/atmosphere_ab.rs", "src/atmosphere_ab_tests.rs"] {
        let content = std::fs::read_to_string(path).unwrap_or_else(|_| panic!("{path} readable"));
        assert!(!content.contains(pattern), "{path} must not contain unsafe");
    }
}

#[test]
fn no_debt_markers_in_atmosphere_ab_files() {
    let pat_a = concat!("t", "odo");
    let pat_b = concat!("fi", "xme");
    let pat_c = concat!("ha", "ck");
    for path in &["src/atmosphere_ab.rs", "src/atmosphere_ab_tests.rs"] {
        let content = std::fs::read_to_string(path).unwrap_or_else(|_| panic!("{path} readable"));
        let lower = content.to_lowercase();
        assert!(
            !lower.contains(pat_a) && !lower.contains(pat_b) && !lower.contains(pat_c),
            "{path} must not contain debt markers"
        );
    }
}

#[test]
fn atmosphere_ab_files_have_gpl_spdx_header() {
    for path in &["src/atmosphere_ab.rs", "src/atmosphere_ab_tests.rs"] {
        let content = std::fs::read_to_string(path).unwrap_or_else(|_| panic!("{path} readable"));
        assert!(
            content.contains("SPDX-License-Identifier: GPL-3.0-only"),
            "{path} must have GPL SPDX header"
        );
    }
}

#[test]
fn atmosphere_ab_core_under_400_loc() {
    let content =
        std::fs::read_to_string("src/atmosphere_ab.rs").expect("atmosphere_ab.rs readable");
    let loc = content.lines().count();
    assert!(
        loc <= 400,
        "atmosphere_ab.rs has {loc} lines, must stay under 400 after Phase 9.5 split"
    );
}
