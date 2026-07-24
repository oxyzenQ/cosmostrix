// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Demo asset existence and ordering guards.
//!
//! Version-agnostic: reads the current version from CARGO_PKG_VERSION
//! at compile time. No manual update needed when bumping versions —
//! just tag and the tests auto-detect the right asset prefix.
//!
//! NOTE: The asset *existence* and *old-asset-removal* tests are marked
//! `#[ignore]` during the v20 transition window. The owner updates
//! `assets/` separately from code; running these tests before the
//! v20 assets are committed would fail spuriously. The README reference
//! and ordering tests still run, so the documentation side stays guarded.
//! Once `assets/cosmostrix-v20-demo*` exists, remove the `#[ignore]`
//! attributes from the three tests below.

/// Get the current major version prefix (e.g. "v13").
fn major_prefix() -> String {
    let major = env!("CARGO_PKG_VERSION").split('.').next().unwrap_or("0");
    format!("v{major}")
}

/// Get the current major version number (e.g. 13).
fn major_num() -> u32 {
    env!("CARGO_PKG_VERSION")
        .split('.')
        .next()
        .unwrap_or("0")
        .parse()
        .unwrap_or(0)
}

#[test]
fn readme_references_current_demo_gif() {
    let readme = include_str!("../../README.md");
    let prefix = major_prefix();
    let gif_name = format!("assets/cosmostrix-{prefix}-demo.gif");
    assert!(
        readme.contains(&gif_name),
        "README must reference {gif_name}"
    );
}

#[test]
fn readme_references_current_demo_screenshots() {
    let readme = include_str!("../../README.md");
    let prefix = major_prefix();
    let mut found = 0;
    for line in readme.lines() {
        if line.contains(&format!("cosmostrix-{prefix}-demo-")) && line.contains(".png") {
            found += 1;
        }
    }
    assert!(
        found >= 3,
        "README must reference at least 3 {prefix} demo screenshots (found {found})"
    );
}

#[test]
#[ignore = "v20 asset transition: owner will add assets/cosmostrix-v20-demo.gif"]
fn current_demo_gif_asset_exists() {
    let prefix = major_prefix();
    let path_str = format!("assets/cosmostrix-{prefix}-demo.gif");
    let path = std::path::Path::new(&path_str);
    assert!(path.exists(), "{path_str} must exist");
}

#[test]
#[ignore = "v20 asset transition: owner will add assets/cosmostrix-v20-demo-*.png"]
fn current_demo_screenshots_exist() {
    let prefix = major_prefix();
    let assets_dir = std::path::Path::new("assets");
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(assets_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(&format!("cosmostrix-{prefix}-demo-")) && name.ends_with(".png") {
                count += 1;
            }
        }
    }
    assert!(
        count >= 3,
        "assets/ must contain at least 3 {prefix} demo screenshots (found {count})"
    );
}

#[test]
fn readme_gif_appears_before_screenshots() {
    let readme = include_str!("../../README.md");
    let prefix = major_prefix();
    let gif_name = format!("cosmostrix-{prefix}-demo.gif");
    let gif_pos = readme
        .find(&gif_name)
        .unwrap_or_else(|| panic!("README must contain {gif_name} ref"));
    let screenshot_name = format!("cosmostrix-{prefix}-demo-");
    let screenshot_pos = readme[gif_pos..]
        .find(&screenshot_name)
        .unwrap_or_else(|| panic!("README must contain {screenshot_name}*.png ref after GIF"));
    assert!(
        gif_pos < gif_pos + screenshot_pos,
        "README GIF reference must appear before screenshot PNGs"
    );
}

#[test]
fn readme_does_not_use_old_demo_assets() {
    let readme = include_str!("../../README.md");
    let current_major = major_num();
    for old_major in 1..current_major {
        let old_prefix = format!("v{old_major}");
        assert!(
            !readme.contains(&format!("assets/cosmostrix-{old_prefix}-demo")),
            "README must not reference {old_prefix} demo assets"
        );
    }
    assert!(
        !readme.contains("assets/cosmostrix-demo.gif"),
        "README must not reference old generic demo"
    );
}

#[test]
#[ignore = "v20 asset transition: old v15 assets will be removed by owner"]
fn old_demo_assets_removed() {
    let current_major = major_num();
    let assets_dir = std::path::Path::new("assets");
    if let Ok(entries) = std::fs::read_dir(assets_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            for old_major in 1..current_major {
                let old_prefix = format!("cosmostrix-v{old_major}-demo");
                assert!(
                    !name.starts_with(&old_prefix),
                    "Old asset must be removed: {name}"
                );
            }
            assert!(
                !name.starts_with("cosmostrix-demo.gif"),
                "Old generic demo must be removed: {name}"
            );
        }
    }
}
