// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Profile ecosystem and examples doc guard tests (v4.7.0).

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
    let docs = include_str!("../../docs/PROFILE_ECOSYSTEM.md");
    assert!(
        docs.contains("parked for v4.8"),
        "PROFILE_ECOSYSTEM.md must confirm zactrix-20k-lab is parked"
    );
}

// ── v4.7.0 Phase 2: Profile examples doc guard tests ─────────────────────

#[test]
fn v47p2_profile_examples_doc_exists() {
    let docs = include_str!("../../docs/PROFILE_EXAMPLES.md");
    assert!(
        docs.len() > 200,
        "docs/PROFILE_EXAMPLES.md must exist and have meaningful content"
    );
}

#[test]
fn v47p2_examples_doc_mentions_precedence_chain() {
    let docs = include_str!("../../docs/PROFILE_EXAMPLES.md");
    assert!(
        docs.contains("CLI > profile > config > defaults"),
        "PROFILE_EXAMPLES.md must mention CLI > profile > config > defaults"
    );
}

#[test]
fn v47p2_examples_doc_mentions_color_sun_override() {
    let docs = include_str!("../../docs/PROFILE_EXAMPLES.md");
    assert!(
        docs.contains("--color sun"),
        "PROFILE_EXAMPLES.md must mention --color sun override"
    );
}

#[test]
fn v47p2_examples_doc_mentions_auto_color_drift_explicit_only() {
    let docs = include_str!("../../docs/PROFILE_EXAMPLES.md");
    let lower = docs.to_lowercase();
    assert!(
        lower.contains("auto color drift")
            && (lower.contains("explicit") || lower.contains("explicitly")),
        "PROFILE_EXAMPLES.md must say auto color drift is explicit only"
    );
}

#[test]
fn v47p2_examples_doc_includes_atmosphere_pulse() {
    let docs = include_str!("../../docs/PROFILE_EXAMPLES.md");
    assert!(
        docs.contains("atmosphere-pulse") || docs.contains("atmosphere-regime = pulse"),
        "PROFILE_EXAMPLES.md must include atmosphere-pulse profile example"
    );
}

#[test]
fn v47p2_examples_doc_includes_atmosphere_signal() {
    let docs = include_str!("../../docs/PROFILE_EXAMPLES.md");
    assert!(
        docs.contains("atmosphere-signal") || docs.contains("atmosphere-regime = signal"),
        "PROFILE_EXAMPLES.md must include atmosphere-signal profile example"
    );
}

#[test]
fn v47p2_examples_doc_includes_atmosphere_void() {
    let docs = include_str!("../../docs/PROFILE_EXAMPLES.md");
    assert!(
        docs.contains("atmosphere-void") || docs.contains("atmosphere-regime = void"),
        "PROFILE_EXAMPLES.md must include atmosphere-void profile example"
    );
}

#[test]
fn v47p2_examples_doc_includes_atmosphere_monolith_pressure() {
    let docs = include_str!("../../docs/PROFILE_EXAMPLES.md");
    assert!(
        docs.contains("atmosphere-monolith-pressure")
            || docs.contains("atmosphere-regime = monolith-pressure"),
        "PROFILE_EXAMPLES.md must include atmosphere-monolith-pressure example"
    );
}

#[test]
fn v47p2_examples_doc_does_not_include_storm_profile() {
    let docs = include_str!("../../docs/PROFILE_EXAMPLES.md");
    assert!(
        !docs.contains("atmosphere-regime = storm")
            && !docs.contains("profile.*storm")
            && !docs.contains("Storm Profile"),
        "PROFILE_EXAMPLES.md must not include storm profile"
    );
}

#[test]
fn v47p2_config_dump_points_to_profile_examples() {
    let dump = crate::configfile::dump_config_text();
    assert!(
        dump.contains("PROFILE_EXAMPLES"),
        "config dump must point to docs/PROFILE_EXAMPLES.md"
    );
}

#[test]
fn v47p2_list_profiles_points_to_profile_examples() {
    let text = crate::profile::list_profiles_text(&std::collections::BTreeMap::new());
    assert!(
        text.contains("PROFILE_EXAMPLES"),
        "--list-profiles output must point to docs/PROFILE_EXAMPLES.md"
    );
}

#[test]
fn v47p2_profile_examples_use_valid_known_keys() {
    let docs = include_str!("../../docs/PROFILE_EXAMPLES.md");
    // All profile.<name>.<field> in the doc must use known profile fields
    let known_fields = [
        "base",
        "scene",
        "preset",
        "color",
        "charset",
        "fps",
        "speed",
        "density",
        "glitch-level",
        "monolith-size",
        "color-bg",
        "atmosphere-mode",
        "atmosphere-regime",
    ];
    for line in docs.lines() {
        let line = line.trim_start();
        if !line.starts_with("profile.") {
            continue;
        }
        // Extract field from "profile.name.field = value"
        // Split: "profile" . "name" . "field" = "value"
        let parts: Vec<&str> = line.splitn(3, '.').collect();
        if parts.len() >= 3 {
            let field = parts[2].split_whitespace().next().unwrap_or("");
            let field = field.trim_end_matches('=').trim();
            assert!(
                known_fields.contains(&field),
                "PROFILE_EXAMPLES.md uses unknown profile field: {field} in line: {line}"
            );
        }
    }
}

#[test]
fn v47p2_docs_test_modules_under_1000_loc_phase2() {
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
fn v47p2_no_performance_lab_branch_merged() {
    let docs = include_str!("../../docs/PROFILE_EXAMPLES.md");
    assert!(
        docs.contains("zactrix-20k-lab") || docs.contains("v4.8"),
        "PROFILE_EXAMPLES.md must confirm zactrix-20k-lab is parked for v4.8"
    );
}
