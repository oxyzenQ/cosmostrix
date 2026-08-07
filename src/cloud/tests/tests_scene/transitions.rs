// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Scene transition tests — monolith↔glyph switches, dirty-frame behavior,
//! semantic invalidation, spawn debt clearing, force draw.

use std::time::{Duration, Instant};

use super::{has_dirty_cells, make_glyph_cloud, make_monolith_cloud};
use crate::frame::Frame;
use crate::rain_style::RainStyle;

#[test]
fn monolith_to_matrix_changes_rain_style_to_glyph() {
    let mut cloud = make_monolith_cloud();
    assert_eq!(cloud.rain_style(), RainStyle::Monolith);
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    assert_eq!(cloud.rain_style(), RainStyle::Glyph);
}

#[test]
fn matrix_to_monolith_changes_rain_style_to_monolith() {
    let mut cloud = make_glyph_cloud();
    assert_eq!(cloud.rain_style(), RainStyle::Glyph);
    cloud.apply_scene_runtime("monolith", "braille", &[], false);
    assert_eq!(cloud.rain_style(), RainStyle::Monolith);
}

#[test]
fn switching_from_monolith_clears_draw_history() {
    let mut cloud = make_monolith_cloud();
    // Simulate some monolith draw activity
    cloud.monolith_rain.reset(40);
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    // Draw history should be empty after switching away from monolith
    assert_eq!(cloud.monolith_rain.draw_history_count_for_test(), 0);
}

#[test]
fn switching_into_monolith_initializes_state_cleanly() {
    let mut cloud = make_glyph_cloud();
    cloud.droplets.clear();
    cloud.apply_scene_runtime("monolith", "braille", &[], false);
    // Monolith should be reset and ready
    assert_eq!(cloud.rain_style(), RainStyle::Monolith);
    assert_eq!(cloud.monolith_rain.draw_history_count_for_test(), 0);
    assert_eq!(cloud.active_scene(), "monolith");
}

#[test]
fn scene_switch_requests_semantic_invalidate() {
    let mut cloud = make_monolith_cloud();
    cloud.clear_redraw_flags_for_test();
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    assert!(cloud.is_semantic_invalidate());
}

#[test]
fn scene_switch_triggers_force_draw() {
    let mut cloud = make_monolith_cloud();
    cloud.clear_redraw_flags_for_test();
    cloud.apply_scene_runtime("signal", "binary", &[], false);
    assert!(cloud.is_force_draw_everything());
}

/// Scene switch must request semantic invalidation for safe redraw sync.
#[test]
fn scene_switch_glyph_requests_semantic_sync() {
    let mut cloud = make_monolith_cloud();
    cloud.clear_redraw_flags_for_test();
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    assert!(
        cloud.is_semantic_invalidate(),
        "glyph scene switch must request semantic invalidation"
    );
}

#[test]
fn scene_switch_drops_spawn_debt() {
    let mut cloud = make_monolith_cloud();
    cloud.spawn_remainder = 100.0;
    cloud.last_spawn_time = Instant::now() - Duration::from_secs(5);
    // Switching to monolith resets spawn debt
    cloud.apply_scene_runtime("monolith", "braille", &[], false);
    assert!(
        cloud.spawn_remainder < 1.0,
        "monolith scene switch should drop spawn debt"
    );
}

/// After switching monolith → matrix, the first rain frame must produce
/// visible dirty cells — no blank black intermediate screen.
#[test]
fn monolith_to_matrix_produces_dirty_glyph_frame() {
    let mut cloud = make_monolith_cloud();
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    let now = Instant::now();
    cloud.last_spawn_time = now;
    cloud.rain_at(&mut frame, now + Duration::from_millis(16));
    assert!(
        has_dirty_cells(&frame),
        "monolith→matrix: first frame must have dirty glyph cells"
    );
}

/// After switching monolith → signal, the first rain frame must produce
/// visible dirty cells — no blank black intermediate screen.
#[test]
fn monolith_to_signal_produces_dirty_glyph_frame() {
    let mut cloud = make_monolith_cloud();
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    cloud.apply_scene_runtime("signal", "binary", &[], false);
    let now = Instant::now();
    cloud.last_spawn_time = now;
    cloud.rain_at(&mut frame, now + Duration::from_millis(16));
    assert!(
        has_dirty_cells(&frame),
        "monolith→signal: first frame must have dirty glyph cells"
    );
}

