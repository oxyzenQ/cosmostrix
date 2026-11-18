// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! # Phosphor decay calibration lock (night-frontier-Z4c, Lapis 4c)
//!
//! The trail afterglow's decay curve is CALIBRATED: the shipped rate
//! derives from the real P1-class phosphor decay constant under one
//! documented aesthetic time dilation, the per-frame LUT is the
//! calibration module's product, and every half-life below is pinned.
//! These tests are the moat: a future retune that changes the rate,
//! the dilation, or the curve shape fails here with the physical
//! consequence named in the message.
//!
//! Reference data (the standard P-designation table, as reproduced on
//! Wikipedia's Phosphor article and the classic CRT phosphor data
//! sheets): P1 = Zn2SiO4:Mn green willemite, medium 1-100 ms band,
//! characteristic tau 25 ms; P3 = amber, ~13 ms; P4 = white B/W TV,
//! short ~60 us; P7 = radar dual-layer, long (hundreds of ms to
//! seconds). Honest scope: the P1 identity is load-bearing; the other
//! classes are reference data whose cross-class ratios carry the
//! tables' level-of-decay ambiguity (10% vs 1% cutoffs) and are
//! deliberately not engine law.

use crate::constants::{
    P1_GREEN, P3_AMBER, P4_WHITE, P7_RADAR, PHOSPHOR_BOTTOM_DECAY_MULT, PHOSPHOR_CALIBRATED_CLASS,
    PHOSPHOR_DEAD_THRESHOLD, PHOSPHOR_DECAY_RATE, PHOSPHOR_LAYER_DECAY_MULT,
    PHOSPHOR_SIM_TIME_SCALE, PHOSPHOR_TAIL_RESIDUAL, PHOSPHOR_TAU_P1_SECS, phosphor_decay_lut,
    phosphor_half_life_secs,
};

/// The calibration identity: the derived rate is bit-identical to the
/// shipped 8.0 the owner tuned (1 / (0.025 * 5.0) = 8.0 exactly in
/// f32 — 0.025 * 5.0 rounds to the exactly-representable 0.125, and
/// 1 / 0.125 is exact). If this ever drifts, the trail visuals changed
/// — a retune, not a refactor — and the commit must say so.
#[test]
fn z4c_calibration_rate_is_bit_identical_to_shipped() {
    let derived = 1.0 / (PHOSPHOR_TAU_P1_SECS * PHOSPHOR_SIM_TIME_SCALE);
    assert_eq!(
        derived, PHOSPHOR_DECAY_RATE,
        "the derived calibration rate and the exported constant disagree — the constant \
         is no longer the calibration product"
    );
    assert_eq!(
        PHOSPHOR_DECAY_RATE, 8.0,
        "PHOSPHOR_DECAY_RATE drifted from the shipped 8.0 — this is a visual retune \
         (trail envelope changes), not a refactor. Update the dilation or the class tau \
         deliberately and document the owner decision."
    );
}

