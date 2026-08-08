// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Central Control — Dragon Power
//!
//! Single source of truth for every power management, performance
//! adaptive, and thermal/perf threshold parameter. This is the
//! **plug-and-play control file** for the entire power stack — modeled
//! after `central_control_rains.rs` for rain visuals.
//!
//! ## Scope
//!
//! Every constant in this file directly controls *how cosmostrix
//! manages power, performance, and adaptive behavior*. Anything that
//! affects FPS detection, perf_pressure accumulation, self-healer
//! thresholds, idle tiers, endurance health scoring, xterm.js byte
//! budgets, or fd health probes lives here. Non-power constants
//! (terminal limits, density sizing, rain visuals) stay in
//! `constants.rs` / `central_control_rains.rs`.
//!
//! ## How to fine-tune
//!
//! 1. Find the section you want to adjust below.
//! 2. Change the value(s).
//! 3. `cargo build --release` — that's it.
//!
//! All consumers reference `crate::constants::*` which re-exports
//! everything from this module via `pub use central_control_dragon_power::*;`.
//! No call-site changes needed when tuning.
//!
//! ## Feature inventory (12 features)
//!
//! | #  | Feature                    | Main file                       | Modifies                       |
//! |----|----------------------------|---------------------------------|--------------------------------|
//! | 1  | auto-color-drift          | control_color_drift.rs          | palette scheme + climate drift |
//! | 2  | dynamic-default-fps       | termdetect.rs, main.rs          | target_fps                     |
//! | 3  | xterm.js cap + Tier 2     | termdetect.rs, tier2.rs         | target_fps + ANSI bytes        |
//! | 4  | adaptive throttling       | event_loop.rs, activity.rs      | frame_period (idle × 0.5)      |
//! | 5  | phase predictor (P1)      | adaptive.rs                     | is_idle (OR'd with reactive)   |
//! | 6  | adaptive resync (P2)      | adaptive.rs                     | resync interval (20s/60s/120s) |
//! | 7  | reclaim state (P4)        | adaptive.rs                     | madvise(MADV_DONTNEED)         |
//! | 8  | endurance health (P5)     | adaptive.rs                     | score (RSS+jitter+ctxt)        |
//! | 9  | performance self-healer   | adaptive.rs, event_loop.rs      | scene + force_draw + madvise   |
//! | 10 | ambient scheduler         | ambient.rs, ambient_scheduler.rs| scene + palette                |
//! | 11 | climate post-FX           | chroma/post/climate.rs          | per-cell RGB + spawn_scale     |
//! | 12 | perf_pressure pipeline    | event_loop.rs, cloud/rain.rs    | spawn + sim + glitch + vignette|
//!
//! ## Clash zones (5 high-risk areas)
//!
//! These are areas where multiple writers compete for the same resource
//! without a coordinator. The PowerThresholds struct below is the first
//! step toward centralizing them.
//!
//! 1. **FPS / frame_period** — 4 independent writers (dynamic-default-fps,
//!    xterm.js cap, adaptive throttling, self-healer via low-power scene
//!    fps=30, ambient via scene). See the FPS Precedence Chain doc in
//!    `termdetect.rs` for the resolution order.
//! 2. **Scene / palette** — 3 writers (auto-color-drift, self-healer
//!    downgrade, ambient scheduler). scene_generation counter is reactive
//!    guard, not a mutex.
//! 3. **Spawn rate / density** — 4 layered multipliers in cloud/rain.rs
//!    (perf_pressure clamp → entropy → profile → gust → storytelling →
//!    resume_blend → glyph_entry_ramp). None are aware of each other.
//! 4. **Kernel memory (madvise)** — 2 writers (reclaim state rate-limited
//!    1h, self-healer P2 bypass cooldown). Self-healer can defeat
//!    rate-limiting purpose.
//! 5. **Per-cell color** — 2 writers (climate always-on + atmospheric
//!    post-FX). Compose multiplicatively, interaction undocumented.
//!
//! ## Section map
//!
//! | Section                        | Controls                                          |
//! |--------------------------------|---------------------------------------------------|
//! | Perf pressure pipeline         | Increment, decay, spawn factor, classification    |
//! | Idle tiers                     | Threshold, FPS factor, resync intervals           |
//! | Self-healer (P1 + P2)          | Pressure thresholds, downgrade/restore windows    |
//! | Endurance health (P5)          | Investigate threshold, cooldown                   |
//! | Pause / shutdown               | Pause period, shutdown timeout                    |
//! | Stuck-cell sweep (P4)          | Sweep interval, max-per-sweep                     |
//! | FD health probe (P5)           | Probe interval                                    |
//! | xterm.js Tier 2                | Byte budget, window, RIS reset, hard ceiling      |
//! | PowerThresholds struct         | Grouped thresholds for future PowerManager        |
//!
//! ## Calibration history
//!
//! - **v30.6 (power audit consolidation)**: extracted all power management
//!   constants from `constants.rs` into this file. Established single
//!   source of truth. Added `PowerThresholds` struct as the foundation
//!   for a future `PowerManager` coordinator that will own all signal
//!   sampling and expose unified `effective_pressure` / `effective_fps`
//!   / `is_idle` APIs.
//! - **v30.7 (Phase 2 migration)**: behavior code moved from
//!   `src/interactive/adaptive.rs` into submodules of this directory.
//!   Each submodule owns one subsystem (phase_predictor, reclaim_state,
//!   endurance_health, self_healer). `interactive/adaptive.rs` becomes
//!   a thin re-export shim. Layout mirrors `central_control_rains.rs`.
//! - **v30.8 (Phase 3 PowerManager)**: `power_manager` submodule added.
//!   `PowerManager` is the unified coordinator owning `perf_pressure`
//!   accumulation, `is_idle` detection, and effective FPS resolution.
//!   Exposes `effective_pressure()` / `effective_fps()` / `is_idle()`
//!   as the single read APIs for downstream consumers. Thermal guard
//!   (feature #13) is implemented as INPUT to `effective_pressure()`.

