// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-special-1 stage 2 tests: the orbital ring — RK4-Lorenz
//! turbulence on a wide tilted Keplerian ellipse around the ball,
//! with the stage-2.1 reads: gravitational-lensing halo over the
//! top, ball rim co-rotation, entry spiral for fresh motes, and the
//! stage-2.3 reads: the equatorial crossing (the solid line hugs
//! the core's vertical middle), the side-aware occlusion, the disk
//! radial brightness profile and the solid-band density. Covers
//! spawning + orbital advance, the band geometry, the occlusion
//! contract, style-transition recycling, and the shipped
//! motion constants (RK4 stability regime, majestic lap pace).

use std::collections::HashSet;

use super::*;
use crate::cloud::type_rain::black_hole::black_hole::level_rank;
use crate::cloud::type_rain::black_hole::ring::{
    disk_profile_level, occludes_ring_cell, project_ring_mote,
};
use crate::cloud::type_rain::monolith::BrightnessLevel;

/// Shared geometry for the projection checks (mirrors the ball
/// raster's own math: integer center, line-height units).
struct BallGeometry {
    cx: f32,
    cy: f32,
    outer_r: f32,
    /// Semi-major clamp passed to the projection (92% of the
    /// viewport half-width, line-height units — mirrors the draw
    /// pass's own clamp).
    major_limit: f32,
}

impl BallGeometry {
    fn new(cols: u16, lines: u16) -> Self {
        let unit = (cols as f32 / 4.0).min(lines as f32 / 2.0);
        Self {
            cx: ((cols - 1) / 2) as f32,
            cy: ((lines - 1) / 2) as f32,
            outer_r: unit * crate::constants::BLACK_HOLE_BALL_FRACTION,
            major_limit: 0.92 * cols as f32 / 4.0,
        }
    }

    /// Aspect-corrected distance from the ball center, in
    /// line-height units (the same units the raster scans).
    fn dist(&self, col: f32, line: f32) -> f32 {
        let dx = (col - self.cx) / 2.0;
        let dy = line - self.cy;
        (dx * dx + dy * dy).sqrt()
    }
}

#[test]
fn black_hole_ring_spawns_motes_and_orbits() {
    // The stage-2 contract: motes spawn (deficit-bounded accumulator)
    // and every active mote's orbital angle strictly advances — the
    // ring orbits, it never stalls.
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames_to_steady(&mut cloud, &mut frame);

    let active = cloud.black_hole_rain.active_motes_for_test();
    assert!(active > 0, "the ring must spawn motes (got {active})");

    let before: Vec<Option<f32>> = cloud
        .black_hole_rain
        .motes_for_test()
        .iter()
        .map(|m| if m.active { Some(m.phi) } else { None })
        .collect();

    // 30 frames at 60 FPS = 0.48 s of orbit — well under the minimum
    // lifetime (14 s - 15% variance), so every before-active mote is
    // still alive and comparable per index.
    run_frames(&mut cloud, &mut frame, 30, 16);
    let motes = cloud.black_hole_rain.motes_for_test();
    let mut compared = 0;
    for (idx, was) in before.iter().enumerate() {
        if let Some(phi_before) = was {
            let m = &motes[idx];
            assert!(
                m.active,
                "mote {idx} absorbed too early (lifetime contract)"
            );
            assert!(
                m.phi > phi_before + 0.01,
                "mote {idx} orbital angle stalled ({} -> {})",
                phi_before,
                m.phi
            );
            compared += 1;
        }
    }
    assert!(compared > 0, "no active motes to compare");
}

