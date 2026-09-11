// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Core quasar-style behavior contracts (NIGHT-research-8):
//! scene resolution, the ignition assembly, the capture economy
//! and its starvation-free bookkeeping, the Kepler shear, the
//! doppler asymmetry, the jet ride, the flare cycle, the drawn
//! bounds, the diff cleanup, pause freeze, style transitions,
//! resize on the burning engine, speed scaling, and the
//! sustained-boundedness integration.

use super::*;

use crate::constants::{
    QUAS_DISK_INNER, QUAS_DOPPLER_RUNG, QUAS_FLARE_CLOCK_MEAN, QUAS_MAX_DISK, QUAS_SIM_TIME_PER_CPS,
};

#[test]
fn quasar_scene_resolves_style_and_fields() {
    let s = crate::scene::get_scene("quasar").expect("quasar scene exists");
    assert_eq!(s.config.rain_style, RainStyle::Quasar);
    assert_eq!(s.config.color, Some("stars"));
    assert_eq!(s.config.charset, Some("braille"));
    assert_eq!(s.config.fps, Some(60.0));
    assert_eq!(s.config.speed, Some(18.0));
    assert_eq!(s.config.density, Some(0.60));
    assert_eq!(
        s.config.glitch_level,
        Some(crate::config::GlitchLevel::None)
    );
    // Style dispatch sanity: structured family, accumulator spawn.
    assert!(!RainStyle::Quasar.is_droplet_family());
    assert!(RainStyle::Quasar.uses_spawn_remainder());
    // Label round-trip (the scene-custom `rain` field surface).
    assert_eq!(RainStyle::Quasar.as_str(), "quasar");
    assert_eq!(RainStyle::from_label("quasar"), Some(RainStyle::Quasar));
    assert_eq!(RainStyle::from_label("Quasar"), Some(RainStyle::Quasar));
    assert_eq!(RainStyle::from_label("agn"), Some(RainStyle::Quasar));
    assert!(RainStyle::valid_labels_hint().contains("quasar"));
    // The scene joins the interactive cycle (a new scene that
    // forgets to join fails the scene tests' coverage contract).
    assert!(crate::scene::SCENE_ORDER.contains(&"quasar"));
}

#[test]
fn quas_engine_assembles_through_ignition() {
    // The birth contract: the ignition completes over its window
    // and the steady engine carries the full disk and both beams.
    let mut cloud = make_quas_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    // The ignition is one-shot choreography: ~8.1 sim-s at the
    // reference dial, then the steady state. Run well past it so
    // the capture economy fills the disk.
    let ignition_frames = (8.1 / DT_SIM_PER_FRAME).ceil() as u32;
    run_frames(&mut cloud, &mut frame, ignition_frames + 900, 16);
    assert!(
        cloud.quasar_rain.lit_for_test(),
        "the engine never lit (t = {:.2})",
        cloud.quasar_rain.ignition_t_for_test()
    );
    let target = cloud.quasar_rain.disk.len();
    assert!(
        target >= QUAS_MAX_DISK.min(64),
        "disk pool too small: {target}"
    );
    let active = cloud.quasar_rain.disk_active_for_test();
    assert!(
        active >= target.saturating_sub(2),
        "the disk did not assemble: {active}/{target}"
    );
    let jets = cloud.quasar_rain.jet_states_for_test();
    assert_eq!(
        jets.len(),
        crate::constants::QUAS_JET_PER_BEAM * 2,
        "the beams never fired"
    );
}

#[test]
fn quas_disk_particles_stay_in_bounds() {
    // The geometry contract: every active disk orbit projects
    // inside the viewport (the caps computed at reset).
    let mut cloud = make_quas_cloud(100, 40);
    let mut frame = Frame::new(100, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 1200, 16);
    let (cols, lines) = (100.0, 40.0);
    for (f, theta, _) in cloud.quasar_rain.disk_states_for_test() {
        let (sin, cos) = theta.sin_cos();
        let x = cloud.quasar_rain.geom.cx + cloud.quasar_rain.geom.axis * f * cos;
        let y = cloud.quasar_rain.geom.cy
            + cloud.quasar_rain.geom.axis * f * sin * crate::constants::QUAS_DISK_TILT;
        assert!((-0.99..cols).contains(&x), "disk x out of bounds: {x:.2}");
        assert!((-0.99..lines).contains(&y), "disk y out of bounds: {y:.2}");
        assert!(
            (QUAS_DISK_INNER - 1e-3..=1.0 + 1e-3).contains(&f),
            "orbit fraction out of band: {f:.3}"
        );
    }
}

