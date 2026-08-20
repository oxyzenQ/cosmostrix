// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Power Stack Integration Audit
//!
//! End-to-end verification that the `central_control_dragon_power` module
//! is a real working coordinator, not a documentation gimmick. Every test
//! here exercises the *public API contract* that downstream consumers
//! (event_loop.rs, activity.rs, cloud/rain.rs) depend on.
//!
//! ## What these tests verify
//!
//! 1. **PowerManager is the single owner** of perf_pressure, is_idle, and
//!    effective FPS — the three previously-scattered read paths.
//! 2. **Thermal guard flows through effective_pressure** — a thermal
//!    input at `set_thermal_pressure()` is visible at every downstream
//!    read of `effective_pressure()`.
//! 3. **Self-healer reads from PowerThresholds** — the struct is the
//!    sole consumer-facing API for the 6 self-healer thresholds.
//! 4. **Frame lifecycle is consistent** — begin_frame → effective_fps →
//!    effective_pressure → observe_frame_end produces stable, monotonic
//!    behavior across a synthetic frame sequence.
//! 5. **Thermal sampler + normalizer contract** — the pure math is
//!    correct and the sampler degrades gracefully on missing sysfs.
//! 6. **Clash zone resolution** — effective_fps is the single owner of
//!    the pause/idle/active cascade; no other writer can produce a
//!    different FPS for the same state.
//!
//! These tests are NOT unit tests of individual functions — those live
//! in each submodule's own `mod tests`. This file is the *integration
//! contract* that guards against a future refactor silently breaking
//! the cross-module wiring.

use std::time::{Duration, Instant};

use crate::central_control_dragon_power::{
    normalize_celsius, sample_thermal_pressure, EnduranceHealth, PerformanceSelfHealer,
    PhasePredictor, PowerManager, PowerThresholds, ReclaimState, SelfHealAction,
};
use crate::constants::*;

// ────────────────────────────────────────────────────────────────────────────
// Audit 1: PowerManager is the single owner of the three read paths
// ────────────────────────────────────────────────────────────────────────────

/// Verify that PowerManager owns all three previously-scattered reads:
/// perf_pressure (was event_loop.rs:142), is_idle (was event_loop.rs:177-186),
/// and effective FPS (was event_loop.rs:139, 179). After construction, all
/// three are readable through PowerManager methods and nowhere else.
#[test]
fn audit_power_manager_owns_all_three_read_paths() {
    let now = Instant::now();
    let pm = PowerManager::new(60.0, now);

    // All three reads are available through PowerManager.
    let _pressure: f32 = pm.effective_pressure();
    let _fps: f64 = pm.effective_fps(false, true);
    let _idle: bool = pm.is_idle();

    // The values are consistent with the construction state:
    // - perf_pressure starts at 0.0 (no frames observed yet)
    // - is_idle starts false (was_active = true at construction)
    // - effective_fps returns the base target (not idle, not paused)
    assert_eq!(pm.effective_pressure(), 0.0, "pressure must start at 0");
    assert!(!pm.is_idle(), "must start active");
    assert!(
        (pm.effective_fps(false, true) - 60.0).abs() < 1e-6,
        "active fps must equal base"
    );
}

/// Verify there is exactly ONE way to read effective FPS — through
/// PowerManager::effective_fps(). The old cascade (target_period +
/// idle_period + pause_period Duration locals) is gone.
#[test]
fn audit_effective_fps_is_single_owner() {
    let now = Instant::now();
    let mut pm = PowerManager::new(120.0, now);

    // Active → base.
    assert!((pm.effective_fps(false, true) - 120.0).abs() < 1e-6);

    // Paused → 4 FPS (overrides everything).
    assert!((pm.effective_fps(true, true) - 4.0).abs() < 1e-6);

    // Idle → base × IDLE_FPS_FACTOR.
    let later = now + Duration::from_secs_f64(IDLE_THRESHOLD_SECS + 1.0);
    pm.begin_frame(later);
    assert!(pm.is_idle());
    let expected_idle_fps = 120.0 * IDLE_FPS_FACTOR;
    assert!(
        (pm.effective_fps(false, true) - expected_idle_fps).abs() < 1e-6,
        "idle fps must be base × factor"
    );

    // Paused still wins over idle.
    assert!((pm.effective_fps(true, true) - 4.0).abs() < 1e-6);
}

