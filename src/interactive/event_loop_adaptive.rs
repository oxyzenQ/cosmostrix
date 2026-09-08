// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Adaptive throttling + reclaim — extracted from `event_loop.rs` to keep
//! that file under the 800-LOC cap. Pure code motion — no behavior change.
//!
//! NIGHT-hunter-21: takes the loop context (the old 7-parameter list
//! carried the pacing/reclaim mutable refs; all are ctx fields now).

use std::time::Instant;

use super::adaptive::adaptive_resync_interval;
use super::event_loop_ctx::LoopCtx;

/// Results from adaptive throttling.
pub(crate) struct ThrottleResult {
    pub loop_now: Instant,
    pub is_idle: bool,
    pub scene_generation_at_frame_start: u64,
}

/// Adaptive throttling: reduce FPS when idle to save CPU.
///
/// Captures loop_now, scene_generation_at_frame_start, begins PowerManager
/// frame, computes idle resync interval, and if sustained idle exceeds the
/// interval: forces full redraw + hints kernel to reclaim stale pages via
/// madvise(MADV_DONTNEED).
pub(crate) fn run_adaptive_throttle(ctx: &mut LoopCtx) -> ThrottleResult {
    // Adaptive throttling: reduce FPS when idle to save CPU.
    let loop_now = Instant::now();
    // Capture scene generation at frame start — u64 copy for self-healer.
    let scene_generation_at_frame_start = ctx.scene.scene_generation;
    // (Phase 3): PowerManager.begin_frame — is_idle, predictor, idle_started.
    let is_idle = ctx.power_manager.begin_frame(loop_now);
    // P2: adaptive resync interval based on sustained idle duration.
    let idle_secs = ctx
        .power_manager
        .idle_started()
        .map(|t| loop_now.saturating_duration_since(t).as_secs_f64())
        .unwrap_or(0.0);
    let effective_resync_interval = adaptive_resync_interval(idle_secs);
    if is_idle
        && loop_now
            .saturating_duration_since(ctx.last_resync_time)
            .as_secs_f64()
            >= effective_resync_interval
    {
        ctx.cloud.force_draw_everything();
        ctx.last_resync_time = loop_now;
        ctx.next_frame = loop_now;
        // P4: Hint kernel to reclaim stale pages during sustained idle.
        if ctx.reclaim_state.should_reclaim(loop_now) {
            let cells_ptr = ctx.frame.cells.as_ptr();
            let cells_len = ctx.frame.cells.len() * std::mem::size_of_val(&ctx.frame.cells[0]);
            // SAFETY: frame.cells is a valid Vec allocation.
            // hint_reclaim_pages advises only pages fully interior to
            // the allocation (never shared arena edge pages) — see
            // reclaim_state.rs for the corrected MADV_DONTNEED
            // semantics (zero-fill-on-demand). The zeroed interior
            // cells read as blank: force_draw_everything() was set
            // above, and the next rain_at() bumps the content
            // generation before any cell is read.
            unsafe {
                super::adaptive::hint_reclaim_pages(cells_ptr as *const u8, cells_len);
            }
            ctx.reclaim_state.mark_reclaimed(loop_now);
        }
    }
    ThrottleResult {
        loop_now,
        is_idle,
        scene_generation_at_frame_start,
    }
}
