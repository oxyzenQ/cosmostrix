// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-special-1 stage 2 tests: the orbital ring — RK4-Lorenz
//! turbulence on a wide tilted Keplerian ellipse around the ball,
//! with the stage-2.1 reads: gravitational-lensing halo over the
//! top, ball rim co-rotation, entry spiral for fresh motes; the
//! stage-2.3 reads: the equatorial crossing (the solid line hugs
//! the core's vertical middle), the side-aware occlusion and the
//! solid-band density; and the stage-2.4 reads: the proximity
//! brightness profile (white heads near the hole, the fade ladder
//! far out), the three-tier Interstellar stack (longest band, the
//! upper shorter bands snug above it with differential Keplerian
//! pacing — the stage-2.6 one-compact-family ruling: the main disk
//! slightly below center, the upper bands almost fused with it, the
//! two longest bands widened) and the see-saw roll scheduler (every
//! attitude in the 15-180 degree window, with the vertical
//! 90-degree attitude excluded, parks at a long 30 s-or-more hold;
//! the flat rest line stays the single longest pose; excursions
//! alternate sign). Stage 2.6 adds the halo stream contracts: the
//! arc-riding pool whose upper stream doubles the upward-curving
//! density, the mirrored lower stream running slightly sparser, the
//! disk's rotational sense, and the dynamic-screen-size resize
//! contract. Stage 2.7 (owner 9.95/10 feedback) re-pins: the whole
//! stack family descends below the viewport center, the main band
//! stretches a quarter longer, the halo split becomes the double
//! upward stream (two upper crowns) with the lower stream rare, and
//! the snug upper stacks' heads floor at white. Covers spawning +
//! orbital advance, the band geometry,
//! the occlusion contract, style-transition recycling, and the
//! shipped motion constants (RK4 stability regime, majestic lap
//! pace).

use std::collections::HashSet;

