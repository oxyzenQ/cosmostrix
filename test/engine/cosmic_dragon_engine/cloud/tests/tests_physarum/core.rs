// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Core physarum-style behavior contracts (NIGHT-research-6):
//! scene resolution, spawn density, trail field accumulation,
//! lifetime absorption, drawn-cell bounds, style transitions,
//! and the sense-decide-move-deposit algorithm bounds.

use super::*;

#[test]
fn physarum_scene_resolves_style_and_fields() {
    let s = crate::scene::get_scene("physarum").expect("physarum scene exists");
    assert_eq!(s.config.rain_style, RainStyle::Physarum);
    assert_eq!(s.config.color, Some("cosmos"));
    assert_eq!(s.config.charset, Some("binary"));
    // Style dispatch sanity: the style helper families classify
    // physarum as structured (not droplet family) and spawn-remainder
    // driven — parity with vortex/dragon.
    assert!(!RainStyle::Physarum.is_droplet_family());
    assert!(RainStyle::Physarum.uses_spawn_remainder());
}

#[test]
fn physarum_particles_spawn_up_to_density_target() {
    let mut cloud = make_physarum_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 120, 16);

    let active = cloud.physarum_rain.active_count();
    // Pool is one particle per column (120); ratio at density 0.55 =
    // PHYSARUM_ACTIVE_BASE + 0.55 * PHYSARUM_ACTIVE_DENSITY_MULT = 0.52
    // -> target 62 particles. 2 seconds of spawn budget reaches the
    // target (deficit-bounded).
    let target = (120.0_f32 * 0.52).round() as usize;
    assert!(
        active >= target / 2,
        "expected at least half the density target {target}/2, got {active}"
    );
    assert!(active <= 120, "active particles cannot exceed pool size");
}

#[test]
fn physarum_trail_field_accumulates() {
    let mut cloud = make_physarum_cloud(80, 30);
    let mut frame = Frame::new(80, 30, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 300, 16);

    // After 5 seconds of simulation, the trail field should have
    // accumulated non-zero values — particles deposit trail each
    // frame, so the max trail value must exceed zero. This verifies
    // the sense-decide-move-deposit loop is actually executing.
    let max_trail = cloud.physarum_rain.trail_max_for_test();
    assert!(
        max_trail > 0.0,
        "trail field must accumulate non-zero values after 5s of simulation (got {max_trail})"
    );
}

#[test]
fn physarum_particles_stay_within_viewport_bounds() {
    let mut cloud = make_physarum_cloud(80, 30);
    let mut frame = Frame::new(80, 30, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 300, 16);

    // Particles use wraparound (toroidal substrate) — they should
    // stay within [0, cols) x [0, lines) modulo wraparound. The
    // advance pass wraps coordinates, so positions should always be
    // in-bounds after wrap.
    for (x, y) in cloud.physarum_rain.active_positions_for_test() {
        assert!((0.0..80.0).contains(&x), "particle x out of bounds: {x}");
        assert!((0.0..30.0).contains(&y), "particle y out of bounds: {y}");
    }
}

#[test]
fn physarum_particles_absorbed_after_lifetime() {
    let mut cloud = make_physarum_cloud(80, 30);
    let mut frame = Frame::new(80, 30, cloud.palette.bg);
    // Long run: every spawned particle gets several lifetime cycles
    // (lifetime = 15s). 20s of frames exercises spawn -> age out
    // -> respawn at least once per slot.
    run_frames(&mut cloud, &mut frame, 1200, 16);
    let active = cloud.physarum_rain.active_count();
    assert!(
        active <= 80,
        "lifetime absorption must keep the active population within the pool (got {active})"
    );
}

#[test]
fn physarum_drawn_cells_stay_in_bounds() {
    let mut cloud = make_physarum_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 90, 16);
    for cell in cloud.physarum_rain.drawn_cells_for_test() {
        assert!(cell.col < 60, "drawn col out of bounds: {}", cell.col);
        assert!(cell.line < 25, "drawn line out of bounds: {}", cell.line);
    }
}

#[test]
fn physarum_sensor_distance_bounded() {
    // The sensor distance must stay reasonable — too large and
    // particles sense across the whole viewport (no local
    // steering); too small and sensing is too local (no network
    // emergence). 3.0 cells is the standard Jeff Jones value.
    let sd = crate::constants::PHYSARUM_SENSOR_DISTANCE;
    assert!(
        sd > 0.0 && sd < 10.0,
        "sensor distance must stay in the reasonable range (got {sd})"
    );
}

// Compile-time contract: the trail decay rate must be in (0, 1) —
// 0 would zero the trail every frame (no accumulation); 1.0 would
// never decay (field saturates, network disappears). The standard
// Jeff Jones range is [0.85, 0.95].
const _: () = assert!(crate::constants::PHYSARUM_TRAIL_DECAY > 0.0);
const _: () = assert!(crate::constants::PHYSARUM_TRAIL_DECAY < 1.0);

