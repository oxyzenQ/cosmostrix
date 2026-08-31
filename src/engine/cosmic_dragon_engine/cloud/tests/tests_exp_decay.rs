// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Exp decay easing regression tests — extracted from
//! `cloud/tests/mod.rs` to keep that file under the 800-LOC hard cap.
//!
//! These tests lock the asymmetric k_decel=1.2 / k_accel=0.8 / settle 5%
//! contract for pause/resume easing.

use super::*;

// ── Exp decay easing regression tests (v50.0.0-beta.5 masterclass consolidation) ──────

/// Verify the pause decel exp decay math: at t=0, blend = 1.0 (full
/// speed); at t=SETTLE (~2.5s for k=1.2), blend <= 5% → snap to fully
/// paused. This locks the asymmetric k_decel=1.2 / settle 5% contract.
#[test]
fn pause_decel_exp_decay_settles_at_documented_threshold() {
    let mut cloud = make_cloud();
    let mut frame = Frame::new(20, 10, cloud.palette.bg);
    let now = Instant::now();

    // Start decel.
    cloud.toggle_pause();
    assert!(cloud.pause_start.is_some(), "decel must start");
    assert!(!cloud.pause, "must not be fully paused immediately");

    // At t=0 (first frame after toggle), pause_blend should be ~1.0
    // (exp(-1.2*0) = 1.0). Cloud is still running, not settled.
    cloud.rain_at(&mut frame, now);
    assert!(!cloud.pause, "at t=0 cloud is still running (blend=1.0)");
    assert!(
        cloud.pause_start.is_some(),
        "decel still in progress at t=0"
    );

    // At t=1s, blend = exp(-1.2) = 0.301. Still > 5% → not settled.
    cloud.rain_at(&mut frame, now + Duration::from_secs(1));
    assert!(
        !cloud.pause,
        "at t=1s blend=0.30 still above settle threshold"
    );
    assert!(
        cloud.pause_start.is_some(),
        "decel still in progress at t=1s"
    );

    // At t=3s, blend = exp(-3.6) = 0.027. Below 5% → snap to fully paused.
    cloud.rain_at(&mut frame, now + Duration::from_secs(3));
    assert!(
        cloud.pause,
        "at t=3s blend < 5% → must snap to fully paused"
    );
    assert!(
        cloud.pause_start.is_none(),
        "pause_start cleared after settle snap"
    );
    assert_eq!(
        cloud.resume_blend, 0.0,
        "resume_blend must be 0 after fully paused"
    );
}

/// Verify the resume accel exp decay math: at t=0, blend = 0.05 (floor);
/// at t=SETTLE (~3.3s for k=0.9), blend >= 95% → snap to full speed.
/// This locks the asymmetric k_resume=0.9 / settle 95% contract.
#[test]
fn resume_accel_exp_decay_settles_at_documented_threshold() {
    let mut cloud = make_cloud();
    let mut frame = Frame::new(20, 10, cloud.palette.bg);
    let now = Instant::now();

    // Set up: fully paused, then unpause to start resume ramp.
    cloud.pause = true;
    cloud.pause_time = Some(now - Duration::from_secs(5));
    cloud.toggle_pause(); // BRANCH 2: unpause, sets resume_start = now
    assert!(cloud.resume_start.is_some(), "resume ramp must start");
    assert_eq!(
        cloud.resume_blend, 0.0,
        "blend starts at 0 right after toggle"
    );

    // At t=0 (first frame), approach = 0, blend = max(0, 0.05) = 0.05 (floor).
    cloud.rain_at(&mut frame, now);
    assert!(
        cloud.resume_blend >= 0.05,
        "first-frame blend must hit the 0.05 floor (approach=0 + floor)"
    );
    assert!(
        cloud.resume_start.is_some(),
        "ramp still in progress at t=0"
    );

    // At t=1s, approach = 1 - exp(-0.9) = 0.593, blend = 0.593. Below 95%.
    cloud.rain_at(&mut frame, now + Duration::from_secs(1));
    assert!(
        cloud.resume_blend < 0.95,
        "at t=1s blend=0.59 still below settle threshold"
    );
    assert!(
        cloud.resume_start.is_some(),
        "ramp still in progress at t=1s"
    );

    // At t=4s, approach = 1 - exp(-3.6) = 0.973. Above 95% → snap to full.
    cloud.rain_at(&mut frame, now + Duration::from_secs(4));
    assert_eq!(
        cloud.resume_blend, 1.0,
        "at t=4s blend > 95% → must snap to 1.0"
    );
    assert!(
        cloud.resume_start.is_none(),
        "resume_start cleared after settle snap"
    );
}

