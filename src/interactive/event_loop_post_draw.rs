// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Post-draw frame accounting — extracted from `event_loop.rs` to keep
//! that file under the 800-LOC cap. Pure code motion — no behavior change.

use std::sync::atomic::Ordering;
use std::time::Instant;

use super::adaptive::PowerManager;
use super::hud::HudState;
use super::watchdog::FRAME_COUNTER;
use crate::cloud::Cloud;
use crate::terminal::Terminal;

/// Results from post-draw accounting — consumed by perf_stats + self-healer.
pub(crate) struct PostDrawMetrics {
    pub work_s: f32,
    pub overshoot: f32,
    pub utilization: f32,
}

/// Post-draw accounting: frame counter, work time, write overshoot, HUD
/// metrics, and PowerManager frame-end observation.
///
/// Returns (work_s, overshoot, utilization) for downstream consumers
/// (perf_stats display + self-healer).
pub(crate) fn post_draw_accounting(
    hud_state: &mut HudState,
    power_manager: &mut PowerManager,
    term: &Terminal,
    cloud: &Cloud,
    work_start: Instant,
    frame_period_s: f32,
) -> PostDrawMetrics {
    FRAME_COUNTER.fetch_add(1, Ordering::Relaxed);

    let work_s = work_start.elapsed().as_secs_f32();

    // v30 (VSCode crash fix): feed write latency into perf_pressure.
    // VSCode's xterm.js falls behind over long runs; a write taking
    // >50% of frame period signals the consumer cannot keep up.
    //
    // (bug fix): also feed a synthetic overshoot when the last
    // flush was suppressed by Tier 2.1 byte-budget backpressure.
    // Otherwise the suppression masks itself: no write_with_recovery
    // call → last_write_ns stale → perf_pressure doesn't accumulate
    // → self-healer never fires even though xterm.js is backing up.
    let write_overshoot = if frame_period_s > 0.0 {
        let raw = ((term.last_write_ns() as f32 / 1e9) / frame_period_s - 0.5).clamp(0.0, 2.0);
        // Suppressed flush: synthetic 1.0 signal (layered via .max).
        if term.last_flush_suppressed() {
            raw.max(1.0)
        } else {
            raw
        }
    } else {
        0.0
    };

    // Live HUD: push frame time, sample RSS + CPU%, recompute metrics.
    // All methods short-circuit when HUD is off (zero cost).
    hud_state.push_frame_time(work_s as f64 * 1000.0);
    hud_state.maybe_sample_rss();
    hud_state.maybe_sample_cpu();
    hud_state.update_metrics(cloud.hud_colors());

    let overshoot = ((work_s / frame_period_s) - 1.0).clamp(0.0, 2.0);
    let utilization = work_s / frame_period_s;
    // (Phase 3): PowerManager.observe_frame_end() replaces the
    // inline perf_pressure increment/decay. Same math, same constants.
    // overshoot is kept as a local for the perf_stats overshoot-frame
    // counter below.
    power_manager.observe_frame_end(work_s, frame_period_s, write_overshoot);

    PostDrawMetrics {
        work_s,
        overshoot,
        utilization,
    }
}
