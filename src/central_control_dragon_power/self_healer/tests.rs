// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! self_healer tests, extracted from inline `mod tests { ... }` block.
//!
//! Uses `use super::*;` to access parent's private items unchanged.

use super::*;
use std::time::Duration;

#[test]
fn self_healer_starts_healthy_and_returns_none() {
    let mut h = PerformanceSelfHealer::new();
    let now = Instant::now();
    // Low pressure, healthy score → no action.
    let action = h.observe(0.1, now, Some(95.0));
    assert_eq!(action, SelfHealAction::None);
    assert!(!h.is_downgraded());
}

#[test]
fn self_healer_p1_does_not_downgrade_before_window() {
    let mut h = PerformanceSelfHealer::new();
    let t0 = Instant::now();
    // 29 seconds of sustained high pressure — one second short of the
    // 30s downgrade window. Should NOT fire.
    for i in 0..29 {
        let t = t0 + Duration::from_secs(i);
        let action = h.observe(SELF_HEAL_PRESSURE_HIGH, t, Some(95.0));
        assert_eq!(
            action,
            SelfHealAction::None,
            "should not downgrade at t={i}s"
        );
    }
    assert!(!h.is_downgraded());
}

#[test]
fn self_healer_p1_downgrades_after_window() {
    let mut h = PerformanceSelfHealer::new();
    let t0 = Instant::now();
    // 30 seconds of sustained high pressure — exactly at the window.
    // The 31st observation (t=30s) should fire the downgrade.
    for i in 0..30 {
        let t = t0 + Duration::from_secs(i);
        let _ = h.observe(SELF_HEAL_PRESSURE_HIGH, t, Some(95.0));
    }
    let action = h.observe(
        SELF_HEAL_PRESSURE_HIGH,
        t0 + Duration::from_secs(30),
        Some(95.0),
    );
    assert_eq!(action, SelfHealAction::DowngradeScene);
    assert!(h.is_downgraded());
}

#[test]
fn self_healer_p1_hysteresis_single_cool_frame_breaks_streak() {
    let mut h = PerformanceSelfHealer::new();
    let t0 = Instant::now();
    // 20 seconds of high pressure.
    for i in 0..20 {
        let t = t0 + Duration::from_secs(i);
        let _ = h.observe(SELF_HEAL_PRESSURE_HIGH, t, Some(95.0));
    }
    // One cool frame (low pressure) — breaks the streak.
    let _ = h.observe(
        SELF_HEAL_PRESSURE_LOW,
        t0 + Duration::from_secs(20),
        Some(95.0),
    );
    // 10 more seconds of high pressure (total 30s high, but split).
    for i in 21..31 {
        let t = t0 + Duration::from_secs(i);
        let action = h.observe(SELF_HEAL_PRESSURE_HIGH, t, Some(95.0));
        assert_eq!(
            action,
            SelfHealAction::None,
            "streak was broken at t=20s; should not downgrade at t={i}s"
        );
    }
    assert!(!h.is_downgraded());
}

#[test]
fn self_healer_p1_middle_band_does_not_accumulate() {
    let mut h = PerformanceSelfHealer::new();
    let t0 = Instant::now();
    // 60 seconds of middle-band pressure (between LOW and HIGH).
    // Neither streak should accumulate, so no downgrade ever fires.
    let mid = (SELF_HEAL_PRESSURE_LOW + SELF_HEAL_PRESSURE_HIGH) / 2.0;
    for i in 0..60 {
        let t = t0 + Duration::from_secs(i);
        let action = h.observe(mid, t, Some(95.0));
        assert_eq!(action, SelfHealAction::None);
    }
    assert!(!h.is_downgraded());
}

#[test]
fn self_healer_p1_restore_after_recovery_window() {
    let mut h = PerformanceSelfHealer::new();
    let t0 = Instant::now();
    // Trigger downgrade at t=30s.
    for i in 0..31 {
        let t = t0 + Duration::from_secs(i);
        let _ = h.observe(SELF_HEAL_PRESSURE_HIGH, t, Some(95.0));
    }
    assert!(h.is_downgraded());
    h.record_downgrade("storm");

    // 60 seconds of sustained low pressure — should restore at t=91.
    for i in 31..91 {
        let t = t0 + Duration::from_secs(i);
        let _ = h.observe(SELF_HEAL_PRESSURE_LOW, t, Some(95.0));
    }
    let action = h.observe(
        SELF_HEAL_PRESSURE_LOW,
        t0 + Duration::from_secs(91),
        Some(95.0),
    );
    assert_eq!(action, SelfHealAction::RestoreScene);
    assert!(!h.is_downgraded());
    // The saved scene should be retrievable.
    let restored = h.take_pre_degraded_scene();
    assert_eq!(restored.as_deref(), Some("storm"));
}