#[test]
fn quas_disk_kepler_shear_is_monotone() {
    // Law 2: omega = K / f^1.5 — the inner edge must lap the
    // outer (the shear IS the rotation read).
    use crate::cloud::quasar::particles::disk_omega;
    let mut prev = f32::MAX;
    let mut f = QUAS_DISK_INNER;
    while f <= 1.0 {
        let omega = disk_omega(f);
        assert!(omega < prev, "omega not monotone at f={f:.3}");
        prev = omega;
        f += 0.05;
    }
    let ratio = disk_omega(QUAS_DISK_INNER) / disk_omega(1.0);
    assert!(
        ratio > 4.0 && ratio < 10.0,
        "the shear reads wrong: inner/outer = {ratio:.2}"
    );
}

#[test]
fn quas_doppler_asymmetry_exists() {
    // Law 2's doppler: over a full disk, both limbs exist (some
    // particles approach, some recede) and the los stays bounded.
    use crate::cloud::quasar::particles::los_of;
    let mut approaching = 0;
    let mut receding = 0;
    let mut n = 0;
    let mut cloud = make_quas_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 900, 16);
    for (_, theta, _) in cloud.quasar_rain.disk_states_for_test() {
        let los = los_of(theta);
        assert!((-1.0..=1.0).contains(&los), "los out of band: {los:.3}");
        if los > QUAS_DOPPLER_RUNG {
            approaching += 1;
        } else if los < -QUAS_DOPPLER_RUNG {
            receding += 1;
        }
        n += 1;
    }
    assert!(n > 10, "the disk is too thin to read: {n}");
    assert!(
        approaching > 0 && receding > 0,
        "no doppler asymmetry: {approaching} approaching / {receding} receding of {n}"
    );
}

#[test]
fn quas_infall_absorbs_and_feeds_without_starvation() {
    // Law 3 + the DNA starvation lesson: captures fire, the disk
    // grows from them, and the infall population turns over
    // honestly (an absorbed streamer frees its slot — the spawn
    // gate reads the true count, the soup never starves).
    let mut cloud = make_quas_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 900, 16);
    assert!(
        cloud.quasar_rain.absorptions_for_test() > 10,
        "the rain never fed the engine"
    );
    assert!(
        cloud.quasar_rain.disk_active_for_test() > 0,
        "no disk grew from the captures"
    );
    // The honest bookkeeping: the active count matches the pool's
    // actual active particles (the counter-starvation contract).
    let actual = cloud.quasar_rain.infall.iter().filter(|p| p.active).count();
    assert_eq!(
        cloud.quasar_rain.infall_active_for_test(),
        actual,
        "the infall counter drifted (the starvation bug class)"
    );
    // Steady state: the infall population sustains (the pool is
    // at or near its target, not decayed to zero).
    assert!(
        cloud.quasar_rain.infall_active_for_test() >= 3,
        "the infall starved to a silent sky"
    );
}

#[test]
fn quas_jets_ride_and_recycle() {
    // Law 4: both beams stream — s rides [0, 1) and the recycling
    // never accumulates (the wrap holds).
    let mut cloud = make_quas_cloud(100, 40);
    let mut frame = Frame::new(100, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 900, 16);
    let jets = cloud.quasar_rain.jet_states_for_test();
    assert_eq!(jets.len(), crate::constants::QUAS_JET_PER_BEAM * 2);
    let up = jets.iter().filter(|(_, side)| *side > 0.0).count();
    let down = jets.iter().filter(|(_, side)| *side < 0.0).count();
    assert_eq!(up, down, "the beams are asymmetric");
    for (s, _) in &jets {
        assert!((0.0..1.0).contains(s), "jet s out of band: {s:.3}");
        assert!(s.is_finite(), "NaN jet position");
    }
}

