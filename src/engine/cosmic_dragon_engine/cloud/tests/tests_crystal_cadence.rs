// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Crystal Dragon cadence contract (v80.0.0-alpha.1 S-master-HUNT-6).
//!
//! Owner bug (2026-09-03): `--crystal-dragon-secs 10m` still drifted every
//! 60s, and enabling crystal-dragon via a config edit produced a burst of
//! drifts within milliseconds ("flashy"). Root cause: the drift DECISION in
//! `crystal_dragon_tick` ran every frame (gated only by the 60s dwell floor),
//! so `polling_secs` never paced anything slower than the floor, and every
//! live-reload rebuild (which resets `drift_active`) re-armed an immediate
//! drift.
//!
//! The HUNT-6 contract pinned here (structural timing bounds — deterministic
//! regardless of the RNG's theme draw, which may legitimately decide to stay
//! on the current theme):
//! 1. A drift decision may only run on a tick where the polling interval
//!    elapsed — `polling_secs` is the cadence governor. No fire may appear
//!    strictly before a poll boundary, and at most one fire can exist per
//!    boundary.
//! 2. The arming tick (first tick after activation, `last_poll == None`)
//!    polls the sensor but decides nothing — the clock starts at that tick.
//! 3. A pre-set mid-cycle clock (reload while the engine is ON) keeps the
//!    boundary phase — no early fire.
//! 4. `inherit_ecosystem_state` from an engine-OFF cloud resets the clock
//!    (fresh arm on the off->on enable); from a running engine it keeps it.

use std::time::{Duration, Instant};

use rand::rngs::StdRng;
use rand::SeedableRng;

use super::make_cloud;

/// Drive `crystal_dragon_tick` over simulated wall-clock time at ~60fps
/// and collect the offsets (seconds from `start`) at which the engine
/// emitted a drift decision.
///
/// `drift_chance` is forced to 1.0 so every decision-eligible tick that
/// passes dwell attempts a drift; the theme clock is aged far past the
/// dwell floor so dwell never masks a boundary. What remains is pure
/// TIMING: which ticks are decision-eligible.
fn drift_fire_offsets(polling_secs: f32, total_secs: u64, last_poll: Option<Instant>) -> Vec<f32> {
    let mut cloud = make_cloud();
    cloud.crystal_dragon = true;
    cloud.crystal_dragon_control.polling_secs = polling_secs;
    cloud.crystal_dragon_control.min_dwell_secs = polling_secs.min(60.0);
    cloud.crystal_dragon_control.drift_chance = 1.0;
    cloud.crystal_dragon_last_poll = last_poll;
    cloud.mt = StdRng::seed_from_u64(0x4855_4E54);

    // Age the theme far beyond any dwell floor so dwell never gates.
    let start = Instant::now();
    cloud
        .crystal_dragon_sensor
        .record_theme_transition(start - Duration::from_secs(3_600));

    let frame_dt = Duration::from_millis(16);
    let frames = total_secs.saturating_mul(1_000) / 16;
    let mut fires = Vec::new();
    for i in 0..frames {
        let now = start + frame_dt.saturating_mul(i as u32);
        if cloud.crystal_dragon_tick(now).is_some() {
            fires.push(now.saturating_duration_since(start).as_secs_f32());
        }
    }
    fires
}

/// The arming contract: a freshly activated engine (last_poll = None) must
/// NOT drift on its first tick — the clock starts there and the first
/// decision is owed one full polling interval later. This is what kills
/// the owner's "burst drift in milliseconds after enabling via config"
/// symptom (the first tick previously polled AND decided immediately).
#[test]
fn first_tick_arms_the_clock_without_a_drift() {
    let mut cloud = make_cloud();
    cloud.crystal_dragon = true;
    cloud.crystal_dragon_control.polling_secs = 600.0;
    cloud.crystal_dragon_control.drift_chance = 1.0;
    cloud.mt = StdRng::seed_from_u64(0x4855_4E54);
    let start = Instant::now();
    cloud.crystal_dragon_sensor.record_theme_transition(start);

    // Arming tick: no drift, and the clock is pinned to this tick.
    assert!(
        cloud.crystal_dragon_tick(start).is_none(),
        "the arming tick must not emit a drift decision"
    );
    assert_eq!(
        cloud.crystal_dragon_last_poll,
        Some(start),
        "the arming tick must pin the cadence clock"
    );

    // 10 minutes of frames (600s, 16ms apart) — none may fire: the first
    // boundary is at arming + 600s.
    let mut fired = false;
    for i in 1..(600_u64 * 62) {
        let now = start + Duration::from_millis(16) * i as u32;
        if cloud.crystal_dragon_tick(now).is_some() {
            // The boundary tick itself (i * 16ms >= 600s) is allowed.
            let offset = i as f32 * 0.016;
            assert!(
                offset >= 599.0,
                "a drift fired at {offset:.3}s — before the first 600s poll boundary"
            );
            fired = true;
        }
    }
    assert!(
        !fired,
        "in a 600s window starting from the arming tick the first boundary \
         (exactly at +600s) must fall outside the window's last frame"
    );
}

