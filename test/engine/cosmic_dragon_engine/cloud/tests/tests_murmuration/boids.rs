// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Boids-physics-level behavior contracts (NIGHT-research-7): the
//! flight band's speed clamps, the separation floor, the startle
//! impulse's scatter, the spatial hash's window, the anchor's
//! roam, and the wall banking's steering.

use crate::cloud::murmuration::boids::{
    clamp_speed, flock_forces, integrate_bird, startle, Anchor, Bird, BirdRandom, Edge, FlockCtx,
    MurmHash,
};
use crate::cloud::murmuration::murmuration::MurmurationRain;
use crate::constants::{MURM_NEIGHBOR_R, MURM_PANIC_IMPULSE, MURM_SPEED_MAX};
use rand::{distr::Uniform, rngs::StdRng, SeedableRng};

fn fixed_random(seed: u64) -> (StdRng, Uniform<f32>) {
    (
        StdRng::seed_from_u64(seed),
        Uniform::new(0.0, 1.0).expect("unit interval"),
    )
}

#[test]
fn speed_clamp_preserves_heading_and_band() {
    // A slow bird speeds up to min, a fast bird slows to max, and
    // the heading (the velocity ratio) survives the clamp.
    let mut b = Bird::vacant();
    b.vx = 0.0;
    b.vy = 2.0;
    clamp_speed(&mut b, 7.0, 26.0);
    assert!((b.speed() - 7.0).abs() < 1e-4, "slow bird not floored");
    assert!((b.vx / b.speed()).abs() < 1e-4, "heading lost (was +y)");

    b.vx = 60.0;
    b.vy = 0.0;
    clamp_speed(&mut b, 7.0, 26.0);
    assert!((b.speed() - 26.0).abs() < 1e-4, "fast bird not capped");

    // Degenerate: zero velocity re-seeds a straight heading.
    b.vx = 0.0;
    b.vy = 0.0;
    clamp_speed(&mut b, 7.0, 26.0);
    assert!(b.speed() >= 7.0 - 1e-4, "degenerate bird not re-seeded");
}

#[test]
fn separation_pushes_neighbors_apart() {
    // Two birds stacked nearly on top of each other: the force
    // accumulation points from the other bird toward this one
    // (the push apart), zero at distance.
    let mut other = Bird::vacant();
    other.active = true;
    other.x = 40.0;
    other.y = 20.0;
    other.vx = 5.0;
    other.vy = 0.0;
    let mut b = Bird::vacant();
    b.active = true;
    b.x = 41.0;
    b.y = 20.0;
    b.vx = 5.0;
    b.vy = 0.0;
    let anchor = Anchor::new();
    let birds = [other, b];
    let ctx = FlockCtx {
        birds: &birds,
        anchor: &anchor,
        coh_weight: 1.0,
        cols: 80,
        lines: 40,
    };
    let (ax, _ay) = flock_forces(&birds[1], &[0, 1], 1, &ctx);
    // This bird sits at x+1 of the other: separation pushes it
    // further +x (away).
    assert!(ax > 0.0, "separation does not push apart: ax={ax}");
}

#[test]
fn cohesion_pulls_toward_local_centroid() {
    // A lone pair with matching velocities INSIDE the neighbor
    // window (d < NEIGHBOR_R): the cohesion term pulls each
    // toward the other (through the breathing weight), while the
    // matched velocities zero the alignment term.
    let mut other = Bird::vacant();
    other.active = true;
    other.x = 36.0;
    other.y = 20.0;
    other.vx = 5.0;
    other.vy = 1.0;
    let mut b = Bird::vacant();
    b.active = true;
    b.x = 30.0;
    b.y = 20.0;
    b.vx = 5.0;
    b.vy = 1.0;
    let anchor = Anchor::new();
    let birds = [other, b];
    let ctx = FlockCtx {
        birds: &birds,
        anchor: &anchor,
        coh_weight: 2.0,
        cols: 80,
        lines: 40,
    };
    let (ax, _ay) = flock_forces(&birds[1], &[0, 1], 1, &ctx);
    // The neighbor is at +x: the net pull (cohesion minus the
    // separation edge case at d=20 > SEP_R) points +x.
    assert!(ax > 0.0, "cohesion does not pull toward centroid: ax={ax}");
}

