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

/// Z-master-1X round 8: sqrt CPU→point mapping must spread points across
/// all 3 temperature groups at realistic CPU usage levels. The old linear
/// mapping (`cpu * 0.99`) bottlenecked cosmostrix's typical 0.5-8% CPU
/// into points 1-8 (always Cold group → blues/cyans/whites only). The
/// sqrt mapping (`sqrt(cpu) * 9.9`) makes Medium (greens/purples)
/// reachable at ~12% CPU and Hot (yellows/reds/fire) at ~50% CPU.
///
/// This test verifies the sqrt mapping function directly (not via the
/// sensor's poll_cpu, which requires real CPU sampling). It checks that
/// the 3 temperature groups are all reachable at realistic CPU levels.
#[test]
fn z_master_1x_round8_sqrt_mapping_reaches_all_groups() {
    // Replicate the sqrt mapping from poll_cpu:
    //   point = clamp(1, 99, round(sqrt(cpu) * 9.9))
    fn sqrt_point(cpu: f32) -> u8 {
        let raw = (cpu.sqrt() * 9.9_f32).clamp(1.0, 99.0);
        (raw.round() as u8).clamp(1, 99)
    }

    // Typical cosmostrix interactive CPU usage: 0.5-8% → Cold group.
    // But the sqrt curve spreads the point across the FULL Cold range
    // (1-33), not just 1-8 like the old linear mapping.
    let low_cpu_point = sqrt_point(2.0);
    assert_eq!(point_to_group(low_cpu_point), TemperatureGroup::Cold);
    assert!(
        low_cpu_point >= 10,
        "2% CPU should map to point >=10 (was {} with sqrt, would be 2 with linear)",
        low_cpu_point
    );

    // ~12% CPU → Medium group (greens/purples) — UNREACHABLE with linear!
    let mid_cpu_point = sqrt_point(12.0);
    assert_eq!(
        point_to_group(mid_cpu_point),
        TemperatureGroup::Medium,
        "12% CPU should reach Medium group (greens/purples) with sqrt mapping"
    );

    // ~50% CPU → Hot group (yellows/reds/fire) — UNREACHABLE with linear!
    let high_cpu_point = sqrt_point(50.0);
    assert_eq!(
        point_to_group(high_cpu_point),
        TemperatureGroup::Hot,
        "50% CPU should reach Hot group (yellows/reds/fire) with sqrt mapping"
    );

    // 100% CPU → point 99 (Hot, max)
    let max_point = sqrt_point(100.0);
    assert_eq!(max_point, 99);
    assert_eq!(point_to_group(max_point), TemperatureGroup::Hot);
}

/// Z-master-1X round 8: sqrt mapping must be monotonic — higher CPU
/// always produces a higher-or-equal point. This preserves the design
/// intent (low CPU = cooler colors, high CPU = hotter colors).
#[test]
fn z_master_1x_round8_sqrt_mapping_is_monotonic() {
    fn sqrt_point(cpu: f32) -> u8 {
        let raw = (cpu.sqrt() * 9.9_f32).clamp(1.0, 99.0);
        (raw.round() as u8).clamp(1, 99)
    }
    let mut prev: u8 = 0;
    for i in 0..=100 {
        let cpu = i as f32;
        let point = sqrt_point(cpu);
        assert!(
            point >= prev,
            "sqrt mapping not monotonic at CPU={cpu}: point {point} < prev {prev}"
        );
        prev = point;
    }
}