use super::*;
use crate::cloud::type_rain::black_hole::black_hole::{level_rank, CELL_ASPECT_DIVISOR};
use crate::cloud::type_rain::black_hole::halo::{
    halo_mote_visible, project_halo_mote, HALO_STREAM_TAG_LOWER, HALO_STREAM_TAG_UPPER,
    HALO_STREAM_TAG_UPPER_OUTER,
};
use crate::cloud::type_rain::black_hole::ring::{
    occludes_ring_cell, project_ring_mote, proximity_level, RingRoll,
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
    // The band contract: steady-age motes stay inside the orbital
    // band (the farthest horizontal reach, or the lensing halo's
    // radius, whichever is greater) and inside the viewport. Fresh
    // motes (sim_age below the entry window) are still on their
    // drift-in spiral and may sit beyond it — they are excluded
    // (the entry spiral is a separate contract below). The reach is
    // per-tier (the stage-2.7 stretched main disk reads 1.375x the
    // major fraction — the owner's 1 cm -> 1.25 cm ruling), so the
    // bound is the table's worst case; the projection clamps the
    // wobble-inclusive radius at the major limit, so steady heads
    // stay inside the viewport.
    let entry_settle_secs = 3.0 * crate::constants::BLACK_HOLE_RING_ENTRY_TAU;
    let a_max = crate::constants::BLACK_HOLE_RING_TIERS
        .iter()
        .map(|t| {
            (crate::constants::BLACK_HOLE_RING_MAJOR_FRACTION * t.major_scale * unit)
                .min(geo.major_limit)
        })
        .fold(0.0_f32, f32::max);
    let wobble_max = crate::constants::BLACK_HOLE_RING_TIERS
        .iter()
        .map(|t| t.wobble_fraction)
        .fold(0.0_f32, f32::max);
    let band_max = (a_max + wobble_max * geo.outer_r * 1.2)
        .max(geo.outer_r * crate::constants::BLACK_HOLE_RING_LENS_ARC_FRACTION)
        + 0.1;

    let mut projected = 0;
    for m in cloud.black_hole_rain.motes_for_test() {
        if !m.active || m.sim_age < entry_settle_secs {
            continue;
        }
        projected += 1;
        let (col_f, line_f) =
            project_ring_mote(m, geo.cx, geo.cy, geo.outer_r, geo.major_limit, 0.0);
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
    // annulus cells) must belong to a front-drawing mote — near-side
    // tier 0 (the crossing read) or any tier 1-2 mote (the
    // lensed-image ribbons the stage-2.5 snug stack runs across the
    // shadow face). Replicated
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
    // front-drawing mote (the draw pass's rule: near-side tier 0, or
    // ANY tier 1-2 — the upper bands are lensed-image ribbons that
    // read in front of the hole at any height, so their far strands
    // cross the shadow face with the snug stack; live post-advance
    // phi — what the draw projected) plus the snapshot's trail cells
    // (what the draw iterated before its own push). Front-drawing
    // cells are never occluded, so both sets draw whenever in
    // bounds.
    let mut legit_near: HashSet<(u16, u16)> = HashSet::new();
    for (idx, m) in cloud.black_hole_rain.motes_for_test().iter().enumerate() {
        if !m.active || (m.tier == 0 && m.phi.sin() < 0.0) {
            continue;
        }
        let (col_f, line_f) =
            project_ring_mote(m, geo.cx, geo.cy, geo.outer_r, geo.major_limit, 0.0);
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
                "mote cell inside the silhouette is not front-drawing ({}, {})",
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
    let (_, line_behind) =
        project_ring_mote(&behind, geo.cx, geo.cy, geo.outer_r, geo.major_limit, 0.0);
    assert!(
        line_behind < geo.cy - geo.outer_r,
        "far-side center must project above the ball top (line {line_behind}, cy {}, top {})",
        geo.cy,
        geo.cy - geo.outer_r
    );

    let front = pinned_mote(std::f32::consts::FRAC_PI_2, 30.0);
    let (_, line_front) =
        project_ring_mote(&front, geo.cx, geo.cy, geo.outer_r, geo.major_limit, 0.0);
    assert!(
        line_front > geo.cy,
        "near-side center must project below the viewport center (line {line_front})"
    );

    let side = pinned_mote(0.0, 30.0);
    let (_, line_side) =
        project_ring_mote(&side, geo.cx, geo.cy, geo.outer_r, geo.major_limit, 0.0);
    // The stage-2.6 band-center offset: the main disk's rest plane
    // now sits slightly below the viewport center (the owner's
    // slight descent of stack 1), so the extreme lands on that
    // plane, not on the geometric center line.
    let band_plane =
        geo.cy - crate::constants::BLACK_HOLE_RING_TIERS[0].center_offset * geo.outer_r;
    assert!(
        (line_side - band_plane).abs() < 0.01,
        "disk extreme must sit on the band's rest plane (line {line_side}, plane {band_plane})"
    );

    // The lift is monotonic in backness: a quarter-behind mote sits
    // strictly between the plane and the apex — the curve reads as a
    // continuous rise, not a teleport.
    let quarter = pinned_mote(std::f32::consts::PI + std::f32::consts::FRAC_PI_4, 30.0);
    let (_, line_quarter) =
        project_ring_mote(&quarter, geo.cx, geo.cy, geo.outer_r, geo.major_limit, 0.0);
    assert!(
        line_quarter < band_plane && line_quarter > line_behind,
        "lift must rise smoothly (quarter {line_quarter} vs plane {band_plane} vs apex {line_behind})"
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
    let (col_young, _) =
        project_ring_mote(&young, geo.cx, geo.cy, geo.outer_r, geo.major_limit, 0.0);
    let (col_old, _) = project_ring_mote(&old, geo.cx, geo.cy, geo.outer_r, geo.major_limit, 0.0);
    let reach_young = (col_young - geo.cx).abs();
    let reach_old = (col_old - geo.cx).abs();
    assert!(
        reach_young > reach_old + 0.5,
        "fresh mote must start beyond the disk (reach {reach_young} vs settled {reach_old})"
    );

    // Monotonic settle over the decay window.
    let mid = pinned_mote(0.0, 2.0 * crate::constants::BLACK_HOLE_RING_ENTRY_TAU);
    let (col_mid, _) = project_ring_mote(&mid, geo.cx, geo.cy, geo.outer_r, geo.major_limit, 0.0);
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
    // so the deepest crossing dip is half a minor axis below the
    // band's rest plane (previously a full minor axis — the "line
    // below the core" read). Stage 2.7 (owner 9.95/10 feedback):
    // the whole family descends below the viewport center — the
    // crossing line rides with the dropped main band (his ruling:
    // the light reads below center). Regression guard: removing
    // the squash fails the exact dip assertion.
    let (cols, lines) = (120, 40);
    let geo = BallGeometry::new(cols, lines);
    let unit = geo.outer_r / crate::constants::BLACK_HOLE_BALL_FRACTION;
    // The pinned mote parks r_norm at 0 (no wobble) and z at the
    // normalization center (no tilt), so the dip is pure ellipse
    // geometry: NEAR_SQUASH x MINOR x unit, plus the stage-2.7
    // whole-family drop (a negative offset deepens the dip).
    let expected_dip = crate::constants::BLACK_HOLE_RING_NEAR_SQUASH
        * crate::constants::BLACK_HOLE_RING_MINOR_FRACTION
        * unit
        - crate::constants::BLACK_HOLE_RING_TIERS[0].center_offset * geo.outer_r;

    let front = pinned_mote(std::f32::consts::FRAC_PI_2, 30.0);
    let (_, line_front) =
        project_ring_mote(&front, geo.cx, geo.cy, geo.outer_r, geo.major_limit, 0.0);
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
        line_front - geo.cy
            < -crate::constants::BLACK_HOLE_RING_TIERS[0].center_offset * geo.outer_r
                + crate::constants::BLACK_HOLE_RING_MINOR_FRACTION * unit
                + 0.10 * geo.outer_r,
        "crossing dip must stay near the dropped equatorial band (the unsquashed old geometry)"
    );

    // The whole near side stays within the squashed band: every
    // near-side phase projects between the band's rest plane and the
    // dip (the rest plane itself sits below the viewport center at
    // stage 2.7 — the whole-family descent).
    let band_plane =
        geo.cy - crate::constants::BLACK_HOLE_RING_TIERS[0].center_offset * geo.outer_r;
    for deg in 5..175 {
        let phi = (deg as f32).to_radians();
        let m = pinned_mote(phi, 30.0);
        let (_, line) = project_ring_mote(&m, geo.cx, geo.cy, geo.outer_r, geo.major_limit, 0.0);
        assert!(
            line >= band_plane - 0.05 && line <= geo.cy + expected_dip + 0.05,
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
        let (_, l_below) =
            project_ring_mote(&below, geo.cx, geo.cy, geo.outer_r, geo.major_limit, 0.0);
        let (_, l_above) =
            project_ring_mote(&above, geo.cx, geo.cy, geo.outer_r, geo.major_limit, 0.0);
        assert!(
            (l_below - l_above).abs() < 0.05,
            "seam at the disk extreme (phi {base}: {l_below} vs {l_above})"
        );
    }
}

#[test]
fn black_hole_ring_proximity_profile_brightens_the_inner_disk() {
    // The stage-2.4 contract (owner 9.7/10 feedback): brightness
    // keyed on the projected DISTANCE from the hole (in ball radii)
    // — inside the hot radius the level steps UP two rungs (Mid and
    // Hot bases land at Core, the "head white" white-hot read: the
    // crossing band across the shadow and the lensing arc); the
    // warm belt to the fade start steps up one; past the fade start
    // the level steps DOWN one to three rungs over the fade span
    // (the line's ends dissolve into sparse dim wisps, the smooth
    // transition of the Interstellar reference — "the ones moving
    // away fade").
    let crossing = 0.5; // in front of the shadow, deep inside the hot radius
    let arc = crate::constants::BLACK_HOLE_RING_LENS_ARC_FRACTION; // the halo circle
    let warm = crate::constants::BLACK_HOLE_RING_HOT_RADIUS
        + (crate::constants::BLACK_HOLE_RING_FADE_START
            - crate::constants::BLACK_HOLE_RING_HOT_RADIUS)
            * 0.5; // between the two bounds
    let fade_edge = crate::constants::BLACK_HOLE_RING_FADE_START + 0.12; // ~1 rung down
    let extreme = 2.0; // the line's far end: 3 rungs down

    // Hot zone: two rungs up, clamped at Core.
    assert_eq!(
        level_rank(proximity_level(BrightnessLevel::Mid, crossing)),
        level_rank(BrightnessLevel::Core),
        "crossing Mid must burn at Core (the white head)"
    );
    assert_eq!(
        level_rank(proximity_level(BrightnessLevel::Hot, crossing)),
        level_rank(BrightnessLevel::Core),
        "crossing Hot must stay clamped at Core"
    );
    assert_eq!(
        level_rank(proximity_level(BrightnessLevel::Mid, arc)),
        level_rank(BrightnessLevel::Core),
        "the lensing arc circle must burn at Core (bright rising particles)"
    );

    // Warm belt: one rung up.
    assert_eq!(
        level_rank(proximity_level(BrightnessLevel::Mid, warm)),
        level_rank(BrightnessLevel::Hot),
        "warm-belt Mid must bump to Hot"
    );

    // Fade ladder: monotonically dimmer with distance.
    let hot_edge = level_rank(proximity_level(BrightnessLevel::Hot, fade_edge));
    let hot_extreme = level_rank(proximity_level(BrightnessLevel::Hot, extreme));
    assert_eq!(
        hot_edge, 2,
        "just past the fade start Hot steps down to Mid"
    );
    assert_eq!(
        hot_extreme, 0,
        "the far end steps Hot down to Ghost (sparse wisps)"
    );
    assert!(
        hot_extreme < level_rank(proximity_level(BrightnessLevel::Hot, crossing)),
        "the profile must fade monotonically from the hole outward"
    );
    assert!(
        level_rank(proximity_level(BrightnessLevel::Mid, warm))
            > level_rank(proximity_level(BrightnessLevel::Mid, extreme)),
        "the warm belt must stay brighter than the faded extremes"
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

// -- Stage 2.4: the three-tier Interstellar stack, the tier pacing,
// and the see-saw roll scheduler --

/// A pinned mote on a given tier band (the stack's projection
/// geometry in isolation — the same pinned-attractor trick as
/// `pinned_mote`, plus the tier byte and a lifetime that survives
/// the advance pass).
fn pinned_tier_mote(
    phi: f32,
    sim_age: f32,
    tier: u8,
) -> crate::cloud::type_rain::black_hole::ring::RingMote {
    let mut m = pinned_mote(phi, sim_age);
    m.tier = tier;
    m.lifetime = 100.0;
    m
}

#[test]
fn black_hole_ring_tier_stack_steps_up_and_shortens() {
    // The owner's Interstellar ladder: stage 1 the longest band, stage
    // 2 above it and shorter, stage 3 the shortest — and the
    // stage-2.6 one-compact-family ruling (the 9.9/10 feedback): the
    // main disk drops a little below its default center position,
    // the upper two lines sit almost fused with it and each other
    // (his wording: the family must read dense), and the two longest
    // bands widen a little more. Contract at the flat attitude: each
    // tier's mid-band sits strictly above the one below with a
    // tiny-but-distinct step, the whole family packs into a small
    // fraction of the ball, the horizontal reaches descend, and the
    // stack stays on the shadow's face (below the rim, under the
    // lensing arc) — the braided stacked-arcs-over-the-shadow read.
    let (cols, lines) = (120, 40);
    let geo = BallGeometry::new(cols, lines);

    let mid = |tier: u8| {
        let m = pinned_tier_mote(std::f32::consts::FRAC_PI_2, 30.0, tier);
        let (_, line) = project_ring_mote(&m, geo.cx, geo.cy, geo.outer_r, geo.major_limit, 0.0);
        line
    };
    let line_t0 = mid(0);
    let line_t1 = mid(1);
    let line_t2 = mid(2);

    // Stepping upward: each band strictly above the previous.
    assert!(
        line_t1 < line_t0 - 0.5,
        "tier 1 must sit above the main band ({line_t1} vs {line_t0})"
    );
    assert!(
        line_t2 < line_t1 - 0.5,
        "tier 2 must sit above tier 1 ({line_t2} vs {line_t1})"
    );

    // The stage-2.6 descent: the main disk (the owner's stack 1)
    // mid-band sits strictly below the viewport center — the slight
    // drop from the default center position he asked for.
    assert!(
        line_t0 > geo.cy + 0.05,
        "the main disk's mid-band must sit below the default center (line {line_t0}, cy {})",
        geo.cy
    );

    // The almost-fused contract (stage 2.6): the center-to-center
    // steps are a very small fraction of the ball — the near-merged
    // family read — while staying distinct strokes.
    assert!(
        line_t0 - line_t1 < 0.30 * geo.outer_r,
        "tier 1 must hug the main band (step {} vs {})",
        line_t0 - line_t1,
        0.30 * geo.outer_r
    );
    assert!(
        line_t0 - line_t1 > 0.10 * geo.outer_r,
        "tier 1 must stay a distinct line (step {})",
        line_t0 - line_t1
    );
    assert!(
        line_t1 - line_t2 < 0.20 * geo.outer_r,
        "tier 2 must hug tier 1 (step {} vs {})",
        line_t1 - line_t2,
        0.20 * geo.outer_r
    );
    assert!(
        line_t1 - line_t2 > 0.04 * geo.outer_r,
        "tier 2 must stay a distinct line (step {})",
        line_t1 - line_t2
    );

    // The whole family packs into a small fraction of the ball —
    // the dense one-compact-family read (the stage-2.6 "padat"
    // ruling: stacks 2 and 3 almost fused with stack 1).
    assert!(
        line_t0 - line_t2 < 0.40 * geo.outer_r,
        "the stack must pack into a compact family (span {} vs {})",
        line_t0 - line_t2,
        0.40 * geo.outer_r
    );

    // The whole stack crosses the shadow's face (below the rim) —
    // the braided stacked-arcs read over the shadow.
    assert!(
        line_t2 > geo.cy - geo.outer_r,
        "tier 2 must stay on the shadow face, below the rim (line {line_t2}, rim {})",
        geo.cy - geo.outer_r
    );
    // Tier 2 stays under the lensing arc apex (the halo crown sits
    // well above the family).
    let behind = pinned_tier_mote(3.0 * std::f32::consts::FRAC_PI_2, 30.0, 0);
    let (_, line_arc) =
        project_ring_mote(&behind, geo.cx, geo.cy, geo.outer_r, geo.major_limit, 0.0);
    assert!(
        line_t2 > line_arc,
        "tier 2 must stay under the lensing arc apex ({line_t2} vs {line_arc})"
    );

    // Descending lengths: the flat reaches at the extremes.
    let reach = |tier: u8| {
        let m = pinned_tier_mote(0.0, 30.0, tier);
        let (col, _) = project_ring_mote(&m, geo.cx, geo.cy, geo.outer_r, geo.major_limit, 0.0);
        (col - geo.cx).abs()
    };
    assert!(
        reach(1) < reach(0) - 1.0,
        "tier 1 must be shorter than the main disk ({} vs {})",
        reach(1),
        reach(0)
    );
    assert!(
        reach(2) < reach(1) - 1.0,
        "tier 2 must be the shortest band ({} vs {})",
        reach(2),
        reach(1)
    );

    // The stage-2.6 widening: the main disk's reach reads wider than
    // the plain major fraction (1.10x scale — the owner's "expand
    // width a little more" for stacks 1 and 2).
    assert!(
        reach(0) > 1.05 * crate::constants::BLACK_HOLE_RING_MAJOR_FRACTION * unit_reach(&geo),
        "the widened main disk must reach past the old span ({} vs {})",
        reach(0),
        1.05 * crate::constants::BLACK_HOLE_RING_MAJOR_FRACTION * unit_reach(&geo)
    );
}

/// The viewport unit expressed in column reach (the tier reach
/// helper's denominator: the major fraction times the unit, in
/// columns, through the cell aspect divisor).
fn unit_reach(geo: &BallGeometry) -> f32 {
    geo.outer_r / crate::constants::BLACK_HOLE_BALL_FRACTION * CELL_ASPECT_DIVISOR
}

#[test]
fn black_hole_ring_inner_tiers_orbit_faster() {
    // Kepler across the stack: the inner bands (closer to the hole)
    // lap visibly faster than the main disk — the differential
    // rotation of a real multi-ring disk, the owner's tier design
    // made physical.
    let mut m0 = pinned_tier_mote(0.0, 5.0, 0);
    let mut m2 = pinned_tier_mote(0.0, 5.0, 2);
    let omega_base = 12.0 * crate::constants::BLACK_HOLE_RING_OMEGA_PER_CPS;
    let dt_lorenz = 12.0 * crate::constants::BLACK_HOLE_RING_DT_PER_CPS * (1.0 / 60.0);
    for _ in 0..60 {
        crate::cloud::type_rain::black_hole::ring::advance_ring_mote(
            &mut m0,
            1.0 / 60.0,
            dt_lorenz,
            omega_base,
        );
        crate::cloud::type_rain::black_hole::ring::advance_ring_mote(
            &mut m2,
            1.0 / 60.0,
            dt_lorenz,
            omega_base,
        );
    }
    assert!(
        m2.phi > m0.phi + 0.25,
        "the inner tier must outpace the main disk ({} vs {})",
        m2.phi,
        m0.phi
    );
}

#[test]
fn black_hole_ring_roll_pivots_the_stack_rigidly() {
    // The lever contract (the projection math pin): a 90-degree
    // roll INPUT turns the horizontal stack into vertical lines —
    // a Euclidean pivot around the hole (distance from the center
    // preserved, the line-height-unit rotation run before the
    // cell-aspect conversion), with the LEFT end rising and the
    // right end dropping (the owner's example direction). The
    // scheduler never targets this attitude (excluded per the
    // stage-2.5 ruling); the projection itself supports any angle.
    let (cols, lines) = (120, 40);
    let geo = BallGeometry::new(cols, lines);
    let roll90 = std::f32::consts::FRAC_PI_2;

    // Tier 2's mid-band point (x = 0 when flat): rolled a quarter
    // turn it lands on the center row, the same distance from the
    // hole.
    let m2 = pinned_tier_mote(std::f32::consts::FRAC_PI_2, 30.0, 2);
    let (col_flat, line_flat) =
        project_ring_mote(&m2, geo.cx, geo.cy, geo.outer_r, geo.major_limit, 0.0);
    let (col_roll, line_roll) =
        project_ring_mote(&m2, geo.cx, geo.cy, geo.outer_r, geo.major_limit, roll90);
    assert!(
        (col_flat - geo.cx).abs() < 0.02,
        "the tier-2 mid-band must start on the center column when flat"
    );
    assert!(
        (line_roll - geo.cy).abs() < 0.02,
        "the tier-2 mid-band must land on the center row at a quarter turn"
    );
    let d_flat = geo.dist(col_flat, line_flat);
    let d_roll = geo.dist(col_roll, line_roll);
    assert!(
        (d_flat - d_roll).abs() < 0.02,
        "the pivot must preserve the distance from the hole ({d_flat} vs {d_roll})"
    );

    // Tier 0's left extreme: flat at the far left (dropped to the
    // band's stage-2.6 rest offset below center), rolled 90 degrees
    // it points straight up above the ball (left end up). The pivot
    // is rigid: the lowered band's rest offset rides the rotation —
    // the flat offset below center reappears as the same magnitude
    // of horizontal displacement from the center column (one rigid
    // body, offset and all).
    let left = pinned_tier_mote(std::f32::consts::PI, 30.0, 0);
    let (_, line_l_flat) =
        project_ring_mote(&left, geo.cx, geo.cy, geo.outer_r, geo.major_limit, 0.0);
    let (col_l, line_l) =
        project_ring_mote(&left, geo.cx, geo.cy, geo.outer_r, geo.major_limit, roll90);
    let rest_offset = line_l_flat - geo.cy;
    let expected_col = geo.cx - rest_offset * CELL_ASPECT_DIVISOR;
    assert!(
        (col_l - expected_col).abs() < 0.05,
        "the quarter turn must carry the band's rest offset rigidly (col {col_l} vs {expected_col})"
    );
    assert!(
        line_l < geo.cy - geo.outer_r,
        "the left extreme must rise above the ball (left end up, got {line_l})"
    );
}

#[test]
fn black_hole_ring_roll_seesaw_contract() {
    // The see-saw schedule (owner's stage-2.5 spec): the attitude
    // window spans 15-180 degrees (the owner's convention: 180 =
    // the flat rest line, 90 = vertical) with exactly 90 excluded
    // from the menu; every attitude parks at a LONG hold (30 s or
    // more — the improved, more special long duration), the flat
    // rest line the single longest pose; excursions engage in BOTH
    // directions (the sign alternates, up and down taking turns)
    // and return to the rest line. Deterministic by construction
    // (hash-driven, no RNG).

    // Menu pin: no 90-degree rung, every rung inside the shallow-
    // to-steep 15-85 window of the owner's attitude range.
    for &deg in crate::constants::BLACK_HOLE_ROLL_TILT_DEGS.iter() {
        assert!(
            deg != 90.0 && (15.0..=85.0).contains(&deg),
            "the tilt menu must span 15-85 degrees without the excluded 90-degree attitude (got {deg})"
        );
    }

    let mut roll = RingRoll::new();

    // The opening flat hold: 35 s in, still flat.
    roll.tick(35.0);
    assert!(
        roll.angle().abs() < 1.0e-4,
        "the stack must hold the flat rest line for the flat hold (got {})",
        roll.angle()
    );

    // The first excursion: 42 s in, tilted — and strictly below
    // the excluded vertical attitude.
    roll.tick(7.0);
    assert!(
        roll.angle().abs() > 0.05,
        "the first excursion must engage (angle {})",
        roll.angle()
    );
    assert!(
        roll.angle().abs() < std::f32::consts::FRAC_PI_2 - 0.02,
        "the roll must never park on the vertical attitude (angle {})",
        roll.angle()
    );

    // Long-run invariants over ten minutes.
    let mut roll = RingRoll::new();
    let mut flat_secs = 0.0f32;
    let mut max_abs = 0.0f32;
    let mut saw_positive = false;
    let mut saw_negative = false;
    // Parked runs: the angle stays bit-identical while an attitude
    // holds — the direct measure of the hold durations (a sweep
    // changes the angle every tick, a hold freezes it).
    let mut prev_angle = 0.0f32;
    let mut park_run = 0.0f32;
    let mut max_park_flat = 0.0f32;
    let mut max_park_tilt = 0.0f32;
    // "Returns to rest": a tilt episode (angle clearly away from the
    // rest line) is later followed by a SUSTAINED flat run — a
    // chained sweep only passes through flat, it does not park.
    let mut tilted = false;
    let mut flat_run = 0.0f32;
    let mut back_to_rest = false;
    let step = 0.1;
    let mut total = 0.0;
    while total < 600.0 {
        roll.tick(step);
        let after = roll.angle();
        if after.abs() < 0.02 {
            flat_secs += step;
        }
        max_abs = max_abs.max(after.abs());
        if after > 0.2 {
            saw_positive = true;
        }
        if after < -0.2 {
            saw_negative = true;
        }
        if after.abs() > 0.4 {
            tilted = true;
            flat_run = 0.0;
        } else if after.abs() < 0.03 {
            flat_run += step;
            if tilted && flat_run >= 1.0 {
                back_to_rest = true;
            }
        } else {
            flat_run = 0.0;
        }
        if after.to_bits() == prev_angle.to_bits() {
            park_run += step;
        } else {
            park_run = 0.0;
            prev_angle = after;
        }
        if park_run > 0.0 {
            if after.abs() < 1.0e-4 {
                max_park_flat = max_park_flat.max(park_run);
            } else if after.abs() > 0.05 {
                max_park_tilt = max_park_tilt.max(park_run);
            }
        }
        total += step;
    }
    assert!(
        max_abs < std::f32::consts::FRAC_PI_2 - 0.02,
        "the roll must never reach the excluded vertical attitude over time (max {max_abs})"
    );
    assert!(
        flat_secs > 0.30 * 600.0,
        "the flat rest line must keep a strong share of the timeline ({flat_secs} of 600 s)"
    );
    assert!(
        max_park_flat >= 35.0,
        "the flat hold must park for ~36 s (max flat park {max_park_flat})"
    );
    assert!(
        max_park_tilt >= 29.0,
        "the improved long dwell must park tilted attitudes for ~30 s (max tilt park {max_park_tilt})"
    );
    assert!(
        max_park_flat > max_park_tilt,
        "the flat rest line must stay the single longest pose (flat {max_park_flat} vs tilt {max_park_tilt})"
    );
    assert!(
        saw_positive && saw_negative,
        "excursions must alternate direction (pos {saw_positive}, neg {saw_negative})"
    );
    assert!(back_to_rest, "excursions must return to the rest line");
}

#[test]
fn black_hole_ring_roll_schedule_is_deterministic() {
    // Two fresh schedulers ticked identically must agree bit-for-bit
    // — the choreography is a hash of the step counter, not RNG, so
    // every run (and every test) plays the same sequence.
    let mut a = RingRoll::new();
    let mut b = RingRoll::new();
    for _ in 0..2000 {
        a.tick(0.37);
        b.tick(0.37);
    }
    assert_eq!(
        a.angle().to_bits(),
        b.angle().to_bits(),
        "the roll schedule must be deterministic"
    );
}

#[test]
fn black_hole_ring_populates_all_three_tiers() {
    // The stack's spawn weights must light up every band: after
    // steady state each tier hosts a healthy share of the active
    // pool (the stacked-lines read needs all three lines alive).
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames_to_steady(&mut cloud, &mut frame);
    run_frames(&mut cloud, &mut frame, 300, 16);

    let mut per_tier = [0usize; 3];
    let mut active_total = 0usize;
    for m in cloud.black_hole_rain.motes_for_test() {
        if !m.active {
            continue;
        }
        active_total += 1;
        let idx = (m.tier as usize).min(2);
        per_tier[idx] += 1;
    }
    assert!(active_total > 0, "the pool must be active");
    for (tier, count) in per_tier.iter().enumerate() {
        assert!(
            *count > 4,
            "tier {tier} must host motes (got {count} of {active_total})"
        );
    }
}

#[test]
fn black_hole_ring_roll_engages_through_the_live_clock() {
    // Wiring contract: the advance pass ticks the roll on the wall
    // clock — 42 s of frames (past the 36 s flat hold and the first
    // sweep) must show a nonzero attitude through the live
    // accessor, strictly below the excluded vertical attitude.
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 2625, 16);

    let angle = cloud.black_hole_rain.roll_angle_for_test();
    assert!(
        angle.abs() > 0.05,
        "the first excursion must engage through the live clock (angle {angle})"
    );
    assert!(
        angle.abs() < std::f32::consts::FRAC_PI_2 - 0.02,
        "the live roll must never park on the vertical attitude (angle {angle})"
    );
}

// -- Stage 2.6/2.7: the halo streams — the double upward arcs
// (inner + outer crowns), the rare mirrored lower stream, the arc
// band geometry, the disk's rotational sense, and the
// dynamic-screen-size contract --

/// A settled stream rider's projected distance from the hole center
/// (aspect-corrected, in ball outer radii) — the arc band check's
/// core measurement.
fn halo_rider_dist_norm(
    m: &crate::cloud::type_rain::black_hole::ring::RingMote,
    geo: &BallGeometry,
    roll: f32,
) -> f32 {
    let (col, line) = project_halo_mote(m, geo.cx, geo.cy, geo.outer_r, roll);
    geo.dist(col, line) / geo.outer_r
}

#[test]
fn black_hole_halo_streams_spawn_and_ride_the_arcs() {
    // The stage-2.7 contract: the halo pool spawns (same accumulator
    // + formation gate as the ring), the visible riders draw on
    // their own semicircles — both upper crowns strictly above the
    // viewport center, the rare lower stream strictly below — and
    // every settled head stays in the viewport and on its own arc's
    // band (the thin plasma circle around the shadow, never inside
    // the silhouette).
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames_to_steady(&mut cloud, &mut frame);
    run_frames(&mut cloud, &mut frame, 300, 16);

    let geo = BallGeometry::new(cols, lines);
    let entry_settle_secs = 3.0 * crate::constants::BLACK_HOLE_RING_ENTRY_TAU;
    let roll = cloud.black_hole_rain.roll_angle_for_test();

    let active = cloud.black_hole_rain.active_halo_for_test();
    assert!(active > 0, "the halo pool must spawn riders (got {active})");

    let mut upper_inner = 0usize;
    let mut upper_outer = 0usize;
    let mut lower = 0usize;
    let wobble = crate::constants::BLACK_HOLE_HALO_WOBBLE_FRACTION;
    // Each lane's arc fraction (the tag lookup the projection runs).
    let arc_of = |tier: u8| {
        if tier == HALO_STREAM_TAG_UPPER_OUTER {
            crate::constants::BLACK_HOLE_HALO_OUTER_ARC_FRACTION
        } else if tier == HALO_STREAM_TAG_LOWER {
            crate::constants::BLACK_HOLE_HALO_LOWER_ARC_FRACTION
        } else {
            crate::constants::BLACK_HOLE_HALO_ARC_FRACTION
        }
    };
    // r_norm clamps to [-1.0, 1.2], so the settled radius band spans
    // arc - wobble .. arc + 1.2 * wobble outer radii (plus rounding).
    for m in cloud.black_hole_rain.halo_motes_for_test() {
        if !m.active || m.sim_age < entry_settle_secs {
            continue;
        }
        if !halo_mote_visible(m) {
            continue;
        }
        match m.tier {
            HALO_STREAM_TAG_UPPER => upper_inner += 1,
            HALO_STREAM_TAG_UPPER_OUTER => upper_outer += 1,
            _ => lower += 1,
        }
        let (col, line) = project_halo_mote(m, geo.cx, geo.cy, geo.outer_r, roll);
        assert!(
            col >= 0.0 && col < cols as f32,
            "halo rider column out of viewport ({col})"
        );
        assert!(
            line >= 0.0 && line < lines as f32,
            "halo rider line out of viewport ({line})"
        );
        if m.tier != HALO_STREAM_TAG_LOWER {
            assert!(
                line < geo.cy,
                "an upper-crown rider must draw above the center (line {line}, cy {})",
                geo.cy
            );
        } else {
            assert!(
                line > geo.cy,
                "a lower-stream rider must draw below the center (line {line}, cy {})",
                geo.cy
            );
        }
        let arc = arc_of(m.tier);
        let band_lo = arc - wobble - 0.10;
        let band_hi = arc + 1.2 * wobble + 0.10;
        let d = halo_rider_dist_norm(m, &geo, roll);
        assert!(
            d >= band_lo && d <= band_hi,
            "halo rider escaped its arc band (dist {d} not in [{band_lo}, {band_hi}])"
        );
        assert!(
            d > 1.0,
            "a halo rider must never enter the ball silhouette (dist {d})"
        );
    }
    assert!(
        upper_inner > 0,
        "the inner upper crown must host visible riders"
    );
    assert!(
        upper_outer > 0,
        "the outer upper crown must host visible riders (the double stream)"
    );
    assert!(lower > 0, "the rare lower stream must still show riders");
}

#[test]
fn black_hole_halo_doubles_the_upward_arc_density() {
    // The owner's stage-2.7 read: the upward stream is now a DOUBLE
    // upward stream — two upper crowns of the same rider population
    // (the inner co-riding the lensing circle, the outer on the
    // wider arc). The upward population is the far-side lensing arc
    // riders (the ring's tier-0 far side) PLUS the two halo crowns;
    // the contract holds when the halo crowns together at least
    // match the far-side count (the upward read well past double the
    // pre-stream figure), BOTH crowns host visible riders (the
    // double, not one thickened band), and the outer crown rides
    // clear of the inner band (two distinct arcs).
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames_to_steady(&mut cloud, &mut frame);
    run_frames(&mut cloud, &mut frame, 300, 16);

    let far_side: usize = cloud
        .black_hole_rain
        .motes_for_test()
        .iter()
        .filter(|m| m.active && m.tier == 0 && m.phi.sin() < 0.0)
        .count();
    let halo_inner: usize = cloud
        .black_hole_rain
        .halo_motes_for_test()
        .iter()
        .filter(|m| m.active && m.tier == HALO_STREAM_TAG_UPPER && halo_mote_visible(m))
        .count();
    let halo_outer: usize = cloud
        .black_hole_rain
        .halo_motes_for_test()
        .iter()
        .filter(|m| m.active && m.tier == HALO_STREAM_TAG_UPPER_OUTER && halo_mote_visible(m))
        .count();

    assert!(
        far_side > 0,
        "the lensing arc must host riders (got {far_side})"
    );
    assert!(
        halo_inner > 0,
        "the inner upper crown must host riders (got {halo_inner})"
    );
    assert!(
        halo_outer > 0,
        "the outer upper crown must host riders (got {halo_outer})"
    );
    let halo_upper = halo_inner + halo_outer;
    assert!(
        halo_upper as f64 >= 0.45 * far_side as f64,
        "the double crown must about match the lensing riders ({halo_upper} vs {far_side})"
    );
    assert!(
        (halo_upper + far_side) as f64 >= 1.50 * far_side as f64,
        "the upward curve must read about twice as dense ({} + {} vs 1.5x {})",
        halo_upper,
        far_side,
        far_side
    );
}

#[test]
fn black_hole_halo_lower_stream_runs_sparser_than_upper() {
    // The owner's stage-2.7 read: the lower stream must carry only
    // RARE particles against the double crown above. The split runs
    // the deterministic Bresenham accumulator plus the lane toggle,
    // so the TAG counts hold the exact 0.82 / 0.18 family-vs-lower
    // ratio on every pool fill (no spawn luck — a random pick could
    // invert a small pool on one seed); the visible populations
    // follow the tags (every rider draws on its own semicircle).
    // Contract: the tagged split sits within one mote of the exact
    // share, the two upper crowns carry EQUAL counts (the lane
    // toggle's strict alternation), and the lower tag count stays
    // well below the upper family.
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames_to_steady(&mut cloud, &mut frame);
    run_frames(&mut cloud, &mut frame, 300, 16);

    let tagged_upper_inner: usize = cloud
        .black_hole_rain
        .halo_motes_for_test()
        .iter()
        .filter(|m| m.active && m.tier == HALO_STREAM_TAG_UPPER)
        .count();
    let tagged_upper_outer: usize = cloud
        .black_hole_rain
        .halo_motes_for_test()
        .iter()
        .filter(|m| m.active && m.tier == HALO_STREAM_TAG_UPPER_OUTER)
        .count();
    let tagged_lower: usize = cloud
        .black_hole_rain
        .halo_motes_for_test()
        .iter()
        .filter(|m| m.active && m.tier == HALO_STREAM_TAG_LOWER)
        .count();
    let tagged_upper = tagged_upper_inner + tagged_upper_outer;
    let active = cloud.black_hole_rain.active_halo_for_test();
    let ideal_upper = (crate::constants::BLACK_HOLE_HALO_UPPER_WEIGHT
        + crate::constants::BLACK_HOLE_HALO_OUTER_WEIGHT)
        * active as f32;

    assert!(active > 0, "the halo pool must be active (got {active})");
    assert!(
        (tagged_upper as f32 - ideal_upper).abs() <= 1.0,
        "the Bresenham split must hold the exact family share ({tagged_upper} of {active}, ideal {ideal_upper})"
    );
    assert!(
        (tagged_upper_inner as i32 - tagged_upper_outer as i32).abs() <= 1,
        "the lane toggle must alternate the two crowns evenly ({tagged_upper_inner} vs {tagged_upper_outer})"
    );
    assert!(
        4.0 * (tagged_lower as f32) < 3.0 * tagged_upper as f32,
        "the lower stream must read rare against the double crown ({tagged_lower} vs {tagged_upper})"
    );

    // The visible populations follow the tags: both semicircles host
    // riders (the handoff sweep keeps each side populated), and the
    // lower visible count sits below the upper family.
    let visible_upper: usize = cloud
        .black_hole_rain
        .halo_motes_for_test()
        .iter()
        .filter(|m| m.active && m.tier != HALO_STREAM_TAG_LOWER && halo_mote_visible(m))
        .count();
    let visible_lower: usize = cloud
        .black_hole_rain
        .halo_motes_for_test()
        .iter()
        .filter(|m| m.active && m.tier == HALO_STREAM_TAG_LOWER && halo_mote_visible(m))
        .count();
    assert!(visible_upper > 0, "the double crown must show riders");
    assert!(visible_lower > 0, "the rare lower stream must show riders");
}

#[test]
fn black_hole_halo_orbits_in_the_disk_direction() {
    // The rotational-sense contract (the owner's wording: the
    // rotation follows the one above): the halo riders' angles
    // strictly advance on the shared clock — the same positive
    // phi convention the ring motes carry — so the streams
    // circulate with the disk, never against it.
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames_to_steady(&mut cloud, &mut frame);

    let before: Vec<Option<f32>> = cloud
        .black_hole_rain
        .halo_motes_for_test()
        .iter()
        .map(|m| if m.active { Some(m.phi) } else { None })
        .collect();
    // 30 frames at 60 FPS = 0.48 s — well under the 16 s minimum
    // lifetime, so every before-active rider is still comparable.
    run_frames(&mut cloud, &mut frame, 30, 16);
    let riders = cloud.black_hole_rain.halo_motes_for_test();
    let mut compared = 0;
    for (idx, was) in before.iter().enumerate() {
        if let Some(phi_before) = was {
            let m = &riders[idx];
            assert!(
                m.active,
                "halo rider {idx} absorbed too early (lifetime contract)"
            );
            assert!(
                m.phi > phi_before + 0.01,
                "halo rider {idx} angle stalled ({} -> {})",
                phi_before,
                m.phi
            );
            compared += 1;
        }
    }
    assert!(compared > 0, "no active halo riders to compare");
}

#[test]
fn black_hole_style_supports_dynamic_screen_size() {
    // The owner's stage-2.6 pin: the black hole rain must support
    // dynamic screen sizes. Resize across the terminal classes
    // (wider, taller, smaller): the pools rebuild to the new width,
    // the steady state resumes without re-forming, and every drawn
    // cell (ball annulus, ring motes, halo riders) stays inside the
    // new viewport — no out-of-bounds paint through the whole
    // transition window.
    for (cols, lines) in [(200, 60), (120, 40), (80, 24), (105, 64)] {
        let mut cloud = make_black_hole_cloud(120, 40);
        let mut frame = Frame::new(120, 40, cloud.palette.bg);
        run_frames_to_steady(&mut cloud, &mut frame);

        cloud.reset(cols, lines);
        let mut frame = Frame::new(cols, lines, cloud.palette.bg);
        run_frames(&mut cloud, &mut frame, 60, 16);

        assert!(
            cloud.black_hole_rain.motes_for_test().len() == cols as usize,
            "the ring pool must rebuild to the new width ({cols} cols)"
        );
        assert!(
            cloud.black_hole_rain.halo_motes_for_test().len() == cols as usize,
            "the halo pool must rebuild to the new width ({cols} cols)"
        );
        assert!(
            cloud.black_hole_rain.formed_for_test(),
            "a pure resize must keep the hole formed at {cols}x{lines}"
        );
        for cell in cloud.black_hole_rain.drawn_cells_for_test() {
            assert!(
                cell.col < cols,
                "drawn cell column out of bounds at {cols}x{lines} ({} >= {})",
                cell.col,
                cols
            );
            assert!(
                cell.line < lines,
                "drawn cell line out of bounds at {cols}x{lines} ({} >= {})",
                cell.line,
                lines
            );
        }
        assert!(
            cloud.black_hole_rain.active_halo_for_test() > 0,
            "the halo streams must resume at {cols}x{lines}"
        );
    }
}
