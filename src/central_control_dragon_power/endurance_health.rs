// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! P5: Endurance Health Score (EHS).
//!
//! A single 0–100 metric tracking long-endurance process stability based
//! on three orthogonal signals:
//!
//! - **Memory stability** — RSS variance over recent samples (ring buffer
//!   of 60 readings). Lower variance = higher score. Sampled on Linux
//!   via `/proc/self/status`.
//! - **Frame jitter** — EMA of frame time in ms. Lower jitter = higher
//!   score. Cross-platform (frame time is always available).
//! - **Context switch rate** — EMA of voluntary switches per second.
//!   Lower rate = higher score. Sampled on Linux via `/proc/self/stat`.
//!
//! ## Classification bands
//!
//! - `>= 80` → `"healthy"` — process is stable, no action needed.
//! - `60–80` → `"degraded"` — mild instability, monitor.
//! - `< 60` → `"investigate"` — significant instability; the P2
//!   self-healer uses this band to trigger an immediate frame invalidate
//!   + memory reclaim hint (see `self_healer.rs`).
//!
//! ## Weighting
//!
//! The score is a weighted average: memory 40%, jitter 35%, context
//! switches 25%. Memory dominates because RSS variance is the earliest
//! indicator of a stuck/leaking long-endurance process.
//!
//! All subsystems here are zero-allocation, single-threaded, and
//! backward-compatible with the existing architecture invariants.

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
    /// CC2-02: per-EMA init flag for `frame_jitter_ema`. Previously the
    /// `if self.updates == 0` check was shared across `push_frame_time`
    /// and `push_ctxt_rate`, but `push_frame_time` is called every frame
    /// (60 FPS) while `recompute()` only runs every 60 frames — so the
    /// first 59 pushes each overwrote the EMA with the latest value
    /// instead of doing proper EMA smoothing. Per-EMA init flag fixes this.
    frame_jitter_set: bool,
    /// EMA of context switch rate (switches/sec).
    ctxt_switch_ema: f64,
    /// CC2-02: per-EMA init flag for `ctxt_switch_ema`. Same fix as
    /// `frame_jitter_set` — prevents the first 59 pushes from overwriting
    /// the EMA with the latest value instead of smoothing.
    /// Only read inside `push_ctxt_rate()` which is cfg-gated to Linux.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    ctxt_switch_set: bool,
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
            frame_jitter_set: false,
            ctxt_switch_ema: 0.0,
            ctxt_switch_set: false,
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
        // CC2-02: per-EMA init flag so the first push seeds and subsequent
        // pushes actually do EMA smoothing. Previously the `if self.updates
        // == 0` check was true for the first 60 frames (because `updates`
        // is only incremented in `recompute`), so each push overwrote the
        // EMA with the latest value.
        if !self.frame_jitter_set {
            self.frame_jitter_ema = frame_time_ms;
            self.frame_jitter_set = true;
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
        // CC2-02: per-EMA init flag so the first push seeds and subsequent
        // pushes actually do EMA smoothing. Previously `self.updates == 0`
        // was used, but `updates` is only incremented in `recompute()`
        // (every 60 frames), so the first 59 pushes each overwrote the
        // EMA with the latest value.
        if !self.ctxt_switch_set {
            self.ctxt_switch_ema = switches_per_sec;
            self.ctxt_switch_set = true;
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
        // CC2-07: formula is `100 - var*0.1`, so variance of 0 → score 100,
        // variance of 1000 (≈32 KB std dev) → score 0. The previous comment
        // said 10000 (100 KB²) which was off by 10×.
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