#[test]
fn quas_feed_flare_fires() {
    // Law 1's drama: the clock fires over a long run; a fired
    // flare arms the knot, locks the pulse and re-rolls the core.
    let mut cloud = make_quas_cloud(100, 40);
    let mut frame = Frame::new(100, 40, cloud.palette.bg);
    // Run past the ignition, then well past the flare mean.
    let settle = (8.1 / DT_SIM_PER_FRAME).ceil() as u32;
    run_frames(&mut cloud, &mut frame, settle + 1500, 16);
    assert!(
        cloud.quasar_rain.flares_for_test() > 0,
        "the feed-flare clock never fired"
    );
    // The deterministic arm: plant the clock, watch the fire.
    cloud.quasar_rain.arm_flare_for_test(0.001);
    run_frames(&mut cloud, &mut frame, 2, 16);
    assert!(
        cloud.quasar_rain.knot_for_test().is_some(),
        "the flare did not launch a knot"
    );
    let pulse = cloud.quasar_rain.pulse_for_test();
    assert!(pulse > 0.8, "the flare did not lock the pulse: {pulse:.3}");
    assert!(
        (0.0..=1.0).contains(&pulse),
        "the pulse left its band: {pulse:.3}"
    );
}

#[test]
fn quas_flare_clock_mean_is_bounded() {
    // The re-arm contract: the clock re-arms inside its band (the
    // startle's variance-band precedent).
    let mut cloud = make_quas_cloud(100, 40);
    let mut frame = Frame::new(100, 40, cloud.palette.bg);
    let settle = (8.1 / DT_SIM_PER_FRAME).ceil() as u32;
    run_frames(&mut cloud, &mut frame, settle + 3000, 16);
    // At the mean, ~3-4 flares fire over 30 s of sim time.
    let flares = cloud.quasar_rain.flares_for_test();
    assert!(
        (1..=12).contains(&flares),
        "flare rate out of band: {flares}"
    );
    let _ = QUAS_FLARE_CLOCK_MEAN;
}

#[test]
fn quas_drawn_cells_stay_in_bounds() {
    // The renderer's floor: every cell the draw pass claims sits
    // inside the viewport (halo, disk, jets, infall, core).
    let mut cloud = make_quas_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 300, 16);
    for cell in cloud.quasar_rain.drawn_cells_for_test() {
        assert!(cell.col < 80, "drawn cell col out of bounds: {}", cell.col);
        assert!(
            cell.line < 40,
            "drawn cell line out of bounds: {}",
            cell.line
        );
    }
}

#[test]
fn quas_repaints_without_residue() {
    // The diff-cleanup contract: a force-redraw + continued
    // frames keeps the drawn population in the same order (no
    // stale cells, no residue ghosts).
    let mut cloud = make_quas_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 400, 16);
    let baseline = cloud.quasar_rain.drawn_cells_for_test().len();
    cloud.clear_redraw_flags_for_test();
    cloud.force_draw_everything();
    run_frames(&mut cloud, &mut frame, 30, 16);
    let after = cloud.quasar_rain.drawn_cells_for_test().len();
    assert!(
        after > baseline / 2 && after < baseline * 2 + 64,
        "drawn population unstable: {baseline} -> {after}"
    );
}

#[test]
fn quas_pause_freezes_the_engine() {
    // The family pause contract: with pause armed the advance
    // pass integrates nothing — every orbit, beam and clock holds
    // exactly.
    let mut cloud = make_quas_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 400, 16);
    cloud.pause = true;
    cloud.pause_time = Some(Instant::now());
    let before = (
        cloud.quasar_rain.disk_states_for_test(),
        cloud.quasar_rain.jet_states_for_test(),
        cloud.quasar_rain.ignition_t_for_test(),
    );
    run_frames(&mut cloud, &mut frame, 30, 16);
    let after = (
        cloud.quasar_rain.disk_states_for_test(),
        cloud.quasar_rain.jet_states_for_test(),
        cloud.quasar_rain.ignition_t_for_test(),
    );
    assert_eq!(before.0, after.0, "the disk turned while paused");
    assert_eq!(before.1, after.1, "the beams streamed while paused");
    assert_eq!(before.2, after.2, "the ignition advanced while paused");
}

