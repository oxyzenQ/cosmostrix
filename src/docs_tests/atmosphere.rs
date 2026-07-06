// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Documentation guard tests for controlled atmosphere preset docs (v4.6.0 Phases 3–4).
//!
//! These tests verify that `docs/ATMOSPHERE_PRESETS.md` contains all required
//! preset names, mappings, constraints, and safety invariants. A separate
//! module is used because `docs_tests/release.rs` is at ~819 LOC and must
//! stay under 1000 LOC.

const PRESET_NAMES: &[&str] = &[
    "atmosphere-calm",
    "atmosphere-pulse",
    "atmosphere-signal",
    "atmosphere-compression",
    "atmosphere-void",
    "atmosphere-monolith-pressure",
];

const PRESET_MAPPINGS: &[(&str, &str, &str)] = &[
    ("atmosphere-calm", "disabled", "calm"),
    ("atmosphere-pulse", "controlled-live", "pulse"),
    ("atmosphere-signal", "controlled-live", "signal"),
    ("atmosphere-compression", "controlled-live", "compression"),
    ("atmosphere-void", "controlled-live", "void"),
    (
        "atmosphere-monolith-pressure",
        "controlled-live",
        "monolith-pressure",
    ),
];

/// Helper: read the full presets doc at compile time.
fn presets_doc() -> &'static str {
    include_str!("../../docs/ATMOSPHERE_PRESETS.md")
}

/// Helper: read the config dump text at compile time.
fn config_dump() -> &'static str {
    crate::configfile::dump_config_text()
}

// ── Test 1: ATMOSPHERE_PRESETS.md exists ──

#[test]
fn v46p3_presets_doc_exists() {
    let doc = presets_doc();
    assert!(doc.len() > 100, "ATMOSPHERE_PRESETS.md must be non-trivial");
}

// ── Test 2: every preset name appears ──

#[test]
fn v46p3_presets_doc_contains_all_preset_names() {
    let doc = presets_doc();
    for &name in PRESET_NAMES {
        assert!(
            doc.contains(name),
            "ATMOSPHERE_PRESETS.md must mention preset '{name}'"
        );
    }
}

// ── Test 3: every preset mapping appears ──

#[test]
fn v46p3_presets_doc_contains_all_mappings() {
    let doc = presets_doc();
    for &(name, mode, regime) in PRESET_MAPPINGS {
        assert!(doc.contains(name), "doc must mention preset '{name}'");
        assert!(
            doc.contains(mode),
            "doc must mention mode '{mode}' for preset '{name}'"
        );
        assert!(
            doc.contains(regime),
            "doc must mention regime '{regime}' for preset '{name}'"
        );
    }
}

// ── Test 4: presets doc says opt-in only ──

#[test]
fn v46p3_presets_doc_says_opt_in_only() {
    let doc = presets_doc();
    let lower = doc.to_lowercase();
    assert!(
        lower.contains("opt-in only"),
        "ATMOSPHERE_PRESETS.md must state presets are opt-in only"
    );
}

// ── Test 5: presets doc says storm preset does not exist ──

#[test]
fn v46p3_presets_doc_storm_preset_does_not_exist() {
    let doc = presets_doc();
    assert!(
        doc.contains("Storm preset does not exist")
            || doc.to_lowercase().contains("storm preset does not exist"),
        "ATMOSPHERE_PRESETS.md must state storm preset does not exist"
    );
}

// ── Test 6: presets doc says no color change ──

#[test]
fn v46p3_presets_doc_says_no_color_change() {
    let doc = presets_doc();
    let lower = doc.to_lowercase();
    assert!(
        lower.contains("no color change"),
        "ATMOSPHERE_PRESETS.md must state no color change"
    );
}

// ── Test 7: presets doc says no terminal effects ──

#[test]
fn v46p3_presets_doc_says_no_terminal_effects() {
    let doc = presets_doc();
    let lower = doc.to_lowercase();
    assert!(
        lower.contains("no terminal effects"),
        "ATMOSPHERE_PRESETS.md must state no terminal effects"
    );
}

// ── Test 8: presets doc says default remains disabled/protected/identity ──

#[test]
fn v46p3_presets_doc_says_default_disabled_protected_identity() {
    let doc = presets_doc();
    assert!(
        doc.contains("disabled/protected/identity"),
        "ATMOSPHERE_PRESETS.md must state default remains disabled/protected/identity"
    );
}

// ── Test 9: presets doc says terminal writer remains single-owner ──

