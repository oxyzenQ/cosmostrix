// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! power_manager tests, extracted from inline `mod tests { ... }` block.
//!
//! Uses `use super::*;` to access parent's private items unchanged.

use super::*;
use std::hint::black_box;
use std::time::Duration;

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
    let fps = black_box(pm).effective_fps(false, true);
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
    assert!((pm.effective_fps(false, true) - 144.0).abs() < 1e-6);
}

#[test]
fn effective_fps_paused_returns_4_fps() {
    let now = Instant::now();
    let pm = PowerManager::new(144.0, now);
    let paused_fps = black_box(pm).effective_fps(true, true);
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

    let idle_fps = pm.effective_fps(false, true);
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
    let fps = pm.effective_fps(true, true);
    assert!((fps - 4.0).abs() < 1e-6);
}

#[test]
fn set_target_fps_updates_effective_fps() {
    let now = Instant::now();
    let mut pm = PowerManager::new(60.0, now);
    assert!((pm.effective_fps(false, true) - 60.0).abs() < 1e-6);

    pm.set_target_fps(120.0);
    assert!((pm.effective_fps(false, true) - 120.0).abs() < 1e-6);
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
    assert!((pm.effective_fps(false, true) - 15.0).abs() < 1e-6);
}

// ── power_dragon toggle ──────────────────────────────────────────────

#[test]
fn power_dragon_false_skips_idle_fps_reduction() {
    let now = Instant::now();
    let mut pm = PowerManager::new(60.0, now);

    // Enter idle state.
    let later = now + std::time::Duration::from_secs_f64(IDLE_THRESHOLD_SECS + 1.0);
    let is_idle = pm.begin_frame(later);
    assert!(is_idle, "must be idle for this test to be meaningful");

    // With power_dragon=true (default): idle reduces FPS.
    let fps_with_pd = pm.effective_fps(false, true);
    let expected_idle_fps = 60.0 * IDLE_FPS_FACTOR;
    assert!(
        (fps_with_pd - expected_idle_fps).abs() < 1e-6,
        "power_dragon=true should apply idle FPS factor"
    );

    // With power_dragon=false: idle does NOT reduce FPS.
    let fps_without_pd = pm.effective_fps(false, false);
    assert!(
        (fps_without_pd - 60.0).abs() < 1e-6,
        "power_dragon=false should return base FPS even when idle"
    );

    // Verify the difference is real (not a coincidence).
    assert!(
        fps_without_pd > fps_with_pd,
        "power_dragon=false must yield higher FPS than true when idle"
    );
}

#[test]
fn power_dragon_false_does_not_affect_paused_fps() {
    let now = Instant::now();
    let pm = PowerManager::new(60.0, now);
    let expected = 1000.0 / PAUSE_PERIOD_MS as f64;

    // Paused FPS is always 4 regardless of power_dragon.
    let fps_true = black_box(&pm).effective_fps(true, true);
    let fps_false = black_box(&pm).effective_fps(true, false);
    assert!((fps_true - expected).abs() < 1e-6);
    assert!((fps_false - expected).abs() < 1e-6);
}

#[test]
fn power_dragon_false_does_not_affect_active_fps() {
    let now = Instant::now();
    let pm = PowerManager::new(60.0, now);

    // Active FPS is always base regardless of power_dragon.
    let fps_true = pm.effective_fps(false, true);
    let fps_false = pm.effective_fps(false, false);
    assert!((fps_true - 60.0).abs() < 1e-6);
    assert!((fps_false - 60.0).abs() < 1e-6);
}

// ── S-master-HUNT-23: output drain backoff ────────────────────────────
//
// The drain backoff closes the output loop: measured terminal write
// latency overshoot (content write + flush syscall, per DRAWN frame)
// scales effective_fps down toward the terminal's sustainable drain
// rate; clean writes decay it back. These tests lock the math, the
// power_dragon gate, the composition with idle, the pause override,
// and the floor interaction with a low user-configured base.

#[test]
fn hunts23_drain_backoff_rises_on_write_overshoot() {
    let now = Instant::now();
    let mut pm = PowerManager::new(144.0, now);
    assert_eq!(pm.drain_backoff(), 0.0);
    // Fully blocked flush: write_overshoot 2.0 → +0.1 per frame.
    for _ in 0..10 {
        pm.observe_frame_end(0.005, 1.0 / 144.0, 2.0);
    }
    assert!(
        (pm.drain_backoff() - 1.0).abs() < 1e-6,
        "10 frames of blocked writes must saturate the backoff, got {}",
        pm.drain_backoff()
    );
    // Full backoff → floor at 25% of base: 144 × 0.25 = 36.
    assert!((pm.effective_fps(false, true) - 36.0).abs() < 1e-6);
}