/// After switching signal → monolith, the monolith scene should render
/// correctly (monolith has its own draw path, not glyph droplets).
#[test]
fn signal_to_monolith_produces_visible_frame() {
    let mut cloud = make_glyph_cloud();
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    cloud.apply_scene_runtime("monolith", "braille", &[], false);
    let now = Instant::now();
    cloud.last_spawn_time = now;
    cloud.rain_at(&mut frame, now + Duration::from_millis(16));
    assert!(
        has_dirty_cells(&frame),
        "signal→monolith: first frame must have dirty cells"
    );
}

/// Switching monolith → matrix must clear monolith draw history so no
/// monolith segmented residue persists in the glyph scene.
#[test]
fn monolith_to_matrix_clears_monolith_history_no_blank() {
    let mut cloud = make_monolith_cloud();
    cloud.monolith_rain.reset(40);
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    // Monolith history must be empty
    assert_eq!(cloud.monolith_rain.draw_history_count_for_test(), 0);
    // Glyph pool must be populated (not blank)
    assert!(!cloud.droplets.is_empty());
    let alive_count = cloud.droplets.iter().filter(|d| d.is_alive).count();
    assert!(
        alive_count > 0,
        "warm-start should seed at least 1 active droplet (got {alive_count})"
    );
    // First frame must render visible content
    cloud.last_spawn_time = Instant::now();
    cloud.rain(&mut frame);
    assert!(has_dirty_cells(&frame));
}

/// Switching monolith → signal must clear monolith draw history and
/// produce visible glyph content on the first frame.
#[test]
fn monolith_to_signal_clears_monolith_history_no_blank() {
    let mut cloud = make_monolith_cloud();
    cloud.monolith_rain.reset(40);
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    cloud.apply_scene_runtime("signal", "binary", &[], false);
    assert_eq!(cloud.monolith_rain.draw_history_count_for_test(), 0);
    assert!(!cloud.droplets.is_empty());
    cloud.last_spawn_time = Instant::now();
    cloud.rain(&mut frame);
    assert!(has_dirty_cells(&frame));
}

/// Repeated forward cycling (x key) through all scenes never yields
/// a blank frame. Each scene transition must produce dirty cells.
#[test]
fn repeated_forward_cycle_never_blank() {
    let mut cloud = make_monolith_cloud();
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    let scenes = ["matrix", "signal", "monolith"];
    for scene in &scenes {
        cloud.apply_scene_runtime(scene, "binary", &[], false);
        frame.clear_dirty();
        cloud.last_spawn_time = Instant::now();
        cloud.rain(&mut frame);
        assert!(
            has_dirty_cells(&frame),
            "forward cycle: scene '{scene}' must produce dirty frame"
        );
    }
}

/// Repeated uppercase X cycling forward through all scenes never yields
/// a blank frame. Each scene transition must produce dirty cells.
#[test]
fn repeated_uppercase_forward_cycle_never_blank() {
    let mut cloud = make_monolith_cloud();
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    let scenes = ["matrix", "signal", "monolith"];
    for scene in &scenes {
        cloud.apply_scene_runtime(scene, "binary", &[], false);
        frame.clear_dirty();
        cloud.last_spawn_time = Instant::now();
        cloud.rain(&mut frame);
        assert!(
            has_dirty_cells(&frame),
            "uppercase forward cycle: scene '{scene}' must produce dirty frame"
        );
    }
}

// ── apply_ambient_entry regressions ──
//
// v30.2: ambient entries are now scene-name-only. The previous regression
// (color/charset/speed/density being silently lost) is impossible by
// construction — the scene IS the spec, so when ambient fires `signal`,
// all of signal's defaults (color/charset/speed/density/glitch) are applied
// atomically via apply_scene_runtime_with_cfg. There is no override layer
// to lose.
//
// These tests verify the v30.2 contract:
// 1. apply_ambient_entry with a built-in scene name applies that scene's
//    managed defaults.
// 2. apply_ambient_entry with a custom scene name looks up the
//    [scene-custom.<name>] block, applies base-scene defaults first, then
//    the block's own overrides.

use crate::ambient::AmbientEntry;
use crate::cloud::Cloud;
use crate::runtime::ColorScheme;
use std::collections::HashMap;

/// Helper: build a glyph cloud whose starting state mirrors the cinematic
/// scene's defaults (color=neon-purple, charset=zen, speed=9.0, density=0.75).
fn make_cinematic_like_cloud() -> Cloud {
    let mut cloud = make_glyph_cloud();
    // Apply cinematic scene to mirror real startup state.
    cloud.apply_scene_runtime("cinematic", "zen", &[], false);
    // Verify our baseline assumptions — if these fail, the test setup is
    // wrong, not the bug.
    assert_eq!(cloud.color_scheme(), ColorScheme::NeonPurple);
    assert!((cloud.chars_per_sec - 9.0).abs() < 0.01);
    assert!((cloud.droplet_density - 0.75).abs() < 0.01);
    cloud
}