#[test]
fn self_healer_p1_restore_requires_full_window() {
    let mut h = PerformanceSelfHealer::new();
    let t0 = Instant::now();
    // Trigger downgrade.
    for i in 0..31 {
        let t = t0 + Duration::from_secs(i);
        let _ = h.observe(SELF_HEAL_PRESSURE_HIGH, t, Some(95.0));
    }
    h.record_downgrade("cosmos");

    // 59 seconds of low pressure — one short of the 60s restore window.
    for i in 31..90 {
        let t = t0 + Duration::from_secs(i);
        let action = h.observe(SELF_HEAL_PRESSURE_LOW, t, Some(95.0));
        assert_eq!(action, SelfHealAction::None, "should not restore at t={i}s");
    }
    assert!(h.is_downgraded(), "should still be downgraded");
}

#[test]
fn self_healer_p1_high_pressure_while_downgraded_does_not_re_downgrade() {
    let mut h = PerformanceSelfHealer::new();
    let t0 = Instant::now();
    // Downgrade.
    for i in 0..31 {
        let t = t0 + Duration::from_secs(i);
        let _ = h.observe(SELF_HEAL_PRESSURE_HIGH, t, Some(95.0));
    }
    h.record_downgrade("storm");
    assert!(h.is_downgraded());

    // More high pressure — should NOT fire another DowngradeScene.
    let action = h.observe(
        SELF_HEAL_PRESSURE_HIGH,
        t0 + Duration::from_secs(100),
        Some(95.0),
    );
    assert_eq!(action, SelfHealAction::None);
}

#[test]
fn self_healer_p2_health_mitigation_fires_on_low_score() {
    let mut h = PerformanceSelfHealer::new();
    let now = Instant::now();
    // Score below investigate threshold → should fire.
    let action = h.observe(0.1, now, Some(50.0));
    assert_eq!(action, SelfHealAction::TriggerHealthMitigation);
}

#[test]
fn self_healer_p2_health_mitigation_respects_cooldown() {
    let mut h = PerformanceSelfHealer::new();
    let t0 = Instant::now();
    // First fire at t=0.
    let action = h.observe(0.1, t0, Some(40.0));
    assert_eq!(action, SelfHealAction::TriggerHealthMitigation);

    // 10 seconds later — within cooldown. Should NOT fire.
    let action = h.observe(0.1, t0 + Duration::from_secs(10), Some(40.0));
    assert_eq!(action, SelfHealAction::None);

    // 31 seconds later — past cooldown. Should fire.
    let action = h.observe(0.1, t0 + Duration::from_secs(31), Some(40.0));
    assert_eq!(action, SelfHealAction::TriggerHealthMitigation);
}

#[test]
fn self_healer_p2_none_score_skips_health_check() {
    let mut h = PerformanceSelfHealer::new();
    let now = Instant::now();
    // No health score (perf_stats off) → P2 skipped entirely.
    // Even with high pressure, no health mitigation should fire.
    let action = h.observe(SELF_HEAL_PRESSURE_HIGH, now, None);
    // P1 won't fire either (no accumulated streak), so None.
    assert_eq!(action, SelfHealAction::None);
}

#[test]
fn self_healer_p2_evaluated_before_p1() {
    // When both P1 and P2 conditions are met on the same tick, P2 wins.
    let mut h = PerformanceSelfHealer::new();
    let t0 = Instant::now();
    // Accumulate 29s of high pressure with a healthy score. This sets
    // high_pressure_since = t0 but does NOT fire the downgrade yet
    // (elapsed = 29s < 30s window).
    for i in 0..30 {
        let t = t0 + Duration::from_secs(i);
        let _ = h.observe(SELF_HEAL_PRESSURE_HIGH, t, Some(95.0));
    }
    assert!(!h.is_downgraded(), "should not be downgraded yet");

    // Now at t=30, P1 would fire (elapsed = 30s >= 30s window). But
    // the health score drops to investigate level on this same tick.
    // P2 should win (evaluated first) and P1 state should stay clean.
    let action = h.observe(
        SELF_HEAL_PRESSURE_HIGH,
        t0 + Duration::from_secs(30),
        Some(40.0),
    );
    assert_eq!(action, SelfHealAction::TriggerHealthMitigation);
    // P1 state should be unchanged (not downgraded).
    assert!(!h.is_downgraded());
}

