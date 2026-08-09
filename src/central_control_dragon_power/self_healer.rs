// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Performance Self-Healer (P1 + P2).
//!
//! Encapsulates two orthogonal auto-mitigation policies as a single state
//! machine so the event loop only needs to call `observe(...)` once per
//! frame and apply the returned [`SelfHealAction`]:
//!
//! - **P1 (auto scene downgrade)** — when `perf_pressure` stays at or above
//!   `PowerThresholds::pressure_high` (0.6) for
//!   `PowerThresholds::downgrade_secs` (30s), switch to the lighter
//!   fallback scene ("low-power") to shed load. When pressure stays at or
//!   below `PowerThresholds::pressure_low` (0.3) for
//!   `PowerThresholds::restore_secs` (60s), restore the prior scene.
//!   Hysteresis gap (0.6 → 0.3) and a middle-band dead zone prevent
//!   flapping under borderline load.
//! - **P2 (EnduranceHealth mitigation)** — when the
//!   [`EnduranceHealth`](crate::central_control_dragon_power::EnduranceHealth)
//!   score drops below `PowerThresholds::health_investigate` (60.0, the
//!   "investigate" band), trigger an immediate frame invalidate + memory
//!   reclaim hint (`madvise(MADV_DONTNEED)`) to clear potential stuck
//!   state. The `PowerThresholds::health_cooldown_secs` (30s) cooldown
//!   prevents a persistently unhealthy process from force-redrawing every
//!   recompute cycle.
//!
//! ## P2 evaluation order
//!
//! P2 (health mitigation) is checked *before* P1 (scene actions) on every
//! tick. Rationale: P2 is a symptom-level response (force redraw +
//! madvise), while P1 is a cause-level response (shed load). If both
//! fire on the same tick, the symptom fix lands first so the next health
//! recompute sees a cleaner state.
//!
//! ## State machine
//!
//! ```text
//!                  ┌─────────────────────┐
//!                  │   Healthy (Normal)  │ ←─ default state
//!                  │ pre_degraded = None │
//!                  └──────────┬──────────┘
//!        sustained high       │       sustained low
//!         pressure (30s)      │       pressure (60s)
//!               ▼             │             ▼
//!   ┌───────────────────┐     │     ┌───────────────────┐
//!   │  Downgraded       │◄────┴────►│  Healthy          │
//!   │  pre_degraded=Some│           │  pre_degraded=None│
//!   └───────────────────┘           └───────────────────┘
//! ```
//!
//! P2 (health mitigation) is orthogonal — it can fire from either the
//! Healthy or Downgraded state and does not change the P1 state.
//!
//! The struct is intentionally tiny (5 fields, all `Option`/scalar) so it
//! lives entirely in cache and costs nothing when [`SelfHealAction::None`]
//! is returned (the common case). All subsystems here are zero-allocation,
//! single-threaded, and backward-compatible with the existing architecture
//! invariants.

use std::time::Instant;

use crate::constants::*;

/// Actions the self-healer may request from the event loop.
///
/// The self-healer is a *pure policy* — it does not touch `Cloud`, `Frame`,
/// or stdout directly. It returns an action enum, and the event loop applies
/// it. This keeps the side-effect surface testable in isolation and lets
/// the event loop batch/defer actions as needed (e.g., skip a downgrade
/// when the user is in fixed mode or has explicitly chosen a scene).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelfHealAction {
    /// No action this tick. Steady state.
    None,
    /// P1: switch to the lighter fallback scene (e.g. "low-power").
    /// The event loop saves the current scene name and applies the fallback.
    DowngradeScene,
    /// P1: restore the scene that was active before the last downgrade.
    RestoreScene,
    /// P2: EnduranceHealth dropped into the investigate band. Force a full
    /// redraw and bypass the ReclaimState cooldown to issue an madvise hint.
    /// The event loop calls `cloud.force_draw_everything()` and
    /// `hint_reclaim_pages()` directly.
    TriggerHealthMitigation,
}

