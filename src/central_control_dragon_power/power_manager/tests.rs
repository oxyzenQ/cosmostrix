// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! power_manager tests, extracted from inline `mod tests { ... }` block.
//!
//! Uses `use super::*;` to access parent's private items unchanged.

use super::*;
use std::hint::black_box;

/// Helper: construct a PowerManager with a known `last_input_time`
/// offset so idle-threshold tests are deterministic.
fn make_pm_at(now: Instant) -> PowerManager {
    PowerManager::new(60.0, now)
}

// ── Construction ───────────────────────────────────────────────────────

#[test]
fn power_manager_starts_active_with_zero_pressure() {
    let now = Instant::now();
    let pm = make_pm_at(now);
    assert!(!pm.is_idle());
    assert_eq!(pm.effective_pressure(), 0.0);
    assert_eq!(pm.base_target_fps(), 60.0);
    assert!(pm.idle_started().is_none());
    assert_eq!(pm.phase_predictor().transitions_observed(), 0);
}

#[test]
fn power_manager_clamps_zero_base_fps_to_one() {
    // A misbehaving upstream (config reload with fps=0) must not
    // panic on 1.0/fps inside effective_fps().
    let now = Instant::now();
    let pm = PowerManager::new(0.0, now);
    assert_eq!(pm.base_target_fps(), 1.0);
    let fps = black_box(pm).effective_fps(false);
    assert!(fps.is_finite() && fps > 0.0);
}

#[test]
fn power_manager_clamps_negative_base_fps_to_one() {
    let now = Instant::now();
    let pm = PowerManager::new(-5.0, now);
    assert_eq!(pm.base_target_fps(), 1.0);
}

// ── effective_pressure ────────────────────────────────────────────────

#[test]
fn effective_pressure_includes_thermal_input() {
    let now = Instant::now();
    let mut pm = make_pm_at(now);
    // Base pressure from overshoot frames.
    pm.observe_frame_end(0.020, 0.016, 0.0); // 0.020s work / 0.016s period → overshoot
    let base = pm.effective_pressure();
    assert!(base > 0.0, "base pressure should be > 0 after overshoot");

    // Thermal pressure adds to base.
    pm.set_thermal_pressure(0.3);
    let with_thermal = pm.effective_pressure();
    assert!((with_thermal - (base + 0.3)).abs() < 1e-6);
}

#[test]
fn effective_pressure_clamps_to_one_with_thermal() {
    let now = Instant::now();
    let mut pm = make_pm_at(now);
    // Drive base pressure near 1.0 with repeated overshoots.
    for _ in 0..20 {
        pm.observe_frame_end(0.040, 0.016, 0.0); // 2.5× overshoot
    }
    assert!((pm.effective_pressure() - 1.0).abs() < 1e-6);

    // Thermal pressure would push above 1.0 — must clamp.
    pm.set_thermal_pressure(0.5);
    assert_eq!(pm.effective_pressure(), 1.0);
}

#[test]
fn set_thermal_pressure_clamps_input() {
    let now = Instant::now();
    let mut pm = make_pm_at(now);
    pm.set_thermal_pressure(-0.5);
    assert_eq!(pm.effective_pressure(), 0.0);
    pm.set_thermal_pressure(2.0);
    // 1.0 thermal alone → effective_pressure = 1.0 (base is 0.0).
    assert!((pm.effective_pressure() - 1.0).abs() < 1e-6);
}

#[test]
fn effective_pressure_decays_on_normal_frame() {
    let now = Instant::now();
    let mut pm = make_pm_at(now);
    // Build up pressure.
    pm.observe_frame_end(0.030, 0.016, 0.0);
    let after_overshoot = pm.effective_pressure();
    assert!(after_overshoot > 0.0);

    // Normal frame (work < period) → pressure decays.
    pm.observe_frame_end(0.005, 0.016, 0.0);
    let after_decay = pm.effective_pressure();
    assert!(after_decay < after_overshoot);
}

#[test]
fn effective_pressure_never_goes_negative() {
    let now = Instant::now();
    let mut pm = make_pm_at(now);
    // Many normal frames with no prior overshoot — should stay at 0.
    for _ in 0..100 {
        pm.observe_frame_end(0.005, 0.016, 0.0);
    }
    assert_eq!(pm.effective_pressure(), 0.0);
}

// ── effective_fps ─────────────────────────────────────────────────────

#[test]
fn effective_fps_active_returns_base() {
    let now = Instant::now();
    let pm = PowerManager::new(144.0, now);
    // No idle (just constructed, was_active=true).
    assert!((pm.effective_fps(false) - 144.0).abs() < 1e-6);
}

