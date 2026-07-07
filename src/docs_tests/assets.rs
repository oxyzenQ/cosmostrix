// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Demo asset existence and ordering guards.

#[test]
fn readme_references_v12_demo_gif() {
    let readme = include_str!("../../README.md");
    assert!(
        readme.contains("assets/cosmostrix-v12-demo.gif"),
        "README must reference the v12 demo GIF"
    );
}

#[test]
fn readme_references_v12_demo_screenshots() {
    let readme = include_str!("../../README.md");
    for name in &[
        "cosmostrix-v12-demo-cyberpunk.png",
        "cosmostrix-v12-demo-hacker.png",
        "cosmostrix-v12-demo-retro.png",
        "cosmostrix-v12-demo-braille.png",
        "cosmostrix-v12-demo-blocks.png",
    ] {
        assert!(readme.contains(name), "README must reference {name}");
    }
}

#[test]
fn v12_demo_gif_asset_exists() {
    let path = std::path::Path::new("assets/cosmostrix-v12-demo.gif");
    assert!(path.exists(), "assets/cosmostrix-v12-demo.gif must exist");
}

#[test]
fn v12_demo_cyberpunk_asset_exists() {
    let path = std::path::Path::new("assets/cosmostrix-v12-demo-cyberpunk.png");
    assert!(
        path.exists(),
        "assets/cosmostrix-v12-demo-cyberpunk.png must exist"
    );
}

#[test]
fn v12_demo_hacker_asset_exists() {
    let path = std::path::Path::new("assets/cosmostrix-v12-demo-hacker.png");
    assert!(
        path.exists(),
        "assets/cosmostrix-v12-demo-hacker.png must exist"
    );
}

#[test]
fn v12_demo_retro_asset_exists() {
    let path = std::path::Path::new("assets/cosmostrix-v12-demo-retro.png");
    assert!(
        path.exists(),
        "assets/cosmostrix-v12-demo-retro.png must exist"
    );
}

#[test]
fn v12_demo_braille_asset_exists() {
    let path = std::path::Path::new("assets/cosmostrix-v12-demo-braille.png");
    assert!(
        path.exists(),
        "assets/cosmostrix-v12-demo-braille.png must exist"
    );
}

#[test]
fn v12_demo_blocks_asset_exists() {
    let path = std::path::Path::new("assets/cosmostrix-v12-demo-blocks.png");
    assert!(
        path.exists(),
        "assets/cosmostrix-v12-demo-blocks.png must exist"
    );
}

#[test]
fn readme_gif_appears_before_screenshots() {
    let readme = include_str!("../../README.md");
    let gif_pos = readme
        .find("cosmostrix-v12-demo.gif")
        .expect("README must contain GIF ref");
    let cyberpunk_pos = readme
        .find("cosmostrix-v12-demo-cyberpunk.png")
        .expect("README must contain cyberpunk PNG ref");
    assert!(
        gif_pos < cyberpunk_pos,
        "README GIF reference must appear before screenshot PNGs"
    );
}

#[test]
fn readme_does_not_use_old_demo_gif_as_primary() {
    let readme = include_str!("../../README.md");
    assert!(
        !readme.contains("cosmostrix-demo.gif"),
        "README must not reference the old demo GIF"
    );
}

#[test]
fn old_demo_gif_removed_from_assets() {
    let path = std::path::Path::new("assets/cosmostrix-demo.gif");
    assert!(
        !path.exists(),
        "Old assets/cosmostrix-demo.gif should have been removed"
    );
}
