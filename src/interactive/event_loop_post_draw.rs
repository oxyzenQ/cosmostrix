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

// ─── S-master-HUNT-24: dynamic effects congestion gate ─────────────────────
//
// The static half of the effects auto-gate lives in build_cloud_cfg.rs
// (known CPU-rendered terminals: VTE family, konsole, foot, xterm.js,
// console TTY). This is the DYNAMIC half: the safety net for terminals
// the env-based detection could not see (an unmarked VTE build, a CPU
// terminal nested behind tmux with a generic TERM, or a GPU terminal
// genuinely overloaded at extreme cell counts).
//
// Signal: PowerManager::drain_backoff() (HUNT-23) — it rises when the
// flush syscall latency exceeds the frame budget (the terminal cannot
// drain our ANSI rate) and decays on clean writes. Sustained backoff is
// therefore a DIRECT measurement of "this terminal cannot keep up".
//
// Behavior: when backoff stays >= AUTO_FX_DRAIN_BACKOFF_THRESHOLD for
// AUTO_FX_SUSTAIN_SECS of wall time, cosmetic effects are disabled on
// the live Cloud for the rest of the session (sticky — no flapping).
// Rain-core visuals are untouched. In-flight particles fade out
// naturally on the next update tick (same contract as --no-effects).
//
// Sticky-by-design rationale: re-enabling on recovery would flap — with
// effects off the pipe recovers, which would re-enable effects, which
// re-congests, on a ~30s period; the user would see periodic effect
// bursts. A GPU terminal that just went through 4s of SUSTAINED drain
// backoff has lost almost nothing (the false-positive cost is one
// session of no click sparks); a CPU terminal gains a session that
// finally stays smooth.

/// Drain-backoff level (0.0..=1.0) at or above which the terminal counts
/// as congested for the effects gate. 0.20 is reached after ~4 sustained
/// overshooting frames (rise 0.05/unit) — well above the noise of a
/// single slow write, far below what a genuinely saturated pipe reaches.
pub(crate) const AUTO_FX_DRAIN_BACKOFF_THRESHOLD: f32 = 0.20;

/// How long the congestion must persist before effects are disabled.
/// Brief spikes (resize burst, one heavy frame) reset the timer; only a
/// sustained inability to drain disables the effects layer.
pub(crate) const AUTO_FX_SUSTAIN_SECS: f64 = 4.0;

/// Runtime state for the dynamic effects congestion gate (HUNT-24).
pub(crate) struct EffectsAutoGate {
    /// When sustained (>= threshold) drain congestion was first observed.
    congestion_since: Option<Instant>,
    /// True once the gate has disabled effects this session (sticky).
    disabled_this_session: bool,
}

impl EffectsAutoGate {
    pub(crate) const fn new() -> Self {
        Self {
            congestion_since: None,
            disabled_this_session: false,
        }
    }

    /// Observe one frame's drain state. Called right after
    /// post_draw_accounting with `power_manager.drain_backoff()`.
    ///
    /// Disables effects on `cloud` (and pushes a runtime diagnostic)
    /// when congestion sustains past AUTO_FX_SUSTAIN_SECS. No-op once
    /// disabled (sticky) or when effects are already off (--no-effects
    /// / static gate / bench mode).
    pub(crate) fn observe(&mut self, drain_backoff: f32, now: Instant, cloud: &mut Cloud) {
        if self.disabled_this_session || !cloud.effects_enabled {
            // Already gated (this session) or effects were never on —
            // nothing to observe. Also keeps the timer from running on
            // runs where the user explicitly opted out.
            self.congestion_since = None;
            return;
        }
        if drain_backoff >= AUTO_FX_DRAIN_BACKOFF_THRESHOLD {
            match self.congestion_since {
                None => self.congestion_since = Some(now),
                Some(first) => {
                    if now.saturating_duration_since(first).as_secs_f64() >= AUTO_FX_SUSTAIN_SECS {
                        cloud.set_effects_enabled(false);
                        self.disabled_this_session = true;
                        self.congestion_since = None;
                        crate::live_config::push_runtime_diag(
                            "[auto-fx] sustained output congestion (drain backoff for 4s+) — cosmetic effects disabled for this session: the terminal cannot sustain the effects' ANSI rate; rain-core visuals are unaffected",
                        );
                    }
                }
            }
        } else {
            // Clean (or merely noisy) frame — reset the sustain timer.
            self.congestion_since = None;
        }
    }
}