#[test]
fn black_hole_ring_heads_stay_in_the_band() {
    // Every projected head stays inside the orbital band (never
    // beyond the wobbled outer edge) and inside the viewport; heads
    // dipping closer than the band's inner edge are crossing the
    // ball silhouette (the tilted ellipse squeezes its top and
    // bottom over the hole) — that is the 3D crossing, not an escape.
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    // Long enough that early-spawned motes pass the entry settle
    // window (3 tau) and sit on the steady disk for the band check.
    run_frames_to_steady(&mut cloud, &mut frame);
    run_frames(&mut cloud, &mut frame, 210, 16);

    let geo = BallGeometry::new(cols, lines);
    let unit = geo.outer_r / crate::constants::BLACK_HOLE_BALL_FRACTION;
    let a_mean = (crate::constants::BLACK_HOLE_RING_MAJOR_FRACTION * unit).min(geo.major_limit);
    // The band contract: steady-age motes stay inside the orbital
    // band (the farthest horizontal reach, or the lensing halo's
    // radius, whichever is greater) and inside the viewport. Fresh
    // motes (sim_age below the entry window) are still on their
    // drift-in spiral and may sit beyond it — they are excluded
    // (the entry spiral is a separate contract below).
    let entry_settle_secs = 3.0 * crate::constants::BLACK_HOLE_RING_ENTRY_TAU;
    let band_max = (a_mean + crate::constants::BLACK_HOLE_RING_WOBBLE_FRACTION * geo.outer_r * 1.2)
        .max(geo.outer_r * crate::constants::BLACK_HOLE_RING_LENS_ARC_FRACTION)
        + 0.1;

    let mut projected = 0;
    for m in cloud.black_hole_rain.motes_for_test() {
        if !m.active || m.sim_age < entry_settle_secs {
            continue;
        }
        projected += 1;
        let (col_f, line_f) = project_ring_mote(m, geo.cx, geo.cy, geo.outer_r, geo.major_limit);
        let col = col_f.round();
        let line = line_f.round();
        assert!(
            col >= 0.0 && col < cols as f32,
            "ring head column out of viewport ({col})"
        );
        assert!(
            line >= 0.0 && line < lines as f32,
            "ring head line out of viewport ({line})"
        );
        let dist = geo.dist(col, line);
        assert!(
            dist <= band_max + 0.75,
            "ring head escaped the band (dist {dist} > {band_max})"
        );
    }
    assert!(projected > 0, "no settled motes to project");
}

#[test]
fn black_hole_ring_occlusion_is_side_aware() {
    // The 3D layering contract (stage 2.3): occlusion keys on the
    // orbit side, not the screen height. Near-side cells pass in
    // front of the hole at ANY height (the z-tilt breathes them
    // above the equator without them vanishing — the fix for the
    // "line reads below the middle" feedback); far-side cells hide
    // while inside the silhouette at any height (the far side's
    // visible share lives on the lensing arc outside it).
    let (cols, lines) = (120, 40);
    let geo = BallGeometry::new(cols, lines);
    let cx_i = ((cols - 1) / 2) as i32;
    let cy_i = ((lines - 1) / 2) as i32;
    // Cells straddling the silhouette: (0, -2) and (0, +2) sit well
    // inside; (30, 0) is far outside to the right.
    let inside_above = (cx_i as u16, (cy_i - 2).max(0) as u16);
    let inside_below = (cx_i as u16, (cy_i + 2) as u16);
    let outside = ((cx_i + 30) as u16, cy_i as u16);

    // Near side: never occluded, anywhere (the crossing read).
    assert!(
        !occludes_ring_cell(
            inside_above.0,
            inside_above.1,
            cx_i,
            cy_i,
            geo.outer_r,
            true
        ),
        "near-side cell above the equator must draw in front of the hole"
    );
    assert!(
        !occludes_ring_cell(
            inside_below.0,
            inside_below.1,
            cx_i,
            cy_i,
            geo.outer_r,
            true
        ),
        "near-side cell below the equator must draw in front of the hole"
    );
    assert!(
        !occludes_ring_cell(outside.0, outside.1, cx_i, cy_i, geo.outer_r, true),
        "near-side cell outside the silhouette must draw"
    );

    // Far side: hidden while inside the silhouette, at ANY height
    // (the old rule only hid above-center — a far-side mote below
    // the equator inside the silhouette leaked through).
    assert!(
        occludes_ring_cell(
            inside_above.0,
            inside_above.1,
            cx_i,
            cy_i,
            geo.outer_r,
            false
        ),
        "far-side cell inside the silhouette above center must hide"
    );
    assert!(
        occludes_ring_cell(
            inside_below.0,
            inside_below.1,
            cx_i,
            cy_i,
            geo.outer_r,
            false
        ),
        "far-side cell inside the silhouette below center must hide"
    );
    assert!(
        !occludes_ring_cell(outside.0, outside.1, cx_i, cy_i, geo.outer_r, false),
        "far-side cell outside the silhouette must draw (the lensing arc)"
    );
}

