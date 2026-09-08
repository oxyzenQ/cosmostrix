// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The invented-math contracts of the corona arcade
//! (NIGHT-special-4, laws 1, 3, 4, 5): the loop-count scaling, the
//! footpoint repulsion spread, the wall + drift + span clamps, the
//! width breath bounds, the bounded flux charge, the flare cycle
//! transitions, and the granulation surface bounds.

use rand::{distr::Uniform, rngs::StdRng, SeedableRng};

use crate::cloud::solar_flare::loops::{
    granule_level, height_band, loop_count_for_cols, loop_level, width_band, CoronaArcade,
    LoopPhase, SolarRandom,
};
use crate::constants::{
    SOLAR_DRIFT_MAX, SOLAR_FLUX_MAX, SOLAR_GRANULE_MAX, SOLAR_GRANULE_MIN, SOLAR_W_MIN,
};

use crate::cloud::monolith::BrightnessLevel;

fn fixed_random(seed: u64) -> (StdRng, Uniform<f32>) {
    (StdRng::seed_from_u64(seed), Uniform::new(0.0, 1.0).unwrap())
}

#[test]
fn loop_count_scales_with_viewport() {
    assert_eq!(loop_count_for_cols(0), 1);
    assert_eq!(loop_count_for_cols(1), 1);
    assert_eq!(loop_count_for_cols(10), 1);
    assert_eq!(loop_count_for_cols(80), 8);
    assert_eq!(loop_count_for_cols(200), 8); // hard cap
    assert_eq!(loop_count_for_cols(400), 8); // hard cap
}

#[test]
fn law1_repulsion_spreads_the_arcade() {
    // Two loops planted with facing feet inside the gap floor must
    // separate (the inverse-gap push dominates at the floor gap —
    // the aurora lesson, pinned from birth: the sign was inverted
    // there; here it is contracted from the start).
    let mut arcade = CoronaArcade::new();
    arcade.reset(80, 40);
    arcade.clear_loops_for_test();
    let (mut rng, chance) = fixed_random(42);
    let mut random = SolarRandom {
        rng: &mut rng,
        rand_chance: &chance,
    };
    arcade.plant_loop_for_test(40.0, 12.0, 8.0);
    arcade.plant_loop_for_test(44.0, 12.0, 8.0);
    let gap_before = {
        let loops = arcade.loops();
        (loops[1].cx - loops[0].cx).abs()
    };
    for _ in 0..240 {
        arcade.advance(1.0 / 60.0, &mut random);
    }
    let gap_after = {
        let loops = arcade.loops();
        (loops[1].cx - loops[0].cx).abs()
    };
    // The separation is slow and majestic (the wind coupling drags
    // both loops toward the same global target, eating part of the
    // push) — the contract is the SIGN of the force (spread, never
    // collapse), not its magnitude.
    assert!(
        gap_after > gap_before + 2.0,
        "repulsion failed to spread: {gap_before} -> {gap_after}"
    );
}

#[test]
fn law1_loops_stay_inside_the_viewport() {
    // The hard bounds at a deliberately abusive dt: centers clamp
    // inside the footpoint margins, velocities clamp, spans clamp —
    // the bounded-by-construction contract.
    let mut arcade = CoronaArcade::new();
    arcade.reset(60, 30);
    let (mut rng, chance) = fixed_random(7);
    let mut random = SolarRandom {
        rng: &mut rng,
        rand_chance: &chance,
    };
    let (_, w_hi) = width_band(60);
    let (_, h_cap) = height_band(30);
    for _ in 0..1800 {
        arcade.advance(1.0 / 30.0, &mut random); // large dt — the clamp contract
        for lp in arcade.loops() {
            assert!(
                (lp.w * 0.5..=(59.0 - lp.w * 0.5)).contains(&lp.cx) || lp.w >= 57.0,
                "loop center escaped the footpoint margin: cx={} w={}",
                lp.cx,
                lp.w
            );
            assert!(
                lp.w >= SOLAR_W_MIN - 1e-3 && lp.w <= w_hi + 1e-3,
                "span escaped: {}",
                lp.w
            );
            assert!(lp.h <= h_cap + 1e-3, "height escaped: {}", lp.h);
            assert!(
                lp.vx.abs() <= SOLAR_DRIFT_MAX + 1e-3,
                "velocity escaped: {}",
                lp.vx
            );
            assert!(lp.flux <= SOLAR_FLUX_MAX + 1e-4, "flux escaped the clamp");
        }
    }
}

#[test]
fn law1_width_breath_is_bounded() {
    // Over a long run the footpoint span breathes inside its band
    // and the apex height never leaves its cap (the anchor re-roll
    // + glide contract).
    let mut arcade = CoronaArcade::new();
    arcade.reset(80, 40);
    let (mut rng, chance) = fixed_random(99);
    let mut random = SolarRandom {
        rng: &mut rng,
        rand_chance: &chance,
    };
    let (_, w_hi) = width_band(80);
    let (_, h_cap) = height_band(40);
    for _ in 0..3000 {
        arcade.advance(1.0 / 60.0, &mut random);
        for lp in arcade.loops() {
            assert!(
                (SOLAR_W_MIN - 1e-3..=w_hi + 1e-3).contains(&lp.w),
                "width breath escaped: {}",
                lp.w
            );
            assert!(
                (0.0..=h_cap + 1e-3).contains(&lp.h),
                "height breath escaped: {}",
                lp.h
            );
        }
    }
}