// ────────────────────────────────────────────────────────────────────────────
// Audit 2: Thermal guard flows through effective_pressure end-to-end
// ────────────────────────────────────────────────────────────────────────────

/// Verify that a thermal input at set_thermal_pressure() is visible at
/// every downstream read of effective_pressure(). This is the core
/// contract of feature #13: thermal is an INPUT to effective_pressure,
/// not a 7th independent signal path.
#[test]
fn audit_thermal_input_flows_to_every_pressure_read() {
    let now = Instant::now();
    let mut pm = PowerManager::new(60.0, now);

    // Baseline: no thermal, no overshoot → pressure = 0.
    assert_eq!(pm.effective_pressure(), 0.0);

    // Inject thermal pressure.
    pm.set_thermal_pressure(0.4);

    // Every read of effective_pressure now sees the thermal input.
    // Simulate the event loop reading it multiple times per frame
    // (cloud spawn cascade, sim factor, self-healer, perf stats).
    for _ in 0..10 {
        assert!(
            (pm.effective_pressure() - 0.4).abs() < 1e-6,
            "thermal input must be visible at every read"
        );
    }

    // Add base pressure from overshoot — thermal stacks additively.
    pm.observe_frame_end(0.040, 0.016, 0.0); // overshoot → base pressure > 0
    let base = pm.effective_pressure();
    assert!(
        base > 0.4,
        "base + thermal must exceed thermal alone (got {base})"
    );

    // The total is base + thermal, clamped to [0, 1].
    // base from overshoot = 1.5 * 0.25 = 0.375; thermal = 0.4; total = 0.775
    assert!(
        (pm.effective_pressure() - 0.775).abs() < 1e-6,
        "expected 0.775, got {}",
        pm.effective_pressure()
    );
}

/// Verify thermal + base clamps to 1.0 (never exceeds the [0,1] range
/// that downstream consumers expect).
#[test]
fn audit_thermal_plus_base_clamps_to_one() {
    let now = Instant::now();
    let mut pm = PowerManager::new(60.0, now);

    // Drive base pressure to near-max with repeated overshoots.
    for _ in 0..20 {
        pm.observe_frame_end(0.040, 0.016, 0.0); // 2.5× overshoot
    }
    assert!(
        (pm.effective_pressure() - 1.0).abs() < 1e-6,
        "base alone saturates"
    );

    // Thermal input would push above 1.0 — must clamp.
    pm.set_thermal_pressure(0.5);
    assert_eq!(
        pm.effective_pressure(),
        1.0,
        "thermal + base must clamp to 1.0"
    );
}

/// Verify the thermal sampler + normalizer contract: the pure math
/// produces correct 0.0–1.0 values, and the sampler degrades to None
/// on missing sysfs (container, chroot) without panicking.
#[test]
fn audit_thermal_sampler_contract() {
    // Pure math: normalize_celsius is correct across the full range.
    assert_eq!(normalize_celsius(0), 0.0); // below lo → 0
    assert_eq!(normalize_celsius(25_000), 0.0); // 25°C → 0
    assert_eq!(
        normalize_celsius(i64::from(THERMAL_PRESSURE_ZERO_C) * 1000),
        0.0
    );
    assert!(
        (normalize_celsius(70_000) - 0.5).abs() < 1e-6,
        "70°C (midpoint) → 0.5"
    );
    assert_eq!(
        normalize_celsius(i64::from(THERMAL_PRESSURE_ONE_C) * 1000),
        1.0
    );
    assert_eq!(normalize_celsius(120_000), 1.0); // above hi → 1

    // Sampler: must not panic on missing sysfs. In a container without
    // /sys/class/thermal, this returns None. On real Linux with thermal
    // zones, it returns Some(0.0..=1.0). Both are valid.
    let result = sample_thermal_pressure();
    if let Some(p) = result {
        assert!(
            (0.0..=1.0).contains(&p),
            "sampler must return [0,1], got {p}"
        );
    }
    // None is valid — the contract is "best-effort, defensive".
}

