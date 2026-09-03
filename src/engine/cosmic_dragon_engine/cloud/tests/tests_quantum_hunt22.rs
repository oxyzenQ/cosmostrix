// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! S-master-HUNT-22 regression tests: real-time particle physics.
//!
//! The quantum-ripple particle family (mouse-click sparks, border-touch
//! splash crowns) previously integrated
//! `dt = min(dt_raw, 1/30, max_sim_delta)` per frame. On slow terminals
//! (VTE: 10-15 FPS real with perf_pressure pinned at 1.0, so
//! max_sim_delta = 15ms), each 67-200ms real frame admitted only
//! 15-33ms of particle physics — a permanent 10-30% time dilation. The
//! 4.0s ripple stretched to 20-40 wall-clock seconds of slow drift
//! ("snow ice"), then the velocity decay froze particles mid-air
//! ("stuck"), and they only vanished once the diluted sim_age finally
//! crossed the lifetime ("disappears by itself").
//!
//! HUNT-22 replaces the chain with
//! `dt = min(dt_raw, PARTICLE_MAX_FRAME_DT_SECS) * resume_blend`:
//! particles move and age on the same REAL clock as the co-spawned
//! flash wave, completing their intended duration at any frame rate.
//! These tests lock that contract.

use std::time::{Duration, Instant};

use super::tests_quantum::make_truecolor_cloud;
use super::tests_quantum_v50::pin_one_particle;
use crate::constants::{
    PARTICLE_MAX_FRAME_DT_SECS, QUANTUM_RIPPLE_LIFETIME_SECS, QUANTUM_RIPPLE_SPEED,
};
use crate::frame::Frame;
use crate::runtime::ColorScheme;

/// A slow terminal's REAL frame interval must advance particle physics
/// by the FULL real delta — not the old 1/30 cap, and not the dilated
/// sim_cap the event loop applies under perf pressure.
///
/// Simulates the reported VTE scenario: 10 FPS (100ms frames) with the
/// event loop's pressure-throttled `max_sim_delta` (15ms) active. After
/// LIFETIME_SECS of wall-clock time, the particle must have EXPIRED.
/// Under the old clamped clock the same particle accumulated only
/// ~1.3s of sim_age after 4.0 real seconds — still drifting, the
/// exact "snow ice / stuck" regression this hunt fixes.
#[test]
fn quantum_particles_expire_in_real_time_on_slow_terminals() {
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    let spawn_time = Instant::now();
    let idx = pin_one_particle(&mut cloud, 10, 5, 0.0, 0.0, spawn_time);

    // Simulate the VTE event-loop state: perf_pressure saturated → the
    // event loop pins max_sim_delta at 15ms (60 FPS target × 0.3 factor).
    // The particle path must be decoupled from this dilated clock.
    cloud.set_max_sim_delta(Duration::from_millis(15));

    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let frame_interval = Duration::from_millis(100); // 10 FPS real
    let total_real = Duration::from_secs_f32(QUANTUM_RIPPLE_LIFETIME_SECS + 0.5); // lifetime + margin
    let mut t = spawn_time;
    let mut sim_age_seen = 0.0_f32;
    while t < spawn_time + total_real {
        t += frame_interval;
        cloud.apply_quantum_ripple(&mut frame, t);
        sim_age_seen = sim_age_seen.max(cloud.quantum_particles[idx].sim_age);
    }

    assert!(
        !cloud.quantum_particles[idx].active,
        "particle must expire after {:.1}s of REAL time at 10 FPS (sim_age reached {:.2}s; \
         under the old dilated clock it was still active with sim_age ~1.3s — the \
         stuck/snow-ice regression)",
        total_real.as_secs_f32(),
        sim_age_seen
    );
    assert_eq!(
        cloud.quantum_active_count, 0,
        "active_count must return to zero once the last particle expires"
    );
}

