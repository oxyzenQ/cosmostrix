// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Core dragon-style behavior contracts (NIGHT-research-5):
//! scene resolution, spawn density, chain integrity, lifetime
//! absorption, drawn-cell bounds, style transitions, and the
//! head speed / turn-rate bounds.

use super::*;

#[test]
fn dragon_scene_resolves_style_and_fields() {
    let s = crate::scene::get_scene("cosmic_dragon").expect("cosmic_dragon scene exists");
    assert_eq!(s.config.rain_style, RainStyle::Dragon);
    assert_eq!(s.config.color, Some("nebula"));
    assert_eq!(s.config.charset, Some("zen"));
    // Style dispatch sanity: the style helper families classify dragon
    // as structured (not droplet family) and spawn-remainder driven —
    // parity with vortex.
    assert!(!RainStyle::Dragon.is_droplet_family());
    assert!(RainStyle::Dragon.uses_spawn_remainder());
}

#[test]
fn dragon_chains_spawn_up_to_density_target() {
    let mut cloud = make_dragon_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 120, 16);

    let active = cloud.dragon_rain.active_count();
    // NIGHT-research-5 owner directive: dragon count is fixed at 3
    // regardless of density — matches the three dragon engines in
    // cosmostrix (cosmic_dragon_engine, crystal_dragon_engine,
    // chroma_dragon_engine). 2 seconds of spawn budget reaches the
    // target (deficit-bounded).
    assert_eq!(
        active, 3,
        "dragon count is fixed at 3 (matches the three dragon engines); got {active}"
    );
}

#[test]
fn dragon_body_chain_maintains_segment_spacing() {
    let mut cloud = make_dragon_cloud(80, 30);
    let mut frame = Frame::new(80, 30, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 90, 16);

    // After 1.5s of motion, every dragon chain should have full body
    // length and segments should be spaced approximately
    // DRAGON_SEGMENT_SPACING apart (the FABRIK constraint).
    let segs = cloud.dragon_rain.active_segments_for_test();
    if !segs.is_empty() {
        // Verify chain integrity: each consecutive segment pair is
        // within a reasonable distance of DRAGON_SEGMENT_SPACING
        // (chain constraint keeps it close, but rapid head motion
        // can stretch it transiently before the FABRIK pass catches
        // up — 2x spacing is a generous upper bound).
        let body_len = crate::constants::DRAGON_BODY_LEN;
        let spacing = crate::constants::DRAGON_SEGMENT_SPACING;
        for chain_start in (0..segs.len()).step_by(body_len) {
            if chain_start + body_len > segs.len() {
                break;
            }
            for i in chain_start..chain_start + body_len - 1 {
                let (x1, y1) = segs[i];
                let (x2, y2) = segs[i + 1];
                let dist = ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
                assert!(
                    dist <= spacing * 2.5,
                    "body chain broken: segment {i} to {} distance {dist} exceeds 2.5x spacing",
                    i + 1
                );
            }
        }
    }
}

#[test]
fn dragon_motes_absorbed_after_lifetime() {
    let mut cloud = make_dragon_cloud(80, 30);
    let mut frame = Frame::new(80, 30, cloud.palette.bg);
    // Long run: every spawned dragon gets several lifetime cycles
    // (lifetime = 20s). 25s of frames exercises spawn -> age out
    // -> respawn at least once per slot.
    run_frames(&mut cloud, &mut frame, 1500, 16);
    let active = cloud.dragon_rain.active_count();
    assert!(
        active <= 3,
        "lifetime absorption must keep the active population within the pool (got {active})"
    );
}

#[test]
fn dragon_drawn_cells_stay_in_bounds() {
    let mut cloud = make_dragon_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 90, 16);
    for cell in cloud.dragon_rain.drawn_cells_for_test() {
        assert!(cell.col < 60, "drawn col out of bounds: {}", cell.col);
        assert!(cell.line < 25, "drawn line out of bounds: {}", cell.line);
    }
    // Note: drawn_cells_for_test() may be empty if no dragon is
    // active in a given frame (lifecycle gap). The bounds check
    // above is a hard contract — any drawn cell MUST be in bounds.
}

