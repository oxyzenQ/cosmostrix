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
/// interval: forces full redraw + reclaims the frame buffer's stale pages
/// via the `reclaim_frame_cells` helper (madvise(MADV_DONTNEED) + the
/// zeroed-cell normalization of NIGHT-hunt-43 — see reclaim_state.rs for
/// the full contract).
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
            // NIGHT-hunt-43: route through reclaim_frame_cells, which
            // normalizes the zeroed cells immediately after the madvise.
            // The pre-fix code called hint_reclaim_pages directly here,
            // with a SAFETY comment claiming "the next rain_at() bumps
            // the content generation before any cell is read" — a
            // stale assumption even then: HUNT-25 had already moved the
            // Glyph (droplet family) force path to Frame::force_repaint,
            // which does NOT bump the generation (only the thirteen
            // structured styles still run clear_with_bg). A gen-matched
            // zeroed cell was emitted as a raw NUL byte that terminals
            // silently drop, so the pre-reclaim glyph stayed on screen
            // while the model said blank — and no model-side cleanup
            // could see it (the stuck-cell sweep skips fg-less cells;
            // phosphor only arms cells written this frame). The stranded
            // glyph persisted until a random droplet happened to pass
            // through that exact cell: the owner's "glitch shift rain"
            // report on the Glyph type, first visible after the first
            // idle resync (~30 s into an unattended screensaver run).
            super::adaptive::reclaim_frame_cells(&mut ctx.frame, &mut ctx.reclaim_state, loop_now);
        }
    }
    ThrottleResult {
        loop_now,
        is_idle,
        scene_generation_at_frame_start,
    }
}
