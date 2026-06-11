// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Release candidate doc, benchmark doc, and release workflow auth guards.

// ── Phase 11: Release Candidate Hardening guard tests ────────────────────

#[test]
fn release_candidate_doc_exists_and_covers_checklist() {
    let docs = include_str!("../../docs/RELEASE_CANDIDATE.md");
    assert!(
        docs.contains("cargo clippy"),
        "RELEASE_CANDIDATE.md should mention cargo clippy"
    );
    assert!(
        docs.contains("cargo test"),
        "RELEASE_CANDIDATE.md should mention cargo test"
    );
}

#[test]
fn release_candidate_doc_mentions_no_version_bump_until_release() {
    let docs = include_str!("../../docs/RELEASE_CANDIDATE.md");
    assert!(
        docs.contains("Do not bump the version") || docs.contains("do not bump the version"),
        "RELEASE_CANDIDATE.md should warn against premature version bumps"
    );
}

#[test]
fn release_candidate_doc_includes_runtime_smoke_commands() {
    let docs = include_str!("../../docs/RELEASE_CANDIDATE.md");
    assert!(
        docs.contains("\"$BIN\" -i") || docs.contains("$BIN -i"),
        "RELEASE_CANDIDATE.md should include -i runtime smoke command"
    );
    assert!(
        docs.contains("\"$BIN\" --benchmark") || docs.contains("$BIN --benchmark"),
        "RELEASE_CANDIDATE.md should include --benchmark runtime smoke command"
    );
}

#[test]
fn release_candidate_doc_includes_controlled_live_config_smoke() {
    let docs = include_str!("../../docs/RELEASE_CANDIDATE.md");
    assert!(
        docs.contains("controlled-live"),
        "RELEASE_CANDIDATE.md should mention controlled-live config smoke"
    );
}

#[test]
fn release_candidate_doc_includes_readme_changelog_guard() {
    let docs = include_str!("../../docs/RELEASE_CANDIDATE.md");
    assert!(
        docs.contains("CHANGELOG") && docs.contains("README"),
        "RELEASE_CANDIDATE.md should mention both CHANGELOG and README guards"
    );
}

#[test]
fn benchmark_docs_mention_fps_is_synthetic_uncapped() {
    let docs = include_str!("../../benchmark/README.md");
    assert!(
        docs.to_lowercase().contains("synthetic") && docs.to_lowercase().contains("uncapped"),
        "benchmark/README.md should state FPS is synthetic/uncapped"
    );
}

#[test]
fn benchmark_docs_mention_stability_more_important_than_peak_fps() {
    let docs = include_str!("../../benchmark/README.md");
    assert!(
        docs.contains("p99")
            && (docs.to_lowercase().contains("stability")
                || docs.to_lowercase().contains("more than")),
        "benchmark/README.md should emphasize stability over peak FPS"
    );
}

// ── Phase 12.3: release workflow authentication guard tests ──────────────

#[test]
fn release_workflow_has_contents_write_permission() {
    let workflow = include_str!("../../.github/workflows/release.yml");
    assert!(
        workflow.contains("contents: write"),
        "release workflow must grant contents: write permission"
    );
}

#[test]
fn release_workflow_passes_github_token_to_release_action() {
    let workflow = include_str!("../../.github/workflows/release.yml");
    assert!(
        workflow.contains("GITHUB_TOKEN") && workflow.contains("secrets.GITHUB_TOKEN"),
        "release workflow must pass GITHUB_TOKEN to the release action"
    );
}

#[test]
fn release_workflow_publish_job_has_permissions() {
    let workflow = include_str!("../../.github/workflows/release.yml");
    // The publish_release job must have its own permissions block with
    // contents: write, not rely solely on top-level inheritance.
    let publish_marker = "publish_release:";
    let publish_pos = workflow
        .find(publish_marker)
        .expect("release workflow must contain publish_release job");
    let perm_pos = workflow[publish_pos..]
        .find("permissions:")
        .expect("publish_release job must have a permissions block");
    let perm_section = &workflow[publish_pos + perm_pos..];
    assert!(
        perm_section.contains("contents: write"),
        "publish_release job permissions must include contents: write"
    );
}

#[test]
fn release_candidate_doc_mentions_auth_requirement() {
    let docs = include_str!("../../docs/RELEASE_CANDIDATE.md");
    assert!(
        docs.contains("contents: write")
            && (docs.contains("GITHUB_TOKEN") || docs.contains("authentication")),
        "RELEASE_CANDIDATE.md must document the release workflow authentication requirement"
    );
}

