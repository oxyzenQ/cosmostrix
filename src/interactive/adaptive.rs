// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Adaptive learning subsystem for long-endurance stability.
//!
//! This module implements five improvements derived from 72-hour endurance
//! telemetry analysis:
//!
//! - **P1: Phase-Aware Adaptive Pacing (PAP)** — Learns the daily activity
//!   cycle and proactively transitions to idle mode before the reactive
//!   30-second threshold fires.
//! - **P2: Idle Phase Aggressive Coalescing (IPAC)** — Progressively stretches
//!   the idle resync interval after sustained inactivity to reduce forced
//!   redraw CPU spikes.
//! - **P4: Memory Pressure Adaptive Reclaim (MPAR)** — Hints the kernel to
//!   reclaim stale frame buffer pages during idle, smoothing RSS step-downs.
//! - **P5: Endurance Health Score (EHS)** — A single 0–100 metric tracking
//!   memory stability, frame jitter, and context switch rate.
//!
//! P3 (Context Switch Batching) is handled at the Terminal level via its
//! existing BufWriter; no additional code is needed here.
//!
//! All subsystems are zero-allocation, single-threaded, and backward-compatible
//! with the existing architecture invariants.

use std::time::{Duration, Instant};

use crate::constants::*;

// ────────────────────────────────────────────────────────────────────────────
// P1: Phase-Aware Adaptive Pacing
// ────────────────────────────────────────────────────────────────────────────

/// Phase predictor based on historical activity patterns.
///
/// Uses exponential moving average (EMA) of activity transition times
/// (seconds since midnight) to predict whether the process should be in
/// active or idle mode. After observing ≥2 full cycles, the predictor
/// can proactively suggest idle mode before the reactive 30-second threshold.
///
/// The predictor is intentionally simple: a single EMA per transition
/// boundary. This avoids per-second histograms that would consume memory
/// and add complexity for marginal accuracy gains.
#[derive(Debug, Clone)]
pub(crate) struct PhasePredictor {
    /// EMA of active-phase start time (seconds since local midnight).
    active_start_ema: f64,
    /// EMA of active-phase end time (seconds since local midnight).
    active_end_ema: f64,
    /// Number of transitions recorded.
    transitions_observed: u64,
    /// Learning rate (alpha) for EMA updates.
    alpha: f64,
}

impl PhasePredictor {
    /// Create a new predictor with default learning rate.
    pub(crate) fn new() -> Self {
        Self {
            active_start_ema: 0.0,
            active_end_ema: 0.0,
            transitions_observed: 0,
            alpha: 0.3,
        }
    }

    /// Record a phase transition.
    ///
    /// # Arguments
    /// - `to_active`: `true` if transitioning idle→active, `false` if active→idle.
    /// - `secs_since_midnight`: Local wall-clock seconds since midnight (0–86400).
    pub(crate) fn record_transition(&mut self, to_active: bool, secs_since_midnight: f64) {
        let t = secs_since_midnight.rem_euclid(86400.0);
        if to_active {
            self.active_start_ema = if self.transitions_observed == 0 {
                t
            } else {
                self.alpha * t + (1.0 - self.alpha) * self.active_start_ema
            };
        } else {
            self.active_end_ema = if self.transitions_observed == 0 {
                t
            } else {
                self.alpha * t + (1.0 - self.alpha) * self.active_end_ema
            };
        }
        self.transitions_observed = self.transitions_observed.saturating_add(1);
    }

    /// Predict whether the process should be in active mode.
    ///
    /// Returns `Some(true)` if active is predicted, `Some(false)` if idle is
    /// predicted, or `None` if insufficient data (< 2 transitions).
    pub(crate) fn predicts_active(&self, secs_since_midnight: f64) -> Option<bool> {
        if self.transitions_observed < 2 {
            return None;
        }
        let t = secs_since_midnight.rem_euclid(86400.0);
        // Handle wrap-around: active phase may cross midnight.
        if self.active_start_ema <= self.active_end_ema {
            // Normal: active period doesn't cross midnight.
            Some(t >= self.active_start_ema && t < self.active_end_ema)
        } else {
            // Wrap-around: active period crosses midnight.
            Some(t >= self.active_start_ema || t < self.active_end_ema)
        }
    }

    /// Number of transitions observed so far.
    pub(crate) fn transitions_observed(&self) -> u64 {
        self.transitions_observed
    }
}

