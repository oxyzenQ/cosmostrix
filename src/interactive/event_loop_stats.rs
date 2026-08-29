// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! SessionStats construction — extracted from `event_loop.rs` to keep that
//! file under the 800-LOC cap. Pure code motion — no behavior change.

use std::time::Instant;

use super::activity::FrameTimeTracker;
use super::adaptive::{EnduranceHealth, PowerManager};
use super::event_loop_finalize::SessionStats;
use crate::cloud::Cloud;

/// Inputs for building SessionStats.
pub(crate) struct StatsInputs<'a> {
    pub start_time: Instant,
    pub perf_frames: u64,
    pub perf_drawn_frames: u64,
    pub perf_idle_frames: u64,
    pub perf_overshoot_frames: u64,
    pub perf_dirty_sum: u64,
    pub perf_dirty_samples: u64,
    pub perf_work_sum_s: f64,
    pub perf_work_max_s: f64,
    pub perf_pressure_sum: f64,
    pub perf_pressure_max: f32,
    pub perf_utilization_sum: f64,
    pub perf_utilization_max: f32,
    pub frame_time_tracker: &'a FrameTimeTracker,
    pub power_manager: &'a PowerManager,
    pub endurance_health: &'a EnduranceHealth,
    pub cloud: &'a Cloud,
}

/// Build the SessionStats struct from loop-local state.
///
/// Assembles all perf counters + power manager + endurance health + grid
/// dimensions into the final SessionStats passed to finalize_session.
pub(crate) fn build_session_stats(inp: StatsInputs<'_>) -> SessionStats<'_> {
    let StatsInputs {
        start_time,
        perf_frames,
        perf_drawn_frames,
        perf_idle_frames,
        perf_overshoot_frames,
        perf_dirty_sum,
        perf_dirty_samples,
        perf_work_sum_s,
        perf_work_max_s,
        perf_pressure_sum,
        perf_pressure_max,
        perf_utilization_sum,
        perf_utilization_max,
        frame_time_tracker,
        power_manager,
        endurance_health,
        cloud,
    } = inp;
    SessionStats {
        start_time,
        perf_frames,
        perf_drawn_frames,
        perf_idle_frames,
        perf_overshoot_frames,
        perf_dirty_sum,
        perf_dirty_samples,
        // Exit-time grid snapshot — denominator for the runtime
        // avg_dirty_cell_ratio_percent (owner request 2026-08-23).
        grid_cols: cloud.cols,
        grid_lines: cloud.lines,
        perf_work_sum_s,
        perf_work_max_s,
        perf_pressure_sum,
        perf_pressure_max,
        perf_utilization_sum,
        perf_utilization_max,
        frame_time_tracker,
        power_manager_phase_transitions: power_manager.phase_transitions_observed(),
        power_manager_base_target_fps: power_manager.base_target_fps(),
        endurance_health_score: endurance_health.score(),
        endurance_health_classification: endurance_health.classification(),
    }
}
