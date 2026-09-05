// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Core flux-style behavior contracts (task-19): scene resolution,
//! family classification, fixed-step determinism, speed scaling,
//! recycling, draw bounds, frame-stream liveness, and style
//! transitions.

use super::*;

#[test]
fn flux_scene_resolves_style_and_fields() {
    let s = crate::scene::get_scene("flux").expect("flux scene exists");
    assert_eq!(s.config.rain_style, RainStyle::Flux);
    assert_eq!(s.config.color, Some("ocean"));
    assert_eq!(s.config.charset, Some("minimal"));
    // Flux is structured-family: own pool + drawn-cell diff cleanup,
    // spawn-remainder driven like monolith and vortex.
    assert!(!RainStyle::Flux.is_droplet_family());
    assert!(RainStyle::Flux.uses_spawn_remainder());
}

#[test]
fn flux_scene_order_position() {
    use crate::scene::SCENE_ORDER;
    // Style flagships follow the core trio (owner-pinned order):
    // position 4 = vortex, position 5 = flux, position 6 = lorenz
    // (NIGHT-research-4 merge added lorenz after flux).
    assert_eq!(SCENE_ORDER[3], "vortex");
    assert_eq!(SCENE_ORDER[4], "flux");
    assert_eq!(crate::scene::cycle_scene("vortex", 1), "flux");
    assert_eq!(crate::scene::cycle_scene("flux", 1), "lorenz");
}

#[test]
fn flux_spawn_reaches_density_target() {
    let mut cloud = make_flux_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    // Production semantics: per-frame spawn budget is clamped by the
    // sim cap, so the pool ramps gradually (no artificial warm-start
    // 1 s backlog). 2 s in: at least 60% of the target; 5 s total:
    // the full density target (deficit-bounded steady state).
    run_frames(&mut cloud, &mut frame, 120, 17);
    let lanes = cloud.cols as usize;
    let target = crate::cloud::FluxRain::target_for_test(lanes, 0.70);
    let mid = cloud.flux_rain.active_count();
    assert!(
        mid >= target * 3 / 5,
        "2 s ramp must reach 60% of the target ({mid}/{target})"
    );
    run_frames(&mut cloud, &mut frame, 180, 17);
    let active = cloud.flux_rain.active_count();
    assert!(
        active >= target.saturating_sub(1) && active <= target,
        "steady state must hold the density target ({active}/{target})"
    );
}

#[test]
fn flux_fixed_step_determinism() {
    let mut cloud = make_flux_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    // Inject motes directly so the pool is active from frame 0 (the
    // spawn ramp would otherwise leave advance idle on early frames).
    for i in 0..5_usize {
        cloud
            .flux_rain
            .set_mote_for_test(i, 10.0 + i as f32, 6.0, 0.0, 0.0);
    }
    // 17 ms > FLUX_SIM_DT (16.67 ms): every frame after the first
    // consumes exactly one solver step (the first frame only arms
    // the clock — dt is zero with no prior last_step).
    run_frames(&mut cloud, &mut frame, 5, 17);
    assert_eq!(
        cloud.flux_rain.sim_steps_for_test(),
        4,
        "one solver step per frame after the first (4 steps / 5 frames)"
    );

    // Backlog drop: a single 500 ms frame with a wide sim cap (an
    // uncapped/stalled terminal) must NOT integrate 30 steps — the
    // per-frame cap bounds it and the residual is dropped.
    let before = cloud.flux_rain.sim_steps_for_test();
    let start = Instant::now();
    cloud.last_spawn_time = start;
    cloud.last_phosphor_time = start;
    cloud.set_max_sim_delta(Duration::from_millis(500));
    let now = start + Duration::from_millis(500);
    cloud.rain_at(&mut frame, now);
    frame.clear_dirty();
    let stepped = cloud.flux_rain.sim_steps_for_test() - before;
    assert_eq!(
        stepped,
        u64::from(crate::constants::FLUX_MAX_STEPS_PER_FRAME),
        "a 500 ms stall integrates at most the per-frame cap"
    );
}

#[test]
fn flux_gravity_scales_with_speed_keys() {
    // The up/down speed keys scale gravity (clamped 0.25..3.0): a
    // 3x chars_per_sec run must produce measurably faster downward
    // motion over the same simulated window.
    let (slow_vy, fast_vy) = {
        let mut cloud = make_flux_cloud(60, 25);
        let mut frame = Frame::new(60, 25, cloud.palette.bg);
        run_frames(&mut cloud, &mut frame, 30, 17);
        let speeds = cloud.flux_rain.active_speeds_for_test();
        let mean = speeds.iter().map(|v| v.1.abs()).sum::<f32>() / speeds.len().max(1) as f32;
        cloud.set_chars_per_sec(54.0);
        run_frames(&mut cloud, &mut frame, 30, 17);
        let speeds2 = cloud.flux_rain.active_speeds_for_test();
        let mean2 = speeds2.iter().map(|v| v.1.abs()).sum::<f32>() / speeds2.len().max(1) as f32;
        (mean, mean2)
    };
    assert!(
        fast_vy > slow_vy,
        "3x speed must energize the fluid ({fast_vy} vs {slow_vy})"
    );
}