#[test]
fn hunts23_drain_backoff_recovers_slowly_on_clean_writes() {
    let now = Instant::now();
    let mut pm = PowerManager::new(60.0, now);
    for _ in 0..10 {
        pm.observe_frame_end(0.005, 1.0 / 60.0, 2.0);
    }
    assert!((pm.drain_backoff() - 1.0).abs() < 1e-6);
    // Clean writes decay by OUTPUT_DRAIN_BACKOFF_FALL per frame.
    pm.observe_frame_end(0.005, 1.0 / 60.0, 0.0);
    let expected = 1.0 - OUTPUT_DRAIN_BACKOFF_FALL;
    assert!(
        (pm.drain_backoff() - expected).abs() < 1e-6,
        "clean write must decay the backoff by exactly FALL, got {}",
        pm.drain_backoff()
    );
    // Full recovery takes ~1/FALL frames — slow by design (no flapping).
    for _ in 0..600 {
        pm.observe_frame_end(0.005, 1.0 / 60.0, 0.0);
    }
    assert_eq!(pm.drain_backoff(), 0.0);
    assert!((pm.effective_fps(false, true) - 60.0).abs() < 1e-6);
}

#[test]
fn hunts23_drain_backoff_disabled_when_power_dragon_off() {
    let now = Instant::now();
    let mut pm = PowerManager::new(144.0, now);
    for _ in 0..10 {
        pm.observe_frame_end(0.005, 1.0 / 144.0, 2.0);
    }
    // The backoff STATE still accumulates (the signal is real), but the
    // cadence must ignore it when the user disabled adaptive protection
    // (owner Option D contract, same gating as the idle FPS reduction).
    assert!((pm.drain_backoff() - 1.0).abs() < 1e-6);
    assert!((pm.effective_fps(false, false) - 144.0).abs() < 1e-6);
}

#[test]
fn hunts23_drain_backoff_composes_with_idle() {
    let now = Instant::now();
    let mut pm = PowerManager::new(60.0, now);
    // Enter idle (no input for the threshold).
    let idle_point = now + Duration::from_secs_f64(IDLE_THRESHOLD_SECS + 1.0);
    pm.begin_frame(idle_point);
    assert!(pm.is_idle());
    for _ in 0..10 {
        pm.observe_frame_end(0.005, 1.0 / 30.0, 2.0);
    }
    // idle (×0.5) then backoff (×0.25): 60 → 30 → 7.5, floored at 12.
    let fps = pm.effective_fps(false, true);
    assert!(
        (fps - 12.0).abs() < 1e-6,
        "idle + full backoff must hit the floor (12), got {}",
        fps
    );
}

#[test]
fn hunts23_drain_backoff_floor_never_raises_low_user_base() {
    let now = Instant::now();
    let mut pm = PowerManager::new(10.0, now); // user pinned --fps 10
    for _ in 0..10 {
        pm.observe_frame_end(0.005, 0.1, 2.0);
    }
    let fps = pm.effective_fps(false, true);
    assert!(
        fps <= 10.0,
        "the backoff floor must never RAISE the target above the user base, got {}",
        fps
    );
}

#[test]
fn hunts23_drain_backoff_paused_takes_precedence() {
    let now = Instant::now();
    let mut pm = PowerManager::new(60.0, now);
    for _ in 0..10 {
        pm.observe_frame_end(0.005, 1.0 / 60.0, 2.0);
    }
    let expected = 1000.0 / PAUSE_PERIOD_MS as f64;
    assert!((pm.effective_fps(true, true) - expected).abs() < 1e-6);
}

#[test]
fn hunts23_drain_backoff_zero_write_overshoot_no_rise() {
    let now = Instant::now();
    let mut pm = PowerManager::new(60.0, now);
    // Overshoot from WORK time only (CPU), write clean — backoff must NOT
    // move: this is CPU pressure (spawn throttle's domain), not drain.
    for _ in 0..50 {
        pm.observe_frame_end(0.040, 1.0 / 60.0, 0.0);
    }
    assert_eq!(pm.drain_backoff(), 0.0);
    assert!((pm.effective_fps(false, true) - 60.0).abs() < 1e-6);
}