#[test]
fn quas_style_transition_round_trip() {
    // The scene-cycle contract: switching away empties the
    // engine; switching back re-arms the ignition — the birth
    // replays and the engine returns.
    let mut cloud = make_quas_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 500, 16);
    assert!(!cloud.quasar_rain.drawn_cells_for_test().is_empty());

    cloud.transition_rain_style(RainStyle::Glyph);
    assert_eq!(
        cloud.quasar_rain.active_count(),
        0,
        "style exit left the engine burning"
    );

    cloud.transition_rain_style(RainStyle::Quasar);
    assert!(
        !cloud.quasar_rain.lit_for_test(),
        "re-entry skipped the ignition"
    );
    let mut frame2 = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame2, 900, 16);
    assert!(
        cloud.quasar_rain.lit_for_test(),
        "the engine never returned after re-entry"
    );
}

#[test]
fn quas_resize_keeps_the_burning_engine() {
    // The DNA resize contract: a pure resize on a lit engine
    // rebuilds the steady state immediately (no second ignition,
    // no disassembled disk).
    let mut cloud = make_quas_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 900, 16);
    assert!(cloud.quasar_rain.lit_for_test());
    cloud.reset(100, 50);
    assert!(
        cloud.quasar_rain.lit_for_test(),
        "the resize re-ignited the engine"
    );
    assert_eq!(
        cloud.quasar_rain.disk_active_for_test(),
        cloud.quasar_rain.disk.len(),
        "the resize disassembled the disk"
    );
    let mut frame2 = Frame::new(100, 50, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame2, 60, 16);
    assert!(!cloud.quasar_rain.drawn_cells_for_test().is_empty());
}

#[test]
fn quas_speed_keys_scale_the_ignition() {
    // The family speed contract: the birth rides sim-time, so a
    // doubled clock doubles the ignition's progress over the same
    // wall frames (deterministic — the clock is pure dt_sim).
    let run = |cps: f32| -> f32 {
        let mut cloud = make_quas_cloud(80, 40);
        cloud.set_chars_per_sec(cps);
        let mut frame = Frame::new(80, 40, cloud.palette.bg);
        run_frames(&mut cloud, &mut frame, 120, 16);
        cloud.quasar_rain.ignition_t_for_test()
    };
    let slow = run(18.0);
    let fast = run(36.0);
    assert!(
        (fast - slow * 2.0).abs() < 0.05,
        "the ignition clock did not scale: slow {slow:.3}, fast {fast:.3}"
    );
    let _ = QUAS_SIM_TIME_PER_CPS;
}

#[test]
fn quas_sustained_boundedness() {
    // The LTS integration: a long run holds every invariant —
    // finite states, in-band orbits, no NaN, the drawn cells in
    // the viewport, the counters honest.
    let mut cloud = make_quas_cloud(60, 30);
    let mut frame = Frame::new(60, 30, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 3600, 16);
    for (f, theta, charge) in cloud.quasar_rain.disk_states_for_test() {
        assert!(f.is_finite() && theta.is_finite(), "NaN disk state");
        assert!(
            (QUAS_DISK_INNER - 1e-3..=1.0 + 1e-3).contains(&f),
            "orbit drift"
        );
        assert!(
            (0.0..=1.0).contains(&charge),
            "charge out of band: {charge:.3}"
        );
    }
    for (s, _) in cloud.quasar_rain.jet_states_for_test() {
        assert!(s.is_finite() && (0.0..1.0).contains(&s), "jet drift");
    }
    for (f, _) in cloud.quasar_rain.infall_states_for_test() {
        assert!(f.is_finite() && f > 0.99, "infall out of band: {f:.3}");
    }
    for cell in cloud.quasar_rain.drawn_cells_for_test() {
        assert!(cell.col < 60 && cell.line < 30, "drawn cell out of bounds");
    }
    let actual = cloud.quasar_rain.infall.iter().filter(|p| p.active).count();
    assert_eq!(
        cloud.quasar_rain.infall_active_for_test(),
        actual,
        "counter drift over the long run"
    );
}

