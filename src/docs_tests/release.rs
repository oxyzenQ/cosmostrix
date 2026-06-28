// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

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

// ── v4.9.0 Phase 1: The Wolf — Benchmark release guard tests ─────────────

// Static guard: benchmark/README.md must contain a release benchmark
// section for the current version.  When preparing a new release, this
// guard must be updated to the new version.  The pattern is: for each
// release N, the benchmark README must have a section mentioning that
// version before the release tag is created.

#[test]
fn benchmark_release_guard_current_version_has_report() {
    let docs = include_str!("../../benchmark/README.md");
    // Guard: v5.0.0 is the current release; its benchmark section must exist.
    assert!(
        docs.contains("v5.0.0"),
        "benchmark/README.md must contain a section for the current release (v5.0.0)"
    );
}

#[test]
fn benchmark_release_guard_mentions_release_benchmark() {
    let docs = include_str!("../../benchmark/README.md");
    let lower = docs.to_lowercase();
    assert!(
        lower.contains("release benchmark"),
        "benchmark/README.md must mention 'release benchmark' for the current version"
    );
}

#[test]
fn benchmark_release_guard_has_run_count() {
    let docs = include_str!("../../benchmark/README.md");
    // The v4.8.0 section must mention a 5-run benchmark.
    assert!(
        docs.contains("Run count: 5") || docs.contains("5-run") || docs.contains("5 run"),
        "benchmark/README.md must mention the benchmark run count (5)"
    );
}

#[test]
fn benchmark_release_guard_terminal_writer_single_owner() {
    let docs = include_str!("../../benchmark/README.md");
    assert!(
        docs.contains("terminal_writer") && docs.contains("single-owner"),
        "benchmark/README.md must state terminal_writer is single-owner"
    );
}

#[test]
fn benchmark_release_guard_compute_parallelism_disabled() {
    let docs = include_str!("../../benchmark/README.md");
    assert!(
        docs.contains("compute_parallelism") && docs.contains("disabled"),
        "benchmark/README.md must state compute_parallelism is disabled"
    );
}

#[test]
fn benchmark_release_guard_50k_not_reached() {
    let docs = include_str!("../../benchmark/README.md");
    let lower = docs.to_lowercase();
    assert!(
        lower.contains("not reached") && lower.contains("not promised"),
        "benchmark/README.md must state 50k FPS was not reached and not promised"
    );
}

#[test]
fn benchmark_release_guard_preserves_invariants_table() {
    let docs = include_str!("../../benchmark/README.md");
    // The current release section must have an invariants table with honest values.
    assert!(
        docs.contains("single-threaded-renderer"),
        "benchmark/README.md must report actual_execution as single-threaded-renderer"
    );
}

// ── v4.9.0 Phase 5: RC prep release guard tests ─────────────────────────

/// Extract the text of a versioned benchmark section from benchmark/README.md.
/// Finds `\n## vX.Y.Z`, then slices until the next `\n## v` header.
fn extract_bench_section<'a>(docs: &'a str, version_prefix: &str) -> &'a str {
    let marker = format!("\n## {version_prefix}");
    let start = docs
        .find(&marker)
        .unwrap_or_else(|| panic!("benchmark README must have section {version_prefix}"));
    let section = &docs[start + 1..]; // skip the leading \n
    let next_header = section.find("\n## ").unwrap_or(section.len());
    &section[..next_header]
}

#[test]
fn benchmark_v49_has_5_run_evidence() {
    let docs = include_str!("../../benchmark/README.md");
    let v49_section = extract_bench_section(docs, "v4.9.0");
    assert!(
        v49_section.contains("Run count: 5")
            || v49_section.contains("5-run")
            || v49_section.contains("5 Run"),
        "v4.9.0 benchmark section must mention 5-run benchmark"
    );
    assert!(
        v49_section.contains("excellent"),
        "v4.9.0 benchmark section must report excellent frame_time_stability"
    );
    assert!(
        v49_section.contains("single-owner"),
        "v4.9.0 benchmark section must state terminal_writer is single-owner"
    );
    assert!(
        v49_section.contains("disabled"),
        "v4.9.0 benchmark section must state compute_parallelism is disabled"
    );
    assert!(
        v49_section.contains("single-threaded-renderer"),
        "v4.9.0 benchmark section must report actual_execution"
    );
}

#[test]
fn benchmark_v49_mentions_50k_not_promised() {
    let docs = include_str!("../../benchmark/README.md");
    let v49_section = extract_bench_section(docs, "v4.9.0");
    let lower = v49_section.to_lowercase();
    assert!(
        lower.contains("not reached") && lower.contains("not promised"),
        "v4.9.0 section must state 50k was not reached and not promised"
    );
}

#[test]
fn benchmark_v49_mentions_heavy_message_not_comparable() {
    let docs = include_str!("../../benchmark/README.md");
    let v49_section = extract_bench_section(docs, "v4.9.0");
    let normalized: String = v49_section.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        normalized.to_lowercase().contains("not comparable"),
        "v4.9.0 section must state heavy message/matrix mode is not comparable"
    );
}

// ── v5.0.0 Phase 5: RC prep release guard tests ────────────────────────

#[test]
fn benchmark_v50_has_5_run_evidence() {
    let docs = include_str!("../../benchmark/README.md");
    let v50_section = extract_bench_section(docs, "v5.0.0");
    assert!(
        v50_section.contains("Run count: 5")
            || v50_section.contains("5-run")
            || v50_section.contains("5 Run"),
        "v5.0.0 benchmark section must mention 5-run benchmark"
    );
    assert!(
        v50_section.contains("excellent"),
        "v5.0.0 benchmark section must report excellent frame_time_stability"
    );
    assert!(
        v50_section.contains("single-owner"),
        "v5.0.0 benchmark section must state terminal_writer is single-owner"
    );
    assert!(
        v50_section.contains("disabled"),
        "v5.0.0 benchmark section must state compute_parallelism is disabled"
    );
    assert!(
        v50_section.contains("single-threaded-renderer"),
        "v5.0.0 benchmark section must report actual_execution"
    );
}

#[test]
fn benchmark_v50_mentions_50k_not_promised() {
    let docs = include_str!("../../benchmark/README.md");
    let v50_section = extract_bench_section(docs, "v5.0.0");
    let lower = v50_section.to_lowercase();
    assert!(
        lower.contains("not reached") && lower.contains("not promised"),
        "v5.0.0 section must state 50k was not reached and not promised"
    );
}

#[test]
fn benchmark_v50_mentions_heavy_message_not_comparable() {
    let docs = include_str!("../../benchmark/README.md");
    let v50_section = extract_bench_section(docs, "v5.0.0");
    let normalized: String = v50_section.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        normalized.to_lowercase().contains("not comparable"),
        "v5.0.0 section must state heavy message/matrix mode is not comparable"
    );
}

#[test]
fn roadmap_marks_v50_phase5_release_prep() {
    let docs = include_str!("../../docs/ROADMAP.md");
    // Phase 5 should exist and be pending or have a commit hash
    let phase5_line = docs
        .lines()
        .find(|l| l.contains("Phase 5") && l.to_lowercase().contains("release candidate prep"));
    assert!(
        phase5_line.is_some(),
        "roadmap must have a Phase 5 row for Release candidate prep"
    );
}

#[test]
fn changelog_has_v49_section() {
    let docs = include_str!("../../CHANGELOG.md");
    assert!(
        docs.contains("v4.9.0"),
        "CHANGELOG.md must have a v4.9.0 section"
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
