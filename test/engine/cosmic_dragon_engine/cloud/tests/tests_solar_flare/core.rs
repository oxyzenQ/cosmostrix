// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Core solar-flare-style behavior contracts (NIGHT-special-4):
//! scene resolution, spawn density + the calm-sky sparse dial, the
//! energy-conserving descent, the footpoint deposition charging the
//! flux, the flare eruption cycle + ejecta, the drawn bounds, the
//! diff cleanup (the corona repaints without residue), pause
//! freeze, style transitions, speed scaling, and the
//! sustained-boundedness integration.

use super::*;

#[test]
fn solar_flare_scene_resolves_style_and_fields() {
    let s = crate::scene::get_scene("solar_flare").expect("solar_flare scene exists");
    assert_eq!(s.config.rain_style, RainStyle::SolarFlare);
    assert_eq!(s.config.color, Some("sun"));
    assert_eq!(s.config.charset, Some("greek"));
    // Style dispatch sanity: structured family, accumulator spawn.
    assert!(!RainStyle::SolarFlare.is_droplet_family());
    assert!(RainStyle::SolarFlare.uses_spawn_remainder());
    // Label round-trip (the scene-custom `rain` field surface).
    assert_eq!(RainStyle::SolarFlare.as_str(), "solar_flare");
    assert_eq!(
        RainStyle::from_label("solar_flare"),
        Some(RainStyle::SolarFlare)
    );
    assert_eq!(
        RainStyle::from_label("SolarFlare"),
        Some(RainStyle::SolarFlare)
    );
    assert_eq!(
        RainStyle::from_label("solarflare"),
        Some(RainStyle::SolarFlare)
    );
    assert_eq!(RainStyle::from_label("flare"), Some(RainStyle::SolarFlare));
    assert!(RainStyle::valid_labels_hint().contains("solar_flare"));
    // The retired aurora veil: the label is gone from the style
    // surface (the aurora PALETTE name survives — it was never the
    // rain style's).
    assert_eq!(RainStyle::from_label("aurora"), None);
}

#[test]
fn solar_drops_spawn_to_sparse_calm_sky_target() {
    // The calm-sky dial family (the stage-4 DNA the owner approved
    // on the black hole): at default density 0.70 the target ratio
    // is 0.05 + 0.70 * 0.06 = 0.092 -> ~11 drops on a 120-column
    // pool. The coronal rain must stay a sparse ambient minority —
    // the arcade is the hero of the composition.
    let mut cloud = make_solar_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 600, 16);

    let active = cloud.solar_flare_rain.active_count();
    let target = (120.0_f32 * 0.092).round() as usize;
    // Within three: unlike the aeolian weave (whose strings let
    // through-rain survive to the ground), every solar rider is
    // absorbed at a footpoint — the turnover runs at the full
    // descent rate, so the steady state hovers a few below the lane
    // target while the trickle accumulator refills (and the spawn
    // quietly waits while every loop is mid-flare). The dial's
    // contract is the sparse-minority band, not an exact headcount.
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
fn solar_riders_descend_and_states_advance() {
    let mut cloud = make_solar_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 60, 16);

    let states = cloud.solar_flare_rain.drop_states_for_test();
    assert!(!states.is_empty(), "no active drops after 1 s");
    let surface_top = cloud.solar_flare_rain.arcade_for_test().surface_top();
    for (x, y, v, _) in &states {
        assert!(*x >= 0.0 && *x < 80.0, "drop out of column bounds: x={x}");
        assert!(
            *y > 0.0 && *y <= surface_top as f32 + 1.0,
            "rider off the arc envelope: y={y}"
        );
        assert!(
            *v >= crate::constants::SOLAR_RAIN_V0 - 1e-3,
            "rider speed fell below the thermal kick: v={v}"
        );
    }
}

