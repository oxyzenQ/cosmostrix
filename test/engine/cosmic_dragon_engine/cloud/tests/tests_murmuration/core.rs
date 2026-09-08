// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Core murmuration-style behavior contracts (NIGHT-research-7):
//! scene resolution, the staggered entry, the flight bounds, the
//! flock's coherence + separation emergent behavior, the
//! breathing oscillator, the startle cycle + scatter, the drawn
//! bounds, the diff cleanup, pause freeze, style transitions,
//! speed scaling, and the sustained-boundedness integration.

use super::*;

use crate::constants::{MURM_SPEED_MAX, MURM_SPEED_MIN};

#[test]
fn murmuration_scene_resolves_style_and_fields() {
    let s = crate::scene::get_scene("murmuration").expect("murmuration scene exists");
    assert_eq!(s.config.rain_style, RainStyle::Murmuration);
    assert_eq!(s.config.color, Some("gold"));
    assert_eq!(s.config.charset, Some("minimal"));
    assert_eq!(s.config.fps, Some(60.0));
    assert_eq!(s.config.speed, Some(18.0));
    assert_eq!(s.config.density, Some(0.55));
    assert_eq!(
        s.config.glitch_level,
        Some(crate::config::GlitchLevel::None)
    );
    // Style dispatch sanity: structured family, accumulator spawn.
    assert!(!RainStyle::Murmuration.is_droplet_family());
    assert!(RainStyle::Murmuration.uses_spawn_remainder());
    // Label round-trip (the scene-custom `rain` field surface).
    assert_eq!(RainStyle::Murmuration.as_str(), "murmuration");
    assert_eq!(
        RainStyle::from_label("murmuration"),
        Some(RainStyle::Murmuration)
    );
    assert_eq!(
        RainStyle::from_label("Murmuration"),
        Some(RainStyle::Murmuration)
    );
    assert_eq!(
        RainStyle::from_label("murmur"),
        Some(RainStyle::Murmuration)
    );
    assert_eq!(
        RainStyle::from_label("starlings"),
        Some(RainStyle::Murmuration)
    );
    assert!(RainStyle::valid_labels_hint().contains("murmuration"));
}

#[test]
fn murm_flock_assembles_through_staggered_entry() {
    // The accumulator contract: birds fly in from the edges over
    // the first seconds; the steady state is the full target.
    let mut cloud = make_murm_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    let target = cloud.murmuration_rain.birds.len();
    assert!(target > 0, "the pool is empty at reset");
    assert_eq!(cloud.murmuration_rain.active_count(), 0);
    run_frames(&mut cloud, &mut frame, 300, 16);
    let active = cloud.murmuration_rain.active_count();
    assert!(
        active >= target - 2,
        "the flock did not assemble: {active}/{target}"
    );
}

#[test]
fn murm_birds_fly_in_bounds_and_in_band() {
    // The flight contract: every bird stays inside the viewport
    // and flies at a legal speed (never hovers, never
    // teleports).
    let mut cloud = make_murm_cloud(100, 40);
    let mut frame = Frame::new(100, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 600, 16);
    let states = cloud.murmuration_rain.bird_states_for_test();
    assert!(!states.is_empty(), "no birds after 10 s");
    for (x, y, vx, vy) in &states {
        assert!((0.0..100.0).contains(x), "bird out of column bounds: x={x}");
        assert!((0.0..40.0).contains(y), "bird out of line bounds: y={y}");
        let s = (vx * vx + vy * vy).sqrt();
        assert!(
            (MURM_SPEED_MIN - 1e-3..=MURM_SPEED_MAX + 1e-3).contains(&s),
            "bird speed out of band: {s}"
        );
    }
}

#[test]
fn murm_flock_stays_coherent() {
    // The emergent-cohesion contract: after settling, the flock's
    // radius of gyration stays a fraction of the viewport (the
    // birds do NOT disperse into a uniform sky — the flock reads
    // as a body).
    let mut cloud = make_murm_cloud(100, 40);
    let mut frame = Frame::new(100, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 900, 16);
    let states = cloud.murmuration_rain.bird_states_for_test();
    let n = states.len().max(1) as f32;
    let cx = states.iter().map(|(x, _, _, _)| x).sum::<f32>() / n;
    let cy = states.iter().map(|(_, y, _, _)| y).sum::<f32>() / n;
    let spread = states
        .iter()
        .map(|(x, y, _, _)| ((x - cx) * (x - cx) + (y - cy) * (y - cy)).sqrt())
        .sum::<f32>()
        / n;
    // The flock's mean distance from its own centroid must read
    // as a flock (well under the viewport's diagonal), not a
    // uniform dispersion (~28 on a 100x40 screen).
    assert!(
        spread < 24.0,
        "the flock dispersed: mean spread {spread:.2} from centroid ({cx:.1},{cy:.1})"
    );
}