/// Performance self-healer — encapsulates P1 (auto scene downgrade) and
/// P2 (EnduranceHealth-triggered mitigation) as a single state machine.
///
/// See the module-level docs for the state machine diagram and the P2
/// evaluation order rationale.
///
/// v30.9: thresholds migrated from standalone `SELF_HEAL_*` constants to
/// a `PowerThresholds` instance. The struct is now the sole source of
/// truth; the standalone constants have been removed from `mod.rs`.
#[derive(Debug, Clone)]
pub(crate) struct PerformanceSelfHealer {
    /// Tunable thresholds. Owned by value because `PowerThresholds` is
    /// `Copy` — no allocation, no indirection. Constructed with
    /// `PowerThresholds::defaults()`; tests can override via
    /// [`with_thresholds`](Self::with_thresholds).
    thresholds: PowerThresholds,
    /// When sustained-high-pressure accumulation started. `None` when not
    /// currently accumulating. Reset to `None` on any low-pressure sample
    /// (hysteresis — a single cool frame breaks the streak).
    high_pressure_since: Option<Instant>,
    /// When sustained-low-pressure recovery started. Only meaningful when
    /// currently downgraded.
    low_pressure_since: Option<Instant>,
    /// The scene name captured at the moment of downgrade, to be restored
    /// when pressure recovers. `None` when not downgraded.
    pre_degraded_scene: Option<String>,
    /// Whether we are currently in the downgraded state.
    is_downgraded: bool,
    /// When the last health mitigation was triggered. Used for the
    /// cooldown window.
    last_health_mitigation: Option<Instant>,
}

impl PerformanceSelfHealer {
    /// Fallback scene applied on downgrade. Hardcoded to "low-power" —
    /// the built-in scene specifically designed for low-CPU operation
    /// (speed=5, density=0.45, glitch_level=None). Exposed as a constant
    /// so tests and the event loop can reference it without magic strings.
    ///
    /// **v35.2 audit note (FPS-F2)**: the "low-power" scene's `fps=30`
    /// field is **startup-only by design** — `Cloud::apply_scene_runtime`
    /// does NOT apply `fps` at runtime, only `speed`/`density`/`color`/
    /// `charset`/`glitch_level`. So the CPU shed from a downgrade comes
    /// from the lower speed/density/glitch, NOT from a runtime FPS drop.
    /// This is intentional: letting the self-healer override the user's
    /// `--fps` would create a precedence ambiguity. The runtime idle
    /// factor and pause period remain the only runtime FPS modifiers.
    pub(crate) const FALLBACK_SCENE: &'static str = "low-power";

    pub(crate) fn new() -> Self {
        Self {
            thresholds: PowerThresholds::defaults(),
            high_pressure_since: None,
            low_pressure_since: None,
            pre_degraded_scene: None,
            is_downgraded: false,
            last_health_mitigation: None,
        }
    }

    /// Override the default thresholds. Test-only — production code uses
    /// `PowerThresholds::defaults()` (which mirrors the former standalone
    /// constants).
    #[cfg(test)]
    pub(crate) fn with_thresholds(mut self, thresholds: PowerThresholds) -> Self {
        self.thresholds = thresholds;
        self
    }

    /// Read-only access to the thresholds. Test-only — production code
    /// interacts with the healer through `observe()` + `record_downgrade()`
    /// + `take_pre_degraded_scene()` + `reset()`.
    #[cfg(test)]
    pub(crate) fn thresholds(&self) -> PowerThresholds {
        self.thresholds
    }

