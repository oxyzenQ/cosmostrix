// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Demo asset existence and ordering guards.
//!
//! Demo assets use stable, version-less names (introduced in
//! 27f7d4f5, replacing the old version-prefixed scheme):
//!   assets/cosmostrix-video.gif    raw 60 fps render (encode source)
//!   assets/cosmostrix-video.webp   animated demo embedded in README
//!   assets/cosmostrix-4-scene.png  static four-scene showcase
//!
//! Version bumps no longer rename assets or rewrite README demo
//! refs; anything still keyed to the retired cosmostrix-v{MAJOR}-demo
//! naming is stale and must be removed. assets/tools.md documents
//! how the video assets are regenerated (gifski and ffmpeg).

/// The animated demo embedded in README.md.
const VIDEO_WEBP: &str = "assets/cosmostrix-video.webp";

/// The raw render the webp is encoded from (see assets/tools.md).
const VIDEO_GIF: &str = "assets/cosmostrix-video.gif";

/// The static four-scene showcase embedded in README.md.
const SCENE_PNG: &str = "assets/cosmostrix-4-scene.png";

#[test]
fn readme_references_video_demo() {
    let readme = include_str!("../../README.md");
    assert!(
        readme.contains(VIDEO_WEBP),
        "README must reference {VIDEO_WEBP}"
    );
}

#[test]
fn readme_references_scene_screenshot() {
    let readme = include_str!("../../README.md");
    assert!(
        readme.contains(SCENE_PNG),
        "README must reference {SCENE_PNG}"
    );
}

#[test]
fn video_assets_exist() {
    for path in [VIDEO_GIF, VIDEO_WEBP] {
        assert!(
            std::path::Path::new(path).exists(),
            "{path} must exist (regenerate via the commands in assets/tools.md)"
        );
    }
}

#[test]
fn scene_screenshot_exists() {
    assert!(
        std::path::Path::new(SCENE_PNG).exists(),
        "{SCENE_PNG} must exist"
    );
}

#[test]
fn readme_video_appears_before_screenshots() {
    let readme = include_str!("../../README.md");
    let video_pos = readme
        .find(VIDEO_WEBP)
        .unwrap_or_else(|| panic!("README must contain {VIDEO_WEBP} ref"));
    let scene_pos = readme
        .find(SCENE_PNG)
        .unwrap_or_else(|| panic!("README must contain {SCENE_PNG} ref"));
    assert!(
        video_pos < scene_pos,
        "README video demo must appear before the scene screenshot"
    );
}

#[test]
fn readme_does_not_use_retired_demo_assets() {
    let readme = include_str!("../../README.md");
    for line in readme.lines() {
        let stale = line.contains("cosmostrix-v") && line.contains("-demo");
        assert!(
            !stale,
            "README must not use the retired version-prefixed demo scheme: {line}"
        );
    }
    assert!(
        !readme.contains("assets/cosmostrix-demo.gif"),
        "README must not reference the old generic demo"
    );
}

#[test]
fn retired_demo_assets_removed() {
    let assets_dir = std::path::Path::new("assets");
    let entries = std::fs::read_dir(assets_dir).expect("assets/ directory must exist");
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let stale = name.starts_with("cosmostrix-v") && name.contains("-demo");
        assert!(
            !stale,
            "Retired version-prefixed demo asset must be removed: {name}"
        );
        assert!(
            !name.starts_with("cosmostrix-demo.gif"),
            "Old generic demo must be removed: {name}"
        );
    }
}
