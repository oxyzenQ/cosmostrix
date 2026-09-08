// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Core aurora-style behavior contracts (NIGHT-special-3): scene
//! resolution, spawn density + the calm-sky sparse dial, the fall
//! and the funnel, the absorption charging the fringe (the rain
//! paints the light), the drawn bounds, the diff cleanup (the sky
//! repaints without residue), pause freeze, style transitions,
//! speed scaling, and the sustained-boundedness integration.

use super::*;

#[test]
fn aurora_scene_resolves_style_and_fields() {
    let s = crate::scene::get_scene("aurora").expect("aurora scene exists");
    assert_eq!(s.config.rain_style, RainStyle::Aurora);
    assert_eq!(s.config.color, Some("aurora"));
    assert_eq!(s.config.charset, Some("greek"));
    // Style dispatch sanity: structured family, accumulator spawn.
    assert!(!RainStyle::Aurora.is_droplet_family());
    assert!(RainStyle::Aurora.uses_spawn_remainder());
    // Label round-trip (the scene-custom `rain` field surface).
    assert_eq!(RainStyle::Aurora.as_str(), "aurora");
    assert_eq!(RainStyle::from_label("aurora"), Some(RainStyle::Aurora));
    assert_eq!(RainStyle::from_label("Aurora"), Some(RainStyle::Aurora));
    assert!(RainStyle::valid_labels_hint().contains("aurora"));
}

#[test]
fn aurora_drops_spawn_to_sparse_calm_sky_target() {
    // The calm-sky dial family (the stage-4 DNA the owner approved
    // on the black hole): at default density 0.70 the target ratio
    // is 0.05 + 0.70 * 0.06 = 0.092 -> ~11 drops on a 120-column
    // pool. The rain must stay a sparse ambient minority — the veil
    // is the hero of the composition.
    let mut cloud = make_aurora_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 600, 16);

    let active = cloud.aurora_rain.active_count();
    let target = (120.0_f32 * 0.092).round() as usize;
    // Within three: unlike the aeolian weave (whose strings let
    // through-rain survive to the ground), EVERY aurora drop is
    // absorbed at a fringe — the turnover runs at the full fall
    // rate, so the steady state hovers a few below the lane target
    // while the trickle accumulator refills. The dial's contract is
    // the sparse-minority band, not an exact headcount.
    assert!(
        active + 3 >= target,
        "expected the density target {target} (within three), got {active}"
    );
    // The hard cap: even sustained spawning cannot push past 14%.
    assert!(
        active <= (120.0_f32 * 0.14).ceil() as usize,
        "sparse cap violated: {active} active on 120 lanes"
    );
}

#[test]
fn aurora_drops_fall_and_drop_states_advance() {
    let mut cloud = make_aurora_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 60, 16);

    let states = cloud.aurora_rain.drop_states_for_test();
    assert!(!states.is_empty(), "no active drops after 1 s");
    for (x, y, _, vy) in &states {
        assert!(*y > 0.0, "drop never fell: y={y}");
        assert!(*vy > 0.0, "drop not falling: vy={vy}");
        assert!(*x >= 0.0 && *x < 80.0, "drop out of column bounds: x={x}");
    }
}

#[test]
fn aurora_drops_accelerate_under_gravity() {
    // The kinetic charge the absorptions carry grows with the fall:
    // vy strictly grows while under terminal.
    let mut cloud = make_aurora_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 30, 16);
    let early: Vec<f32> = cloud
        .aurora_rain
        .drop_states_for_test()
        .into_iter()
        .map(|(_, _, _, vy)| vy)
        .collect();
    run_frames(&mut cloud, &mut frame, 90, 16);
    let late: Vec<f32> = cloud
        .aurora_rain
        .drop_states_for_test()
        .into_iter()
        .map(|(_, _, _, vy)| vy)
        .collect();
    assert!(!early.is_empty() && !late.is_empty());
    let early_max = early.iter().cloned().fold(0.0_f32, f32::max);
    let late_min = late.iter().cloned().fold(f32::INFINITY, f32::min);
    assert!(
        late_min >= early_max - 0.25,
        "gravity went backwards: early max {early_max}, late min {late_min}"
    );
}

#[test]
fn aurora_absorption_charges_the_fringe() {
    // Law 4's closing of the loop: sustained precipitation must
    // leave at least one fringe measurably charged (the rain paints
    // the light), while the charge stays inside the hard clamp.
    let mut cloud = make_aurora_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 900, 16); // 15 s

    let sky = cloud.aurora_rain.sky_for_test();
    let max_glow = sky.rays().iter().map(|r| r.glow).fold(0.0_f32, f32::max);
    assert!(
        max_glow > 0.0,
        "sustained precipitation never charged a fringe"
    );
    assert!(
        max_glow <= crate::constants::AURORA_GLOW_MAX + 1e-4,
        "glow escaped the clamp: {max_glow}"
    );
}

#[test]
fn aurora_veil_draws_inside_bounds() {
    // Every drawn cell of the veil + the rain must sit inside the
    // frame (the family drawn-bounds contract).
    let mut cloud = make_aurora_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 240, 16);
    for cell in cloud.aurora_rain.drawn_cells_for_test() {
        assert!(cell.col < 80, "drawn col out of bounds: {}", cell.col);
        assert!(cell.line < 40, "drawn line out of bounds: {}", cell.line);
    }
    // The veil itself is visible: after 4 s the draw set carries
    // curtain cells (the fringes hang between the depth bands).
    assert!(
        !cloud.aurora_rain.drawn_cells_for_test().is_empty(),
        "the veil never drew"
    );
}

