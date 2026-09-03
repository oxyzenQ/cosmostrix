// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Crystal Dragon Engine configuration.
//!
//! Holds the tuning knobs that control how the Crystal Dragon engine
//! samples the system and selects color themes. v80.0.0-alpha.1: the
//! polling interval is now user-tunable via `--crystal-dragon-secs` /
//! the `crystal-dragon-secs` config key (range 0.0..=86400.0) — the
//! harmony twin of `ambient-snapback-secs`. Every other knob remains
//! an owner-chosen constant (silent-elegant mode: not config-exposed,
//! deliberately — see the over-engineering guard notes per field).

// ── Polling interval ─────────────────────────────────────────────────────

/// Default sensor polling interval: 60 seconds.
///
/// The owner chose 60 s for the silent-elegant aesthetic (Option A).
/// At 60 s, the engine checks CPU or CLOCK once per minute and may
/// (probabilistically) transition to a new color theme. This is slow
/// enough to feel organic rather than mechanical, and fast enough to
/// react to real load changes within a minute.
///
/// v80.0.0-alpha.1: this const seeds the DEFAULT only — the runtime
/// value lives in `CrystalDragonControl.polling_secs`, overridden by
/// `--crystal-dragon-secs` / `crystal-dragon-secs` (CLI > config >
/// default; live-reload re-applies on edit). Applied in
/// `CloudConfig::create_cloud`; the drift-cycle self-reset in
/// `post_rain.rs` follows the configured value, not this const.
pub(crate) const CRYSTAL_DRAGON_POLLING_SECS: f32 = 60.0;

/// Minimum dwell time in the current color theme before a transition
/// is allowed. Prevents flicker when CPU% hovers near a group boundary.
/// At 60 s, the theme can change at most once per minute even if
/// the sensor reports a different group on consecutive polls.
///
/// v80.0.0-alpha.1 (S-master-HUNT-3, owner bug: "--crystal-dragon-secs 6 still used 60s"):
/// this const is the floor at the DEFAULT 60 s cadence — NOT an absolute
/// lock. `CloudConfig::create_cloud` applies
/// `min(CRYSTAL_DRAGON_MIN_DWELL_SECS, polling_secs)`: an explicit
/// faster user cadence lowers the dwell floor to match (the knob must
/// be real, not a gimmick pinned at 60 s), a slower cadence keeps the
/// 60 s floor (the poll timer paces it — enforced for real by the
/// S-master-HUNT-6 poll gate: the drift decision only runs on a poll
/// boundary). Anti-flicker for the untuned case, obedience for the
/// tuned case.
pub(crate) const CRYSTAL_DRAGON_MIN_DWELL_SECS: f32 = 60.0;

// ── Probabilistic drift chance ───────────────────────────────────────────

/// Probability (0..1) that a drift tick actually triggers a palette drift.
///
/// v80.0.0-alpha.1 (S-master-HUNT-6, owner bug 2026-09-03: "the 10m knob
/// still drifts every 60s + bursts in milliseconds on config edits"):
/// `crystal_dragon_tick` runs every frame, but the drift DECISION block
/// only runs on a tick where the polling interval actually elapsed (the
/// poll gate) — so this chance is evaluated once per POLL BOUNDARY, and
/// the cadence governor is `polling_secs` (+ the dwell floor and the
/// drift-cycle visibility window in post_rain.rs), NOT this value.
/// 0.12 means a drift fires within moments of a boundary becoming
/// dwell-eligible, which keeps the timing slightly unpredictable —
/// cinematic without changing the cadence contract. (Pre-HUNT-6 the
/// dice rolled every frame and the effective cadence was the 60s dwell
/// floor regardless of a slower `polling_secs` — the exact bug the
/// owner reported.)
pub(crate) const CRYSTAL_DRAGON_DRIFT_CHANCE: f32 = 0.12;

// ── EMA smoothing ────────────────────────────────────────────────────────