/// The half-life law: t_half = tau * ln(2) for every class in the
/// reference table, and the decay LUT lands EXACTLY on 0.5 at each
/// layer's half-life.
#[test]
fn z4c_half_life_is_tau_ln2_and_lut_lands_half() {
    // Class-level half-lives (real data pins).
    let p1_half = phosphor_half_life_secs(PHOSPHOR_TAU_P1_SECS);
    assert_eq!(p1_half, 0.025_f32 * std::f32::consts::LN_2);
    let p7_half = phosphor_half_life_secs(P7_RADAR.tau_secs);
    assert_eq!(p7_half, 0.2_f32 * std::f32::consts::LN_2);
    // The calibrated sim class: its simulated tau is the real tau times
    // the dilation; its simulated half-life is the one the renderer runs.
    let sim_tau = PHOSPHOR_TAU_P1_SECS * PHOSPHOR_SIM_TIME_SCALE;
    assert!(
        (sim_tau - 0.125).abs() < 1e-6,
        "sim tau drifted from 125 ms"
    );
    assert!(
        (phosphor_half_life_secs(sim_tau) - 0.125 * std::f32::consts::LN_2).abs() < 1e-6,
        "simulated half-life drifted from 86.64 ms"
    );

    // LUT at the half-life: for every (layer, bottom-band) variant, a
    // cell decaying for exactly one half-life must carry exactly half
    // its energy. The builder feeds one elapsed to all slots, so each
    // variant is checked with its own half-life and its own slot.
    for (i, &lm) in PHOSPHOR_LAYER_DECAY_MULT.iter().enumerate() {
        // Base (non-bottom) slot: elapsed = ln2 / (rate * lm).
        let t_half = std::f32::consts::LN_2 / (PHOSPHOR_DECAY_RATE * lm);
        let lut = phosphor_decay_lut(PHOSPHOR_DECAY_RATE * t_half, 0.0);
        assert!(
            (lut[i * 2] - 0.5).abs() < 1e-6,
            "layer {i} (mult {lm}): base factor at half-life {} != 0.5",
            lut[i * 2]
        );
        // Bottom slot: elapsed = ln2 / (rate * lm * bottom_mult).
        let t_half_bottom =
            std::f32::consts::LN_2 / (PHOSPHOR_DECAY_RATE * lm * PHOSPHOR_BOTTOM_DECAY_MULT);
        let lut_b = phosphor_decay_lut(
            0.0,
            PHOSPHOR_DECAY_RATE * PHOSPHOR_BOTTOM_DECAY_MULT * t_half_bottom,
        );
        assert!(
            (lut_b[i * 2 + 1] - 0.5).abs() < 1e-6,
            "layer {i} (mult {lm}): bottom factor at half-life {} != 0.5",
            lut_b[i * 2 + 1]
        );
    }
}

/// The tau point law: one time constant of decay lands on 1/e, and the
/// per-layer effective time constants are the P1 sim tau divided by
/// the layer multiplier (the parallax structure documented on
/// PHOSPHOR_LAYER_DECAY_MULT).
#[test]
fn z4c_tau_point_lands_on_one_over_e_and_layer_taus_match() {
    for (i, &lm) in PHOSPHOR_LAYER_DECAY_MULT.iter().enumerate() {
        let rate = PHOSPHOR_DECAY_RATE * lm;
        let tau_layer = 1.0 / rate;
        let expected = (std::f32::consts::E).recip();
        let got = (-rate * tau_layer).exp();
        assert!(
            (got - expected).abs() < 1e-6,
            "layer {i} (mult {lm}): factor at tau {got} != 1/e"
        );
        // The documented parallax taus: 0.125 s sim tau / mult.
        let sim_tau = PHOSPHOR_TAU_P1_SECS * PHOSPHOR_SIM_TIME_SCALE;
        assert!(
            (tau_layer - sim_tau / lm).abs() < 1e-6,
            "layer {i} tau {tau_layer} does not match sim_tau/{lm}"
        );
    }
}

/// The LUT is the exponential law, pinned structurally: every slot
/// stays inside (0, 1), and doubling the elapsed time squares the
/// factor (factor(2t) == factor(t)^2) for every slot.
#[test]
fn z4c_lut_is_strictly_monotone_decaying() {
    let dt = 1.0 / 60.0;
    let one_step = phosphor_decay_lut(PHOSPHOR_DECAY_RATE * dt, PHOSPHOR_DECAY_RATE * dt);
    let two_steps = phosphor_decay_lut(
        PHOSPHOR_DECAY_RATE * 2.0 * dt,
        PHOSPHOR_DECAY_RATE * 2.0 * dt,
    );
    for i in 0..one_step.len() {
        assert!(
            one_step[i] > 0.0 && one_step[i] < 1.0,
            "slot {i} escaped the (0, 1) interval: {}",
            one_step[i]
        );
        let squared = one_step[i] * one_step[i];
        assert!(
            (two_steps[i] - squared).abs() < 1e-5,
            "slot {i}: factor(2t) {} != factor(t)^2 {squared} — the LUT is not the \
             exponential law",
            two_steps[i]
        );
    }
}

