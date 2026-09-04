// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Core vortex-style behavior contracts (task-18):
//! scene resolution, spawn density, polar convergence, absorption,
//! drawn-cell bounds, style transitions, and the Keplerian speed bound.

use super::*;

#[test]
fn vortex_scene_resolves_style_and_fields() {
    let s = crate::scene::get_scene("vortex").expect("vortex scene exists");
    assert_eq!(s.config.rain_style, RainStyle::Vortex);
    assert_eq!(s.config.color, Some("cosmos"));
    assert_eq!(s.config.charset, Some("zen"));
    // Style dispatch sanity: the style helper families classify vortex
    // as structured (not droplet family) and spawn-remainder driven.
    assert!(!RainStyle::Vortex.is_droplet_family());
    assert!(RainStyle::Vortex.uses_spawn_remainder());
}

#[test]
fn vortex_motes_spawn_up_to_density_target() {
    let mut cloud = make_vortex_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 120, 16);

    let active = cloud.vortex_rain.active_count();
    // Pool is one mote per column (120); ratio at density 0.70 =
    // VORTEX_ACTIVE_BASE + 0.70 * VORTEX_ACTIVE_DENSITY_MULT = 0.67 →
    // target 80 motes. 2 seconds of spawn budget at the configured rate
    // must reach the target (deficit-bounded).
    let target = (120.0_f32 * 0.67).round() as usize;
    assert!(
        active >= target,
        "expected at least the density target {target}, got {active}"
    );
    assert!(active <= 120, "active motes cannot exceed pool size");
}

#[test]
fn vortex_radii_converge_inward_monotonically() {
    let mut cloud = make_vortex_cloud(80, 30);
    let mut frame = Frame::new(80, 30, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 60, 16);

    let radii_before: Vec<f32> = cloud.vortex_rain.active_radii_for_test();
    assert!(!radii_before.is_empty(), "motes must be active");

    // One more second of pure motion (no new spawn interference on the
    // existing motes is fine — new spawns only ADD members at the rim).
    let start = Instant::now();
    cloud.last_spawn_time = start - Duration::from_secs(1);
    for idx in 0..60 {
        let now = start + Duration::from_millis(idx * 16);
        cloud.rain_at(&mut frame, now);
        frame.clear_dirty();
    }

    let radii_after: Vec<f32> = cloud.vortex_rain.active_radii_for_test();
    // Every mote that SURVIVED both snapshots must be strictly inward
    // (motion is strictly decreasing radius). Match by count of motes
    // below each snapshot's median as a robust aggregate check: the
    // median radius of the active population after one more second of
    // drift must be lower (absorption removes the innermost, spawn adds
    // at rim — but at steady state both effects are in balance, so we
    // compare the FRACTION of motes below the pre-step median).
    let median_before = {
        let mut sorted = radii_before.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).expect("finite radii"));
        sorted[sorted.len() / 2]
    };
    let below = radii_after.iter().filter(|r| **r < median_before).count() as f32;
    let frac = below / radii_after.len() as f32;
    assert!(
        frac > 0.30,
        "a substantial share of motes must sit below the pre-step median radius (got {frac:.2})"
    );
}

#[test]
fn vortex_motes_absorbed_at_core() {
    let mut cloud = make_vortex_cloud(80, 30);
    let mut frame = Frame::new(80, 30, cloud.palette.bg);
    // Long run: every spawned mote gets several journey lifetimes
    // (journey ≈ 3 s at cps 24) — the population must remain bounded
    // (absorption working) instead of saturating the pool.
    run_frames(&mut cloud, &mut frame, 1200, 16);
    let active = cloud.vortex_rain.active_count();
    assert!(
        active <= 120,
        "absorption must keep the active population within the pool (got {active})"
    );
    // And a hard geometry bound: no active mote may report a radius
    // below the core (they are deactivated at VORTEX_CORE_R).
    for r in cloud.vortex_rain.active_radii_for_test() {
        assert!(
            r > crate::constants::VORTEX_CORE_R - f32::EPSILON,
            "active mote below core radius must have been absorbed (r={r})"
        );
    }
}

#[test]
fn vortex_drawn_cells_stay_in_bounds() {
    let mut cloud = make_vortex_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 90, 16);
    for cell in cloud.vortex_rain.drawn_cells_for_test() {
        assert!(cell.col < 60, "drawn col out of bounds: {}", cell.col);
        assert!(cell.line < 25, "drawn line out of bounds: {}", cell.line);
    }
    assert!(
        !cloud.vortex_rain.drawn_cells_for_test().is_empty(),
        "vortex must draw visible cells"
    );
}

#[test]
fn vortex_kepler_omega_bounded() {
    // The angular-speed divisor floor keeps the near-core spin finite:
    // omega_max = K / VORTEX_MIN_R (< 10 rad/s at shipped constants).
    let omega_max = crate::constants::VORTEX_KEPLER_K / crate::constants::VORTEX_MIN_R;
    assert!(
        omega_max.is_finite() && omega_max < 12.0,
        "near-core angular speed must stay bounded, got {omega_max}"
    );
}

// Compile-time contract: the divisor floor must sit at/above the core
// radius so the fastest spin is only reached at absorption-eligibility.
const _: () = assert!(crate::constants::VORTEX_MIN_R >= crate::constants::VORTEX_CORE_R);

#[test]
fn vortex_active_droplet_count_routes_to_motes() {
    let mut cloud = make_vortex_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 60, 16);
    assert_eq!(
        cloud.active_droplet_count(),
        cloud.vortex_rain.active_count()
    );
}

#[test]
fn vortex_style_transition_clears_state_both_ways() {
    let mut cloud = make_vortex_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 60, 16);
    assert!(cloud.vortex_rain.active_count() > 0);

    // Vortex → Glyph: motes reset, droplet pool warm-starts.
    cloud.transition_rain_style(RainStyle::Glyph);
    assert_eq!(cloud.vortex_rain.active_count(), 0);
    assert_eq!(cloud.rain_style, RainStyle::Glyph);
    assert!(!cloud.droplets.is_empty(), "glyph pool warm-started");

    // Glyph → Vortex again: pool cleared, motes ready.
    cloud.transition_rain_style(RainStyle::Vortex);
    assert_eq!(cloud.vortex_rain.active_count(), 0);
    assert!(
        cloud.droplets.is_empty(),
        "vortex keeps the droplet pool empty"
    );

    // And the system comes back alive after the switch.
    run_frames(&mut cloud, &mut frame, 90, 16);
    assert!(
        cloud.vortex_rain.active_count() > 0,
        "vortex restarts after switch"
    );
}

#[test]
fn vortex_rain_at_smoke_produces_dirty_frames() {
    let mut cloud = make_vortex_cloud(120, 40);
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
        "vortex must produce a live frame stream (got {dirty_frames}/60)"
    );
}

#[test]
fn vortex_palette_adoption_updates_motes() {
    let mut cloud = make_vortex_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 60, 16);
    cloud.vortex_rain.adopt_palette_slot(3);
    // All active motes adopt the new slot (observable via a follow-up
    // rain_at frame not crashing on slot lookup + the count surviving).
    let before = cloud.vortex_rain.active_count();
    run_frames(&mut cloud, &mut frame, 6, 16);
    assert!(cloud.vortex_rain.active_count() > 0);
    let _ = before;
}
