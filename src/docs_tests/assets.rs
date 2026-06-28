// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Demo asset existence and ordering guards.

#[test]
fn readme_references_v4_demo_gif() {
    let readme = include_str!("../../README.md");
    assert!(
        readme.contains("assets/cosmostrix-v4-demo.gif"),
        "README must reference the v4 demo GIF"
    );
}

#[test]
fn readme_references_v4_demo_video() {
    let readme = include_str!("../../README.md");
    assert!(
        readme.contains("assets/cosmostrix-v4-demo.mp4"),
        "README must reference the v4 demo video"
    );
}

#[test]
fn readme_references_v4_demo_binary_poster() {
    let readme = include_str!("../../README.md");
    assert!(
        readme.contains("assets/cosmostrix-v4-demo-binary.png"),
        "README must reference the v4 binary demo poster"
    );
}

#[test]
fn readme_references_v4_demo_retro_poster() {
    let readme = include_str!("../../README.md");
    assert!(
        readme.contains("assets/cosmostrix-v4-demo-retro.png"),
        "README must reference the v4 retro demo poster"
    );
}

#[test]
fn v4_demo_gif_asset_exists() {
    let path = std::path::Path::new("assets/cosmostrix-v4-demo.gif");
    assert!(path.exists(), "assets/cosmostrix-v4-demo.gif must exist");
}

#[test]
fn v4_demo_video_asset_exists() {
    let path = std::path::Path::new("assets/cosmostrix-v4-demo.mp4");
    assert!(path.exists(), "assets/cosmostrix-v4-demo.mp4 must exist");
}

#[test]
fn v4_demo_binary_poster_asset_exists() {
    let path = std::path::Path::new("assets/cosmostrix-v4-demo-binary.png");
    assert!(
        path.exists(),
        "assets/cosmostrix-v4-demo-binary.png must exist"
    );
}

#[test]
fn v4_demo_retro_poster_asset_exists() {
    let path = std::path::Path::new("assets/cosmostrix-v4-demo-retro.png");
    assert!(
        path.exists(),
        "assets/cosmostrix-v4-demo-retro.png must exist"
    );
}

#[test]
fn readme_gif_appears_before_poster_pngs() {
    let readme = include_str!("../../README.md");
    let gif_pos = readme
        .find("cosmostrix-v4-demo.gif")
        .expect("README must contain GIF ref");
    let binary_pos = readme
        .find("cosmostrix-v4-demo-binary.png")
        .expect("README must contain binary PNG ref");
    let retro_pos = readme
        .find("cosmostrix-v4-demo-retro.png")
        .expect("README must contain retro PNG ref");
    assert!(
        gif_pos < binary_pos,
        "README GIF reference must appear before binary PNG"
    );
    assert!(
        binary_pos < retro_pos,
        "README binary PNG must appear before retro PNG"
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
