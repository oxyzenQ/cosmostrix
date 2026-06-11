// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Documentation guard tests for controlled atmosphere preset docs (v4.6.0 Phase 3).
//!
//! These tests verify that `docs/ATMOSPHERE_PRESETS.md` contains all required
//! preset names, mappings, constraints, and safety invariants. A separate
//! module is used because `docs_tests/zactrix.rs` is at ~819 LOC and must
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
        ("src/docs_tests/zactrix.rs", include_str!("zactrix.rs")),
    ];
    for (name, content) in &files {
        let loc = content.lines().count();
        assert!(loc < 1000, "{name} is {loc} LOC — must be under 1000");
    }
}
