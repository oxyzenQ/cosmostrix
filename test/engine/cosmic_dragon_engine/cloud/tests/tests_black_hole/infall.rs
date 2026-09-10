// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-special-1 stage 3 tests: the glyph infall — the rain that
//! becomes the accretion material. Covers the gravitational-field
//! contracts (straight fall beyond the influence edge, the elegant
//! bend toward the hole inside it), the capture contracts (the
//! accretion brake decays tangential speed, fly-bys tighten into
//! inspirals, the plunge accelerates as it falls), the horizon
//! contract (a mote crossing the event horizon is eaten — the empty
//! core is never painted), the pool contracts (one lane per column,
//! deficit-bounded spawn with the formation gate, clean resize), the
//! kinetic-heat speed ladder, and the dynamic-screen-size draw
//! bounds.

use super::*;
use crate::cloud::type_rain::black_hole::black_hole::level_rank;
use crate::cloud::type_rain::black_hole::infall::{
    advance_infall_mote, level_for_speed, InfallMote, InfallStream,
};
use crate::cloud::type_rain::black_hole::ring::proximity_level;
use crate::cloud::type_rain::monolith::BrightnessLevel;

/// A pinned infall mote with a controlled state (the free-function
/// physics contracts run on these, exactly like the ring tests'
/// `pinned_mote`): a generous lifetime so only the physics can retire
/// it, and the fall speed of a fresh spawn.
fn pinned_infall(x: f32, y: f32, vx: f32, vy: f32) -> InfallMote {
    let mut m = InfallMote::vacant();
    m.active = true;
    m.x = x;
    m.y = y;
    m.vx = vx;
    m.vy = vy;
    m.lifetime = 60.0;
    m
}

/// A generous exit envelope for the pinned motes (the departure
/// contract gets its own pinned test below; the physics tests must
/// never hit it).
const PIN_EXIT_W: f32 = 8.0;
const PIN_EXIT_H: f32 = 8.0;

/// Step a pinned mote through `sim_secs` of the field (the function
/// sub-steps internally — one call is one contiguous flight segment).
fn fly(m: &mut InfallMote, sim_secs: f32) -> bool {
    advance_infall_mote(m, sim_secs, PIN_EXIT_W, PIN_EXIT_H)
}

/// Drive the cloud to the steady state AND past the rain's slow fill:
/// the stage-4 trickle reaches its lane budget over several seconds
/// (the calm ramp is part of the read, not a bug), so the infall
/// contracts must observe the filled sky, not the arrival. The 220
/// shared frames carry the formation intro; 600 more (9.6 s of rain
/// at the harness clock) let the drizzle settle at its sparse target.
fn run_frames_to_rain_steady(cloud: &mut Cloud, frame: &mut Frame) {
    run_frames_to_steady(cloud, frame);
    run_frames(cloud, frame, 600, 16);
}

#[test]
fn black_hole_infall_spawns_rain_after_formation() {
    // The stage-3 contract: after the formation intro the third pool
    // fills — glyphs fall over the system (the ambient layer), and
    // every active mote is in flight (inside the envelope, moving).
    // The long window also catches mid-life motes: a captured glyph
    // whipping its inspiral can travel upward for a stretch, so the
    // flight check is the mote's speed, not the sign of vy.
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames_to_rain_steady(&mut cloud, &mut frame);

    let active = cloud.black_hole_rain.active_infall_for_test();
    assert!(active > 0, "the infall must spawn glyphs (got {active})");
    let motes = cloud.black_hole_rain.infall_motes_for_test();
    assert_eq!(
        motes.len(),
        cols as usize,
        "one lane per column (the family pool contract)"
    );
    let mut flying = 0;
    for m in motes.iter().filter(|m| m.active) {
        let speed_sq = m.vx * m.vx + m.vy * m.vy;
        assert!(
            speed_sq > 1.0e-6,
            "an active glyph must be in flight (vx {} vy {})",
            m.vx,
            m.vy
        );
        assert!(
            m.y > -(crate::constants::BLACK_HOLE_INFALL_INFLUENCE_FRACTION * 2.0),
            "a spawned glyph must have entered the scene (y {})",
            m.y
        );
        flying += 1;
    }
    assert_eq!(flying, active, "the active count must match the pool");
}

