// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-special-1 stage 2 tests: the orbital ring — RK4-Lorenz
//! turbulence on a tilted Keplerian ellipse around the ball. Covers
//! spawning + orbital advance, the band geometry, the far-side
//! occlusion contract, style-transition recycling, and the shipped
//! motion constants (RK4 stability regime, majestic lap pace).

use std::collections::HashSet;

use super::*;
use crate::cloud::type_rain::black_hole::ring::{occludes_ring_cell, project_ring_mote};

/// Shared geometry for the projection checks (mirrors the ball
/// raster's own math: integer center, line-height units).
struct BallGeometry {
    cx: f32,
    cy: f32,
    outer_r: f32,
}

impl BallGeometry {
    fn new(cols: u16, lines: u16) -> Self {
        let unit = (cols as f32 / 4.0).min(lines as f32 / 2.0);
        Self {
            cx: ((cols - 1) / 2) as f32,
            cy: ((lines - 1) / 2) as f32,
            outer_r: unit * crate::constants::BLACK_HOLE_BALL_FRACTION,
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
    run_frames(&mut cloud, &mut frame, 60, 16);

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
    run_frames(&mut cloud, &mut frame, 90, 16);

    let geo = BallGeometry::new(cols, lines);
    let band_max = geo.outer_r
        * (crate::constants::BLACK_HOLE_RING_RADIUS_FRACTION
            + crate::constants::BLACK_HOLE_RING_WOBBLE_FRACTION * 1.2);

    let mut projected = 0;
    for m in cloud.black_hole_rain.motes_for_test() {
        if !m.active {
            continue;
        }
        projected += 1;
        let (col_f, line_f) = project_ring_mote(m, geo.cx, geo.cy, geo.outer_r);
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
    assert!(projected > 0, "no active motes to project");
}

#[test]
fn black_hole_ring_occlusion_hides_far_side() {
    // The 3D layering contract: no mote cell inside the ball
    // silhouette ABOVE the viewport center ever reaches the frame —
    // the far side of the tilted disk passes behind the hole. The
    // ball's own annulus cells are excluded from the check (they
    // legitimately occupy the upper silhouette).
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 120, 16);

    let geo = BallGeometry::new(cols, lines);
    let cy_i = ((lines - 1) / 2) as i32;
    let cx_i = ((cols - 1) / 2) as i32;

    let ball_cells: HashSet<(u16, u16)> = cloud
        .black_hole_rain
        .ring_cells_for_test()
        .iter()
        .map(|c| (c.col, c.line))
        .collect();
    let drawn = cloud.black_hole_rain.drawn_cells_for_test();
    let mut mote_cells = 0;
    for cell in drawn {
        if ball_cells.contains(&(cell.col, cell.line)) {
            continue;
        }
        mote_cells += 1;
        // The occlusion rule itself must agree with the drawn set:
        // every drawn mote cell is either outside the silhouette or
        // on the near side (below the center line).
        assert!(
            !occludes_ring_cell(cell.col, cell.line, cx_i, cy_i, geo.outer_r),
            "far-side mote cell drawn through the ball at ({}, {})",
            cell.col,
            cell.line
        );
        if geo.dist(cell.col as f32, cell.line as f32) < geo.outer_r - 0.25 {
            assert!(
                cell.line > cy_i as u16,
                "mote cell inside the silhouette above center at ({}, {})",
                cell.col,
                cell.line
            );
        }
    }
    assert!(mote_cells > 0, "no ring cells drawn this frame");
}

#[test]
fn black_hole_ring_survives_style_transition() {
    // Family contract: leaving the style wipes the mote pool (no
    // orbiting ghosts in another scene), re-entry respawns it from a
    // clean baseline.
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 60, 16);
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
    run_frames(&mut cloud, &mut frame, 30, 16);
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