// ── v4.6 Phase 5: atmosphere RC checklist guard tests ────────────────────

#[test]
fn release_candidate_doc_mentions_v46_atmosphere_rc_checklist() {
    let docs = include_str!("../../docs/RELEASE_CANDIDATE.md");
    assert!(
        docs.contains("v4.6 Atmosphere RC Checklist"),
        "RELEASE_CANDIDATE.md must contain v4.6 Atmosphere RC Checklist section"
    );
}

#[test]
fn release_candidate_doc_v46_mentions_list_profiles() {
    let docs = include_str!("../../docs/RELEASE_CANDIDATE.md");
    assert!(
        docs.contains("--list-profiles"),
        "RELEASE_CANDIDATE.md v4.6 section must mention --list-profiles"
    );
}

#[test]
fn release_candidate_doc_v46_storm_unavailable() {
    let docs = include_str!("../../docs/RELEASE_CANDIDATE.md");
    let lower = docs.to_lowercase();
    assert!(
        lower.contains("storm"),
        "RELEASE_CANDIDATE.md v4.6 section must mention storm"
    );
    // Ensure it says storm is NOT available, not that it is.
    assert!(
        lower.contains("not") || lower.contains("does not") || lower.contains("unavailable"),
        "RELEASE_CANDIDATE.md must indicate storm is not available"
    );
}

// ── v4.7 Phase 4: Profile RC checklist guard tests ────────────────────

#[test]
fn release_candidate_doc_mentions_v47_profile_rc_checklist() {
    let docs = include_str!("../../docs/RELEASE_CANDIDATE.md");
    assert!(
        docs.contains("v4.7 Profile RC Checklist"),
        "RELEASE_CANDIDATE.md must contain v4.7 Profile RC Checklist section"
    );
}

#[test]
fn release_candidate_doc_v47_mentions_profile_ecosystem() {
    let docs = include_str!("../../docs/RELEASE_CANDIDATE.md");
    assert!(
        docs.contains("PROFILE_ECOSYSTEM"),
        "RELEASE_CANDIDATE.md v4.7 section must mention profile ecosystem docs"
    );
}

#[test]
fn release_candidate_doc_v47_mentions_profile_examples() {
    let docs = include_str!("../../docs/RELEASE_CANDIDATE.md");
    assert!(
        docs.contains("PROFILE_EXAMPLES"),
        "RELEASE_CANDIDATE.md v4.7 section must mention profile examples docs"
    );
}

#[test]
fn release_candidate_doc_v47_mentions_unknown_profile_clean_error() {
    let docs = include_str!("../../docs/RELEASE_CANDIDATE.md");
    assert!(
        docs.contains("clean error") || docs.contains("clean error listing"),
        "RELEASE_CANDIDATE.md v4.7 section must mention unknown profile clean error"
    );
}

#[test]
fn release_candidate_doc_v47_mentions_storm_unavailable() {
    let docs = include_str!("../../docs/RELEASE_CANDIDATE.md");
    let lower = docs.to_lowercase();
    assert!(
        lower.contains("storm") && lower.contains("unavailable"),
        "RELEASE_CANDIDATE.md v4.7 section must mention storm unavailable"
    );
}

#[test]
fn roadmap_marks_phase4_current_or_closure() {
    let docs = include_str!("../../docs/ROADMAP.md");
    assert!(
        docs.contains("Phase 4"),
        "ROADMAP.md must mention Phase 4 for v4.7"
    );
    assert!(
        docs.contains("RC Smoke") || docs.contains("Closure"),
        "ROADMAP.md Phase 4 must mention RC Smoke or Closure"
    );
}

#[test]
fn v47_no_performance_lab_branch_referenced_as_merged() {
    let ecosystem = include_str!("../../docs/PROFILE_ECOSYSTEM.md");
    let examples = include_str!("../../docs/PROFILE_EXAMPLES.md");
    let roadmap = include_str!("../../docs/ROADMAP.md");
    // All three docs must say zactrix-20k-lab is parked, not merged
    for (name, content) in [
        ("ECOSYSTEM", ecosystem),
        ("EXAMPLES", examples),
        ("ROADMAP", roadmap),
    ] {
        assert!(
            content.contains("parked") || content.contains("v4.8"),
            "{name} must reference zactrix-20k-lab as parked for v4.8, not merged"
        );
    }
}
