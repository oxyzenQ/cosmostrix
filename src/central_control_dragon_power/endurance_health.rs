// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! P5: Endurance Health Score (EHS).
//!
//! A single 0–100 metric tracking long-endurance process stability based
//! on three orthogonal signals:
//!
//! - Memory stability — RSS variance over recent samples (ring buffer
//!   of 60 readings). Lower variance = higher score. Sampled on Linux
//!   via `/proc/self/status`.
//! - Frame work utilization — EMA of `work_s / frame_period_s`
//!   (unitless, 1.0 = the frame used its entire budget). Lower
//!   utilization = higher score. Cross-platform.
//! - Context switch rate — EMA of voluntary switches per second.
//!   Lower rate = higher score. Sampled on Linux via `/proc/self/stat`.
//!
//! S-master-HUNT-23: the frame signal used to be the ABSOLUTE work time
//! in ms with `100 - ms * 10` — anything ≥ 10 ms scored ZERO, which
//! calibrated the bands to Alacritty-class renderers only. On VTE or a
//! busy foot, NORMAL healthy operation (frame work 8–15 ms at a 16.7 ms
//! budget, utilization 0.5–0.9) already landed in the "investigate"
//! band (< 60) permanently — terminal slowness was being misread as
//! process instability, arming the P2 self-healer every 30 s. The
//! signal is now RELATIVE to the frame period: utilization, so a
//! terminal that keeps up at its own pace scores healthy, and only
//! SUSTAINED saturation (> 1.0 utilization EMA) reads as degraded
//! (floored at 40 — output saturation alone is the drain backoff's and
//! the spawn throttle's domain, not a memory-mitigation trigger).
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
//! The score is a weighted average: memory 40%, frame utilization 35%,
//! context switches 25%. Memory dominates because RSS variance is the
//! earliest indicator of a stuck/leaking long-endurance process.
//!
//! All subsystems here are zero-allocation, single-threaded, and
//! backward-compatible with the existing architecture invariants.