#[test]
fn black_hole_ring_draws_no_far_side_cells_inside_the_silhouette() {
    // Integration half of the occlusion contract: whatever the draw
    // pass put inside the ball silhouette (excluding the ball's own
    // annulus cells) must belong to a NEAR-side mote. Replicated
    // here from the live mote state — with one timing subtlety: the
    // draw iterates each mote's trail BEFORE pushing the current
    // head into it, so the trail cells of the frame under test are
    // the PREVIOUS frame's trail buffer. The mote pool is therefore
    // snapshotted one frame early (head positions from the live
    // post-advance state, trail cells from the snapshot) — the
    // exact inputs the draw pass used.
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames_to_steady(&mut cloud, &mut frame);
    run_frames(&mut cloud, &mut frame, 119, 16);

    // Snapshot the pool one frame BEFORE the frame under test: this
    // is the trail buffer the next draw will iterate (RingMote is
    // Copy — a flat clone is exact).
    let snapshot: Vec<_> = cloud.black_hole_rain.motes_for_test().to_vec();
    // The frame under test: advance + draw with the snapshot's trail.
    run_frames(&mut cloud, &mut frame, 1, 16);

    let geo = BallGeometry::new(cols, lines);

    let ball_cells: HashSet<(u16, u16)> = cloud
        .black_hole_rain
        .ring_cells_for_test()
        .iter()
        .map(|c| (c.col, c.line))
        .collect();

    // Legit inside-silhouette cells: every in-bounds head of a
    // near-side mote (live post-advance phi — what the draw
    // projected) plus the snapshot's trail cells (what the draw
    // iterated before its own push). Near cells are never occluded,
    // so both sets draw whenever in bounds.
    let mut legit_near: HashSet<(u16, u16)> = HashSet::new();
    for (idx, m) in cloud.black_hole_rain.motes_for_test().iter().enumerate() {
        if !m.active || m.phi.sin() < 0.0 {
            continue;
        }
        let (col_f, line_f) = project_ring_mote(m, geo.cx, geo.cy, geo.outer_r, geo.major_limit);
        let col = col_f.round() as i32;
        let line = line_f.round() as i32;
        if col < 0 || line < 0 || col >= cols as i32 || line >= lines as i32 {
            continue;
        }
        legit_near.insert((col as u16, line as u16));
        if idx < snapshot.len() && snapshot[idx].active {
            for t in 0..snapshot[idx].trail_len as usize {
                let (tc, tl) = snapshot[idx].trail[t];
                if tc < cols && tl < lines {
                    legit_near.insert((tc, tl));
                }
            }
        }
    }
    assert!(!legit_near.is_empty(), "no near-side motes to cross-check");

    let drawn = cloud.black_hole_rain.drawn_cells_for_test();
    let mut mote_cells = 0;
    let mut inside_drawn = 0;
    for cell in drawn {
        if ball_cells.contains(&(cell.col, cell.line)) {
            continue;
        }
        mote_cells += 1;
        if geo.dist(cell.col as f32, cell.line as f32) < geo.outer_r {
            inside_drawn += 1;
            assert!(
                legit_near.contains(&(cell.col, cell.line)),
                "mote cell inside the silhouette is not near-side ({}, {})",
                cell.col,
                cell.line
            );
        }
    }
    assert!(mote_cells > 0, "no ring cells drawn this frame");
    assert!(
        inside_drawn > 0,
        "the near side must cross in front of the silhouette (the crossing read)"
    );
}

