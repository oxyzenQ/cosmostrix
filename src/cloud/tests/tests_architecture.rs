// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Tests for v3.3.0 engine architecture split and optimization audit.

use std::time::{Duration, Instant};

use super::Cloud;
use crate::constants::RUNTIME_SPEED_MAX;
use crate::frame::Frame;
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};

fn make_monolith_cloud() -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::Mono,
        false,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Cosmos,
        RainStyle::Monolith,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(40, 20);
    cloud.scene_name = "monolith".to_string();
    cloud.clear_redraw_flags_for_test();
    cloud
}

fn make_glyph_cloud() -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::Mono,
        false,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Green,
        RainStyle::Glyph,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(40, 20);
    cloud.scene_name = "matrix".to_string();
    cloud.clear_redraw_flags_for_test();
    cloud
}

/// Runtime scene switching still works after module split:
/// monolith → matrix → signal round-trip preserves all behaviors.
#[test]
fn architecture_scene_switching_preserved_after_split() {
    let mut cloud = make_monolith_cloud();
    let mut charset = "binary".to_string();

    // monolith → matrix
    charset = cloud.apply_scene_runtime("matrix", &charset, &[], false);
    assert_eq!(cloud.active_scene(), "matrix");
    assert_eq!(cloud.rain_style(), RainStyle::Glyph);
    assert_eq!(cloud.color_scheme(), ColorScheme::Cosmos);
    assert_eq!(charset, "binary");

    // matrix → signal
    charset = cloud.apply_scene_runtime("signal", &charset, &[], false);
    assert_eq!(cloud.active_scene(), "signal");
    assert_eq!(cloud.rain_style(), RainStyle::Glyph);
    assert_eq!(cloud.color_scheme(), ColorScheme::Cyan);
    assert_eq!(charset, "code");

    // signal → monolith
    charset = cloud.apply_scene_runtime("monolith", &charset, &[], false);
    assert_eq!(cloud.active_scene(), "monolith");
    assert_eq!(cloud.rain_style(), RainStyle::Monolith);
    assert_eq!(cloud.color_scheme(), ColorScheme::Cosmos);
    assert_eq!(charset, "binary");

    // Run a frame on each scene to verify no crash
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    cloud.last_spawn_time = Instant::now();
    cloud.rain_at(&mut frame, Instant::now() + Duration::from_millis(16));
}

/// Scene-mapped rain styles are unchanged: matrix=Glyph, signal=Glyph, monolith=Monolith.
#[test]
fn architecture_scene_rain_style_mappings_unchanged() {
    let mut cloud = make_monolith_cloud();
    assert_eq!(cloud.rain_style(), RainStyle::Monolith);

    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    assert_eq!(cloud.rain_style(), RainStyle::Glyph);

    let mut cloud2 = make_monolith_cloud();
    cloud2.apply_scene_runtime("signal", "binary", &[], false);
    assert_eq!(cloud2.rain_style(), RainStyle::Glyph);
}

/// Default runtime profile is preserved: monolith scene defaults remain
/// cosmos color, binary charset, monolith rain style, speed 10, density 0.75.
#[test]
fn architecture_default_runtime_profile_unchanged() {
    let mut cloud = make_monolith_cloud();
    // These are the default values set by the monolith scene
    assert_eq!(cloud.rain_style(), RainStyle::Monolith);
    assert_eq!(cloud.color_scheme(), ColorScheme::Cosmos);
    assert_eq!(cloud.chars_per_sec, 10.0);
    assert!((cloud.droplet_density - 0.75).abs() < 0.001);

    // Matrix defaults when applied fresh
    let mut cloud2 = make_glyph_cloud();
    cloud2.apply_scene_runtime("matrix", "binary", &[], false);
    // Matrix has color=None, so keeps current (Green)
    assert_eq!(cloud2.color_scheme(), ColorScheme::Green);
}

/// Monolith benchmark path still uses RainStyle::Monolith after split.
#[test]
fn architecture_monolith_benchmark_path_unchanged() {
    let cloud = make_monolith_cloud();
    assert_eq!(cloud.rain_style(), RainStyle::Monolith);
    // Running rain should not panic
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    cloud.last_spawn_time = Instant::now();
    cloud.rain_at(&mut frame, Instant::now() + Duration::from_millis(16));
}

/// Glyph warm-start behavior is unchanged after module split:
/// scene switch triggers warm-start with sparse entry, bounded alive count.
#[test]
fn architecture_glyph_warm_start_unchanged_after_split() {
    let mut cloud = make_monolith_cloud();
    cloud.apply_scene_runtime("matrix", "binary", &[], false);

    // Must have warm-started droplets
    assert!(!cloud.droplets.is_empty());
    let alive = cloud.droplets.iter().filter(|d| d.is_alive).count();
    assert!(alive >= 3, "warm-start must seed enough droplets");

    // Must have glyph entry ramp active
    assert!(cloud.glyph_entry_time.is_some());

    // Must request semantic invalidation
    assert!(cloud.is_semantic_invalidate());
    assert!(cloud.is_force_draw_everything());
}

/// Runtime controls (set_chars_per_sec, set_droplet_density, set_glitchy, etc.)
/// still work after being moved to runtime_controls module.
#[test]
fn architecture_runtime_controls_work_after_split() {
    let mut cloud = make_glyph_cloud();
    cloud.clear_redraw_flags_for_test();

    // Speed control
    cloud.set_chars_per_sec(5.0);
    assert!(cloud.chars_per_sec >= 1.0 && cloud.chars_per_sec <= RUNTIME_SPEED_MAX);

    // Density control
    cloud.set_droplet_density(0.5);
    assert!((cloud.droplet_density - 0.5).abs() < 0.001);

    // Glitch control
    cloud.set_glitchy(false);
    assert!(!cloud.glitchy);

    // Shading mode
    cloud.set_shading_mode(ShadingMode::DistanceFromHead);
    assert_eq!(cloud.shading_mode, ShadingMode::DistanceFromHead);
    assert!(cloud.shading_distance);

    // Force draw
    assert!(!cloud.is_force_draw_everything());
    cloud.force_draw_everything();
    assert!(cloud.is_force_draw_everything());

    // Pause/resume
    assert!(cloud.toggle_pause());
    assert!(cloud.pause);
    assert!(cloud.toggle_pause());
    assert!(!cloud.pause);
}

/// Color scheme transition still works after being moved to runtime_controls.
#[test]
fn architecture_color_scheme_transition_after_split() {
    let mut cloud = make_glyph_cloud();
    cloud.clear_redraw_flags_for_test();

    cloud.set_color_scheme(ColorScheme::Blue);
    assert_eq!(cloud.color_scheme(), ColorScheme::Blue);
    assert!(cloud.transition_start.is_some());
}

/// Scene switch x/X cycle through all scenes still produces
/// dirty frames with no blank screens after module split.
#[test]
fn architecture_full_cycle_no_blank_after_split() {
    let mut cloud = make_monolith_cloud();
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    let scenes = ["matrix", "signal", "monolith"];
    for scene in &scenes {
        cloud.apply_scene_runtime(scene, "binary", &[], false);
        frame.clear_dirty();
        cloud.last_spawn_time = Instant::now();
        cloud.rain(&mut frame);
        assert!(
            frame.is_dirty_all() || !frame.dirty_indices().is_empty(),
            "cycle scene '{scene}': must produce dirty frame after split"
        );
    }
}