impl Default for PhasePredictor {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute seconds since local midnight from a `SystemTime` instant.
///
/// Uses `chrono`-free arithmetic: extracts hour/minute/second from the
/// local time offset. Since cosmostrix doesn't depend on chrono, we use
/// a simple approach: the event loop tracks `Instant`-based elapsed time,
/// and the caller provides the local time-of-day in seconds.
///
/// In practice, the event loop calls this with `local_secs()` which
/// reads `/etc/localtime` via libc `localtime_r`. For environments without
/// timezone support, falls back to 0.0 (predictions start from midnight).
#[cfg(target_os = "linux")]
pub(crate) fn local_secs_since_midnight() -> f64 {
    use std::mem::MaybeUninit;
    // SAFETY: libc::time(NULL) is the documented POSIX call — writes nothing
    // when the pointer is NULL, returns time_t or -1 on error. No preconditions.
    let now = unsafe { libc::time(std::ptr::null_mut()) };
    if now < 0 {
        return 0.0;
    }
    let mut tm: MaybeUninit<libc::tm> = MaybeUninit::uninit();
    let tm_ptr = tm.as_mut_ptr();
    // SAFETY: localtime_r is the thread-safe POSIX variant. It reads `now`
    // (a valid time_t value, checked >= 0 above) and writes into our
    // MaybeUninit<tm> buffer. Returns NULL on failure (handled below).
    if unsafe { libc::localtime_r(&now, tm_ptr) }.is_null() {
        return 0.0;
    }
    // SAFETY: localtime_r returned non-NULL, which per POSIX means the tm
    // struct has been fully initialized. assume_init() is now sound.
    let tm = unsafe { tm.assume_init() };
    (tm.tm_hour as f64 * 3600.0) + (tm.tm_min as f64 * 60.0) + tm.tm_sec as f64
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn local_secs_since_midnight() -> f64 {
    // Fallback: use UTC seconds. The predictor still works, just in UTC.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    secs.rem_euclid(86400.0)
}

// ────────────────────────────────────────────────────────────────────────────
// P2: Idle Phase Aggressive Coalescing
// ────────────────────────────────────────────────────────────────────────────

/// Adaptive resync interval: progressively stretches the idle redraw resync
/// interval based on sustained idle duration.
///
/// After 1 hour of continuous idle, the interval grows from 20s → 60s.
/// After 4 hours, it grows to 120s. This reduces forced redraw CPU spikes
/// during long idle periods (typically 13+ hours per day in long-endurance runs).
///
/// # Arguments
/// - `idle_duration_secs`: How long the process has been continuously idle.
///
/// # Returns
/// The resync interval in seconds.
///
/// v25.15 (perf audit): the tier thresholds and intervals are now named
/// constants in `constants.rs` (`SECS_PER_HOUR`, `SECS_PER_4_HOURS`,
/// `IDLE_RESYNC_TIER_2_SECS`, `IDLE_RESYNC_TIER_3_SECS`). Previously these
/// were four magic numbers inline.
pub(crate) fn adaptive_resync_interval(idle_duration_secs: f64) -> f64 {
    if idle_duration_secs < SECS_PER_HOUR {
        // < 1 hour idle: standard interval (20s).
        IDLE_REDRAW_RESYNC_INTERVAL_SECS
    } else if idle_duration_secs < SECS_PER_4_HOURS {
        // 1–4 hours idle: 60s interval (3× reduction).
        IDLE_RESYNC_TIER_2_SECS
    } else {
        // > 4 hours idle: 120s interval (6× reduction).
        IDLE_RESYNC_TIER_3_SECS
    }
}

// ────────────────────────────────────────────────────────────────────────────
// P4: Memory Pressure Adaptive Reclaim
// ────────────────────────────────────────────────────────────────────────────

/// Hint the Linux kernel to reclaim stale file-backed pages.
///
/// During sustained idle periods, the frame buffer's previous-generation
/// dirty regions are no longer needed. `madvise(MADV_DONTNEED)` tells the
/// kernel these pages can be reclaimed without swapping — they'll be
/// zero-filled on next access.
///
/// This smooths the RSS step-down that the kernel would otherwise perform
/// as a sudden event during memory pressure.
///
/// # Safety
/// This function is only effective on Linux. On other platforms it's a no-op.
/// The caller must ensure `ptr` points to a mapped region of at least `len`
/// bytes.
#[cfg(target_os = "linux")]
pub(crate) unsafe fn hint_reclaim_pages(ptr: *const u8, len: usize) {
    if len == 0 || ptr.is_null() {
        return;
    }
    // MADV_DONTNEED = 4 on Linux
    let ret = libc::madvise(ptr as *mut libc::c_void, len, libc::MADV_DONTNEED);
    // Ignore EINVAL/EINVAL (pages not reclaimable) — best-effort.
    let _ = ret;
}

#[cfg(not(target_os = "linux"))]
pub(crate) unsafe fn hint_reclaim_pages(_ptr: *const u8, _len: usize) {
    // No-op on non-Linux platforms.
}

/// Track whether memory reclaim has been performed recently to avoid
/// hammering madvise on every idle resync.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ReclaimState {
    /// When the last reclaim hint was issued.
    last_reclaim: Option<Instant>,
    /// Minimum interval between reclaim hints (1 hour).
    min_interval: Duration,
}

impl ReclaimState {
    pub(crate) fn new() -> Self {
        Self {
            last_reclaim: None,
            min_interval: Duration::from_secs(3600),
        }
    }