#[test]
fn black_hole_ring_survives_style_transition() {
    // Family contract: leaving the style wipes the mote pool (no
    // orbiting ghosts in another scene), re-entry respawns it from a
    // clean baseline.
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames_to_steady(&mut cloud, &mut frame);
    assert!(
        cloud.black_hole_rain.active_motes_for_test() > 0,
        "motes must be active before the transition"
    );

    cloud.transition_rain_style(RainStyle::Vortex);
    assert_eq!(
        cloud.black_hole_rain.active_motes_for_test(),
        0,
        "style exit must wipe the mote pool"
    );

    cloud.transition_rain_style(RainStyle::BlackHole);
    // Re-entry replays the formation intro (accretion gate closed
    // until the hole is whole) — fast-forward to steady, where the
    // ring must have respawned.
    run_frames_to_steady(&mut cloud, &mut frame);
    assert!(
        cloud.black_hole_rain.active_motes_for_test() > 0,
        "style re-entry must respawn the ring"
    );
}

#[test]
fn black_hole_ring_lap_pace_is_majestic() {
    // Constant sanity: at the scene default speed 12 one full orbit
    // takes ~11.6 s — a calm accretion read, not a frantic carousel.
    // The bounds keep future tuning honest (fast enough to feel
    // alive within a lifetime, slow enough to read as orbital).
    let omega = 12.0 * crate::constants::BLACK_HOLE_RING_OMEGA_PER_CPS;
    let lap = std::f32::consts::TAU / omega;
    assert!(
        lap > 8.0 && lap < 20.0,
        "one lap at scene default speed takes {lap}s (out of the majestic range)"
    );
}

#[test]
fn black_hole_ring_rk4_step_bounded() {
    // The integration dt is chars_per_sec * RING_DT_PER_CPS * dt_wall
    // (same mapping as the lorenz style). At the scene default speed
    // and 60 FPS it sits far below the RK4 stability threshold for
    // the canonical Lorenz system (literature: dt < 0.01 stable).
    let cps = 12.0_f32;
    let dt_wall = 1.0 / 60.0;
    let dt = cps * crate::constants::BLACK_HOLE_RING_DT_PER_CPS * dt_wall;
    assert!(
        dt > 0.0 && dt < 0.01,
        "RK4 dt must stay in the stable regime (got {dt})"
    );
}

// Compile-time contract: the Keplerian shear exponent is Kepler's
// third law (minus three-halves) — any drift here would silently
// change the differential-rotation signature of the disk.
const _: () = assert!(crate::constants::BLACK_HOLE_RING_KEPLER_EXP == 1.5);

// -- Stage 2.1: the gravitational-lensing halo, the entry spiral,
// and the ball's co-rotation --

/// A synthetic mote pinned to a given orbital phase with the Lorenz
/// state parked at the normalization centers (r_norm = 0, z_norm =
/// 0) — projection geometry in isolation, no turbulence.
fn pinned_mote(phi: f32, sim_age: f32) -> crate::cloud::type_rain::black_hole::ring::RingMote {
    let mut m = crate::cloud::type_rain::black_hole::ring::RingMote::vacant();
    m.active = true;
    m.phi = phi;
    m.sim_age = sim_age;
    m.x = crate::constants::BLACK_HOLE_RING_R_NORM_CENTER;
    m.y = 0.0;
    m.z = crate::constants::BLACK_HOLE_RING_Z_NORM_CENTER;
    m
}