#[test]
fn quas_narrow_terminal_renders_degenerate_but_safe() {
    // The degenerate-terminal safety: a 20x6 viewport still
    // carries the engine, in bounds, no panic.
    let mut cloud = make_quas_cloud(20, 6);
    let mut frame = Frame::new(20, 6, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 300, 16);
    for cell in cloud.quasar_rain.drawn_cells_for_test() {
        assert!(
            cell.col < 20 && cell.line < 6,
            "narrow-terminal cell out of bounds"
        );
    }
    assert!(
        cloud.quasar_rain.disk.len() >= crate::constants::QUAS_MIN_DISK,
        "narrow terminal under the disk floor"
    );
}

#[test]
fn quas_adopt_palette_and_clear_draw_history() {
    // The transition + invalidation contracts: palette adoption
    // reaches every active particle; the draw history clear
    // empties the diff baseline.
    let mut cloud = make_quas_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 400, 16);
    cloud.quasar_rain.adopt_palette_slot(3);
    let adopted = cloud
        .quasar_rain
        .disk
        .iter()
        .chain(cloud.quasar_rain.jets.iter())
        .filter(|p| p.active && p.palette_slot == 3)
        .count();
    assert!(adopted > 0, "no particle adopted the palette slot");
    let cells = cloud.quasar_rain.drawn_cells_for_test().len();
    assert!(cells > 0, "no drawn cells to clear");
    cloud.quasar_rain.clear_draw_history();
    assert!(cloud.quasar_rain.drawn_cells_for_test().is_empty());
}

// ─── NIGHT-research-14: the soft-light contracts ─────────────────────

#[test]
fn quas_soft_head_cap_steps_core_down_one_rung() {
    // The NIGHT-research-14 soft-light cap, pinned on the pure
    // function (the black hole's soft_head_cap precedent): Core
    // steps down to Hot (the retired standing white blend), every
    // other level passes through unchanged — the cap is a
    // ceiling, not a regrade, so the doppler swing, the charge
    // boost, the ignition ramp and the step-up ladders keep
    // shaping the band below it.
    use crate::cloud::type_rain::monolith::BrightnessLevel;
    use crate::cloud::type_rain::quasar::draw::{level_rank, soft_head_level};
    assert_eq!(
        level_rank(soft_head_level(BrightnessLevel::Core)),
        level_rank(BrightnessLevel::Hot),
        "Core must step down to the soft warm ceiling"
    );
    for level in [
        BrightnessLevel::Hot,
        BrightnessLevel::Mid,
        BrightnessLevel::Dim,
        BrightnessLevel::Ghost,
    ] {
        assert_eq!(
            level_rank(soft_head_level(level)),
            level_rank(level),
            "the cap must pass {level:?} through unchanged"
        );
    }
}

#[test]
fn quas_disk_composes_soft_warm_never_core() {
    // The NIGHT-research-14 disk composition, pinned on the pure
    // ladders exactly as the draw site composes them (the black
    // hole ball-surface precedent): the radial temperature ladder
    // through BOTH step-ups (the doppler's approaching limb and
    // the fresh-feed charge — the two standing sources that used
    // to lift the inner disk to Core), the ignition cap at the
    // full law, and the soft-light ceiling — the composed level
    // never lands Core at any temperature, line-of-sight or
    // charge state. The standing surface's absolute ceiling is
    // the soft warm rung.
    use crate::cloud::type_rain::monolith::BrightnessLevel;
    use crate::cloud::type_rain::quasar::draw::{
        cap_level, disk_level, level_rank, soft_head_level, step_down_level, step_up_level,
    };
    use crate::constants::QUAS_DOPPLER_RUNG;
    let full_law = cap_level_rank_steady();
    for i in 0..=200 {
        let t = i as f32 / 200.0;
        for los in [-1.0_f32, -0.8, -0.5, 0.0, 0.5, 0.8, 1.0] {
            for charged in [false, true] {
                let mut level = disk_level(t);
                if los > QUAS_DOPPLER_RUNG {
                    level = step_up_level(level);
                } else if los < -QUAS_DOPPLER_RUNG {
                    level = step_down_level(level);
                }
                if charged {
                    level = step_up_level(level);
                }
                level = cap_level(level, full_law);
                level = soft_head_level(level);
                assert!(
                    level_rank(level) <= level_rank(BrightnessLevel::Hot),
                    "the composed disk must never land Core (t {t}, los {los}, charged {charged})"
                );
            }
        }
    }
}