// ─── Behavior submodules ────────────────────────────────────────────────────
//
// Each submodule owns one adaptive subsystem. They are declared here so
// `crate::central_control_dragon_power::*` re-exports their public items
// to all consumers via `crate::constants::*` (which itself re-exports
// this module).

mod endurance_health;
mod phase_predictor;
mod power_manager;
mod reclaim_state;
mod self_healer;
mod thermal_sampler;

pub(crate) use endurance_health::*;
pub(crate) use phase_predictor::*;
pub(crate) use power_manager::*;
pub(crate) use reclaim_state::*;
pub(crate) use self_healer::*;
pub(crate) use thermal_sampler::*;

// ─── Perf pressure pipeline ─────────────────────────────────────────────────
//
// perf_pressure is a 0.0–1.0 scalar tracking how overloaded the renderer
// is relative to the target frame period. It accumulates on overshoot
// frames (work_s > frame_period_s) and decays on normal frames. Fed into:
//   - spawn rate scaling (PERF_PRESSURE_SPAWN_FACTOR)
//   - self-healer P1 (downgrade/restore scene)
//   - self-healer P2 (health mitigation)
//   - perf stats summary (low/medium/high classification)

/// Pressure spawn scaling factor: reduces spawn rate under perf pressure.
pub(crate) const PERF_PRESSURE_SPAWN_FACTOR: f32 = 0.75;

/// Performance pressure increment per overshoot frame.
pub(crate) const PERF_PRESSURE_INCREMENT: f32 = 0.25;

/// Performance pressure decay per normal frame.
pub(crate) const PERF_PRESSURE_DECAY: f32 = 0.02;

/// Perf-pressure classification threshold: below this = "low" pressure.
/// Used in the post-run perf report to bucket the average pressure into
/// low/medium/high. 0.05 = 5% average overshoot.
pub(crate) const PERF_PRESSURE_CLASS_LOW: f64 = 0.05;

/// Perf-pressure classification threshold: below this = "medium" pressure.
/// 0.30 = 30% average overshoot — sustained mild overload. Above = "high".
pub(crate) const PERF_PRESSURE_CLASS_MEDIUM: f64 = 0.30;