#[test]
fn effective_fps_paused_returns_4_fps() {
    let now = Instant::now();
    let pm = PowerManager::new(144.0, now);
    let paused_fps = black_box(pm).effective_fps(true);
    let expected = 1000.0 / PAUSE_PERIOD_MS as f64;
    assert!((paused_fps - expected).abs() < 1e-6);
    // PAUSE_PERIOD_MS = 250 → 4 FPS.
    assert!((paused_fps - 4.0).abs() < 1e-6);
}

#[test]
fn effective_fps_idle_applies_idle_factor() {
    let now = Instant::now();
    let mut pm = PowerManager::new(60.0, now);
    // Force idle by advancing time past IDLE_THRESHOLD_SECS.
    let later = now + std::time::Duration::from_secs_f64(IDLE_THRESHOLD_SECS + 1.0);
    let is_idle = pm.begin_frame(later);
    assert!(is_idle, "should be idle after threshold+1s with no input");

    let idle_fps = pm.effective_fps(false);
    let expected = 60.0 * IDLE_FPS_FACTOR;
    assert!((idle_fps - expected).abs() < 1e-6);
}

#[test]
fn effective_fps_paused_overrides_idle() {
    let now = Instant::now();
    let mut pm = PowerManager::new(60.0, now);
    let later = now + std::time::Duration::from_secs_f64(IDLE_THRESHOLD_SECS + 1.0);
    pm.begin_frame(later);
    assert!(pm.is_idle());

    // Paused must win over idle.
    let fps = pm.effective_fps(true);
    assert!((fps - 4.0).abs() < 1e-6);
}

#[test]
fn set_target_fps_updates_effective_fps() {
    let now = Instant::now();
    let mut pm = PowerManager::new(60.0, now);
    assert!((pm.effective_fps(false) - 60.0).abs() < 1e-6);

    pm.set_target_fps(120.0);
    assert!((pm.effective_fps(false) - 120.0).abs() < 1e-6);
}

// ── is_idle / begin_frame ─────────────────────────────────────────────

#[test]
fn begin_frame_reports_active_when_input_recent() {
    let now = Instant::now();
    let mut pm = make_pm_at(now);
    // 5 seconds later — well under IDLE_THRESHOLD_SECS (30s).
    let is_idle = pm.begin_frame(now + std::time::Duration::from_secs(5));
    assert!(!is_idle);
    assert!(!pm.is_idle());
    assert!(pm.idle_started().is_none());
}

#[test]
fn begin_frame_reports_idle_after_threshold() {
    let now = Instant::now();
    let mut pm = make_pm_at(now);
    let later = now + std::time::Duration::from_secs_f64(IDLE_THRESHOLD_SECS + 1.0);
    let is_idle = pm.begin_frame(later);
    assert!(is_idle);
    assert!(pm.is_idle());
    assert!(pm.idle_started().is_some());
}

#[test]
fn begin_frame_tracks_idle_started_first_transition_only() {
    let now = Instant::now();
    let mut pm = make_pm_at(now);

    // First idle transition sets idle_started.
    let t1 = now + std::time::Duration::from_secs_f64(IDLE_THRESHOLD_SECS + 1.0);
    pm.begin_frame(t1);
    let first_idle_started = pm.idle_started();
    assert!(first_idle_started.is_some());

    // Second idle frame does NOT overwrite idle_started.
    let t2 = now + std::time::Duration::from_secs_f64(IDLE_THRESHOLD_SECS + 5.0);
    pm.begin_frame(t2);
    assert_eq!(pm.idle_started(), first_idle_started);
}

#[test]
fn note_activity_clears_idle_state() {
    let now = Instant::now();
    let mut pm = make_pm_at(now);

    // Enter idle.
    let t1 = now + std::time::Duration::from_secs_f64(IDLE_THRESHOLD_SECS + 1.0);
    pm.begin_frame(t1);
    assert!(pm.is_idle());
    assert!(pm.idle_started().is_some());

    // Activity clears idle — was_active flips back to true and
    // idle_started is reset to None. We check is_idle() directly
    // rather than calling begin_frame again because begin_frame
    // recomputes from the phase predictor (which now has 2
    // transitions recorded at the same wall-clock instant, making
    // its prediction degenerate). The contract of note_activity is:
    // immediately after the call, is_idle() returns false.
    pm.note_activity(t1);
    assert!(!pm.is_idle());
    assert!(pm.idle_started().is_none());
}

#[test]
fn note_activity_records_phase_transition_idle_to_active() {
    let now = Instant::now();
    let mut pm = make_pm_at(now);

    // Enter idle.
    let t1 = now + std::time::Duration::from_secs_f64(IDLE_THRESHOLD_SECS + 1.0);
    pm.begin_frame(t1);
    assert!(pm.is_idle());
    let transitions_after_idle = pm.phase_predictor().transitions_observed();
    // begin_frame records the idle→active? No: idle transition is
    // active→idle, recorded by begin_frame. So transitions should
    // be 1 after the first idle.
    assert_eq!(transitions_after_idle, 1);

    // Activity transitions idle→active.
    pm.note_activity(t1);
    assert_eq!(pm.phase_predictor().transitions_observed(), 2);
}