#[test]
fn apply_ambient_entry_builtin_scene_applies_scene_defaults() {
    // v30.2: ambient entry with a built-in scene name applies that scene's
    // managed defaults atomically. No override layer — the scene IS the spec.
    let mut cloud = make_cinematic_like_cloud();
    let entry = AmbientEntry {
        hour: 13,
        minute: 0,
        scene: "signal".to_string(),
    };
    let cfg = HashMap::new();
    let charset_preset = cloud.apply_ambient_entry(&entry, "zen", &[], false, &cfg);

    // signal scene's defaults: color=aurora, charset=retro, speed=14.0,
    // density=0.55. All should be applied.
    assert_eq!(
        cloud.color_scheme(),
        ColorScheme::Aurora,
        "signal scene should set color=aurora"
    );
    assert_eq!(
        charset_preset, "retro",
        "signal scene should set charset=retro"
    );
    assert!(
        (cloud.chars_per_sec - 14.0).abs() < 0.01,
        "signal scene should set speed=14.0, got {}",
        cloud.chars_per_sec
    );
    assert!(
        (cloud.droplet_density - 0.55).abs() < 0.01,
        "signal scene should set density=0.55, got {}",
        cloud.droplet_density
    );
}

#[test]
fn apply_ambient_entry_custom_scene_applies_base_scene_then_overrides() {
    // v30.2: ambient entry with a custom scene name looks up the
    // [scene-custom.<name>] block, applies base-scene defaults first,
    // then the block's own overrides.
    //
    // Setup: custom scene "afternoon" with base-scene=signal, color=cosmos,
    // speed=12.0. Should result in:
    //   - rain_style = signal's (Glyph)
    //   - color = cosmos (override)
    //   - charset = retro (from signal base)
    //   - speed = 12.0 (override)
    //   - density = 0.55 (from signal base)
    //   - glitch = signal's default (Default)
    let mut cloud = make_cinematic_like_cloud();
    let entry = AmbientEntry {
        hour: 15,
        minute: 0,
        scene: "afternoon".to_string(),
    };
    let mut cfg = HashMap::new();
    cfg.insert(
        "scene-custom.afternoon.base-scene".to_string(),
        "signal".to_string(),
    );
    cfg.insert(
        "scene-custom.afternoon.color".to_string(),
        "cosmos".to_string(),
    );
    cfg.insert(
        "scene-custom.afternoon.speed".to_string(),
        "12.0".to_string(),
    );
    let charset_preset = cloud.apply_ambient_entry(&entry, "zen", &[], false, &cfg);

    assert_eq!(
        cloud.color_scheme(),
        ColorScheme::Cosmos,
        "custom scene color=cosmos must override signal base's aurora"
    );
    assert_eq!(
        charset_preset, "retro",
        "custom scene with no charset field must inherit signal base's retro"
    );
    assert!(
        (cloud.chars_per_sec - 12.0).abs() < 0.01,
        "custom scene speed=12.0 must override signal base's 14.0, got {}",
        cloud.chars_per_sec
    );
    assert!(
        (cloud.droplet_density - 0.55).abs() < 0.01,
        "custom scene with no density must inherit signal base's 0.55, got {}",
        cloud.droplet_density
    );
}

#[test]
fn apply_ambient_entry_custom_scene_without_base_scene_uses_glyph_rain() {
    // v30.2: a custom scene with no base-scene falls back to Glyph rain
    // style and applies only the block's own overrides. Missing fields
    // retain the cloud's current state (no reset to defaults).
    let mut cloud = make_cinematic_like_cloud();
    let entry = AmbientEntry {
        hour: 18,
        minute: 0,
        scene: "minimal".to_string(),
    };
    let mut cfg = HashMap::new();
    cfg.insert(
        "scene-custom.minimal.color".to_string(),
        "neon-green".to_string(),
    );
    // No base-scene, no speed/density — those retain current values.
    let _ = cloud.apply_ambient_entry(&entry, "zen", &[], false, &cfg);

    assert_eq!(
        cloud.color_scheme(),
        ColorScheme::NeonGreen,
        "custom scene color=neon-green must be applied"
    );
    // Speed and density should NOT change (no base-scene, no override).
    assert!(
        (cloud.chars_per_sec - 9.0).abs() < 0.01,
        "custom scene with no base-scene and no speed must retain current 9.0, got {}",
        cloud.chars_per_sec
    );
    assert!(
        (cloud.droplet_density - 0.75).abs() < 0.01,
        "custom scene with no base-scene and no density must retain current 0.75, got {}",
        cloud.droplet_density
    );
}