// ─── Idle tiers ──────────────────────────────────────────────────────────────
//
// When no user input arrives for IDLE_THRESHOLD_SECS, the renderer enters
// idle mode: frame_period is multiplied by IDLE_FPS_FACTOR (0.5 = half
// FPS), and the resync interval climbs through three tiers (20s → 60s →
// 120s) based on how long the idle has lasted. Any input instantly
// restores full performance.

/// Seconds of no user input before entering idle mode. In idle mode the
/// effective FPS target is reduced to conserve CPU/battery, and
/// atmospheric subsystem tick rates are lowered.
pub(crate) const IDLE_THRESHOLD_SECS: f64 = 30.0;

/// Effective FPS multiplier while idle. 0.5 = 30 FPS at 60 target.
/// Raised from 0.25 (v25) to keep phosphor decay visually smooth.
pub(crate) const IDLE_FPS_FACTOR: f64 = 0.5;

/// Wall-clock interval for one-shot full redraws while idle. Keeps
/// terminal/compositor state synchronized when idle FPS makes frame-count
/// drift correction too sparse.
pub(crate) const IDLE_REDRAW_RESYNC_INTERVAL_SECS: f64 = 20.0;

/// One hour in seconds — boundary between standard and 1-hour idle tier.
pub(crate) const SECS_PER_HOUR: f64 = 3600.0;

/// Four hours in seconds — boundary between 1-hour and 4-hour idle tier.
pub(crate) const SECS_PER_4_HOURS: f64 = 14400.0;

/// Resync interval (seconds) for 1–4 hours of sustained idle.
/// 3× reduction from the standard 20s interval.
pub(crate) const IDLE_RESYNC_TIER_2_SECS: f64 = 60.0;

/// Resync interval (seconds) for >4 hours of sustained idle.
/// 6× reduction from the standard 20s interval.
pub(crate) const IDLE_RESYNC_TIER_3_SECS: f64 = 120.0;

// ─── Self-healer P1 + P2 ─────────────────────────────────────────────────────
//
// P1 — Auto scene downgrade: when perf_pressure stays high for a sustained
//      window, switch to a lighter scene (low-power) to shed load. When
//      pressure recovers for a sustained window, restore the prior scene.
//
// P2 — Endurance-health mitigations: when the EnduranceHealth score
//      (RSS variance + frame jitter + context switches) drops into the
//      "investigate" band, trigger an immediate frame invalidate + memory
//      reclaim hint to clear potential stuck state.

/// perf_pressure threshold above which sustained-pressure accumulation
/// counts toward the auto-downgrade trigger. Set below the phosphor skip
/// gate (0.7) so the downgrade fires *before* visual quality degrades.
pub(crate) const SELF_HEAL_PRESSURE_HIGH: f32 = 0.6;

/// perf_pressure threshold below which sustained-pressure recovery counts
/// toward the auto-restore trigger. Hysteresis gap (0.6 → 0.3) prevents
/// oscillation when pressure hovers near the boundary.
pub(crate) const SELF_HEAL_PRESSURE_LOW: f32 = 0.3;

/// Seconds of sustained high perf_pressure before auto-downgrade fires.
/// 30s rides out transient spikes (compile jobs, window drags) but catches
/// genuine sustained overload before the user kills the process.
pub(crate) const SELF_HEAL_DOWNGRADE_SECS: f64 = 30.0;

/// Seconds of sustained low perf_pressure before auto-restore fires.
/// Deliberately longer than the downgrade window (60s vs 30s) to give the
/// restored scene a stable runway. Prevents flapping under borderline load.
pub(crate) const SELF_HEAL_RESTORE_SECS: f64 = 60.0;

/// EnduranceHealth score below which immediate mitigations fire.
/// Matches the "investigate" band from EnduranceHealth::classification().
pub(crate) const SELF_HEAL_HEALTH_INVESTIGATE: f64 = 60.0;

/// Minimum seconds between consecutive health-triggered mitigations.
/// 30s cooldown prevents a persistently unhealthy process from
/// force-redrawing every recompute cycle (≈1s).
pub(crate) const SELF_HEAL_HEALTH_COOLDOWN_SECS: f64 = 30.0;