/// The core owner bug: a 10-minute cadence (600s) must not drift at the
/// 60s dwell floor. Over 1200s of simulated time there are at most two
/// poll boundaries (at +600s and +1200s) — every fire must sit on one of
/// them, and nothing may fire before the first boundary.
#[test]
fn slow_cadence_paces_drifts_at_polling_secs_not_the_60s_floor() {
    let fires = drift_fire_offsets(600.0, 1_200, None);
    assert!(
        fires.iter().all(|t| *t >= 599.0),
        "no drift decision before the first 600s poll boundary (got {fires:?})"
    );
    assert!(
        fires.len() <= 2,
        "at most one drift decision per poll boundary: 1200s at a 600s cadence \
         allows at most 2, got {fires:?}"
    );
}

/// LTS guard: the tuned-fast case (owner-verified 3s cadence — "elegantly
/// changes every 3s") keeps firing on its boundaries. Over 12s there are
/// at most 4 boundaries (after the arm at +0); every fire must sit at a
/// 3s boundary and at least one must actually fire (the engine is alive
/// at a fast cadence).
#[test]
fn fast_cadence_still_fires_on_its_boundaries() {
    let fires = drift_fire_offsets(3.0, 12, None);
    assert!(
        fires.iter().all(|t| *t >= 2.5),
        "no drift decision before the first 3s poll boundary (got {fires:?})"
    );
    assert!(
        fires.len() <= 4,
        "at most one drift decision per 3s boundary (got {fires:?})"
    );
    assert!(
        !fires.is_empty(),
        "a 3s cadence over 12s must yield at least one drift decision"
    );
}

/// The reload contract (engine ON): a pre-set clock 300s into a 600s cycle
/// keeps the boundary phase — the only boundary in the next 700s window is
/// at +300s. Nothing may fire before it, and at most one fire exists.
#[test]
fn mid_cycle_clock_keeps_the_boundary_phase() {
    let start = Instant::now();
    let fires = drift_fire_offsets(600.0, 700, Some(start - Duration::from_secs(300)));
    assert!(
        fires.iter().all(|t| *t >= 299.0),
        "a mid-cycle inherited clock must not fire early (got {fires:?})"
    );
    assert!(
        fires.len() <= 1,
        "only one boundary (+300s) exists in a 700s window (got {fires:?})"
    );
}

/// The off->on enable path: the OLD cloud had crystal-dragon OFF, so
/// `inherit_ecosystem_state` must reset the clock to None (arm fresh) even
/// if the old cloud carried a stale Some(last_poll) — e.g. the engine was
/// on earlier in the session, was switched off, and is being re-enabled.
#[test]
fn inherit_from_off_engine_arms_a_fresh_clock() {
    let mut old = make_cloud();
    old.crystal_dragon = false; // engine OFF at reload time
    old.crystal_dragon_last_poll = Some(Instant::now() - Duration::from_secs(3_600));

    let mut fresh = make_cloud();
    fresh.inherit_ecosystem_state(&old);
    assert!(
        fresh.crystal_dragon_last_poll.is_none(),
        "an off->on enable must arm a fresh cadence clock (None)"
    );
}

/// The on->on reload path: the cadence phase survives so an unrelated
/// config edit mid-cycle cannot produce a mid-cycle drift decision.
#[test]
fn inherit_from_running_engine_keeps_the_clock() {
    let stale = Instant::now() - Duration::from_secs(120);
    let mut old = make_cloud();
    old.crystal_dragon = true;
    old.crystal_dragon_last_poll = Some(stale);

    let mut fresh = make_cloud();
    fresh.inherit_ecosystem_state(&old);
    assert_eq!(
        fresh.crystal_dragon_last_poll,
        Some(stale),
        "a running engine's cadence phase must survive the reload"
    );
}

/// Hysteresis guard: dwell still blocks a drift on a genuine poll boundary
/// when the theme was entered too recently — the HUNT-6 poll gate must not
/// bypass the anti-flicker floor.
#[test]
fn dwell_floor_still_blocks_a_boundary_drift() {
    let mut cloud = make_cloud();
    cloud.crystal_dragon = true;
    cloud.crystal_dragon_control.polling_secs = 600.0;
    cloud.crystal_dragon_control.min_dwell_secs = 60.0;
    cloud.crystal_dragon_control.drift_chance = 1.0;
    cloud.mt = StdRng::seed_from_u64(0x4855_4E54);
    let start = Instant::now();
    // Arm the clock 600s in the past: the next tick IS a poll boundary.
    cloud.crystal_dragon_last_poll = Some(start - Duration::from_secs(600));
    // Theme entered 0.5s ago — far under the 60s dwell floor.
    cloud
        .crystal_dragon_sensor
        .record_theme_transition(start - Duration::from_millis(500));
    assert!(
        cloud.crystal_dragon_tick(start).is_none(),
        "a poll boundary must still respect the dwell floor (theme entered 0.5s ago, floor 60s)"
    );
    // The boundary was consumed by the blocked tick (clock advanced) —
    // the next 600s are decision-ineligible by construction (poll gate).
    assert!(
        cloud
            .crystal_dragon_tick(start + Duration::from_secs(1))
            .is_none(),
        "after the boundary is consumed, no further decision may run this cycle"
    );
}