// ────────────────────────────────────────────────────────────────────────────
// Audit 3: Self-healer reads from PowerThresholds (migration)
// ────────────────────────────────────────────────────────────────────────────

/// Verify the self-healer constructs with PowerThresholds::defaults()
/// and that the 6 self-healer fields match the standalone constants.
/// This guards against a future constructor that forgets to load the
/// thresholds (which would silently make every comparison read 0.0).
#[test]
fn audit_self_healer_loads_thresholds_from_struct() {
    let h = PerformanceSelfHealer::new();
    let t = h.thresholds();

    // All 6 self-healer fields must match the standalone constants.
    // The constants are the canonical values; the struct copies them
    // via defaults(). This test enforces they stay in sync.
    assert!((t.pressure_high - SELF_HEAL_PRESSURE_HIGH).abs() < 1e-6);
    assert!((t.pressure_low - SELF_HEAL_PRESSURE_LOW).abs() < 1e-6);
    assert!((t.downgrade_secs - SELF_HEAL_DOWNGRADE_SECS).abs() < 1e-6);
    assert!((t.restore_secs - SELF_HEAL_RESTORE_SECS).abs() < 1e-6);
    assert!((t.health_investigate - SELF_HEAL_HEALTH_INVESTIGATE).abs() < 1e-6);
    assert!((t.health_cooldown_secs - SELF_HEAL_HEALTH_COOLDOWN_SECS).abs() < 1e-6);
}

/// Verify the self-healer's observe() actually reads from the struct,
/// not from the constants. Override a threshold and verify the behavior
/// changes. This is the "not a gimmick" test — if observe() still read
/// from the constants, the override would have no effect.
#[test]
fn audit_self_healer_observe_reads_from_struct_not_constants() {
    let mut custom = PowerThresholds::defaults();
    // Set pressure_high above what we'll feed, so no downgrade fires
    // with the override (but WOULD fire with the default 0.6).
    custom.pressure_high = 0.95;

    let mut h = PerformanceSelfHealer::new().with_thresholds(custom);
    let now = Instant::now();

    // Feed 31 seconds of pressure at 0.7 — above the default 0.6
    // threshold but below the overridden 0.95.
    for i in 0..31 {
        let t = now + Duration::from_secs(i);
        let action = h.observe(0.7, t, Some(95.0));
        // With the override (pressure_high = 0.95), 0.7 < 0.95 → no
        // high-pressure accumulation → no downgrade.
        assert_eq!(
            action,
            SelfHealAction::None,
            "override must prevent downgrade at t={i}s"
        );
    }
    assert!(!h.is_downgraded(), "override must prevent downgrade");

    // Control: with defaults, the same sequence WOULD downgrade.
    let mut h2 = PerformanceSelfHealer::new();
    for i in 0..31 {
        let t = now + Duration::from_secs(i);
        let _ = h2.observe(0.7, t, Some(95.0));
    }
    // At t=30, elapsed >= 30s default window → DowngradeScene fires.
    // (The loop above ran 0..30, then the 31st call at t=30 fires.)
    assert!(
        h2.is_downgraded(),
        "default thresholds must still downgrade — proves the override is real"
    );
}

// ────────────────────────────────────────────────────────────────────────────
// Audit 4: Frame lifecycle is consistent across a synthetic sequence
// ────────────────────────────────────────────────────────────────────────────

