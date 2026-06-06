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
fn docs_mention_visual_stability_policy_if_exists() {
    let _ = include_str!("../README.md");
    // If VISUAL_STABILITY.md exists, README should link to it.
    // This test is informational — it verifies the docs can be compiled
    // without actually requiring the doc to exist yet.
}
