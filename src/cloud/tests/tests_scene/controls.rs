// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Controls tests — speed/density/glitch/color after scene switch,
//! unknown scene guard, existing controls still work.

use super::{make_glyph_cloud, make_monolith_cloud};
use crate::constants::RUNTIME_SPEED_MAX;
use crate::runtime::ColorScheme;

#[test]
fn monolith_scene_applies_neon_purple_color() {
    let mut cloud = make_glyph_cloud();
    cloud.apply_scene_runtime("monolith", "braille", &[], false);
    assert_eq!(cloud.color_scheme(), ColorScheme::NeonPurple);
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
    // Signal scene sets density=0.55
    assert!((cloud.droplet_density - 0.55).abs() < 0.001);
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
    // Matrix scene applies neon-green color.
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    assert_eq!(cloud.color_scheme(), ColorScheme::NeonGreen);
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

/// v35.2 audit-test (FPS-F2/F3 contract): `apply_scene_runtime("low-power")`
/// applies the scene's `speed`/`density`/`color`/`charset`/`glitch_level`
/// at runtime, but NOT its `fps` field. Cloud does not own `target_fps` —
/// it lives on `PowerManager` in `event_loop.rs`, which is NOT mutated by
/// `apply_scene_runtime`. This locks the "scene fps is startup-only by
/// design" contract documented in `termdetect.rs` §FPS Precedence Chain.
/// If a future refactor adds runtime fps application, this test will fail
/// loudly, forcing the author to either (a) update the docs + this test
/// to acknowledge the new behavior, or (b) revert the change.
#[test]
fn low_power_scene_runtime_applies_speed_density_glitch_only_not_fps() {
    let mut cloud = make_glyph_cloud();
    cloud.set_chars_per_sec(60.0);
    cloud.set_droplet_density(1.0);
    cloud.apply_scene_runtime("low-power", "binary", &[], false);

    // These fields ARE applied at runtime (the CPU shed):
    assert_eq!(cloud.chars_per_sec, 5.0, "low-power scene must set speed=5");
    assert_eq!(
        cloud.droplet_density, 0.45,
        "low-power scene must set density=0.45"
    );
    assert!(!cloud.glitchy, "low-power scene must set glitch_level=None");
    assert_eq!(
        cloud.color_scheme(),
        ColorScheme::Green,
        "low-power scene must set color=green"
    );

    // The fps=30 field from `low-power`'s SceneConfig is NOT applied at
    // runtime — there is no `cloud.target_fps` field to even read. The
    // event loop's `PowerManager.base_target_fps()` stays at whatever was
    // resolved at startup (CLI/config/dynamic-default). This is the
    // documented FPS-F2/F3 contract.
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
}