#[test]
fn black_hole_ring_lens_lifts_far_side_over_the_top() {
    // The owner's stage-2 feedback: particles approaching the hole's
    // edge must curve UP. Contract: directly behind the hole the far
    // side projects onto the halo arc ABOVE the ball's top (the
    // lensed image); directly in front it crosses BELOW the center
    // (the near side in front of the shadow); at the extremes it
    // meets the disk plane (continuity — no jump between the flat
    // ellipse and the arc).
    let (cols, lines) = (120, 40);
    let geo = BallGeometry::new(cols, lines);

    let behind = pinned_mote(3.0 * std::f32::consts::FRAC_PI_2, 30.0);
    let (_, line_behind) = project_ring_mote(&behind, geo.cx, geo.cy, geo.outer_r, geo.major_limit);
    assert!(
        line_behind < geo.cy - geo.outer_r,
        "far-side center must project above the ball top (line {line_behind}, cy {}, top {})",
        geo.cy,
        geo.cy - geo.outer_r
    );

    let front = pinned_mote(std::f32::consts::FRAC_PI_2, 30.0);
    let (_, line_front) = project_ring_mote(&front, geo.cx, geo.cy, geo.outer_r, geo.major_limit);
    assert!(
        line_front > geo.cy,
        "near-side center must project below the viewport center (line {line_front})"
    );

    let side = pinned_mote(0.0, 30.0);
    let (_, line_side) = project_ring_mote(&side, geo.cx, geo.cy, geo.outer_r, geo.major_limit);
    assert!(
        (line_side - geo.cy).abs() < 0.01,
        "disk extreme must sit on the disk plane (line {line_side}, cy {})",
        geo.cy
    );

    // The lift is monotonic in backness: a quarter-behind mote sits
    // strictly between the plane and the apex — the curve reads as a
    // continuous rise, not a teleport.
    let quarter = pinned_mote(std::f32::consts::PI + std::f32::consts::FRAC_PI_4, 30.0);
    let (_, line_quarter) =
        project_ring_mote(&quarter, geo.cx, geo.cy, geo.outer_r, geo.major_limit);
    assert!(
        line_quarter < geo.cy && line_quarter > line_behind,
        "lift must rise smoothly (quarter {line_quarter} vs plane {} vs apex {line_behind})",
        geo.cy
    );
}

#[test]
fn black_hole_ring_entry_spiral_drifts_inward() {
    // The accretion read: a fresh mote projects beyond the disk and
    // settles onto it exponentially — strictly closer to the center
    // as it ages, never farther.
    let (cols, lines) = (120, 40);
    let geo = BallGeometry::new(cols, lines);

    let young = pinned_mote(0.0, 0.05);
    let old = pinned_mote(0.0, 30.0);
    let (col_young, _) = project_ring_mote(&young, geo.cx, geo.cy, geo.outer_r, geo.major_limit);
    let (col_old, _) = project_ring_mote(&old, geo.cx, geo.cy, geo.outer_r, geo.major_limit);
    let reach_young = (col_young - geo.cx).abs();
    let reach_old = (col_old - geo.cx).abs();
    assert!(
        reach_young > reach_old + 0.5,
        "fresh mote must start beyond the disk (reach {reach_young} vs settled {reach_old})"
    );

    // Monotonic settle over the decay window.
    let mid = pinned_mote(0.0, 2.0 * crate::constants::BLACK_HOLE_RING_ENTRY_TAU);
    let (col_mid, _) = project_ring_mote(&mid, geo.cx, geo.cy, geo.outer_r, geo.major_limit);
    let reach_mid = (col_mid - geo.cx).abs();
    assert!(
        reach_young > reach_mid && reach_mid > reach_old,
        "entry settle must be monotonic (young {reach_young} > mid {reach_mid} > old {reach_old})"
    );
}