// ── NIGHT-hunter-2: visual-pressure EMA ─────────────────────────────────
//
// The EMA decouples the VISUAL pressure consumers (phosphor skip
// hysteresis, spawn scale, glitch gate, sim cap) from the raw
// fast-attack control signal. These tests lock the contract the
// time-constant rationale promises:
//   - transient congestion spikes stay below every visual threshold
//   - sustained congestion still fully engages them
//   - the filter is frame-rate independent (wall-clock based)
//   - release decays with the documented time constant
//   - the power-dragon Option D gate returns exactly 0.0 when off

/// Helper: drive `n` frames of full write-overshoot pressure at `fps`
/// with begin_frame advancing the wall clock, mirroring the production
/// begin_frame -> observe_frame_end pairing.
fn nh2_drive_pressure(pm: &mut PowerManager, start: Instant, secs: f64, fps: f64) -> Instant {
    let step = 1.0 / fps;
    let n = (secs * fps).round() as i64;
    let mut t = start;
    for _ in 0..n.max(0) {
        t += Duration::from_secs_f64(step);
        pm.begin_frame(t);
        // write_overshoot 2.0 (fully blocked flush) pins raw pressure at
        // 1.0 within a couple frames, exactly like the measured PTY runs.
        pm.observe_frame_end(0.001, step as f32, 2.0);
    }
    t
}

/// Helper: advance the clock with NO pressure (clean frames).
fn nh2_drive_calm(pm: &mut PowerManager, start: Instant, secs: f64, fps: f64) -> Instant {
    let step = 1.0 / fps;
    let n = (secs * fps).round() as i64;
    let mut t = start;
    for _ in 0..n.max(0) {
        t += Duration::from_secs_f64(step);
        pm.begin_frame(t);
        pm.observe_frame_end(0.001, step as f32, 0.0);
    }
    t
}

#[test]
fn nh2_short_spike_stays_below_every_visual_threshold() {
    // The measured congestion spikes of the drain loop last ~0.5-1.5 s at
    // full pressure. With tau = 2.5 s the EMA peak after 1.5 s of full
    // pressure is 1 - e^(-0.6) ~= 0.45 — below the phosphor skip LOW
    // threshold (0.50). After 0.5 s it is ~= 0.18, below the glitch gate
    // (0.35). Both must hold: transients may not move any visual system.
    let now = Instant::now();
    let mut pm = PowerManager::new(144.0, now);

    nh2_drive_pressure(&mut pm, now, 0.5, 144.0);
    assert!(
        pm.visual_pressure() < 0.35,
        "0.5s spike must stay below the glitch gate, got {}",
        pm.visual_pressure()
    );

    let mut pm2 = PowerManager::new(144.0, now);
    nh2_drive_pressure(&mut pm2, now, 1.5, 144.0);
    assert!(
        pm2.visual_pressure() < 0.50,
        "1.5s spike must stay below the phosphor skip LOW threshold, got {}",
        pm2.visual_pressure()
    );
}

#[test]
fn nh2_sustained_congestion_engages_full_protection() {
    // 8 s of sustained full pressure (the VTE / xterm.js backlog case the
    // skip thresholds exist for) must bring the EMA past the HIGH
    // threshold (0.70): 1 - e^(-8/2.5) ~= 0.96. Protection timing shifts
    // by ~3 s vs raw, which the slow-developing backlog case tolerates.
    let now = Instant::now();
    let mut pm = PowerManager::new(60.0, now);
    let t = nh2_drive_pressure(&mut pm, now, 8.0, 60.0);
    assert!(
        pm.visual_pressure() > 0.70,
        "8s sustained congestion must cross the skip HIGH threshold, got {}",
        pm.visual_pressure()
    );
    // And it must still be rising toward the raw value, not stuck.
    let t2 = nh2_drive_pressure(&mut pm, t, 4.0, 60.0);
    assert!(
        pm.visual_pressure() > 0.90,
        "12s sustained congestion must approach the raw signal, got {}",
        pm.visual_pressure()
    );
    let _ = t2;
}

#[test]
fn nh2_ema_is_frame_rate_independent() {
    // Same wall time + same pressure input at 30 FPS and 144 FPS must
    // land on the same EMA value (wall-clock alpha, not per-frame step).
    let now = Instant::now();
    let mut pm30 = PowerManager::new(30.0, now);
    let mut pm144 = PowerManager::new(144.0, now);
    nh2_drive_pressure(&mut pm30, now, 3.0, 30.0);
    nh2_drive_pressure(&mut pm144, now, 3.0, 144.0);
    let a = pm30.visual_pressure();
    let b = pm144.visual_pressure();
    assert!(
        (a - b).abs() < 0.02,
        "3s of full pressure must filter identically at 30fps ({a}) and 144fps ({b})"
    );
}