#[test]
fn law3_flux_charge_is_bounded() {
    // Repeated heavy depositions clamp at FLUX_MAX and arm the
    // flash window (bounded by construction, law 3b).
    let mut arcade = CoronaArcade::new();
    arcade.reset(80, 40);
    arcade.clear_loops_for_test();
    arcade.plant_loop_for_test(40.0, 12.0, 8.0);
    let (mut rng, chance) = fixed_random(11);
    let mut random = SolarRandom {
        rng: &mut rng,
        rand_chance: &chance,
    };
    for i in 0..40 {
        arcade.absorb(0, 10.0);
        assert!(
            arcade.flux_for_test(0) <= SOLAR_FLUX_MAX + 1e-4,
            "flux escaped the clamp on deposition {i}: {}",
            arcade.flux_for_test(0)
        );
    }
    // The decay drains it: after enough quiet ticks the flux falls
    // away (law 3a — the corona cools).
    for _ in 0..600 {
        arcade.advance(1.0 / 60.0, &mut random);
    }
    assert!(
        arcade.flux_for_test(0) < SOLAR_FLUX_MAX,
        "the corona never cooled"
    );
}

#[test]
fn law4_the_flare_gate_promotes_the_flux_richest_loop() {
    // Deterministic arm: plant two charged loops (the richer one
    // second), arm the clock, tick — the flux-richest Stable loop
    // destabilizes, and only one loop erupts at a time.
    let mut arcade = CoronaArcade::new();
    arcade.reset(80, 40);
    arcade.clear_loops_for_test();
    let (mut rng, chance) = fixed_random(21);
    let mut random = SolarRandom {
        rng: &mut rng,
        rand_chance: &chance,
    };
    arcade.plant_loop_for_test(20.0, 12.0, 8.0);
    arcade.plant_loop_for_test(50.0, 12.0, 8.0);
    arcade.plant_flux_for_test(0, 1.3);
    arcade.plant_flux_for_test(1, 2.0);
    arcade.arm_flare_clock_for_test();
    let erupted = arcade.advance(1.0 / 60.0, &mut random);
    assert_eq!(
        erupted,
        Some(1),
        "the gate did not pick the flux-richest loop"
    );
    assert_eq!(
        arcade.loop_phase_for_test(1),
        Some(LoopPhase::Erupting),
        "the promoted loop did not enter Erupting"
    );
    assert_ne!(
        arcade.loop_phase_for_test(0),
        Some(LoopPhase::Erupting),
        "two loops erupted at once — the singular-event contract"
    );
}

#[test]
fn law4_the_cycle_rebirths_the_loop() {
    // The phase machine runs the full flare cycle and re-seeds as
    // an Emerging arc with a reset flux (the carpet's turnover).
    let mut arcade = CoronaArcade::new();
    arcade.reset(80, 40);
    arcade.clear_loops_for_test();
    let (mut rng, chance) = fixed_random(33);
    let mut random = SolarRandom {
        rng: &mut rng,
        rand_chance: &chance,
    };
    arcade.plant_loop_for_test(40.0, 12.0, 8.0);
    arcade.plant_flux_for_test(0, 2.0);
    arcade.arm_flare_clock_for_test();
    assert!(arcade.advance(1.0 / 60.0, &mut random).is_some());

    // Erupting (1.4 s) -> Detaching (1.8 s) -> Emerging (1.2 s):
    // the rebirth window opens at 3.2 s and closes at 4.4 s.
    let mut secs = 0.0;
    while secs < 1.3 {
        arcade.advance(1.0 / 60.0, &mut random);
        secs += 1.0 / 60.0;
    }
    assert_eq!(arcade.loop_phase_for_test(0), Some(LoopPhase::Erupting));
    while secs < 3.0 {
        arcade.advance(1.0 / 60.0, &mut random);
        secs += 1.0 / 60.0;
    }
    assert_eq!(arcade.loop_phase_for_test(0), Some(LoopPhase::Detaching));
    assert!(arcade.flux_for_test(0) <= 1e-6);
    while secs < 3.6 {
        arcade.advance(1.0 / 60.0, &mut random);
        secs += 1.0 / 60.0;
    }
    assert_eq!(arcade.loop_phase_for_test(0), Some(LoopPhase::Emerging));
}

#[test]
fn law5_granulation_stays_bounded() {
    // The surface random walk clamps inside its heat band across a
    // long abusive-dt run.
    let mut arcade = CoronaArcade::new();
    arcade.reset(80, 40);
    let (mut rng, chance) = fixed_random(55);
    let mut random = SolarRandom {
        rng: &mut rng,
        rand_chance: &chance,
    };
    for _ in 0..1200 {
        arcade.advance(1.0 / 30.0, &mut random);
        for col in 0..80 {
            let heat = arcade.granule_heat(col);
            assert!(
                (SOLAR_GRANULE_MIN - 1e-4..=SOLAR_GRANULE_MAX + 1e-4).contains(&heat),
                "granule heat escaped the band: {heat}"
            );
        }
    }
}

#[test]
fn law3_the_ladder_reads_the_flux() {
    // The corona's brightness ladder: quiet Ghost, rained-on Mid,
    // fed Hot, the fresh-flash Core window.
    assert!(matches!(loop_level(0.0, f32::MAX), BrightnessLevel::Ghost));
    assert!(matches!(loop_level(0.30, f32::MAX), BrightnessLevel::Mid));
    assert!(matches!(loop_level(0.8, f32::MAX), BrightnessLevel::Hot));
    // Core requires BOTH the strong flux and the fresh flash.
    assert!(matches!(loop_level(2.0, 0.0), BrightnessLevel::Core));
    assert!(matches!(loop_level(2.0, f32::MAX), BrightnessLevel::Hot));
    // The granulation ladder: quiet grit, warm cells, hot cells.
    assert!(matches!(granule_level(0.2), BrightnessLevel::Ghost));
    assert!(matches!(granule_level(0.6), BrightnessLevel::Mid));
    assert!(matches!(granule_level(0.9), BrightnessLevel::Hot));
}