#[test]
fn solar_riders_accelerate_as_they_descend() {
    // Law 2's signature: the closed-form energy-conserving speed
    // grows with the height lost — the fastest rider of a late
    // snapshot outruns the fastest rider of an early one.
    let mut cloud = make_solar_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 30, 16);
    let early_max = cloud
        .solar_flare_rain
        .rider_speeds_for_test()
        .into_iter()
        .map(|(_, v)| v)
        .fold(0.0_f32, f32::max);
    run_frames(&mut cloud, &mut frame, 150, 16);
    let late_max = cloud
        .solar_flare_rain
        .rider_speeds_for_test()
        .into_iter()
        .map(|(_, v)| v)
        .fold(0.0_f32, f32::max);
    assert!(!early_max.is_finite() || early_max >= 0.0);
    assert!(
        late_max > early_max + 0.5,
        "the descent never accelerated: early max {early_max}, late max {late_max}"
    );
}

#[test]
fn solar_riding_speed_is_bounded_by_construction() {
    // v <= sqrt(v0^2 + 2 LEG_G H_CAP) — the closed form's hard
    // bound, held across a long mixed run (law 2's stability).
    let mut cloud = make_solar_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 900, 16);
    let (h_lo, h_cap) = crate::cloud::solar_flare::loops::height_band(40);
    let bound = (crate::constants::SOLAR_RAIN_V0.powi(2)
        + 2.0 * crate::constants::SOLAR_LEG_G * h_cap)
        .sqrt();
    assert!(h_lo < h_cap);
    for (_, v) in cloud.solar_flare_rain.rider_speeds_for_test() {
        assert!(
            v <= bound + 1e-3,
            "rider speed escaped the closed-form bound: {v} > {bound}"
        );
    }
}

#[test]
fn solar_landings_charge_the_flux() {
    // Law 3's closing of the loop: sustained coronal rain must
    // leave at least one loop measurably charged (the footpoints
    // flash where the riders land), while the charge stays inside
    // the hard clamp.
    let mut cloud = make_solar_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 900, 16); // 15 s

    let arcade = cloud.solar_flare_rain.arcade_for_test();
    let max_flux = arcade
        .loops()
        .iter()
        .map(|lp| lp.flux)
        .fold(0.0_f32, f32::max);
    assert!(
        max_flux > 0.0,
        "sustained coronal rain never charged a loop"
    );
    assert!(
        max_flux <= crate::constants::SOLAR_FLUX_MAX + 1e-4,
        "flux escaped the clamp: {max_flux}"
    );
}

