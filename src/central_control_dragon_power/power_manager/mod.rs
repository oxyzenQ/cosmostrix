// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Power Manager — unified coordinator for the power/perf stack.
//!
//! Owns the three previously-scattered read paths that competed for the
//! same resources without a single owner:
//!
//! - [`effective_pressure`](PowerManager::effective_pressure) — replaces
//!   the inline `perf_pressure` accumulator in `event_loop.rs`. Same
//!   increment + decay math, same constants, now owned by one struct.
//! - [`effective_fps`](PowerManager::effective_fps) — replaces the
//!   `target_period` / `idle_period` / `pause_period` `Duration` cascade.
//!   Returns the target FPS for this frame; the caller converts to
//!   `Duration::from_secs_f64(1.0 / fps)`.
//! - [`is_idle`](PowerManager::is_idle) — replaces the
//!   `reactive_idle || predicted_idle` OR.
//!
//! ## Clash zones resolved
//!
//! | # | Clash zone                   | Resolution                                  |
//! |---|------------------------------|---------------------------------------------|
//! | 1 | FPS / frame_period (4-writer)| `effective_fps()` single owner              |
//! | 3 | Spawn rate (4-multiplier)    | downstream consumer of `effective_pressure` |
//! | 4 | madvise (2-writer)           | already coordinated via `ReclaimState`      |
//!
//! Clash zone 2 (scene/palette) and clash zone 5 (per-cell color) are
//! NOT owned by PowerManager — they are visual concerns, not power
//! concerns. They remain coordinated by `scene_generation` counter
//! (zone 2) and multiplicative composition (zone 5).
//!
//! ## Thermal guard (feature #13)
//!
//! The thermal guard is implemented as an INPUT to
//! [`effective_pressure`](PowerManager::effective_pressure), NOT as a
//! 7th independent signal path. Callers feed a 0.0–1.0 thermal pressure
//! scalar via [`set_thermal_pressure`](PowerManager::set_thermal_pressure);
//! it is added to the base `perf_pressure` and clamped to 1.0. This
//! means every downstream consumer of `effective_pressure()` (spawn
//! cascade, self-healer, sim factor) automatically responds to thermal
//! throttling without per-consumer wiring.
//!
//! The actual thermal sensor sampling (Linux
//! `/sys/class/thermal/thermal_zone*/temp`, macOS SMC, Windows WMI) is
//! a future feature — the input API is ready so the sampling layer can
//! be added without touching `PowerManager` internals.
//!
//! ## Frame lifecycle
//!
//! The event loop calls the methods in this order every frame:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │ 1. begin_frame(now)        → returns is_idle for this frame │
//! │ 2. effective_fps(paused)   → frame_period = 1.0 / fps       │
//! │ 3. effective_pressure()    → feed cloud + self-healer       │
//! │ 4. ── frame work happens ──                                │
//! │ 5. observe_frame_end(...)  → updates perf_pressure          │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! `is_idle()` returns the value computed by the last `begin_frame()`
//! call. `effective_pressure()` returns the value updated by the last
//! `observe_frame_end()` call (or 0.0 before the first frame).
//!
//! ## Migration status (Phase 3)
//!
//! This struct is the authoritative source for `perf_pressure`,
//! `is_idle`, and effective FPS. The event loop constructs one
//! `PowerManager` at startup and calls `begin_frame()` +
//! `observe_frame_end()` every frame; downstream consumers read
//! `effective_pressure()` / `effective_fps()` / `is_idle()` instead of
//! the previously-scattered local variables.

use std::time::Instant;

use crate::constants::*;

/// Unified power-management coordinator. Owns signal sampling state,
/// `perf_pressure` accumulation, idle detection, and effective FPS
/// resolution.
///
/// Replaces the scattered writers that previously competed for these
/// resources (see module docs for the clash-zone inventory).
///
/// The struct is intentionally NOT `Copy` — it owns a `PhasePredictor`
/// with EMA state that must be preserved across frames. The event loop
/// holds a single instance by value and passes `&mut self` to the
/// mutation methods.
///
/// struct + API are defined and wired into `event_loop.rs`.
/// `PowerManager` is constructed at startup and called every frame via
/// `begin_frame()` + `observe_frame_end()`.
pub(crate) struct PowerManager {
    thresholds: PowerThresholds,