// ─── Pause / shutdown ────────────────────────────────────────────────────────

/// Pause polling period in milliseconds. When cloud.pause is true, the
/// frame_period is replaced with this value (250ms = 4 FPS).
pub(crate) const PAUSE_PERIOD_MS: u64 = 250;

/// Graceful shutdown timeout in seconds (force-exit if flush blocks).
pub(crate) const SHUTDOWN_TIMEOUT_SECS: u64 = 2;

/// Monotonic clock jump guard: skip frame if elapsed exceeds this.
pub(crate) const CLOCK_JUMP_GUARD_SECS: f64 = 10.0;

// ─── P3: stdout /dev/tty fallback ────────────────────────────────────────────

/// Maximum consecutive /dev/tty fallback recoveries before propagating
/// the underlying stdout error. Defensive bound against pathological loops.
#[cfg(unix)]
pub(crate) const STDOUT_FALLBACK_MAX_RECOVERIES: u32 = 3;

// ─── P4: stuck-cell sweep ────────────────────────────────────────────────────

/// Frames between stuck-cell sweeps. 3600 frames ≈ 60s at 60 FPS.
/// Deliberately longer than FULL_REDRAW_INTERVAL_FRAMES — the full redraw
/// catches most stuck cells; the sweep catches drift between redraws.
pub(crate) const STUCK_CELL_SWEEP_INTERVAL_FRAMES: u64 = 3600;

/// Maximum stuck cells the sweep clears per pass. Prevents a pathological
/// case (e.g., after a resize race) from clearing tens of thousands of
/// cells in one sweep. The next full redraw catches the rest.
pub(crate) const STUCK_CELL_MAX_PER_SWEEP: usize = 256;

// ─── P5: fd health probe ─────────────────────────────────────────────────────

/// Frames between proactive stdout fd health probes. 3600 frames ≈ 60s
/// at 60 FPS. Matches the P4 stuck-cell sweep cadence — both are
/// "background hygiene" passes on the same slow tick.
pub(crate) const FD_HEALTH_PROBE_INTERVAL_FRAMES: u64 = 3600;

// ─── Feature #13: thermal sensor sampling ────────────────────────────────────
//
// Linux exposes per-zone CPU/SoC temperatures under
// /sys/class/thermal/thermal_zone*/temp (millidegrees Celsius). The
// sampler reads the hottest zone and normalizes to 0.0–1.0 via the
// linear ramp below. The result is fed into
// PowerManager::set_thermal_pressure() so every downstream consumer
// of effective_pressure() (spawn cascade, self-healer, sim factor)
// automatically responds to thermal throttling without per-consumer
// wiring.
//
// The sampler is best-effort: if /sys/class/thermal is missing
// (container, chroot), the call returns None and the previous
// thermal_pressure value is preserved — a transient read failure
// must NOT reset the thermal input to 0.0 (which would un-throttle
// the renderer mid-emergency).

/// Frames between thermal sensor samples. 600 frames ≈ 10s at 60 FPS.
/// Thermal mass is slow — sub-second sampling adds syscall cost
/// without changing the result. 10s cadence catches a thermal ramp
/// within ~1 effective_pressure recompute window.
pub(crate) const THERMAL_SAMPLER_INTERVAL_FRAMES: u64 = 600;

/// Temperature (°C) at which thermal_pressure = 0.0 (cool). Below this
/// the device is cool enough that no throttling is expected.
pub(crate) const THERMAL_PRESSURE_ZERO_C: i32 = 50;

/// Temperature (°C) at which thermal_pressure = 1.0 (throttle). At or
/// above this the device is at or past the throttle threshold and the
/// renderer should shed maximum load. 90 °C matches the typical
/// junction-temperature throttle band of x86_64 mobile and desktop
/// SoCs.
pub(crate) const THERMAL_PRESSURE_ONE_C: i32 = 90;