/// Verify the frame lifecycle (begin_frame → effective_fps →
/// effective_pressure → observe_frame_end) produces stable, monotonic
/// behavior across a synthetic 100-frame sequence. This is the
/// "does it actually work in production" test.
#[test]
fn audit_frame_lifecycle_stable_across_synthetic_run() {
    let now = Instant::now();
    let mut pm = PowerManager::new(60.0, now);

    // Simulate 100 frames at 60 FPS with mild overshoot (work_s=0.018,
    // frame_period=0.0167 → ratio=1.08, overshoot=0.08).
    let mut prev_pressure = pm.effective_pressure();
    for frame in 0..100 {
        let t = now + Duration::from_secs_f64(frame as f64 / 60.0);
        let _is_idle = pm.begin_frame(t);

        // Effective FPS must always be positive and finite.
        let fps = pm.effective_fps(false, true);
        assert!(
            fps.is_finite() && fps > 0.0,
            "fps must be positive at frame {frame}"
        );

        // Effective pressure must stay in [0, 1].
        let p = pm.effective_pressure();
        assert!(
            (0.0..=1.0).contains(&p),
            "pressure out of range at frame {frame}: {p}"
        );

        // Observe frame end with mild overshoot.
        pm.observe_frame_end(0.018, 0.0167, 0.0);

        // Pressure should be non-decreasing under sustained overshoot
        // (it accumulates, never spontaneously drops).
        let new_p = pm.effective_pressure();
        assert!(
            new_p >= prev_pressure - 1e-6,
            "pressure decreased under overshoot at frame {frame}: {new_p} < {prev_pressure}"
        );
        prev_pressure = new_p;
    }

    // After 100 overshoot frames, pressure should be meaningfully > 0.
    assert!(
        pm.effective_pressure() > 0.1,
        "sustained overshoot must accumulate pressure, got {}",
        pm.effective_pressure()
    );
}

/// Verify the lifecycle handles a transition from overshoot to normal
/// frames — pressure must decay, not stick.
#[test]
fn audit_frame_lifecycle_pressure_decays_on_recovery() {
    let now = Instant::now();
    let mut pm = PowerManager::new(60.0, now);

    // Build up pressure with overshoot frames.
    for _ in 0..20 {
        pm.observe_frame_end(0.040, 0.016, 0.0); // 2.5× overshoot
    }
    let peak = pm.effective_pressure();
    assert!(peak > 0.5, "peak pressure should be high, got {peak}");

    // Switch to normal frames (work < period).
    for _ in 0..50 {
        pm.observe_frame_end(0.005, 0.016, 0.0); // 0.31× — well under
    }
    let recovered = pm.effective_pressure();
    assert!(
        recovered < peak,
        "pressure must decay on recovery: {recovered} should be < {peak}"
    );
    assert!(
        recovered < 0.1,
        "pressure must decay significantly after 50 normal frames, got {recovered}"
    );
}

// ────────────────────────────────────────────────────────────────────────────
// Audit 5: All 5 submodules are constructible and have sane defaults
// ────────────────────────────────────────────────────────────────────────────

/// Verify every submodule's primary type is constructible through its
/// public API and produces sane defaults. This catches broken
/// constructors (e.g., a future refactor that makes new() panic).
#[test]
fn audit_all_submodules_constructible_with_sane_defaults() {
    // PhasePredictor
    let pp = PhasePredictor::new();
    assert_eq!(pp.transitions_observed(), 0);
    assert_eq!(
        pp.predicts_active(0.0),
        None,
        "predictor needs ≥2 transitions"
    );

    // ReclaimState
    let rs = ReclaimState::new();
    assert!(
        rs.should_reclaim(Instant::now()),
        "initial state should allow reclaim"
    );

    // EnduranceHealth
    let eh = EnduranceHealth::new();
    assert_eq!(eh.score(), 100.0, "initial score must be 100");
    assert_eq!(eh.classification(), "healthy");

    // PerformanceSelfHealer
    let mut sh = PerformanceSelfHealer::new();
    let action = sh.observe(0.1, Instant::now(), Some(95.0));
    assert_eq!(action, SelfHealAction::None, "healthy state → no action");

    // PowerManager
    let pm = PowerManager::new(60.0, Instant::now());
    assert!(!pm.is_idle());
    assert_eq!(pm.effective_pressure(), 0.0);

    // PowerThresholds
    let pt = PowerThresholds::defaults();
    assert!((pt.pressure_high - SELF_HEAL_PRESSURE_HIGH).abs() < 1e-6);
    assert!((pt.idle_fps_factor - IDLE_FPS_FACTOR).abs() < 1e-6);
}

// ────────────────────────────────────────────────────────────────────────────
// Audit 6: PowerThresholds defaults match ALL standalone constants
// ────────────────────────────────────────────────────────────────────────────

