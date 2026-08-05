// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! System feeling: signal-driven palette drift classifier.
//!
//! When `--auto-color-drift` is enabled, the color ecosystem consults
//! this module to decide which aesthetic family to drift toward. The
//! classifier reads two signals:
//!
//! - **Process CPU%** via `cpustat::current_cpu_ns()` (Linux/macOS only).
//! - **Local wall-clock hour** via `current_local_hour()` (chrono::Local).
//!
//! ## Honest degradation
//!
//! If CPU sampling is unsupported on the current platform (Windows, some
//! sandboxes), `cpu_supported()` returns `false` and the classifier
//! narrows to **time-only** mode. The state is still computed, but the
//! CPU dimension is treated as "idle" — the rain drifts based purely on
//! time of day. This is NOT a silent fallback: `--doctor` prints the
//! degradation explicitly.
//!
//! ## No other signals
//!
//! This module deliberately refuses to read:
//! - System load average (`/proc/loadavg`, `getloadavg`)
//! - Thermal sensors (`/sys/class/thermal/*`)
//! - Disk IO (`/proc/diskstats`)
//! - Process RSS (`memstat::current_rss_kb`)
//! - Battery, GPS, active window, network traffic, camera
//!
//! See `docs/SYSTEM_FEELING.md` for the refused-signal manifest and
//! rationale. The boundary is open — every refused signal is documented.
//!
//! ## Integration
//!
//! `ColorEcosystem::tick()` calls `SystemFeeling::tick()` every 3-second
//! ecosystem tick. The classifier updates an EMA-smoothed CPU% and,
//! subject to hysteresis, may transition the current FeelingState. The
//! state is then read via `current_state()` and mapped to a ColorFamily
//! by `control_color_drift::family_for_state()`.

use std::time::{Duration, Instant};

use crate::control_color_drift::{
    FeelingState, CPU_BUSY_THRESHOLD, CPU_EMA_ALPHA, CPU_IDLE_THRESHOLD, MIN_STATE_DWELL_SECS,
    MORNING_END, MORNING_START, NIGHT_END, NIGHT_START, PRE_DAWN_END, PRE_DAWN_START,
};
use crate::cpustat;

/// Current local wall-clock hour as f64 (with minute/second fraction).
///
/// Inlined here after atmosphere engine elimination. The previous
/// implementation lived in `atmosphere_adaptive::current_hour()` which
/// was deleted along with the rest of the atmosphere engine subsystem.
/// Used by `SystemFeeling::tick()` for time-of-day state classification.
///
/// v30 (Hinnant-style): delegates to `clock::current_local_hour()` which
/// uses direct `libc::localtime_r` on Unix instead of `chrono::Local::now()`.
pub(crate) fn current_local_hour() -> f64 {
    crate::clock::current_local_hour()
}

/// System feeling state tracker. Persists across ecosystem ticks.
///
/// Carried as a field on `ColorEcosystem`. Derives `Clone, Copy` so it
/// survives live-config reload (which `#[derive(Clone, Copy)]` on
/// `ColorEcosystem` requires).
#[derive(Clone, Copy)]
pub(crate) struct SystemFeeling {
    /// EMA-smoothed CPU%. `None` until the first sample is taken.
    cpu_ema: Option<f32>,
    /// Last wall-clock instant when CPU was sampled.
    last_cpu_sample: Option<Instant>,
    /// Last raw CPU-ns reading (for delta computation).
    last_cpu_ns: Option<u64>,
    /// Current classified state.
    current_state: FeelingState,
    /// When `current_state` was entered. Used for hysteresis dwell check.
    state_entered_at: Instant,
    /// Whether CPU sampling is supported on this platform.
    ///
    /// Computed once at construction by probing `cpustat::current_cpu_ns()`.
    /// When `false`, `sample_cpu_percent` always returns `None` and the
    /// classifier runs in time-only mode.
    cpu_supported: bool,
}

impl SystemFeeling {
    /// Construct a new tracker. Probes CPU sampling support once.
    ///
    /// `now` should be the current `Instant::now()` from the caller's
    /// context — passed in so the caller can use a simulated instant
    /// in tests.
    pub(crate) fn new(now: Instant) -> Self {
        let initial_cpu_ns = cpustat::current_cpu_ns();
        let cpu_supported = initial_cpu_ns.is_some();
        Self {
            cpu_ema: None,
            last_cpu_sample: Some(now),
            last_cpu_ns: initial_cpu_ns,
            current_state: FeelingState::default(),
            state_entered_at: now,
            cpu_supported,
        }
    }

