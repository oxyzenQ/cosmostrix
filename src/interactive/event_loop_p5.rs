// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! P5 endurance health sampling + fd health probe + thermal sensor.
//!
//! Extracted from `event_loop.rs` to keep that file under the 800-LOC cap.
//! Pure code motion — no behavior change.
//!
//! Handles three always-on monitoring tasks (not gated by --perf-stats):
//! 1. Endurance health: push frame time + RSS + ctxt-switch rate, recompute.
//! 2. stdout fd health probe: detect fd corruption before a write fails.
//! 3. Thermal sensor sampling (Linux only): feed into PowerManager.
//!
//! NIGHT-hunter-21: takes the loop context + the per-frame observation.
//! The old 11-parameter signature carried the Linux-gated
//! `last_ctxt_switches` ref and the `perf_rss_samples` counter as
//! separate mutable params; they are `ctx` fields now (the cfg-gated
//! field keeps its platform gate at the definition site).

use super::event_loop_ctx::{FrameObs, LoopCtx};
use crate::central_control_power_dragon::sample_thermal_pressure;
use crate::constants::{FD_HEALTH_PROBE_INTERVAL_FRAMES, THERMAL_SAMPLER_INTERVAL_FRAMES};

/// Run P5 endurance health sampling + fd health probe + thermal sampling.
///
/// Returns `false` when stdout fd corruption was detected and the caller
/// should break the rain loop. Returns `true` to continue.
///
/// S-master-HUNT-23: `obs.frame_period_s` feeds the utilization signal
/// (`obs.work_s / frame_period_s`) pushed into EnduranceHealth — the frame
/// signal is RELATIVE to the frame budget so slow-but-healthy terminals
/// are not classified as unstable (see endurance_health.rs module docs).
pub(crate) fn sample_p5_health(ctx: &mut LoopCtx, obs: &FrameObs) -> bool {
    // ── P5: Endurance health sampling (ALWAYS ON) ──
    // v80.0.0-beta.1 pause freeze (owner bug fix 2026-08-30): paused frames are
    // 4 Hz input polls, not render work — pushing their near-zero work
    // times would inflate the endurance score during a pause. Gate on
    // the same is_paused_or_decelerating() predicate the HUD freeze uses;
    // on resume the window continues from the last active sample.
    if !ctx.cloud.is_paused_or_decelerating() {
        // HUNT-23: utilization (work/budget), not absolute ms — see fn docs.
        let frame_budget_s = f64::from(obs.frame_period_s).max(1e-6);
        ctx.endurance_health
            .push_frame_utilization(obs.work_s as f64 / frame_budget_s);
    }
    if ctx.perf_rss_samples.is_multiple_of(60) {
        #[cfg(target_os = "linux")]
        {
            let rss = crate::sysstat::procstat::read_self_rss_kb();
            ctx.endurance_health.push_rss(rss as f64);
        }
        let elapsed = obs
            .work_start
            .saturating_duration_since(ctx.last_ctxt_sample)
            .as_secs_f64();
        if elapsed > 0.0 {
            #[cfg(target_os = "linux")]
            {
                let cur = crate::sysstat::procstat::read_self_voluntary_ctxt();
                if ctx.last_ctxt_switches > 0 {
                    let rate = (cur.saturating_sub(ctx.last_ctxt_switches)) as f64 / elapsed;
                    ctx.endurance_health.push_ctxt_rate(rate);
                }
                ctx.last_ctxt_switches = cur;
            }
            ctx.last_ctxt_sample = obs.work_start;
        }
        ctx.endurance_health.recompute();
        ctx.hud_state
            .set_endurance_health_score(ctx.endurance_health.score());
    }
    ctx.perf_rss_samples = ctx.perf_rss_samples.saturating_add(1);

    // P5: periodic stdout fd health probe.
    if ctx
        .perf_rss_samples
        .is_multiple_of(FD_HEALTH_PROBE_INTERVAL_FRAMES)
        && !ctx.term.probe_stdout_health()
    {
        ctx.cloud.raining = false;
        return false;
    }

    // Feature #13: thermal sensor sampling (Linux only).
    if ctx
        .perf_rss_samples
        .is_multiple_of(THERMAL_SAMPLER_INTERVAL_FRAMES)
    {
        if let Some(p) = sample_thermal_pressure() {
            ctx.power_manager.set_thermal_pressure(p);
        }
    }

    true
}
