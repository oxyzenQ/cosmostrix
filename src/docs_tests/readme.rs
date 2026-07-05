// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! README structure and content guards.

// ── Phase 9.1: README / Changelog guard tests ──────────────────────────

#[test]
fn readme_must_not_contain_release_notes_section() {
    let readme = include_str!("../../README.md");
    let lowercase = readme.to_lowercase();
    assert!(
        !lowercase.contains("\n## release notes"),
        "README must not contain a top-level 'Release notes' section"
    );
    assert!(
        !lowercase.contains("\n### release notes"),
        "README must not contain a second-level 'Release notes' section"
    );
}

#[test]
fn readme_must_not_contain_old_version_history_blocks() {
    let readme = include_str!("../../README.md");
    assert!(
        !readme.contains("v3.1.0 (in development)"),
        "README must not contain stale 'v3.1.0 (in development)'"
    );
    assert!(
        !readme.contains("\n### v2.2.0"),
        "README must not contain v2.2.0 release note heading"
    );
    assert!(
        !readme.contains("\n### v2.1.0"),
        "README must not contain v2.1.0 release note heading"
    );
    assert!(
        !readme.contains("\n### v2.0.0"),
        "README must not contain v2.0.0 release note heading"
    );
}

#[test]
fn readme_must_link_to_changelog() {
    let readme = include_str!("../../README.md");
    assert!(
        readme.contains("CHANGELOG.md"),
        "README must link to CHANGELOG.md"
    );
}

#[test]
fn changelog_exists_and_contains_historical_notes() {
    let changelog = include_str!("../../CHANGELOG.md");
    assert!(
        changelog.contains("## v3.1.0"),
        "CHANGELOG must contain v3.1.0 entry"
    );
    assert!(
        changelog.contains("## v2.2.0"),
        "CHANGELOG must contain v2.2.0 entry"
    );
    assert!(
        changelog.contains("## v2.1.0"),
        "CHANGELOG must contain v2.1.0 entry"
    );
    assert!(
        changelog.contains("## v2.0.0"),
        "CHANGELOG must contain v2.0.0 entry"
    );
    assert!(
        !changelog.contains("in development)"),
        "CHANGELOG must not contain stale 'in development' wording"
    );
}

#[test]
fn readme_keeps_canonical_repo_casing() {
    let readme = include_str!("../../README.md");
    assert!(
        readme.contains("github.com/oxyzenQ"),
        "README must contain canonical repo casing github.com/oxyzenQ"
    );
    let lower = "github.com/".to_string() + concat!("oxyzen", "q");
    assert!(
        !readme.contains(&lower),
        "README must not contain wrong-cased repo owner"
    );
}

#[test]
fn changelog_keeps_canonical_repo_casing() {
    let changelog = include_str!("../../CHANGELOG.md");
    let lower = "github.com/".to_string() + concat!("oxyzen", "q");
    assert!(
        !changelog.contains(&lower),
        "CHANGELOG must not contain wrong-cased repo owner"
    );
}