#[test]
fn v46p3_presets_doc_says_terminal_writer_single_owner() {
    let doc = presets_doc();
    assert!(
        doc.contains("single-owner"),
        "ATMOSPHERE_PRESETS.md must mention terminal writer remains single-owner"
    );
}

// ── Test 10: config dump mentions atmosphere keys ──

#[test]
fn v46p3_config_dump_mentions_atmosphere_keys() {
    let dump = config_dump();
    assert!(
        dump.contains("atmosphere-mode"),
        "config dump must mention atmosphere-mode"
    );
    assert!(
        dump.contains("atmosphere-regime"),
        "config dump must mention atmosphere-regime"
    );
}

// ── Test 11: config dump examples do not include storm ──

#[test]
fn v46p3_config_dump_no_storm_example() {
    let dump = config_dump();
    // The dump may mention "storm" in a rejection note, but must not have
    // an example that sets atmosphere-regime = storm.
    assert!(
        !dump.contains("atmosphere-regime = storm"),
        "config dump must not include a storm example line"
    );
}

// ── Test 12: profile examples cover all 6 presets ──

#[test]
fn v46p3_presets_doc_profile_examples_cover_all_presets() {
    let doc = presets_doc();
    for &name in PRESET_NAMES {
        // Each preset should have a profile block like [profile.atmosphere-calm]
        let profile_block = format!("[profile.{name}]");
        assert!(
            doc.contains(&profile_block),
            "ATMOSPHERE_PRESETS.md must contain profile block '{profile_block}'"
        );
    }
}

// ── Test 13: --color sun stickiness remains documented ──

#[test]
fn v46p3_presets_doc_color_sun_sticky() {
    let doc = presets_doc();
    assert!(
        doc.contains("--color sun") || doc.contains("color sun"),
        "ATMOSPHERE_PRESETS.md must mention --color sun stickiness"
    );
}

// ── Test 14: auto color drift remains opt-in only ──

#[test]
fn v46p3_presets_doc_auto_color_drift_opt_in() {
    let doc = presets_doc();
    assert!(
        doc.to_lowercase().contains("auto color drift") && doc.to_lowercase().contains("opt-in"),
        "ATMOSPHERE_PRESETS.md must document auto color drift remains opt-in"
    );
}

// ── Test 15: Zactrix 20k lab documented as parked for v4.8 ──

#[test]
fn v46p3_presets_doc_zactrix_parked_v48() {
    let doc = presets_doc();
    assert!(
        doc.contains("zactrix-20k-lab"),
        "ATMOSPHERE_PRESETS.md must mention zactrix-20k-lab"
    );
    assert!(
        doc.contains("v4.8"),
        "ATMOSPHERE_PRESETS.md must mention v4.8 for Zactrix"
    );
}

// ── Test 16: no docs test module exceeds 1000 LOC ──

#[test]
fn v46p3_docs_test_files_under_1000_loc() {
    let files = [
        (
            "src/docs_tests/atmosphere.rs",
            include_str!("atmosphere.rs"),
        ),
        ("src/docs_tests/release.rs", include_str!("release.rs")),
    ];
    for (name, content) in &files {
        let loc = content.lines().count();
        assert!(loc < 1000, "{name} is {loc} LOC — must be under 1000");
    }
}

// ── Phase 4: Discoverability doc tests ──

/// Helper: read ATMOSPHERE_EXPANSION.md at compile time.
fn expansion_doc() -> &'static str {
    include_str!("../../docs/ATMOSPHERE_EXPANSION.md")
}

/// Helper: read ATMOSPHERE_ENGINE.md at compile time.
fn engine_doc() -> &'static str {
    include_str!("../../docs/ATMOSPHERE_ENGINE.md")
}

#[test]
fn v46p4_presets_doc_mentions_list_profiles_discoverability() {
    let doc = presets_doc();
    assert!(
        doc.contains("--list-profiles"),
        "ATMOSPHERE_PRESETS.md must mention --list-profiles discoverability"
    );
}

#[test]
fn v46p4_expansion_doc_mentions_phase4_discoverability() {
    let doc = expansion_doc();
    assert!(
        doc.contains("--list-profiles"),
        "ATMOSPHERE_EXPANSION.md must mention --list-profiles discoverability"
    );
    assert!(
        doc.contains("Phase 3") && doc.contains("Phase 4"),
        "ATMOSPHERE_EXPANSION.md must reference Phase 3 and Phase 4"
    );
}

