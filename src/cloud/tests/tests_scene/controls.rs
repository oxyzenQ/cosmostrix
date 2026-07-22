// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Controls tests — speed/density/glitch/color after scene switch,
//! unknown scene guard, existing controls still work.

use super::{make_glyph_cloud, make_monolith_cloud};
use crate::constants::RUNTIME_SPEED_MAX;
use crate::runtime::ColorScheme;

#[test]
fn monolith_scene_applies_cosmos_color() {
    let mut cloud = make_glyph_cloud();
    cloud.apply_scene_runtime("monolith", "braille", &[], false);
    assert_eq!(cloud.color_scheme(), ColorScheme::Cosmos);
}

#[test]
fn signal_scene_applies_aurora_color() {
    let mut cloud = make_glyph_cloud();
    cloud.apply_scene_runtime("signal", "binary", &[], false);
    assert_eq!(cloud.color_scheme(), ColorScheme::Aurora);
}

#[test]
fn speed_updates_after_scene_switch() {
    let mut cloud = make_glyph_cloud();
    cloud.set_chars_per_sec(5.0);
    cloud.apply_scene_runtime("monolith", "braille", &[], false);
    // Monolith scene sets speed=30
    assert_eq!(cloud.chars_per_sec, 30.0);
}

#[test]
fn speed_remains_clamped_after_scene_switch() {
    let mut cloud = make_glyph_cloud();
    cloud.apply_scene_runtime("monolith", "braille", &[], false);
    // Speed should be within valid range
    assert!(cloud.chars_per_sec >= 1.0);
    assert!(cloud.chars_per_sec <= RUNTIME_SPEED_MAX);
}

#[test]
fn density_updates_after_scene_switch() {
    let mut cloud = make_glyph_cloud();
    cloud.set_droplet_density(1.0);
    cloud.apply_scene_runtime("monolith", "braille", &[], false);
    // Monolith scene sets density=0.85
    assert!((cloud.droplet_density - 0.85).abs() < 0.001);
}

#[test]
fn signal_density_is_high() {
    let mut cloud = make_glyph_cloud();
    cloud.apply_scene_runtime("signal", "binary", &[], false);
    // Signal scene sets density=0.70
    assert!((cloud.droplet_density - 0.70).abs() < 0.001);
}

#[test]
fn glitch_level_subtle_applied_from_monolith() {
    let mut cloud = make_glyph_cloud();
    cloud.glitchy = false;
    cloud.glitch_pct = 0.0;
    cloud.apply_scene_runtime("monolith", "braille", &[], false);
    assert!(cloud.glitchy);
    // Subtle glitch: pct=0.03
    assert!((cloud.glitch_pct - 0.03).abs() < 0.001);
}

#[test]
fn matrix_scene_keeps_current_color() {
    let mut cloud = make_glyph_cloud();
    // Matrix has color=None, so it should keep current color (Green)
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    assert_eq!(cloud.color_scheme(), ColorScheme::Green);
}

#[test]
fn unknown_scene_does_not_change_state() {
    let mut cloud = make_monolith_cloud();
    let original_scene = cloud.active_scene().to_string();
    let original_style = cloud.rain_style();
    let result = cloud.apply_scene_runtime("nonexistent", "binary", &[], false);
    assert_eq!(cloud.active_scene(), original_scene);
    assert_eq!(cloud.rain_style(), original_style);
    assert_eq!(result, "binary");
}

#[test]
fn existing_controls_still_work_after_scene_switch() {
    let mut cloud = make_monolith_cloud();
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    // Speed up/down should still work
    let original_cps = cloud.chars_per_sec;
    cloud.set_chars_per_sec(original_cps + 1.0);
    assert!(cloud.chars_per_sec > original_cps);
    // Density should still work
    cloud.set_droplet_density(cloud.droplet_density + 0.1);
    // Glitch toggle should still work
    cloud.set_glitchy(!cloud.glitchy);
}