#[test]
fn black_hole_infall_respects_the_formation_gate() {
    // No rain falls into a half-born hole: through the whole ~3.1 s
    // birth sequence the infall pool stays empty (the ring's own
    // formation-gate contract, carried onto the third pool).
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 100, 16);
    assert!(
        !cloud.black_hole_rain.formed_for_test(),
        "the harness must still be inside the formation intro"
    );
    assert_eq!(
        cloud.black_hole_rain.active_infall_for_test(),
        0,
        "no glyphs may fall before the horizon blooms"
    );
}

#[test]
fn black_hole_infall_far_rain_falls_straight() {
    // Beyond the influence edge the field is zero: the ambient rain
    // keeps its straight matrix line — no bend, no drift, no
    // acceleration (the read the scene keeps at its edges).
    let fall = crate::constants::BLACK_HOLE_INFALL_FALL_SPEED;
    let mut m = pinned_infall(5.0, -3.0, 0.0, fall);
    let x0 = m.x;
    let y0 = m.y;
    let retired = fly(&mut m, 2.0);
    assert!(!retired, "far rain must survive its flight");
    assert!(
        (m.x - x0).abs() < 1.0e-4,
        "far rain must not drift sideways ({} -> {})",
        x0,
        m.x
    );
    assert!(
        (m.y - (y0 + fall * 2.0)).abs() < 1.0e-3,
        "far rain must fall at the constant fall speed ({} -> {})",
        y0,
        m.y
    );
    assert!(
        (m.vy - fall).abs() < 1.0e-4,
        "no acceleration beyond the field (vy {})",
        m.vy
    );
}

#[test]
fn black_hole_infall_bends_toward_the_hole() {
    // The elegant bend: inside the influence radius gravity curves
    // the fall toward the hole — the mote's horizontal position
    // pulls inward and its velocity gains a component pointing at
    // the center (the trajectory read of light passing a deep well).
    // The follow-up legs then prove the bend resolves into capture.
    let fall = crate::constants::BLACK_HOLE_INFALL_FALL_SPEED;
    let mut m = pinned_infall(1.7, -1.4, 0.0, fall);
    let x0 = m.x;
    // Phase 1 — half a second, mid-field: the mote is still above
    // the hole's latitude, far outside the horizon, so only the bend
    // can show.
    let retired = fly(&mut m, 0.5);
    assert!(!retired, "the bending mote must still be in flight");
    assert!(
        m.x < x0 - 0.02,
        "the mote must bend toward the hole (x {} -> {})",
        x0,
        m.x
    );
    assert!(
        m.vx < 0.0,
        "the velocity must point at the hole (vx {} at x {})",
        m.vx,
        m.x
    );
    // Phase 2 — the capture resolves: the launch energy is negative
    // (sub-circular) and the brake only decays it further, so the
    // horizon must eat the mote within the follow-up legs.
    let mut gone = retired;
    for _ in 0..16 {
        gone = gone || fly(&mut m, 0.5);
        if gone {
            break;
        }
    }
    assert!(gone, "the bent mote must end eaten (x {} y {})", m.x, m.y);
}

