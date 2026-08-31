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
    /// Dragon Engine v2: predictive throttle. The EMA trend shows pressure
    /// rising rapidly — activate aggressive_throttle BEFORE the reactive
    /// downgrade threshold is hit. The event loop sets
    /// `cloud.aggressive_throttle = true` (same as DowngradeScene but
    /// earlier + lighter — no scene change, just steeper spawn-scale curve).
    PreemptiveThrottle,
}

/// Performance self-healer — encapsulates P1 (auto scene downgrade) and
/// P2 (EnduranceHealth-triggered mitigation) as a single state machine.
///
/// See the module-level docs for the state machine diagram and the P2
/// evaluation order rationale.
///
/// the healer now reads thresholds from a `PowerThresholds`
/// instance (constructed via `PowerThresholds::defaults()`). The
/// standalone constants in `mod.rs` remain as the canonical values that
/// `defaults()` copies.
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
    // ── Dragon Engine v2: predictive trend tracking ──
    /// EMA of perf_pressure (alpha 0.3 — responsive but smoothed).
    /// Tracks the recent pressure trend so the healer can predict
    /// spikes BEFORE they hit the reactive downgrade threshold.
    pressure_ema: f32,
    /// Previous EMA value (used to compute the slope/trend).
    pressure_ema_prev: f32,
    /// Whether preemptive throttle is currently active (avoids re-firing
    /// every tick — once activated, stays until pressure drops or the
    /// reactive DowngradeScene fires).
    preemptive_throttle_active: bool,
}

impl PerformanceSelfHealer {
    /// AB-11 (dragon power audit, option 2): this constant is retained for
    /// reference but NO LONGER USED for scene switching. The old design
    /// called `cloud.apply_scene_runtime("low-power")` on sustained high
    /// CPU pressure, which silently overrode the user's color, charset,
    /// density, speed, and glitch_level — violating the owner's principle
    /// that dragon power must not change visual identity.
    ///
    /// The new design sets `cloud.aggressive_throttle = true` instead,
    /// which only affects the spawn-scale formula (steeper curve + lower
    /// floor) and disables glitches — the user's visual settings are never
    /// touched. This constant is kept so the audit trail is visible and
    /// future contributors understand what the old behavior was.
    #[cfg(test)]
    pub(crate) const FALLBACK_SCENE: &'static str = "low-power";

    pub(crate) fn new() -> Self {
        Self {
            thresholds: PowerThresholds::defaults(),
            high_pressure_since: None,
            low_pressure_since: None,
            pre_degraded_scene: None,
            is_downgraded: false,
            last_health_mitigation: None,
            pressure_ema: 0.0,
            pressure_ema_prev: 0.0,
            preemptive_throttle_active: false,
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
        // ── Dragon Engine v2: update EMA trend ──
        // Alpha 0.3 — responsive enough to catch real spikes, smoothed
        // enough to ignore single-frame jitter.
        const PREDICTIVE_EMA_ALPHA: f32 = 0.3;
        self.pressure_ema_prev = self.pressure_ema;
        self.pressure_ema = self.pressure_ema * (1.0 - PREDICTIVE_EMA_ALPHA)
            + perf_pressure * PREDICTIVE_EMA_ALPHA;
        let trend = self.pressure_ema - self.pressure_ema_prev;

        // ── Dragon Engine v2: predictive preemptive throttle ──
        // If pressure is rising rapidly (trend > 0.05 per tick) AND we're
        // not already downgraded AND pressure is in the "warning zone"
        // (above pressure_low but below pressure_high), fire preemptive
        // throttle BEFORE the reactive downgrade threshold is hit.
        //
        // Threshold rationale: trend 0.05/tick at ~60Hz = 3.0/sec pressure
        // increase. That's a fast spike — typical gradual load changes are
        // <0.01/tick. 0.05 filters out noise, catches real spikes.
        //
        // The "warning zone" gate (pressure_low < ema < pressure_high)
        // prevents firing when we're already idle (no point throttling at
        // 0% pressure) or already at downgrade threshold (the reactive P1
        // path handles that).
        if !self.is_downgraded
            && !self.preemptive_throttle_active
            && trend > 0.05
            && self.pressure_ema > self.thresholds.pressure_low
            && self.pressure_ema < self.thresholds.pressure_high
        {
            self.preemptive_throttle_active = true;
            return SelfHealAction::PreemptiveThrottle;
        }
        // Clear preemptive throttle when pressure drops or reactive
        // downgrade fires (the reactive path is stronger).
        if self.preemptive_throttle_active
            && (self.pressure_ema <= self.thresholds.pressure_low || self.is_downgraded)
        {
            self.preemptive_throttle_active = false;
        }

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
mod tests;