// ─── Tier 2: xterm.js byte-budget + RIS reset ────────────────────────────────
//
// Three defense layers against xterm.js V8 OOM (SIGTRAP) over multi-hour
// runs. All gated on `xtermjs_host` (VSCode, Hyper, WaveTerminal, Tabby,
// WarpTerminal, and any future Electron+xterm.js host):
//
//   1. Byte budget per window — suppresses flush when rolling window
//      exceeds XTERMJS_BYTE_BUDGET_PER_WINDOW.
//   2. RIS reset threshold — emits ESC c when cumulative bytes cross
//      XTERMJS_RIS_RESET_BYTES, clearing xterm.js's in-memory buffer.
//   3. Hard ceiling — forces RIS at XTERMJS_HARD_CEILING_BYTES regardless
//      of window state. Defensive last resort.
//
// All thresholds sized for the 30 FPS cap from Tier 1: at ~7 MB/sec worst
// case, the 10-second window budget of 40 MB gives ~5x headroom.

/// Byte budget per window for xterm.js hosts. 40 MB ≈ 5 seconds of
/// sustained 7 MB/sec output. When exceeded, `flush_ansi` suppresses the
/// write to let xterm.js drain.
pub(crate) const XTERMJS_BYTE_BUDGET_PER_WINDOW: u64 = 40 * 1024 * 1024;

/// Frames in the rolling byte-budget window. 600 frames ≈ 10s at 60 FPS,
/// ≈ 20s at the 30 FPS xterm.js cap.
pub(crate) const XTERMJS_BYTE_BUDGET_WINDOW_FRAMES: u64 = 600;

/// Cumulative bytes since last RIS reset that triggers an ESC c emission.
/// 50 MB ≈ 7 seconds at the 7 MB/sec Tier 1 cap.
pub(crate) const XTERMJS_RIS_RESET_BYTES: u64 = 50 * 1024 * 1024;

/// Absolute cumulative ceiling before a forced RIS, even mid-flush.
/// 200 MB is the "xterm.js approaching V8 heap pressure" zone.
pub(crate) const XTERMJS_HARD_CEILING_BYTES: u64 = 200 * 1024 * 1024;

// ─── PowerThresholds struct (foundation for PowerManager) ────────────────────
//
// v30.6: grouped all power management thresholds into a single type.
// v30.8: PowerManager (in power_manager.rs) now consumes this struct.
// PowerManager owns an instance plus all signal sampling state
// (perf_pressure accumulator, idle timer, phase predictor, thermal
// pressure input) and exposes the unified APIs:
//   - effective_pressure() -> f32       (replaces scattered perf_pressure reads)
//   - effective_fps() -> f64            (replaces the 4-writer FPS cascade)
//   - is_idle() -> bool                 (replaces reactive || predicted OR)
//
// The constants above remain the active source of truth for the
// PowerThresholds::defaults() constructor. PowerManager reads them
// indirectly through the struct fields.

/// Grouped power management thresholds. Consumed by `PowerManager`
/// (see `power_manager.rs`) and `PerformanceSelfHealer` (see
/// `self_healer.rs`). The constants above remain the active source
/// of truth; `PowerThresholds::defaults()` reads them at
/// construction time.
///
/// v30.9: `PerformanceSelfHealer::observe()` now reads all 6
/// self-healer fields (`pressure_high`, `pressure_low`,
/// `downgrade_secs`, `restore_secs`, `health_investigate`,
/// `health_cooldown_secs`) from this struct instead of from the
/// standalone constants. The struct is now the sole consumer-facing
/// API for these thresholds; the standalone constants remain as the
/// canonical values that `defaults()` copies into the struct.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PowerThresholds {
    /// perf_pressure threshold for P1 downgrade trigger (0.6).
    pub pressure_high: f32,
    /// perf_pressure threshold for P1 restore trigger (0.3).
    pub pressure_low: f32,
    /// Seconds of sustained high pressure before downgrade (30.0).
    pub downgrade_secs: f64,
    /// Seconds of sustained low pressure before restore (60.0).
    pub restore_secs: f64,
    /// EnduranceHealth score below which P2 mitigation fires (60.0).
    pub health_investigate: f64,
    /// Minimum seconds between P2 mitigations (30.0).
    pub health_cooldown_secs: f64,
    /// Seconds of no input before idle mode (30.0).
    pub idle_threshold_secs: f64,
    /// FPS multiplier while idle (0.5 = half FPS).
    pub idle_fps_factor: f64,
    /// perf_pressure increment per overshoot frame (0.25).
    pub pressure_increment: f32,
    /// perf_pressure decay per normal frame (0.02).
    pub pressure_decay: f32,
}