/// EMA alpha for CPU% smoothing. 0.0 = frozen, 1.0 = raw sample.
/// 0.25 means ~75% weight on history, ~25% on new sample — smooths
/// per-minute sampling jitter without lagging far behind real load.
pub(crate) const CRYSTAL_DRAGON_CPU_EMA_ALPHA: f32 = 0.25;

// ── Stack-allocated CDF capacity ─────────────────────────────────────────

/// Z-master-1X round 9 masterclass: maximum themes per temperature group.
/// Used to size the stack-allocated `[f32; N]` arrays in `calc_v1_select`
/// so the drift path avoids heap allocation entirely. Groups have exactly
/// 14 themes; 16 covers that + the 2 reserved themes defensively.
pub(crate) const CRYSTAL_DRAGON_MAX_THEMES_PER_GROUP: usize = 16;

// ── Sensor mode ──────────────────────────────────────────────────────────

/// Sensor input mode for the Crystal Dragon engine.
///
/// CPU mode is primary (reads process CPU% via sysinfo/procfs).
/// CLOCK mode is the fallback (derives a point from UTC time-of-day)
/// when CPU sampling is unsupported on the current platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CrystalDragonSensorMode {
    /// Read process CPU% and map to point 1–99.
    Cpu,
    /// Derive point from UTC hour + minute (no CPU dependency).
    Clock,
}

// ── Calc method ──────────────────────────────────────────────────────────

/// Calculation method for theme selection within a temperature group.
///
/// calc-v1 (probabilistic weighted selection) shipped with the initial
/// Crystal Dragon release. Since the Dragon Engine v2 upgrade (merge
/// d55442d) calc-v2 (pattern state machine with recency memory) is
/// implemented and is the DEFAULT — see `point_system::calc_v2_select`
/// and the DriftHistory ring buffer. calc-v1 is preserved as the
/// legacy option.
///
/// `allow(dead_code)`: the legacy `Calc` variant is matched in
/// `runtime_controls::crystal_dragon_tick` but only constructed in
/// tests — deliberately preserved, not zombie.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum CrystalDragonCalcMethod {
    /// Probabilistic weighted selection: themes closer to the current
    /// point receive higher weight, but any theme in the group can be
    /// selected. This produces organic, unpredictable transitions.
    Calc,
    /// Pattern state machine with recency memory (implemented in
    /// Dragon Engine v2; the default since that merge). Applies a
    /// recency penalty to recently selected themes, preventing
    /// A->B->A oscillation — see `point_system::DriftHistory`.
    CalcV2,
}

// ── Config struct ────────────────────────────────────────────────────────

/// Configuration for the Crystal Dragon engine.
///
/// All fields use the owner-chosen defaults. This struct exists so
/// future CLI/config-file exposure can override them without changing
/// the engine code. S-master-1-v2: every field is now read at runtime
/// (drift_chance via `crystal_dragon_tick`, cpu_ema_alpha via the
/// sensor's EMA) — the former duplicate const shadowing is gone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CrystalDragonControl {
    /// Sensor polling interval in seconds.
    pub polling_secs: f32,
    /// Minimum seconds in current theme before transition allowed.
    pub min_dwell_secs: f32,
    /// Probability that a poll tick triggers a drift event.
    pub drift_chance: f32,
    /// EMA alpha for CPU% smoothing.
    pub cpu_ema_alpha: f32,
    /// Active sensor mode (CPU or CLOCK).
    pub sensor_mode: CrystalDragonSensorMode,
    /// Active calc method (Calc or CalcV2).
    pub calc_method: CrystalDragonCalcMethod,
}

impl Default for CrystalDragonControl {
    fn default() -> Self {
        Self {
            polling_secs: CRYSTAL_DRAGON_POLLING_SECS,
            min_dwell_secs: CRYSTAL_DRAGON_MIN_DWELL_SECS,
            drift_chance: CRYSTAL_DRAGON_DRIFT_CHANCE,
            cpu_ema_alpha: CRYSTAL_DRAGON_CPU_EMA_ALPHA,
            sensor_mode: CrystalDragonSensorMode::Cpu,
            calc_method: CrystalDragonCalcMethod::CalcV2,
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
