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
use crate::central_control_power_dragon::SELF_HEAL_PRESSURE_LOW;
use crate::cloud::Cloud;
use crate::frame::Frame;

/// Value inputs for [`run_self_healer`] — the per-frame observation the
/// policy consumes.
///
/// NIGHT-hunter-21: these seven values were the trailing positional
/// parameters of the old 11-parameter signature — the
/// `scene_generation`/`scene_generation_at_frame_start` `u64` pair and
/// the pressure/score float pair were positionally transposable. Named
/// fields end that. The four MUTABLE targets (healer, reclaim, cloud,
/// frame) stay granular: they are distinct types, so the compiler
/// already rejects transposition — and the unit tests keep constructing
/// four small objects instead of a full loop context.
pub(crate) struct HealInputs<'a> {
    /// Live-reloaded effective config (power_dragon gate).
    pub cfg: &'a CloudConfig,
    /// Active scene name (downgrade diagnostics + policy bookkeeping).
    pub scene_name: &'a str,
    /// Current scene-family change counter.
    pub scene_generation: u64,
    /// Scene-family counter captured at frame start (reset detector).
    pub scene_generation_at_frame_start: u64,
    /// PowerManager effective pressure (the applied feed).
    pub effective_pressure: f32,
    /// Frame-start timestamp (reclaim cooldown bookkeeping).
    pub loop_now: Instant,
    /// Endurance health score (P5, always-on sampling).
    pub endurance_health_score: f64,
}

/// Run the performance self-healer for one frame.
///
/// Resets on scene change, observes current pressure + endurance score,
/// then applies the returned action (force redraw, madvise, throttle,
/// or restore). Mutates cloud/frame/reclaim_state/self_healer as needed.
/// Note: `frame` is only used on Linux (madvise hint path). On non-Linux
/// it's accepted but unused — prefixed with `_frame` via #[allow(unused)].
pub(crate) fn run_self_healer(
    self_healer: &mut PerformanceSelfHealer,
    reclaim_state: &mut ReclaimState,
    cloud: &mut Cloud,
    #[allow(unused_variables)] frame: &mut Frame,
    inputs: HealInputs,
) {
    // Performance self-healer (P1+P2): pure policy returning an action
    // enum. always pass Some(score) (P5 sampling always-on).
    // Reset on scene change BEFORE observe() so we don't fire on the
    // same frame the user switched. Phase D: u64 counter compare.
    if inputs.scene_generation != inputs.scene_generation_at_frame_start {
        self_healer.reset();
    }

    // v80.0.0-beta.1 power-dragon gate: the config contract for `power-dragon = false`
    // is "disables aggressive_throttle + idle FPS reduction" — but the flag
    // could have engaged while the dragon was still on and stayed set after
    // a live-reload turned it off (RestoreScene only fires on pressure
    // recovery). Release it here so glitches / CRT vignette / the spawn
    // curve return to their zero-pressure behavior immediately, and reset
    // the downgrade bookkeeping so re-enabling the dragon re-arms cleanly.
    if !inputs.cfg.power_dragon && cloud.aggressive_throttle {
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
        inputs.effective_pressure,
        inputs.loop_now,
        Some(inputs.endurance_health_score),
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
            if inputs.effective_pressure < SELF_HEAL_PRESSURE_LOW {
                cloud.force_draw_everything();
            }
            // NIGHT-hunt-43: unified reclaim entry point — the madvise
            // hint + normalize_reclaimed_cells + cooldown mark travel
            // together (see reclaim_frame_cells' doc for the full
            // chain). HUNT-26 originally added the normalization inline
            // at this site only; the P4 idle resync site drifted
            // without it, which is how the stranded-glyph bug came back
            // for the Glyph family. The helper is cross-platform: on
            // non-Linux the madvise is a no-op and the normalize scan
            // finds no zeroed cells, so it degrades to the cooldown-mark
            // behavior this site had before.
            super::adaptive::reclaim_frame_cells(frame, reclaim_state, inputs.loop_now);
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
            if inputs.cfg.power_dragon && !self_healer.is_downgraded() {
                self_healer.record_downgrade(inputs.scene_name);
                cloud.set_aggressive_throttle(true);
                crate::live_config::push_runtime_diag(&format!(
                    "[self-heal] sustained high CPU pressure — throttling spawn rate (visual identity preserved: scene='{}')",
                    inputs.scene_name
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
            if inputs.cfg.power_dragon {
                cloud.set_aggressive_throttle(true);
                crate::live_config::push_runtime_diag(
                    "[self-heal v2] predictive throttle — CPU pressure rising rapidly, throttling early",
                );
            }
        }
    }
}