#[test]
fn wall_banking_steers_inward() {
    // A bird near the left margin banks right (inward); centered
    // birds get no wall force.
    let mut b = Bird::vacant();
    b.active = true;
    b.x = 2.0;
    b.y = 20.0;
    b.vx = 0.0;
    b.vy = 5.0;
    let anchor = Anchor::new();
    let birds = [b];
    let ctx = FlockCtx {
        birds: &birds,
        anchor: &anchor,
        coh_weight: 1.0,
        cols: 80,
        lines: 40,
    };
    let (ax, _ay) = flock_forces(&birds[0], &[0], 0, &ctx);
    assert!(ax > 0.0, "no inward banking at the left margin: ax={ax}");

    b.x = 40.0;
    b.y = 20.0;
    let birds = [b];
    let ctx = FlockCtx {
        birds: &birds,
        anchor: &anchor,
        coh_weight: 1.0,
        cols: 80,
        lines: 40,
    };
    let (ax, ay) = flock_forces(&birds[0], &[0], 0, &ctx);
    // No walls, no neighbors: only the anchor pull remains
    // (toward the center-ish anchor) — the wall contribution is
    // zero by construction (both inside the margin band).
    assert!(ax.abs() < 0.0 + 60.0);
    assert!(ay.abs() < 60.0);
    assert!(
        (ax - (anchor.x - 40.0) * crate::constants::MURM_ANCHOR_W).abs() < 1e-3,
        "centered bird's force is not pure anchor pull: ax={ax}"
    );
}

#[test]
fn startle_kicks_birds_away_from_predator() {
    // A bird inside the panic radius takes an outward kick; a
    // bird outside takes none.
    let mut b = Bird::vacant();
    b.x = 40.0;
    b.y = 20.0;
    b.vx = 0.0;
    b.vy = 8.0;
    startle(&mut b, 30.0, 20.0);
    assert!(
        b.vx > 0.0,
        "startle did not kick the bird away: vx={}",
        b.vx
    );
    assert!(b.panicked(), "startle did not arm the panic window");
    assert!(b.speed() > 8.0, "the kick added no speed: {}", b.speed());
    let mut far = Bird::vacant();
    far.x = 0.0;
    far.y = 39.0;
    far.vx = 1.0;
    far.vy = 1.0;
    startle(&mut far, 79.0, 0.0);
    assert!(!far.panicked(), "a bird outside the radius was startled");
}

#[test]
fn startle_impulse_saturates_at_speed_max() {
    // The kick lands on the velocity, but the clamp caps the
    // result: no bird ever exceeds V_MAX through a startle.
    let mut b = Bird::vacant();
    b.x = 40.0;
    b.y = 20.0;
    b.vx = 0.0;
    b.vy = 8.0;
    startle(&mut b, 39.5, 20.0);
    clamp_speed(&mut b, 7.0, MURM_SPEED_MAX);
    assert!(
        b.speed() <= MURM_SPEED_MAX + 1e-4,
        "startle exceeded the flight band: {}",
        b.speed()
    );
    assert!(
        b.speed() >= MURM_PANIC_IMPULSE * 0.0 + MURM_SPEED_MAX * 0.8 - 1e-4,
        "the panicking bird did not floor near max"
    );
}

#[test]
fn integration_keeps_birds_in_bounds() {
    // A bird flying hard at the corner: the integration's
    // re-project backstop keeps it inside the viewport, and the
    // speed stays in the band.
    let mut b = Bird::vacant();
    b.x = 79.0;
    b.y = 39.0;
    b.vx = 100.0;
    b.vy = 100.0;
    integrate_bird(&mut b, 0.0, 0.0, 0.016, 80, 40);
    assert!(b.x >= 0.0 && b.x < 80.0, "x out of bounds: {}", b.x);
    assert!(b.y >= 0.0 && b.y < 40.0, "y out of bounds: {}", b.y);
    assert!(b.speed() <= MURM_SPEED_MAX + 1e-4, "speed out of band");
}

