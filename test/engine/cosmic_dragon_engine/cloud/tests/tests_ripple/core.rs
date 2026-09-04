// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Core ripple-style behavior contracts (task-18): scene resolution,
//! surface geometry, droplet end-cap, impact hooks, ring expiry, the
//! zero-overlap region contract, and style transitions.

use super::*;

#[test]
fn ripple_scene_resolves_style_and_fields() {
    let s = crate::scene::get_scene("ripple").expect("ripple scene exists");
    assert_eq!(s.config.rain_style, RainStyle::Ripple);
    assert_eq!(s.config.color, Some("ocean"));
    assert_eq!(s.config.charset, Some("matrix"));
    // Ripple is droplet-family: shares the droplet pool + phosphor Pass 2,
    // but is NOT spawn-remainder driven (per-column droplet timing).
    assert!(RainStyle::Ripple.is_droplet_family());
    assert!(!RainStyle::Ripple.uses_spawn_remainder());
}

#[test]
fn ripple_water_line_geometry() {
    use crate::cloud::ripple::RippleSurface;
    // 40-line viewport: surface 3 rows above the bottom, droplets stop
    // another 3 rows above that.
    assert_eq!(RippleSurface::water_line(40), 37);
    assert_eq!(RippleSurface::droplet_end_line(40), 34);
    // Small viewports clamp sanely: the water line never rises above
    // row 1, and the droplet zone saturates at row 0 (a degenerate but
    // valid 1-row fall — terminals this small are clamped by the engine
    // anyway, the geometry must simply stay in-range).
    assert!(RippleSurface::water_line(4) >= 1);
    assert_eq!(RippleSurface::droplet_end_line(6), 0);
}

#[test]
fn ripple_droplets_stop_above_the_surface() {
    let mut cloud = make_ripple_cloud(60, 25);
    let surface_end = crate::cloud::ripple::RippleSurface::droplet_end_line(25);
    // Sample many specs: every end_line (natural AND early-death rolls)
    // must respect the surface cap.
    for col in 0..60u16 {
        let spec = cloud.build_droplet_spec(col);
        assert!(
            spec.end_line <= surface_end,
            "droplet end_line {} must be capped at surface {} (col {col})",
            spec.end_line,
            surface_end
        );
    }
}

#[test]
fn ripple_impacts_fire_and_rings_expire() {
    use crate::cloud::ripple::RippleStep;
    let mut cloud = make_ripple_cloud(60, 25);

    // Direct impact injection: two impacts on the water line.
    let water_line = crate::cloud::ripple::RippleSurface::water_line(25);
    let mut rng = cloud.mt.clone();
    for col in [10u16, 30] {
        cloud
            .ripple_surface
            .spawn_impact(col, water_line, 0, &cloud.rand_chance, &mut rng);
    }
    assert_eq!(
        cloud.ripple_surface.active_ring_count_for_test(),
        2,
        "both impacts must open rings"
    );
    assert!(
        cloud.ripple_surface.active_splash_count_for_test() >= 4,
        "each impact hops 2-4 splash particles"
    );

    // Advance well past the longest ring lifetime (1.6 s × 1.15 ≈ 1.9 s)
    // with no new impacts: everything must expire.
    let start = Instant::now();
    for idx in 0..150 {
        let now = start + Duration::from_millis(idx * 16);
        let step = RippleStep {
            now,
            lines: 25,
            chars_per_sec: 20.0,
            max_sim_delta: Duration::from_millis(16),
            resume_blend: 1.0,
        };
        cloud.ripple_surface.advance(&step);
    }
    assert_eq!(cloud.ripple_surface.active_ring_count_for_test(), 0);
    assert_eq!(cloud.ripple_surface.active_splash_count_for_test(), 0);
}

// Compile-time contract: the three zones (droplet fall / splash rise /
// surface rings) stay disjoint by construction — the droplet clearance
// must strictly exceed the splash rise cap.
const _: () =
    assert!(crate::constants::RIPPLE_DROPLET_CLEAR_ROWS > crate::constants::RIPPLE_SPLASH_MAX_RISE);

#[test]
fn ripple_region_contract_never_overlaps_droplets() {
    // Runtime side of the zone contract (see the const pin above for the
    // compile-time half): the splash floor must sit below the droplet zone.
    // And the splash rise cap bounds the min-y clamp in advance().
    let water_line = crate::cloud::ripple::RippleSurface::water_line(40);
    let min_y = water_line.saturating_sub(crate::constants::RIPPLE_SPLASH_MAX_RISE);
    let droplet_end = crate::cloud::ripple::RippleSurface::droplet_end_line(40);
    assert!(
        min_y > droplet_end,
        "splash floor ({min_y}) must sit below the droplet zone ({droplet_end})"
    );
}

#[test]
fn ripple_integration_produces_rings_from_real_droplets() {
    let mut cloud = make_ripple_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    // 3 seconds at cps 20: droplets spawn at the top, fall to the
    // capped end_line, die at the surface, and open ripples.
    run_frames(&mut cloud, &mut frame, 180, 16);
    assert!(
        cloud.ripple_surface.active_ring_count_for_test() > 0,
        "droplet surface deaths must open ripple rings"
    );
    // The frame stream stays alive throughout.
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
        "ripple must keep the frame stream live ({dirty}/30)"
    );
}

#[test]
fn ripple_style_transition_resets_surface() {
    let mut cloud = make_ripple_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 180, 16);
    assert!(cloud.ripple_surface.active_ring_count_for_test() > 0);

    // Ripple → Glyph: the surface must go quiet (no rings/splashes
    // survive the switch), droplet pool stays warm.
    cloud.transition_rain_style(RainStyle::Glyph);
    assert_eq!(cloud.ripple_surface.active_ring_count_for_test(), 0);
    assert_eq!(cloud.ripple_surface.active_splash_count_for_test(), 0);
    assert!(!cloud.droplets.is_empty());

    // Glyph → Ripple: fresh surface, rain resumes, impacts return.
    cloud.transition_rain_style(RainStyle::Ripple);
    run_frames(&mut cloud, &mut frame, 180, 16);
    assert!(
        cloud.ripple_surface.active_ring_count_for_test() > 0,
        "ripple re-arms after a style round-trip"
    );
}

#[test]
fn ripple_active_droplet_count_routes_to_droplets() {
    let mut cloud = make_ripple_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 60, 16);
    let expected = cloud.droplets.iter().filter(|d| d.is_alive).count();
    assert_eq!(cloud.active_droplet_count(), expected);
}
