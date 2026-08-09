// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! P1: Phase-Aware Adaptive Pacing (PAP).
//!
//! Phase predictor based on historical activity patterns. Uses an
//! exponential moving average (EMA) of activity transition times
//! (seconds since midnight) to predict whether the process should be
//! in active or idle mode. After observing at least 2 full cycles the
//! predictor becomes a proactive idle signal that fires *before* the
//! reactive 30-second idle threshold — smoothing long-endurance CPU
//! step-downs at predictable daily boundaries (lunch, end-of-day).
//!
//! The [`local_secs_since_midnight`] helper converts a wall-clock
//! `Instant`-derived elapsed time into seconds-since-local-midnight
//! using libc `localtime_r` on Linux (UTC fallback elsewhere) so the
//! predictor can compare EMA anchors recorded at different wall-clock
//! times of day without pulling in `chrono`.
//!
//! All subsystems here are zero-allocation, single-threaded, and
//! backward-compatible with the existing architecture invariants.

/// Phase predictor based on historical activity patterns.
///
/// Uses exponential moving average (EMA) of activity transition times
/// (seconds since midnight) to predict whether the process should be in
/// active or idle mode. After observing ≥2 full cycles, the predictor
/// becomes a proactive idle signal that fires *before* the reactive
/// 30-second idle threshold — smoothing long-endurance CPU step-downs
/// at predictable daily boundaries (lunch, end-of-day).
#[derive(Debug, Clone)]
pub(crate) struct PhasePredictor {
    /// EMA of active-phase start time (seconds since local midnight).
    active_start_ema: f64,
    /// EMA of active-phase end time (seconds since local midnight).
    active_end_ema: f64,
    /// Number of transitions recorded.
    transitions_observed: u64,
    /// Per-EMA initialization flags (CC2-01). Previously the global
    /// `transitions_observed == 0` check was used to seed each EMA, but
    /// when the first transition was `to_active=false`, `active_start_ema`
    /// stayed at the default `0.0`. The next `to_active=true` transition
    /// would then compute `0.3 * t + 0.7 * 0.0 = 0.3 * t`, biased toward
    /// midnight. Per-EMA init flags fix this without affecting the
    /// converged predictor.
    active_start_set: bool,
    active_end_set: bool,
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
            active_start_set: false,
            active_end_set: false,
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
            // CC2-01: per-EMA init flag avoids the cross-contamination where
            // a to_active=false transition leaves active_start_ema at 0.0,
            // biasing the next to_active=true update toward midnight.
            self.active_start_ema = if !self.active_start_set {
                self.active_start_set = true;
                t
            } else {
                self.alpha * t + (1.0 - self.alpha) * self.active_start_ema
            };
        } else {
            self.active_end_ema = if !self.active_end_set {
                self.active_end_set = true;
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
/// In practice, the event loop calls `local_secs_since_midnight()` directly
/// (power_manager.rs). For environments without timezone support, falls back
/// to 0.0 (predictions start from midnight).
#[cfg(unix)]
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

// cfg(not(unix)): Fallback uses UTC seconds. The predictor still works,
// just in UTC. Mirrors the gate choice in clock.rs and ambient.rs so all
// Hinnant-style sites agree on what "now" means on macOS/FreeBSD.
#[cfg(not(unix))]
pub(crate) fn local_secs_since_midnight() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    secs.rem_euclid(86400.0)
}

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
    fn phase_predictor_predicts_active_after_two_transitions() {
        // Active period: 09:00 → 17:00 (32400s → 61200s).
        // Need multiple cycles for EMA to converge (alpha=0.3).
        let mut p = PhasePredictor::new();
        for _ in 0..5 {
            p.record_transition(true, 9.0 * 3600.0);
            p.record_transition(false, 17.0 * 3600.0);
        }
        // At noon (43200s), should be predicted active.
        assert_eq!(p.predicts_active(12.0 * 3600.0), Some(true));
        // At 18:00 (64800s), should be predicted idle.
        assert_eq!(p.predicts_active(18.0 * 3600.0), Some(false));
    }

    #[test]
    fn phase_predictor_handles_midnight_wraparound() {
        // Active period crosses midnight: 22:00 → 06:00.
        // Need multiple cycles for EMA to converge (alpha=0.3).
        let mut p = PhasePredictor::new();
        for _ in 0..5 {
            p.record_transition(true, 22.0 * 3600.0);
            p.record_transition(false, 6.0 * 3600.0);
        }
        // 23:00 is active.
        assert_eq!(p.predicts_active(23.0 * 3600.0), Some(true));
        // 03:00 is active (after midnight).
        assert_eq!(p.predicts_active(3.0 * 3600.0), Some(true));
        // 12:00 is idle.
        assert_eq!(p.predicts_active(12.0 * 3600.0), Some(false));
    }

    #[test]
    fn phase_predictor_records_transition_count() {
        let mut p = PhasePredictor::new();
        assert_eq!(p.transitions_observed(), 0);
        p.record_transition(true, 100.0);
        assert_eq!(p.transitions_observed(), 1);
        p.record_transition(false, 200.0);
        assert_eq!(p.transitions_observed(), 2);
    }

    #[test]
    fn phase_predictor_ema_converges() {
        // Feed 10 identical transitions — EMA should converge so the
        // boundary lands within 100s of the true value. Verified via
        // a probe just past the boundary (28800 + 100 = 28900).
        let mut p = PhasePredictor::new();
        for _ in 0..10 {
            p.record_transition(true, 28800.0);
            p.record_transition(false, 64800.0);
        }
        // At 28900s (100s past 8:00) the predictor should report active.
        assert_eq!(p.predicts_active(28900.0), Some(true));
        // At 28700s (100s before 8:00) the predictor should report idle.
        assert_eq!(p.predicts_active(28700.0), Some(false));
    }
}