#[test]
fn hash_window_finds_near_neighbors() {
    // The 3x3 bucket scan: a bird at the grid center finds its
    // close neighbors and not the far ones.
    let mut birds: Vec<Bird> = Vec::new();
    for (i, (x, y)) in [(40.0, 20.0), (42.0, 20.0), (10.0, 5.0), (75.0, 35.0)]
        .into_iter()
        .enumerate()
    {
        let mut b = Bird::vacant();
        b.active = true;
        b.x = x;
        b.y = y;
        b.vx = 5.0 + i as f32;
        b.vy = 1.0;
        birds.push(b);
    }
    let mut hash = MurmHash::default();
    hash.rebuild(&birds, 80, 40);
    let mut out = Vec::new();
    hash.neighbors(40.0, 20.0, &mut out);
    assert!(out.contains(&0), "self missing from the window");
    assert!(out.contains(&1), "near neighbor missing from the window");
    assert!(!out.contains(&2), "far neighbor leaked into the window");
    assert!(!out.contains(&3), "far neighbor leaked into the window");

    // The window spans the bucket boundary: a neighbor across
    // the boundary IS a window member (the 3x3 scan).
    let mut near_boundary = Bird::vacant();
    near_boundary.active = true;
    near_boundary.x = 40.0 + MURM_NEIGHBOR_R * 0.9;
    near_boundary.y = 20.0 + MURM_NEIGHBOR_R * 0.9;
    birds.push(near_boundary);
    hash.rebuild(&birds, 80, 40);
    let mut out2 = Vec::new();
    hash.neighbors(40.0, 20.0, &mut out2);
    assert!(out2.contains(&4), "boundary neighbor missing");
}

#[test]
fn hash_rebuild_skips_inactive_birds() {
    let mut birds = vec![Bird::vacant(); 4];
    birds[0].active = true;
    birds[0].x = 40.0;
    birds[0].y = 20.0;
    let mut hash = MurmHash::default();
    hash.rebuild(&birds, 80, 40);
    let mut out = Vec::new();
    hash.neighbors(40.0, 20.0, &mut out);
    assert_eq!(out, vec![0], "inactive birds leaked into the hash");
}

#[test]
fn anchor_roams_and_reflects() {
    // The anchor walks within the viewport and reflects off the
    // walls (never exits).
    let (mut rng, chance) = fixed_random(7);
    let mut random = BirdRandom {
        rng: &mut rng,
        rand_chance: &chance,
    };
    let mut a = Anchor::new();
    a.reset(80, 40);
    for _ in 0..600 {
        a.advance(0.05, 80, 40, &mut random);
        assert!(a.x >= 0.0 && a.x <= 79.0, "anchor x out of bounds: {}", a.x);
        assert!(a.y >= 0.0 && a.y <= 39.0, "anchor y out of bounds: {}", a.y);
    }
    // It moved somewhere over 30 sim-seconds.
    assert!(a.x != 40.0 || a.y != 16.0, "the anchor never roamed");
}

#[test]
fn edge_activation_flies_inward() {
    // A bird activated at the top edge flies downward (into the
    // sky); from the right edge, leftward.
    let mut b = Bird::vacant();
    b.activate_at_edge(Edge::Top, 40.0, 3, 0.5);
    assert!(b.vy > 0.0, "top-edge bird does not fly inward");
    let mut r = Bird::vacant();
    r.activate_at_edge(Edge::Right, 20.0, 3, 0.5);
    assert!(r.vx < 0.0, "right-edge bird does not fly inward");
    assert!(r.x < 0.0, "right-edge bird does not start off-screen");
    let mut l = Bird::vacant();
    l.activate_at_edge(Edge::Left, 20.0, 3, 0.5);
    assert!(
        l.vx > 0.0 && l.x >= 0.0,
        "left-edge bird does not fly inward"
    );
}

#[test]
fn flock_target_clamps_to_band() {
    // The population dial: viewport-derived, clamped, monotone
    // in density.
    let narrow = MurmurationRain::flock_target(20, 0.55);
    assert!(
        narrow >= crate::constants::MURM_MIN_BIRDS,
        "narrow flock under the floor"
    );
    let wide = MurmurationRain::flock_target(400, 0.55);
    assert!(
        wide <= crate::constants::MURM_MAX_BIRDS,
        "wide flock over the cap"
    );
    assert!(
        MurmurationRain::flock_target(120, 1.0) >= MurmurationRain::flock_target(120, 0.4),
        "flock target not monotone in density"
    );
    let tiny = MurmurationRain::flock_target(1, 0.1);
    assert!(
        tiny >= crate::constants::MURM_MIN_BIRDS,
        "degenerate viewport under the floor"
    );
}