    // ── perf_pressure accumulator (was event_loop.rs:142) ──
    perf_pressure: f32,

    // ── is_idle state (was event_loop.rs:177-186) ──
    last_input_time: Instant,
    phase_predictor: PhasePredictor,
    was_active: bool,
    idle_started: Option<Instant>,

    // ── effective FPS state (was event_loop.rs:139, 179) ──
    base_target_fps: f64,

    // ── Thermal guard input (feature #13) ──
    // 0.0 = cool, 1.0 = thermal emergency. Added to perf_pressure in
    // effective_pressure(). Future feature will sample real thermal
    // data; for now this stays 0.0 unless explicitly set.
    thermal_pressure: f32,
}

impl PowerManager {
    /// Construct a new `PowerManager` with the given base target FPS.
    ///
    /// The base target FPS is the value resolved by the upstream
    /// precedence chain (CLI > config > `dynamic_default` > xterm.js
    /// cap) — `PowerManager` does NOT re-resolve it. Use
    /// [`set_target_fps`](Self::set_target_fps) when live config reload
    /// changes the base.
    ///
    /// `now` seeds the idle timer so the first `begin_frame()` call
    /// does not falsely report idle (which would happen if
    /// `last_input_time` were `Instant::now()` at first read).
    #[must_use]
    pub(crate) fn new(base_target_fps: f64, now: Instant) -> Self {
        Self {
            thresholds: PowerThresholds::defaults(),
            perf_pressure: 0.0,
            last_input_time: now,
            phase_predictor: PhasePredictor::new(),
            was_active: true,
            idle_started: None,
            base_target_fps: base_target_fps.max(1.0),
            thermal_pressure: 0.0,
        }
    }

    /// Override the default thresholds. Test-only — production code
    /// uses the constants via [`PowerThresholds::defaults`].
    #[cfg(test)]
    pub(crate) fn with_thresholds(mut self, thresholds: PowerThresholds) -> Self {
        self.thresholds = thresholds;
        self
    }

    /// User input arrived — reset the idle timer and record the phase
    /// transition for the predictor.
    ///
    /// Replaces the inline `last_input_time = now` update +
    /// `phase_predictor.record_transition(true, ...)` call in
    /// `event_loop.rs`.
    ///
    /// The caller is still responsible for `last_resync_time` updates
    /// and `cloud.force_draw_everything()` — those are not power
    /// concerns and remain in `event_loop.rs`.
    pub(crate) fn note_activity(&mut self, now: Instant) {
        self.last_input_time = now;
        // Phase transition: idle → active. The predictor learns the
        // active-phase start time from this transition.
        if !self.was_active {
            self.phase_predictor
                .record_transition(true, local_secs_since_midnight());
            self.was_active = true;
        }
        // Clear idle_started so the next begin_frame() sees a fresh
        // idle window if input stops again.
        self.idle_started = None;
    }

    /// Live config reload changed the base target FPS.
    ///
    /// Replaces the inline `target_period` + `idle_period` `Duration`
    /// recomputation in `event_loop.rs` (lines 400-407). The next
    /// `effective_fps()` call will use the new base.
    pub(crate) fn set_target_fps(&mut self, fps: f64) {
        self.base_target_fps = fps.max(1.0);
    }