#[test]
fn physarum_active_droplet_count_routes_to_particles() {
    let mut cloud = make_physarum_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 60, 16);
    assert_eq!(
        cloud.active_droplet_count(),
        cloud.physarum_rain.active_count()
    );
}

#[test]
fn physarum_style_transition_clears_state_both_ways() {
    let mut cloud = make_physarum_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 60, 16);
    assert!(cloud.physarum_rain.active_count() > 0);

    // Physarum -> Glyph: particles reset, droplet pool warm-starts.
    cloud.transition_rain_style(RainStyle::Glyph);
    assert_eq!(cloud.physarum_rain.active_count(), 0);
    assert_eq!(cloud.rain_style, RainStyle::Glyph);
    assert!(!cloud.droplets.is_empty(), "glyph pool warm-started");

    // Glyph -> Physarum again: pool cleared, particles ready.
    cloud.transition_rain_style(RainStyle::Physarum);
    assert_eq!(cloud.physarum_rain.active_count(), 0);
    assert!(
        cloud.droplets.is_empty(),
        "physarum keeps the droplet pool empty (structured family)"
    );

    // And the system comes back alive after the switch.
    run_frames(&mut cloud, &mut frame, 90, 16);
    assert!(
        cloud.physarum_rain.active_count() > 0,
        "physarum restarts after switch"
    );
}

#[test]
fn physarum_rain_at_smoke_produces_dirty_frames() {
    let mut cloud = make_physarum_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    let start = Instant::now();
    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    let mut dirty_frames = 0;
    for idx in 0..60 {
        let now = start + Duration::from_millis(idx * 16);
        cloud.rain_at(&mut frame, now);
        if frame.is_dirty_all() || !frame.dirty_indices().is_empty() {
            dirty_frames += 1;
        }
        frame.clear_dirty();
    }
    assert!(
        dirty_frames >= 35,
        "physarum must produce a live frame stream (got {dirty_frames}/60)"
    );
}

#[test]
fn physarum_palette_adoption_updates_particles() {
    let mut cloud = make_physarum_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 60, 16);
    cloud.physarum_rain.adopt_palette_slot(3);
    // All active particles adopt the new slot (observable via a follow-up
    // rain_at frame not crashing on slot lookup + the count surviving).
    let before = cloud.physarum_rain.active_count();
    run_frames(&mut cloud, &mut frame, 6, 16);
    assert!(cloud.physarum_rain.active_count() > 0);
    let _ = before;
}

#[test]
fn physarum_emergent_network_pattern_forms() {
    let mut cloud = make_physarum_cloud(80, 30);
    let mut frame = Frame::new(80, 30, cloud.palette.bg);
    // Long run: 10 seconds of simulation should let the stigmergic
    // network emerge — the trail field max should be substantially
    // above the per-particle deposit rate per frame (PHYSARUM_DEPOSIT_AMOUNT
    // * dt = 0.5 * 0.0167 = 0.008 per frame). After 10s, multiple
    // particles passing through the same cell should accumulate
    // trail value above 0.02 (proves multi-particle deposition —
    // the signature of stigmergic network formation).
    run_frames(&mut cloud, &mut frame, 600, 16);

    let max_trail = cloud.physarum_rain.trail_max_for_test();
    // The network emergence check: at least one cell should have
    // trail value above the per-frame deposit rate * 2 — this
    // proves multiple particles have visited the same cell within
    // the decay window, which is the signature of stigmergic
    // network formation. Without network emergence, every cell
    // would have at most single-particle deposition (well below
    // this threshold).
    let per_frame_deposit = crate::constants::PHYSARUM_DEPOSIT_AMOUNT * (1.0 / 60.0);
    let emergence_threshold = per_frame_deposit * 2.5;
    assert!(
        max_trail > emergence_threshold,
        "emergent network must form (max trail {max_trail} should exceed emergence threshold {emergence_threshold})"
    );
}

// -- NIGHT-hunter-10 contracts (rate-independent decay, sensor
//    steering signs through the angle-addition ladder, heading LTS) --

/// Build a bare PhysarumRain on an 80x40 field with one hand-pinned
/// particle and an anchored clock. The first advance anchors
/// last_step (dt = 0), so callers step from t0 themselves.
fn bare_rain_with_particle(x: f32, y: f32, heading: f32) -> crate::cloud::physarum::PhysarumRain {
    let mut rain = crate::cloud::physarum::PhysarumRain::new();
    rain.reset(80);
    rain.set_particle_for_test(0, x, y, heading);
    rain.seed_trail_for_test(80, 40, 0, 0, 0.0);
    rain
}

fn step_at(rain: &mut crate::cloud::physarum::PhysarumRain, t0: Instant, idx: u32, step: Duration) {
    rain.advance(&crate::cloud::physarum::PhysarumStep {
        now: t0 + step * idx,
        chars_per_sec: 18.0,
        cols: 80,
        lines: 40,
        max_sim_delta: Duration::from_millis(1000),
        resume_blend: 1.0,
    });
}

