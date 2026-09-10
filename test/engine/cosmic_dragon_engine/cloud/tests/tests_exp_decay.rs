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

/// Verify the resume accel exp decay math: at t=0, blend starts at ~0
/// (NO floor — NIGHT-hunter-8 removed the 0.05 first-frame jump, the
/// rain eases from frozen continuously); at t=SETTLE (~3.3s for k=0.9),
/// blend >= 95% → snap to full speed. This locks the asymmetric
/// k_resume=0.9 / settle 95% contract.
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

    // At t=0 (first frame), approach = 0, blend = 0.0 — NO floor jump
    // (NIGHT-hunter-8: the old 0.05 floor made the first frame jump
    // from frozen 0% to 5% speed in one step).
    cloud.rain_at(&mut frame, now);
    assert!(
        cloud.resume_blend < 0.02,
        "first-frame blend must start near zero (got {}) — no 0.05 floor jump",
        cloud.resume_blend
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

/// NIGHT-hunter-8: aborting a mid-deceleration pause (rapid p-tap) must
/// ramp smoothly from the CURRENT decel blend with the FAST abort rate —
/// not snap to 1.0 (the "little jump" the owner reported) and not drag
/// through the slow wake-up ramp (the old "rain stuck" bug).
#[test]
fn abort_decel_ramps_smoothly_from_current_blend_with_fast_rate() {
    let mut cloud = make_cloud();
    let mut frame = Frame::new(20, 10, cloud.palette.bg);
    let now = Instant::now();

    // Start deceleration, advance to t=1s (blend = exp(-1.2) ≈ 0.301).
    cloud.toggle_pause(); // BRANCH 3
    cloud.rain_at(&mut frame, now + Duration::from_secs(1));
    let blend_at_abort = cloud.resume_blend;
    assert!(
        (blend_at_abort - 0.301).abs() < 0.01,
        "setup: decel blend at t=1s should be ~0.30, got {blend_at_abort}"
    );

    // Abort: 'p' again at t=1s.
    cloud.toggle_pause(); // BRANCH 1
    assert!(
        cloud.resume_start.is_some(),
        "BRANCH 1 must start a resume ramp (NIGHT-hunter-8 — was a hard snap)"
    );
    assert!(
        cloud.pause_start.is_none(),
        "BRANCH 1 must clear pause_start"
    );
    assert_eq!(
        cloud.resume_blend, blend_at_abort,
        "BRANCH 1 must preserve the current blend (no snap — continuity at the abort instant)"
    );
    assert_eq!(
        cloud.resume_blend_start, blend_at_abort,
        "the ramp must interpolate from the decel's blend value"
    );

    // First frame after abort: blend barely moves (continuous, no jump).
    let t_abort = cloud.resume_start.unwrap();
    cloud.rain_at(&mut frame, t_abort + Duration::from_millis(16));
    assert!(
        (cloud.resume_blend - blend_at_abort).abs() < 0.08,
        "first post-abort frame must move gradually (got {} from {})",
        cloud.resume_blend,
        blend_at_abort
    );

    // Fast rate: 95% within ~0.6s (the slow 0.9 rate would need ~2.8s
    // from this start — the old stuck-feel).
    cloud.rain_at(&mut frame, t_abort + Duration::from_millis(600));
    assert!(
        cloud.resume_blend >= 0.95 || cloud.resume_start.is_none(),
        "fast abort rate must recover ~95% within 0.6s (got {})",
        cloud.resume_blend
    );
    assert_eq!(
        cloud.resume_blend, 1.0,
        "settled abort ramp must snap cleanly to full speed"
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

    // BRANCH 1: abort decel → pause_start cleared, a FAST resume ramp
    // takes over (NIGHT-hunter-8 — the hard snap to 1.0 was the owner's
    // "little jump"; the ramp preserves blend continuity).
    cloud.toggle_pause();
    assert!(
        cloud.pause_start.is_none(),
        "BRANCH 1 must clear pause_start"
    );
    assert!(
        cloud.resume_start.is_some(),
        "BRANCH 1 must start the fast abort-resume ramp (NIGHT-hunter-8)"
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

// ─────────────────────────────────────────────────────────────────
// NIGHT-hunter-28: resume phase continuity (the "small jump" fix)
// ─────────────────────────────────────────────────────────────────

/// NIGHT-hunter-28 (owner report: "for resume like still have small
/// jump so feel not smooth elegantly when see seriously high
/// detail"): unpause must preserve every droplet's frozen
/// `advance_remainder` — NOT re-randomize it.
///
/// The remainder drives BOTH head brightness
/// (`1.0 + fractional_progress * FRACTIONAL_HEAD_BRIGHTNESS_AMP`) and
/// the timing of the next row advance. The old
/// `rand_chance.sample()` re-randomization reshuffled every head's
/// brightness by up to ±15% in a single frame — the global shimmer
/// pop at the resume instant. Preservation is C0-continuous in both
/// the visual ramp and the advance schedule, and the spawn-time
/// phase jitter (SPAWN_PHASE_JITTER=true) already guarantees the
/// spread the old randomization tried to re-inject after the
/// zeroing bug.
#[test]
fn unpause_preserves_frozen_advance_remainders_night_hunter28() {
    let mut cloud = make_cloud();
    let mut frame = Frame::new(60, 40, cloud.palette.bg);
    let now = Instant::now();

    // Run the rain so a spread of live droplets exists.
    let mut t = now;
    for _ in 0..30 {
        cloud.rain_at(&mut frame, t);
        t += Duration::from_millis(16);
    }

    // Fully pause (decel through settle — the real user path). The
    // population may naturally churn DURING the decel window (tails
    // expire at the decaying blend) — that is not what this test
    // locks. The contract is at the FREEZE boundary: once fully
    // paused, rain_at returns early and NOTHING mutates; unpause
    // must hand the renderer the SAME frozen phases.
    cloud.toggle_pause(); // BRANCH 3: start decel
    let mut tp = t;
    for _ in 0..200 {
        // ~3.2s of decel: settles to `pause = true`.
        cloud.rain_at(&mut frame, tp);
        if cloud.pause {
            break;
        }
        tp += Duration::from_millis(16);
    }
    assert!(cloud.pause, "precondition: fully paused");

    // Snapshot the frozen phases (the full pause guarantees no
    // further mutation — rain_at returns before touching droplets).
    let remainders_frozen: Vec<f32> = cloud
        .droplets
        .iter()
        .filter(|d| d.is_alive)
        .map(|d| d.advance_remainder)
        .collect();
    assert!(
        remainders_frozen.len() > 3,
        "precondition: a live droplet population exists after the decel window"
    );
    // Belt-and-suspenders: a paused rain_at must not move them.
    cloud.rain_at(&mut frame, tp + Duration::from_millis(16));
    let remainders_still: Vec<f32> = cloud
        .droplets
        .iter()
        .filter(|d| d.is_alive)
        .map(|d| d.advance_remainder)
        .collect();
    assert_eq!(
        remainders_still, remainders_frozen,
        "the freeze itself must not touch the phase"
    );

    // THE FIX: the unpause call itself must preserve every frozen
    // remainder. The old code re-randomized each one here
    // (`rand_chance.sample()`) — the global brightness reshuffle
    // behind the owner's resume "small jump". Nothing runs between
    // the toggle and the snapshot, so this is an exact-equality
    // contract on the removed mutation.
    cloud.toggle_pause(); // BRANCH 2: unpause
    let remainders_after: Vec<f32> = cloud
        .droplets
        .iter()
        .filter(|d| d.is_alive)
        .map(|d| d.advance_remainder)
        .collect();
    assert_eq!(
        remainders_after, remainders_frozen,
        "NIGHT-hunter-28: unpause must preserve the frozen phase (no re-randomization)"
    );

    // The continuation itself (first frames advancing at the growing
    // resume blend) is already locked by the exp-decay math tests
    // above — `advance(now, lines, resume_blend)` is the same
    // time-scaled path the decel uses, and the phase equality at the
    // toggle boundary (the removed mutation) is the complete
    // regression contract for this fix.
}

/// The complementary half of NIGHT-hunter-28: the preserved remainders
/// must retain their SPREAD (no lockstep). This locks the reason the
/// old zeroing + randomization existed — with preservation, the
/// spawn-time jitter carries the spread through the freeze/thaw, so
/// the resumed rain does not march in synchronized rows.
#[test]
fn preserved_remainders_keep_their_spread_after_resume_night_hunter28() {
    let mut cloud = make_cloud();
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    let now = Instant::now();

    let mut t = now;
    for _ in 0..40 {
        cloud.rain_at(&mut frame, t);
        t += Duration::from_millis(16);
    }
    let remainders: Vec<f32> = cloud
        .droplets
        .iter()
        .filter(|d| d.is_alive)
        .map(|d| d.advance_remainder)
        .collect();
    assert!(remainders.len() > 3, "precondition: live population");

    // Pause through settle, then unpause.
    cloud.toggle_pause();
    let mut tp = t;
    for _ in 0..200 {
        cloud.rain_at(&mut frame, tp);
        if cloud.pause {
            break;
        }
        tp += Duration::from_millis(16);
    }
    cloud.toggle_pause();

    let after: Vec<f32> = cloud
        .droplets
        .iter()
        .filter(|d| d.is_alive)
        .map(|d| d.advance_remainder)
        .collect();
    // Distinct-enough spread: at least 3 distinct values across the
    // population (a lockstep would collapse them all to one).
    let mut distinct = after.clone();
    distinct.sort_by(|a, b| a.partial_cmp(b).unwrap());
    distinct.dedup_by(|a, b| (*a - *b).abs() < 0.05);
    assert!(
        distinct.len() >= 3,
        "resumed remainders lost their spread (lockstep): {after:?}"
    );
}