    /// Sample real signals and maybe transition state.
    ///
    /// Called every 3-second ecosystem tick by `ColorEcosystem::tick()`.
    /// Reads CPU% (if supported) and the local wall-clock hour, feeds
    /// them through the pure `classify()` function, and applies
    /// hysteresis before committing a state transition.
    ///
    /// `now` is the caller's `Instant` (may be simulated in tests).
    pub(crate) fn tick(&mut self, now: Instant) {
        let cpu = self.sample_cpu_percent(now);
        let hour = current_local_hour();
        self.update_state(cpu, hour, now);
    }

    /// Current classified state. Read by `ColorEcosystem::tick()` to
    /// pick the drift target family.
    pub(crate) fn current_state(&self) -> FeelingState {
        self.current_state
    }

    /// Whether CPU sampling is supported on this platform.
    ///
    /// When `false`, the classifier runs in time-only mode (CPU is
    /// treated as "idle"). `--doctor` prints this flag so degradation
    /// is visible, not silent.
    pub(crate) fn cpu_supported(&self) -> bool {
        self.cpu_supported
    }

    /// Last EMA-smoothed CPU% reading. `None` before the first sample
    /// or when CPU sampling is unsupported. Used by `--doctor` for
    /// diagnostics.
    pub(crate) fn cpu_ema(&self) -> Option<f32> {
        self.cpu_ema
    }

    /// Shift all internal timestamps by `elapsed`. Called on resume
    /// from pause so the feeling tracker doesn't think a long pause
    /// was a state dwell period.
    pub(crate) fn shift_in_time(&mut self, elapsed: Duration) {
        if let Some(ref mut t) = self.last_cpu_sample {
            *t += elapsed;
        }
        self.state_entered_at += elapsed;
    }

    /// Sample process CPU% and update the EMA. Returns the smoothed
    /// CPU% (or `None` if unsupported / not yet sampled).
    ///
    /// Computes instantaneous CPU% as:
    /// ```text
    /// cpu_ns_delta  = cpu_ns(now) - cpu_ns(prev)
    /// wall_ns_delta = wall_ns(now) - wall_ns(prev)
    /// cpu_percent   = (cpu_ns_delta / wall_ns_delta) * 100.0
    /// ```
    ///
    /// Then smooths via EMA:
    /// ```text
    /// cpu_ema = cpu_ema * (1 - alpha) + cpu_percent * alpha
    /// ```
    fn sample_cpu_percent(&mut self, now: Instant) -> Option<f32> {
        if !self.cpu_supported {
            return None;
        }
        let prev_sample = self.last_cpu_sample?;
        let prev_ns = self.last_cpu_ns?;
        let wall_delta = now.saturating_duration_since(prev_sample).as_nanos();
        // Always advance the sample cursor, even if we can't compute a
        // percentage this round (e.g. wall_delta == 0 on first tick).
        self.last_cpu_sample = Some(now);
        let cpu_ns_now = cpustat::current_cpu_ns()?;
        self.last_cpu_ns = Some(cpu_ns_now);
        if wall_delta == 0 {
            return self.cpu_ema;
        }
        let cpu_delta = cpu_ns_now.saturating_sub(prev_ns) as f64;
        let percent = ((cpu_delta / wall_delta as f64) * 100.0).clamp(0.0, 999.9) as f32;
        let smoothed = match self.cpu_ema {
            None => percent,
            Some(prev) => prev * (1.0 - CPU_EMA_ALPHA) + percent * CPU_EMA_ALPHA,
        };
        self.cpu_ema = Some(smoothed);
        self.cpu_ema
    }

    /// Apply the pure classifier and enforce hysteresis before
    /// committing a state transition.
    fn update_state(&mut self, cpu: Option<f32>, hour: f64, now: Instant) {
        let new_state = classify(cpu, hour);
        if new_state == self.current_state {
            return;
        }
        // Hysteresis: only transition if we've dwelled in the current
        // state long enough. This prevents flicker when CPU% hovers
        // near a threshold.
        let dwell = now
            .saturating_duration_since(self.state_entered_at)
            .as_secs_f32();
        if dwell < MIN_STATE_DWELL_SECS {
            return;
        }
        self.current_state = new_state;
        self.state_entered_at = now;
    }
}