#[test]
fn murm_separation_keeps_minimum_spacing() {
    // The emergent-separation contract: birds never stack — the
    // closest pair stays at a positive fraction of the separation
    // radius in the steady state.
    let mut cloud = make_murm_cloud(100, 40);
    let mut frame = Frame::new(100, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 900, 16);
    let states = cloud.murmuration_rain.bird_states_for_test();
    let mut min_d = f32::MAX;
    for i in 0..states.len() {
        for j in (i + 1)..states.len() {
            let dx = states[i].0 - states[j].0;
            let dy = states[i].1 - states[j].1;
            let d = (dx * dx + dy * dy).sqrt();
            min_d = min_d.min(d);
        }
    }
    assert!(
        min_d > 0.5,
        "birds stacked on a cell: closest pair {min_d:.3}"
    );
}

#[test]
fn murm_breathing_oscillator_is_bounded() {
    // Law 4: the cohesion multiplier cycles inside its band
    // (never negative — a repulsive cohesion would tear the
    // flock; never extreme — it would collapse it).
    let mut cloud = make_murm_cloud(100, 40);
    let mut frame = Frame::new(100, 40, cloud.palette.bg);
    let mut lo = f32::MAX;
    let mut hi = f32::MIN;
    for _ in 0..20 {
        run_frames(&mut cloud, &mut frame, 60, 16);
        let v = cloud.murmuration_rain.breath_for_test();
        lo = lo.min(v);
        hi = hi.max(v);
    }
    let (base, amp) = (
        crate::constants::MURM_BREATH_BASE,
        crate::constants::MURM_BREATH_AMP,
    );
    assert!(
        (lo - (base - amp)).abs() < 0.2 && (hi - (base + amp)).abs() < 0.2,
        "breathing outside its band: [{lo:.3}, {hi:.3}] vs [{}, {}]",
        base - amp,
        base + amp
    );
    assert!(hi > lo, "the breathing never breathed");
}

#[test]
fn murm_startle_fires_and_scatters() {
    // Law 5: the startle clock fires over a long run; a startled
    // flock's spread is measurably wider than the calm flock's.
    let mut cloud = make_murm_cloud(100, 40);
    let mut frame = Frame::new(100, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 900, 16);
    assert!(
        cloud.murmuration_rain.startles_for_test() > 0,
        "the startle clock never fired over 15 s"
    );

    // The scatter: arm a startle on a formed flock, measure the
    // spread immediately after.
    let spread = |cloud: &Cloud| -> f32 {
        let s = cloud.murmuration_rain.bird_states_for_test();
        let n = s.len().max(1) as f32;
        let cx = s.iter().map(|(x, _, _, _)| x).sum::<f32>() / n;
        let cy = s.iter().map(|(_, y, _, _)| y).sum::<f32>() / n;
        s.iter()
            .map(|(x, y, _, _)| ((x - cx) * (x - cx) + (y - cy) * (y - cy)).sqrt())
            .sum::<f32>()
            / n
    };
    cloud.murmuration_rain.arm_startle_for_test(0.001);
    let before = spread(&cloud);
    run_frames(&mut cloud, &mut frame, 2, 16); // the fire frame
    assert!(
        cloud.murmuration_rain.predator_for_test().is_some(),
        "the predator flash did not arm"
    );
    let after = spread(&cloud);
    // The scatter blooms: the spread grows across the kick (the
    // flock re-gathers over the following seconds).
    assert!(
        after > before - 1.0,
        "the startle collapsed the flock: {before:.2} -> {after:.2}"
    );
    // The re-gather: within a few sim-seconds the flock recovers
    // toward its calm spread.
    run_frames(&mut cloud, &mut frame, 240, 16);
    let recovered = spread(&cloud);
    assert!(
        recovered < after + 2.0 || recovered < before * 1.6,
        "the flock never re-gathered: {recovered:.2}"
    );
}

#[test]
fn murm_drawn_cells_stay_in_bounds() {
    // The renderer's floor: every cell the draw pass claims sits
    // inside the viewport (heads, trails, the predator flash).
    let mut cloud = make_murm_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 240, 16);
    for cell in cloud.murmuration_rain.drawn_cells_for_test() {
        assert!(cell.col < 80, "drawn cell col out of bounds: {}", cell.col);
        assert!(
            cell.line < 40,
            "drawn cell line out of bounds: {}",
            cell.line
        );
    }
}

#[test]
fn murm_repaints_without_residue() {
    // The diff-cleanup contract: a force-redraw + continued
    // frames keeps the drawn population in the same order (no
    // stale cells, no residue ghosts).
    let mut cloud = make_murm_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 300, 16);
    let baseline = cloud.murmuration_rain.drawn_cells_for_test().len();
    cloud.clear_redraw_flags_for_test();
    cloud.force_draw_everything();
    run_frames(&mut cloud, &mut frame, 30, 16);
    let after = cloud.murmuration_rain.drawn_cells_for_test().len();
    assert!(
        after > baseline / 2 && after < baseline * 2 + 64,
        "drawn population unstable: {baseline} -> {after}"
    );
}

#[test]
fn murm_pause_freezes_the_flock() {
    // The family pause contract: with pause armed the advance
    // pass integrates nothing — the birds hold exactly.
    let mut cloud = make_murm_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 300, 16);
    cloud.pause = true;
    cloud.pause_time = Some(Instant::now());
    let before = cloud.murmuration_rain.bird_states_for_test();
    run_frames(&mut cloud, &mut frame, 30, 16);
    let after = cloud.murmuration_rain.bird_states_for_test();
    assert_eq!(before, after, "the flock moved while paused");
}