// ── observe_frame_end ─────────────────────────────────────────────────

#[test]
fn observe_frame_end_accumulates_pressure_on_overshoot() {
    let now = Instant::now();
    let mut pm = make_pm_at(now);
    // 0.040s work / 0.016s period → ratio = 2.5, overshoot = 2.5-1.0 = 1.5
    // (clamped to [0,2] → 1.5). pressure = 1.5 * 0.25 = 0.375.
    pm.observe_frame_end(0.040, 0.016, 0.0);
    assert!((pm.effective_pressure() - 0.375).abs() < 1e-6);
}

#[test]
fn observe_frame_end_decays_pressure_on_normal_frame() {
    let now = Instant::now();
    let mut pm = make_pm_at(now);
    // Build up pressure first.
    pm.observe_frame_end(0.040, 0.016, 0.0);
    let after_overshoot = pm.effective_pressure();

    // Normal frame (work < period) → decay.
    pm.observe_frame_end(0.005, 0.016, 0.0);
    let after_decay = pm.effective_pressure();
    assert!((after_decay - (after_overshoot - PERF_PRESSURE_DECAY)).abs() < 1e-6);
}

#[test]
fn observe_frame_end_write_overshoot_adds_pressure() {
    let now = Instant::now();
    let mut pm = make_pm_at(now);

    // No work overshoot, write overshoot only.
    pm.observe_frame_end(0.005, 0.016, 1.0);
    // Expected: write_overshoot=1.0, pressure = 1.0 * 0.25 = 0.25.
    assert!((pm.effective_pressure() - 0.25).abs() < 1e-6);
}

#[test]
fn observe_frame_end_zero_period_does_not_panic() {
    let now = Instant::now();
    let mut pm = make_pm_at(now);
    // frame_period_s = 0.0 would divide by zero if not guarded.
    pm.observe_frame_end(0.040, 0.0, 0.0);
    // Should not panic; pressure stays at 0 (no overshoot computed).
    assert_eq!(pm.effective_pressure(), 0.0);
}

// ── PowerThresholds integration ───────────────────────────────────────

#[test]
fn power_manager_uses_thresholds_for_increment_and_decay() {
    // Custom thresholds: increment=0.5, decay=0.1.
    let thresholds = PowerThresholds {
        pressure_increment: 0.5,
        pressure_decay: 0.1,
        ..PowerThresholds::defaults()
    };
    let now = Instant::now();
    let mut pm = PowerManager::new(60.0, now).with_thresholds(thresholds);

    // ratio = 2.5, overshoot = 1.5, pressure = 1.5 * 0.5 = 0.75.
    pm.observe_frame_end(0.040, 0.016, 0.0);
    assert!((pm.effective_pressure() - 0.75).abs() < 1e-6);

    // Normal frame → decay by 0.1.
    pm.observe_frame_end(0.005, 0.016, 0.0);
    assert!((pm.effective_pressure() - 0.65).abs() < 1e-6);
}

#[test]
fn power_manager_uses_thresholds_for_idle_threshold() {
    // Custom idle threshold: 5 seconds.
    let thresholds = PowerThresholds {
        idle_threshold_secs: 5.0,
        ..PowerThresholds::defaults()
    };
    let now = Instant::now();
    let mut pm = PowerManager::new(60.0, now).with_thresholds(thresholds);

    // At 3 seconds — under custom 5s threshold.
    let is_idle = pm.begin_frame(now + std::time::Duration::from_secs(3));
    assert!(!is_idle);

    // At 6 seconds — over custom 5s threshold.
    let is_idle = pm.begin_frame(now + std::time::Duration::from_secs(6));
    assert!(is_idle);
}

#[test]
fn power_manager_uses_thresholds_for_idle_fps_factor() {
    // Custom idle factor: 0.25 (was 0.5).
    let thresholds = PowerThresholds {
        idle_fps_factor: 0.25,
        ..PowerThresholds::defaults()
    };
    let now = Instant::now();
    let mut pm = PowerManager::new(60.0, now).with_thresholds(thresholds);

    // Enter idle.
    let later = now + std::time::Duration::from_secs_f64(IDLE_THRESHOLD_SECS + 1.0);
    pm.begin_frame(later);
    assert!(pm.is_idle());

    // 60 * 0.25 = 15 FPS.
    assert!((pm.effective_fps(false) - 15.0).abs() < 1e-6);
}
