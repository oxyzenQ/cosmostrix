// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Demo asset existence and ordering guards.

#[test]
fn readme_references_v11_demo_gif() {
    let readme = include_str!("../../README.md");
    assert!(
        readme.contains("assets/cosmostrix-v11-demo.gif"),
        "README must reference the v11 demo GIF"
    );
}

#[test]
fn v11_demo_gif_asset_exists() {
    let path = std::path::Path::new("assets/cosmostrix-v11-demo.gif");
    assert!(path.exists(), "assets/cosmostrix-v11-demo.gif must exist");
}

#[test]
fn v11_demo_retro_asset_exists() {
    let path = std::path::Path::new("assets/cosmostrix-v11-demo-retro.png");
    assert!(
        path.exists(),
        "assets/cosmostrix-v11-demo-retro.png must exist"
    );
}

#[test]
fn v11_demo_braille_asset_exists() {
    let path = std::path::Path::new("assets/cosmostrix-v11-demo-braille.png");
    assert!(
        path.exists(),
        "assets/cosmostrix-v11-demo-braille.png must exist"
    );
}

#[test]
fn v11_demo_hex_asset_exists() {
    let path = std::path::Path::new("assets/cosmostrix-v11-demo-hex.png");
    assert!(
        path.exists(),
        "assets/cosmostrix-v11-demo-hex.png must exist"
    );
}

#[test]
fn readme_gif_appears_before_screenshots() {
    let readme = include_str!("../../README.md");
    let gif_pos = readme
        .find("cosmostrix-v11-demo.gif")
        .expect("README must contain GIF ref");
    let retro_pos = readme
        .find("cosmostrix-v11-demo-retro.png")
        .expect("README must contain retro PNG ref");
    assert!(
        gif_pos < retro_pos,
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
