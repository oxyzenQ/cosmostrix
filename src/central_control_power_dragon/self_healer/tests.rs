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
    // 30s downgrade window. Should NOT fire DowngradeScene.
    // Dragon Engine v2: PreemptiveThrottle MAY fire (it's lighter than
    // DowngradeScene — just sets aggressive_throttle, no scene change).
    // The test only asserts no DOWNGRADE happens before the window.
    for i in 0..29 {
        let t = t0 + Duration::from_secs(i);
        let action = h.observe(SELF_HEAL_PRESSURE_HIGH, t, Some(95.0));
        assert_ne!(
            action,
            SelfHealAction::DowngradeScene,
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
        assert_ne!(
            action,
            SelfHealAction::DowngradeScene,
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

// ── Dragon Engine v2 depth-verify: predictive EMA throttle ──────────────
//
// The v2 merge (d55442d) shipped the EMA trend predictor + PreemptiveThrottle
// action but ZERO tests for the predictive path (all 18 existing tests cover
// the reactive P1/P2 paths). These tests are the missing proof that the
// "heals before it breaks" predictor is real and working.

/// A steep sustained spike (0.1 -> 0.8 held) fires the predictor within
/// three observations — long before the 30 s reactive downgrade window.
/// This is the predictor's designed trigger window: instant load spikes
/// (delta > 0.167/tick => EMA trend > 0.05/tick).
#[test]
fn self_healer_v2_preemptive_throttle_fires_on_steep_spike() {
    let mut h = PerformanceSelfHealer::new();
    let t0 = Instant::now();
    let mut fired = false;
    // Baseline idle frame, then the spike lands and stays.
    for (i, p) in [0.1f32, 0.8, 0.8, 0.8, 0.8, 0.8].iter().enumerate() {
        let t = t0 + Duration::from_secs(i as u64);
        let action = h.observe(*p, t, None);
        assert_ne!(
            action,
            SelfHealAction::DowngradeScene,
            "reactive downgrade must not fire — 30s window not elapsed"
        );
        if action == SelfHealAction::PreemptiveThrottle {
            fired = true;
            break;
        }
    }
    assert!(
        fired,
        "a sustained steep spike must fire PreemptiveThrottle within a few frames"
    );
    assert!(!h.is_downgraded());
    assert!(h.preemptive_throttle_active);
}

/// A gradual ramp (delta 0.05/tick) never crosses the trend threshold
/// (0.3 * 0.05 = 0.015 < 0.05) — pinned here as the documented noise
/// filter contract: the predictor targets instant spikes only, gradual
/// load belongs to the reactive P1 path.
#[test]
fn self_healer_v2_gradual_ramp_is_noise_filtered_by_design() {
    let mut h = PerformanceSelfHealer::new();
    let t0 = Instant::now();
    // 0.10 -> 0.50 over 9 ticks: a realistic gradual load ramp.
    for i in 0..9 {
        let p = 0.10 + 0.05 * i as f32;
        let t = t0 + Duration::from_secs(i);
        let action = h.observe(p, t, None);
        assert_ne!(
            action,
            SelfHealAction::PreemptiveThrottle,
            "gradual ramp (0.05/tick) is below the trend threshold — noise filter by design"
        );
    }
    assert!(!h.preemptive_throttle_active);
}

/// Constant mid-band pressure converges: the trend decays to ~0 before the
/// EMA crosses pressure_low, so the predictor must stay silent. A gradual
/// load change is not a spike — no pre-throttling.
#[test]
fn self_healer_v2_steady_mid_band_pressure_never_fires_preemptive() {
    let mut h = PerformanceSelfHealer::new();
    let t0 = Instant::now();
    for i in 0..50 {
        let t = t0 + Duration::from_secs(i);
        let action = h.observe(0.35, t, None);
        assert_ne!(
            action,
            SelfHealAction::PreemptiveThrottle,
            "steady 0.35 must not fire the predictor (trend converges to 0)"
        );
    }
    assert!(!h.preemptive_throttle_active);
}

/// A single hard spike from idle (0.1 -> 0.8 -> 0.05) jumps the EMA by 0.231
/// but the EMA (0.261) is still below pressure_low, inside the "no point
/// throttling at idle" filter — the predictor must NOT fire.
#[test]
fn self_healer_v2_single_spike_from_idle_does_not_fire() {
    let mut h = PerformanceSelfHealer::new();
    let t0 = Instant::now();
    for (i, p) in [0.1f32, 0.8, 0.05].iter().enumerate() {
        let t = t0 + Duration::from_secs(i as u64);
        let action = h.observe(*p, t, None);
        assert_ne!(
            action,
            SelfHealAction::PreemptiveThrottle,
            "single spike from idle must be filtered by the warning-zone gate"
        );
        assert_ne!(
            action,
            SelfHealAction::DowngradeScene,
            "one-second spike must not downgrade (30s window)"
        );
    }
    assert!(!h.preemptive_throttle_active);
}

/// After firing, pressure recovery (EMA <= pressure_low) must clear the
/// preemptive flag so the next rising trend can fire again.
#[test]
fn self_healer_v2_preemptive_clears_when_pressure_recovers() {
    let mut h = PerformanceSelfHealer::new();
    let t0 = Instant::now();
    // Fire it via a steep sustained spike.
    let mut fired = false;
    for (i, p) in [0.1f32, 0.8, 0.8, 0.8, 0.8, 0.8].iter().enumerate() {
        let action = h.observe(*p, t0 + Duration::from_secs(i as u64), None);
        if action == SelfHealAction::PreemptiveThrottle {
            fired = true;
            break;
        }
    }
    assert!(fired, "precondition: predictor must fire on the spike");
    assert!(h.preemptive_throttle_active);

    // Recover: idle pressure pulls the EMA below pressure_low within a
    // few observations (0.42 * 0.7^n + 0.05 -> below 0.3 by n=2).
    let mut cleared = false;
    for i in 0..10 {
        let action = h.observe(0.05, t0 + Duration::from_secs(20 + i), None);
        assert_ne!(action, SelfHealAction::PreemptiveThrottle);
        if !h.preemptive_throttle_active {
            cleared = true;
            break;
        }
    }
    assert!(
        cleared,
        "recovery below pressure_low must clear the preemptive flag"
    );
}

/// Regression for the v2 reset gap: reset() (scene switch / config rebuild)
/// must clear the EMA trend and the preemptive flag — otherwise a phantom
/// trend carried over from the previous scene could pre-throttle the next
/// one, and a stale active flag would suppress re-firing.
#[test]
fn self_healer_reset_clears_v2_predictive_state() {
    let mut h = PerformanceSelfHealer::new();
    let t0 = Instant::now();
    let mut fired = false;
    for (i, p) in [0.1f32, 0.8, 0.8, 0.8, 0.8, 0.8].iter().enumerate() {
        let action = h.observe(*p, t0 + Duration::from_secs(i as u64), None);
        if action == SelfHealAction::PreemptiveThrottle {
            fired = true;
            break;
        }
    }
    assert!(fired, "precondition: predictor must fire on the spike");
    assert!(h.pressure_ema > 0.0);
    assert!(h.preemptive_throttle_active);

    h.reset();
    assert_eq!(h.pressure_ema, 0.0, "reset must zero the EMA");
    assert_eq!(h.pressure_ema_prev, 0.0, "reset must zero the EMA history");
    assert!(
        !h.preemptive_throttle_active,
        "reset must clear the preemptive flag"
    );
}