    /// Returns `true` if a reclaim hint should be issued now.
    pub(crate) fn should_reclaim(&self, now: Instant) -> bool {
        match self.last_reclaim {
            None => true,
            Some(last) => now.saturating_duration_since(last) >= self.min_interval,
        }
    }

    /// Record that a reclaim hint was issued at `now`.
    pub(crate) fn mark_reclaimed(&mut self, now: Instant) {
        self.last_reclaim = Some(now);
    }
}

impl Default for ReclaimState {
    fn default() -> Self {
        Self::new()
    }
}

// ────────────────────────────────────────────────────────────────────────────
// P5: Endurance Health Score
// ────────────────────────────────────────────────────────────────────────────

/// Endurance Health Score: a 0–100 metric based on:
/// - Memory stability (RSS variance over recent samples)
/// - Frame jitter (rolling average frame time)
/// - Context switch rate (voluntary switches per second)
///
/// The score is designed to be a single number operators can monitor.
/// A score > 80 means healthy; 60–80 means degraded; < 60 means investigate.
#[derive(Debug, Clone)]
pub(crate) struct EnduranceHealth {
    /// Ring buffer of recent RSS readings (KB).
    rss_samples: [f64; 60],
    /// Write cursor for `rss_samples` (free-running modulo 60).
    ///
    /// Only ever used inside `push_rss`, which is `#[cfg(target_os = "linux")]`
    /// because it reads `/proc/self/status`. On FreeBSD/macOS/Windows the
    /// `push_rss` method is cfg'd out, leaving this field with zero uses —
    /// which would trip `-D warnings` under the project's clippy config.
    /// The cfg_attr suppresses the dead-code lint only on non-Linux
    /// platforms; on Linux the field is still flagged normally if it ever
    /// becomes truly unused.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    rss_idx: usize,
    rss_count: usize,
    /// EMA of frame time (ms).
    frame_jitter_ema: f64,
    /// EMA of context switch rate (switches/sec).
    ctxt_switch_ema: f64,
    /// Last computed score.
    score: f64,
    /// Number of updates received.
    updates: u64,
}

impl EnduranceHealth {
    /// Number of RSS samples required before the score is meaningful.
    const MIN_SAMPLES: usize = 5;

    pub(crate) fn new() -> Self {
        Self {
            rss_samples: [0.0; 60],
            rss_idx: 0,
            rss_count: 0,
            frame_jitter_ema: 0.0,
            ctxt_switch_ema: 0.0,
            score: 100.0,
            updates: 0,
        }
    }

    /// Push a new RSS reading (KB).
    ///
    /// Only called on Linux (reads /proc/self/status). Cfg-gated to avoid
    /// dead_code warnings on FreeBSD/macOS/Windows where RSS sampling is
    /// not implemented.
    #[cfg(target_os = "linux")]
    pub(crate) fn push_rss(&mut self, rss_kb: f64) {
        self.rss_samples[self.rss_idx] = rss_kb;
        self.rss_idx = (self.rss_idx + 1) % 60;
        if self.rss_count < 60 {
            self.rss_count += 1;
        }
    }

    /// Update frame jitter EMA. `frame_time_ms` is the latest frame time in ms.
    pub(crate) fn push_frame_time(&mut self, frame_time_ms: f64) {
        if self.updates == 0 {
            self.frame_jitter_ema = frame_time_ms;
        } else {
            self.frame_jitter_ema = 0.95 * self.frame_jitter_ema + 0.05 * frame_time_ms;
        }
    }

