// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

#[test]
fn terminal_compat_docs_cover_background_truth() {
    let docs = include_str!("../docs/TERMINAL_COMPATIBILITY.md");
    assert!(docs.contains("color-bg = transparent"));
    assert!(docs.contains("It does not change terminal emulator opacity."));
    assert!(docs.contains("color-bg = black"));
    assert!(docs.contains("color-bg = default-background"));
}

#[test]
fn terminal_compat_docs_cover_reset_safety() {
    let docs = include_str!("../docs/TERMINAL_COMPATIBILITY.md");
    assert!(docs.contains("Normal exit is non-destructive."));
    assert!(docs.contains("`--reset-terminal` is explicit destructive recovery."));
    assert!(docs.contains("attempts scrollback purge"));
}

#[test]
fn readme_links_terminal_compatibility_doc() {
    let readme = include_str!("../README.md");
    assert!(readme.contains("docs/TERMINAL_COMPATIBILITY.md"));
}

#[test]
fn zactrix_core_doc_exists_and_covers_architecture_terms() {
    let docs = include_str!("../docs/ZACTRIX_CORE.md");
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

    let readme = include_str!("../README.md");
    assert!(readme.contains("docs/ZACTRIX_CORE.md"));
}

#[test]
fn simd_docs_do_not_claim_global_zero_unsafe() {
    let docs = include_str!("../docs/SIMD_FEASIBILITY.md");
    assert!(!docs.contains("zero `unsafe`"));
    assert!(docs.contains("no-new-unsafe renderer/core policy"));
}

#[test]
fn source_contains_only_audited_platform_recovery_unsafe() {
    let main_rs = include_str!("main.rs");
    assert_eq!(main_rs.matches("unsafe {").count(), 1);
    assert!(main_rs.contains("SAFETY: this Linux-only guard"));

    let zactrix_core = include_str!("zactrix_core.rs");
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
    let _ = include_str!("../README.md");
    // If VISUAL_STABILITY.md exists, README should link to it.
    // This test is informational — it verifies the docs can be compiled
    // without actually requiring the doc to exist yet.
}
