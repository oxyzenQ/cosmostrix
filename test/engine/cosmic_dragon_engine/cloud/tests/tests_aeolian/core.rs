// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Core aeolian-style behavior contracts (NIGHT-special-2): scene
//! resolution, spawn density + the calm-sky sparse dial, the fall,
//! the pluck-on-crossing (the rain rings the instrument), the drawn
//! bounds, the diff cleanup (the instrument goes silent again),
//! pause freeze, style transitions, speed scaling, and the
//! sustained-boundedness integration.

use super::*;

#[test]
fn aeolian_scene_resolves_style_and_fields() {
    let s = crate::scene::get_scene("aeolian").expect("aeolian scene exists");
    assert_eq!(s.config.rain_style, RainStyle::Aeolian);
    assert_eq!(s.config.color, Some("aurora"));
    assert_eq!(s.config.charset, Some("runic"));
    // Style dispatch sanity: structured family, accumulator spawn.
    assert!(!RainStyle::Aeolian.is_droplet_family());
    assert!(RainStyle::Aeolian.uses_spawn_remainder());
    // Label round-trip (the scene-custom `rain` field surface).
    assert_eq!(RainStyle::Aeolian.as_str(), "aeolian");
    assert_eq!(RainStyle::from_label("aeolian"), Some(RainStyle::Aeolian));
    assert_eq!(RainStyle::from_label("Aeolian"), Some(RainStyle::Aeolian));
    assert!(RainStyle::valid_labels_hint().contains("aeolian"));
}

#[test]
fn aeolian_drops_spawn_to_sparse_calm_sky_target() {
    // The calm-sky dial family (the stage-4 DNA the owner approved
    // on the black hole): at default density 0.70 the target ratio
    // is 0.05 + 0.70 * 0.09 = 0.113 -> ~13 drops on a 120-column
    // pool. The rain must stay a sparse ambient minority.
    let mut cloud = make_aeolian_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 600, 16);

    let active = cloud.aeolian_rain.active_count();
    let target = (120.0_f32 * 0.113).round() as usize;
    // Within one: the target is a steady-state attractor, not a
    // floor -- a drop can be captured on the very frame this reads,
    // so the accumulator hovers at target or one below while it
    // refills.
    assert!(
        active + 1 >= target,
        "expected the density target {target} (within one), got {active}"
    );
    // The hard cap: even sustained spawning cannot push past 16%.
    assert!(
        active <= (120.0_f32 * 0.16).ceil() as usize,
        "sparse cap violated: {active} active on 120 lanes"
    );
}

#[test]
fn aeolian_drops_fall_and_drop_states_advance() {
    let mut cloud = make_aeolian_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 60, 16);

    let states = cloud.aeolian_rain.drop_states_for_test();
    assert!(!states.is_empty(), "no active drops after 1 s");
    for (x, y, _, vy) in &states {
        assert!(*y > 0.0, "drop never fell: y={y}");
        assert!(*vy > 0.0, "drop not falling: vy={vy}");
        assert!(*x >= 0.0 && *x < 80.0, "drop out of column bounds: x={x}");
    }
}

#[test]
fn aeolian_drops_accelerate_under_gravity() {
    // Law 6 state: gravity feeds the fall (vy strictly grows while
    // under terminal) — the kinetic charge the plucks carry.
    let mut cloud = make_aeolian_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 30, 16);
    let early: Vec<f32> = cloud
        .aeolian_rain
        .drop_states_for_test()
        .iter()
        .map(|(_, _, _, vy)| *vy)
        .collect();
    run_frames(&mut cloud, &mut frame, 60, 16);
    let late: Vec<f32> = cloud
        .aeolian_rain
        .drop_states_for_test()
        .iter()
        .map(|(_, _, _, vy)| *vy)
        .collect();
    let early_mean = early.iter().sum::<f32>() / early.len().max(1) as f32;
    let late_mean = late.iter().sum::<f32>() / late.len().max(1) as f32;
    assert!(
        late_mean >= early_mean,
        "gravity not feeding the fall: early {early_mean} late {late_mean}"
    );
}

