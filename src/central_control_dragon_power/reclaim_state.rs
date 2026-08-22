// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! P4: Memory Pressure Adaptive Reclaim (MPAR) + P2: Idle Phase Aggressive
//! Coalescing (IPAC).
//!
//! Two tightly-coupled idle-mode subsystems that share the same underlying
//! concern — what should cosmostrix do when nothing has changed for a long
//! time:
//!
//! - **P2 (IPAC)** — [`adaptive_resync_interval`] progressively stretches
//!   the idle redraw resync interval (20s → 60s → 120s) based on how long
//!   the process has been continuously idle. This reduces forced redraw
//!   CPU spikes during long idle periods (typically 13+ hours per day in
//!   long-endurance runs).
//! - **P4 (MPAR)** — [`hint_reclaim_pages`] tells the Linux kernel to
//!   reclaim stale file-backed frame buffer pages via `madvise(MADV_DONTNEED)`
//!   during sustained idle. [`ReclaimState`] tracks the last reclaim time
//!   to avoid hammering madvise on every idle resync — the minimum
//!   interval is 1 hour.
//!
//! Both subsystems are zero-allocation, single-threaded, and
//! backward-compatible with the existing architecture invariants.

use std::time::{Duration, Instant};

use crate::constants::*;

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
/// (perf audit): the tier thresholds and intervals are now named
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
    let ret = libc::madvise(ptr as *mut libc::c_void, len, libc::MADV_DONTNEED);
    // CC2-06: ignore all errors (pages not reclaimable, ENOMEM, etc.) —
    // best-effort. The dead `MADV_DONTNEED = 4` comment was removed (the
    // code uses the symbolic constant, not the literal).
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