#[test]
fn black_hole_infall_capture_inspirals_into_the_hole() {
    // The accretion-brake contract: a mote crossing the capture zone
    // with tangential speed does not fly by — the brake decays its
    // sideways drift lap by lap (the radius never climbs back above
    // its sub-circular launch) until the horizon eats it. The
    // owner's wording: falling glyphs bend elegantly into the core.
    let mut m = pinned_infall(
        1.3,
        0.0,
        0.0,
        crate::constants::BLACK_HOLE_INFALL_FALL_SPEED,
    );
    let r0 = (m.x * m.x + m.y * m.y).sqrt();
    // The launch is sub-circular (the circular speed at 1.30 outer
    // radii is sqrt(G / r) = 2.0 against the fall speed 1.32), so the
    // mote's orbital energy is negative — bound from the first step,
    // and every sampled radius must sit at or under the launch.
    let mut retired = false;
    for _ in 0..10 {
        retired = retired || fly(&mut m, 1.0);
        if retired {
            break;
        }
        let r = (m.x * m.x + m.y * m.y).sqrt();
        assert!(
            r <= r0 + 0.05,
            "the inspiral must not climb above the launch (r {r} vs {r0})"
        );
    }
    assert!(retired, "the captured mote must be eaten");
    assert!(!m.active, "a retired mote must leave the pool's active set");
}

#[test]
fn black_hole_infall_horizon_eats_the_plunge() {
    // The plunge contract: a dead-center fall crosses the photon ring
    // and is eaten at the event horizon — absorbed, never surviving
    // inside the empty core.
    let mut m = pinned_infall(
        0.05,
        -2.2,
        0.0,
        crate::constants::BLACK_HOLE_INFALL_FALL_SPEED,
    );
    let mut retired = false;
    for _ in 0..16 {
        retired = retired || fly(&mut m, 0.5);
        if retired {
            break;
        }
        let r = (m.x * m.x + m.y * m.y).sqrt();
        assert!(
            r >= crate::constants::BLACK_HOLE_CORE_FRACTION,
            "a living mote is never inside the horizon (r {})",
            r
        );
    }
    assert!(retired, "the plunge must end at the horizon");
}

#[test]
fn black_hole_infall_fall_gains_speed() {
    // Kepler's second law, the exchange of height for speed: the fall
    // through the inner field accelerates the mote (its speed grows
    // while gravity does work on it — the kinetic-heat source). A
    // near-radial leg: the brake spares the plunge (its own contract
    // below), so the acceleration reads clean.
    let fall = crate::constants::BLACK_HOLE_INFALL_FALL_SPEED;
    let mut m = pinned_infall(0.3, -1.9, 0.0, fall);
    let speed0 = (m.vx * m.vx + m.vy * m.vy).sqrt();
    let speed1;
    {
        let retired = fly(&mut m, 0.6);
        assert!(!retired, "the mote must still be in flight");
        speed1 = (m.vx * m.vx + m.vy * m.vy).sqrt();
    }
    assert!(
        speed1 > speed0 + 0.25,
        "the fall must accelerate ({} -> {})",
        speed0,
        speed1
    );
}

#[test]
fn black_hole_infall_departed_motes_despawn() {
    // The envelope contract: a mote that leaves the spawn envelope
    // plus the exit margin is dropped (its streak is cleared by the
    // diff cleanup on the next frame).
    let mut m = pinned_infall(4.5, 4.5, 3.0, 3.0);
    let retired = advance_infall_mote(&mut m, 0.2, 4.0, 4.0);
    assert!(retired, "the departed mote must despawn");
    assert!(!m.active);
    assert_eq!(m.trail_len, 0, "the streak must retire with the mote");
}

#[test]
fn black_hole_infall_speed_ladder_grades_kinetic_heat() {
    // The kinetic-heat ladder: slow distant rain Ghost, the calm
    // entry Ghost (stage 4: a fresh drop at the fall speed reads dim —
    // only the fall's own acceleration lifts the ladder), the
    // accelerating fall Mid, the approach Hot, the whip Core.
    assert!(matches!(level_for_speed(0.8), BrightnessLevel::Ghost));
    assert!(matches!(
        level_for_speed(crate::constants::BLACK_HOLE_INFALL_FALL_SPEED),
        BrightnessLevel::Ghost
    ));
    assert!(matches!(level_for_speed(1.8), BrightnessLevel::Mid));
    assert!(matches!(level_for_speed(2.5), BrightnessLevel::Hot));
    assert!(matches!(level_for_speed(3.6), BrightnessLevel::Core));
}

