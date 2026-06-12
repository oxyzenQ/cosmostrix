// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Version, tagline, casing, AUR, and changelog metadata guards.

// ── Phase 10.6: description/tagline consistency guard tests ────────────

#[test]
fn cargo_toml_uses_canonical_tagline() {
    let cargo = include_str!("../../Cargo.toml");
    assert!(
        cargo.contains("description = \"Production-grade cinematic Matrix rain renderer for serious terminal environments.\""),
        "Cargo.toml description must use the canonical tagline"
    );
}

#[test]
fn readme_uses_canonical_tagline() {
    let readme = include_str!("../../README.md");
    assert!(
        readme.contains(
            "Production-grade cinematic Matrix rain renderer for serious terminal environments."
        ),
        "README.md must contain the canonical tagline"
    );
}

#[test]
fn runtime_identity_uses_canonical_tagline() {
    let ri = include_str!("../renderer_info.rs");
    assert!(
        ri.contains(
            "production-grade cinematic Matrix rain renderer for serious terminal environments."
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

// ── v4.9.0 release metadata guard tests ──────────────────────────────

#[test]
fn cargo_toml_version_matches_changelog_latest() {
    let cargo = include_str!("../../Cargo.toml");
    assert!(
        cargo.contains(r#"version = "4.9.0""#),
        "Cargo.toml must have version = \"4.9.0\""
    );
    assert!(
        !cargo.contains(r#"version = "4.0.1""#),
        "Cargo.toml must not contain old version 4.0.1"
    );
    assert!(
        !cargo.contains(r#"version = "4.5.0""#),
        "Cargo.toml must not contain old version 4.5.0"
    );
    assert!(
        !cargo.contains(r#"version = "4.6.0""#),
        "Cargo.toml must not contain old version 4.6.0"
    );
    assert!(
        !cargo.contains(r#"version = "4.7.0""#),
        "Cargo.toml must not contain old version 4.7.0"
    );
    assert!(
        !cargo.contains(r#"version = "4.8.0""#),
        "Cargo.toml must not contain old version 4.8.0"
    );
}

#[test]
fn readme_uses_v490_tag_in_install_example() {
    let readme = include_str!("../../README.md");
    assert!(
        readme.contains(r#"TAG="v4.9.0""#),
        "README install example must use TAG=\"v4.9.0\" as the current release tag"
    );
}

// ── Phase 12.2: v4.0.1 release metadata guard tests ──────────────────────

#[test]
fn aur_pkgbuild_pkgver_matches_release() {
    let pkgbuild = include_str!("../../aur/cosmostrix-bin/PKGBUILD");
    assert!(
        pkgbuild.contains("pkgver=4.9.0"),
        "PKGBUILD must have pkgver=4.9.0"
    );
    assert!(
        !pkgbuild.contains("pkgver=4.0.1"),
        "PKGBUILD must not contain old pkgver=4.0.1"
    );
    assert!(
        !pkgbuild.contains("pkgver=4.5.0"),
        "PKGBUILD must not contain old pkgver=4.5.0"
    );
    assert!(
        !pkgbuild.contains("pkgver=4.6.0"),
        "PKGBUILD must not contain old pkgver=4.6.0"
    );
    assert!(
        !pkgbuild.contains("pkgver=4.7.0"),
        "PKGBUILD must not contain old pkgver=4.7.0"
    );
    assert!(
        !pkgbuild.contains("pkgver=4.8.0"),
        "PKGBUILD must not contain old pkgver=4.8.0"
    );
}

#[test]
fn aur_srcinfo_pkgver_matches_release() {
    let srcinfo = include_str!("../../aur/cosmostrix-bin/.SRCINFO");
    assert!(
        srcinfo.contains("pkgver = 4.9.0"),
        "SRCINFO must have pkgver = 4.9.0"
    );
    assert!(
        !srcinfo.contains("pkgver = 4.0.1"),
        "SRCINFO must not contain old pkgver = 4.0.1"
    );
    assert!(
        !srcinfo.contains("pkgver = 4.5.0"),
        "SRCINFO must not contain old pkgver = 4.5.0"
    );
    assert!(
        !srcinfo.contains("pkgver = 4.6.0"),
        "SRCINFO must not contain old pkgver = 4.6.0"
    );
    assert!(
        !srcinfo.contains("pkgver = 4.7.0"),
        "SRCINFO must not contain old pkgver = 4.7.0"
    );
    assert!(
        !srcinfo.contains("pkgver = 4.8.0"),
        "SRCINFO must not contain old pkgver = 4.8.0"
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
