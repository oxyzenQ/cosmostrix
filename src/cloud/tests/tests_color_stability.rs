// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Color stability and endurance tests.
//!
//! Verifies that explicit CLI/config/profile color remains sticky by default,
//! that autonomous palette drift is gated behind the opt-in `auto_color_drift`
//! flag, and that intentional color changes (user keys, scene cycling) still
//! work as expected. All tests simulate many minutes of wall-clock time without
//! actual sleeping, using deterministic `Instant::now() + offset` values.

use std::time::{Duration, Instant};

use rand::rngs::StdRng;
use rand::SeedableRng;

use super::Cloud;
use crate::frame::Frame;
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};

/// Helper: create a standard test cloud with Sun color scheme.
fn make_sun_cloud() -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::TrueColor,
        false,
        ShadingMode::DistanceFromHead,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Sun,
        RainStyle::Glyph,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(40, 20);
    cloud
}

/// Helper: create a cloud with the default Green scheme.
fn make_green_cloud() -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::TrueColor,
        false,
        ShadingMode::DistanceFromHead,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Green,
        RainStyle::Glyph,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(40, 20);
    cloud
}

/// Simulate N frames at 60fps without wall-clock sleeping.
/// Returns the final color scheme.
fn simulate_frames(cloud: &mut Cloud, num_frames: u64, start: Instant) -> ColorScheme {
    let frame_dt = Duration::from_micros(16_667); // ~60fps
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    for i in 0..num_frames {
        let now = start + frame_dt.saturating_mul(i as u32);
        cloud.last_spawn_time = now - Duration::from_millis(16);
        cloud.last_phosphor_time = now;
        cloud.rain_at(&mut frame, now);
    }

    cloud.color_scheme()
}

// Test 1: Fixed CLI color (sun) stays sun across simulated minutes

#[test]
fn fixed_color_sun_stays_sun_across_simulated_minutes() {
    let mut cloud = make_sun_cloud();

    // Verify auto_color_drift defaults to false
    assert!(
        !cloud.auto_color_drift,
        "auto_color_drift must default to false"
    );

    // Simulate 1 minute at 60fps = 3,600 frames (ample for ecosystem ticks every 3s = 20 ticks)
    let start = Instant::now();
    let final_scheme = simulate_frames(&mut cloud, 3_600, start);

    assert_eq!(
        final_scheme,
        ColorScheme::Sun,
        "Sun color must remain sticky across simulated time without drift"
    );
}

// Test 2: Profile color (sun) stays sun across simulated minutes

#[test]
fn profile_color_sun_stays_sun_across_simulated_minutes() {
    // Simulates what happens when a profile sets color=sun:
    // the Cloud is created with Sun and auto_color_drift defaults false.
    let mut cloud = make_sun_cloud();
    assert!(!cloud.auto_color_drift);

    let start = Instant::now();
    let final_scheme = simulate_frames(&mut cloud, 3_600, start);

    assert_eq!(
        final_scheme,
        ColorScheme::Sun,
        "Profile-set Sun color must remain sticky across simulated time"
    );
}

// Test 3: Default monolith color does not drift without opt-in

