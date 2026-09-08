// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Performance stats display accounting — extracted from `event_loop.rs`
//! to keep that file under the 800-LOC cap. Pure code motion — no behavior change.
//!
//! NIGHT-hunter-21: takes the loop context + the per-frame observation.
//! The old 22-parameter signature carried twelve mutable accumulators
//! (the `f64` trio `perf_work_sum_s`/`perf_pressure_sum`/
//! `perf_utilization_sum` was positionally transposable) and eight
//! trailing frame values — the accumulators are `ctx.perf` named
//! fields now, the frame values travel in [`FrameObs`] named fields.

use super::event_loop_ctx::{FrameObs, LoopCtx};

/// Update performance display counters when --perf-stats is enabled.
///
/// Always increments `ctx.perf.frames` + pushes the frame time (for
/// the post-exit FPS summary). When --perf-stats is on (`ctx.config
/// .startup.perf_stats`), also tracks drawn/idle frames, dirty-cell
/// accounting, work time, pressure, utilization, and overshoot.
pub(crate) fn update_perf_stats(ctx: &mut LoopCtx, obs: &FrameObs) {
    ctx.perf.frames = ctx.perf.frames.saturating_add(1);
    ctx.frame_time_tracker.push(obs.work_s as f64 * 1000.0);
    if ctx.config.startup.perf_stats {
        if obs.did_draw {
            ctx.perf.drawn_frames = ctx.perf.drawn_frames.saturating_add(1);
        } else {
            ctx.perf.idle_frames = ctx.perf.idle_frames.saturating_add(1);
        }
        let dirty_count = if obs.is_dirty_all {
            (ctx.frame.width as u64) * (ctx.frame.height as u64)
        } else {
            obs.dirty_len as u64
        };
        ctx.perf.dirty_sum = ctx.perf.dirty_sum.saturating_add(dirty_count);
        ctx.perf.dirty_samples = ctx.perf.dirty_samples.saturating_add(1);
        ctx.perf.work_sum_s += obs.work_s as f64;
        ctx.perf.work_max_s = ctx.perf.work_max_s.max(obs.work_s as f64);
        ctx.perf.pressure_sum += ctx.power_manager.effective_pressure() as f64;
        ctx.perf.pressure_max = ctx
            .perf
            .pressure_max
            .max(ctx.power_manager.effective_pressure());
        ctx.perf.utilization_sum += obs.utilization as f64;
        ctx.perf.utilization_max = ctx.perf.utilization_max.max(obs.utilization);
        if obs.overshoot > 0.0 {
            ctx.perf.overshoot_frames = ctx.perf.overshoot_frames.saturating_add(1);
        }
    }
}