    /// Observe the current `perf_pressure` and elapsed wall-clock time,
    /// returning the action the event loop should take this tick.
    ///
    /// `now` is passed in (rather than read via `Instant::now()`) so the
    /// function is deterministic and testable with synthetic clocks.
    /// `health_score` is the latest `EnduranceHealth::score()` value, or
    /// `None` if the health tracker hasn't accumulated enough samples yet.
    ///
    /// ## P2 evaluation order
    ///
    /// Health mitigation is checked *before* P1 scene actions. Rationale:
    /// health mitigation is a symptom-level response (force redraw +
    /// madvise), while P1 is a cause-level response (shed load). If both
    /// fire on the same tick, we want the symptom fix to land first so
    /// the next health recompute sees a cleaner state.
    pub(crate) fn observe(
        &mut self,
        perf_pressure: f32,
        now: Instant,
        health_score: Option<f64>,
    ) -> SelfHealAction {
        // ── P2: health mitigation (orthogonal to P1 state) ──
        if let Some(score) = health_score {
            if score < self.thresholds.health_investigate {
                let cooldown_ok = match self.last_health_mitigation {
                    None => true,
                    Some(last) => {
                        now.saturating_duration_since(last).as_secs_f64()
                            >= self.thresholds.health_cooldown_secs
                    }
                };
                if cooldown_ok {
                    self.last_health_mitigation = Some(now);
                    return SelfHealAction::TriggerHealthMitigation;
                }
            }
        }

        // ── P1: scene downgrade / restore ──
        if perf_pressure >= self.thresholds.pressure_high {
            // Pressure is high — accumulate (or start) the high streak.
            if self.high_pressure_since.is_none() {
                self.high_pressure_since = Some(now);
            }
            // Any high-pressure frame breaks the low-pressure recovery streak.
            self.low_pressure_since = None;

            if !self.is_downgraded {
                let since = self.high_pressure_since.unwrap_or(now);
                let elapsed = now.saturating_duration_since(since).as_secs_f64();
                if elapsed >= self.thresholds.downgrade_secs {
                    // Fire downgrade — the event loop will fill pre_degraded_scene
                    // via record_downgrade() once it has applied the scene switch.
                    self.is_downgraded = true;
                    return SelfHealAction::DowngradeScene;
                }
            }
        } else if perf_pressure <= self.thresholds.pressure_low {
            // Pressure is low — accumulate (or start) the recovery streak.
            if self.low_pressure_since.is_none() {
                self.low_pressure_since = Some(now);
            }
            // Any low-pressure frame breaks the high-pressure accumulation streak.
            self.high_pressure_since = None;

            if self.is_downgraded {
                let since = self.low_pressure_since.unwrap_or(now);
                let elapsed = now.saturating_duration_since(since).as_secs_f64();
                if elapsed >= self.thresholds.restore_secs {
                    self.is_downgraded = false;
                    // Clear streaks so a fresh downgrade requires a full new window.
                    self.high_pressure_since = None;
                    self.low_pressure_since = None;
                    return SelfHealAction::RestoreScene;
                }
            }
        } else {
            // Middle band (LOW < pressure < HIGH) — hysteresis dead zone.
            // Neither streak accumulates. This is the deliberate "do nothing"
            // band that prevents flapping under borderline load.
        }

        SelfHealAction::None
    }

    /// Called by the event loop *after* applying a `DowngradeScene` action,
    /// to record the scene name that should be restored later. The caller
    /// passes the scene name that was active immediately before the switch.
    pub(crate) fn record_downgrade(&mut self, prior_scene: &str) {
        self.pre_degraded_scene = Some(prior_scene.to_string());
    }

    /// Called by the event loop *after* applying a `RestoreScene` action,
    /// to clear the saved scene. Returns the scene name to restore, or
    /// `None` if no prior scene was recorded (defensive — should not happen
    /// if the state machine is wired correctly).
    pub(crate) fn take_pre_degraded_scene(&mut self) -> Option<String> {
        self.pre_degraded_scene.take()
    }

    /// Whether the healer is currently in the downgraded state. Used by
    /// the event loop to avoid double-applying downgrades and to skip
    /// user-initiated scene changes while downgraded (the user's choice
    /// wins; we clear the downgrade state).
    #[cfg(test)]
    pub(crate) fn is_downgraded(&self) -> bool {
        self.is_downgraded
    }

    /// Reset all state. Called when the user manually switches scenes (their
    /// choice should override any auto-downgrade in flight) or when the
    /// Cloud is rebuilt from a live config reload.
    pub(crate) fn reset(&mut self) {
        self.high_pressure_since = None;
        self.low_pressure_since = None;
        self.pre_degraded_scene = None;
        self.is_downgraded = false;
        // Intentionally do NOT reset last_health_mitigation — the cooldown
        // should persist across scene changes to prevent abuse.
    }
}

impl Default for PerformanceSelfHealer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
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
        // v35.3 (FPS-F7): comment corrected — fps=30 was removed in v35.2
        // (scene fps is startup-only by design; the CPU shed comes from
        // speed/density/glitch, not from a runtime fps drop).
        assert_eq!(PerformanceSelfHealer::FALLBACK_SCENE, "low-power");
    }

    #[test]
    fn self_healer_loads_default_thresholds_at_construction() {
        // v30.9: the healer must construct with PowerThresholds::defaults()
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
}
