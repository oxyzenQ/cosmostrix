// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Sub-component timing tracker for the premium benchmark.
//!
//! Extracted from `bench.rs` to keep that file under its 850-LOC guard.
//! Accumulates per-frame sim/render/io timings so the benchmark report
//! can show where frame time is actually spent — distinguishing
//! "benchmark mainan" from "profiling tool".
//!
//! ## Component definitions
//! - **sim_ms**: time in `cloud.rain_at()` before the first frame mutation
//!   (atmosphere events, spawn rate, droplet physics). Read from
//!   `cloud.last_sim_ms()` after `rain_at` returns.
//! - **render_ms**: time in `cloud.rain_at()` during phosphor/anomaly/
//!   atmospheric frame mutations. Read from `cloud.last_render_ms()`.
//! - **io_ms**: time OUTSIDE `rain_at()` within the frame loop — dirty
//!   checks, `clear_dirty`, bookkeeping. In benchmark mode NO terminal
//!   write happens, so this is dirty-tracking overhead, not real IO.
//!   Labeled honestly in the report.

/// Accumulates per-frame component timings and produces averages + peaks.
pub(crate) struct ComponentTimer {
    sim_sum: f64,
    render_sum: f64,
    io_sum: f64,
    sim_max: f64,
    render_max: f64,
    io_max: f64,
    frames: u64,
}

impl ComponentTimer {
    pub(crate) fn new() -> Self {
        Self {
            sim_sum: 0.0,
            render_sum: 0.0,
            io_sum: 0.0,
            sim_max: 0.0,
            render_max: 0.0,
            io_max: 0.0,
            frames: 0,
        }
    }

    /// Record one frame's component timings.
    ///
    /// `io_ms` should already be clamped to >= 0 by the caller (the
    /// benchmark loop clamps against clock skew between cores).
    #[inline]
    pub(crate) fn record(&mut self, sim_ms: f64, render_ms: f64, io_ms: f64) {
        self.sim_sum += sim_ms;
        self.render_sum += render_ms;
        self.io_sum += io_ms;
        if sim_ms > self.sim_max {
            self.sim_max = sim_ms;
        }
        if render_ms > self.render_max {
            self.render_max = render_ms;
        }
        if io_ms > self.io_max {
            self.io_max = io_ms;
        }
        self.frames += 1;
    }

    /// Compute final averages + peaks. Division by `frames.max(1)` guards
    /// against the (impossible but defensive) zero-frame case.
    ///
    /// Returns `(avg_sim, avg_render, avg_io, max_sim, max_render, max_io)`.
    pub(crate) fn finalize(&self) -> (f64, f64, f64, f64, f64, f64) {
        let n = self.frames.max(1) as f64;
        (
            self.sim_sum / n,
            self.render_sum / n,
            self.io_sum / n,
            self.sim_max,
            self.render_max,
            self.io_max,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_timer_averages_and_peaks() {
        let mut t = ComponentTimer::new();
        t.record(1.0, 2.0, 0.5);
        t.record(3.0, 4.0, 1.5);
        t.record(2.0, 6.0, 1.0);
        let (avg_sim, avg_render, avg_io, max_sim, max_render, max_io) = t.finalize();
        // Averages: sim = (1+3+2)/3 = 2.0, render = (2+4+6)/3 = 4.0, io = (0.5+1.5+1.0)/3 = 1.0
        assert!((avg_sim - 2.0).abs() < 1e-9);
        assert!((avg_render - 4.0).abs() < 1e-9);
        assert!((avg_io - 1.0).abs() < 1e-9);
        // Peaks: sim=3, render=6, io=1.5
        assert!((max_sim - 3.0).abs() < 1e-9);
        assert!((max_render - 6.0).abs() < 1e-9);
        assert!((max_io - 1.5).abs() < 1e-9);
    }

    #[test]
    fn component_timer_zero_frames_returns_zero_averages() {
        let t = ComponentTimer::new();
        let (avg_sim, avg_render, avg_io, max_sim, max_render, max_io) = t.finalize();
        assert_eq!(avg_sim, 0.0);
        assert_eq!(avg_render, 0.0);
        assert_eq!(avg_io, 0.0);
        assert_eq!(max_sim, 0.0);
        assert_eq!(max_render, 0.0);
        assert_eq!(max_io, 0.0);
    }

    #[test]
    fn component_timer_sum_consistency() {
        // For each frame: sim + render + io should sum to total frame time.
        // Verify the tracker preserves this invariant across multiple frames.
        let mut t = ComponentTimer::new();
        let frames = [(1.0, 2.0, 0.5), (3.0, 4.0, 1.5), (2.0, 6.0, 1.0)];
        let mut expected_total = 0.0_f64;
        for (s, r, i) in frames {
            t.record(s, r, i);
            expected_total += s + r + i;
        }
        let (avg_sim, avg_render, avg_io, _, _, _) = t.finalize();
        let avg_total = avg_sim + avg_render + avg_io;
        let expected_avg = expected_total / frames.len() as f64;
        assert!(
            (avg_total - expected_avg).abs() < 1e-9,
            "avg_total ({avg_total}) must equal expected_avg ({expected_avg})"
        );
    }
}
