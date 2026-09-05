// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-hunter-8 pause/resume smoothness tests: the wall-clock
//! subsystems outside the blend easing — the wind-gust state machine
//! and the cinematic event clocks — must shift by the pause duration
//! on resume, continuing exactly where they froze. Before the fix,
//! the gust jumped a full phase on the first post-resume tick (a
//! spawn-scale surge) and ghosts aged by the pause duration and
//! popped out instead of fading.

use std::time::{Duration, Instant};

use super::super::ghost_events::{CinematicEvent, EventCtx, GhostEventScheduler};
use super::super::living_rain::GustState;

fn make_ctx(now: Instant) -> EventCtx {
    EventCtx {
        cols: 80,
        lines: 24,
        ghost_base_color: (200, 200, 200),
        color_pipeline: crate::engine::cosmic_dragon_engine::runtime::ColorPipeline::LegacyRgb,
        now,
    }
}

/// A gust mid-Attack must resume mid-Attack: the first post-resume
/// tick advances the multiplier by ONE frame's worth of interpolation,
/// not a whole phase transition to the Hold peak.
#[test]
fn gust_shift_in_time_resumes_mid_phase_without_jump() {
    let now = Instant::now();
    let mut gust = GustState::new(now);

    // Drive out of Idle by ticking past the idle window.
    let idle_max = crate::constants::GUST_IDLE_MAX_SECS + 1.0;
    let m = gust.tick(now + Duration::from_secs_f64(idle_max), &mut rand::rng());

    // Freeze mid-attack: capture the multiplier, "pause" 30s (no ticks),
    // then shift and tick once as the resume would. The post-shift tick
    // time is (last tick time + pause + one frame).
    let last_tick = now + Duration::from_secs_f64(idle_max);
    let frozen = gust.tick(last_tick, &mut rand::rng());
    assert!(
        frozen < 1.0 || frozen <= m,
        "setup: still interpolating (idle -> attack window)"
    );

    let pause_len = Duration::from_secs(30);
    gust.shift_in_time(pause_len);
    let after = gust.tick(
        last_tick + pause_len + Duration::from_millis(16),
        &mut rand::rng(),
    );

    // One frame of interpolation must be imperceptible: without the
    // shift, the tick landed 30s past the phase boundary and the state
    // machine fired a transition (multiplier pinned to the next phase's
    // value — a jump of up to GUST_PEAK_MAX - 1.0 = 0.8 in one frame).
    let jump = (after - frozen).abs();
    assert!(
        jump < 0.35,
        "one post-resume frame must advance the gust by one frame of interpolation, not a phase jump (frozen={frozen}, after={after}, jump={jump})"
    );

    // The unshifted control (the pre-fix behavior): the same tick time
    // against an UNSHIFTED gust lands 30s past the phase boundary and
    // fires a transition.
    let mut control = GustState::new(now);
    let _ = control.tick(now + Duration::from_secs_f64(idle_max), &mut rand::rng());
    let control_frozen = control.tick(last_tick, &mut rand::rng());
    let control_after = control.tick(last_tick + pause_len, &mut rand::rng());
    // The control's multiplier jumped a whole phase (or more) — this is
    // the defect the shift fixes. Assert it moved MORE than the shifted
    // path so the test documents the difference (both stay in [1, peak],
    // so compare the absolute jump magnitudes).
    let control_jump = (control_after - control_frozen).abs();
    assert!(
        control_jump > jump,
        "the unshifted gust must show the phase-jump defect (control jump {control_jump} should exceed the shifted jump {jump})"
    );
}

/// A ghost mid-life must survive a pause: shift_in_time moves its clock
/// forward so the post-resume age excludes the pause duration.
#[test]
fn ghost_events_shift_in_time_excludes_pause_from_age() {
    use crate::engine::cosmic_dragon_engine::cloud::events::ghost::GhostEvent;

    let now = Instant::now();

    // Pause for 60s WITHOUT shifting: a mid-life ghost expires instantly
    // (the pre-fix pop-out).
    let ghost = GhostEvent::new(10, 5, now);
    let ctx_unshifted = make_ctx(now + Duration::from_secs(60));
    assert!(
        ghost.is_finished(&ctx_unshifted),
        "unshifted ghost ages by the pause (the pre-fix pop-out behavior)"
    );

    // With the shift, a fresh ghost at the same instant is still mid-life.
    let mut shifted = GhostEvent::new(10, 5, now);
    shifted.shift_in_time(Duration::from_secs(60));
    let ctx_shifted = make_ctx(now + Duration::from_secs(60));
    assert!(
        !shifted.is_finished(&ctx_shifted),
        "shifted ghost must continue mid-life at the resume instant"
    );

    // The scheduler's shift_in_time must reach its events without
    // adding or removing any: spawn through the public evaluate path,
    // then shift.
    let mut scheduler = GhostEventScheduler::new(now);
    scheduler.enable_events();
    for _ in 0..2000 {
        scheduler.evaluate_triggers(0.0, 80, 24, false, false);
    }
    let count = scheduler.ghost_count();
    scheduler.shift_in_time(Duration::from_secs(60));
    assert_eq!(
        scheduler.ghost_count(),
        count,
        "shift_in_time must not add or remove events"
    );
}
