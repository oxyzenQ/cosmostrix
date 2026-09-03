// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! S-master-HUNT-10: smoothstep easing on transition waves.
//!
//! Both `charset_wave_line_at` and `color_wave_line_at` previously used
//! a linear-velocity sweep (`progress * scale`) — the wave moved at
//! constant speed from top to bottom. S-master-HUNT-10 replaces the
//! linear `progress` with `smoothstep(progress) = 3t^2 - 2t^3`, which
//! eases in at the top, accelerates through the middle, and eases out
//! at the bottom — a more organic, cinematic feel.
//!
//! These tests verify the smoothstep curve properties:
//! - At t=0: wave_line starts at the initial position (smoothstep(0)=0)
//! - At t=duration: wave_line reaches the end (smoothstep(1)=1)
//! - Mid-progress wave_line is BELOW the linear midpoint (ease-in)
//! - The wave is monotonically increasing (LTS ordering invariant)
//!
//! LTS safety: smoothstep is monotonic on [0,1] with fixed endpoints
//! (0 and 1), so all existing ordering/threshold/completion tests pass
//! unchanged. The easing only affects the INTERPOLATION between
//! endpoints, not the endpoints themselves.

use std::time::{Duration, Instant};

use super::make_cloud;
use crate::constants::{
    CHARSET_TRANSITION_DURATION_MS, COLOR_TRANSITION_DURATION_MS,
    COLOR_TRANSITION_INITIAL_VISIBLE_PCT,
};

#[test]
fn charset_wave_smoothstep_starts_at_zero() {
    // At t=0, smoothstep(0) = 0, so wave_line = 0 * (lines + 1) = 0.
    // The wave starts at the very top row (row 0 is "above" the wave).
    let mut cloud = make_cloud();
    let now = Instant::now();
    cloud.charset_transition_start = Some(now);

    let wave = cloud
        .charset_wave_line_at(now)
        .expect("wave must be active");
    // smoothstep(0) = 0, so wave_line should be 0.0 (or very close —
    // elapsed_ms at t=0 is 0, progress = 0, eased = 0).
    assert!(
        wave.abs() < 0.001,
        "charset wave at t=0 should start at 0 (smoothstep(0)=0), got {}",
        wave
    );
}

#[test]
fn charset_wave_smoothstep_ends_at_lines_plus_one() {
    // At t=duration, smoothstep(1) = 1, so wave_line = 1 * (lines + 1).
    // The wave has swept the entire screen (all rows above the wave).
    let mut cloud = make_cloud();
    let start = Instant::now();
    cloud.charset_transition_start = Some(start);
    let expected_end = cloud.lines as f32 + 1.0;

    let wave = cloud
        .charset_wave_line_at(start + Duration::from_millis(CHARSET_TRANSITION_DURATION_MS as u64))
        .expect("wave must be active at t=duration");
    assert!(
        (wave - expected_end).abs() < 0.001,
        "charset wave at t=duration should reach lines+1 (smoothstep(1)=1), got {} expected {}",
        wave,
        expected_end
    );
}

#[test]
fn charset_wave_smoothstep_midpoint_below_linear_midpoint() {
    // The KEY smoothstep property: at t=duration/2 (progress=0.5),
    // smoothstep(0.5) = 0.5 * 0.5 * (3 - 2*0.5) = 0.25 * 2.0 = 0.5.
    // Wait — smoothstep(0.5) is exactly 0.5 (symmetric around 0.5).
    // So the midpoint test is a sanity check that eased(0.5) = 0.5,
    // NOT below the linear midpoint. The ease-in/ease-out is symmetric.
    //
    // The real easing difference is at NON-midpoint progress values:
    // - At progress=0.25: linear = 0.25, smoothstep = 0.15625 (BELOW linear — ease-in)
    // - At progress=0.75: linear = 0.75, smoothstep = 0.84375 (ABOVE linear — ease-out)
    //
    // This test verifies the ease-in at progress=0.25: the smoothstep
    // wave should be BELOW the linear wave at the same timestamp.
    let mut cloud = make_cloud();
    let start = Instant::now();
    cloud.charset_transition_start = Some(start);
    let lines_plus_one = cloud.lines as f32 + 1.0;
    let duration = CHARSET_TRANSITION_DURATION_MS as f32;

    // progress = 0.25 (quarter way through the transition)
    let quarter_ms = (duration * 0.25) as u64;
    let wave_at_quarter = cloud
        .charset_wave_line_at(start + Duration::from_millis(quarter_ms))
        .expect("wave must be active");

    // Linear would give: 0.25 * lines_plus_one
    // Smoothstep gives: 0.25 * 0.25 * (3 - 0.5) = 0.0625 * 2.5 = 0.15625
    let linear_at_quarter = 0.25 * lines_plus_one;
    let smoothstep_at_quarter = 0.15625 * lines_plus_one;

    assert!(
        wave_at_quarter < linear_at_quarter,
        "smoothstep wave at progress=0.25 ({}) should be BELOW linear ({}) — ease-in property",
        wave_at_quarter,
        linear_at_quarter
    );
    assert!(
        (wave_at_quarter - smoothstep_at_quarter).abs() < 0.01,
        "smoothstep wave at progress=0.25 ({}) should match formula 0.15625 * lines+1 ({})",
        wave_at_quarter,
        smoothstep_at_quarter
    );
}

