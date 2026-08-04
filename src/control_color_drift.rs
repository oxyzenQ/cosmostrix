// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Owner-editable control file for `--auto-color-drift` system feeling.
//!
//! This is the **single taste file** for signal-driven palette drift. The
//! runtime logic lives in `system_feeling.rs` and `cloud/ecosystem.rs`; this
//! file only holds the numbers and the state→family mapping that the owner
//! can tune without touching algorithm code.
//!
//! ## What lives here
//!
//! - [`FeelingState`] — the 5 emotional states the classifier emits.
//! - Threshold constants for CPU% and time-of-day boundaries.
//! - Hysteresis / smoothing knobs.
//! - [`family_for_state`] — which [`ColorFamily`] each state drifts toward.
//!
//! ## What does NOT live here
//!
//! - The `ColorFamily` enum itself (lives in `cloud/ecosystem.rs` with the
//!   color graph data).
//! - The `family_members(family)` table (same place — it's graph data).
//! - Signal sampling code (lives in `system_feeling.rs`).
//! - The `tick()` integration (lives in `cloud/ecosystem.rs`).
//!
//! ## Editing guide
//!
//! To retune sensitivity: change `CPU_BUSY_THRESHOLD` / `CPU_IDLE_THRESHOLD`.
//! To retune time windows: change the `*_START` / `*_END` constants.
//! To remap which colors go with which mood: edit `family_for_state`.
//! To change how sticky a state is: edit `MIN_STATE_DWELL_SECS`.
//! To change CPU smoothing aggressiveness: edit `CPU_EMA_ALPHA`.
//!
//! Nothing else in the codebase defines taste constants for system feeling.
//! If you find yourself adding thresholds elsewhere, move them here instead.

use crate::cloud::ecosystem::ColorFamily;

// ── Emotional states ──────────────────────────────────────────────────────

/// The 5 emotional states the system feeling classifier emits.
///
/// These map 1:1 onto the existing 5-phase atmosphere taxonomy so the
/// signal-driven drift and the time-driven atmosphere engine speak the
/// same language. Order is stable for indexing but the classifier does
/// not rely on ordering — `family_for_state` is a `match`, not a lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FeelingState {
    /// Daytime + low CPU. System is breathing freely.
    Calm,
    /// Morning hours + low-mid CPU. Fresh, sparse energy.
    Pulse,
    /// High CPU at any hour. System is working hard — urgency, heat.
    Signal,
    /// Deep night + low CPU. Silent space, glitchy.
    Void,
    /// Pre-dawn + non-idle CPU. Pressure before dawn, dense and dark.
    Compression,
}

impl FeelingState {
    /// Stable string label for diagnostics (`--doctor`, logs).
    pub(crate) fn label(self) -> &'static str {
        match self {
            FeelingState::Calm => "calm",
            FeelingState::Pulse => "pulse",
            FeelingState::Signal => "signal",
            FeelingState::Void => "void",
            FeelingState::Compression => "compression",
        }
    }
}

impl Default for FeelingState {
    /// Default state on cold start. Calm is the safest neutral — it
    /// maps to the BlueWater family which is visually unobtrusive.
    fn default() -> Self {
        FeelingState::Calm
    }
}

// ── CPU thresholds (percent) ──────────────────────────────────────────────

/// CPU% at or above this counts as "busy" → pushes toward Signal/Compression.
pub(crate) const CPU_BUSY_THRESHOLD: f32 = 50.0;

/// CPU% at or below this counts as "idle" → allows Calm/Void/Pulse.
pub(crate) const CPU_IDLE_THRESHOLD: f32 = 15.0;

// ── Time-of-day boundaries (local hour, 0.0–24.0) ────────────────────────

/// Hour when "night" begins (22:00 local). Hours >= this or < NIGHT_END
/// count as night.
pub(crate) const NIGHT_START: f64 = 22.0;

/// Hour when "night" ends (06:00 local).
pub(crate) const NIGHT_END: f64 = 6.0;

/// Start of the pre-dawn compression window (03:00 local).
pub(crate) const PRE_DAWN_START: f64 = 3.0;

/// End of the pre-dawn compression window (06:00 local).
pub(crate) const PRE_DAWN_END: f64 = 6.0;

/// Start of the morning pulse window (06:00 local).
pub(crate) const MORNING_START: f64 = 6.0;

/// End of the morning pulse window (12:00 local).
pub(crate) const MORNING_END: f64 = 12.0;

// ── Hysteresis / smoothing ────────────────────────────────────────────────

/// Minimum seconds in the current state before a transition is allowed.
/// Prevents flicker between states when CPU% hovers near a threshold.
/// At 60s, the state can change at most once per minute.
pub(crate) const MIN_STATE_DWELL_SECS: f32 = 60.0;

/// EMA alpha for CPU% smoothing. 0.0 = frozen (never update), 1.0 = raw
/// sample (no smoothing). 0.3 means ~70% weight on history, ~30% on new
/// sample — smooths 3-second sampling jitter without lagging too far
/// behind real load changes.
pub(crate) const CPU_EMA_ALPHA: f32 = 0.3;

// ── State → color family mapping ──────────────────────────────────────────

/// Map a feeling state to its target color family.
///
/// This is the **only** place where mood → color is decided. Edit this
/// match to remap which family each state drifts toward. The families
/// themselves are defined in `cloud/ecosystem.rs::family_members`.
///
/// Current mapping rationale:
///
/// | State | Family | Why |
/// |-------|--------|-----|
/// | Calm | BlueWater | Cool, unobtrusive — system breathing freely |
/// | Pulse | Green | Fresh Matrix green — morning energy |
/// | Signal | RedFire | Hot reds/oranges — system under load |
/// | Void | PurpleNebula | Deep cosmic purples — silent night |
/// | Compression | GrayMoon | Neutral grays — pre-dawn pressure |
#[must_use]
pub(crate) const fn family_for_state(state: FeelingState) -> ColorFamily {
    match state {
        FeelingState::Calm => ColorFamily::BlueWater,
        FeelingState::Pulse => ColorFamily::Green,
        FeelingState::Signal => ColorFamily::RedFire,
        FeelingState::Void => ColorFamily::PurpleNebula,
        FeelingState::Compression => ColorFamily::GrayMoon,
    }
}