#[test]
fn v46p4_engine_doc_status_phase4() {
    let doc = engine_doc();
    assert!(
        doc.contains("Phase 4") || doc.contains("Phase 5"),
        "ATMOSPHERE_ENGINE.md must reference Phase 4 or later"
    );
    assert!(
        doc.contains("Discoverability") || doc.contains("Closure"),
        "ATMOSPHERE_ENGINE.md must mention Discoverability or Closure"
    );
}

#[test]
fn v46p4_expansion_doc_discoverability_storm_not_shown() {
    let doc = expansion_doc();
    let section = doc;
    assert!(
        section.contains("Storm preset does not exist"),
        "ATMOSPHERE_EXPANSION.md must state storm preset does not exist"
    );
}

#[test]
fn v46p4_expansion_doc_discoverability_single_owner() {
    let doc = expansion_doc();
    assert!(
        doc.contains("single-owner"),
        "ATMOSPHERE_EXPANSION.md must mention terminal writer single-owner"
    );
}

#[test]
fn v46p4_expansion_doc_discoverability_zactrix_parked() {
    let doc = expansion_doc();
    assert!(
        doc.contains("zactrix-20k-lab") || doc.contains("Zactrix"),
        "ATMOSPHERE_EXPANSION.md must mention Zactrix parked for v4.8"
    );
}

// ── Phase 5: RC smoke / closure tests ──

/// Helper: read rc-smoke.sh at compile time.
fn rc_smoke_script() -> &'static str {
    include_str!("../../scripts/rc-smoke.sh")
}

#[test]
fn v46p5_rc_smoke_checks_list_profiles() {
    let script = rc_smoke_script();
    assert!(
        script.contains("--list-profiles"),
        "rc-smoke.sh must check --list-profiles"
    );
    assert!(
        script.contains("CONTROLLED ATMOSPHERE PRESETS"),
        "rc-smoke.sh must verify CONTROLLED ATMOSPHERE PRESETS in output"
    );
}

#[test]
fn v46p5_rc_smoke_checks_all_six_presets() {
    let script = rc_smoke_script();
    let names = [
        "atmosphere-calm",
        "atmosphere-pulse",
        "atmosphere-signal",
        "atmosphere-compression",
        "atmosphere-void",
        "atmosphere-monolith-pressure",
    ];
    for name in names {
        assert!(
            script.contains(name),
            "rc-smoke.sh must check for preset '{name}'"
        );
    }
}

#[test]
fn v46p5_rc_smoke_rejects_storm() {
    let script = rc_smoke_script();
    assert!(
        script.contains("atmosphere-storm"),
        "rc-smoke.sh must verify atmosphere-storm is absent"
    );
}

#[test]
fn v46p5_rc_smoke_checks_controlled_live_fields() {
    let script = rc_smoke_script();
    assert!(
        script.contains("config_gate: armed"),
        "rc-smoke.sh must check config_gate armed"
    );
    assert!(
        script.contains("visual_runtime: protected"),
        "rc-smoke.sh must check visual_runtime protected"
    );
    assert!(
        script.contains("shadow_risk: whisper"),
        "rc-smoke.sh must check shadow_risk whisper"
    );
}

#[test]
fn v46p5_expansion_doc_mentions_phase5_closure() {
    let doc = expansion_doc();
    assert!(
        doc.contains("Phase 5"),
        "ATMOSPHERE_EXPANSION.md must mention Phase 5"
    );
    assert!(
        doc.contains("RC Smoke") || doc.contains("rc-smoke"),
        "ATMOSPHERE_EXPANSION.md must mention RC Smoke"
    );
}

#[test]
fn v46p5_roadmap_marks_phase5_current_or_closure() {
    let doc = include_str!("../../docs/ROADMAP.md");
    assert!(doc.contains("Phase 5"), "ROADMAP.md must mention Phase 5");
    assert!(
        doc.contains("RC Smoke") || doc.contains("Closure"),
        "ROADMAP.md must mention RC Smoke or Closure for Phase 5"
    );
}

#[test]
fn v46p5_docs_test_files_still_under_1000_loc() {
    let files = [
        (
            "src/docs_tests/atmosphere.rs",
            include_str!("atmosphere.rs"),
        ),
        ("src/docs_tests/release.rs", include_str!("release.rs")),
    ];
    for (name, content) in &files {
        let loc = content.lines().count();
        assert!(loc < 1000, "{name} is {loc} LOC — must be under 1000");
    }
}
