// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Version, tagline, casing, AUR, and changelog metadata guards.

// ── Phase 10.6: description/tagline consistency guard tests ────────────

#[test]
fn cargo_toml_uses_canonical_tagline() {
    let cargo = include_str!("../../Cargo.toml");
    assert!(
        cargo.contains("description = \"Professional-grade cinematic Matrix rain renderer for serious terminal environments.\""),
        "Cargo.toml description must use the canonical tagline"
    );
}

#[test]
fn readme_uses_canonical_tagline() {
    let readme = include_str!("../../README.md");
    assert!(
        readme.contains(
            "Professional-grade cinematic Matrix rain renderer for serious terminal environments."
        ),
        "README.md must contain the canonical tagline"
    );
}

#[test]
fn runtime_identity_uses_canonical_tagline() {
    let ri = include_str!("../renderer_info.rs");
    assert!(
        ri.contains(
            "professional-grade cinematic Matrix rain renderer for serious terminal environments."
        ),
        "renderer_info.rs identity must use the canonical tagline"
    );
}

#[test]
fn readme_does_not_contain_stale_high_performance_tagline() {
    let readme = include_str!("../../README.md");
    assert!(
        !readme.contains("High-performance cinematic Matrix rain renderer for the terminal."),
        "README must not contain the old 'High-performance' tagline"
    );
}

#[test]
fn changelog_uses_568_not_570() {
    let changelog = include_str!("../../CHANGELOG.md");
    assert!(
        changelog.contains("568 deterministic tests"),
        "CHANGELOG.md must say 568 deterministic tests"
    );
    assert!(
        !changelog.contains("570 deterministic tests"),
        "CHANGELOG.md must not contain stale 570 deterministic tests"
    );
}

// ── Phase 12.1: v4.0.0 release metadata guard tests ──────────────────────

#[test]
fn changelog_has_v400_entry_above_v390() {
    let changelog = include_str!("../../CHANGELOG.md");
    let v400_pos = changelog
        .find("## v4.0.0")
        .expect("CHANGELOG must contain v4.0.0 entry");
    let v390_pos = changelog
        .find("## v3.9.0")
        .expect("CHANGELOG must contain v3.9.0 entry");
    assert!(
        v400_pos < v390_pos,
        "CHANGELOG v4.0.0 entry must appear above v3.9.0"
    );
}

#[test]
fn changelog_v400_mentions_default_runtime_protected_identity() {
    let changelog = include_str!("../../CHANGELOG.md");
    let lower = changelog.to_lowercase();
    assert!(
        lower.contains("application_mode = disabled")
            && lower.contains("effective_runtime = identity")
            && lower.contains("shadow_risk = identity"),
        "CHANGELOG v4.0.0 must mention default runtime remains protected/identity"
    );
}

#[test]
fn changelog_v400_mentions_no_multithreaded_terminal_rendering() {
    let changelog = include_str!("../../CHANGELOG.md");
    let lower = changelog.to_lowercase();
    assert!(
        lower.contains("no actual multithreaded terminal rendering")
            || lower.contains("single-owner"),
        "CHANGELOG v4.0.0 must mention no multithreaded terminal rendering"
    );
}

#[test]
fn changelog_v400_mentions_demo_refresh() {
    let changelog = include_str!("../../CHANGELOG.md");
    assert!(
        changelog.to_lowercase().contains("demo refresh")
            || changelog.to_lowercase().contains("gif-first"),
        "CHANGELOG v4.0.0 must mention demo refresh"
    );
}

// ── Active release metadata guard tests (version-agnostic) ───────────
//
// These tests verify that all active release metadata files agree on the
// current package version. The source of truth is `env!("CARGO_PKG_VERSION")`,
// injected by cargo at compile time from Cargo.toml's [package] version field.
//
// Previously these tests hardcoded the version string (e.g. "5.0.0") which
// meant every version bump broke the test suite. Now they dynamically
// compare against the compile-time version, so a version bump via
// `./scripts/version-to.sh` requires ZERO test file edits.
//
// The old "must not contain old version X.Y.Z" assertions were also removed:
// they were pure noise (any version != X.Y.Z passes) and accumulated one
// block per historical version, making the file grow forever.

/// Compile-time current package version (e.g. "5.0.1").
///
/// Injected by cargo from Cargo.toml [package] version. This is the single
/// source of truth for what "current release" means in test assertions.
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[test]
fn cargo_toml_has_current_release_version() {
    let cargo = include_str!("../../Cargo.toml");
    let expected = format!("version = \"{}\"", CURRENT_VERSION);
    assert!(
        cargo.contains(&expected),
        "Cargo.toml must have version = \"{}\" (current package version from env! CARGO_PKG_VERSION)",
        CURRENT_VERSION
    );
}

#[test]
fn readme_uses_current_release_tag_in_install_example() {
    let readme = include_str!("../../README.md");
    let expected_tag = format!("TAG=\"v{}\"", CURRENT_VERSION);
    assert!(
        readme.contains(&expected_tag),
        "README install example must use TAG=\"v{}\" as the current release tag",
        CURRENT_VERSION
    );
}

#[test]
fn aur_pkgbuild_pkgver_matches_release() {
    let pkgbuild = include_str!("../../aur/cosmostrix-bin/PKGBUILD");
    let expected = format!("pkgver={}", CURRENT_VERSION);
    assert!(
        pkgbuild.contains(&expected),
        "PKGBUILD must have {} (must match Cargo.toml package version)",
        expected
    );
}

#[test]
fn aur_srcinfo_pkgver_matches_release() {
    let srcinfo = include_str!("../../aur/cosmostrix-bin/.SRCINFO");
    let expected = format!("pkgver = {}", CURRENT_VERSION);
    assert!(
        srcinfo.contains(&expected),
        ".SRCINFO must have {} (must match Cargo.toml package version)",
        expected
    );
}

#[test]
fn no_active_metadata_still_uses_v400() {
    // Active metadata (Cargo.toml, PKGBUILD, .SRCINFO, README install tag)
    // must not reference 4.0.0. Historical CHANGELOG references are allowed.
    let cargo = include_str!("../../Cargo.toml");
    let pkgbuild = include_str!("../../aur/cosmostrix-bin/PKGBUILD");
    let srcinfo = include_str!("../../aur/cosmostrix-bin/.SRCINFO");

    assert!(
        !cargo.contains(r#"version = "4.0.0""#),
        "Cargo.toml must not have version = \"4.0.0\""
    );
    assert!(
        !pkgbuild.contains("pkgver=4.0.0"),
        "PKGBUILD must not have pkgver=4.0.0"
    );
    assert!(
        !srcinfo.contains("pkgver = 4.0.0"),
        "SRCINFO must not have pkgver = 4.0.0"
    );
}