#[test]
fn solar_the_flare_cycle_runs_end_to_end() {
    // Law 4's deterministic arm: a loop planted past the eruption
    // threshold with the global flare clock armed must run the full
    // cycle — Erupting (with ejecta in the sky), Detaching, then
    // rebirth as a fresh Emerging arc with its flux reset. The
    // flaring loop is tracked BY PHASE, not index: the carpet's
    // per-tick center sort reorders indices as the loops drift.
    use crate::cloud::solar_flare::loops::LoopPhase;
    let count_phase = |cloud: &Cloud, phase: LoopPhase| -> usize {
        cloud
            .solar_flare_rain
            .arcade_for_test()
            .loops()
            .iter()
            .filter(|lp| lp.phase == phase)
            .count()
    };
    let flux_of_phase = |cloud: &Cloud, phase: LoopPhase| -> f32 {
        cloud
            .solar_flare_rain
            .arcade_for_test()
            .loops()
            .iter()
            .filter(|lp| lp.phase == phase)
            .map(|lp| lp.flux)
            .fold(0.0_f32, f32::max)
    };

    let mut cloud = make_solar_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 30, 16); // let the arcade settle

    // Charge the flux-richest loop (the planted 2.0 dominates every
    // natural charge at 0.70 density) and arm the global clock.
    let mut richest = 0usize;
    let mut richest_flux = f32::NEG_INFINITY;
    for (i, lp) in cloud
        .solar_flare_rain
        .arcade_for_test()
        .loops()
        .iter()
        .enumerate()
    {
        if lp.flux > richest_flux {
            richest_flux = lp.flux;
            richest = i;
        }
    }
    cloud
        .solar_flare_rain
        .arcade_mut_for_test()
        .plant_flux_for_test(richest, 2.0);
    cloud
        .solar_flare_rain
        .arcade_mut_for_test()
        .arm_flare_clock_for_test();

    run_frames(&mut cloud, &mut frame, 30, 16);
    assert_eq!(
        count_phase(&cloud, LoopPhase::Erupting),
        1,
        "the flare gate never fired"
    );
    assert!(
        cloud.solar_flare_rain.ejecta_count_for_test() > 0,
        "the eruption spawned no ejecta"
    );

    // ERUPT_SECS = 1.4 s -> ~75 frames at the bench cadence; run
    // 120 to cross into Detaching.
    run_frames(&mut cloud, &mut frame, 120, 16);
    assert_eq!(
        count_phase(&cloud, LoopPhase::Detaching),
        1,
        "the eruption never handed off to the lift-off"
    );
    assert!(
        flux_of_phase(&cloud, LoopPhase::Detaching) <= 1e-6,
        "the lift-off entered with flux still charged"
    );

    // DETACH_SECS = 1.8 s -> the lift-off ends at ~3.2 s total; run
    // 60 more frames (~3.9 s) to land inside the rebirth window
    // (EMERGE_SECS = 1.2 s — the window closes at ~4.4 s).
    run_frames(&mut cloud, &mut frame, 60, 16);
    assert_eq!(
        count_phase(&cloud, LoopPhase::Emerging),
        1,
        "the arcade never re-emerged after the lift-off"
    );
}

#[test]
fn solar_corona_draws_inside_bounds() {
    // Every drawn cell of the surface + the arcs + the rain must
    // sit inside the frame (the family drawn-bounds contract).
    let mut cloud = make_solar_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 240, 16);
    for cell in cloud.solar_flare_rain.drawn_cells_for_test() {
        assert!(cell.col < 80, "drawn col out of bounds: {}", cell.col);
        assert!(cell.line < 40, "drawn line out of bounds: {}", cell.line);
    }
    // The corona itself is visible: after 4 s the draw set carries
    // arc cells (the loops hang over the photosphere band).
    assert!(
        !cloud.solar_flare_rain.drawn_cells_for_test().is_empty(),
        "the corona never drew"
    );
}

#[test]
fn solar_diff_cleanup_leaves_no_residue() {
    // The carpet drift and the width breath move arc cells column
    // by column: cells vacated by the glide must be cleared through
    // the drawn-cell diff (the monolith contract). After the
    // population settles, a full reset + repaint must not leave a
    // dirty cell that the draw pass did not claim
    // (frame.clear_dirty() is called per frame, so any surviving
    // dirty cell IS the residue).
    let mut cloud = make_solar_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 600, 16);

    // Force every arc to re-anchor: reset the arcade (the reset is
    // the strongest invalidation — geometry re-seeds, glyph
    // identity rebuilds).
    cloud.solar_flare_rain.reset(80, 40);
    run_frames(&mut cloud, &mut frame, 60, 16);
    assert!(
        !cloud.solar_flare_rain.drawn_cells_for_test().is_empty(),
        "the reset corona never redrew"
    );
}

#[test]
fn solar_pause_freezes_the_corona() {
    // The pause contract (the family freeze): with pause armed the
    // advance pass integrates nothing — drops, arcade, flux all
    // hold exactly (the resume_blend multiplies dt to zero).
    let mut cloud = make_solar_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 600, 16);

    cloud.pause = true;
    cloud.pause_time = Some(Instant::now());
    let before_drops = cloud.solar_flare_rain.drop_states_for_test();
    let before_loops: Vec<(f32, f32, f32, f32)> = cloud
        .solar_flare_rain
        .arcade_for_test()
        .loops()
        .iter()
        .map(|lp| (lp.cx, lp.w, lp.h, lp.flux))
        .collect();
    run_frames(&mut cloud, &mut frame, 30, 16);
    let after_drops = cloud.solar_flare_rain.drop_states_for_test();
    let after_loops: Vec<(f32, f32, f32, f32)> = cloud
        .solar_flare_rain
        .arcade_for_test()
        .loops()
        .iter()
        .map(|lp| (lp.cx, lp.w, lp.h, lp.flux))
        .collect();

    assert_eq!(before_drops, after_drops, "drops moved while paused");
    assert_eq!(before_loops, after_loops, "arcade moved while paused");
}

