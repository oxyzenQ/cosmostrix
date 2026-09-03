// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! crystal_dragon_control tests, extracted from inline `mod tests { ... }` block in
//! crystal_dragon_control.rs (Pattern D → Pattern C unification).
//!
//! Uses `use super::*;` to access crystal_dragon_control.rs's private items unchanged.

use super::*;
#[test]
fn default_uses_owner_chosen_values() {
    let cfg = CrystalDragonControl::default();
    assert_eq!(cfg.polling_secs, 60.0);
    assert_eq!(cfg.min_dwell_secs, 60.0);
    // S-master-HUNT-7: 1.0 = deterministic boundary fire (the documented
    // cadence contract). A regression back to a fractional value would
    // starve the drift cadence by 1/value cadences per drift (the owner's
    // "crdr: on but nothing drifts" bug).
    assert_eq!(cfg.drift_chance, 1.0);
    assert!((cfg.cpu_ema_alpha - 0.25).abs() < f32::EPSILON);
    assert_eq!(cfg.sensor_mode, CrystalDragonSensorMode::Cpu);
    assert_eq!(cfg.calc_method, CrystalDragonCalcMethod::CalcV2);
}

#[test]
fn sensor_mode_labels_are_stable() {
    // Ensure the enum variants exist and are debug-printable.
    assert_eq!(format!("{:?}", CrystalDragonSensorMode::Cpu), "Cpu");
    assert_eq!(format!("{:?}", CrystalDragonSensorMode::Clock), "Clock");
}

#[test]
fn calc_method_labels_are_stable() {
    assert_eq!(format!("{:?}", CrystalDragonCalcMethod::Calc), "Calc");
    assert_eq!(format!("{:?}", CrystalDragonCalcMethod::CalcV2), "CalcV2");
}
