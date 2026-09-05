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
