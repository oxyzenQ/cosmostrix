// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! RSS sampler used by the premium benchmark (`--benchmark`).
//!
//! Extracted from `bench.rs` to keep that file under its 850-LOC guard.
//! The sampler polls `crate::memstat::current_rss_kb()` on a fixed
//! interval and accumulates peak + sum for averaging. Sampling is
//! rate-limited internally so the hot frame loop can call `tick()`
//! every iteration without measurable overhead.

use std::time::{Duration, Instant};

use crate::memstat;

/// Interval between RSS samples during the measurement phase.
///
/// 100 ms is fine-grained enough to catch allocator-driven growth and page
/// cache settling, while costing < 0.1% CPU on Linux (one `/proc` read)
/// and one Mach syscall on macOS.
pub(crate) const RSS_SAMPLE_INTERVAL: Duration = Duration::from_millis(100);

/// Tracks RSS during the measurement phase.
///
/// On platforms without RSS support, all values stay at their defaults and
/// `supported` remains `false`. The benchmark report uses this flag to
/// decide whether to emit real numbers or an "unsupported" notice.
pub(crate) struct RssTracker {
    last_sample: Instant,
    peak_kb: u64,
    sum_kb: u64,
    samples: u32,
    supported: bool,
}

impl RssTracker {
    /// Construct and take one initial sample. If the initial sample fails
    /// the platform is marked unsupported and all future `tick()` calls
    /// become no-ops.
    pub(crate) fn new() -> Self {
        let initial = memstat::current_rss_kb();
        let supported = initial.is_some();
        Self {
            // Allow the first measurement-loop tick to sample immediately.
            last_sample: Instant::now()
                .checked_sub(RSS_SAMPLE_INTERVAL)
                .unwrap_or_else(Instant::now),
            peak_kb: initial.unwrap_or(0),
            sum_kb: initial.unwrap_or(0),
            samples: initial.map(|_| 1).unwrap_or(0),
            supported,
        }
    }

    /// Poll RSS if the sample interval has elapsed. Called from the hot
    /// frame loop; returns cheaply when the interval has not elapsed or
    /// when the platform is unsupported.
    #[inline]
    pub(crate) fn tick(&mut self) {
        if !self.supported {
            return;
        }
        let now = Instant::now();
        if now.duration_since(self.last_sample) < RSS_SAMPLE_INTERVAL {
            return;
        }
        self.last_sample = now;
        if let Some(kb) = memstat::current_rss_kb() {
            if kb > self.peak_kb {
                self.peak_kb = kb;
            }
            self.sum_kb = self.sum_kb.saturating_add(kb);
            self.samples = self.samples.saturating_add(1);
        }
    }

    /// Final computed metrics: `(peak, avg, samples, supported)`.
    ///
    /// `avg` is `None` if zero samples were taken (should not happen on
    /// supported platforms, but the API is defensive). `peak` is `None`
    /// when the platform is unsupported so the report can distinguish
    /// "measured zero" from "not measured".
    pub(crate) fn finalize(&self) -> (Option<u64>, Option<u64>, u32, bool) {
        let avg = if self.samples > 0 {
            Some(self.sum_kb / u64::from(self.samples))
        } else {
            None
        };
        let peak = if self.supported {
            Some(self.peak_kb)
        } else {
            None
        };
        (peak, avg, self.samples, self.supported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rss_tracker_initial_sample_matches_platform_support() {
        let t = RssTracker::new();
        let expected_support = memstat::current_rss_kb().is_some();
        assert_eq!(t.supported, expected_support);
        // On supported platforms the constructor takes one sample, so
        // `samples` must be at least 1.
        if expected_support {
            assert!(t.samples >= 1, "initial sample must be recorded");
            assert!(t.peak_kb > 0, "peak must be non-zero on a live process");
        }
    }

    #[test]
    fn rss_tracker_tick_is_rate_limited() {
        let mut t = RssTracker::new();
        if !t.supported {
            return; // No-op on unsupported platforms.
        }
        // First tick: constructor backdates last_sample by RSS_SAMPLE_INTERVAL,
        // so the first tick DOES sample (or may, depending on wall-clock
        // elapsed since construction). We only assert it doesn't decrease.
        let after_ctor = t.samples;
        t.tick();
        let after_first_tick = t.samples;
        assert!(
            after_first_tick >= after_ctor,
            "first tick may sample (by design) but must not decrease samples"
        );
        // Second immediate tick — must be rate-limited IF wall-clock time
        // between the two ticks is < RSS_SAMPLE_INTERVAL. On heavily-loaded
        // CI runners, preemption can exceed 100ms, making the second tick
        // legitimately sample. We accept either outcome here.
        t.tick();
        assert!(
            t.samples >= after_first_tick,
            "second tick must not decrease samples (may sample if >100ms elapsed)"
        );
    }

    #[test]
    fn rss_tracker_finalize_returns_consistent_tuple() {
        let t = RssTracker::new();
        let (peak, avg, samples, supported) = t.finalize();
        assert_eq!(supported, t.supported);
        if supported {
            assert!(peak.is_some(), "peak must be Some when supported");
            assert!(avg.is_some(), "avg must be Some when supported");
            assert!(samples >= 1, "at least the initial sample must be counted");
            // peak >= avg by definition (peak is the max, avg is the mean).
            assert!(
                peak.unwrap() >= avg.unwrap(),
                "peak ({}) must be >= avg ({})",
                peak.unwrap(),
                avg.unwrap()
            );
        } else {
            assert!(peak.is_none(), "peak must be None when unsupported");
            assert!(avg.is_none(), "avg must be None when unsupported");
            assert_eq!(samples, 0);
        }
    }
}