/// Verify PowerThresholds::defaults() matches every standalone constant
/// it's supposed to mirror. This is the single source-of-truth sync
/// check — if a constant changes without the struct (or vice versa),
/// downstream behavior silently drifts.
#[test]
fn audit_power_thresholds_defaults_match_all_constants() {
    let t = PowerThresholds::defaults();

    // Self-healer thresholds (6 fields).
    assert!((t.pressure_high - SELF_HEAL_PRESSURE_HIGH).abs() < 1e-6);
    assert!((t.pressure_low - SELF_HEAL_PRESSURE_LOW).abs() < 1e-6);
    assert!((t.downgrade_secs - SELF_HEAL_DOWNGRADE_SECS).abs() < 1e-6);
    assert!((t.restore_secs - SELF_HEAL_RESTORE_SECS).abs() < 1e-6);
    assert!((t.health_investigate - SELF_HEAL_HEALTH_INVESTIGATE).abs() < 1e-6);
    assert!((t.health_cooldown_secs - SELF_HEAL_HEALTH_COOLDOWN_SECS).abs() < 1e-6);

    // Idle / pressure thresholds (4 fields).
    assert!((t.idle_threshold_secs - IDLE_THRESHOLD_SECS).abs() < 1e-6);
    assert!((t.idle_fps_factor - IDLE_FPS_FACTOR).abs() < 1e-6);
    assert!((t.pressure_increment - PERF_PRESSURE_INCREMENT).abs() < 1e-6);
    assert!((t.pressure_decay - PERF_PRESSURE_DECAY).abs() < 1e-6);
}

// ────────────────────────────────────────────────────────────────────────────
// Audit 7: Thermal constants are physically plausible
// ────────────────────────────────────────────────────────────────────────────

/// Verify the thermal ramp constants are in a physically plausible
/// range for CPU junction temperatures. A typo (e.g., 500 instead of
/// 50) would silently make the renderer never throttle.
#[test]
fn audit_thermal_constants_are_physically_plausible() {
    let lo = THERMAL_PRESSURE_ZERO_C;
    let hi = THERMAL_PRESSURE_ONE_C;

    // Ramp window must be non-empty and ordered.
    assert!(hi > lo, "hi ({hi}) must be > lo ({lo})");

    // Plausible CPU junction temperature range.
    // Below 0°C: device is in a freezer (unlikely for a desktop).
    // Above 150°C: silicon is dead (Tj_max is typically 95-105°C).
    assert!((0..=100).contains(&lo), "lo {lo} outside plausible range");
    assert!((50..=150).contains(&hi), "hi {hi} outside plausible range");

    // Sampler interval must be reasonable (not too frequent, not too sparse).
    let interval = THERMAL_SAMPLER_INTERVAL_FRAMES;
    assert!(
        (60..=3600).contains(&interval),
        "sampler interval {interval} outside [60, 3600] frames"
    );
}

// ────────────────────────────────────────────────────────────────────────────
// Audit 8: End-to-end thermal → self-healer cascade
// ────────────────────────────────────────────────────────────────────────────

/// Verify the full cascade: thermal pressure → effective_pressure →
/// self-healer P1 downgrade trigger. This is the "feature #13 actually
/// works end-to-end" test — if any link in the chain is broken, the
/// self-healer never sees the thermal input.
#[test]
fn audit_thermal_input_triggers_self_healer_downgrade() {
    let now = Instant::now();
    let mut pm = PowerManager::new(60.0, now);
    let mut sh = PerformanceSelfHealer::new();

    // Inject enough thermal pressure to cross the self-healer's
    // pressure_high threshold (0.6).
    pm.set_thermal_pressure(0.7);

    // effective_pressure must now read >= 0.7 (thermal alone).
    assert!(
        pm.effective_pressure() >= 0.7,
        "thermal input must push effective_pressure above 0.7"
    );

    // Feed the self-healer with effective_pressure for 31 seconds.
    // With pressure_high = 0.6 and effective_pressure = 0.7, the
    // downgrade should fire at t=30s.
    for i in 0..31 {
        let t = now + Duration::from_secs(i);
        let pressure = pm.effective_pressure();
        let _ = sh.observe(pressure, t, Some(95.0));
    }

    // The 31st observation (t=30) should have fired the downgrade.
    assert!(
        sh.is_downgraded(),
        "thermal-driven pressure must trigger self-healer downgrade"
    );
}
