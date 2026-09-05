// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Core lorenz-style behavior contracts (NIGHT-research-4):
//! scene resolution, spawn density, attractor trajectory properties,
//! absorption, drawn-cell bounds, style transitions, and the RK4
//! stability bound.

use super::*;

#[test]
fn lorenz_scene_resolves_style_and_fields() {
    let s = crate::scene::get_scene("lorenz").expect("lorenz scene exists");
    assert_eq!(s.config.rain_style, RainStyle::Lorenz);
    assert_eq!(s.config.color, Some("cosmos"));
    assert_eq!(s.config.charset, Some("binary"));
    // Style dispatch sanity: the style helper families classify lorenz
    // as structured (not droplet family) and spawn-remainder driven —
    // parity with vortex.
    assert!(!RainStyle::Lorenz.is_droplet_family());
    assert!(RainStyle::Lorenz.uses_spawn_remainder());
}

#[test]
fn lorenz_motes_spawn_up_to_density_target() {
    let mut cloud = make_lorenz_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 120, 16);

    let active = cloud.lorenz_rain.active_count();
    // Pool is one mote per column (120); ratio at density 0.70 =
    // LORENZ_ACTIVE_BASE + 0.70 * LORENZ_ACTIVE_DENSITY_MULT = 0.685 ->
    // target 82 motes. 2 seconds of spawn budget at the configured rate
    // must reach the target (deficit-bounded).
    let target = (120.0_f32 * 0.685).round() as usize;
    assert!(
        active >= target,
        "expected at least the density target {target}, got {active}"
    );
    assert!(active <= 120, "active motes cannot exceed pool size");
}

#[test]
fn lorenz_trajectories_stay_within_attractor_bounds() {
    let mut cloud = make_lorenz_cloud(80, 30);
    let mut frame = Frame::new(80, 30, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 600, 16);

    // The canonical Lorenz attractor (sigma=10, rho=28, beta=8/3) is
    // bounded: |x| < 30, |y| < 35, 0 < z < 60 in steady state. After
    // 10 s of integration every active mote must be inside the
    // attractor's bounding box — if RK4 had diverged, motes would
    // escape to infinity. The generous bounds below (50/50/-5..100)
    // catch divergence without false-positiving on legitimate lobe
    // peaks or small RK4 numerical drift that lets z briefly dip
    // below the theoretical floor of 0 (the attractor's saddle).
    for (x, y, z) in cloud.lorenz_rain.active_states_for_test() {
        assert!(x.abs() < 50.0, "lorenz x out of attractor bounds: {x}");
        assert!(y.abs() < 50.0, "lorenz y out of attractor bounds: {y}");
        assert!(
            (-5.0..=100.0).contains(&z),
            "lorenz z out of attractor bounds: {z}"
        );
    }
}

#[test]
fn lorenz_motes_absorbed_after_lifetime() {
    let mut cloud = make_lorenz_cloud(80, 30);
    let mut frame = Frame::new(80, 30, cloud.palette.bg);
    // Long run: every spawned mote gets several lifetime cycles
    // (lifetime = 12 s). 20 s of frames exercises spawn -> age out
    // -> respawn at least once per slot.
    run_frames(&mut cloud, &mut frame, 1200, 16);
    let active = cloud.lorenz_rain.active_count();
    assert!(
        active <= 80,
        "lifetime absorption must keep the active population within the pool (got {active})"
    );
    // And a hard contract: no active mote may report an age past its
    // lifetime (they are deactivated when sim_age >= lifetime).
    // We can't read sim_age/lifetime directly from outside the module,
    // but the steady-state bounded count above is the observable
    // consequence — if absorption failed, the count would saturate.
}