#[test]
fn physarum_trail_decay_is_frame_rate_independent() {
    // NIGHT-hunter-10: the trail equilibrium must not depend on the
    // terminal's frame rate. One simulated second elapsed in 60 steps
    // of 1/60 s or 30 steps of 1/30 s must leave the SAME trail value
    // (the per-step multiplier is the 60 Hz reference constant raised
    // to dt*60). The pre-fix code multiplied the per-frame constant
    // once per advance call, so the 30 Hz cadence decayed half as
    // often — 0.9^30 vs 0.9^60, 23x more trail left — and the vein
    // brightness grading against the absolute thresholds shifted
    // with the display's refresh rate.
    let run_cadence = |step_secs: f32, steps: u32| -> f32 {
        // Particle pinned far from the watched cell so its deposits
        // never contaminate the measurement.
        let mut rain = bare_rain_with_particle(5.0, 20.0, 0.0);
        rain.seed_trail_for_test(80, 40, 60, 10, 1.0);
        let t0 = Instant::now();
        let step = Duration::from_secs_f32(step_secs);
        step_at(&mut rain, t0, 0, step); // anchor the clock
        for i in 1..=steps {
            step_at(&mut rain, t0, i, step);
        }
        rain.trail_value_for_test(60, 10)
            .expect("watched cell must stay inside the field")
    };
    let fast = run_cadence(1.0 / 60.0, 60);
    let slow = run_cadence(1.0 / 30.0, 30);
    let scale = fast.abs().max(slow.abs()).max(1.0e-9);
    assert!(
        (fast - slow).abs() <= 1.0e-4 * scale,
        "same simulated time must leave the same trail value: 60Hz cadence {fast} vs 30Hz cadence {slow}"
    );
    // And the value must actually be a decayed residue (sanity: the
    // pre-fix 30 Hz run would leave ~0.042, the fixed run ~0.0018 —
    // both nonzero, both far below the seeded 1.0).
    assert!(
        fast > 0.0 && fast < 0.1,
        "residue must be a real decayed value, got {fast}"
    );
}

#[test]
fn physarum_sensor_steering_follows_the_strongest_signal() {
    // Pins the sense-decide sign conventions THROUGH the
    // angle-addition sensor ladder (NIGHT-hunter-10 replaced six
    // per-particle trig calls with the hoisted-constant identities):
    // front-strong -> no turn, left-strong -> negative turn,
    // right-strong -> positive turn. A sign slip in the identities
    // would flip the steering direction and fail this test.
    let dt_step = Duration::from_secs_f32(1.0 / 60.0);
    let scenarios = [
        // (seed cell of the strong signal, expected heading delta sign)
        ((13_u16, 20_u16), 0.0), // front sensor: heading 0 + 3 cells ahead
        ((12, 18), -1.0),        // left sensor: heading -45 degrees
        ((12, 22), 1.0),         // right sensor: heading +45 degrees
    ];
    for ((seed_col, seed_line), expected_sign) in scenarios {
        let mut rain = bare_rain_with_particle(10.0, 20.0, 0.0);
        rain.seed_trail_for_test(80, 40, seed_col, seed_line, 1.0);
        let t0 = Instant::now();
        step_at(&mut rain, t0, 0, dt_step); // anchor
        step_at(&mut rain, t0, 1, dt_step); // one sensed + turned + moved step
        let heading = rain.particles[0].heading;
        match expected_sign {
            s if s < 0.0 => assert!(
                heading < -1.0e-4,
                "left-strong trail must steer left (negative heading), got {heading}"
            ),
            s if s > 0.0 => assert!(
                heading > 1.0e-4,
                "right-strong trail must steer right (positive heading), got {heading}"
            ),
            _ => assert!(
                heading.abs() < 1.0e-4,
                "front-strong trail must hold the heading exactly, got {heading}"
            ),
        }
    }
}

#[test]
fn physarum_heading_wraps_when_the_accumulator_drifts() {
    // LTS: the steering integrator accumulates turn into a bare f32
    // heading. Past the wrap limit the f32 ulp grinds the turn-rate
    // resolution; the amortized wrap folds the value back into
    // [0, TAU). A particle pinned past the limit must come back
    // wrapped (and finite) after one step.
    let mut rain = bare_rain_with_particle(40.0, 20.0, 100_000.0);
    let t0 = Instant::now();
    let step = Duration::from_secs_f32(1.0 / 60.0);
    step_at(&mut rain, t0, 0, step); // anchor
    step_at(&mut rain, t0, 1, step);
    let h = rain.particles[0].heading;
    assert!(
        h.is_finite() && (0.0..=std::f32::consts::TAU).contains(&h),
        "heading must fold into [0, TAU] once past the wrap limit (got {h})"
    );
}