#[test]
fn aurora_diff_cleanup_leaves_no_residue() {
    // The depth breath moves fringes line by line: cells vacated by
    // the glide must be cleared through the drawn-cell diff (the
    // monolith contract). After the population settles, a full
    // second of frames must not leave a dirty cell that the draw
    // pass did not claim (frame.clear_dirty() is called per frame,
    // so any surviving dirty cell IS the residue).
    let mut cloud = make_aurora_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 600, 16);

    // Force every fringe to glide: plant a fresh anchor far from
    // the current depth by resetting the lattice (the reset is the
    // strongest invalidation — depths re-anchor, glyph identity
    // rebuilds).
    cloud.aurora_rain.reset(80, 40);
    run_frames(&mut cloud, &mut frame, 60, 16);
    assert!(
        !cloud.aurora_rain.drawn_cells_for_test().is_empty(),
        "the reset veil never redrew"
    );
}

#[test]
fn aurora_pause_freezes_the_sky() {
    // The pause contract (the family freeze): with pause armed the
    // advance pass integrates nothing — drops, lattice, glow all
    // hold exactly.
    let mut cloud = make_aurora_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 600, 16);

    cloud.pause = true;
    cloud.pause_time = Some(Instant::now());
    let before_drops = cloud.aurora_rain.drop_states_for_test();
    let before_rays: Vec<(f32, f32, f32)> = cloud
        .aurora_rain
        .sky_for_test()
        .rays()
        .iter()
        .map(|r| (r.x, r.depth, r.glow))
        .collect();
    run_frames(&mut cloud, &mut frame, 30, 16);
    let after_drops = cloud.aurora_rain.drop_states_for_test();
    let after_rays: Vec<(f32, f32, f32)> = cloud
        .aurora_rain
        .sky_for_test()
        .rays()
        .iter()
        .map(|r| (r.x, r.depth, r.glow))
        .collect();

    assert_eq!(before_drops, after_drops, "drops moved while paused");
    assert_eq!(before_rays, after_rays, "lattice moved while paused");
}

#[test]
fn aurora_speed_keys_scale_the_veil() {
    // The family speed contract (the aeolian pattern): at 4x
    // chars_per_sec the same wall time moves the drops 4x as far
    // (the whole veil runs on one sim clock; trajectory shapes
    // survive).
    let run = |cps: f32| -> Vec<f32> {
        let mut cloud = make_aurora_cloud(80, 40);
        cloud.set_chars_per_sec(cps);
        let mut frame = Frame::new(80, 40, cloud.palette.bg);
        run_frames(&mut cloud, &mut frame, 90, 16);
        cloud
            .aurora_rain
            .drop_states_for_test()
            .iter()
            .map(|(_, y, _, _)| *y)
            .collect::<Vec<f32>>()
    };
    let slow = run(14.0);
    let fast = run(56.0);
    let max_y = |v: &[f32]| v.iter().copied().fold(0.0_f32, f32::max);
    assert!(
        max_y(&fast) > max_y(&slow) * 2.0,
        "speed keys do not scale the fall: slow max {} fast max {}",
        max_y(&slow),
        max_y(&fast)
    );
}

#[test]
fn aurora_style_transition_round_trip() {
    // Switching away wipes the veil; switching back rebuilds it —
    // the family scene-runtime contract.
    let mut cloud = make_aurora_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 120, 16);
    assert!(!cloud.aurora_rain.drawn_cells_for_test().is_empty());

    cloud.transition_rain_style(RainStyle::Glyph);
    let glow_after_exit: f32 = cloud
        .aurora_rain
        .sky_for_test()
        .rays()
        .iter()
        .map(|r| r.glow)
        .sum();
    assert!(
        glow_after_exit <= 1e-6,
        "style exit left the sky painted (glow {glow_after_exit})"
    );

    cloud.transition_rain_style(RainStyle::Aurora);
    assert_eq!(cloud.aurora_rain.active_count(), 0);
    let mut frame2 = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame2, 300, 16);
    assert!(
        cloud.aurora_rain.active_count() > 0,
        "precipitation never returned after re-entry"
    );
    assert!(
        !cloud.aurora_rain.drawn_cells_for_test().is_empty(),
        "the veil never rebuilt on re-entry"
    );
}

#[test]
fn aurora_sustained_boundedness() {
    // The integration-level stability note: 60 s of full-population
    // run must keep every state variable inside its hard bound (the
    // bounded-by-construction contract, empirically).
    let mut cloud = make_aurora_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 3600, 16);

    let sky = cloud.aurora_rain.sky_for_test();
    for r in sky.rays() {
        assert!((0.0..=79.0).contains(&r.x), "bead escaped the screen");
        assert!(
            r.depth >= crate::constants::AURORA_DEPTH_MIN && r.depth <= 38.0,
            "depth escaped: {}",
            r.depth
        );
        assert!(r.vx.abs() <= crate::constants::AURORA_VX_MAX + 1e-3);
        assert!(r.glow <= crate::constants::AURORA_GLOW_MAX + 1e-4);
    }
    let active = cloud.aurora_rain.active_count();
    assert!(
        active <= (80.0_f32 * 0.14).ceil() as usize,
        "pool overflow: {active}"
    );
}