/// The reference class table carries the REAL published values, and
/// the ordering matches the standard table (P4 short < P3 medium <
/// P1 medium < P7 long). The calibrated class is P1.
#[test]
fn z4c_reference_classes_carry_real_published_values() {
    // Full-row pins — every field of every reference class is real
    // standard-table data, and a silent edit breaks the moat.
    assert_eq!(P1_GREEN.name, "P1 (green willemite)");
    assert_eq!(P1_GREEN.composition, "Zn2SiO4:Mn");
    assert_eq!(P1_GREEN.color, "green 525 nm");
    assert_eq!(
        P1_GREEN.persistence_class,
        "medium, 1-100 ms band (oscilloscopes, monochrome monitors)"
    );
    assert_eq!(P1_GREEN.tau_secs, 0.025, "P1 tau drifted from 25 ms");
    assert_eq!(P3_AMBER.composition, "ZnBeSiO4:Mn");
    assert_eq!(P3_AMBER.tau_secs, 0.013, "P3 tau drifted from 13 ms");
    assert_eq!(P4_WHITE.composition, "ZnS:Ag + (Zn,Cd)S:Ag");
    assert_eq!(P4_WHITE.tau_secs, 0.000_06, "P4 tau drifted from 60 us");
    assert_eq!(
        P7_RADAR.tau_secs, 0.2,
        "P7 long-layer tau drifted from 200 ms"
    );
    let order = [
        P4_WHITE.tau_secs,
        P3_AMBER.tau_secs,
        P1_GREEN.tau_secs,
        P7_RADAR.tau_secs,
    ];
    assert!(
        order.windows(2).all(|w| w[0] < w[1]),
        "reference class ordering drifted from the standard table \
         (P4 < P3 < P1 < P7): {order:?}"
    );
    assert!(
        std::ptr::eq(PHOSPHOR_CALIBRATED_CLASS, &P1_GREEN),
        "the calibrated class must stay P1 — the green monochrome monitor \
         identity the matrix-rain aesthetic descends from"
    );
}

/// The shipped trail envelope, pinned end to end: a cell entering at
/// PHOSPHOR_TAIL_RESIDUAL dies (<= PHOSPHOR_DEAD_THRESHOLD) after the
/// documented frame counts per layer at 60 fps — the SAME envelope the
/// pre-Z4c law produced (the calibration is bit-identical by test 1;
/// this test pins the physical consequence in visible numbers).
///
/// Derived: frames(n) = ceil(ln(residual/dead) / (rate * mult * dt)).
#[test]
fn z4c_shipped_envelope_pins_tail_to_death_frames() {
    let dt = 1.0 / 60.0;
    let ratio = (PHOSPHOR_TAIL_RESIDUAL as f32 / PHOSPHOR_DEAD_THRESHOLD as f32).ln();
    let expected_frames =
        |mult: f32, bottom: f32| (ratio / (PHOSPHOR_DECAY_RATE * mult * bottom * dt)).ceil() as u32;
    // Back layer (mult 1.9): 13 frames (~217 ms) — snappy shadow exit.
    assert_eq!(expected_frames(1.9, 1.0), 13, "back-layer envelope drifted");
    // Mid layer (mult 1.15): 22 frames (~367 ms) — the smooth dissolve.
    assert_eq!(expected_frames(1.15, 1.0), 22, "mid-layer envelope drifted");
    // Front layer (mult 0.65): 38 frames (~632 ms) — the lingering trail.
    assert_eq!(
        expected_frames(0.65, 1.0),
        38,
        "front-layer envelope drifted"
    );
    // Bottom band accelerates: back-bottom ~8 frames.
    assert_eq!(
        expected_frames(1.9, PHOSPHOR_BOTTOM_DECAY_MULT),
        8,
        "bottom-band acceleration drifted"
    );
}
