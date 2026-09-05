// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Numerical solver contracts (task-19): P2G conservation, momentum
//! averaging, gravity force, projection divergence reduction, wall
//! boundary, and out-of-range clamping. These pin the PIC/FLIP grid
//! half — the contracts that make the style a real fluid method
//! rather than a scripted effect.

use crate::cloud::flux_field::{FluxField, FluxVel};

const EPS: f32 = 1.0e-4;

#[test]
fn flux_field_sizing_follows_viewport() {
    // 120x40: x in [0,120) -> 61 nodes; y in [0,20) -> 11 nodes
    // (spacing 2 screen units on both axes).
    let f = FluxField::new(120, 40);
    assert_eq!(f.dims(), (61, 11));
    // Degenerate viewports clamp to a valid 2x2+ field.
    let tiny = FluxField::new(1, 1);
    let (w, h) = tiny.dims();
    assert!(
        w >= 3 && h >= 3,
        "degenerate field stays indexable ({w}x{h})"
    );
}

#[test]
fn flux_p2g_empty_field_is_calm() {
    // No splats: the vacuum contract — empty regions carry zero
    // velocity, gravity does not apply without particle weight.
    let mut f = FluxField::new(120, 40);
    f.begin_p2g();
    f.finish_p2g();
    f.apply_gravity_snapshot_project(crate::constants::FLUX_SIM_DT, 55.0);
    let v = f.sample(60.0, 10.0);
    assert!(
        v.vx.abs() < EPS && v.vy.abs() < EPS,
        "empty field stays calm"
    );
}

#[test]
fn flux_p2g_splat_identity_at_node() {
    // A single particle sitting exactly on a node conveys its full
    // velocity to the field there (bilinear identity).
    let mut f = FluxField::new(120, 40);
    f.begin_p2g();
    f.splat(60.0, 10.0, FluxVel { vx: 3.0, vy: 9.0 });
    f.finish_p2g();
    let v = f.sample(60.0, 10.0);
    assert!((v.vx - 3.0).abs() < EPS, "vx identity at node ({})", v.vx);
    assert!((v.vy - 9.0).abs() < EPS, "vy identity at node ({})", v.vy);
}

#[test]
fn flux_p2g_two_particles_average_momentum() {
    // Two particles at the same node with velocities 4 and 8: the
    // normalized node carries their momentum average (6).
    let mut f = FluxField::new(120, 40);
    f.begin_p2g();
    f.splat(60.0, 10.0, FluxVel { vx: 4.0, vy: 4.0 });
    f.splat(60.0, 10.0, FluxVel { vx: 8.0, vy: 8.0 });
    f.finish_p2g();
    let v = f.sample(60.0, 10.0);
    assert!((v.vx - 6.0).abs() < EPS, "momentum average vx ({})", v.vx);
    assert!((v.vy - 6.0).abs() < EPS, "momentum average vy ({})", v.vy);
}

#[test]
fn flux_gravity_adds_weighted_downward_velocity() {
    // Gravity applies only on weight-carrying nodes; screen space
    // has +y downward, so gravity adds +vy.
    let dt = crate::constants::FLUX_SIM_DT;
    let mut f = FluxField::new(120, 40);
    f.begin_p2g();
    f.splat(60.0, 10.0, FluxVel { vx: 0.0, vy: 0.0 });
    f.finish_p2g();
    f.apply_gravity_snapshot_project(dt, 55.0);
    let v = f.sample(60.0, 10.0);
    // Post-projection the value shifts slightly (mass conservation
    // spreads the momentum), but the downward impulse dominates.
    assert!(
        v.vy > 55.0 * dt * 0.5,
        "gravity impulse must survive the projection ({})",
        v.vy
    );
}

#[test]
fn flux_projection_reduces_divergence() {
    // THE incompressibility contract: a deliberately divergent
    // field (all nodes blowing outward from the center) must have
    // its divergence sharply reduced by the Jacobi projection.
    let mut f = FluxField::new(120, 40);
    let (w, h) = f.dims();
    // Radial outflow: velocity points away from the grid center.
    let (ci, cj) = (w as f32 * 0.5, h as f32 * 0.5);
    for j in 0..h {
        for i in 0..w {
            let dx = i as f32 - ci;
            let dy = j as f32 - cj;
            let len = (dx * dx + dy * dy).sqrt().max(1.0);
            f.set_velocity_for_test(i, j, dx / len * 5.0, dy / len * 5.0);
        }
    }
    let before = f.max_abs_divergence_for_test();
    f.apply_gravity_snapshot_project(crate::constants::FLUX_SIM_DT, 0.0);
    let after = f.max_abs_divergence_for_test();
    assert!(
        after < before * 0.5,
        "projection must halve the divergence ({before} -> {after})"
    );
}

#[test]
fn flux_wall_boundary_blocks_through_flow() {
    // After a projection the left/right wall columns carry zero
    // horizontal velocity (no-through-flow); vertical slip stays.
    let mut f = FluxField::new(120, 40);
    f.begin_p2g();
    // A strong jet aimed at the left wall.
    f.splat(6.0, 10.0, FluxVel { vx: -8.0, vy: 12.0 });
    f.finish_p2g();
    f.apply_gravity_snapshot_project(crate::constants::FLUX_SIM_DT, 0.0);
    let (w, h) = f.dims();
    let _ = w;
    for j in 0..h {
        let left = f.wall_u_for_test(j, false);
        let right = f.wall_u_for_test(j, true);
        assert!(
            left.abs() < EPS && right.abs() < EPS,
            "walls block through-flow (left {left}, right {right} at row {j})"
        );
    }
}

#[test]
fn flux_sample_clamps_outside_positions() {
    // Extreme positions (far outside the viewport) clamp to the
    // border instead of panicking — the rim splat/sample contract.
    let mut f = FluxField::new(120, 40);
    f.begin_p2g();
    f.splat(-50.0, -50.0, FluxVel { vx: 1.0, vy: 1.0 });
    f.finish_p2g();
    let v = f.sample(1.0e6, 1.0e6);
    assert!(v.vx.is_finite() && v.vy.is_finite());
    let v2 = f.sample(-1.0e6, -1.0e6);
    assert!(v2.vx.is_finite() && v2.vy.is_finite());
}

#[test]
fn flux_splat_rejects_non_finite_inputs() {
    // NaN/inf inputs must be silently dropped (defensive contract —
    // one bad mote can never poison the grid).
    let mut f = FluxField::new(120, 40);
    f.begin_p2g();
    f.splat(f32::NAN, 10.0, FluxVel { vx: 1.0, vy: 1.0 });
    f.splat(60.0, f32::INFINITY, FluxVel { vx: 1.0, vy: 1.0 });
    f.splat(
        60.0,
        10.0,
        FluxVel {
            vx: f32::NAN,
            vy: 1.0,
        },
    );
    f.finish_p2g();
    let v = f.sample(60.0, 10.0);
    assert!(
        v.vx.abs() < EPS && v.vy.abs() < EPS,
        "non-finite splats are dropped"
    );
}