// ── S-master-HUNT-7: shipped defaults honor the cadence contract ──────────
//
// Owner bug (2026-09-03, post-210aed3): `crystal-dragon = 1` in config with
// the CLI-locked 120s cadence produced ZERO visible drifts while the HUD
// reported `crdr: on`; the 3s tuned case visibly worked. Root cause: the
// HUNT-6 poll gate moved the drift dice from per-frame to per-boundary, but
// the shipped `drift_chance` stayed 0.12 — a value calibrated for the
// per-frame world (~8 frames to a pass at 60fps). Per boundary it starved
// the cadence by 8.3x: expected time to the first drift was
// `polling_secs / 0.12` (~16.7 minutes at the 120s cadence, ~8.3 minutes at
// the 60s default). The fix ships `drift_chance = 1.0`: every dwell-eligible
// poll boundary FIRES, matching the deterministic rhythm post_rain.rs
// documents and the 1.0 semantics the HUNT-6 tests above already lock. The
// two tests below exercise the SHIPPED defaults (the helpers above force
// `drift_chance = 1.0` explicitly; these must NOT) so a regression back to
// a fractional dice value fails immediately.

/// Same harness as `drift_fire_offsets` but with the SHIPPED control
/// defaults for `drift_chance` (only the timing fields are set) — this is
/// the production configuration an owner actually runs.
fn drift_fire_offsets_shipped_defaults(
    polling_secs: f32,
    total_secs: u64,
    last_poll: Option<Instant>,
) -> Vec<f32> {
    let mut cloud = make_cloud();
    cloud.crystal_dragon = true;
    cloud.crystal_dragon_control.polling_secs = polling_secs;
    cloud.crystal_dragon_control.min_dwell_secs = polling_secs.min(60.0);
    // NOTE: drift_chance deliberately NOT set — the shipped default must
    // carry the cadence contract on its own.
    cloud.crystal_dragon_last_poll = last_poll;
    cloud.mt = StdRng::seed_from_u64(0x4855_4E54);

    let start = Instant::now();
    cloud
        .crystal_dragon_sensor
        .record_theme_transition(start - Duration::from_secs(3_600));

    let frame_dt = Duration::from_millis(16);
    let frames = total_secs.saturating_mul(1_000) / 16;
    let mut fires = Vec::new();
    for i in 0..frames {
        let now = start + frame_dt.saturating_mul(i as u32);
        if cloud.crystal_dragon_tick(now).is_some() {
            fires.push(now.saturating_duration_since(start).as_secs_f32());
        }
    }
    fires
}

/// The owner's exact case 2 shape: engine on, a slow (CLI-locked style)
/// cadence, dwell long elapsed. Over 1200s at a 600s cadence there are two
/// poll boundaries (+600s, +1200s) — BOTH must fire with the shipped
/// defaults. At the pre-HUNT-7 0.12 dice both boundaries firing had a
/// ~1.4% chance; a single missed boundary already starves the cadence past
/// the next poll cycle (the "crdr: on but nothing drifts" symptom).
#[test]
fn shipped_defaults_fire_on_both_boundaries_of_a_slow_cadence() {
    let fires = drift_fire_offsets_shipped_defaults(600.0, 1_200, None);
    assert!(
        fires.iter().all(|t| *t >= 599.0),
        "no drift decision before the first 600s poll boundary (got {fires:?})"
    );
    assert!(
        !fires.is_empty(),
        "the shipped defaults must fire on the first 600s boundary — a silent \
         engine while the HUD reports crdr: on is the HUNT-7 regression"
    );
}

/// The owner's case 3 shape (visibly working): a fast 3s cadence must fire
/// on (nearly) every boundary with the shipped defaults — 12s hold 4
/// boundaries. The rare calc-v2 same-theme double-draw can no-op a single
/// boundary (~0.5% with 14 themes per group); requiring 3 of 4 tolerates
/// exactly one such no-op while still failing hard under any fractional
/// dice (at the old 0.12, P(3+ fires in 4 boundaries) is ~0.2%).
#[test]
fn shipped_defaults_fire_on_nearly_every_fast_boundary() {
    let fires = drift_fire_offsets_shipped_defaults(3.0, 12, None);
    assert!(
        fires.iter().all(|t| *t >= 2.5),
        "no drift decision before the first 3s poll boundary (got {fires:?})"
    );
    assert!(
        fires.len() >= 3,
        "a 3s cadence over 12s must fire on at least 3 of 4 boundaries with \
         the shipped defaults (got {fires:?}) — the pre-HUNT-7 0.12 dice \
         starved this to a ~0.5-fire average"
    );
}
