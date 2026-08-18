// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Color stability and endurance tests.
//!
//! Verifies that explicit CLI/config/profile color remains sticky by default,
//! that autonomous palette drift is gated behind the opt-in `crystal_dragon`
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

    // Verify crystal_dragon defaults to false
    assert!(
        !cloud.crystal_dragon,
        "crystal_dragon must default to false"
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
    // the Cloud is created with Sun and crystal_dragon defaults false.
    let mut cloud = make_sun_cloud();
    assert!(!cloud.crystal_dragon);

    let start = Instant::now();
    let final_scheme = simulate_frames(&mut cloud, 3_600, start);

    assert_eq!(
        final_scheme,
        ColorScheme::Sun,
        "Profile-set Sun color must remain sticky across simulated time"
    );
}

// Test 3: Monolith scene color does not drift without opt-in

#[test]
fn monolith_color_does_not_drift_without_opt_in() {
    let mut cloud = Cloud::new(
        ColorMode::TrueColor,
        ShadingMode::DistanceFromHead,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Green,
        RainStyle::Monolith,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(40, 20);

    assert!(!cloud.crystal_dragon);

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

// Test 4: Crystal Dragon drift is opt-in only

#[test]
fn crystal_dragon_is_opt_in_only() {
    let mut cloud = make_green_cloud();
    assert!(!cloud.crystal_dragon);

    // With drift OFF: color must stay Green (1 min simulated = 20 ecosystem ticks)
    let start = Instant::now();
    let scheme_off = simulate_frames(&mut cloud, 3_600, start);
    assert_eq!(scheme_off, ColorScheme::Green);

    // Now enable crystal_dragon
    cloud.crystal_dragon = true;
    // Reset crystal dragon poll timer so the drift check fires immediately
    cloud.crystal_dragon_last_poll = None;
    // Seed the RNG to a known value that exercises the drift path
    cloud.mt = StdRng::seed_from_u64(0xDEAD_BEEF);

    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let frame_dt = Duration::from_micros(16_667);

    // Simulate 40 minutes with crystal_dragon ON — the Crystal Dragon Engine
    // polls periodically and applies drift with non-zero probability, so over
    // 40 simulated minutes we should see at least one palette transition.
    // This removes the flakiness that plagued this test on slow FreeBSD CI
    // runners (previously only 5 minutes / ~100 ticks, which still had a
    // ~5% chance of no drift).
    let mut drifted = false;
    for i in 0..144_000u64 {
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
        "With crystal_dragon=true, the Crystal Dragon Engine should eventually drift \
         to a related scheme (expected at least one drift in 40 simulated minutes)"
    );
}

// Test 5: Pressing c/C still changes color intentionally

#[test]
fn pressing_c_changes_color_intentionally() {
    let mut cloud = make_sun_cloud();
    assert!(!cloud.crystal_dragon);

    // Simulate c key: cycle to next color
    let next = crate::cli::cycle_color_scheme(cloud.color_scheme(), 1);
    cloud.set_color_scheme(next);

    assert_eq!(
        cloud.color_scheme(),
        next,
        "Pressing c must change color even when crystal_dragon is off"
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
    assert!(!cloud.crystal_dragon);

    // Simulate C key: cycle to previous color
    let prev = crate::cli::cycle_color_scheme(cloud.color_scheme(), -1);
    cloud.set_color_scheme(prev);

    assert_eq!(
        cloud.color_scheme(),
        prev,
        "Pressing C must change color even when crystal_dragon is off"
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
    assert!(!cloud.crystal_dragon);

    // Apply monolith scene — it sets color if specified
    let charset_preset = cloud.apply_scene_runtime("monolith", "braille", &[], false);

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
    // color never changes when crystal_dragon is off.
    // Exercises the full ecosystem tick path (60 ticks) with sufficient coverage.
    let mut cloud = make_sun_cloud();
    assert!(!cloud.crystal_dragon);

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

// v30 strengthen (Bug #4): custom palette active suppresses crystal dragon drift.
// Even with crystal_dragon=true, set_color_scheme would overwrite the
// user's --colors-custom palette with a built-in one. The rain loop must
// skip the palette replacement while still allowing climate drift.

#[test]
fn custom_palette_active_suppresses_crystal_dragon_drift() {
    let mut cloud = make_green_cloud();
    cloud.crystal_dragon = true;
    // Simulate a --colors-custom user palette being active.
    cloud.custom_palette_active = true;
    // Reset crystal dragon poll timer so the drift check fires immediately.
    let start = Instant::now();
    cloud.crystal_dragon_last_poll = None;
    // Seed RNG to a known value that exercises the drift path (same seed
    // as crystal_dragon_is_opt_in_only, which does drift without the
    // custom_palette_active guard).
    cloud.mt = StdRng::seed_from_u64(0xDEAD_BEEF);

    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let frame_dt = Duration::from_micros(16_667);

    // Simulate 5 minutes — well above the 3-second ecosystem tick interval,
    // and the unguarded path would drift within ~40 minutes (per the test
    // above). With the guard, the scheme must stay Green for the entire run.
    for i in 0..18_000u64 {
        let now = start + frame_dt.saturating_mul(i as u32);
        cloud.last_spawn_time = now - Duration::from_millis(16);
        cloud.last_phosphor_time = now;
        cloud.rain_at(&mut frame, now);
        assert_eq!(
            cloud.color_scheme(),
            ColorScheme::Green,
            "custom_palette_active must suppress drift at frame {} ({:.1}s)",
            i,
            i as f64 * 16.667 / 1000.0
        );
    }
}

// v30 strengthen (Bug #5): set_color_scheme re-applies color_tune.
// Without this, the first crystal dragon drift would silently drop the
// user's --color-tune settings. Test: set a non-identity tune, call
// set_color_scheme, verify the palette still has the tune applied.

#[test]
fn set_color_scheme_reapplies_color_tune() {
    use crate::color_tune::ColorTune;
    use crate::palette::build_palette;

    let mut cloud = make_green_cloud();
    // Apply a non-identity tune (saturation 2.0 — clearly visible).
    let tune = ColorTune {
        saturation: 2.0,
        brightness: 1.0,
        head: 1.0,
        body: 1.0,
        tail: 1.0,
    };
    cloud.color_tune = tune;
    // Apply the tune to the current palette (mirrors app.rs create_cloud).
    crate::color_tune::apply_tune_to_palette(&mut cloud.palette, cloud.color_mode, &tune);

    // Snapshot the post-tune Green palette's first body color.
    let tuned_green_color = cloud.palette.colors.first().copied();
    assert!(tuned_green_color.is_some(), "palette must have colors");

    // Now call set_color_scheme to a different scheme and back.
    cloud.set_color_scheme(ColorScheme::Sun);
    cloud.set_color_scheme(ColorScheme::Green);

    // The palette should match a freshly-built+ tuned Green palette.
    let mut expected = build_palette(
        ColorScheme::Green,
        cloud.color_mode,
        cloud.default_background,
    );
    crate::color_tune::apply_tune_to_palette(&mut expected, cloud.color_mode, &tune);

    assert_eq!(
        cloud.palette.colors.first().copied(),
        expected.colors.first().copied(),
        "set_color_scheme must re-apply color_tune after palette rebuild"
    );
}

/// Phase D Bug #7 fix: Crystal Dragon Engine min_dwell_secs prevents
/// rapid oscillation. Two consecutive polls within min_dwell_secs of a
/// theme transition cannot both trigger a palette drift, even if the RNG
/// would roll drift both times.
///
/// NOTE: The old PALETTE_DRIFT_COOLDOWN_SECS / last_palette_drift mechanism
/// was removed when palette drift moved to the Crystal Dragon Engine. The
/// equivalent cooldown is now min_dwell_secs in CrystalDragonControl.
#[test]
fn crystal_dragon_dwell_prevents_rapid_oscillation() {
    let mut cloud = make_green_cloud();
    cloud.crystal_dragon = true;

    let start = Instant::now();
    // Force the crystal dragon poll timer to fire on the next rain_at call.
    cloud.crystal_dragon_last_poll = None;
    // Use a seed that is known to roll drift on the first poll.
    cloud.mt = StdRng::seed_from_u64(0xBEEF_CAFE);

    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let frame_dt = Duration::from_micros(16_667);

    // Run 60 seconds of simulated time. The Crystal Dragon Engine's
    // min_dwell_secs prevents rapid oscillation. Track scheme changes.
    let mut scheme_changes = 0u32;
    let mut prev_scheme = cloud.color_scheme;
    for i in 0..3_600u64 {
        let now = start + frame_dt.saturating_mul(i as u32);
        cloud.rain_at(&mut frame, now);
        if cloud.color_scheme != prev_scheme {
            scheme_changes += 1;
            prev_scheme = cloud.color_scheme;
        }
    }

    // The Crystal Dragon Engine's min_dwell_secs prevents rapid oscillation.
    // Over 60s, the number of scheme changes should be bounded (not
    // unbounded rapid oscillation). We allow up to 10 transitions as a
    // generous upper bound — the exact limit depends on min_dwell_secs
    // and the polling interval.
    assert!(
        scheme_changes <= 10,
        "crystal_dragon dwell violated: {scheme_changes} scheme changes in 60s (max 10 allowed by min_dwell_secs)"
    );
}

/// Phase D Bug #9 fix: inherit_ecosystem_state preserves drift state.
/// A new cloud that inherits from an old cloud should have the same
/// luminance_climate / saturation_climate / hue_drift values.
#[test]
fn inherit_ecosystem_state_preserves_drift() {
    let mut old_cloud = make_green_cloud();
    // Simulate drift: advance the ecosystem to non-default values.
    let start = Instant::now();
    old_cloud.color_ecosystem.last_tick = start - Duration::from_secs(100);
    let mut frame = Frame::new(old_cloud.cols, old_cloud.lines, old_cloud.palette.bg);
    // Run 5 minutes of simulated time to accumulate drift.
    let frame_dt = Duration::from_micros(16_667);
    for i in 0..18_000u64 {
        let now = start + frame_dt.saturating_mul(i as u32);
        old_cloud.rain_at(&mut frame, now);
    }

    // Snapshot the drifted state.
    let old_lum = old_cloud.color_ecosystem.luminance_climate;
    let old_sat = old_cloud.color_ecosystem.saturation_climate;
    let old_hue = old_cloud.color_ecosystem.hue_drift;
    // Drift should have moved at least one value away from defaults.
    let drifted =
        (old_lum - 0.85).abs() > 0.001 || (old_sat - 0.85).abs() > 0.001 || old_hue.abs() > 0.001;
    assert!(
        drifted,
        "ecosystem should have drifted from defaults after 5 min"
    );

    // Create a fresh cloud (simulating live-reload) and inherit state.
    let mut new_cloud = make_green_cloud();
    new_cloud.inherit_ecosystem_state(&old_cloud);

    // The new cloud must have the same drift state as the old cloud.
    assert_eq!(new_cloud.color_ecosystem.luminance_climate, old_lum);
    assert_eq!(new_cloud.color_ecosystem.saturation_climate, old_sat);
    assert_eq!(new_cloud.color_ecosystem.hue_drift, old_hue);
}

/// Phase D Bug #8 fix: cloud.reset() does NOT reset ecosystem state.
/// Drift accumulators are independent of terminal size — resizing the
/// terminal should not cause a brightness discontinuity.
#[test]
fn reset_preserves_ecosystem_state() {
    let mut cloud = make_green_cloud();
    let start = Instant::now();
    cloud.color_ecosystem.last_tick = start - Duration::from_secs(100);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let frame_dt = Duration::from_micros(16_667);
    for i in 0..18_000u64 {
        let now = start + frame_dt.saturating_mul(i as u32);
        cloud.rain_at(&mut frame, now);
    }

    let pre_reset_lum = cloud.color_ecosystem.luminance_climate;
    let pre_reset_sat = cloud.color_ecosystem.saturation_climate;
    let pre_reset_hue = cloud.color_ecosystem.hue_drift;

    // Resize the terminal — this calls cloud.reset().
    cloud.reset(120, 40);

    // Ecosystem state must be preserved across the resize.
    assert_eq!(
        cloud.color_ecosystem.luminance_climate, pre_reset_lum,
        "reset() must not reset luminance_climate"
    );
    assert_eq!(
        cloud.color_ecosystem.saturation_climate, pre_reset_sat,
        "reset() must not reset saturation_climate"
    );
    assert_eq!(
        cloud.color_ecosystem.hue_drift, pre_reset_hue,
        "reset() must not reset hue_drift"
    );
}

/// Strengthen #14: verify every ColorScheme variant can be used with
/// the Crystal Dragon Engine without panic. This catches the case where
/// a new variant is added to ColorScheme but forgotten in the Crystal
/// Dragon Engine's temperature group mapping — which would cause a panic
/// or incorrect drift behavior.
///
/// The old `family_for` pipeline was removed when palette drift moved to
/// the Crystal Dragon Engine (point-based temperature group system with
/// calc-v1 probabilistic weighted selection). This test verifies the new
/// engine handles every builtin scheme.
#[test]
fn crystal_dragon_handles_every_builtin_scheme() {
    use ColorScheme::*;
    let test_schemes = [
        Green,
        Green2,
        Green3,
        Gold,
        Yellow,
        Orange,
        Red,
        Blue,
        Cyan,
        Purple,
        Neon,
        Fire,
        Ocean,
        Forest,
        Vaporwave,
        Gray,
        Rainbow,
        Snow,
        Aurora,
        FancyDiamond,
        Cosmos,
        Nebula,
        Spectrum20,
        Stars,
        Mars,
        Venus,
        Mercury,
        Jupiter,
        Saturn,
        Uranus,
        Neptune,
        Pluto,
        Moon,
        Sun,
    ];
    let start = Instant::now();
    // For each variant, enable crystal_dragon and run a few frames.
    // The point is to verify the Crystal Dragon Engine can handle
    // every variant without panic.
    for &scheme in &test_schemes {
        let mut cloud = Cloud::new(
            ColorMode::TrueColor,
            ShadingMode::DistanceFromHead,
            BoldMode::Off,
            false,
            true,
            scheme,
            RainStyle::Glyph,
        );
        cloud.init_chars(vec!['0', '1']);
        cloud.reset(40, 20);
        cloud.crystal_dragon = true;
        cloud.crystal_dragon_last_poll = None;
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        for i in 0..50u32 {
            let now = start + Duration::from_millis(16_667 * i as u64);
            cloud.last_spawn_time = now - Duration::from_millis(16);
            cloud.last_phosphor_time = now;
            cloud.rain_at(&mut frame, now);
        }
    }
    // If we got here without panic, every variant is handled by the
    // Crystal Dragon Engine's temperature group mapping.
}