#[test]
fn aeolian_strings_ring_when_rain_crosses() {
    // Law 4 + 5 integration: after enough frames for drops to reach
    // the first string row (~13 lines at ~1.3 lines/sim-s), the
    // field must carry energy (captures and grazes both pluck).
    let mut cloud = make_aeolian_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 900, 16);

    let l1 = cloud.aeolian_rain.strings_for_test().l1_norm_for_test();
    assert!(l1 > 0.0, "the rain never rang the instrument (L1={l1})");
}

#[test]
fn aeolian_field_stays_bounded_over_long_watch() {
    // The integration form of the L1 contraction proof: a long
    // watch under continuous weather must never let the field
    // amplitude run away (the decay drains what the plucks feed).
    let mut cloud = make_aeolian_cloud(80, 40);
    cloud.set_droplet_density(5.0); // max weather
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 3000, 16);

    let l1 = cloud.aeolian_rain.strings_for_test().l1_norm_for_test();
    assert!(l1 < 200.0, "field runaway over a long watch (L1={l1})");
    assert!(l1 > 0.0, "field never rang under max weather");
}

#[test]
fn aeolian_drawn_cells_stay_in_bounds() {
    let mut cloud = make_aeolian_cloud(60, 25);
    let mut frame = Frame::new(60, 25, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 900, 16);
    for cell in cloud.aeolian_rain.drawn_cells_for_test() {
        assert!(cell.col < 60, "drawn col out of bounds: {}", cell.col);
        assert!(cell.line < 25, "drawn line out of bounds: {}", cell.line);
    }
    assert!(
        !cloud.aeolian_rain.drawn_cells_for_test().is_empty(),
        "aeolian must draw visible cells (drops at minimum)"
    );
}

#[test]
fn aeolian_diff_cleanup_clears_vacated_cells() {
    // The drawn-cell diff cleanup contract: a cell that was drawn at
    // frame N and is NOT redrawn later must be cleared back to a
    // space (the pulse sprinted away / the drop fell on). Verified
    // against the drawn-cell set itself, so new plucks elsewhere on
    // the string cannot confound the read (a redrawn cell is simply
    // not in the vacated set).
    let mut cloud = make_aeolian_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);

    // Ring the instrument directly so a definite pulse exists.
    cloud.aeolian_rain.strings_mut_for_test().pluck(0, 40, 6.0);
    run_frames(&mut cloud, &mut frame, 60, 16);

    let old_cells: Vec<(u16, u16)> = cloud
        .aeolian_rain
        .drawn_cells_for_test()
        .iter()
        .map(|c| (c.col, c.line))
        .collect();
    assert!(!old_cells.is_empty(), "nothing drawn to vacate");

    // The pulse sprints at ~50 cells/s: 60 more frames (1.28 sim-s)
    // move it ~64 cells -- far past any cell it held at the snapshot.
    run_frames(&mut cloud, &mut frame, 60, 16);
    let new_set: std::collections::HashSet<(u16, u16)> = cloud
        .aeolian_rain
        .drawn_cells_for_test()
        .iter()
        .map(|c| (c.col, c.line))
        .collect();

    let mut vacated = 0usize;
    for (col, line) in &old_cells {
        if new_set.contains(&(*col, *line)) {
            continue;
        }
        vacated += 1;
        let ch = frame
            .index(*col, *line)
            .map(|i| frame.cell_at_index_ref(i).ch);
        assert_eq!(
            ch,
            Some(' '),
            "vacated cell ({col},{line}) was never cleared back to a space"
        );
    }
    assert!(vacated > 0, "no vacated cells found to verify the clear");
}

#[test]
fn aeolian_pause_freezes_the_weave() {
    let mut cloud = make_aeolian_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 600, 16);

    // Ring it, then freeze.
    cloud.aeolian_rain.strings_mut_for_test().pluck(0, 40, 6.0);
    cloud.pause = true;
    cloud.pause_time = Some(Instant::now());
    let before = cloud.aeolian_rain.strings_for_test().l1_norm_for_test();
    let states_before = cloud.aeolian_rain.drop_states_for_test();
    run_frames(&mut cloud, &mut frame, 30, 16);
    let after = cloud.aeolian_rain.strings_for_test().l1_norm_for_test();
    let states_after = cloud.aeolian_rain.drop_states_for_test();
    assert!(
        (after - before).abs() < 1e-4,
        "paused field moved: {before} -> {after}"
    );
    assert_eq!(
        states_before.len(),
        states_after.len(),
        "paused pool changed size"
    );
    for (a, b) in states_before.iter().zip(states_after.iter()) {
        assert_eq!(a.0, b.0, "paused drop moved columns");
        assert_eq!(a.1, b.1, "paused drop fell (y {} -> {})", a.1, b.1);
    }
}