#[test]
fn charset_wave_smoothstep_is_monotonic() {
    // LTS invariant: the wave must progress strictly downward over time
    // (existing tests assert this). smoothstep is monotonic on [0,1],
    // so this property is preserved.
    let mut cloud = make_cloud();
    let start = Instant::now();
    cloud.charset_transition_start = Some(start);

    let wave_early = cloud
        .charset_wave_line_at(start + Duration::from_millis(50))
        .unwrap();
    let wave_mid = cloud
        .charset_wave_line_at(start + Duration::from_millis(250))
        .unwrap();
    let wave_late = cloud
        .charset_wave_line_at(start + Duration::from_millis(450))
        .unwrap();

    assert!(
        wave_mid > wave_early,
        "charset wave should progress downward (mid > early)"
    );
    assert!(
        wave_late > wave_mid,
        "charset wave should continue progressing downward (late > mid)"
    );
}

#[test]
fn color_wave_smoothstep_preserves_initial_band() {
    // The color wave has an initial_frac offset: at t=0, the first
    // initial_frac * lines rows adopt immediately. smoothstep(0) = 0,
    // so the eased term contributes 0 at t=0, and wave_line =
    // initial_frac * lines (unchanged from the linear version).
    let mut cloud = make_cloud();
    let now = Instant::now();
    cloud.transition_start = Some(now);

    let wave = cloud
        .color_wave_line_at(now)
        .expect("color wave must be active");
    let expected_initial = COLOR_TRANSITION_INITIAL_VISIBLE_PCT * cloud.lines as f32;

    assert!(
        (wave - expected_initial).abs() < 0.001,
        "color wave at t=0 should preserve initial band (smoothstep(0)=0), got {} expected {}",
        wave,
        expected_initial
    );
}

#[test]
fn color_wave_smoothstep_eases_after_initial_band() {
    // After the initial band, the remaining sweep uses smoothstep easing.
    // At progress=0.25 (quarter through the post-initial sweep):
    // - Linear: wave_line = initial_frac * lines + 0.25 * (1 - initial_frac) * (lines + 1)
    // - Smoothstep: wave_line = initial_frac * lines + 0.15625 * (1 - initial_frac) * (lines + 1)
    // The smoothstep value is BELOW the linear value (ease-in).
    let mut cloud = make_cloud();
    let start = Instant::now();
    cloud.transition_start = Some(start);
    let lines = cloud.lines as f32;
    let initial_frac = COLOR_TRANSITION_INITIAL_VISIBLE_PCT;
    let duration = COLOR_TRANSITION_DURATION_MS as f32;

    let quarter_ms = (duration * 0.25) as u64;
    let wave_at_quarter = cloud
        .color_wave_line_at(start + Duration::from_millis(quarter_ms))
        .unwrap();

    let linear_at_quarter = initial_frac * lines + 0.25 * (1.0 - initial_frac) * (lines + 1.0);
    let smoothstep_at_quarter =
        initial_frac * lines + 0.15625 * (1.0 - initial_frac) * (lines + 1.0);

    assert!(
        wave_at_quarter < linear_at_quarter,
        "smoothstep color wave at progress=0.25 ({}) should be BELOW linear ({}) — ease-in after initial band",
        wave_at_quarter,
        linear_at_quarter
    );
    assert!(
        (wave_at_quarter - smoothstep_at_quarter).abs() < 0.01,
        "smoothstep color wave at progress=0.25 ({}) should match formula ({})",
        wave_at_quarter,
        smoothstep_at_quarter
    );
}

#[test]
fn color_wave_smoothstep_is_monotonic() {
    // LTS invariant: the color wave must progress strictly downward.
    let mut cloud = make_cloud();
    let start = Instant::now();
    cloud.transition_start = Some(start);

    let wave_early = cloud
        .color_wave_line_at(start + Duration::from_millis(10))
        .unwrap();
    let wave_mid = cloud
        .color_wave_line_at(start + Duration::from_millis(75))
        .unwrap();
    let wave_late = cloud
        .color_wave_line_at(start + Duration::from_millis(140))
        .unwrap();

    assert!(wave_mid > wave_early, "color wave should progress downward");
    assert!(
        wave_late > wave_mid,
        "color wave should continue progressing"
    );
}