    /// Thermal guard input (feature #13). 0.0 = cool, 1.0 = thermal
    /// emergency. Added to base `perf_pressure` in
    /// [`effective_pressure`](Self::effective_pressure).
    ///
    /// the production sampler is now wired in `event_loop.rs`
    /// via `sample_thermal_pressure()` (Linux only). On non-Linux or
    /// in containers without thermal sysfs, the sampler returns `None`
    /// and this method is not called — the thermal input stays at 0.0
    /// and `effective_pressure` is identical to the base `perf_pressure`.
    ///
    /// # Clamping
    ///
    /// Values outside `[0.0, 1.0]` are clamped to the nearest endpoint.
    /// This prevents a misbehaving thermal sampler from pushing
    /// `effective_pressure` above 1.0 (which would be a silent no-op
    /// due to the clamp inside `effective_pressure`).
    pub(crate) fn set_thermal_pressure(&mut self, pressure: f32) {
        // CC2-03: explicit NaN guard + clamp. `f32::clamp` propagates NaN,
        // so a future SMC/WMI thermal sampler returning NaN would corrupt
        // effective_pressure() downstream. Map NaN → 0.0 first, then clamp.
        self.thermal_pressure = if pressure.is_nan() {
            0.0
        } else {
            pressure.clamp(0.0, 1.0)
        };
    }

    /// Called at the start of each frame, BEFORE frame work begins.
    /// Computes the idle state for this frame and updates the phase
    /// predictor + `idle_started` tracker.
    ///
    /// Replaces `event_loop.rs:505-524` (`reactive_idle ||
    /// predicted_idle` OR + `phase_predictor.record_transition` +
    /// `idle_started` tracking).
    ///
    /// Returns `is_idle` so the caller can use it for `effective_fps()`
    /// and resync scheduling without a second call.
    pub(crate) fn begin_frame(&mut self, now: Instant) -> bool {
        let reactive_idle = now
            .saturating_duration_since(self.last_input_time)
            .as_secs_f64()
            >= self.thresholds.idle_threshold_secs;
        let predicted_idle = self
            .phase_predictor
            .predicts_active(local_secs_since_midnight())
            .map(|active| !active)
            .unwrap_or(false);
        let is_idle = reactive_idle || predicted_idle;

        let now_active = !is_idle;
        if now_active != self.was_active {
            // Phase transition: record it so the predictor learns the
            // active-phase boundaries over time.
            self.phase_predictor
                .record_transition(now_active, local_secs_since_midnight());
            self.was_active = now_active;
        }

        if is_idle && self.idle_started.is_none() {
            self.idle_started = Some(now);
        } else if !is_idle {
            self.idle_started = None;
        }

        is_idle
    }

    /// Called at the end of each frame, AFTER frame work completes.
    /// Updates `perf_pressure` based on `work_s / frame_period_s` and
    /// the write-latency overshoot.
    ///
    /// Replaces `event_loop.rs:1092-1101` (`perf_pressure` increment /
    /// decay + write-overshoot accumulation).
    ///
    /// # Arguments
    ///
    /// - `work_s` — frame work duration in seconds (was `work_s` in
    ///   `event_loop.rs`).
    /// - `frame_period_s` — the period used for this frame. The caller
    ///   computes this from `effective_fps()` so the math is
    ///   self-consistent (the same `frame_period` feeds both the
    ///   scheduling and the overshoot computation).
    /// - `write_overshoot` — terminal write latency overshoot (already
    ///   computed by the caller from `term.last_write_ns()` +
    ///   `term.last_flush_suppressed()`). Kept as a parameter because
    ///   terminal state is not a power concern.
    pub(crate) fn observe_frame_end(
        &mut self,
        work_s: f32,
        frame_period_s: f32,
        write_overshoot: f32,
    ) {
        // ── perf_pressure increment/decay (was event_loop.rs:1092-1101) ──
        let overshoot = if frame_period_s > 0.0 {
            ((work_s / frame_period_s) - 1.0).clamp(0.0, 2.0)
        } else {
            0.0
        };
        if overshoot > 0.0 {
            self.perf_pressure =
                (self.perf_pressure + (overshoot * self.thresholds.pressure_increment)).min(1.0);
        } else {
            self.perf_pressure = (self.perf_pressure - self.thresholds.pressure_decay).max(0.0);
        }
        if write_overshoot > 0.0 {
            self.perf_pressure = (self.perf_pressure
                + (write_overshoot * self.thresholds.pressure_increment))
                .min(1.0);
        }
    }