#[test]
fn aeolian_style_transition_resets_and_rebuilds() {
    // Exit wipes the field (a dormant instrument must not carry a
    // ringing state); entry rebuilds the pool and the weather
    // grows again.
    let mut cloud = make_aeolian_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 900, 16);
    assert!(cloud.aeolian_rain.strings_for_test().l1_norm_for_test() > 0.0);

    cloud.transition_rain_style(RainStyle::Glyph);
    assert_eq!(
        cloud.aeolian_rain.strings_for_test().l1_norm_for_test(),
        0.0,
        "style exit left the field ringing"
    );

    cloud.transition_rain_style(RainStyle::Aeolian);
    assert_eq!(cloud.aeolian_rain.active_count(), 0);
    let mut frame2 = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame2, 300, 16);
    assert!(
        cloud.aeolian_rain.active_count() > 0,
        "weather never returned after re-entry"
    );
}

#[test]
fn aeolian_speed_keys_scale_the_whole_weave() {
    // The family speed contract: at 4x chars_per_sec the same wall
    // time moves the drops 4x as far (the whole weave runs on one
    // sim clock; trajectory shapes survive).
    let run = |cps: f32| -> Vec<f32> {
        let mut cloud = make_aeolian_cloud(80, 40);
        cloud.set_chars_per_sec(cps);
        let mut frame = Frame::new(80, 40, cloud.palette.bg);
        run_frames(&mut cloud, &mut frame, 90, 16);
        cloud
            .aeolian_rain
            .drop_states_for_test()
            .iter()
            .map(|(_, y, _, _)| *y)
            .collect::<Vec<f32>>()
    };
    let slow = run(12.0);
    let fast = run(48.0);
    let max_y = |v: &[f32]| v.iter().copied().fold(0.0_f32, f32::max);
    assert!(
        max_y(&fast) > max_y(&slow) * 2.0,
        "speed keys do not scale the fall: slow max {} fast max {}",
        max_y(&slow),
        max_y(&fast)
    );
}

#[test]
fn aeolian_trickle_never_bursts() {
    // The stage-4 trickle contract (mirrors the infall's): on a
    // manual continuous clock the per-frame active-count jump is at
    // most 1 (the deficit-bounded accumulator never lets a frame
    // spawn two).
    let mut cloud = make_aeolian_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    let start = Instant::now();
    cloud.last_spawn_time = start;
    cloud.last_phosphor_time = start;
    let mut prev = 0usize;
    for idx in 0..600 {
        let now = start + Duration::from_millis(idx * 16);
        cloud.rain_at(&mut frame, now);
        frame.clear_dirty();
        let now_count = cloud.aeolian_rain.active_count();
        assert!(
            now_count <= prev + 1,
            "spawn burst at frame {idx}: {prev} -> {now_count}"
        );
        prev = now_count;
    }
    assert!(prev > 0, "the weather never started");
}

#[test]
fn aeolian_active_count_routes_to_drops() {
    // The HUD / metrics routing: active_droplet_count() reads the
    // aeolian pool.
    let mut cloud = make_aeolian_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 300, 16);
    assert_eq!(
        cloud.active_droplet_count(),
        cloud.aeolian_rain.active_count()
    );
}

#[test]
fn aeolian_resize_rebuilds_the_instrument() {
    // A resize resets the field for the new geometry: strings
    // re-tier and the field starts silent.
    let mut cloud = make_aeolian_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 900, 16);
    let rows_before = cloud.aeolian_rain.strings_for_test().string_rows().len();
    assert_eq!(rows_before, 3); // 40 lines -> three strings

    cloud.reset(100, 60);
    assert_eq!(
        cloud.aeolian_rain.strings_for_test().l1_norm_for_test(),
        0.0,
        "resize left stale field energy"
    );
    assert_eq!(
        cloud.aeolian_rain.strings_for_test().string_rows().len(),
        4,
        "60 lines must re-tier to four strings"
    );
    let mut frame2 = Frame::new(100, 60, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame2, 300, 16);
    assert!(
        cloud.aeolian_rain.active_count() > 0,
        "weather never restarted after resize"
    );
}