/// Anti-teleport: a pathological gap (focus loss / SIGSTOP / post-stall
/// first frame) integrates at most PARTICLE_MAX_FRAME_DT_SECS of
/// physics, never the full multi-second delta.
#[test]
fn quantum_dt_is_capped_by_particle_max_frame_dt() {
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    let spawn_time = Instant::now();
    let idx = pin_one_particle(&mut cloud, 10, 5, 0.0, 0.0, spawn_time);

    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    // 5-second stall (alt-tab, SIGSTOP, terminal freeze).
    cloud.apply_quantum_ripple(&mut frame, spawn_time + Duration::from_secs(5));

    let sim_age = cloud.quantum_particles[idx].sim_age;
    assert!(
        (sim_age - PARTICLE_MAX_FRAME_DT_SECS).abs() < 1e-4,
        "a 5s gap must integrate exactly the anti-teleport cap \
         ({PARTICLE_MAX_FRAME_DT_SECS}s), got {sim_age}s"
    );
    // The cap must be generous enough to let a 10 FPS terminal run in
    // real time (frame interval 100ms < cap) but tight enough that a
    // stall hop stays sub-burst-radius. Compile-time guard (codebase
    // convention — see tests_quantum_v50.rs).
    const _: () = assert!(
        PARTICLE_MAX_FRAME_DT_SECS >= 0.1 && PARTICLE_MAX_FRAME_DT_SECS <= 0.5,
        "PARTICLE_MAX_FRAME_DT_SECS is outside the [0.1, 0.5] real-time window: \
         below 0.1 dilates 10 FPS terminals again, above 0.5 lets stalls \
         teleport particles across the burst radius"
    );
}

/// Real-time motion: at a 10 FPS cadence a particle with a known
/// velocity covers the SAME distance per wall-clock second as at 60
/// FPS. This is the core HUNT-22 contract — motion speed depends on
/// real time, not on how many frames the terminal managed to render.
#[test]
fn quantum_motion_covers_equal_real_distance_at_low_and_high_fps() {
    let setup = |fps_frame_interval: Duration| -> f32 {
        let mut cloud = make_truecolor_cloud(ColorScheme::Green);
        let spawn_time = Instant::now();
        // Pure +x velocity, no decay sensitivity: run only 3 frames so
        // the velocity decay (35%/s) affects both runs nearly equally.
        let idx = pin_one_particle(&mut cloud, 5, 5, QUANTUM_RIPPLE_SPEED, 0.0, spawn_time);
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut t = spawn_time;
        for _ in 0..3 {
            t += fps_frame_interval;
            cloud.apply_quantum_ripple(&mut frame, t);
        }
        cloud.quantum_particles[idx].x
    };

    // 60 FPS: 3 frames × 16.7ms = 50ms of real time.
    let x_high = setup(Duration::from_secs_f32(1.0 / 60.0));
    // 10 FPS: 3 frames × 100ms = 300ms of real time.
    // Both use the same velocity, so distance must scale with REAL time:
    // x_low - x0 ≈ 6 × (x_high - x0) within the decay's second-order effect.
    let x_low = setup(Duration::from_millis(100));

    let d_high = x_high - 5.5; // spawn x = col + 0.5
    let d_low = x_low - 5.5;
    // 300ms vs 50ms → ratio 6.0; velocity decay (~35%/s over ≤0.3s)
    // shrinks the low-FPS run by up to ~10%. Allow 5.0..6.3.
    let ratio = d_low / d_high.max(1e-6);
    assert!(
        (5.0..=6.3).contains(&ratio),
        "distance ratio (10 FPS / 60 FPS) = {ratio:.2}, expected ~6.0 — particle \
         motion must cover equal REAL distance per second regardless of frame rate"
    );
}

/// Unpause must shift the engrave/scorch particle clocks forward by the
/// pause duration (§8.5 family, alongside last_quantum_update_time), so
/// mid-flight sparks/smoke do not burn their anti-teleport budget on
/// the first post-unpause frame.
#[test]
fn unpause_shifts_msg_fill_particle_clocks() {
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    let now = Instant::now();
    let pause_len = Duration::from_secs(2);

    // Fully paused 2 seconds ago; particle clocks are equally stale.
    cloud.pause = true;
    cloud.pause_time = Some(now - pause_len);
    let stale = now - pause_len;
    cloud.last_quantum_update_time = stale;
    cloud.engrave.last_update = stale;
    cloud.scorch.last_update = stale;

    // BRANCH 2 unpause: shifts every particle clock by the pause length.
    cloud.toggle_pause();

    // A correctly shifted clock lands at (or a hair after) `now`, so the
    // remaining lag vs. `now` must be well under the 2s pause length.
    let shift_ok = |t: Instant, name: &str| {
        let remaining_lag = now.saturating_duration_since(t);
        assert!(
            remaining_lag < Duration::from_millis(50),
            "{name} clock was not shifted by the pause duration (still {remaining_lag:?} behind now)"
        );
    };
    shift_ok(cloud.last_quantum_update_time, "quantum");
    shift_ok(cloud.engrave.last_update, "engrave");
    shift_ok(cloud.scorch.last_update, "scorch");
}