/// Verify the glyph entry ramp exp decay: blend rises from 0 toward 1
/// via 1 - exp(-k*t) with k=GLYPH_ENTRY_RAMP_DECAY_RATE; settles at 95%
/// in ~700ms (GLYPH_ENTRY_RAMP_DURATION_MS) → clears glyph_entry_time.
#[test]
fn glyph_entry_ramp_exp_decay_settles_at_documented_duration() {
    use crate::constants::{
        GLYPH_ENTRY_RAMP_DECAY_RATE, GLYPH_ENTRY_RAMP_DURATION_MS, GLYPH_ENTRY_RAMP_MIN_SCALE,
        GLYPH_ENTRY_RAMP_SETTLE_FRAC,
    };

    // Sanity-check the constant derivations: at t=DURATION/1000 sec,
    // the blend must reach SETTLE_FRAC (95%).
    let dur_secs = GLYPH_ENTRY_RAMP_DURATION_MS as f32 / 1000.0;
    let blend_at_settle = 1.0 - (-GLYPH_ENTRY_RAMP_DECAY_RATE * dur_secs).exp();
    assert!(
        (blend_at_settle - GLYPH_ENTRY_RAMP_SETTLE_FRAC).abs() < 0.01,
        "k must be derived so blend(dur) = settle_frac: got {} vs {}",
        blend_at_settle,
        GLYPH_ENTRY_RAMP_SETTLE_FRAC
    );
    // MIN_SCALE must be < SETTLE_FRAC so the ramp interpolates a real range.
    // const-block evaluated at compile time (clippy::assertions_on_constants).
    const {
        assert!(
            GLYPH_ENTRY_RAMP_MIN_SCALE < GLYPH_ENTRY_RAMP_SETTLE_FRAC,
            "MIN_SCALE must be below SETTLE_FRAC"
        );
    }

    // Set glyph_entry_time in the past past the settle threshold and
    // verify rain_at clears it (snap to full speed, ramp state gone).
    let mut cloud = make_cloud();
    let mut frame = Frame::new(20, 10, cloud.palette.bg);
    let now = Instant::now();
    cloud.glyph_entry_time = Some(now - Duration::from_millis(1000));
    // 1000ms > 700ms settle time → next rain_at must clear it.
    cloud.last_spawn_time = now - Duration::from_secs(1);
    cloud.rain_at(&mut frame, now);
    assert!(
        cloud.glyph_entry_time.is_none(),
        "glyph_entry_time must clear after settle threshold elapsed"
    );
}

/// Verify the audit §8.6 invariant: pause_start and resume_start
/// cannot coexist. toggle_pause() must always clear one before setting
/// the other across all three branches (start-decel, abort-decel,
/// unpause-from-paused). This is enforced by a debug_assert! in
/// rain_at, but the underlying state-machine contract is verified here.
#[test]
fn pause_start_and_resume_start_never_coexist_across_toggle_branches() {
    let mut cloud = make_cloud();

    // Initial state: neither set.
    assert!(
        cloud.pause_start.is_none() && cloud.resume_start.is_none(),
        "fresh cloud must have neither easing active"
    );

    // BRANCH 3: start decel → pause_start set, resume_start cleared.
    cloud.toggle_pause();
    assert!(cloud.pause_start.is_some());
    assert!(
        cloud.resume_start.is_none(),
        "BRANCH 3 must clear resume_start"
    );

    // BRANCH 1: abort decel → both cleared (snap to full speed).
    cloud.toggle_pause();
    assert!(
        cloud.pause_start.is_none(),
        "BRANCH 1 must clear pause_start"
    );
    assert!(
        cloud.resume_start.is_none(),
        "BRANCH 1 must NOT start a resume ramp (instant snap to 1.0)"
    );

    // BRANCH 2: fully paused → unpause → resume_start set, pause_start cleared.
    cloud.pause = true;
    cloud.pause_time = Some(Instant::now() - Duration::from_secs(5));
    cloud.toggle_pause();
    assert!(
        cloud.pause_start.is_none(),
        "BRANCH 2 must clear pause_start"
    );
    assert!(
        cloud.resume_start.is_some(),
        "BRANCH 2 must set resume_start"
    );
}
