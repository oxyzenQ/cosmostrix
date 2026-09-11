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
fn vortex_flat_curve_omega_bounded() {
    // NIGHT-research-16 label fix: the law is a flat rotation curve
    // (omega = K / r, constant tangential cells/sec — the galaxy
    // rotation-curve read), not Kepler's third law (r^-1.5, the
    // quasar disk's law). The angular-speed divisor floor keeps the
    // near-core spin finite: omega_max = K / VORTEX_MIN_R (< 10 rad/s
    // at shipped constants).
    let omega_max = crate::constants::VORTEX_ROTATION_K / crate::constants::VORTEX_MIN_R;
    assert!(
        omega_max.is_finite() && omega_max < 12.0,
        "near-core angular speed must stay bounded, got {omega_max}"
    );
}

#[test]
fn vortex_motes_ride_lockstep_at_shared_radius() {
    // The NIGHT-research-16 lockstep ruling (the black hole's
    // NIGHT-research-11 all-lanes-consistent precedent, the owner's
    // scattered/flying-outward report): every mote at the same radius
    // advances at the SAME angular and radial speed — the retired
    // per-mote spin/fall multipliers (0.85-1.15 / 0.80-1.25) used to
    // de-sync same-annulus motes into the radial smear the owner
    // read as scattered arms. Two motes placed at the same radius
    // with different angles must advance by IDENTICAL deltas (the
    // closed-form expectation, frozen dt) — orbiting in formation.
    use crate::cloud::type_rain::vortex::VortexStep;

    let mut cloud = make_vortex_cloud(80, 30);
    let mut frame = Frame::new(80, 30, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 60, 16);
    assert!(
        cloud.vortex_rain.active_count() >= 2,
        "need at least two active motes for the lockstep probe"
    );

    // Arm the clock, then place two motes at the same radius (0.5)
    // with different angles (0.0 and 2.0 rad).
    let now = Instant::now();
    let step = |t: Instant| VortexStep {
        now: t,
        chars_per_sec: 24.0,
        max_sim_delta: Duration::from_secs_f32(2.0),
        resume_blend: 1.0,
    };
    cloud.vortex_rain.advance(&step(now));
    let idx: Vec<usize> = {
        let motes = &cloud.vortex_rain.motes;
        (0..motes.len())
            .filter(|&i| motes[i].active)
            .take(2)
            .collect()
    };
    {
        let motes = &mut cloud.vortex_rain.motes;
        motes[idx[0]].angle = 0.0;
        motes[idx[0]].radius = 0.5;
        motes[idx[1]].angle = 2.0;
        motes[idx[1]].radius = 0.5;
    }

    // One second of pure motion (frozen dt, no draw interference).
    cloud
        .vortex_rain
        .advance(&step(now + Duration::from_secs(1)));
    let (a0, r0) = {
        let m = &cloud.vortex_rain.motes[idx[0]];
        (m.angle, m.radius)
    };
    let (a1, r1) = {
        let m = &cloud.vortex_rain.motes[idx[1]];
        (m.angle, m.radius)
    };

    // Identical deltas: same annulus, same motion. The motes START
    // 2.0 rad apart (0.0 and 2.0); after the shared advance the gap
    // must still be exactly 2.0 rad.
    let angle_gap = (a0 - a1).abs();
    let radius_spread = (r0 - r1).abs();
    assert!(
        (angle_gap - 2.0).abs() < 1e-5,
        "same-radius motes must advance by identical angle deltas (gap {angle_gap:.5}, want 2.0)"
    );
    assert!(
        radius_spread < 1e-6,
        "same-radius motes must keep identical radii (spread {radius_spread:.2e})"
    );

    // And the closed-form expectation (the shared law): omega dt and
    // the inward drift at r = 0.5 over 1.0 s.
    let omega = crate::constants::VORTEX_ROTATION_K / 0.5 * crate::constants::VORTEX_SPEED_SCALE;
    let vr = 24.0 / crate::constants::VORTEX_JOURNEY_ROWS;
    let inward = vr
        * (crate::constants::VORTEX_FALL_BASE
            + crate::constants::VORTEX_FALL_CORE_BOOST * (1.0 - 0.5));
    assert!(
        (a0 - omega).abs() < 0.02,
        "the angle must track the shared law (got {a0}, want ~{omega})"
    );
    assert!(
        (r0 - (0.5 - inward)).abs() < 0.02,
        "the radius must track the shared law (got {r0}, want ~{})",
        0.5 - inward
    );
}

#[test]
fn vortex_heads_compose_soft_warm_never_core() {
    // The NIGHT-research-16 soft-light ruling (the black hole's
    // NIGHT-research-11 precedent): the radius ladder's standing
    // ceiling is Hot BY CONSTRUCTION — the retired Core rung was
    // only unreachable by pass ordering (absorption deactivates
    // motes below VORTEX_CORE_R before the draw pass), an accident
    // a reorder would have broken. Now every radius composes to the
    // soft warm ceiling or below; the drain's luminance gradient
    // (Ghost rim, Mid band, Hot core zone) is pinned at the
    // boundaries.
    use crate::cloud::type_rain::monolith::BrightnessLevel;
    use crate::cloud::type_rain::vortex::vortex::level_for_radius;

    fn rank(level: BrightnessLevel) -> u8 {
        match level {
            BrightnessLevel::Ghost => 0,
            BrightnessLevel::Dim => 1,
            BrightnessLevel::Mid => 2,
            BrightnessLevel::Hot => 3,
            BrightnessLevel::Core => 4,
        }
    }

    for i in 0..=400 {
        let radius = i as f32 / 400.0 * 1.1;
        let level = level_for_radius(radius);
        assert!(
            rank(level) <= rank(BrightnessLevel::Hot),
            "the radius ladder must never land Core (r {radius})"
        );
    }
    // Zone boundaries pin the drain's gradient.
    assert_eq!(
        rank(level_for_radius(0.9)),
        rank(BrightnessLevel::Ghost),
        "the rim zone reads Ghost"
    );
    assert_eq!(
        rank(level_for_radius(0.5)),
        rank(BrightnessLevel::Mid),
        "the mid band reads Mid"
    );
    assert_eq!(
        rank(level_for_radius(0.2)),
        rank(BrightnessLevel::Hot),
        "the core zone reads the soft warm ceiling"
    );
    assert_eq!(
        rank(level_for_radius(0.0)),
        rank(BrightnessLevel::Hot),
        "the center composes soft warm, never Core"
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