impl Default for EffectsAutoGate {
    fn default() -> Self {
        Self::new()
    }
}

/// Post-draw accounting: frame counter, work time, write overshoot, HUD
/// metrics, and PowerManager frame-end observation.
///
/// Returns (work_s, overshoot, utilization) for downstream consumers
/// (perf_stats display + self-healer).
///
/// S-master-HUNT-23: `did_draw` gates the write-latency overshoot. On
/// frames that did not draw (no dirty cells), `term.last_write_ns()` is
/// stale — it still holds the last DRAWN frame's latency. Feeding that
/// stale value every non-drawing frame would keep the drain backoff and
/// perf_pressure pinned on old evidence and hide recovery; conversely a
/// long-stalled frame followed by many no-draw frames would keep
/// injecting its overshoot forever. Zero on non-drawing frames lets
/// both decay on real (non-)evidence.
pub(crate) fn post_draw_accounting(
    hud_state: &mut HudState,
    power_manager: &mut PowerManager,
    term: &Terminal,
    cloud: &Cloud,
    work_start: Instant,
    frame_period_s: f32,
    did_draw: bool,
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
    let write_overshoot = if did_draw && frame_period_s > 0.0 {
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

#[cfg(test)]
mod hunt24_tests {
    //! S-master-HUNT-24: dynamic effects congestion gate unit tests.
    //!
    //! The gate converts sustained drain backoff (the HUNT-23 output
    //! congestion signal) into a sticky cosmetic-effects disable. These
    //! tests lock the contract: sustain threshold, timer reset on
    //! recovery, stickiness, and inertness on effects-off runs.

    use std::time::{Duration, Instant};

    use super::{EffectsAutoGate, AUTO_FX_DRAIN_BACKOFF_THRESHOLD, AUTO_FX_SUSTAIN_SECS};
    use crate::cloud::Cloud;

    /// Same fixture shape as `tests.rs::make_test_cloud()` (stable test
    /// helper duplicated per module — the project's established pattern).
    fn make_test_cloud() -> Cloud {
        let mut cloud = Cloud::new(
            crate::runtime::ColorMode::Mono,
            crate::runtime::ShadingMode::Random,
            crate::runtime::BoldMode::Off,
            false,
            true,
            crate::runtime::ColorScheme::Green,
            crate::rain_style::RainStyle::Glyph,
        );
        cloud.init_chars(vec!['0', '1']);
        cloud.reset(20, 10);
        cloud.clear_redraw_flags_for_test();
        cloud
    }

    #[test]
    fn sustained_congestion_disables_effects_then_sticky() {
        let mut gate = EffectsAutoGate::new();
        let mut cloud = make_test_cloud();
        cloud.set_effects_enabled(true);
        assert!(cloud.effects_enabled);

        let t0 = Instant::now();
        // First congested frame arms the timer.
        gate.observe(AUTO_FX_DRAIN_BACKOFF_THRESHOLD, t0, &mut cloud);
        assert!(
            cloud.effects_enabled,
            "single congested frame must not disable effects"
        );
        // 3s in: still under the 4s sustain window.
        gate.observe(
            AUTO_FX_DRAIN_BACKOFF_THRESHOLD,
            t0 + Duration::from_secs(3),
            &mut cloud,
        );
        assert!(cloud.effects_enabled, "3s < 4s sustain — no disable yet");
        // 4.5s in: threshold crossed.
        gate.observe(
            AUTO_FX_DRAIN_BACKOFF_THRESHOLD,
            t0 + Duration::from_millis(4500),
            &mut cloud,
        );
        assert!(
            !cloud.effects_enabled,
            "4.5s of sustained backoff must disable cosmetic effects"
        );

        // Sticky: even if effects are re-enabled externally (live-reload
        // path), the gate must not re-arm... and once disabled it stays
        // inert regardless of the congestion level.
        cloud.set_effects_enabled(true);
        let t1 = Instant::now();
        gate.observe(AUTO_FX_DRAIN_BACKOFF_THRESHOLD, t1, &mut cloud);
        gate.observe(
            AUTO_FX_DRAIN_BACKOFF_THRESHOLD,
            t1 + Duration::from_secs(60),
            &mut cloud,
        );
        assert!(
            cloud.effects_enabled,
            "gate is inert after firing (sticky) — it never re-disables"
        );
    }

    #[test]
    fn brief_congestion_spikes_reset_the_timer() {
        let mut gate = EffectsAutoGate::new();
        let mut cloud = make_test_cloud();
        cloud.set_effects_enabled(true);

        let t0 = Instant::now();
        // Oscillating: 3.9s congested, then a clean frame (reset), then
        // 3.9s congested again — the sustained window is never reached.
        for i in 0..39 {
            gate.observe(
                AUTO_FX_DRAIN_BACKOFF_THRESHOLD,
                t0 + Duration::from_millis(100 * i),
                &mut cloud,
            );
        }
        // Clean frame — backoff drops, timer resets.
        gate.observe(0.0, t0 + Duration::from_millis(3900), &mut cloud);
        for i in 39..78 {
            gate.observe(
                AUTO_FX_DRAIN_BACKOFF_THRESHOLD,
                t0 + Duration::from_millis(100 * i + 100),
                &mut cloud,
            );
        }
        assert!(
            cloud.effects_enabled,
            "oscillating congestion (reset by clean frames) must never disable effects"
        );
    }

    #[test]
    fn sub_threshold_backoff_is_not_congestion() {
        let mut gate = EffectsAutoGate::new();
        let mut cloud = make_test_cloud();
        cloud.set_effects_enabled(true);

        let t0 = Instant::now();
        // Just under the threshold, sustained forever: not congestion.
        gate.observe(AUTO_FX_DRAIN_BACKOFF_THRESHOLD - 0.01, t0, &mut cloud);
        gate.observe(
            AUTO_FX_DRAIN_BACKOFF_THRESHOLD - 0.01,
            t0 + Duration::from_secs(120),
            &mut cloud,
        );
        assert!(
            cloud.effects_enabled,
            "backoff below threshold is ordinary drain pacing, not congestion"
        );
    }

    #[test]
    fn effects_off_run_keeps_gate_inert() {
        let mut gate = EffectsAutoGate::new();
        let mut cloud = make_test_cloud();
        cloud.set_effects_enabled(false); // --no-effects / static gate / bench

        let t0 = Instant::now();
        gate.observe(AUTO_FX_DRAIN_BACKOFF_THRESHOLD, t0, &mut cloud);
        gate.observe(
            AUTO_FX_DRAIN_BACKOFF_THRESHOLD,
            t0 + Duration::from_secs(600),
            &mut cloud,
        );
        assert!(!cloud.effects_enabled);
        // And once effects are turned ON later, the gate starts fresh —
        // the timer was held clear while effects were off.
        cloud.set_effects_enabled(true);
        gate.observe(
            AUTO_FX_DRAIN_BACKOFF_THRESHOLD,
            t0 + Duration::from_secs(601),
            &mut cloud,
        );
        assert!(
            cloud.effects_enabled,
            "fresh timer after a effects-off period (no inherited congestion)"
        );
    }

    // Contract lock (compile-time — clippy-clean constant assertion):
    // the constants must stay in the regime that catches a CPU terminal
    // (congestion persists for the whole session) without firing on a
    // GPU terminal's transient burst (sub-second). 4s sits comfortably
    // between the two; the threshold must sit above drain noise and
    // below saturation.
    const _: () = {
        assert!(
            AUTO_FX_SUSTAIN_SECS >= 2.0 && AUTO_FX_SUSTAIN_SECS <= 8.0,
            "sustain window must stay in the 2-8s band"
        );
        assert!(
            AUTO_FX_DRAIN_BACKOFF_THRESHOLD > 0.10 && AUTO_FX_DRAIN_BACKOFF_THRESHOLD < 0.50,
            "threshold must sit above noise, below saturation"
        );
    };
}