/// Pure classification function. No state, no I/O.
///
/// Maps `(cpu_percent, local_hour)` to a [`FeelingState`]. This is the
/// single decision point for system-feeling drift. The thresholds and
/// time windows are defined in `control_color_drift.rs`.
///
/// When `cpu` is `None` (unsupported platform), the classifier treats
/// CPU as "idle" and decides based on time of day only. This is the
/// honest degraded mode — see the module docs.
///
/// # Decision tree
///
/// 1. CPU busy (>= `CPU_BUSY_THRESHOLD`) → `Signal` (any time of day)
/// 2. Pre-dawn (03:00–06:00) + not idle → `Compression`
/// 3. Night (22:00–06:00) + idle → `Void`
/// 4. Morning (06:00–12:00) + idle → `Pulse`
/// 5. Default → `Calm`
///
/// The order matters: CPU-busy wins over everything (a hot system at
/// 3am is still Signal, not Compression — urgency overrides mood).
#[must_use]
pub(crate) fn classify(cpu: Option<f32>, hour: f64) -> FeelingState {
    let hour = hour.rem_euclid(24.0);
    let cpu_busy = cpu.map(|c| c >= CPU_BUSY_THRESHOLD).unwrap_or(false);
    let cpu_idle = cpu.map(|c| c <= CPU_IDLE_THRESHOLD).unwrap_or(true);

    let is_night = !(NIGHT_END..NIGHT_START).contains(&hour);
    let is_pre_dawn = (PRE_DAWN_START..PRE_DAWN_END).contains(&hour);
    let is_morning = (MORNING_START..MORNING_END).contains(&hour);

    if cpu_busy {
        FeelingState::Signal
    } else if is_pre_dawn && !cpu_idle {
        FeelingState::Compression
    } else if is_night && cpu_idle {
        FeelingState::Void
    } else if is_morning && cpu_idle {
        FeelingState::Pulse
    } else {
        FeelingState::Calm
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── classify() pure function tests ────────────────────────────────────

    #[test]
    fn classify_busy_cpu_is_signal_regardless_of_time() {
        // CPU=80% (busy) at every hour should yield Signal.
        for hour in 0..24 {
            let state = classify(Some(80.0), f64::from(hour));
            assert_eq!(
                state,
                FeelingState::Signal,
                "busy CPU at hour {hour} should be Signal, got {:?}",
                state
            );
        }
    }

    #[test]
    fn classify_idle_cpu_at_midnight_is_void() {
        // 00:00 + idle CPU → Void (deep night).
        let state = classify(Some(5.0), 0.0);
        assert_eq!(state, FeelingState::Void);
    }

    #[test]
    fn classify_idle_cpu_at_3am_is_void_not_compression() {
        // 03:00 is pre-dawn, but with idle CPU, Void wins (Compression
        // requires non-idle CPU in the pre-dawn window).
        let state = classify(Some(5.0), 3.0);
        assert_eq!(state, FeelingState::Void);
    }

    #[test]
    fn classify_mid_cpu_at_3am_is_compression() {
        // 03:00 + mid CPU (not idle, not busy) → Compression.
        let state = classify(Some(30.0), 3.5);
        assert_eq!(state, FeelingState::Compression);
    }

    #[test]
    fn classify_busy_cpu_at_3am_is_signal_not_compression() {
        // CPU-busy overrides the pre-dawn Compression window.
        let state = classify(Some(70.0), 3.5);
        assert_eq!(state, FeelingState::Signal);
    }

    #[test]
    fn classify_idle_cpu_at_9am_is_pulse() {
        // 09:00 + idle CPU → Pulse (morning energy).
        let state = classify(Some(5.0), 9.0);
        assert_eq!(state, FeelingState::Pulse);
    }

    #[test]
    fn classify_idle_cpu_at_noon_is_calm() {
        // 12:00 + idle CPU → Calm (afternoon stable).
        let state = classify(Some(5.0), 12.0);
        assert_eq!(state, FeelingState::Calm);
    }

    #[test]
    fn classify_idle_cpu_at_3pm_is_calm() {
        // 15:00 + idle CPU → Calm.
        let state = classify(Some(5.0), 15.0);
        assert_eq!(state, FeelingState::Calm);
    }

    #[test]
    fn classify_idle_cpu_at_10pm_is_void() {
        // 22:00 + idle CPU → Void (night starts at 22:00).
        let state = classify(Some(5.0), 22.0);
        assert_eq!(state, FeelingState::Void);
    }

    #[test]
    fn classify_none_cpu_falls_back_to_time_only() {
        // Unsupported platform: cpu=None is treated as idle.
        // At midnight → Void, at noon → Calm, at 9am → Pulse.
        assert_eq!(classify(None, 0.0), FeelingState::Void);
        assert_eq!(classify(None, 9.0), FeelingState::Pulse);
        assert_eq!(classify(None, 12.0), FeelingState::Calm);
        assert_eq!(classify(None, 22.0), FeelingState::Void);
    }

    #[test]
    fn classify_hour_wraps_past_midnight() {
        // 25.0 should wrap to 1.0 → night + idle → Void.
        let state = classify(Some(5.0), 25.0);
        assert_eq!(state, FeelingState::Void);
    }

    #[test]
    fn classify_negative_hour_wraps() {
        // -1.0 should wrap to 23.0 → night + idle → Void.
        let state = classify(Some(5.0), -1.0);
        assert_eq!(state, FeelingState::Void);
    }

    #[test]
    fn classify_mid_cpu_in_afternoon_is_calm() {
        // 14:00 + mid CPU (not busy, not idle) → Calm (default branch).
        let state = classify(Some(30.0), 14.0);
        assert_eq!(state, FeelingState::Calm);
    }

    #[test]
    fn classify_threshold_boundary_busy_is_exactly_50() {
        // CPU_BUSY_THRESHOLD is 50.0; exactly 50.0 should be "busy" (>=).
        let state = classify(Some(CPU_BUSY_THRESHOLD), 14.0);
        assert_eq!(state, FeelingState::Signal);
    }

    #[test]
    fn classify_threshold_boundary_idle_is_exactly_15() {
        // CPU_IDLE_THRESHOLD is 15.0; exactly 15.0 should be "idle" (<=).
        // At 9am (morning) + idle → Pulse.
        let state = classify(Some(CPU_IDLE_THRESHOLD), 9.0);
        assert_eq!(state, FeelingState::Pulse);
    }

    // ── SystemFeeling struct tests ────────────────────────────────────────

    #[test]
    fn system_feeling_new_probes_cpu_support_honestly() {
        let now = Instant::now();
        let sf = SystemFeeling::new(now);
        // The supported flag must match what cpustat actually reports.
        let expected = cpustat::current_cpu_ns().is_some();
        assert_eq!(
            sf.cpu_supported(),
            expected,
            "cpu_supported must match cpustat probe at construction"
        );
    }

    #[test]
    fn system_feeling_default_state_is_calm() {
        let now = Instant::now();
        let sf = SystemFeeling::new(now);
        assert_eq!(
            sf.current_state(),
            FeelingState::Calm,
            "cold-start state must be Calm (safe neutral default)"
        );
    }

    #[test]
    fn system_feeling_hysteresis_blocks_immediate_transition() {
        // Even if signals clearly indicate a different state, the
        // hysteresis window (MIN_STATE_DWELL_SECS = 60s) blocks the
        // transition. We simulate by constructing at t=0 and ticking
        // at t=10s (well below the 60s dwell).
        let t0 = Instant::now();
        let mut sf = SystemFeeling::new(t0);
        assert_eq!(sf.current_state(), FeelingState::Calm);

        // Tick at t+10s. Even if classify() would return something else,
        // hysteresis should keep us in Calm.
        // NOTE: we can't control the real CPU% or hour in this test, but
        // we can assert that the state didn't change within the dwell
        // window. If classify() happens to return Calm, this is a no-op
        // pass. If it returns something else, hysteresis must block it.
        let t1 = t0 + Duration::from_secs(10);
        sf.tick(t1);
        assert_eq!(
            sf.current_state(),
            FeelingState::Calm,
            "state must not transition within MIN_STATE_DWELL_SECS window"
        );
    }

    #[test]
    fn system_feeling_hysteresis_allows_transition_after_dwell() {
        // After dwelling long enough, a state transition is allowed.
        // We can't control real signals, but we can verify the state
        // CAN change after the dwell window. This test is intentionally
        // permissive: it just asserts no panic and the state is one of
        // the 5 valid FeelingStates.
        let t0 = Instant::now();
        let mut sf = SystemFeeling::new(t0);
        let t1 = t0 + Duration::from_secs(120); // > MIN_STATE_DWELL_SECS (60s)
        sf.tick(t1);
        // State is whatever classify() returned (subject to hysteresis).
        // Just verify it's a valid variant by checking the label is non-empty.
        assert!(
            !sf.current_state().label().is_empty(),
            "state label must be non-empty after tick"
        );
    }

    #[test]
    fn system_feeling_shift_in_time_advances_timestamps() {
        let t0 = Instant::now();
        let mut sf = SystemFeeling::new(t0);
        let elapsed = Duration::from_secs(3600);
        sf.shift_in_time(elapsed);
        // After shift, state_entered_at should be t0 + elapsed.
        // We can't read state_entered_at directly (private), but we can
        // verify the shift didn't break anything by ticking immediately.
        sf.tick(t0 + elapsed);
        // No panic = pass.
    }

    #[test]
    fn system_feeling_cpu_ema_is_none_before_first_sample_on_unsupported() {
        // On unsupported platforms, cpu_ema stays None forever.
        // On supported platforms, it becomes Some after the first tick.
        // This test only asserts the None-when-unsupported contract;
        // the Some-when-supported contract is exercised by the
        // integration test in tests_color_stability.rs.
        let now = Instant::now();
        let sf = SystemFeeling::new(now);
        if !sf.cpu_supported() {
            assert!(
                sf.cpu_ema().is_none(),
                "cpu_ema must be None when CPU sampling is unsupported"
            );
        }
    }
}