    /// Update context switch rate EMA. `switches_per_sec` is the current rate.
    ///
    /// Only called on Linux (reads /proc/self/stat for voluntary ctxt switches).
    /// Cfg-gated to avoid dead_code warnings on non-Linux platforms.
    #[cfg(target_os = "linux")]
    pub(crate) fn push_ctxt_rate(&mut self, switches_per_sec: f64) {
        if self.updates == 0 {
            self.ctxt_switch_ema = switches_per_sec;
        } else {
            self.ctxt_switch_ema = 0.95 * self.ctxt_switch_ema + 0.05 * switches_per_sec;
        }
    }

    /// Recompute the health score. Called after pushing new samples.
    pub(crate) fn recompute(&mut self) {
        self.updates = self.updates.saturating_add(1);
        if self.rss_count < Self::MIN_SAMPLES {
            // Not enough data yet — assume healthy.
            self.score = 100.0;
            return;
        }

        // RSS stability: lower variance = higher score.
        // Typical RSS range for cosmostrix: 2796–3044 KB (Δ ~250 KB).
        // A variance of 0 → score 100. Variance of 10000 (100 KB²) → score 0.
        let mean = self.rss_mean();
        let var = self.rss_variance(mean);
        let rss_score = (100.0 - (var * 0.1)).clamp(0.0, 100.0);

        // Frame jitter score: lower jitter = higher score.
        // Typical: 0.1–2.0 ms. Score = 100 - jitter*10.
        let jitter_score = (100.0 - self.frame_jitter_ema * 10.0).clamp(0.0, 100.0);

        // Context switch score: lower rate = higher score.
        // Typical: 40–80 switches/sec. Score = 100 - rate*0.5.
        let ctxt_score = (100.0 - self.ctxt_switch_ema * 0.5).clamp(0.0, 100.0);

        // Weighted average: memory 40%, jitter 35%, context switches 25%.
        self.score = rss_score * 0.4 + jitter_score * 0.35 + ctxt_score * 0.25;
    }

    /// Current health score (0–100).
    pub(crate) fn score(&self) -> f64 {
        self.score
    }

    /// Human-readable classification.
    pub(crate) fn classification(&self) -> &'static str {
        if self.score >= 80.0 {
            "healthy"
        } else if self.score >= 60.0 {
            "degraded"
        } else {
            "investigate"
        }
    }

    fn rss_mean(&self) -> f64 {
        if self.rss_count == 0 {
            return 0.0;
        }
        let sum: f64 = self.rss_samples[..self.rss_count].iter().sum();
        sum / self.rss_count as f64
    }

    fn rss_variance(&self, mean: f64) -> f64 {
        if self.rss_count < 2 {
            return 0.0;
        }
        let sum_sq: f64 = self.rss_samples[..self.rss_count]
            .iter()
            .map(|&v| (v - mean) * (v - mean))
            .sum();
        sum_sq / self.rss_count as f64
    }
}

