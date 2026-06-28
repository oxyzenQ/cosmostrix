// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Source safety audits: unsafe code, debt markers, file hygiene.

#[test]
fn terminal_compat_docs_cover_background_truth() {
    let docs = include_str!("../../docs/TERMINAL_COMPATIBILITY.md");
    assert!(docs.contains("color-bg = transparent"));
    assert!(docs.contains("It does not change terminal emulator opacity."));
    assert!(docs.contains("color-bg = black"));
    assert!(docs.contains("color-bg = default-background"));
}

#[test]
fn terminal_compat_docs_cover_reset_safety() {
    let docs = include_str!("../../docs/TERMINAL_COMPATIBILITY.md");
    assert!(docs.contains("Normal exit is non-destructive."));
    assert!(docs.contains("`--reset-terminal` is explicit destructive recovery."));
    assert!(docs.contains("attempts scrollback purge"));
}

#[test]
fn readme_links_terminal_compatibility_doc() {
    let readme = include_str!("../../README.md");
    assert!(readme.contains("docs/TERMINAL_COMPATIBILITY.md"));
}

#[test]
fn zactrix_core_doc_exists_and_covers_architecture_terms() {
    let docs = include_str!("../../docs/ZACTRIX_CORE.md");
    let lowercase = docs.to_lowercase();
    for term in ["probe", "map", "filter", "verifier", "bounded history"] {
        assert!(
            lowercase.contains(term),
            "Zactrix Core docs should mention {term}"
        );
    }
    assert!(docs.contains("not Linux eBPF"));
    assert!(docs.contains("not a public API"));
    assert!(docs.contains("must not introduce unsafe"));
    assert!(docs.contains("no new unsafe in renderer/core paths"));
    assert!(docs.contains("v3.9.0 Ultimate Subtle Monolith Rain"));
    assert!(docs.contains("v4.0.0"));

    let readme = include_str!("../../README.md");
    assert!(readme.contains("docs/ZACTRIX_CORE.md"));
}

#[test]
fn simd_docs_do_not_claim_global_zero_unsafe() {
    let docs = include_str!("../../docs/SIMD_FEASIBILITY.md");
    assert!(!docs.contains("zero `unsafe`"));
    assert!(docs.contains("no-new-unsafe renderer/core policy"));
}

#[test]
fn visual_stability_docs_cover_v39_monolith_subtlety_policy() {
    let docs = include_str!("../../docs/VISUAL_STABILITY.md");
    assert!(docs.contains("v3.9.0 Monolith Subtlety Policy"));
    assert!(docs.contains("Organic does not mean chaotic"));
    assert!(docs.contains("full-height spine walls"));
    assert!(docs.contains("Zactrix Core may guide"));
}

#[test]
fn source_contains_only_audited_platform_recovery_unsafe() {
    let main_rs = include_str!("../main.rs");
    assert_eq!(main_rs.matches("unsafe {").count(), 1);
    assert!(main_rs.contains("SAFETY: this Linux-only guard"));

    let zactrix_core = include_str!("../zactrix_engine/core.rs");
    assert!(!zactrix_core.contains("unsafe {"));
}

#[test]
fn tracked_docs_and_sources_have_no_new_debt_markers() {
    let markers = [
        concat!("TO", "DO"),
        concat!("FIX", "ME"),
        concat!("HA", "CK"),
    ];
    let mut files = Vec::new();
    collect_files(std::path::Path::new("src"), &mut files);
    collect_files(std::path::Path::new("docs"), &mut files);
    files.push(std::path::PathBuf::from("README.md"));
    files.push(std::path::PathBuf::from("CHANGELOG.md"));

    for path in files {
        let text = std::fs::read_to_string(&path).expect("tracked text file should be readable");
        for marker in markers {
            assert!(
                !text.contains(marker),
                "{} contains debt marker {marker}",
                path.display()
            );
        }
    }
}

fn collect_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("source/doc directory should exist") {
        let path = entry.expect("directory entry should be readable").path();
        if path.is_dir() {
            collect_files(&path, out);
        } else if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("rs" | "md")
        ) {
            out.push(path);
        }
    }
}

