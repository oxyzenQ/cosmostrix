// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Simulation + draw step — extracted from `event_loop.rs` to keep that
//! file under the 800-LOC cap. Pure code motion — no behavior change.

use std::time::{Duration, Instant};

use super::adaptive::PowerManager;
use super::hud::HudState;
use crate::cloud::Cloud;
use crate::constants::{
    SIM_BASE_MULTIPLIER, SIM_FACTOR_MIN, SIM_MAX_CAP_SECS, SIM_MIN_FRACTION,
    SIM_PRESSURE_SCALE_FACTOR,
};
use crate::frame::Frame;
use crate::terminal::{is_terminal_gone, Terminal};

/// Results from the sim+draw step.
pub(crate) struct SimDrawResult {
    pub work_start: Instant,
    pub is_dirty_all: bool,
    pub dirty_len: usize,
    pub did_draw: bool,
    pub terminal_gone: bool,
}

/// Compute sim delta cap + call rain_at + refresh HUD colors + write HUD
/// to frame + term.draw().
///
/// Returns SimDrawResult with work_start, dirty state, did_draw, and
/// terminal_gone flag. When terminal_gone is true, caller should break
/// the rain loop. When draw returns an I/O error (not terminal_gone),
/// it's propagated via Err.
pub(crate) fn run_sim_and_draw(
    cloud: &mut Cloud,
    frame: &mut Frame,
    hud_state: &mut HudState,
    term: &mut Terminal,
    power_manager: &PowerManager,
    frame_period: Duration,
) -> Result<SimDrawResult, std::io::Error> {
    let sim_base_s = frame_period.as_secs_f64() * SIM_BASE_MULTIPLIER;
    // (perf audit): clamp lower bound is now `SIM_FACTOR_MIN`
    // from constants.rs — was a hardcoded `0.3` inline.
    let sim_factor = (1.0
        - (power_manager.effective_pressure() as f64) * SIM_PRESSURE_SCALE_FACTOR)
        .clamp(SIM_FACTOR_MIN, 1.0);
    let sim_min_s = (frame_period.as_secs_f64() * SIM_MIN_FRACTION).max(0.001);
    let sim_max_s = sim_base_s.min(SIM_MAX_CAP_SECS);
    // When frame_period is large (pause mode: 250ms, or very low FPS),
    // sim_min_s can exceed sim_max_s, which would panic in f64::clamp.
    // Sanitize: use sim_max_s as the effective lower bound when inverted.
    let sim_cap_s = if sim_min_s <= sim_max_s {
        (sim_base_s * sim_factor).clamp(sim_min_s, sim_max_s)
    } else {
        sim_max_s
    };
    cloud.set_max_sim_delta(Duration::from_secs_f64(sim_cap_s));
    let work_start = Instant::now();
    // v30 dragon-egg hunt: removed `cloud.is_idle = is_idle` write —
    // the field was a zombie (set here every frame, never read by any
    // cloud code path). The "Weather Director tick" mentioned in the
    // old comment never existed. The interactive event loop already
    // uses `is_idle` directly for frame_period selection above and
    // for the resync logic; the simulation itself does not need it.
    // P1: call rain_at directly with work_start instead of cloud.rain()
    // (which calls Instant::now() internally). Saves 1 Instant::now()
    // per frame (~20ns).
    cloud.rain_at(frame, work_start);
    // Refresh HUD line colors every frame (cheap — 4 brighten_color
    // calls ≈ 2 µs). This is split out of the 1 Hz `update_metrics`
    // tick so a runtime palette change (`c`/`C` key cycle, auto-color-
    // drift, live-config reload, scene transition) is reflected on
    // the very next frame, with no perceptible delay. Previously,
    // colors were computed inside `update_metrics` (rate-limited to
    // 1 Hz), so a palette change took up to 1 second to appear in
    // the HUD — the rain had already adopted the new palette while
    // the HUD still showed the old colors. The owner explicitly
    // flagged this as 'slight delay every owner changes colors at
    // runtime'. The split eliminates the delay without raising the
    // metric-tick rate (which would cause number flicker).
    //
    // Must run BEFORE write_to_frame so the colors used for THIS
    // frame's HUD cells are fresh — write_to_frame reads the Color
    // half of each cached_lines tuple.
    hud_state.refresh_colors(cloud.hud_colors());

    // Write HUD into the frame buffer BEFORE term.draw() so it's
    // part of the same flush — eliminates fullscreen flicker.
    // v16: Pass palette bg so HUD background follows --color-bg setting.
    hud_state.write_to_frame(frame, cloud.cols, cloud.palette.bg);

    // Cache dirty checks once per frame to avoid redundant method calls.
    let is_dirty_all = frame.is_dirty_all();
    let dirty_len = frame.dirty_indices().len();
    let did_draw = is_dirty_all || dirty_len > 0;
    if did_draw {
        if let Err(e) = term.draw(frame) {
            // EIO on Linux = terminal (PTY) was closed/destroyed.
            // BrokenPipe = write to closed pipe (macOS, some Linux).
            // In both cases, the terminal is gone — exit gracefully
            // instead of continuing to write to a dead fd.
            if is_terminal_gone(&e) {
                cloud.raining = false;
                return Ok(SimDrawResult {
                    work_start,
                    is_dirty_all: false,
                    dirty_len: 0,
                    did_draw: false,
                    terminal_gone: true,
                });
            }
            // Other I/O errors: propagate normally.
            return Err(e);
        }
    }

    Ok(SimDrawResult {
        work_start,
        is_dirty_all,
        dirty_len,
        did_draw,
        terminal_gone: false,
    })
}