impl Default for EnduranceHealth {
    fn default() -> Self {
        Self::new()
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Performance Self-Healer (P1 + P2)
// ────────────────────────────────────────────────────────────────────────────

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
/// The struct is intentionally tiny (3 fields, all `Option`/scalar) so it
/// lives entirely in cache and costs nothing when `SelfHealAction::None`
/// is returned (the common case).
///
/// ## State machine
///
/// ```text
///                  ┌─────────────────────┐
///                  │   Healthy (Normal)  │ ←─ default state
///                  │ pre_degraded = None │
///                  └──────────┬──────────┘
///        sustained high       │       sustained low
///         pressure (30s)      │       pressure (60s)
///               ▼             │             ▼
///   ┌───────────────────┐     │     ┌───────────────────┐
///   │  Downgraded       │◄────┴────►│  Healthy          │
///   │  pre_degraded=Some│           │  pre_degraded=None│
///   └───────────────────┘           └───────────────────┘
/// ```
///
/// P2 (health mitigation) is orthogonal — it can fire from either the
/// Healthy or Downgraded state and does not change the P1 state. The
/// cooldown prevents it from firing more than once per
/// `SELF_HEAL_HEALTH_COOLDOWN_SECS`.
#[derive(Debug, Clone)]
pub(crate) struct PerformanceSelfHealer {
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
    /// (fps=30, speed=5, density=0.45). Exposed as a constant so tests
    /// and the event loop can reference it without magic strings.
    pub(crate) const FALLBACK_SCENE: &'static str = "low-power";

    pub(crate) fn new() -> Self {
        Self {
            high_pressure_since: None,
            low_pressure_since: None,
            pre_degraded_scene: None,
            is_downgraded: false,
            last_health_mitigation: None,
        }
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
            if score < SELF_HEAL_HEALTH_INVESTIGATE {
                let cooldown_ok = match self.last_health_mitigation {
                    None => true,
                    Some(last) => {
                        now.saturating_duration_since(last).as_secs_f64()
                            >= SELF_HEAL_HEALTH_COOLDOWN_SECS
                    }
                };
                if cooldown_ok {
                    self.last_health_mitigation = Some(now);
                    return SelfHealAction::TriggerHealthMitigation;
                }
            }
        }

        // ── P1: scene downgrade / restore ──
        if perf_pressure >= SELF_HEAL_PRESSURE_HIGH {
            // Pressure is high — accumulate (or start) the high streak.
            if self.high_pressure_since.is_none() {
                self.high_pressure_since = Some(now);
            }
            // Any high-pressure frame breaks the low-pressure recovery streak.
            self.low_pressure_since = None;

            if !self.is_downgraded {
                let since = self.high_pressure_since.unwrap_or(now);
                let elapsed = now.saturating_duration_since(since).as_secs_f64();
                if elapsed >= SELF_HEAL_DOWNGRADE_SECS {
                    // Fire downgrade — the event loop will fill pre_degraded_scene
                    // via record_downgrade() once it has applied the scene switch.
                    self.is_downgraded = true;
                    return SelfHealAction::DowngradeScene;
                }
            }
        } else if perf_pressure <= SELF_HEAL_PRESSURE_LOW {
            // Pressure is low — accumulate (or start) the recovery streak.
            if self.low_pressure_since.is_none() {
                self.low_pressure_since = Some(now);
            }
            // Any low-pressure frame breaks the high-pressure accumulation streak.
            self.high_pressure_since = None;

            if self.is_downgraded {
                let since = self.low_pressure_since.unwrap_or(now);
                let elapsed = now.saturating_duration_since(since).as_secs_f64();
                if elapsed >= SELF_HEAL_RESTORE_SECS {
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

// ────────────────────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── P1: PhasePredictor ──────────────────────────────────────────────────

    #[test]
    fn phase_predictor_starts_with_no_prediction() {
        let p = PhasePredictor::new();
        assert_eq!(p.predicts_active(0.0), None);
        assert_eq!(p.predicts_active(43200.0), None);
    }

    #[test]
    fn phase_predictor_predicts_after_two_transitions() {
        let mut p = PhasePredictor::new();
        // Active starts at 8:00 (28800s), ends at 18:00 (64800s).
        // Need multiple cycles for EMA to converge.
        for _ in 0..5 {
            p.record_transition(true, 28800.0); // idle → active at 8am
            p.record_transition(false, 64800.0); // active → idle at 6pm
        }

        // At noon → active
        assert_eq!(p.predicts_active(43200.0), Some(true));
        // At midnight → idle
        assert_eq!(p.predicts_active(0.0), Some(false));
        // At 7am → idle
        assert_eq!(p.predicts_active(25200.0), Some(false));
        // At 10am → active
        assert_eq!(p.predicts_active(36000.0), Some(true));
        // At 8pm → idle
        assert_eq!(p.predicts_active(72000.0), Some(false));
    }

    #[test]
    fn phase_predictor_handles_midnight_wraparound() {
        let mut p = PhasePredictor::new();
        // Active from 22:00 (79200s) to 06:00 (21600s) — crosses midnight.
        for _ in 0..5 {
            p.record_transition(true, 79200.0);
            p.record_transition(false, 21600.0);
        }

        // At 23:00 → active
        assert_eq!(p.predicts_active(82800.0), Some(true));
        // At 01:00 → active (past midnight)
        assert_eq!(p.predicts_active(3600.0), Some(true));
        // At 12:00 → idle
        assert_eq!(p.predicts_active(43200.0), Some(false));
    }

    #[test]
    fn phase_predictor_ema_converges() {
        let mut p = PhasePredictor::new();
        // Feed 10 identical transitions — EMA should converge near the true value.
        for _ in 0..10 {
            p.record_transition(true, 28800.0);
            p.record_transition(false, 64800.0);
        }
        // active_start_ema should be close to 28800.
        let diff = (p.active_start_ema - 28800.0).abs();
        assert!(diff < 100.0, "EMA should converge, diff = {diff}");
    }

    // ── P2: adaptive_resync_interval ────────────────────────────────────────

    #[test]
    fn resync_interval_standard_under_1h() {
        assert_eq!(
            adaptive_resync_interval(0.0),
            IDLE_REDRAW_RESYNC_INTERVAL_SECS
        );
        assert_eq!(
            adaptive_resync_interval(1800.0),
            IDLE_REDRAW_RESYNC_INTERVAL_SECS
        );
        assert_eq!(
            adaptive_resync_interval(3599.0),
            IDLE_REDRAW_RESYNC_INTERVAL_SECS
        );
    }

    #[test]
    fn resync_interval_60s_after_1h() {
        assert_eq!(adaptive_resync_interval(3600.0), 60.0);
        assert_eq!(adaptive_resync_interval(7200.0), 60.0);
        assert_eq!(adaptive_resync_interval(14399.0), 60.0);
    }

    #[test]
    fn resync_interval_120s_after_4h() {
        assert_eq!(adaptive_resync_interval(14400.0), 120.0);
        assert_eq!(adaptive_resync_interval(86400.0), 120.0);
    }

    // ── P4: ReclaimState ────────────────────────────────────────────────────

    #[test]
    fn reclaim_state_initial_should_reclaim() {
        let s = ReclaimState::new();
        assert!(s.should_reclaim(Instant::now()));
    }

    #[test]
    fn reclaim_state_respects_min_interval() {
        let mut s = ReclaimState::new();
        let t0 = Instant::now();
        s.mark_reclaimed(t0);
        let t1 = t0 + Duration::from_secs(100);
        assert!(!s.should_reclaim(t1));
        let t2 = t0 + Duration::from_secs(3700);
        assert!(s.should_reclaim(t2));
    }

    // ── P5: EnduranceHealth ─────────────────────────────────────────────────

    #[test]
    fn health_score_starts_at_100() {
        let h = EnduranceHealth::new();
        assert_eq!(h.score(), 100.0);
        assert_eq!(h.classification(), "healthy");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn health_score_stays_100_with_stable_rss() {
        let mut h = EnduranceHealth::new();
        // Push 10 identical RSS readings — variance = 0.
        for _ in 0..10 {
            h.push_rss(2800.0);
        }
        h.push_frame_time(0.5); // 0.5ms jitter
        h.push_ctxt_rate(60.0); // 60 switches/sec
        h.recompute();
        // With 0 variance, 0.5ms jitter, 60 switches/sec:
        // rss_score = 100, jitter_score = 95, ctxt_score = 70
        // weighted = 100*0.4 + 95*0.35 + 70*0.25 = 40 + 33.25 + 17.5 = 90.75
        assert!(h.score() > 85.0, "score should be > 85, got {}", h.score());
        assert_eq!(h.classification(), "healthy");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn health_score_drops_with_high_jitter() {
        let mut h = EnduranceHealth::new();
        for _ in 0..10 {
            h.push_rss(2800.0);
        }
        h.push_frame_time(10.0); // 10ms jitter — very high
        h.push_ctxt_rate(60.0);
        h.recompute();
        // jitter_score = 100 - 10*10 = 0
        // weighted = 100*0.4 + 0*0.35 + 70*0.25 = 40 + 0 + 17.5 = 57.5
        assert!(h.score() < 65.0, "score should be < 65, got {}", h.score());
        assert_eq!(h.classification(), "investigate");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn health_score_drops_with_rss_variance() {
        let mut h = EnduranceHealth::new();
        // Push wildly varying RSS readings.
        for i in 0..10 {
            h.push_rss(2800.0 + (i as f64) * 50.0); // 2800 → 3250
        }
        h.push_frame_time(0.5);
        h.push_ctxt_rate(60.0);
        h.recompute();
        // Variance is large → rss_score drops
        assert!(
            h.score() < 95.0,
            "score should reflect RSS instability, got {}",
            h.score()
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn health_score_needs_min_samples() {
        let mut h = EnduranceHealth::new();
        h.push_rss(2800.0); // Only 1 sample
        h.push_frame_time(10.0);
        h.push_ctxt_rate(100.0);
        h.recompute();
        // Should stay at 100 (insufficient data).
        assert_eq!(h.score(), 100.0);
    }

    // ── Performance Self-Healer (P1 + P2) ───────────────────────────────────

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
        // scene (fps=30, speed=5, density=0.45).
        assert_eq!(PerformanceSelfHealer::FALLBACK_SCENE, "low-power");
    }
}