#[test]
fn default_monolith_color_does_not_drift_without_opt_in() {
    let mut cloud = Cloud::new(
        ColorMode::TrueColor,
        false,
        ShadingMode::DistanceFromHead,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Green,
        RainStyle::Monolith,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(40, 20);

    assert!(!cloud.auto_color_drift);

    // Simulate 30 seconds at 60fps = 1,800 frames (10 ecosystem ticks)
    let start = Instant::now();
    let frame_dt = Duration::from_micros(16_667);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    for i in 0..1_800u64 {
        let now = start + frame_dt.saturating_mul(i as u32);
        cloud.last_spawn_time = now - Duration::from_millis(16);
        cloud.last_phosphor_time = now;
        cloud.rain_at(&mut frame, now);
    }

    assert_eq!(
        cloud.color_scheme(),
        ColorScheme::Green,
        "Default Green must not drift to a related scheme without opt-in"
    );
}

// Test 4: Auto color drift is opt-in only

#[test]
fn auto_color_drift_is_opt_in_only() {
    let mut cloud = make_green_cloud();
    assert!(!cloud.auto_color_drift);

    // With drift OFF: color must stay Green (1 min simulated = 20 ecosystem ticks)
    let start = Instant::now();
    let scheme_off = simulate_frames(&mut cloud, 3_600, start);
    assert_eq!(scheme_off, ColorScheme::Green);

    // Now enable drift
    cloud.auto_color_drift = true;
    // Reset ecosystem tick timer so the drift check fires immediately
    cloud.color_ecosystem.last_tick = start - Duration::from_secs(10);
    // Seed the RNG to a known value that exercises the drift path
    cloud.mt = StdRng::seed_from_u64(0xDEAD_BEEF);

    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let frame_dt = Duration::from_micros(16_667);

    // Simulate 5 minutes with drift ON — the ecosystem ticks every 3 seconds
    // with AUTONOMOUS_PALETTE_DRIFT_CHANCE = 0.03, so over 5 minutes we
    // get ~100 drift attempts. Statistically reliable with this seed (~95% success).
    let mut drifted = false;
    for i in 0..18_000u64 {
        let now = start + frame_dt.saturating_mul(i as u32);
        cloud.last_spawn_time = now - Duration::from_millis(16);
        cloud.last_phosphor_time = now;
        cloud.rain_at(&mut frame, now);
        if cloud.color_scheme() != ColorScheme::Green {
            drifted = true;
            break;
        }
    }

    assert!(
        drifted,
        "With auto_color_drift=true, the ecosystem should eventually drift \
         to a related scheme (expected at least one drift in 20 simulated minutes)"
    );
}

// Test 5: Pressing c/C still changes color intentionally

#[test]
fn pressing_c_changes_color_intentionally() {
    let mut cloud = make_sun_cloud();
    assert!(!cloud.auto_color_drift);

    // Simulate c key: cycle to next color
    let next = crate::cli::cycle_color_scheme(cloud.color_scheme(), 1);
    cloud.set_color_scheme(next);

    assert_eq!(
        cloud.color_scheme(),
        next,
        "Pressing c must change color even when auto_color_drift is off"
    );
    assert_ne!(
        cloud.color_scheme(),
        ColorScheme::Sun,
        "Color must have changed from Sun after pressing c"
    );

    // Simulate 12 seconds — color should stick to the new scheme
    let start = Instant::now();
    let final_scheme = simulate_frames(&mut cloud, 720, start);
    assert_eq!(
        final_scheme, next,
        "User-changed color must remain sticky across simulated minutes"
    );
}

#[test]
fn pressing_shift_c_changes_color_intentionally() {
    let mut cloud = make_sun_cloud();
    assert!(!cloud.auto_color_drift);

    // Simulate C key: cycle to previous color
    let prev = crate::cli::cycle_color_scheme(cloud.color_scheme(), -1);
    cloud.set_color_scheme(prev);

    assert_eq!(
        cloud.color_scheme(),
        prev,
        "Pressing C must change color even when auto_color_drift is off"
    );
    assert_ne!(
        cloud.color_scheme(),
        ColorScheme::Sun,
        "Color must have changed from Sun after pressing C"
    );
}

// Test 6: Scene cycling still applies scene-managed color

#[test]
fn scene_cycle_applies_scene_color_intentionally() {
    let mut cloud = make_sun_cloud();
    assert!(!cloud.auto_color_drift);

    // Apply monolith scene — it sets color if specified
    let charset_preset = cloud.apply_scene_runtime("monolith", "binary", &[], false);

    // The monolith scene may or may not set color — check the scene definition
    // The important thing is: no crash, and the drift gate doesn't interfere.
    // If the scene set a color, it should stick.
    let scheme_after_scene = cloud.color_scheme();

    // Simulate 12 seconds — color should not drift
    let start = Instant::now();
    let final_scheme = simulate_frames(&mut cloud, 720, start);

    assert_eq!(
        final_scheme, scheme_after_scene,
        "Scene-applied color must remain sticky across simulated minutes"
    );
    // charset_preset is returned but we just need to use it to avoid warning
    drop(charset_preset);
}

// Test 7: Benchmark output fields completeness

#[test]
fn benchmark_output_fields_complete() {
    // Verify that the benchmark output includes all required metric fields.
    // This is a documentation/smoke test that ensures we don't silently
    // drop fields from the benchmark report.
    let required_fields: &[&str] = &[
        "avg_fps",
        "median_fps",
        "p95_frame_time",
        "p99_frame_time",
        "frame_time_stability",
        "dirty_cell_ratio",
        "estimated_full_redraw_ratio",
    ];

    // Read the benchmark module source and verify field names exist
    let source = include_str!("../../bench.rs");

    for field in required_fields {
        assert!(
            source.contains(field),
            "Benchmark source must reference required field '{}'",
            field
        );
    }
}

// Test 8: Endurance color stability — default-off gate is effective

#[test]
fn endurance_color_sticky_default_off() {
    // Endurance test: run 3 simulated minutes (10,800 frames) and verify
    // color never changes when auto_color_drift is off.
    // Exercises the full ecosystem tick path (60 ticks) with sufficient coverage.
    let mut cloud = make_sun_cloud();
    assert!(!cloud.auto_color_drift);

    let start = Instant::now();
    let frame_dt = Duration::from_micros(16_667);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    for i in 0..10_800u64 {
        let now = start + frame_dt.saturating_mul(i as u32);
        cloud.last_spawn_time = now - Duration::from_millis(16);
        cloud.last_phosphor_time = now;
        cloud.rain_at(&mut frame, now);

        // Spot-check color every 500 frames
        if i % 500 == 0 {
            assert_eq!(
                cloud.color_scheme(),
                ColorScheme::Sun,
                "Color must remain Sun at simulated frame {} ({:.1}s)",
                i,
                i as f64 * 16.667 / 1000.0
            );
        }
    }

    assert_eq!(
        cloud.color_scheme(),
        ColorScheme::Sun,
        "Sun color must remain sticky across simulated time (endurance)"
    );
}
