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

use std::time::Instant;

use super::adaptive::EnduranceHealth;
use super::hud::HudState;
#[cfg(target_os = "linux")]
use super::intro;
use crate::central_control_dragon_power::{sample_thermal_pressure, PowerManager};
use crate::cloud::Cloud;
use crate::constants::{FD_HEALTH_PROBE_INTERVAL_FRAMES, THERMAL_SAMPLER_INTERVAL_FRAMES};
use crate::terminal::Terminal;

/// Run P5 endurance health sampling + fd health probe + thermal sampling.
///
/// Returns `false` when stdout fd corruption was detected and the caller
/// should break the rain loop. Returns `true` to continue.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sample_p5_health(
    endurance_health: &mut EnduranceHealth,
    hud_state: &mut HudState,
    power_manager: &mut PowerManager,
    term: &mut Terminal,
    cloud: &mut Cloud,
    work_s: f64,
    work_start: Instant,
    perf_rss_samples: &mut u64,
    #[cfg(target_os = "linux")] last_ctxt_switches: &mut u64,
    last_ctxt_sample: &mut Instant,
) -> bool {
    // ── P5: Endurance health sampling (ALWAYS ON) ──
    // v51 pause freeze (owner bug fix 2026-08-30): paused frames are
    // 4 Hz input polls, not render work — pushing their near-zero work
    // times would inflate the endurance score during a pause. Gate on
    // the same is_paused_or_decelerating() predicate the HUD freeze uses;
    // on resume the window continues from the last active sample.
    if !cloud.is_paused_or_decelerating() {
        endurance_health.push_frame_time(work_s * 1000.0);
    }
    if perf_rss_samples.is_multiple_of(60) {
        #[cfg(target_os = "linux")]
        {
            let rss = intro::read_self_rss_kb();
            endurance_health.push_rss(rss as f64);
        }
        let elapsed = work_start
            .saturating_duration_since(*last_ctxt_sample)
            .as_secs_f64();
        if elapsed > 0.0 {
            #[cfg(target_os = "linux")]
            {
                let cur = intro::read_self_voluntary_ctxt();
                if *last_ctxt_switches > 0 {
                    let rate = (cur.saturating_sub(*last_ctxt_switches)) as f64 / elapsed;
                    endurance_health.push_ctxt_rate(rate);
                }
                *last_ctxt_switches = cur;
            }
            *last_ctxt_sample = work_start;
        }
        endurance_health.recompute();
        hud_state.set_endurance_health_score(endurance_health.score());
    }
    *perf_rss_samples = perf_rss_samples.saturating_add(1);

    // P5: periodic stdout fd health probe.
    if perf_rss_samples.is_multiple_of(FD_HEALTH_PROBE_INTERVAL_FRAMES)
        && !term.probe_stdout_health()
    {
        cloud.raining = false;
        return false;
    }

    // Feature #13: thermal sensor sampling (Linux only).
    if perf_rss_samples.is_multiple_of(THERMAL_SAMPLER_INTERVAL_FRAMES) {
        if let Some(p) = sample_thermal_pressure() {
            power_manager.set_thermal_pressure(p);
        }
    }

    true
}