#[test]
fn black_hole_ball_spin_advances_with_the_ring() {
    // The co-rotation contract: the rim's spin phase strictly
    // advances on the shared clock (the hole visibly rotates, never
    // stalls), even before any mote has spawned — the phase rides
    // the advance pass's global clock, not the mote pool.
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 60, 16);

    let spin_before = cloud.black_hole_rain.spin_phase_for_test();
    assert!(
        spin_before > 0.0,
        "spin phase must advance from zero (got {spin_before})"
    );
    run_frames(&mut cloud, &mut frame, 30, 16);
    let spin_after = cloud.black_hole_rain.spin_phase_for_test();
    assert!(
        spin_after > spin_before + 0.01,
        "spin phase stalled ({} -> {})",
        spin_before,
        spin_after
    );

    // Sync sanity: at the scene default speed the spin advances at
    // SPIN_RATE x the ring's mean omega — one rim lap per disk lap
    // at rate 1.0 (the lockstep the owner asked for). The first
    // frame only arms the clock (last_step None -> dt 0), so 60
    // frames = 59 x 16 ms of spin time.
    let omega = 12.0 * crate::constants::BLACK_HOLE_RING_OMEGA_PER_CPS;
    let expected = omega * crate::constants::BLACK_HOLE_RING_SPIN_RATE * (59.0 * 0.016);
    assert!(
        (spin_before - expected).abs() < 0.05,
        "spin phase must track the ring's mean motion (got {spin_before}, want ~{expected})"
    );
}

// -- Stage 2.3: the equatorial crossing, the disk radial profile,
// and the solid-band density --

#[test]
fn black_hole_ring_crossing_band_hugs_the_equator() {
    // The owner's stage-2.3 feedback (9.5/10): the solid line must
    // read at the vertical MIDDLE of the core, not below it. The
    // near side's sine is squashed to NEAR_SQUASH of the minor axis,
    // so the deepest crossing dip is half a minor axis below center
    // (previously a full minor axis — the "line below the core"
    // read). Regression guard: removing the squash fails the
    // dip-bound assertion.
    let (cols, lines) = (120, 40);
    let geo = BallGeometry::new(cols, lines);
    let unit = geo.outer_r / crate::constants::BLACK_HOLE_BALL_FRACTION;
    // The pinned mote parks r_norm at 0 (no wobble) and z at the
    // normalization center (no tilt), so the dip is pure ellipse
    // geometry: NEAR_SQUASH x MINOR x unit.
    let expected_dip = crate::constants::BLACK_HOLE_RING_NEAR_SQUASH
        * crate::constants::BLACK_HOLE_RING_MINOR_FRACTION
        * unit;

    let front = pinned_mote(std::f32::consts::FRAC_PI_2, 30.0);
    let (_, line_front) = project_ring_mote(&front, geo.cx, geo.cy, geo.outer_r, geo.major_limit);
    assert!(
        line_front > geo.cy,
        "the near side still crosses in FRONT (below center), got {line_front}"
    );
    assert!(
        (line_front - geo.cy - expected_dip).abs() < 0.05,
        "crossing dip must equal the squashed half minor axis (dip {}, want ~{expected_dip})",
        line_front - geo.cy
    );
    assert!(
        line_front - geo.cy < crate::constants::BLACK_HOLE_RING_MINOR_FRACTION * unit,
        "crossing dip must stay under the full minor axis (the unsquashed old geometry)"
    );

    // The whole near side stays within the squashed band: every
    // near-side phase projects between the equator and the dip.
    for deg in 5..175 {
        let phi = (deg as f32).to_radians();
        let m = pinned_mote(phi, 30.0);
        let (_, line) = project_ring_mote(&m, geo.cx, geo.cy, geo.outer_r, geo.major_limit);
        assert!(
            line >= geo.cy - 0.05 && line <= geo.cy + expected_dip + 0.05,
            "near-side phase {deg} deg escaped the equatorial band (line {line})"
        );
    }

    // Continuity at the extremes: just below and just above phi = 0
    // (and pi) the projection is continuous — the squash changes the
    // one-sided slopes (near rises at half rate, far falls at full)
    // but not the value, so a tight neighborhood straddles the seam
    // without a jump.
    for base in [0.0, std::f32::consts::PI] {
        let below = pinned_mote(base - 0.008, 30.0);
        let above = pinned_mote(base + 0.008, 30.0);
        let (_, l_below) = project_ring_mote(&below, geo.cx, geo.cy, geo.outer_r, geo.major_limit);
        let (_, l_above) = project_ring_mote(&above, geo.cx, geo.cy, geo.outer_r, geo.major_limit);
        assert!(
            (l_below - l_above).abs() < 0.05,
            "seam at the disk extreme (phi {base}: {l_below} vs {l_above})"
        );
    }
}

