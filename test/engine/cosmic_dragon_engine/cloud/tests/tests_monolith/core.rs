// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

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

// -- NIGHT-research-17: the soft-light round (standing-Core sweep) --

fn soft_rank(level: crate::cloud::type_rain::monolith::BrightnessLevel) -> u8 {
    use crate::cloud::type_rain::monolith::BrightnessLevel::*;
    match level {
        Ghost => 0,
        Dim => 1,
        Mid => 2,
        Hot => 3,
        Core => 4,
    }
}

#[test]
fn monolith_segment_ladder_never_lands_core_standing() {
    // The NIGHT-research-17 soft-light ruling (the black hole's
    // NIGHT-research-11 precedent, dragon entry-reveal idiom): once
    // a stream's arrival-reveal window has expired, the whole draw
    // composes to the warm ceiling or below — no kind at any
    // position in the ladder may land Core. The masterclass audit's
    // monolith finding: the standing Hero head read Core (the 55%
    // white blend) for every frame of the cascade's fall.
    use crate::cloud::type_rain::monolith::monolith::SegmentKind;
    use crate::cloud::type_rain::monolith::monolith_helpers::segment_level;
    use crate::cloud::type_rain::monolith::BrightnessLevel;

    for kind in [
        SegmentKind::Micro,
        SegmentKind::Short,
        SegmentKind::Medium,
        SegmentKind::Hero,
    ] {
        for pos in 0u8..=10 {
            let level = segment_level(kind, pos, false);
            assert!(
                soft_rank(level) <= soft_rank(BrightnessLevel::Hot),
                "a standing segment cell must never land Core ({kind:?} at pos {pos})"
            );
        }
    }
    // The settled Hero head composes at the Hot warm ceiling (the
    // palette's bright stop, no white blend), with its Hot/Hot/Mid
    // body fade untouched.
    assert_eq!(
        soft_rank(segment_level(SegmentKind::Hero, 0, false)),
        soft_rank(BrightnessLevel::Hot),
        "the settled Hero head must read the soft warm ceiling"
    );
    assert_eq!(
        soft_rank(segment_level(SegmentKind::Hero, 1, false)),
        soft_rank(BrightnessLevel::Hot)
    );
    assert_eq!(
        soft_rank(segment_level(SegmentKind::Hero, 3, false)),
        soft_rank(BrightnessLevel::Mid)
    );
}

#[test]
fn monolith_hero_reveal_flashes_core_only_in_the_head_rung() {
    // The transient contract: the arrival-reveal window is the Hero
    // head's ONE Core moment — the cascade's first light while the
    // fresh stream enters (the dragon entry-reveal precedent, 1.5 s
    // window). The flag must touch only the Hero head rung: every
    // other cell of every kind reads the same level with the
    // window open or closed.
    use crate::cloud::type_rain::monolith::monolith::SegmentKind;
    use crate::cloud::type_rain::monolith::monolith_helpers::segment_level;
    use crate::cloud::type_rain::monolith::BrightnessLevel;

    assert_eq!(
        soft_rank(segment_level(SegmentKind::Hero, 0, true)),
        soft_rank(BrightnessLevel::Core),
        "the arrival reveal is the Hero head's one Core flash"
    );
    assert_eq!(
        soft_rank(segment_level(SegmentKind::Hero, 0, false)),
        soft_rank(BrightnessLevel::Hot),
        "the settled Hero head reads the soft warm ceiling"
    );
    for kind in [SegmentKind::Micro, SegmentKind::Short, SegmentKind::Medium] {
        for pos in 0u8..=10 {
            assert_eq!(
                soft_rank(segment_level(kind, pos, true)),
                soft_rank(segment_level(kind, pos, false)),
                "the reveal flag must not touch any non-Hero rung ({kind:?} at pos {pos})"
            );
        }
    }
    // The Hero body behind the flaring head also stays untouched.
    for pos in 1u8..=10 {
        assert_eq!(
            soft_rank(segment_level(SegmentKind::Hero, pos, true)),
            soft_rank(segment_level(SegmentKind::Hero, pos, false)),
            "the reveal flag must not touch the Hero body rungs (pos {pos})"
        );
    }
}

#[test]
fn monolith_fresh_streams_carry_the_arrival_reveal_window() {
    // activate_stream stamps MONOLITH_HERO_REVEAL_SECS onto every
    // fresh cascade: right after the first spawn burst, every
    // active stream is inside its reveal window (the countdown
    // strictly positive), so the scene's Core flash rides only the
    // streams that are actually arriving.
    let mut cloud = make_monolith_cloud(96, 36);
    let mut frame = Frame::new(96, 36, cloud.palette.bg);
    let start = Instant::now();

    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    cloud.rain_at(&mut frame, start);

    let reveals = cloud.monolith_rain.hero_reveals_for_test();
    assert!(
        !reveals.is_empty(),
        "the first spawn burst must activate streams"
    );
    assert!(
        reveals.iter().all(|&r| r > 0.0),
        "every fresh stream must carry the reveal countdown (got {reveals:?})"
    );
}

#[test]
fn monolith_reveal_window_expires_under_sim_time() {
    // The window is a countdown, not a state: after MONOLITH_HERO_
    // REVEAL_SECS of simulated time (200 frames at 16 ms = 3.2 s,
    // more than double the window) no active stream may still hold
    // a reveal countdown, so the standing frame composes entirely
    // to the soft warm ceiling.
    let mut cloud = make_monolith_cloud(96, 36);
    let mut frame = Frame::new(96, 36, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 200, 16);

    let heads = cloud.monolith_rain.active_heads_for_test();
    let reveals = cloud.monolith_rain.hero_reveals_for_test();
    assert!(!heads.is_empty(), "streams must still be active");
    assert!(
        reveals.iter().all(|&r| r == 0.0),
        "the reveal window must expire for every active stream (got {reveals:?})"
    );
}