#[test]
fn flux_motes_fall_downward() {
    let mut cloud = make_flux_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 60, 17);
    let speeds = cloud.flux_rain.active_speeds_for_test();
    assert!(!speeds.is_empty(), "pool must be active after 1 s");
    let down = speeds.iter().filter(|v| v.1 > 0.0).count();
    assert!(
        down >= speeds.len() / 2,
        "gravity-dominated fluid: most motes move downward ({down}/{}) — lateral eddy drift is allowed, net sinking is required",
        speeds.len()
    );
}

#[test]
fn flux_motes_enter_from_above_and_descend() {
    let mut cloud = make_flux_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 8, 17);
    // Spawn positions sit just above the top edge (clamped rim).
    let early = cloud.flux_rain.active_positions_for_test();
    assert!(!early.is_empty());
    assert!(
        early.iter().all(|&(_, y)| (-1.5..=13.5).contains(&y)),
        "positions stay inside the viewport + rim band"
    );
    // After 2+ more seconds the population mean sits well inside the
    // screen (spawned above, gravity pulls through).
    run_frames(&mut cloud, &mut frame, 120, 17);
    let later = cloud.flux_rain.active_positions_for_test();
    let mean_y = later.iter().map(|p| p.1).sum::<f32>() / later.len().max(1) as f32;
    assert!(
        mean_y > 0.0,
        "population must descend into the screen (mean y {mean_y})"
    );
}

#[test]
fn flux_bottom_exit_recycles_motes() {
    let mut cloud = make_flux_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 10, 17);
    assert!(cloud.flux_rain.active_count() > 0);

    // Drop mote 0 past the bottom boundary (y_max = 25/2 = 12.5
    // screen units): the next solver step must retire that exact
    // mote (index-precise — immune to concurrent spawn/despawn).
    cloud.flux_rain.set_mote_for_test(0, 30.0, 20.0, 0.0, 0.0);
    assert!(cloud.flux_rain.motes[0].active);
    // Leap the synthetic clock past the run_frames clock (which runs
    // ahead of wall time — 10 synthetic frames execute in ~1 ms) so
    // the dt into the accumulator is the full 17 ms.
    let start = Instant::now() + Duration::from_secs(5);
    cloud.last_spawn_time = start;
    cloud.set_max_sim_delta(Duration::from_millis(17));
    let now = start + Duration::from_millis(17);
    cloud.rain_at(&mut frame, now);
    frame.clear_dirty();
    assert!(
        !cloud.flux_rain.motes[0].active,
        "a mote past the bottom boundary must recycle"
    );
}

#[test]
fn flux_drawn_cells_within_bounds() {
    let mut cloud = make_flux_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 120, 16);
    for cell in cloud.flux_rain.drawn_cells_for_test() {
        assert!(
            cell.col < 60 && cell.line < 25,
            "drawn cell ({}, {}) out of bounds",
            cell.col,
            cell.line
        );
    }
}

#[test]
fn flux_integration_keeps_frame_live() {
    let mut cloud = make_flux_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 180, 16);
    let start = Instant::now();
    let mut dirty = 0;
    for idx in 0..30 {
        let now = start + Duration::from_millis(idx * 16);
        cloud.rain_at(&mut frame, now);
        if frame.is_dirty_all() || !frame.dirty_indices().is_empty() {
            dirty += 1;
        }
        frame.clear_dirty();
    }
    assert!(
        dirty >= 25,
        "flux must keep the frame stream live ({dirty}/30)"
    );
}

#[test]
fn flux_style_transition_resets_pool() {
    let mut cloud = make_flux_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 120, 16);
    assert!(cloud.flux_rain.active_count() > 0);

    // Flux → Glyph: the pool must go fully quiet; the droplet pool
    // warm-starts so the first post-switch frame has rain.
    cloud.transition_rain_style(RainStyle::Glyph);
    assert_eq!(cloud.flux_rain.active_count(), 0);
    assert!(!cloud.droplets.is_empty());

    // Glyph → Flux: fresh field, rain resumes.
    cloud.transition_rain_style(RainStyle::Flux);
    run_frames(&mut cloud, &mut frame, 120, 16);
    assert!(
        cloud.flux_rain.active_count() > 0,
        "flux re-arms after a style round-trip"
    );
}

#[test]
fn flux_active_droplet_count_routes_to_motes() {
    let mut cloud = make_flux_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 60, 16);
    let expected = cloud.flux_rain.active_count();
    assert_eq!(cloud.active_droplet_count(), expected);
}

// Compile-time pins: the fixed-step cadence contract and the solver
// iteration count stay positive and self-consistent.
const _: () = assert!(crate::constants::FLUX_SIM_DT > 0.0);
const _: () = assert!(crate::constants::FLUX_MAX_STEPS_PER_FRAME >= 1);
const _: () = assert!(crate::constants::FLUX_JACOBI_ITERATIONS >= 1);
const _: () = assert!(crate::constants::FLUX_GRAVITY > 0.0);
const _: () = assert!(crate::constants::FLUX_BRIGHT_HOT > crate::constants::FLUX_BRIGHT_MID);