#[test]
fn black_hole_ring_disk_profile_fades_the_edges() {
    // The Gargantua read: brightness keyed on |cos phi| — the hot
    // inner zone across the shadow steps UP one rung (the solid
    // white line), the mid zone is untouched, and past the fade
    // start the level steps DOWN one to three rungs (the line's
    // ends dissolve into sparse dim wisps, the smooth transition).
    let inner = std::f32::consts::FRAC_PI_2; // cos = 0: directly across the shadow
    let mid = std::f32::consts::FRAC_PI_3; // cos = 0.5: between the zones
    let fade_edge = 0.75_f32.acos(); // cos = 0.75: 1 rung down
    let fade_deep = 0.85_f32.acos(); // cos = 0.85: 2 rungs down
    let extreme = 0.0; // cos = 1: the line's very end, 3 rungs down

    // Inner zone: one rung up, clamped at Core.
    assert_eq!(
        level_rank(disk_profile_level(BrightnessLevel::Mid, inner)),
        level_rank(BrightnessLevel::Hot),
        "inner-zone Mid must bump to Hot (the solid bright line)"
    );
    assert_eq!(
        level_rank(disk_profile_level(BrightnessLevel::Core, inner)),
        level_rank(BrightnessLevel::Core),
        "inner-zone Core must stay clamped at Core"
    );

    // Mid zone: untouched.
    assert_eq!(
        level_rank(disk_profile_level(BrightnessLevel::Mid, mid)),
        level_rank(BrightnessLevel::Mid),
        "mid-zone level must pass through untouched"
    );

    // Fade ladder: monotonically dimmer toward the extreme.
    let hot_edge = level_rank(disk_profile_level(BrightnessLevel::Hot, fade_edge));
    let hot_deep = level_rank(disk_profile_level(BrightnessLevel::Hot, fade_deep));
    let hot_extreme = level_rank(disk_profile_level(BrightnessLevel::Hot, extreme));
    assert_eq!(hot_edge, 2, "cos 0.75 steps Hot down one rung to Mid");
    assert_eq!(hot_deep, 0, "cos 0.85 steps Hot down to Ghost");
    assert_eq!(
        hot_extreme, 0,
        "the extreme steps Hot down to Ghost (sparse wisps)"
    );

    // The extremes of the orbit are where motes dwell longest
    // (horizontal speed -> 0), so the fade also thins the trail:
    // the trail steps down FROM the graded head. Sanity-check the
    // ordering: graded(extreme) is strictly dimmer than graded(inner).
    assert!(
        level_rank(disk_profile_level(BrightnessLevel::Hot, extreme))
            < level_rank(disk_profile_level(BrightnessLevel::Hot, inner)),
        "the profile must fade monotonically from inner zone to extreme"
    );
}

#[test]
fn black_hole_ring_reads_as_a_solid_band() {
    // The density contract (stage 2.3): at the scene density 0.55
    // the steady-state target is BASE + 0.55 x MULT = ~0.74 of the
    // pool — the band reads as a near-continuous line, the
    // Interstellar reference. The floor assertion (60% of the
    // pool) leaves headroom for spawn-scale dips while still
    // failing the old sparse geometry (0.41 target, floor broken).
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames_to_steady(&mut cloud, &mut frame);
    run_frames(&mut cloud, &mut frame, 300, 16);

    let pool = cloud.black_hole_rain.motes_for_test().len();
    let active = cloud.black_hole_rain.active_motes_for_test();
    assert!(pool > 0, "the pool must be sized to the viewport");
    assert!(
        active as f32 >= 0.60 * pool as f32,
        "the band must read as solid: {active}/{pool} active is below the 60% floor"
    );
    assert!(
        active <= pool,
        "active count cannot exceed the pool ({active}/{pool})"
    );
}