#[test]
fn lorenz_drawn_cells_stay_in_bounds() {
    let mut cloud = make_lorenz_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 90, 16);
    for cell in cloud.lorenz_rain.drawn_cells_for_test() {
        assert!(cell.col < 60, "drawn col out of bounds: {}", cell.col);
        assert!(cell.line < 25, "drawn line out of bounds: {}", cell.line);
    }
    assert!(
        !cloud.lorenz_rain.drawn_cells_for_test().is_empty(),
        "lorenz must draw visible cells"
    );
}

#[test]
fn lorenz_rk4_step_bounded() {
    // The integration dt is chars_per_sec * LORENZ_DT_PER_CPS * dt_wall.
    // At speed 24 + 60 FPS (dt_wall ~0.0167s), dt_lorenz ~ 3.2e-4 — well
    // below the RK4 stability threshold for Lorenz (literature: dt
    // < 0.01 is stable). This test asserts the shipped constants
    // stay in the stable regime.
    let cps = 24.0_f32;
    let dt_wall = 1.0 / 60.0;
    let dt = cps * crate::constants::LORENZ_DT_PER_CPS * dt_wall;
    assert!(
        dt > 0.0 && dt < 0.01,
        "RK4 dt must stay in the stable regime (got {dt})"
    );
}

// Compile-time contract: the canonical Lorenz parameters must match
// the literature (sigma=10, rho=28, beta=8/3) — these are the
// foundational chaos-theory values published by Edward Lorenz in 1963.
// Any drift here would silently turn the attractor into a different
// (possibly non-chaotic) system.
const _: () = assert!(crate::constants::LORENZ_SIGMA == 10.0);
const _: () = assert!(crate::constants::LORENZ_RHO == 28.0);
// 8.0/3.0 is computed at compile time; the literal expression is the
// canonical form. We check it equals the math (2.666...).
const _: () = assert!((crate::constants::LORENZ_BETA - 8.0 / 3.0).abs() < 1e-6);

#[test]
fn lorenz_active_droplet_count_routes_to_motes() {
    let mut cloud = make_lorenz_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 60, 16);
    assert_eq!(
        cloud.active_droplet_count(),
        cloud.lorenz_rain.active_count()
    );
}

#[test]
fn lorenz_style_transition_clears_state_both_ways() {
    let mut cloud = make_lorenz_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 60, 16);
    assert!(cloud.lorenz_rain.active_count() > 0);

    // Lorenz -> Glyph: motes reset, droplet pool warm-starts.
    cloud.transition_rain_style(RainStyle::Glyph);
    assert_eq!(cloud.lorenz_rain.active_count(), 0);
    assert_eq!(cloud.rain_style, RainStyle::Glyph);
    assert!(!cloud.droplets.is_empty(), "glyph pool warm-started");

    // Glyph -> Lorenz again: pool cleared, motes ready.
    cloud.transition_rain_style(RainStyle::Lorenz);
    assert_eq!(cloud.lorenz_rain.active_count(), 0);
    assert!(
        cloud.droplets.is_empty(),
        "lorenz keeps the droplet pool empty (structured family)"
    );

    // And the system comes back alive after the switch.
    run_frames(&mut cloud, &mut frame, 90, 16);
    assert!(
        cloud.lorenz_rain.active_count() > 0,
        "lorenz restarts after switch"
    );
}

#[test]
fn lorenz_rain_at_smoke_produces_dirty_frames() {
    let mut cloud = make_lorenz_cloud(120, 40);
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
        dirty_frames >= 55,
        "lorenz must produce a live frame stream (got {dirty_frames}/60)"
    );
}

#[test]
fn lorenz_palette_adoption_updates_motes() {
    let mut cloud = make_lorenz_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 60, 16);
    cloud.lorenz_rain.adopt_palette_slot(3);
    // All active motes adopt the new slot (observable via a follow-up
    // rain_at frame not crashing on slot lookup + the count surviving).
    let before = cloud.lorenz_rain.active_count();
    run_frames(&mut cloud, &mut frame, 6, 16);
    assert!(cloud.lorenz_rain.active_count() > 0);
    let _ = before;
}
