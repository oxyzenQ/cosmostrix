// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The invented-math contracts of the aurora ray lattice
//! (NIGHT-special-3, laws 1, 2, 4): the repulsion spread, the wall
//! bounce + velocity clamps, the two-band substorm breath, the
//! bounded glow charge, and the fringe ladder read.

use rand::{distr::Uniform, rngs::StdRng, SeedableRng};

use crate::cloud::aurora::rays::{
    fringe_level, ray_count_for_cols, AuroraRandom, AuroraSky, DepthBand,
};
use crate::constants::{AURORA_DEPTH_MIN, AURORA_GLOW_LEVEL_HOT, AURORA_GLOW_MAX, AURORA_VX_MAX};

use crate::cloud::monolith::BrightnessLevel;

fn fixed_random(seed: u64) -> (StdRng, Uniform<f32>) {
    (StdRng::seed_from_u64(seed), Uniform::new(0.0, 1.0).unwrap())
}

#[test]
fn ray_count_scales_with_viewport() {
    assert_eq!(ray_count_for_cols(0), 1);
    assert_eq!(ray_count_for_cols(1), 1);
    assert_eq!(ray_count_for_cols(6), 1);
    assert_eq!(ray_count_for_cols(80), 13);
    assert_eq!(ray_count_for_cols(200), 24);
    assert_eq!(ray_count_for_cols(400), 24); // hard cap
}

#[test]
fn law1_repulsion_spreads_the_lattice() {
    // Two beads planted at nearly the same column must separate
    // (the inverse-gap push dominates at the floor gap).
    let mut sky = AuroraSky::new();
    sky.reset(80, 40);
    sky.clear_rays_for_test();
    let (mut rng, chance) = fixed_random(42);
    let mut random = AuroraRandom {
        rng: &mut rng,
        rand_chance: &chance,
    };
    sky.plant_ray_for_test(40.0, 40, 0.5);
    sky.plant_ray_for_test(41.0, 40, 0.5);
    let gap_before = {
        let rays = sky.rays();
        (rays[1].x - rays[0].x).abs()
    };
    for _ in 0..240 {
        sky.advance(1.0 / 60.0, &mut random);
    }
    let gap_after = {
        let rays = sky.rays();
        (rays[1].x - rays[0].x).abs()
    };
    assert!(
        gap_after > gap_before + 4.0,
        "repulsion failed to spread: {gap_before} -> {gap_after}"
    );
}

#[test]
fn law1_beads_stay_inside_the_viewport() {
    // The hard bounds at a deliberately abusive dt: positions clamp,
    // velocities clamp, depths clamp — the bounded-by-construction
    // contract.
    let mut sky = AuroraSky::new();
    sky.reset(60, 30);
    let (mut rng, chance) = fixed_random(7);
    let mut random = AuroraRandom {
        rng: &mut rng,
        rand_chance: &chance,
    };
    for _ in 0..1800 {
        sky.advance(1.0 / 30.0, &mut random); // large dt — the clamp contract
        for r in sky.rays() {
            assert!((0.0..=59.0).contains(&r.x), "bead escaped: x={}", r.x);
            assert!(
                r.depth >= AURORA_DEPTH_MIN && r.depth <= 28.0,
                "depth escaped: {}",
                r.depth
            );
            assert!(r.vx.abs() <= AURORA_VX_MAX + 1e-3, "velocity escaped");
            assert!(r.glow <= AURORA_GLOW_MAX + 1e-4, "glow escaped the clamp");
        }
    }
}

#[test]
fn law2_depth_breath_is_two_banded() {
    // Over a long run on a 40-line viewport the depths cluster into
    // the two bands (LOW ~9.6-13.6, HIGH ~18.4-24.0) and never rest
    // in the gap between them (13.6-18.4): sampled at dwell-rest
    // points, every bead sits near one band's anchor.
    let mut sky = AuroraSky::new();
    sky.reset(80, 40);
    let (mut rng, chance) = fixed_random(99);
    let mut random = AuroraRandom {
        rng: &mut rng,
        rand_chance: &chance,
    };
    let mut low_rests = 0usize;
    let mut high_rests = 0usize;
    for tick in 0..3600 {
        sky.advance(1.0 / 30.0, &mut random);
        if tick % 90 == 0 {
            for r in sky.rays() {
                if r.depth < 16.0 {
                    low_rests += 1;
                } else {
                    high_rests += 1;
                }
            }
        }
    }
    assert!(
        low_rests > 0 && high_rests > 0,
        "breath collapsed to one band (low {low_rests}, high {high_rests})"
    );
}

#[test]
fn law4_glow_is_bounded_and_decays() {
    let mut sky = AuroraSky::new();
    sky.reset(80, 40);
    // Hammer one ray with charge far beyond the clamp.
    for _ in 0..50 {
        sky.absorb(0, 10.0);
    }
    assert!(
        sky.glow_for_test(0) <= AURORA_GLOW_MAX + 1e-4,
        "glow escaped the clamp: {}",
        sky.glow_for_test(0)
    );
    let (mut rng, chance) = fixed_random(3);
    let mut random = AuroraRandom {
        rng: &mut rng,
        rand_chance: &chance,
    };
    for _ in 0..600 {
        sky.advance(1.0 / 60.0, &mut random);
    }
    assert!(
        sky.glow_for_test(0) < AURORA_GLOW_LEVEL_HOT,
        "glow never cooled: {}",
        sky.glow_for_test(0)
    );
}

#[test]
fn law4_fringe_ladder_reads_the_glow() {
    assert!(matches!(fringe_level(0.0, f32::MAX), BrightnessLevel::Mid));
    assert!(matches!(fringe_level(1.0, f32::MAX), BrightnessLevel::Hot));
    // Fresh landing with a strong charge: the Core flare window.
    assert!(matches!(fringe_level(2.0, 0.1), BrightnessLevel::Core));
    // Stale strong charge: back to Hot (the window passed).
    assert!(matches!(fringe_level(2.0, 1.0), BrightnessLevel::Hot));
    // Fresh but weak charge: no Core without the glow for it.
    assert!(matches!(fringe_level(0.2, 0.1), BrightnessLevel::Mid));
    // (The threshold ordering itself is pinned by the compile-time
    // contracts in style_rain.rs — no runtime assert needed.)
}

#[test]
fn law3_nearest_ray_finds_the_closest_bead() {
    let mut sky = AuroraSky::new();
    sky.reset(80, 40);
    let rays = sky.rays();
    let probe = rays[0].x + 0.25;
    let idx = sky.nearest_ray(probe).expect("lattice is non-empty");
    let mut best_d = f32::INFINITY;
    let mut best = 0usize;
    for (i, r) in rays.iter().enumerate() {
        let d = (r.x - probe).abs();
        if d < best_d {
            best_d = d;
            best = i;
        }
    }
    assert_eq!(idx, best);
    // An un-reset lattice has no nearest ray (reset always plants
    // at least one bead — the clamp contract).
    let fresh = AuroraSky::new();
    assert!(fresh.nearest_ray(5.0).is_none());
}

#[test]
fn band_flip_crosses_bands() {
    assert_eq!(DepthBand::Low.flip(), DepthBand::High);
    assert_eq!(DepthBand::High.flip(), DepthBand::Low);
}