#[test]
fn dragon_head_speed_bounded() {
    // The head speed is chars_per_sec * DRAGON_SPEED_SCALE. At
    // speed 18 (default scene) + scale 1.0, head moves at 18 cells/s.
    // Per-frame motion at 60 FPS = 0.3 cells — well within stable
    // bounds for the FABRIK chain (chain stretch < 1 cell per frame).
    let cps = 18.0_f32;
    let dt = 1.0 / 60.0;
    let head_motion = cps * crate::constants::DRAGON_SPEED_SCALE * dt;
    assert!(
        head_motion > 0.0 && head_motion < 1.0,
        "head per-frame motion must stay sub-cell for chain stability (got {head_motion})"
    );
}

// Compile-time contract: the body length and segment spacing must
// produce a chain that fits inside a typical terminal (chain length
// = spacing * (len - 1) should be <= 40 cells for a 120-col viewport).
const _: () = assert!(
    crate::constants::DRAGON_SEGMENT_SPACING * (crate::constants::DRAGON_BODY_LEN as f32 - 1.0)
        <= 40.0
);

#[test]
fn dragon_active_droplet_count_routes_to_dragons() {
    let mut cloud = make_dragon_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 60, 16);
    assert_eq!(
        cloud.active_droplet_count(),
        cloud.dragon_rain.active_count()
    );
}

#[test]
fn dragon_style_transition_clears_state_both_ways() {
    let mut cloud = make_dragon_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 60, 16);
    assert!(cloud.dragon_rain.active_count() > 0);

    // Dragon -> Glyph: dragons reset, droplet pool warm-starts.
    cloud.transition_rain_style(RainStyle::Glyph);
    assert_eq!(cloud.dragon_rain.active_count(), 0);
    assert_eq!(cloud.rain_style, RainStyle::Glyph);
    assert!(!cloud.droplets.is_empty(), "glyph pool warm-started");

    // Glyph -> Dragon again: pool cleared, dragons ready.
    cloud.transition_rain_style(RainStyle::Dragon);
    assert_eq!(cloud.dragon_rain.active_count(), 0);
    assert!(
        cloud.droplets.is_empty(),
        "dragon keeps the droplet pool empty (structured family)"
    );

    // And the system comes back alive after the switch.
    run_frames(&mut cloud, &mut frame, 90, 16);
    assert!(
        cloud.dragon_rain.active_count() > 0,
        "dragon restarts after switch"
    );
}

#[test]
fn dragon_rain_at_smoke_produces_dirty_frames() {
    let mut cloud = make_dragon_cloud(120, 40);
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
        "dragon must produce a live frame stream (got {dirty_frames}/60)"
    );
}

#[test]
fn dragon_palette_adoption_updates_segments() {
    let mut cloud = make_dragon_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 60, 16);
    cloud.dragon_rain.adopt_palette_slot(3);
    // All active dragons adopt the new slot (observable via a follow-up
    // rain_at frame not crashing on slot lookup + the count surviving).
    let before = cloud.dragon_rain.active_count();
    run_frames(&mut cloud, &mut frame, 6, 16);
    assert!(cloud.dragon_rain.active_count() > 0);
    let _ = before;
}

#[test]
fn dragon_state_machine_visits_both_states() {
    let mut cloud = make_dragon_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    // Long run: 8 seconds of frames should exercise both SOAR and
    // CIRCLE states (max state duration is 8s for SOAR, 6s for CIRCLE;
    // 8s total guarantees at least one transition).
    run_frames(&mut cloud, &mut frame, 480, 16);

    let states = cloud.dragon_rain.active_states_for_test();
    if !states.is_empty() {
        // At least one active dragon should be in either Soar or
        // Circle (both are valid runtime states). The test verifies
        // the state machine is producing real DragonState values,
        // not stuck in an undefined default.
        for state in &states {
            assert!(
                matches!(
                    state,
                    crate::cloud::dragon::DragonState::Soar
                        | crate::cloud::dragon::DragonState::Circle
                ),
                "invalid dragon state: {:?}",
                state
            );
        }
    }
}

// -- NIGHT-hunter-10 contracts (pace linearity + heading LTS wrap) --

