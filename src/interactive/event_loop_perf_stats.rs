// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Performance stats display accounting — extracted from `event_loop.rs`
//! to keep that file under the 800-LOC cap. Pure code motion — no behavior change.

use super::activity::FrameTimeTracker;
use crate::central_control_dragon_power::PowerManager;
use crate::frame::Frame;

/// Update performance display counters when --perf-stats is enabled.
///
/// Always increments perf_frames + pushes frame time (for post-exit FPS
/// summary). When --perf-stats is on, also tracks drawn/idle frames,
/// dirty-cell accounting, work time, pressure, utilization, and overshoot.
#[allow(clippy::too_many_arguments)]
pub(crate) fn update_perf_stats(
    perf_frames: &mut u64,
    perf_drawn_frames: &mut u64,
    perf_idle_frames: &mut u64,
    perf_dirty_sum: &mut u64,
    perf_dirty_samples: &mut u64,
    perf_work_sum_s: &mut f64,
    perf_work_max_s: &mut f64,
    perf_pressure_sum: &mut f64,
    perf_pressure_max: &mut f32,
    perf_utilization_sum: &mut f64,
    perf_utilization_max: &mut f32,
    perf_overshoot_frames: &mut u64,
    frame_time_tracker: &mut FrameTimeTracker,
    frame: &Frame,
    power_manager: &PowerManager,
    work_s: f32,
    did_draw: bool,
    is_dirty_all: bool,
    dirty_len: usize,
    overshoot: f32,
    utilization: f32,
    perf_stats_enabled: bool,
) {
    *perf_frames = perf_frames.saturating_add(1);
    frame_time_tracker.push(work_s as f64 * 1000.0);
    if perf_stats_enabled {
        if did_draw {
            *perf_drawn_frames = perf_drawn_frames.saturating_add(1);
        } else {
            *perf_idle_frames = perf_idle_frames.saturating_add(1);
        }
        let dirty_count = if is_dirty_all {
            (frame.width as u64) * (frame.height as u64)
        } else {
            dirty_len as u64
        };
        *perf_dirty_sum = perf_dirty_sum.saturating_add(dirty_count);
        *perf_dirty_samples = perf_dirty_samples.saturating_add(1);
        *perf_work_sum_s += work_s as f64;
        *perf_work_max_s = perf_work_max_s.max(work_s as f64);
        *perf_pressure_sum += power_manager.effective_pressure() as f64;
        *perf_pressure_max = perf_pressure_max.max(power_manager.effective_pressure());
        *perf_utilization_sum += utilization as f64;
        *perf_utilization_max = perf_utilization_max.max(utilization);
        if overshoot > 0.0 {
            *perf_overshoot_frames = perf_overshoot_frames.saturating_add(1);
        }
    }
}