#[test]
fn black_hole_infall_kinetic_heat_composes_with_proximity() {
    // The composition contract: the whip near the shadow lands deep
    // white (speed-graded base + the proximity ladder's two-rung
    // bump), the slow far rain reads Ghost through the fade ladder.
    let whip = proximity_level(BrightnessLevel::Core, 0.9);
    assert!(matches!(whip, BrightnessLevel::Core));
    let far = proximity_level(BrightnessLevel::Mid, 2.6);
    assert!(matches!(far, BrightnessLevel::Ghost));
}

#[test]
fn black_hole_infall_never_paints_the_empty_core() {
    // The stage-1 contract holds for the rain: no infall glyph draws
    // inside the empty core — the head skips the draw below the
    // horizon's radius and the absorption retires the mote exactly
    // there, so the rain's whole drawn footprint (heads and trails)
    // stays at or outside the core. Scoped to the infall's own trail
    // cells: the ring's near side intentionally crosses in front of
    // the core (the approved crossing read), so the unified drawn set
    // is not the right observable for this contract.
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames_to_steady(&mut cloud, &mut frame);
    // Long enough for many rain lives: spawns, captures, plunges.
    run_frames(&mut cloud, &mut frame, 420, 16);

    let unit = (cols as f32 / 4.0).min(lines as f32 / 2.0);
    let outer_r = unit * crate::constants::BLACK_HOLE_BALL_FRACTION;
    let core_r = outer_r * crate::constants::BLACK_HOLE_CORE_FRACTION;
    let cx = ((cols - 1) / 2) as f32;
    let cy = ((lines - 1) / 2) as f32;
    // One cell of tolerance for the rounding at the horizon's edge
    // (a head's physics position sits at the core radius; the cell
    // center can land half a cell inside it).
    let floor = core_r - 1.0;
    let mut checked = 0;
    for m in cloud.black_hole_rain.infall_motes_for_test() {
        if !m.active {
            continue;
        }
        // A living mote is never inside the horizon (the physics
        // contract — checked here as the draw skip's twin).
        let r = (m.x * m.x + m.y * m.y).sqrt();
        assert!(
            r >= crate::constants::BLACK_HOLE_CORE_FRACTION,
            "a living glyph is never inside the horizon (r {r})"
        );
        // The drawn footprint: every trail cell (past heads) sits at
        // or outside the core radius in screen distance.
        for t in 0..m.trail_len as usize {
            let (tc, tl) = m.trail[t];
            let dx = (tc as f32 - cx) / 2.0;
            let dy = tl as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            assert!(
                dist >= floor,
                "a rain trail cell landed inside the empty core (dist {dist} < {floor} at {tc}, {tl})"
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "the rain must have a drawn footprint");
}

#[test]
fn black_hole_infall_drawn_cells_stay_in_bounds() {
    // The dynamic-screen-size contract: every drawn rain cell is
    // inside the viewport (bounds-checked per cell — a live resize
    // window never paints outside).
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames_to_steady(&mut cloud, &mut frame);
    run_frames(&mut cloud, &mut frame, 420, 16);
    for cell in cloud.black_hole_rain.drawn_cells_for_test() {
        assert!(cell.col < cols, "col {} out of bounds", cell.col);
        assert!(cell.line < lines, "line {} out of bounds", cell.line);
    }
}

#[test]
fn black_hole_infall_resizes_cleanly() {
    // The resize contract: the pool rebuilds to the new lane count,
    // everything vacant, and the rain re-fills on the new geometry
    // (the whole layer is fraction-based — no absolute coordinates
    // survive the resize).
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames_to_rain_steady(&mut cloud, &mut frame);

    let (cols2, lines2) = (90, 30);
    let mut frame2 = Frame::new(cols2, lines2, cloud.palette.bg);
    cloud.reset(cols2, lines2);
    cloud.clear_redraw_flags_for_test();
    assert_eq!(
        cloud.black_hole_rain.infall_motes_for_test().len(),
        cols2 as usize,
        "the pool must rebuild to the new lane count"
    );
    assert_eq!(
        cloud.black_hole_rain.active_infall_for_test(),
        0,
        "the pool must rebuild vacant"
    );
    // The resize keeps the formed flag — the rain re-fills without
    // replaying the birth sequence.
    assert!(cloud.black_hole_rain.formed_for_test());
    run_frames_to_rain_steady(&mut cloud, &mut frame2);
    assert!(
        cloud.black_hole_rain.active_infall_for_test() > 0,
        "the rain must re-fill on the resized geometry"
    );
}

#[test]
fn black_hole_infall_target_respects_the_cap() {
    // The ambient read: the rain's active target stays a minority of
    // the pool at every density (the hole is the hero, the rain the
    // weather around it).
    let lanes = 120usize;
    for density in [0.05, 0.55, 1.2, 2.5, 5.0] {
        let target = InfallStream::target_active_for_test(lanes, density);
        assert!(target >= 1, "a non-empty pool always hosts one glyph");
        assert!(
            (target as f32 / lanes as f32)
                <= crate::constants::BLACK_HOLE_INFALL_ACTIVE_MAX + 1.0e-6,
            "the cap must hold (target {target} of {lanes} at density {density})"
        );
    }
}

#[test]
fn black_hole_infall_survives_style_transition() {
    // The transition contract: leaving the style wipes the rain pool
    // (no falling ghosts in another scene), re-entry replays the
    // formation and the rain re-fills — no stale pool, no zombies.
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames_to_rain_steady(&mut cloud, &mut frame);
    assert!(cloud.black_hole_rain.active_infall_for_test() > 0);

    cloud.transition_rain_style(RainStyle::Vortex);
    assert_eq!(
        cloud.black_hole_rain.active_infall_for_test(),
        0,
        "style exit must wipe the rain pool"
    );

    cloud.transition_rain_style(RainStyle::BlackHole);
    // Re-entry replays the formation intro (the rain gate reopens
    // when the hole is whole) — fast-forward to the filled sparse
    // steady state.
    run_frames_to_rain_steady(&mut cloud, &mut frame);
    assert!(
        cloud.black_hole_rain.active_infall_for_test() > 0,
        "the rain must re-form after the style transition"
    );
}

#[test]
fn black_hole_infall_pause_freezes_the_rain() {
    // The pause contract: a zero resume_blend freezes the fall (the
    // family's anti-teleport clock, carried onto the third pool).
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames_to_rain_steady(&mut cloud, &mut frame);
    let before: Vec<(f32, f32)> = cloud
        .black_hole_rain
        .infall_motes_for_test()
        .iter()
        .filter(|m| m.active)
        .map(|m| (m.x, m.y))
        .collect();
    assert!(!before.is_empty());

    cloud.resume_blend = 0.0;
    run_frames(&mut cloud, &mut frame, 30, 16);
    let after: Vec<(f32, f32)> = cloud
        .black_hole_rain
        .infall_motes_for_test()
        .iter()
        .filter(|m| m.active)
        .map(|m| (m.x, m.y))
        .collect();
    // The same motes (same count at least — no spawns either under a
    // frozen clock's zero dt) at the same positions.
    assert_eq!(after.len(), before.len(), "no motes may spawn or retire");
    for (a, b) in before.iter().zip(after.iter()) {
        assert!((a.0 - b.0).abs() < 1.0e-5, "x moved under pause");
        assert!((a.1 - b.1).abs() < 1.0e-5, "y moved under pause");
    }
    cloud.resume_blend = 1.0;
}

#[test]
fn black_hole_infall_speed_keys_scale_sim_time() {
    // The speed-key contract: double the chars-per-second doubles the
    // fall distance over the same wall seconds (the whole field runs
    // on one clock — trajectory shapes are invariant, pacing is not).
    let fall = crate::constants::BLACK_HOLE_INFALL_FALL_SPEED;
    let mut slow = pinned_infall(4.0, -3.0, 0.0, fall);
    let mut fast = pinned_infall(4.0, -3.0, 0.0, fall);
    // The stream's sim-time coupling: dt_sim = dt_wall x cps x
    // SIM_TIME_PER_CPS — 1.0 wall second at 12 cps vs 24 cps.
    let per_cps = crate::constants::BLACK_HOLE_INFALL_SIM_TIME_PER_CPS;
    let dt_slow = 1.0 * 12.0 * per_cps;
    let dt_fast = 1.0 * 24.0 * per_cps;
    fly(&mut slow, dt_slow);
    fly(&mut fast, dt_fast);
    assert!(
        (fast.y - slow.y).abs() > fall * 0.4,
        "the speed keys must scale the fall (slow y {} vs fast y {})",
        slow.y,
        fast.y
    );
}

#[test]
fn black_hole_infall_brake_spares_the_radial_plunge() {
    // The brake's asymmetry contract: inside the capture zone the
    // TANGENTIAL speed decays while the RADIAL speed survives — a
    // plunging mote keeps falling (the drag never holds the rain up).
    let mut m = pinned_infall(
        0.3,
        -1.6,
        0.0,
        crate::constants::BLACK_HOLE_INFALL_FALL_SPEED,
    );
    // Fly into the capture zone: nearly radial by construction.
    let mut retired = false;
    for _ in 0..6 {
        retired = retired || fly(&mut m, 0.25);
        if retired {
            break;
        }
    }
    let r = (m.x * m.x + m.y * m.y).sqrt();
    assert!(
        retired || r < 1.0,
        "the radial plunge must keep falling through the brake (r {r})"
    );
    if retired {
        assert!(!m.active, "the plunge resolved into the horizon");
    }
}

#[test]
fn black_hole_infall_shimmer_mutates_on_new_cells() {
    // The life-sign contract: motion-gated mutation — a glyph that
    // stays continuously in flight re-rolls its character as its
    // head lands on new cells (the only path an active mote's
    // character can change; recycled motes are filtered out by the
    // age comparison). The stage-4 sparse cadence hosts fewer glyphs
    // with shorter lives at any instant, so the harness samples
    // several short windows and accumulates the evidence — at least
    // one continuously-flying glyph must have re-rolled somewhere.
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames_to_rain_steady(&mut cloud, &mut frame);

    let mut tracked = 0usize;
    let mut changed = 0usize;
    for _ in 0..4 {
        let start: Vec<(usize, f32, char)> = cloud
            .black_hole_rain
            .infall_motes_for_test()
            .iter()
            .enumerate()
            .filter(|(_, m)| m.active)
            .map(|(i, m)| (i, m.sim_age, m.ch))
            .collect();
        run_frames(&mut cloud, &mut frame, 100, 16);
        let motes = cloud.black_hole_rain.infall_motes_for_test();
        for (i, age0, ch0) in &start {
            let m = &motes[*i];
            // Continuously alive across the window (an older age than
            // the snapshot's rules out an absorbed-and-respawned recycle).
            if m.active && m.sim_age > age0 + 1.5 {
                tracked += 1;
                if m.ch != *ch0 {
                    changed += 1;
                }
            }
        }
        if changed > 0 {
            break;
        }
    }
    assert!(tracked > 0, "some glyphs must fly continuously ({tracked})");
    assert!(
        changed > 0,
        "the motion-gated shimmer must mutate glyphs ({changed} of {tracked})"
    );
}

#[test]
fn black_hole_infall_lights_up_on_approach() {
    // The composition read: heads near the hole draw brighter than
    // heads far away (the kinetic-heat speed grade composed with the
    // shared proximity ladder). Zones are measured in ball outer
    // radii (the physics's own units): the near band covers the
    // crowns and the photon ring's reach, the far band the screen's
    // upper corners where only the dim ambient rain travels. Only
    // the screen ABOVE the stack's latitude counts (the disk's own
    // bands and the ball's lower half stay out of the measurement).
    // The stage-4 sparse rain visits the far band in crossings, not
    // constant occupancy, so the zones accumulate across a run of
    // frames on one continuous clock (a single frame's snapshot can
    // miss the corner traffic entirely).
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames_to_rain_steady(&mut cloud, &mut frame);

    let unit = (cols as f32 / 4.0).min(lines as f32 / 2.0);
    let outer_r = unit * crate::constants::BLACK_HOLE_BALL_FRACTION;
    let cx = ((cols - 1) / 2) as f32;
    let cy = ((lines - 1) / 2) as f32;
    let mut near_ranks = 0usize;
    let mut near_count = 0usize;
    let mut far_ranks = 0usize;
    let mut far_count = 0usize;
    // Continue the timeline comfortably past the clock the harness
    // already advanced, and seed both timers one frame back so every
    // sampled frame sees a clean inter-frame delta (no catch-up gap).
    let start = Instant::now() + Duration::from_secs(30);
    cloud.last_spawn_time = start - Duration::from_millis(16);
    cloud.last_phosphor_time = start - Duration::from_millis(16);
    for idx in 0..200u64 {
        let now = start + Duration::from_millis(idx * 16);
        cloud.rain_at(&mut frame, now);
        for cell in cloud.black_hole_rain.drawn_cells_for_test() {
            // The upper screen only (line <= cy + 3): the halo crowns,
            // the falling rain, and the annulus's top arc — the stack's
            // bands sit below the filter line.
            if cell.line as f32 > cy + 3.0 {
                continue;
            }
            let dx = (cell.col as f32 - cx) / 2.0;
            let dy = cell.line as f32 - cy;
            let dist_norm = (dx * dx + dy * dy).sqrt() / outer_r;
            let rank = level_rank(cell.level) as usize;
            if (0.7..=1.7).contains(&dist_norm) {
                near_ranks += rank;
                near_count += 1;
            } else if (2.75..=3.4).contains(&dist_norm) {
                far_ranks += rank;
                far_count += 1;
            }
        }
        frame.clear_dirty();
    }
    // Both zones populated (the crowns + near rain in the near band,
    // the ambient rain passing the upper corners in the far band);
    // the near zone's mean brightness must dominate.
    assert!(near_count > 0, "the near zone must carry drawn cells");
    assert!(far_count > 0, "the far zone must carry drawn cells");
    let near_mean = near_ranks as f32 / near_count as f32;
    let far_mean = far_ranks as f32 / far_count as f32;
    assert!(
        near_mean > far_mean,
        "the approach must brighten the rain (near {near_mean} vs far {far_mean})"
    );
}

#[test]
fn black_hole_infall_stays_a_sparse_minority() {
    // The stage-4 calm contract: the rain is weather, not a crowd.
    // At the engine's default density (1.0) the lane target keeps the
    // active population a small minority of the columns, and the live
    // pool at the harness density settles just as sparse — the clean,
    // uncrowded sky the owner asked for (stage 3 filled roughly half
    // the lanes at default density; the read was a downpour).
    let lanes = 200usize;
    let target = InfallStream::target_active_for_test(lanes, 1.0);
    assert!(
        (target as f32 / lanes as f32) <= 0.20,
        "default density must keep the rain a sparse minority (target {target} of {lanes})"
    );

    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames_to_rain_steady(&mut cloud, &mut frame);
    let active = cloud.black_hole_rain.active_infall_for_test();
    assert!(active > 0, "the rain must still live (got {active})");
    assert!(
        (active as f32 / cols as f32) <= 0.20,
        "the steady rain must stay a sparse minority ({active} of {cols})"
    );
}

#[test]
fn black_hole_infall_trickles_never_bursts() {
    // The stage-4 cadence contract: replacements arrive one at a
    // time — no frame may spawn two or more glyphs at once (the
    // trickle read: rain drifts in, never bursts in from the top).
    // Drives one continuous clock frame by frame and watches the
    // active count's per-frame jumps, from the formation intro
    // through the filled steady state.
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);

    let start = Instant::now();
    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    let mut max_jump = 0usize;
    for idx in 0..820u64 {
        let now = start + Duration::from_millis(idx * 16);
        let prev = cloud.black_hole_rain.active_infall_for_test();
        cloud.rain_at(&mut frame, now);
        let jump = cloud
            .black_hole_rain
            .active_infall_for_test()
            .saturating_sub(prev);
        max_jump = max_jump.max(jump);
        frame.clear_dirty();
    }
    assert!(
        cloud.black_hole_rain.active_infall_for_test() > 0,
        "the rain must be flying by the window's end"
    );
    assert!(
        max_jump <= 1,
        "a frame spawned {max_jump} glyphs at once — the rain must trickle, not burst"
    );
}

#[test]
fn black_hole_infall_corotates_with_the_disk() {
    // The NIGHT-research-9 masterclass contract: the ambient rain
    // shares the disk's angular momentum axis. Every infalling glyph
    // is born corotating — the specific angular momentum (x times vy
    // minus y times vx; y-down screen convention, the same sign the
    // ring motes carry
    // with their positive-phi advance) is POSITIVE at spawn, gravity
    // (a central force) preserves it exactly, and the accretion brake
    // only decays its magnitude toward zero — so no live mote may
    // ever carry the counter-rotating sign. Two layers: the direct
    // spawn contract (every activate sample, strict) and the
    // steady-state invariant (every live mote, tolerant of the
    // brake-decayed plunge near zero).
    use rand::{distr::Uniform, rngs::StdRng, SeedableRng};

    // Layer 1 — the spawn contract over a wide spawn envelope.
    let rand_chance = Uniform::new(0.0, 1.0).unwrap();
    let mut rng = StdRng::seed_from_u64(20260911);
    let fall = crate::constants::BLACK_HOLE_INFALL_FALL_SPEED;
    let (spawn_half_w, spawn_half_h) = (3.3_f32, 2.0_f32);
    let h = spawn_half_h + 0.25;
    let mut min_ell = f32::MAX;
    let mut max_ell = f32::MIN;
    for _ in 0..4000 {
        let mut m = InfallMote::vacant();
        crate::cloud::type_rain::black_hole::infall::activate_infall_mote(
            &mut m,
            spawn_half_w,
            spawn_half_h,
            0,
            &rand_chance,
            &mut rng,
        );
        let l = m.x * m.vy - m.y * m.vx;
        assert!(
            l > fall * crate::constants::BLACK_HOLE_INFALL_COROTATION_MIN * 0.99,
            "a spawned glyph must corotate (L {l} at x {} vx {})",
            m.x,
            m.vx
        );
        // The derived impact parameter: the ballistic crossing of the
        // hole's latitude lands at |x| = ell (inside the sampled
        // range, never beyond it).
        let ell = m.x + m.vx * (h / fall);
        min_ell = min_ell.min(ell);
        max_ell = max_ell.max(ell);
    }
    assert!(
        max_ell <= crate::constants::BLACK_HOLE_INFALL_COROTATION_MAX + 1.0e-4,
        "the impact parameter must stay inside the sampled range (max {max_ell})"
    );
    assert!(
        min_ell >= crate::constants::BLACK_HOLE_INFALL_COROTATION_MIN - 1.0e-4,
        "the impact parameter must stay inside the sampled range (min {min_ell})"
    );

    // Layer 2 — the steady-state invariant: every live mote (spawned,
    // bent, braked, mid-whip) keeps the corotating sign.
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames_to_rain_steady(&mut cloud, &mut frame);
    let mut live = 0;
    for m in cloud
        .black_hole_rain
        .infall_motes_for_test()
        .iter()
        .filter(|m| m.active)
    {
        let l = m.x * m.vy - m.y * m.vx;
        assert!(
            l > -0.05,
            "a live glyph must never carry the counter-rotating sign (L {l} at x {} y {})",
            m.x,
            m.y
        );
        live += 1;
    }
    assert!(live > 0, "no live glyphs to verify");
}