#[test]
fn nh2_release_decays_with_documented_time_constant() {
    // Drive the EMA to ~0.70, release pressure, and verify the decay is
    // smooth exponential with tau = 2.5 s — NOT a snap to zero like the
    // raw signal (which hits 0.0 in ~0.8 s at 60 FPS via the 0.02/frame
    // decay). Two locked properties:
    //   1. after ~1.7 s (one tau-fraction e^(-1.7/2.5) ~= 0.51) the EMA
    //      is at or above the idealized instant-zero-target decay value
    //      peak*e^(-1.7/tau), because the real target (raw pressure)
    //      itself decays gradually, and the EMA lags behind even that;
    //   2. the EMA is still materially nonzero — the phosphor/spawn
    //      systems ramp down over seconds, never step.
    let now = Instant::now();
    let mut pm = PowerManager::new(60.0, now);
    let t = nh2_drive_pressure(&mut pm, now, 3.0, 60.0);
    let peak = pm.visual_pressure();
    assert!(peak > 0.65, "precondition: EMA near 0.70, got {peak}");

    let t2 = nh2_drive_calm(&mut pm, t, 1.7, 60.0);
    let _ = t2;
    let after = pm.visual_pressure();
    let ideal_floor = peak * (-1.7f32 / VISUAL_PRESSURE_EMA_TAU_SECS).exp();
    assert!(
        after >= ideal_floor - 1e-4,
        "EMA must decay no faster than the tau=2.5s ideal curve: peak={peak} after={after} ideal={ideal_floor}"
    );
    assert!(
        after < peak,
        "EMA must be decaying after pressure release: peak={peak} after={after}"
    );
    assert!(
        after > 0.30,
        "after 1.7s of release the EMA must still be material (no snap), after={after}"
    );
}

#[test]
fn nh2_applied_visual_pressure_gated_by_power_dragon() {
    // v80 Option D contract, now covering the sim cap too: dragon off →
    // the applied feed is exactly 0.0 no matter the accumulated pressure.
    let now = Instant::now();
    let mut pm = PowerManager::new(60.0, now);
    nh2_drive_pressure(&mut pm, now, 6.0, 60.0);
    assert!(pm.visual_pressure() > 0.5);
    assert_eq!(pm.applied_visual_pressure(false), 0.0);
    assert!((pm.applied_visual_pressure(true) - pm.visual_pressure()).abs() < 1e-6);
}

#[test]
fn nh2_control_side_reads_raw_not_ema() {
    // The drain pacing / self-healer / P5 consumers keep the raw
    // fast-attack signal: after a 0.5 s spike the RAW pressure is pinned
    // at 1.0 while the EMA stays low. Both must be observable at once.
    let now = Instant::now();
    let mut pm = PowerManager::new(60.0, now);
    nh2_drive_pressure(&mut pm, now, 0.5, 60.0);
    assert!(
        (pm.effective_pressure() - 1.0).abs() < 1e-6,
        "raw pressure must still spike to 1.0 for the control side"
    );
    assert!(
        pm.visual_pressure() < 0.35,
        "while the visual EMA filters the same spike away"
    );
}

#[test]
fn nh2_long_stall_does_not_teleport_the_filter() {
    // A SIGSTOP/debugger pause of 10 s between frames must not dump the
    // whole gap into one EMA step (dt capped at 250 ms); the filter moves
    // at most one cap-sized step per frame and re-converges over the
    // frames that follow.
    let now = Instant::now();
    let mut pm = PowerManager::new(60.0, now);
    // Warm the EMA to a known mid value first.
    let t = nh2_drive_pressure(&mut pm, now, 3.0, 60.0);
    let before = pm.visual_pressure();
    // One frame after a 10 s stall, pressure still held (stale-but-present).
    let t2 = t + Duration::from_secs(10);
    pm.begin_frame(t2);
    let after = pm.visual_pressure();
    // One 250 ms step toward target 1.0: 1 - e^(-0.25/2.5) ~= 0.095.
    let max_step = (1.0 - before) * 0.11;
    assert!(
        (after - before) <= max_step,
        "single post-stall frame must move the EMA by at most one capped step, before={before} after={after}"
    );
}