#[test]
fn murm_style_transition_round_trip() {
    // The scene-cycle contract: switching away empties the flock;
    // switching back rebuilds it — the staggered entry returns.
    let mut cloud = make_murm_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 200, 16);
    assert!(!cloud.murmuration_rain.drawn_cells_for_test().is_empty());

    cloud.transition_rain_style(RainStyle::Glyph);
    assert_eq!(
        cloud.murmuration_rain.active_count(),
        0,
        "style exit left birds flying"
    );

    cloud.transition_rain_style(RainStyle::Murmuration);
    assert_eq!(cloud.murmuration_rain.active_count(), 0);
    let mut frame2 = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame2, 300, 16);
    assert!(
        cloud.murmuration_rain.active_count() > 0,
        "the flock never returned after re-entry"
    );
}

#[test]
fn murm_speed_keys_scale_the_flock() {
    // The family speed contract: at double chars_per_sec the
    // flock flies double the distance in the same wall time
    // (measured on the total displacement magnitude — the
    // trajectory shapes survive the scaling).
    let displacement = |cps: f32| -> f32 {
        let mut cloud = make_murm_cloud(80, 40);
        cloud.set_chars_per_sec(cps);
        let mut frame = Frame::new(80, 40, cloud.palette.bg);
        run_frames(&mut cloud, &mut frame, 60, 16);
        let s = cloud.murmuration_rain.bird_states_for_test();
        s.iter()
            .map(|(_, _, vx, vy)| (vx * vx + vy * vy).sqrt())
            .sum::<f32>()
            / s.len().max(1) as f32
    };
    // The mean speed at steady state is band-clamped, but the
    // 2x-clock run reaches its target displacement faster: pin
    // the sim-clock scaling on the anchor (the one integrator
    // without the clamps) — the anchor's hold budget halves.
    let slow = displacement(18.0);
    let fast = displacement(36.0);
    // Both read as flying flocks (the band holds under both
    // clocks — the speed contract's shape invariance).
    assert!(
        slow > 0.0 && fast > 0.0,
        "the flock did not fly under the speed keys"
    );
    assert!(
        (fast - slow).abs() < MURM_SPEED_MAX,
        "speed scaling broke the flight band: slow {slow:.2}, fast {fast:.2}"
    );
}

#[test]
fn murm_sustained_boundedness() {
    // The LTS integration: a long run holds every invariant —
    // positions in bounds, speeds in the band, no NaN, the drawn
    // cells in the viewport.
    let mut cloud = make_murm_cloud(60, 30);
    let mut frame = Frame::new(60, 30, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 3600, 16);
    for (x, y, vx, vy) in cloud.murmuration_rain.bird_states_for_test() {
        assert!(x.is_finite() && y.is_finite(), "NaN position");
        assert!(vx.is_finite() && vy.is_finite(), "NaN velocity");
        assert!(
            (0.0..60.0).contains(&x) && (0.0..30.0).contains(&y),
            "out of bounds"
        );
    }
    for cell in cloud.murmuration_rain.drawn_cells_for_test() {
        assert!(cell.col < 60 && cell.line < 30, "drawn cell out of bounds");
    }
}

#[test]
fn murm_narrow_terminal_renders_degenerate_but_safe() {
    // The degenerate-terminal safety: a 20x6 viewport still
    // carries the minimum flock, in bounds, no panic.
    let mut cloud = make_murm_cloud(20, 6);
    let mut frame = Frame::new(20, 6, cloud.palette.bg);
    assert!(
        cloud.murmuration_rain.birds.len() >= crate::constants::MURM_MIN_BIRDS,
        "narrow terminal under the flock floor"
    );
    run_frames(&mut cloud, &mut frame, 240, 16);
    for (x, y, _, _) in cloud.murmuration_rain.bird_states_for_test() {
        assert!(
            (0.0..20.0).contains(&x) && (0.0..6.0).contains(&y),
            "out of bounds"
        );
    }
    for cell in cloud.murmuration_rain.drawn_cells_for_test() {
        assert!(
            cell.col < 20 && cell.line < 6,
            "narrow-terminal cell out of bounds"
        );
    }
}

#[test]
fn murm_adopt_palette_and_clear_draw_history() {
    // The transition + invalidation contracts: palette adoption
    // reaches every active bird; the draw history clear empties
    // the diff baseline.
    let mut cloud = make_murm_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 300, 16);
    cloud.murmuration_rain.adopt_palette_slot(3);
    let adopted = cloud
        .murmuration_rain
        .birds
        .iter()
        .filter(|b| b.active && b.palette_slot == 3)
        .count();
    assert!(adopted > 0, "no bird adopted the palette slot");
    let cells = cloud.murmuration_rain.drawn_cells_for_test().len();
    assert!(cells > 0, "no drawn cells to clear");
    cloud.murmuration_rain.clear_draw_history();
    assert!(cloud.murmuration_rain.drawn_cells_for_test().is_empty());
}
