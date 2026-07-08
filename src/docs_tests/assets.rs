// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Demo asset existence and ordering guards.
//! Updated for v13 assets — references v13 demo GIF and screenshots.

#[test]
fn readme_references_v13_demo_gif() {
    let readme = include_str!("../../README.md");
    assert!(
        readme.contains("assets/cosmostrix-v13-demo.gif"),
        "README must reference the v13 demo GIF"
    );
}

#[test]
fn readme_references_v13_demo_screenshots() {
    let readme = include_str!("../../README.md");
    for name in &[
        "cosmostrix-v13-demo-binary.png",
        "cosmostrix-v13-demo-hacker.png",
        "cosmostrix-v13-demo-retro.png",
        "cosmostrix-v13-demo-braille.png",
        "cosmostrix-v13-demo-green-retro.png",
    ] {
        assert!(readme.contains(name), "README must reference {name}");
    }
}

#[test]
fn v13_demo_gif_asset_exists() {
    let path = std::path::Path::new("assets/cosmostrix-v13-demo.gif");
    assert!(path.exists(), "assets/cosmostrix-v13-demo.gif must exist");
}

#[test]
fn v13_demo_binary_asset_exists() {
    let path = std::path::Path::new("assets/cosmostrix-v13-demo-binary.png");
    assert!(path.exists(), "assets/cosmostrix-v13-demo-binary.png must exist");
}

#[test]
fn v13_demo_hacker_asset_exists() {
    let path = std::path::Path::new("assets/cosmostrix-v13-demo-hacker.png");
    assert!(path.exists(), "assets/cosmostrix-v13-demo-hacker.png must exist");
}

#[test]
fn v13_demo_retro_asset_exists() {
    let path = std::path::Path::new("assets/cosmostrix-v13-demo-retro.png");
    assert!(path.exists(), "assets/cosmostrix-v13-demo-retro.png must exist");
}

#[test]
fn v13_demo_braille_asset_exists() {
    let path = std::path::Path::new("assets/cosmostrix-v13-demo-braille.png");
    assert!(path.exists(), "assets/cosmostrix-v13-demo-braille.png must exist");
}

#[test]
fn v13_demo_green_retro_asset_exists() {
    let path = std::path::Path::new("assets/cosmostrix-v13-demo-green-retro.png");
    assert!(path.exists(), "assets/cosmostrix-v13-demo-green-retro.png must exist");
}

#[test]
fn readme_gif_appears_before_screenshots() {
    let readme = include_str!("../../README.md");
    let gif_pos = readme
        .find("cosmostrix-v13-demo.gif")
        .expect("README must contain GIF ref");
    let first_screenshot = readme
        .find("cosmostrix-v13-demo-binary.png")
        .expect("README must contain binary PNG ref");
    assert!(
        gif_pos < first_screenshot,
        "README GIF reference must appear before screenshot PNGs"
    );
}

#[test]
fn readme_does_not_use_old_demo_assets() {
    let readme = include_str!("../../README.md");
    assert!(
        !readme.contains("assets/cosmostrix-v12-demo"),
        "README must not reference v12 demos"
    );
    assert!(
        !readme.contains("assets/cosmostrix-v11-demo"),
        "README must not reference v11 demos"
    );
    assert!(
        !readme.contains("assets/cosmostrix-v4-demo"),
        "README must not reference v4 demos"
    );
}

#[test]
fn old_demo_assets_removed() {
    for name in &[
        "cosmostrix-v12-demo.gif",
        "cosmostrix-v12-demo-cyberpunk.png",
        "cosmostrix-v12-demo-blocks.png",
        "cosmostrix-v12-demo-braille.png",
        "cosmostrix-v12-demo-retro.png",
        "cosmostrix-v12-demo-hacker.png",
        "cosmostrix-v11-demo.gif",
        "cosmostrix-v11-demo-retro.png",
        "cosmostrix-v11-demo-braille.png",
        "cosmostrix-v11-demo-hex.png",
        "cosmostrix-v4-demo.gif",
        "cosmostrix-v4-demo.mp4",
        "cosmostrix-v4-demo-binary.png",
        "cosmostrix-v4-demo-retro.png",
        "cosmostrix-demo.gif",
    ] {
        let p = std::path::Path::new("assets").join(name);
        assert!(!p.exists(), "Old asset must be removed: {name}");
    }
}
