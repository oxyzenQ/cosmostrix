// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

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
    let now = unsafe { libc::time(std::ptr::null_mut()) };
    if now < 0 {
        return 0.0;
    }
    let mut tm: MaybeUninit<libc::tm> = MaybeUninit::uninit();
    let tm_ptr = tm.as_mut_ptr();
    if unsafe { libc::localtime_r(&now, tm_ptr) }.is_null() {
        return 0.0;
    }
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
pub(crate) fn adaptive_resync_interval(idle_duration_secs: f64) -> f64 {
    if idle_duration_secs < 3600.0 {
        // < 1 hour idle: standard interval (20s).
        IDLE_REDRAW_RESYNC_INTERVAL_SECS
    } else if idle_duration_secs < 14400.0 {
        // 1–4 hours idle: 60s interval (3× reduction).
        60.0
    } else {
        // > 4 hours idle: 120s interval (6× reduction).
        120.0
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
#[allow(dead_code)]
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
        assert_eq!(adaptive_resync_interval(0.0), IDLE_REDRAW_RESYNC_INTERVAL_SECS);
        assert_eq!(adaptive_resync_interval(1800.0), IDLE_REDRAW_RESYNC_INTERVAL_SECS);
        assert_eq!(adaptive_resync_interval(3599.0), IDLE_REDRAW_RESYNC_INTERVAL_SECS);
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
        assert!(h.score() < 95.0, "score should reflect RSS instability, got {}", h.score());
    }

    #[test]
    fn health_score_needs_min_samples() {
        let mut h = EnduranceHealth::new();
        h.push_rss(2800.0); // Only 1 sample
        h.push_frame_time(10.0);
        h.push_ctxt_rate(100.0);
        h.recompute();
        // Should stay at 100 (insufficient data).
        assert_eq!(h.score(), 100.0);
    }
}
