// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Performance self-healer (P1+P2) — extracted from `event_loop.rs` to
//! keep that file under the 800-LOC cap. Pure code motion — no behavior
//! change.
//!
//! Observes CPU pressure + endurance health score via the
//! `PerformanceSelfHealer` policy, then applies the returned action:
//! - `None`: no mitigation needed.
//! - `TriggerHealthMitigation`: force full redraw + madvise hint.
//! - `DowngradeScene`: set aggressive throttle (visual identity preserved).
//! - `RestoreScene`: clear throttle on pressure recovery.

use std::time::Instant;

use super::adaptive::{PerformanceSelfHealer, ReclaimState, SelfHealAction};
use crate::app::CloudConfig;
use crate::central_control_dragon_power::SELF_HEAL_PRESSURE_LOW;
use crate::cloud::Cloud;
use crate::frame::Frame;

/// Run the performance self-healer for one frame.
///
/// Resets on scene change, observes current pressure + endurance score,
/// then applies the returned action (force redraw, madvise, throttle,
/// or restore). Mutates cloud/frame/reclaim_state/self_healer as needed.
/// Note: `frame` is only used on Linux (madvise hint path). On non-Linux
/// it's accepted but unused — prefixed with `_frame` via #[allow(unused)].
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_self_healer(
    self_healer: &mut PerformanceSelfHealer,
    reclaim_state: &mut ReclaimState,
    cloud: &mut Cloud,
    #[allow(unused_variables)] frame: &mut Frame,
    current_cfg: &CloudConfig,
    scene_name: &str,
    scene_generation: u64,
    scene_generation_at_frame_start: u64,
    power_manager_effective_pressure: f32,
    loop_now: Instant,
    endurance_health_score: f64,
) {
    // Performance self-healer (P1+P2): pure policy returning an action
    // enum. always pass Some(score) (P5 sampling always-on).
    // Reset on scene change BEFORE observe() so we don't fire on the
    // same frame the user switched. Phase D: u64 counter compare.
    if scene_generation != scene_generation_at_frame_start {
        self_healer.reset();
    }

    // v80.0.0-beta.1 power-dragon gate: the config contract for `power-dragon = false`
    // is "disables aggressive_throttle + idle FPS reduction" — but the flag
    // could have engaged while the dragon was still on and stayed set after
    // a live-reload turned it off (RestoreScene only fires on pressure
    // recovery). Release it here so glitches / CRT vignette / the spawn
    // curve return to their zero-pressure behavior immediately, and reset
    // the downgrade bookkeeping so re-enabling the dragon re-arms cleanly.
    if !current_cfg.power_dragon && cloud.aggressive_throttle {
        cloud.set_aggressive_throttle(false);
        self_healer.reset();
        // v80.0.0-alpha.1 (S-master-HUNT-3): verbose-only diagnostic channel — the self-heal
        // family reports automatic engine behavior, not user-actionable
        // state, so it surfaces only under --verbose (owner bug: the
        // predictive-throttle line exposed after every non-verbose run).
        crate::live_config::push_runtime_diag(
            "[self-heal] power-dragon off — releasing aggressive spawn throttle",
        );
    }

    let heal_action = self_healer.observe(
        power_manager_effective_pressure,
        loop_now,
        Some(endurance_health_score),
    );
    match heal_action {
        SelfHealAction::None => {}
        SelfHealAction::TriggerHealthMitigation => {
            // P2: force full redraw + bypass ReclaimState cooldown for
            // immediate madvise hint. Cooldown enforced inside self-healer.
            //
            // S-master-HUNT-23 congestion guard: when effective pressure
            // is elevated (>= SELF_HEAL_PRESSURE_LOW), the dominant cause
            // is OUTPUT congestion — the terminal cannot drain our ANSI
            // rate, so flush() blocks and frame work times balloon. A
            // forced FULL-SCREEN redraw in that state is the single
            // largest ANSI burst the renderer can produce (every cell,
            // typically 100-400 KB) pushed into an already saturated
            // pipe: the write blocks for seconds, the event loop freezes
            // with it ("particles stuck"), and when the terminal finally
            // drains, particles that expired during the stall vanish in
            // one step ("auto-dismiss"). That periodic bomb every 30 s
            // cooldown was the exact reported VTE/foot symptom. Under
            // congestion the madvise (cheap, memory-focused — the actual
            // P2 purpose) is kept and the redraw is skipped; the drain
            // backoff + spawn throttle handle the congestion itself.
            // The full redraw still fires when pressure is LOW — the
            // genuine "stuck visual state / desync" case P2 was designed
            // to clear (its original calibration).
            if power_manager_effective_pressure < SELF_HEAL_PRESSURE_LOW {
                cloud.force_draw_everything();
            }
            #[cfg(target_os = "linux")]
            {
                let cells_ptr = frame.cells.as_ptr();
                let cells_len = frame.cells.len() * std::mem::size_of_val(&frame.cells[0]);
                // SAFETY: frame.cells is a valid Vec allocation.
                // hint_reclaim_pages advises only pages fully interior
                // to the allocation (never shared arena edge pages) —
                // see reclaim_state.rs for the corrected MADV_DONTNEED
                // semantics (zero-fill-on-demand). The zeroed interior
                // cells read as blank: force_draw_everything() was set
                // above, and the next rain_at() bumps the content
                // generation before any cell is read.
                unsafe {
                    super::adaptive::hint_reclaim_pages(cells_ptr as *const u8, cells_len);
                }
                reclaim_state.mark_reclaimed(loop_now);
            }
            #[cfg(not(target_os = "linux"))]
            {
                // Non-Linux: madvise no-op, but mark reclaim state for
                // consistency with the P4 path.
                reclaim_state.mark_reclaimed(loop_now);
            }
        }
        SelfHealAction::DowngradeScene => {
            // AB-11 (option 2): do NOT switch scenes. Set the
            // aggressive_throttle flag instead — rain_at() uses steeper
            // spawn-scale + disables glitches. User's color/charset/
            // density/speed/glitch_level are NEVER touched. Flag clears
            // on pressure recovery.
            // v50: when power_dragon is false, skip throttle entirely
            // (owner Option D — user can disable adaptive protection).
            // v50.0.0-beta.6: use current_cfg.power_dragon (live-reloaded)
            // so live-reloading power_dragon=false immediately disables
            // the throttle — previously used stale startup cfg.power_dragon.
            if current_cfg.power_dragon && !self_healer.is_downgraded() {
                self_healer.record_downgrade(scene_name);
                cloud.set_aggressive_throttle(true);
                crate::live_config::push_runtime_diag(&format!(
                    "[self-heal] sustained high CPU pressure — throttling spawn rate (visual identity preserved: scene='{}')",
                    scene_name
                ));
            }
        }
        SelfHealAction::RestoreScene => {
            // AB-11: clear throttle flag. No scene restore needed.
            if self_healer.is_downgraded() {
                self_healer.take_pre_degraded_scene();
                cloud.set_aggressive_throttle(false);
                crate::live_config::push_runtime_diag(
                    "[self-heal] CPU pressure recovered — spawn throttle released",
                );
            }
        }
        SelfHealAction::PreemptiveThrottle => {
            // Dragon Engine v2: predictive throttle — pressure is rising
            // rapidly but hasn't hit the reactive downgrade threshold yet.
            // Activate aggressive_throttle early to smooth the spike before
            // it causes frame drops. Same flag as DowngradeScene but lighter
            // (no scene change, no pre_degraded_scene capture). The reactive
            // DowngradeScene path can still fire later if pressure sustains.
            if current_cfg.power_dragon {
                cloud.set_aggressive_throttle(true);
                crate::live_config::push_runtime_diag(
                    "[self-heal v2] predictive throttle — CPU pressure rising rapidly, throttling early",
                );
            }
        }
    }
}