    /// Effective `perf_pressure` — unified read replacing scattered
    /// `perf_pressure` reads. Includes the thermal guard as INPUT
    /// (added to base pressure, clamped to 1.0).
    ///
    /// Downstream consumers (`cloud.set_perf_pressure`, self-healer,
    /// sim factor) read this instead of a local `perf_pressure`
    /// variable.
    #[must_use]
    pub(crate) fn effective_pressure(&self) -> f32 {
        (self.perf_pressure + self.thermal_pressure).clamp(0.0, 1.0)
    }

    /// Effective FPS — replaces the `target_period` / `idle_period` /
    /// `pause_period` `Duration` cascade.
    ///
    /// Returns the target FPS for this frame; the caller converts to
    /// `Duration::from_secs_f64(1.0 / fps)`.
    ///
    /// # Arguments
    ///
    /// - `paused` — Cloud pause state (250 ms = 4 FPS when paused).
    ///   Pause is a Cloud state, not a power state, so it's passed as
    ///   a parameter rather than owned by `PowerManager`.
    ///
    /// # Resolution order
    ///
    /// 1. Paused → `1000 / PAUSE_PERIOD_MS` (4 FPS at 250 ms).
    /// 2. Idle → `base_target_fps * IDLE_FPS_FACTOR` (30 FPS at 60
    ///    base × 0.5).
    /// 3. Active → `base_target_fps`.
    ///
    /// This mirrors the previous `if cloud.pause / elif is_idle / else`
    /// cascade in `event_loop.rs:965-972`.
    /// v50: when `power_dragon_enabled` is false, idle FPS reduction
    /// is skipped (owner Option D — user can disable adaptive protection).
    #[must_use]
    pub(crate) fn effective_fps(&self, paused: bool, power_dragon_enabled: bool) -> f64 {
        if paused {
            1000.0 / PAUSE_PERIOD_MS as f64
        } else if power_dragon_enabled && self.is_idle() {
            self.base_target_fps * self.thresholds.idle_fps_factor
        } else {
            self.base_target_fps
        }
    }

    /// Idle state for the current frame. Returns the value computed by
    /// the last [`begin_frame`](Self::begin_frame) call.
    ///
    /// Before the first `begin_frame()` call, returns `false` (start
    /// assuming active — matches the previous `was_active = true`
    /// initialization in `event_loop.rs:183`).
    #[must_use]
    pub(crate) fn is_idle(&self) -> bool {
        !self.was_active
    }

    /// Read-only access to `idle_started` — the `Instant` the current
    /// idle window began, or `None` if active.
    ///
    /// Used by the P2 adaptive resync interval computation in
    /// `event_loop.rs` (`adaptive_resync_interval(idle_secs)`).
    #[must_use]
    pub(crate) fn idle_started(&self) -> Option<Instant> {
        self.idle_started
    }

    /// Read-only access to `base_target_fps` — the upstream-resolved
    /// target FPS (CLI > config > `dynamic_default` > xterm.js cap).
    ///
    /// Used by the HUD `tgt:` line + post-exit perf summary.
    #[must_use]
    pub(crate) fn base_target_fps(&self) -> f64 {
        self.base_target_fps
    }

    /// Read-only access to the phase predictor. Test-only — production
    /// code interacts with the predictor through `begin_frame()` +
    /// `note_activity()`.
    #[cfg(test)]
    pub(crate) fn phase_predictor(&self) -> &PhasePredictor {
        &self.phase_predictor
    }

    /// Read-only access to the phase predictor's transition count.
    /// Used by the post-exit verbose summary.
    #[must_use]
    pub(crate) fn phase_transitions_observed(&self) -> u64 {
        self.phase_predictor.transitions_observed()
    }
}

#[cfg(test)]
mod tests;