impl PowerThresholds {
    /// Default thresholds matching the constants above. `PowerManager`
    /// is constructed with this and then optionally tuned via
    /// `with_thresholds()` in tests.
    #[must_use]
    pub(crate) fn defaults() -> Self {
        Self {
            pressure_high: SELF_HEAL_PRESSURE_HIGH,
            pressure_low: SELF_HEAL_PRESSURE_LOW,
            downgrade_secs: SELF_HEAL_DOWNGRADE_SECS,
            restore_secs: SELF_HEAL_RESTORE_SECS,
            health_investigate: SELF_HEAL_HEALTH_INVESTIGATE,
            health_cooldown_secs: SELF_HEAL_HEALTH_COOLDOWN_SECS,
            idle_threshold_secs: IDLE_THRESHOLD_SECS,
            idle_fps_factor: IDLE_FPS_FACTOR,
            pressure_increment: PERF_PRESSURE_INCREMENT,
            pressure_decay: PERF_PRESSURE_DECAY,
        }
    }
}

impl Default for PowerThresholds {
    fn default() -> Self {
        Self::defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_thresholds_defaults_match_constants() {
        // The defaults MUST match the standalone constants — the struct is
        // the migration target, so any drift here would silently change
        // behavior when call sites switch from constants to struct fields.
        let t = PowerThresholds::defaults();
        assert!((t.pressure_high - SELF_HEAL_PRESSURE_HIGH).abs() < 1e-6);
        assert!((t.pressure_low - SELF_HEAL_PRESSURE_LOW).abs() < 1e-6);
        assert!((t.downgrade_secs - SELF_HEAL_DOWNGRADE_SECS).abs() < 1e-6);
        assert!((t.restore_secs - SELF_HEAL_RESTORE_SECS).abs() < 1e-6);
        assert!((t.health_investigate - SELF_HEAL_HEALTH_INVESTIGATE).abs() < 1e-6);
        assert!((t.health_cooldown_secs - SELF_HEAL_HEALTH_COOLDOWN_SECS).abs() < 1e-6);
        assert!((t.idle_threshold_secs - IDLE_THRESHOLD_SECS).abs() < 1e-6);
        assert!((t.idle_fps_factor - IDLE_FPS_FACTOR).abs() < 1e-6);
        assert!((t.pressure_increment - PERF_PRESSURE_INCREMENT).abs() < 1e-6);
        assert!((t.pressure_decay - PERF_PRESSURE_DECAY).abs() < 1e-6);
    }

    #[test]
    fn power_thresholds_default_trait_matches_defaults_method() {
        // Default::default() must equal PowerThresholds::defaults().
        let a = PowerThresholds::default();
        let b = PowerThresholds::defaults();
        assert!((a.pressure_high - b.pressure_high).abs() < 1e-6);
        assert!((a.idle_fps_factor - b.idle_fps_factor).abs() < 1e-6);
    }

    #[test]
    fn power_thresholds_is_copy() {
        // PowerThresholds must be Copy — future PowerManager will store it
        // by value and pass copies to sub-functions without ownership transfer.
        let a = PowerThresholds::defaults();
        let b = a; // copy, not move
        let _ = a; // a is still valid (Copy)
        assert!((a.pressure_high - b.pressure_high).abs() < 1e-6);
    }

    #[test]
    fn xtermjs_byte_budget_constants_are_sized_correctly() {
        // black_box prevents const-folding so clippy doesn't flag the
        // assertions as assertions_on_constants.
        let budget = std::hint::black_box(XTERMJS_BYTE_BUDGET_PER_WINDOW);
        let ris = std::hint::black_box(XTERMJS_RIS_RESET_BYTES);
        let ceiling = std::hint::black_box(XTERMJS_HARD_CEILING_BYTES);
        let window = std::hint::black_box(XTERMJS_BYTE_BUDGET_WINDOW_FRAMES);
        // The byte budget must be larger than a single frame's worst-case
        // output (≈200 KB at 1024×500) to avoid suppressing every flush.
        assert!(budget > 200 * 1024);
        // RIS reset must fire before the hard ceiling.
        assert!(ris < ceiling);
        // Window frames must be long enough to smooth single-frame spikes
        // but short enough to catch sustained bursts.
        assert!(window >= 100);
        assert!(window <= 10_000);
    }

    #[test]
    fn self_heal_hysteresis_gap_prevents_oscillation() {
        // black_box prevents const-folding so clippy doesn't flag.
        let high = std::hint::black_box(SELF_HEAL_PRESSURE_HIGH);
        let low = std::hint::black_box(SELF_HEAL_PRESSURE_LOW);
        let downgrade = std::hint::black_box(SELF_HEAL_DOWNGRADE_SECS);
        let restore = std::hint::black_box(SELF_HEAL_RESTORE_SECS);
        // The gap between pressure_high and pressure_low is the hysteresis
        // dead zone. Without it, the self-healer would flap on every frame
        // when pressure hovers near the boundary.
        assert!(high > low);
        // The gap must be meaningful — at least 0.2 to ride out normal
        // frame-to-frame variance.
        assert!(high - low >= 0.2);
        // Restore window must be longer than downgrade window to give the
        // restored scene a stable runway (prevents flapping).
        assert!(restore > downgrade);
    }

    #[test]
    fn idle_resync_tiers_are_monotonically_increasing() {
        // black_box prevents const-folding so clippy doesn't flag.
        let tier1 = std::hint::black_box(IDLE_REDRAW_RESYNC_INTERVAL_SECS);
        let tier2 = std::hint::black_box(IDLE_RESYNC_TIER_2_SECS);
        let tier3 = std::hint::black_box(IDLE_RESYNC_TIER_3_SECS);
        // Each idle tier must have a longer resync interval than the last.
        assert!(tier2 > tier1);
        assert!(tier3 > tier2);
    }

    #[test]
    fn fd_health_probe_matches_stuck_cell_sweep_cadence() {
        // black_box prevents const-folding so clippy doesn't flag.
        let probe = std::hint::black_box(FD_HEALTH_PROBE_INTERVAL_FRAMES);
        let sweep = std::hint::black_box(STUCK_CELL_SWEEP_INTERVAL_FRAMES);
        // Both are background hygiene passes on the same slow tick.
        // Keeping them in sync simplifies reasoning about background cost.
        assert_eq!(probe, sweep);
    }

    #[test]
    fn thermal_ramp_constants_are_well_ordered() {
        // The ramp window must be non-empty and in a physically
        // plausible range. Sanity-checked here so a typo in mod.rs
        // (e.g., 500 instead of 50) is caught at test time, not at
        // runtime when the renderer silently never throttles.
        let lo = std::hint::black_box(THERMAL_PRESSURE_ZERO_C);
        let hi = std::hint::black_box(THERMAL_PRESSURE_ONE_C);
        assert!(hi > lo, "hi ({hi}) must be > lo ({lo})");
        // Plausible CPU junction temperature range. Below 0 °C the
        // device is in a freezer; above 150 °C the silicon is dead.
        assert!((0..=100).contains(&lo));
        assert!((50..=150).contains(&hi));
    }

    #[test]
    fn thermal_sampler_interval_is_reasonable() {
        // 600 frames = 10s at 60 FPS. The sampler must NOT run more
        // often than every ~1s (would waste syscalls on slow-moving
        // thermal data) and NOT less often than every ~60s (would
        // miss a thermal ramp until well after it matters).
        let n = std::hint::black_box(THERMAL_SAMPLER_INTERVAL_FRAMES);
        assert!(n >= 60, "interval {n} too short — wastes syscalls");
        assert!(n <= 3600, "interval {n} too long — misses thermal ramps");
    }
}