/// Endurance Health Score: a 0–100 metric based on:
/// - Memory stability (RSS variance over recent samples)
/// - Frame work utilization (EMA of work_s / frame_period_s — HUNT-23)
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
    /// EMA of frame work utilization (`work_s / frame_period_s`, unitless).
    /// 1.0 = the frame consumed its entire period; < 1.0 = headroom.
    /// HUNT-23: replaced the absolute-ms EMA, which was calibrated to
    /// fast terminals only (see module docs).
    frame_util_ema: f64,
    /// CC2-02: per-EMA init flag for `frame_util_ema`. The first push
    /// seeds the EMA; subsequent pushes smooth with alpha 0.05.
    frame_util_set: bool,
    /// EMA of context switch rate (switches/sec).
    ctxt_switch_ema: f64,
    /// CC2-02: per-EMA init flag for `ctxt_switch_ema`. Same fix as
    /// `frame_util_set` — prevents the first 59 pushes from overwriting
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
            frame_util_ema: 0.0,
            frame_util_set: false,
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

    /// Update the frame work utilization EMA. `utilization` is
    /// `work_s / frame_period_s` — unitless, 1.0 = the frame used its
    /// entire budget (HUNT-23: was absolute ms; see module docs for why
    /// that misclassified healthy slow terminals as unstable). Negative
    /// inputs are clamped to 0 (defensive — a negative work_s cannot
    /// occur, but a zero frame_period would produce one).
    pub(crate) fn push_frame_utilization(&mut self, utilization: f64) {
        // CC2-02: per-EMA init flag so the first push seeds and subsequent
        // pushes actually do EMA smoothing.
        if !self.frame_util_set {
            self.frame_util_ema = utilization.max(0.0);
            self.frame_util_set = true;
        } else {
            self.frame_util_ema = 0.95 * self.frame_util_ema + 0.05 * utilization.max(0.0);
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

        // Frame work utilization score: lower utilization = higher score.
        // HUNT-23: RELATIVE to the frame period (see module docs).
        //   util 0.0 → 100   (idle-fast frames)
        //   util 0.5 → 70    (healthy headroom on a busy terminal)
        //   util 1.0 → 40    (saturated — floor; output congestion is the
        //                     drain backoff's domain, not a memory trigger)
        // Blocked-write spirals push the EMA far above 1.0; the floor
        // keeps a PURE output-saturation state from arming the P2
        // memory mitigation on its own (RSS variance must contribute).
        let jitter_score = (100.0 - self.frame_util_ema * 60.0).clamp(40.0, 100.0);

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
    fn health_score_stays_healthy_with_stable_rss() {
        let mut h = EnduranceHealth::new();
        // Push 10 identical RSS readings — variance = 0.
        for _ in 0..10 {
            h.push_rss(2800.0);
        }
        h.push_frame_utilization(0.3); // 30% of frame budget used
        h.push_ctxt_rate(60.0); // 60 switches/sec
        h.recompute();
        // With 0 variance, util 0.3, 60 switches/sec:
        // rss_score = 100, util_score = 82, ctxt_score = 70
        // weighted = 1000.4 + 820.35 + 70*0.25 = 40 + 28.7 + 17.5 = 86.2
        assert!(h.score() > 85.0, "score should be > 85, got {}", h.score());
        assert_eq!(h.classification(), "healthy");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn health_score_hunts23_busy_terminal_not_investigate() {
        // S-master-HUNT-23 regression: a VTE/foot frame that uses 12ms of
        // its 16.7ms budget (util 0.72) is NORMAL for that terminal class.
        // The old absolute-ms formula (100 - 12*10 = jitter 0) classified
        // it "investigate" and armed the P2 full-redraw bomb every 30s.
        // With the utilization signal this must stay OUT of the
        // investigate band when memory is stable.
        let mut h = EnduranceHealth::new();
        for _ in 0..10 {
            h.push_rss(2800.0);
        }
        h.push_frame_utilization(0.72); // busy-but-keeping-up terminal
        h.push_ctxt_rate(60.0);
        h.recompute();
        // util_score = 100 - 0.72*60 = 56.8
        // weighted = 1000.4 + 56.80.35 + 70*0.25 = 40 + 19.88 + 17.5 = 77.38
        assert!(
            h.score() >= 60.0,
            "busy-but-healthy terminal must not be investigate, got {}",
            h.score()
        );
        assert_ne!(h.classification(), "investigate");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn health_score_pure_output_saturation_not_investigate() {
        // S-master-HUNT-23: pure output saturation (blocked-write spiral,
        // EMA util >> 1.0) is the drain backoff's domain — the jitter
        // contribution floors at 40 so a memory-stable process does NOT
        // arm the P2 memory mitigation from output congestion alone.
        let mut h = EnduranceHealth::new();
        for _ in 0..10 {
            h.push_rss(2800.0);
        }
        h.push_frame_utilization(4.0); // deeply saturated frames
        h.push_ctxt_rate(60.0);
        h.recompute();
        // util_score = floor 40
        // weighted = 1000.4 + 400.35 + 70*0.25 = 40 + 14 + 17.5 = 71.5
        assert!(
            h.score() >= 60.0,
            "pure output saturation must not be investigate, got {}",
            h.score()
        );
        assert_ne!(h.classification(), "investigate");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn health_score_investigate_needs_rss_instability() {
        // The "investigate" band must still be reachable the way it was
        // designed: memory instability (RSS variance) dominates the score.
        let mut h = EnduranceHealth::new();
        for i in 0..10 {
            h.push_rss(2800.0 + (i as f64) * 120.0); // large swings → high variance
        }
        h.push_frame_utilization(1.5);
        h.push_ctxt_rate(120.0);
        h.recompute();
        assert!(
            h.score() < 60.0,
            "RSS instability + saturation must reach investigate, got {}",
            h.score()
        );
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
        h.push_frame_utilization(0.3);
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
        h.push_frame_utilization(2.0);
        h.push_ctxt_rate(100.0);
        h.recompute();
        // Should stay at 100 (insufficient data).
        assert_eq!(h.score(), 100.0);
    }

    #[test]
    fn frame_utilization_ema_smooths_and_clamps_negatives() {
        let mut h = EnduranceHealth::new();
        h.push_frame_utilization(-1.0); // defensive clamp to 0
        assert_eq!(h.frame_util_ema, 0.0);
        h.push_frame_utilization(1.0);
        // 0.95 * 0 + 0.05 * 1.0 = 0.05
        assert!((h.frame_util_ema - 0.05).abs() < 1e-9);
    }
}