#[test]
fn apply_ambient_entry_unknown_scene_is_noop() {
    // v30.2: an unknown scene name (not built-in, no [scene-custom.<name>]
    // block) is a no-op — current state is preserved. This matches the
    // apply_scene_runtime contract for unknown scenes.
    let mut cloud = make_cinematic_like_cloud();
    let entry = AmbientEntry {
        hour: 20,
        minute: 0,
        scene: "nonexistent-scene".to_string(),
    };
    let cfg = HashMap::new();
    let charset_preset = cloud.apply_ambient_entry(&entry, "zen", &[], false, &cfg);

    // Nothing should change.
    assert_eq!(cloud.color_scheme(), ColorScheme::NeonPurple);
    assert_eq!(charset_preset, "zen");
    assert!((cloud.chars_per_sec - 9.0).abs() < 0.01);
    assert!((cloud.droplet_density - 0.75).abs() < 0.01);
}

// ── apply_startup_ambient regression (v30.4 hotfix) ──
//
// Bug: `apply_startup_ambient` originally passed `&HashMap::new()` (empty
// cfg) to `apply_ambient_entry`. For custom-scene ambient targets, this
// silently broke the lookup: `apply_custom_scene_runtime` calls
// `collect_custom_scenes(cfg)` which returns an empty map → no custom block
// found → no-op. The function STILL returned `Some(entry)`, which made the
// event loop's dedup check skip the scheduler's first real fire. Net
// result: ambient never applied at startup until the user touched
// config.toml (which triggered live-reload, which DOES pass the real cfg).

use crate::ambient::{apply_startup_ambient, AmbientSchedule};

#[test]
fn apply_startup_ambient_with_empty_cfg_is_noop_for_custom_scene() {
    // Regression proof: with an EMPTY cfg map, a custom-scene ambient target
    // cannot resolve. The cloud stays in its starting state. The function
    // still returns Some(entry) — this is the trap that caused the dedup
    // check to skip the scheduler's first real fire.
    let mut cloud = make_cinematic_like_cloud();
    let schedule = AmbientSchedule {
        entries: vec![AmbientEntry {
            hour: 0,
            minute: 0,
            scene: "afternoon".to_string(),
        }],
    };
    let empty_cfg = HashMap::new();
    let (charset_preset, entry) =
        apply_startup_ambient(&mut cloud, &schedule, "zen", &[], false, &empty_cfg);

    // The bug: entry is Some (claimed applied) but the cloud is unchanged.
    assert!(entry.is_some(), "entry should be returned even for no-op");
    assert_eq!(
        cloud.color_scheme(),
        ColorScheme::NeonPurple,
        "empty cfg must NOT resolve custom scene — cloud stays cinematic"
    );
    assert_eq!(charset_preset, "zen");
}

#[test]
fn apply_startup_ambient_with_real_cfg_applies_custom_scene() {
    // Fix proof: with the REAL cfg map (containing the [scene-custom.<name>]
    // block), the custom scene resolves correctly at startup.
    let mut cloud = make_cinematic_like_cloud();
    let schedule = AmbientSchedule {
        entries: vec![AmbientEntry {
            hour: 0,
            minute: 0,
            scene: "afternoon".to_string(),
        }],
    };
    let mut cfg = HashMap::new();
    cfg.insert(
        "scene-custom.afternoon.base-scene".to_string(),
        "signal".to_string(),
    );
    cfg.insert(
        "scene-custom.afternoon.color".to_string(),
        "cosmos".to_string(),
    );
    let (charset_preset, entry) =
        apply_startup_ambient(&mut cloud, &schedule, "zen", &[], false, &cfg);

    // The fix: entry is Some AND the cloud actually changes.
    assert!(
        entry.is_some(),
        "entry must be returned for an active phase"
    );
    assert_eq!(
        cloud.color_scheme(),
        ColorScheme::Cosmos,
        "real cfg must resolve custom scene — cloud switches to cosmos"
    );
    assert_eq!(
        charset_preset, "retro",
        "base-scene=signal must inherit retro charset"
    );
}
