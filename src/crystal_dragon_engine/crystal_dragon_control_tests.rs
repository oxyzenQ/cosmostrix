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
    assert!((cfg.drift_chance - 0.12).abs() < f32::EPSILON);
    assert!((cfg.cpu_ema_alpha - 0.25).abs() < f32::EPSILON);
    assert_eq!(cfg.sensor_mode, CrystalDragonSensorMode::Cpu);
    assert_eq!(cfg.calc_method, CrystalDragonCalcMethod::Calc);
}

#[test]
fn polling_duration_matches_secs() {
    let cfg = CrystalDragonControl::default();
    assert_eq!(cfg.polling_duration(), Duration::from_secs(60));
}

#[test]
fn min_dwell_duration_matches_secs() {
    let cfg = CrystalDragonControl::default();
    assert_eq!(cfg.min_dwell_duration(), Duration::from_secs(60));
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
