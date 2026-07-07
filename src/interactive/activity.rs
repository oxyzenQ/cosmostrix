// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Activity tracking, idle detection, and frame pacing utilities.
//!
//! Provides helpers for detecting idle state (no user input), managing
//! resync scheduling, and tracking frame timing for performance reports.

use std::time::{Duration, Instant};

use crate::constants::*;

/// Spin-wait until `deadline` is reached, capped at 1ms to avoid wasting CPU
/// on pathological cases (clock jumps, VM pauses).
///
/// Used for the final sub-millisecond portion of frame pacing where OS sleep
/// granularity (~0.5–2ms) is insufficient. The busy-wait ensures we hit the
/// frame deadline with microsecond precision rather than millisecond.
#[inline]
pub(super) fn spin_wait(deadline: Instant) {
    let spin_limit = Duration::from_micros(1000);
    let spin_start = Instant::now();
    while Instant::now() < deadline && spin_start.elapsed() < spin_limit {
        std::hint::spin_loop();
    }
}

#[inline]
pub(super) fn is_runtime_idle(last_input_time: Instant, now: Instant) -> bool {
    now.saturating_duration_since(last_input_time).as_secs_f64() >= IDLE_THRESHOLD_SECS
}

#[inline]
#[allow(dead_code)]
pub(super) fn idle_resync_due(is_idle: bool, last_resync_time: Instant, now: Instant) -> bool {
    is_idle
        && now
            .saturating_duration_since(last_resync_time)
            .as_secs_f64()
            >= IDLE_REDRAW_RESYNC_INTERVAL_SECS
}

#[inline]
pub(super) fn register_activity(
    last_input_time: &mut Instant,
    last_resync_time: &mut Instant,
    now: Instant,
    was_idle: bool,
    force_resync: bool,
) -> bool {
    *last_input_time = now;
    if was_idle || force_resync {
        *last_resync_time = now;
        true
    } else {
        false
    }
}

/// Rolling frame time tracker: allocation-free fixed-size ring buffer.
///
/// Tracks the last 60 frame times in milliseconds. Only used when
/// `--perf-stats` is enabled; otherwise has zero cost.
pub(super) struct FrameTimeTracker {
    times: [f64; 60],
    index: usize,
    count: usize,
}

impl FrameTimeTracker {
    pub(super) const fn new() -> Self {
        Self {
            times: [0.0; 60],
            index: 0,
            count: 0,
        }
    }

    pub(super) fn push(&mut self, ms: f64) {
        self.times[self.index] = ms;
        self.index = (self.index + 1) % 60;
        if self.count < 60 {
            self.count += 1;
        }
    }

    fn rolling_avg(&self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        let sum: f64 = self.times[..self.count].iter().sum();
        sum / self.count as f64
    }

    fn std_dev(&self) -> f64 {
        if self.count < 2 {
            return 0.0;
        }
        let mean = self.rolling_avg();
        let variance: f64 = self.times[..self.count]
            .iter()
            .map(|&t| (t - mean) * (t - mean))
            .sum::<f64>()
            / (self.count - 1) as f64;
        variance.sqrt()
    }

    pub(super) fn jitter_classification(&self) -> &'static str {
        let sd = self.std_dev();
        if sd < 0.5 {
            "low"
        } else if sd < 2.0 {
            "medium"
        } else {
            "high"
        }
    }

    pub(super) fn rolling_avg_ms(&self) -> f64 {
        self.rolling_avg()
    }

    /// p99 frame time (ms) computed from the ring buffer.
    ///
    /// Sorts a snapshot of the buffer on every call — 60 elements is
    /// ~300ns, negligible at the 4 Hz HUD redraw rate. Used by the live
    /// HUD overlay to highlight tail spikes alongside the rolling avg.
    pub(crate) fn p99_ms(&self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        let mut snapshot: Vec<f64> = self.times[..self.count].to_vec();
        snapshot.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let p99_idx = ((snapshot.len() as f64) * 0.99) as usize;
        snapshot[p99_idx.min(snapshot.len() - 1)]
    }
}