#[test]
fn self_healer_reset_clears_all_state_except_cooldown() {
    let mut h = PerformanceSelfHealer::new();
    let t0 = Instant::now();
    // Downgrade + record scene.
    for i in 0..31 {
        let t = t0 + Duration::from_secs(i);
        let _ = h.observe(SELF_HEAL_PRESSURE_HIGH, t, Some(95.0));
    }
    h.record_downgrade("storm");
    // Fire a health mitigation to set the cooldown.
    let _ = h.observe(0.1, t0 + Duration::from_secs(31), Some(40.0));
    assert!(h.is_downgraded());

    // Reset.
    h.reset();
    assert!(!h.is_downgraded());
    assert_eq!(h.take_pre_degraded_scene(), None);

    // Cooldown should persist — a new health mitigation should NOT fire
    // immediately after reset.
    let action = h.observe(0.1, t0 + Duration::from_secs(32), Some(40.0));
    assert_eq!(action, SelfHealAction::None);
}

#[test]
fn self_healer_take_pre_degraded_scene_returns_none_when_empty() {
    let mut h = PerformanceSelfHealer::new();
    assert_eq!(h.take_pre_degraded_scene(), None);
}

#[test]
fn self_healer_fallback_scene_is_low_power() {
    // The fallback scene must be a built-in scene name that exists
    // in the scene registry. "low-power" is the canonical low-CPU
    // scene (speed=5, density=0.45, glitch_level=None).
    // (FPS-F7): comment corrected — fps=30 was removed
    // (scene fps is startup-only by design; the CPU shed comes from
    // speed/density/glitch, not from a runtime fps drop).
    assert_eq!(PerformanceSelfHealer::FALLBACK_SCENE, "low-power");
}

#[test]
fn self_healer_loads_default_thresholds_at_construction() {
    // the healer must construct with PowerThresholds::defaults()
    // so production behavior matches the documented constants. This
    // test guards against a future constructor that forgets to load
    // the thresholds (which would silently make every comparison
    // read 0.0 and never trigger any mitigation).
    let h = PerformanceSelfHealer::new();
    let t = h.thresholds();
    assert!((t.pressure_high - SELF_HEAL_PRESSURE_HIGH).abs() < 1e-6);
    assert!((t.pressure_low - SELF_HEAL_PRESSURE_LOW).abs() < 1e-6);
    assert!((t.downgrade_secs - SELF_HEAL_DOWNGRADE_SECS).abs() < 1e-6);
    assert!((t.restore_secs - SELF_HEAL_RESTORE_SECS).abs() < 1e-6);
    assert!((t.health_investigate - SELF_HEAL_HEALTH_INVESTIGATE).abs() < 1e-6);
    assert!((t.health_cooldown_secs - SELF_HEAL_HEALTH_COOLDOWN_SECS).abs() < 1e-6);
}

#[test]
fn self_healer_with_thresholds_overrides_defaults() {
    // Verify the with_thresholds() builder actually replaces the
    // thresholds — a future refactor that breaks the builder would
    // silently leave the defaults in place.
    let mut custom = PowerThresholds::defaults();
    custom.pressure_high = 0.9; // very high — harder to trigger
    custom.downgrade_secs = 5.0; // very short — fires fast

    let h = PerformanceSelfHealer::new().with_thresholds(custom);
    let t = h.thresholds();
    assert!((t.pressure_high - 0.9).abs() < 1e-6);
    assert!((t.downgrade_secs - 5.0).abs() < 1e-6);
    // Unchanged fields stay at defaults.
    assert!((t.pressure_low - SELF_HEAL_PRESSURE_LOW).abs() < 1e-6);
}

#[test]
fn self_healer_respects_overridden_thresholds_in_observe() {
    // End-to-end: with_thresholds() changes must actually change
    // observe() behavior. Use a shorter downgrade window so the
    // test runs fast.
    let mut custom = PowerThresholds::defaults();
    custom.downgrade_secs = 3.0; // 3s instead of 30s

    let mut h = PerformanceSelfHealer::new().with_thresholds(custom);
    let t0 = Instant::now();
    // t=0,1,2 — elapsed < 3.0, no downgrade. With the default 30s
    // window these would also return None, so this part doesn't
    // distinguish the override — the next call does.
    for i in 0..3 {
        let t = t0 + Duration::from_secs(i);
        let action = h.observe(SELF_HEAL_PRESSURE_HIGH, t, Some(95.0));
        assert_eq!(
            action,
            SelfHealAction::None,
            "should not downgrade at t={i}"
        );
    }
    // At t=3, elapsed = 3.0 >= 3.0 (overridden window) → fires.
    // With the default 30s window this would still return None,
    // so this assertion proves the override took effect.
    let action = h.observe(
        SELF_HEAL_PRESSURE_HIGH,
        t0 + Duration::from_secs(3),
        Some(95.0),
    );
    assert_eq!(action, SelfHealAction::DowngradeScene);
    assert!(h.is_downgraded());
}
