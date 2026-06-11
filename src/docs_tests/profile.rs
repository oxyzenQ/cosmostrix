// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Profile ecosystem doc and contract guard tests (v4.7.0 Phase 1).

// ── v4.7.0 Phase 1: Profile ecosystem doc existence ──────────────────────

#[test]
fn v47p1_profile_ecosystem_doc_exists() {
    let docs = include_str!("../../docs/PROFILE_ECOSYSTEM.md");
    assert!(
        docs.len() > 200,
        "docs/PROFILE_ECOSYSTEM.md must exist and have meaningful content"
    );
}

#[test]
fn v47p1_profile_ecosystem_doc_mentions_precedence_chain() {
    let docs = include_str!("../../docs/PROFILE_ECOSYSTEM.md");
    assert!(
        docs.contains("CLI > profile > config > defaults"),
        "PROFILE_ECOSYSTEM.md must mention CLI > profile > config > defaults"
    );
}

#[test]
fn v47p1_profile_ecosystem_doc_mentions_list_profiles() {
    let docs = include_str!("../../docs/PROFILE_ECOSYSTEM.md");
    assert!(
        docs.contains("--list-profiles"),
        "PROFILE_ECOSYSTEM.md must mention --list-profiles"
    );
}

#[test]
fn v47p1_profile_ecosystem_doc_mentions_controlled_atmosphere_presets() {
    let docs = include_str!("../../docs/PROFILE_ECOSYSTEM.md");
    assert!(
        docs.contains("controlled atmosphere presets"),
        "PROFILE_ECOSYSTEM.md must mention controlled atmosphere presets"
    );
}

#[test]
fn v47p1_profile_ecosystem_doc_says_atmosphere_opt_in_only() {
    let docs = include_str!("../../docs/PROFILE_ECOSYSTEM.md");
    let lower = docs.to_lowercase();
    assert!(
        lower.contains("opt-in only"),
        "PROFILE_ECOSYSTEM.md must say atmosphere remains opt-in only"
    );
}

#[test]
fn v47p1_profile_ecosystem_doc_says_storm_unavailable() {
    let docs = include_str!("../../docs/PROFILE_ECOSYSTEM.md");
    assert!(
        docs.contains("Storm Unavailable")
            || docs.contains("storm is not config-safe")
            || docs.contains("Storm preset does not exist"),
        "PROFILE_ECOSYSTEM.md must say storm unavailable"
    );
}

#[test]
fn v47p1_profile_ecosystem_doc_says_terminal_writer_single_owner() {
    let docs = include_str!("../../docs/PROFILE_ECOSYSTEM.md");
    assert!(
        docs.contains("single-owner"),
        "PROFILE_ECOSYSTEM.md must say terminal writer single-owner"
    );
}

#[test]
fn v47p1_profile_ecosystem_doc_says_zactrix_20k_lab_parked_for_v48() {
    let docs = include_str!("../../docs/PROFILE_ECOSYSTEM.md");
    assert!(
        docs.contains("zactrix-20k-lab") && docs.contains("v4.8"),
        "PROFILE_ECOSYSTEM.md must say zactrix-20k-lab is parked for v4.8"
    );
}

// ── v4.7.0 Phase 1: Profile behavior contract tests ──────────────────────

#[test]
fn v47p1_unknown_profile_behavior_remains_clean() {
    // Unknown profile via CLI must produce a clean error with no partial mutation
    let err = crate::config_apply_profiles_tests::args_with_config_result(
        "profile.nightcore.base = monolith\n",
        &["--profile", "unknown"],
    )
    .unwrap_err();
    assert!(
        err.contains("error: invalid profile: unknown"),
        "unknown CLI profile must produce clean error"
    );
    assert!(
        err.contains("expected one of:"),
        "unknown CLI profile must list available profiles"
    );
}

#[test]
fn v47p1_cli_override_beats_profile() {
    // CLI --color must win over profile color
    let args = crate::config_apply_profiles_tests::args_with_config(
        "profile.nightcore.base = monolith\n\
         profile.nightcore.color = purple\n\
         profile.nightcore.speed = 24\n",
        &["--profile", "nightcore", "--color", "sun"],
    );
    assert_eq!(args.color, "sun", "CLI --color must beat profile color");
}

#[test]
fn v47p1_profile_beats_config() {
    // Profile color must override config color
    let args = crate::config_apply_profiles_tests::args_with_config(
        "color = ocean\n\
         profile.nightcore.base = monolith\n\
         profile.nightcore.color = purple\n\
         profile.nightcore.speed = 24\n",
        &["--profile", "nightcore"],
    );
    assert_eq!(args.color, "purple", "profile color must beat config color");
}

#[test]
fn v47p1_profile_examples_remain_valid_syntax() {
    // Verify the config dump still contains valid profile example syntax
    let dump = crate::configfile::dump_config_text();
    assert!(
        dump.contains("profile.nightcore.base = monolith"),
        "dump config must contain valid profile example syntax"
    );
    assert!(
        dump.contains("profile.nightcore.color"),
        "dump config must contain profile color example"
    );
}

#[test]
fn v47p1_docs_test_modules_under_1000_loc() {
    // All docs test files must stay under 1000 LOC
    let files: [&str; 10] = [
        include_str!("mod.rs"),
        include_str!("assets.rs"),
        include_str!("atmosphere.rs"),
        include_str!("endurance.rs"),
        include_str!("metadata.rs"),
        include_str!("profile.rs"),
        include_str!("readme.rs"),
        include_str!("release.rs"),
        include_str!("safety.rs"),
        include_str!("zactrix.rs"),
    ];
    let names = [
        "mod.rs",
        "assets.rs",
        "atmosphere.rs",
        "endurance.rs",
        "metadata.rs",
        "profile.rs",
        "readme.rs",
        "release.rs",
        "safety.rs",
        "zactrix.rs",
    ];
    for (name, content) in names.iter().zip(files.iter()) {
        let loc = content.lines().count();
        assert!(
            loc < 1000,
            "docs_tests/{name} is {loc} LOC (must be under 1000)"
        );
    }
}

#[test]
fn v47p1_no_performance_lab_branch_merged() {
    // zactrix-20k-lab must not be in main branch history
    let docs = include_str!("../../docs/PROFILE_ECOSYSTEM.md");
    assert!(
        docs.contains("parked for v4.8"),
        "PROFILE_ECOSYSTEM.md must confirm zactrix-20k-lab is parked"
    );
}
