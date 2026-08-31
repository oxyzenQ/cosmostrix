// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! sensor tests, extracted from inline `mod tests { ... }` block in
//! sensor.rs (Pattern D → Pattern C unification).
//!
//! Uses `use super::*;` to access sensor.rs's private items unchanged.

use super::*;
#[test]
fn point_to_group_boundaries() {
    assert_eq!(point_to_group(1), TemperatureGroup::Cold);
    assert_eq!(point_to_group(33), TemperatureGroup::Cold);
    assert_eq!(point_to_group(34), TemperatureGroup::Medium);
    assert_eq!(point_to_group(66), TemperatureGroup::Medium);
    assert_eq!(point_to_group(67), TemperatureGroup::Hot);
    assert_eq!(point_to_group(99), TemperatureGroup::Hot);
}

#[test]
fn group_point_range_covers_full_1_to_99() {
    let (c_lo, c_hi) = group_point_range(TemperatureGroup::Cold);
    let (m_lo, m_hi) = group_point_range(TemperatureGroup::Medium);
    let (h_lo, h_hi) = group_point_range(TemperatureGroup::Hot);
    assert_eq!(c_lo, 1);
    assert_eq!(c_hi + 1, m_lo);
    assert_eq!(m_hi + 1, h_lo);
    assert_eq!(h_hi, 99);
}

#[test]
fn sensor_new_probes_cpu_support_honestly() {
    let now = Instant::now();
    let control = CrystalDragonControl::default();
    let sensor = CrystalDragonSensor::new(now, control);
    let expected = cpustat::current_cpu_ns().is_some();
    assert_eq!(sensor.cpu_supported(), expected);
}

#[test]
fn sensor_new_falls_back_to_clock_when_cpu_unsupported() {
    // On platforms where CPU is supported, effective_mode stays Cpu.
    // On unsupported platforms, it becomes Clock. We can only verify
    // the public side (cpu_supported): if CPU is unsupported, the
    // sensor must report that fact honestly.
    let now = Instant::now();
    let control = CrystalDragonControl::default();
    let sensor = CrystalDragonSensor::new(now, control);
    let cpu_ok = cpustat::current_cpu_ns().is_some();
    assert_eq!(sensor.cpu_supported(), cpu_ok);
}

#[test]
fn sensor_default_point_is_cold_start() {
    let now = Instant::now();
    let control = CrystalDragonControl::default();
    let sensor = CrystalDragonSensor::new(now, control);
    assert_eq!(sensor.current_point(), 17);
    assert_eq!(sensor.current_group(), TemperatureGroup::Cold);
}

#[test]
fn sensor_shift_in_time_no_panic() {
    let now = Instant::now();
    let control = CrystalDragonControl::default();
    let mut sensor = CrystalDragonSensor::new(now, control);
    sensor.shift_in_time(std::time::Duration::from_secs(3600));
    // No panic = pass
}

#[test]
fn poll_clock_returns_valid_range() {
    let now = Instant::now();
    let control = CrystalDragonControl::default();
    let sensor = CrystalDragonSensor::new(now, control);
    let point = sensor.poll_clock();
    assert!(
        (1..=99).contains(&point),
        "clock point {point} outside 1..=99"
    );
}

#[test]
fn record_theme_transition_updates_timestamp() {
    let now = Instant::now();
    let control = CrystalDragonControl::default();
    let mut sensor = CrystalDragonSensor::new(now, control);
    let later = now + std::time::Duration::from_secs(120);
    sensor.record_theme_transition(later);
    assert_eq!(sensor.theme_entered_at(), later);
}