#[test]
fn quas_jet_collar_composes_soft_warm_core_only_in_the_knot_window() {
    // The NIGHT-research-14 jet composition (the black hole
    // stream-heads precedent): the energy ladder under the
    // soft-light ceiling never lands Core along the beam — the
    // launch collar reads the warm ceiling. The knot's step-up
    // rides AFTER the cap, so Core appears only inside the knot's
    // traveling window (QUAS_KNOT_W): the beam's one transient
    // flash, never a standing structure.
    use crate::cloud::type_rain::monolith::BrightnessLevel;
    use crate::cloud::type_rain::quasar::draw::{
        jet_level, level_rank, soft_head_level, step_up_level,
    };
    use crate::constants::QUAS_KNOT_W;
    let knot = 0.5_f32;
    for i in 0..=200 {
        let s = i as f32 / 200.0;
        let capped = soft_head_level(jet_level(s));
        assert!(
            level_rank(capped) <= level_rank(BrightnessLevel::Hot),
            "the capped beam must never land Core (s {s})"
        );
    }
    // Inside the knot's window the step-up is applied (the draw
    // site's condition) and the pulse is allowed to flash: at
    // least one position of the band lifts Hot to Core.
    let mut flashed = false;
    let start = ((knot - QUAS_KNOT_W) * 100.0) as i32;
    let end = ((knot + QUAS_KNOT_W) * 100.0) as i32;
    for h in (start..end).step_by(2) {
        let s = h as f32 / 100.0;
        if level_rank(step_up_level(soft_head_level(jet_level(s))))
            == level_rank(BrightnessLevel::Core)
        {
            flashed = true;
            break;
        }
    }
    assert!(flashed, "the knot pulse must flash Core inside its window");
}

#[test]
fn quas_core_cell_reads_soft_warm_standing_flashes_core_on_flare() {
    // The NIGHT-research-14 core-cell ruling (the owner's report:
    // the permanently Core-bright center glyph — the single
    // brightest standing cell in the codebase — strained his
    // eyes): the engine's heart reads the soft warm ceiling
    // standing and burns Core only inside the flare window (the
    // event-gated flash that rides the glyph re-roll moment). The
    // factor breathes 0.85 to 1.0 on the pulse — never
    // static-flat, never a strain.
    use crate::cloud::type_rain::monolith::BrightnessLevel;
    use crate::cloud::type_rain::quasar::draw::{core_cell_factor, core_cell_level, level_rank};
    assert_eq!(
        level_rank(core_cell_level(false)),
        level_rank(BrightnessLevel::Hot),
        "the standing core cell must read the soft warm ceiling"
    );
    assert_eq!(
        level_rank(core_cell_level(true)),
        level_rank(BrightnessLevel::Core),
        "the flare window is the core cell's one Core flash"
    );
    for i in 0..=100 {
        let pulse = i as f32 / 100.0;
        let factor = core_cell_factor(pulse);
        assert!(
            (0.85..=1.0).contains(&factor),
            "the core cell's breathing must stay in [0.85, 1.0] (pulse {pulse})"
        );
    }
    assert_eq!(
        core_cell_factor(-0.5),
        0.85,
        "the clamp must floor below range"
    );
    assert_eq!(core_cell_factor(1.7), 1.0, "the clamp must cap above range");
}

/// The ignition cap rank at the full law (Steady) — the worst
/// case for Core the disk composition can reach.
fn cap_level_rank_steady() -> u8 {
    use crate::cloud::type_rain::quasar::draw::ignition_cap_rank;
    use crate::cloud::type_rain::quasar::ignition::IgnitionPhase;
    ignition_cap_rank(IgnitionPhase::Steady, 1.0)
}