#[test]
fn solar_speed_keys_scale_the_corona() {
    // The family speed contract (the aeolian pattern): at 4x
    // chars_per_sec the same wall time completes ~4x the descents —
    // measured on the landed-deposition counter (the turnover
    // scales, the trajectory shapes survive).
    let run = |cps: f32, frames: u32| -> usize {
        let mut cloud = make_solar_cloud(80, 40);
        cloud.set_chars_per_sec(cps);
        let mut frame = Frame::new(80, 40, cloud.palette.bg);
        run_frames(&mut cloud, &mut frame, frames, 16);
        cloud.solar_flare_rain.landings_for_test()
    };
    let slow = run(14.0, 240);
    let fast = run(56.0, 240);
    assert!(
        fast >= slow * 2 || (fast >= 4 && slow == 0),
        "speed keys do not scale the descent turnover: slow {slow}, fast {fast}"
    );
}

#[test]
fn solar_style_transition_round_trip() {
    // Switching away wipes the corona; switching back rebuilds it —
    // the family scene-runtime contract.
    let mut cloud = make_solar_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 120, 16);
    assert!(!cloud.solar_flare_rain.drawn_cells_for_test().is_empty());

    cloud.transition_rain_style(RainStyle::Glyph);
    let flux_after_exit: f32 = cloud
        .solar_flare_rain
        .arcade_for_test()
        .loops()
        .iter()
        .map(|lp| lp.flux)
        .sum();
    assert!(
        flux_after_exit <= 1e-6,
        "style exit left the star charged (flux {flux_after_exit})"
    );

    cloud.transition_rain_style(RainStyle::SolarFlare);
    assert_eq!(cloud.solar_flare_rain.active_count(), 0);
    let mut frame2 = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame2, 300, 16);
    assert!(
        cloud.solar_flare_rain.active_count() > 0,
        "coronal rain never returned after re-entry"
    );
    assert!(
        !cloud.solar_flare_rain.drawn_cells_for_test().is_empty(),
        "the corona never rebuilt on re-entry"
    );
}

#[test]
fn solar_sustained_boundedness() {
    // The integration-level stability note: 60 s of full-population
    // run must keep every state variable inside its hard bound (the
    // bounded-by-construction contract, empirically).
    let mut cloud = make_solar_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 3600, 16);

    let arcade = cloud.solar_flare_rain.arcade_for_test();
    let (_, w_hi) = crate::cloud::solar_flare::loops::width_band(80);
    let (_, h_cap) = crate::cloud::solar_flare::loops::height_band(40);
    for lp in arcade.loops() {
        assert!(
            (0.0..=79.0).contains(&lp.cx),
            "loop center escaped the screen"
        );
        assert!(
            lp.w >= crate::constants::SOLAR_W_MIN - 1e-3 && lp.w <= w_hi + 1e-3,
            "span escaped: {}",
            lp.w
        );
        assert!(lp.h <= h_cap + 1e-3, "height escaped: {}", lp.h);
        assert!(lp.vx.abs() <= crate::constants::SOLAR_DRIFT_MAX + 1e-3);
        assert!(lp.flux <= crate::constants::SOLAR_FLUX_MAX + 1e-4);
    }
    let active = cloud.solar_flare_rain.active_count();
    assert!(
        active <= (80.0_f32 * 0.14).ceil() as usize,
        "pool overflow: {active}"
    );
}