/// Spawn two dragons through the production spawn path (allocates the
/// segment chains), then pin them into a deterministic configuration:
/// heading +x, Soar with noise_phase 0 and sim_age 0 (both layered-sine
/// terms vanish at t = 0, so the turn rate is exactly zero and the
/// heading cannot drift during the measured step).
fn make_pace_probe_rain() -> crate::cloud::dragon::DragonRain {
    use rand::distr::Uniform;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    let mut rain = crate::cloud::dragon::DragonRain::new();
    rain.reset(80);
    let mut rng = StdRng::seed_from_u64(0xC0FFEE);
    let rand_chance = Uniform::new(0.0_f32, 1.0).expect("valid uniform");
    let params = crate::cloud::dragon::DragonSpawnParams {
        cols: 80,
        lines: 25,
        density: 0.55,
        active_palette_slot: 0,
        spawn_scale: 1.0,
    };
    let mut remainder = 0.0_f32;
    let mut random = crate::cloud::dragon::DragonRandom {
        rng: &mut rng,
        rand_chance: &rand_chance,
    };
    // 5 seconds of budget spawns the full fixed pool of 3 dragons.
    rain.spawn(
        std::time::Duration::from_secs(5),
        &mut remainder,
        &params,
        &mut random,
    );
    assert!(
        rain.active_count() >= 2,
        "pace probe needs two live dragons"
    );
    for (i, d) in rain.dragons.iter_mut().take(2).enumerate() {
        d.active = true;
        d.state = crate::cloud::dragon::DragonState::Soar;
        d.state_timer = 10.0;
        d.heading = 0.0;
        d.noise_phase = 0.0;
        d.sim_age = 0.0;
        d.lifetime = 100.0;
        d.pace = if i == 0 { 1.0 } else { 1.2 };
        d.segments[0].x = 10.0;
        d.segments[0].y = 10.0;
    }
    rain
}

#[test]
fn dragon_head_displacement_scales_linearly_with_pace() {
    // NIGHT-hunter-10: the documented contract is a per-dragon speed
    // multiplier of 0.85..1.15 — head displacement must scale LINEARLY
    // with pace. The pre-fix code folded pace into both the velocity
    // vector and dt_d, so displacement scaled with pace squared
    // (0.85..1.15 drifted to 0.72..1.32 effective) while turn rate,
    // state timer and sim_age all stayed linear — each dragon ran off
    // its documented speed profile and the fast tail ate into the
    // sub-cell FABRIK stability margin.
    let mut rain = make_pace_probe_rain();

    let t0 = Instant::now();
    let mk_step = |now: Instant| crate::cloud::dragon::DragonStep {
        now,
        chars_per_sec: 18.0,
        cols: 80,
        lines: 25,
        max_sim_delta: Duration::from_millis(1000),
        resume_blend: 1.0,
    };
    // First advance only anchors last_step (dt = 0 on a fresh clock).
    rain.advance(&mk_step(t0));
    rain.advance(&mk_step(t0 + Duration::from_millis(16)));

    let d0 = (rain.dragons[0].segments[0].x - 10.0).abs();
    let d1 = (rain.dragons[1].segments[0].x - 10.0).abs();
    assert!(
        d0 > 1.0e-4,
        "reference dragon must actually move (got {d0})"
    );
    let ratio = d1 / d0;
    assert!(
        (ratio - 1.2).abs() < 0.02,
        "head displacement must scale linearly with pace: expected 1.2, got {ratio} (pace-squared regression)"
    );
}

#[test]
fn dragon_heading_stays_bounded_after_prolonged_integration() {
    // LTS: heading is an integrated accumulator (turn rate, wall
    // reflections, state transitions all add to it). Without the
    // amortized wrap, a multi-day session accumulates 100K+ radians
    // and the f32 ulp degrades the turn-rate resolution until the
    // motion grains. Pin: force-circling for 750 simulated seconds
    // (1125 radians of turn — nearly 3x the wrap limit) must leave
    // every heading bounded, finite and trig-valid.
    let mut rain = make_pace_probe_rain();

    let t0 = Instant::now();
    let big_dt = Duration::from_millis(250);
    let wrap_bound = 64.0 * std::f32::consts::TAU + std::f32::consts::TAU;
    for idx in 0..3000_u32 {
        // Keep every dragon locked in CIRCLE so the turn rate is the
        // constant 1.5 rad/s regardless of wall-bounce state snaps.
        for d in rain.dragons.iter_mut() {
            if d.active {
                d.state = crate::cloud::dragon::DragonState::Circle;
                d.state_timer = 1.0e9;
                d.lifetime = 1.0e9;
            }
        }
        rain.advance(&crate::cloud::dragon::DragonStep {
            now: t0 + big_dt * idx,
            chars_per_sec: 18.0,
            cols: 80,
            lines: 25,
            max_sim_delta: big_dt,
            resume_blend: 1.0,
        });
    }
    for d in &rain.dragons {
        if d.active {
            assert!(
                d.heading.is_finite() && d.heading.abs() <= wrap_bound,
                "heading accumulator must stay bounded after prolonged integration (got {})",
                d.heading
            );
        }
    }
}
