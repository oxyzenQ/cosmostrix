// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Monolith core tests: initialization, sparse rain basics, state sanity,
//! size/density basics, deterministic phase behavior.

use std::time::{Duration, Instant};

use super::{
    average_head_delta, make_monolith_cloud, run_frames, segment_draw_count, visible_chars, Frame,
    MonolithSize,
};
use crate::rain_style::RainStyle;

#[test]
fn monolith_rain_state_initializes_without_allocation_panic() {
    let cloud = make_monolith_cloud(120, 40);

    assert_eq!(cloud.rain_style(), RainStyle::Monolith);
    assert_eq!(cloud.droplet_count(), 0);
    assert_eq!(cloud.active_droplet_count(), 0);
}

#[test]
fn monolith_rain_produces_dirty_frames() {
    let mut cloud = make_monolith_cloud(80, 24);
    let mut frame = Frame::new(80, 24, cloud.palette.bg);
    frame.clear_dirty();

    cloud.last_spawn_time = Instant::now() - Duration::from_secs(1);
    cloud.rain(&mut frame);

    assert!(frame.is_dirty_all() || !frame.dirty_indices().is_empty());
}

#[test]
fn active_monolith_streams_update_speed_without_respawn() {
    let mut cloud = make_monolith_cloud(96, 36);
    let mut frame = Frame::new(96, 36, cloud.palette.bg);
    let start = Instant::now();

    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    cloud.rain_at(&mut frame, start);
    let initial = cloud.monolith_rain.active_heads_for_test();
    assert!(initial.len() > 4);

    cloud.set_chars_per_sec(1.0);
    frame.clear_dirty();
    cloud.rain_at(&mut frame, start + Duration::from_millis(100));
    let slow = cloud.monolith_rain.active_heads_for_test();
    assert_eq!(slow.len(), initial.len());

    cloud.set_chars_per_sec(100.0);
    frame.clear_dirty();
    cloud.rain_at(&mut frame, start + Duration::from_millis(200));
    let fast = cloud.monolith_rain.active_heads_for_test();
    assert_eq!(fast.len(), slow.len());

    let slow_delta = average_head_delta(&initial, &slow);
    let fast_delta = average_head_delta(&slow, &fast);
    assert!(
        fast_delta > slow_delta * 40.0,
        "active streams should use the new global speed immediately (slow={slow_delta}, fast={fast_delta})"
    );
}

#[test]
fn monolith_subtle_phase_behavior_is_deterministic_under_seeded_rng() {
    let mut first = make_monolith_cloud(96, 36);
    let mut second = make_monolith_cloud(96, 36);
    let mut first_frame = Frame::new(96, 36, first.palette.bg);
    let mut second_frame = Frame::new(96, 36, second.palette.bg);
    let start = Instant::now();

    first.last_spawn_time = start - Duration::from_secs(1);
    second.last_spawn_time = first.last_spawn_time;
    first.last_phosphor_time = start;
    second.last_phosphor_time = start;

    for idx in 0..32 {
        let now = start + Duration::from_millis(idx * 16);
        first.rain_at(&mut first_frame, now);
        second.rain_at(&mut second_frame, now);
        first_frame.clear_dirty();
        second_frame.clear_dirty();
    }

    assert_eq!(
        first.monolith_rain.active_heads_for_test(),
        second.monolith_rain.active_heads_for_test(),
        "seeded monolith phase motion should be deterministic"
    );
    assert_eq!(visible_chars(&first_frame), visible_chars(&second_frame));
}

#[test]
fn monolith_size_changes_segment_coverage() {
    let mut small = make_monolith_cloud(80, 24);
    small.set_monolith_size(MonolithSize::Small);
    small.reset(80, 24);
    small.clear_redraw_flags_for_test();
    let mut small_frame = Frame::new(80, 24, small.palette.bg);
    run_frames(&mut small, &mut small_frame, 20, 16);

    let mut large = make_monolith_cloud(80, 24);
    large.set_monolith_size(MonolithSize::Large);
    large.reset(80, 24);
    large.clear_redraw_flags_for_test();
    let mut large_frame = Frame::new(80, 24, large.palette.bg);
    run_frames(&mut large, &mut large_frame, 20, 16);

    let small_segments = segment_draw_count(&small);
    let large_segments = segment_draw_count(&large);
    assert!(
        large_segments > small_segments,
        "large monolith size should draw more segment cells than small (large={large_segments}, small={small_segments})"
    );
}