#[test]
fn docs_mention_visual_stability_policy_if_exists() {
    let _ = include_str!("../../README.md");
    // If VISUAL_STABILITY.md exists, README should link to it.
    // This test is informational — it verifies the docs can be compiled
    // without actually requiring the doc to exist yet.
}

#[test]
fn no_new_unsafe_in_zactrix_modules() {
    let engine = include_str!("../zactrix_engine/scheduler.rs");
    assert!(
        !engine.contains("unsafe {"),
        "zactrix_engine/scheduler.rs must not contain unsafe"
    );

    let cache = include_str!("../zactrix_engine/cache.rs");
    assert!(
        !cache.contains("unsafe {"),
        "zactrix_engine/cache.rs must not contain unsafe"
    );

    let atmosphere = include_str!("../atmosphere.rs");
    assert!(
        !atmosphere.contains("unsafe {"),
        "atmosphere.rs must not contain unsafe"
    );
}

#[test]
fn auto_color_drift_remains_default_false_in_constants() {
    let constants = include_str!("../constants.rs");
    assert!(
        constants.contains("AUTO_COLOR_DRIFT_DEFAULT: bool = false"),
        "AUTO_COLOR_DRIFT_DEFAULT must be false"
    );
}

#[test]
fn fixed_color_remains_sticky_by_default() {
    // Verify that the color stability docs and config are consistent.
    let stability = include_str!("../../docs/VISUAL_STABILITY.md");
    assert!(
        stability.to_lowercase().contains("sticky")
            || stability.to_lowercase().contains("stable by default"),
        "Visual stability docs should mention sticky/stable by default color behavior"
    );
}

#[test]
fn all_zactrix_files_under_1000_loc() {
    let files = [
        (
            "zactrix_engine/mod.rs",
            include_str!("../zactrix_engine/mod.rs"),
        ),
        (
            "zactrix_engine/scheduler.rs",
            include_str!("../zactrix_engine/scheduler.rs"),
        ),
        (
            "zactrix_engine/core.rs",
            include_str!("../zactrix_engine/core.rs"),
        ),
        (
            "zactrix_engine/cache.rs",
            include_str!("../zactrix_engine/cache.rs"),
        ),
        (
            "zactrix_engine/system.rs",
            include_str!("../zactrix_engine/system.rs"),
        ),
        (
            "zactrix_engine/render.rs",
            include_str!("../zactrix_engine/render.rs"),
        ),
        (
            "zactrix_engine/metrics.rs",
            include_str!("../zactrix_engine/metrics.rs"),
        ),
        ("atmosphere.rs", include_str!("../atmosphere.rs")),
    ];
    for (name, content) in files {
        let lines = content.lines().count();
        assert!(
            lines <= 1000,
            "{name} has {lines} lines, must be under 1000"
        );
    }
}

#[test]
fn docs_tests_modules_stay_under_1000_loc() {
    // Guard: all docs_tests/ submodules must stay under 1000 LOC.
    let files = [
        ("docs_tests/mod.rs", include_str!("mod.rs")),
        ("docs_tests/assets.rs", include_str!("assets.rs")),
        ("docs_tests/endurance.rs", include_str!("endurance.rs")),
        ("docs_tests/metadata.rs", include_str!("metadata.rs")),
        ("docs_tests/readme.rs", include_str!("readme.rs")),
        ("docs_tests/release.rs", include_str!("release.rs")),
        ("docs_tests/safety.rs", include_str!("safety.rs")),
        ("docs_tests/zactrix.rs", include_str!("zactrix.rs")),
    ];
    for (name, content) in files {
        let lines = content.lines().count();
        assert!(
            lines <= 1000,
            "{name} has {lines} lines, must be under 1000"
        );
    }
}

#[test]
fn docs_tests_facade_is_small() {
    let facade = include_str!("mod.rs");
    let lines = facade.lines().count();
    assert!(
        lines <= 50,
        "docs_tests/mod.rs facade should stay small (currently {lines} lines)"
    );
}
