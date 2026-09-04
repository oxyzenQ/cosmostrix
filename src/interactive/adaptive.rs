// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Adaptive learning subsystem for long-endurance stability.
//!
//! This module implements five improvements derived from 72-hour endurance
//! telemetry analysis:
//!
//! - P1: Phase-Aware Adaptive Pacing (PAP) — Learns the daily activity
//!   cycle and proactively transitions to idle mode before the reactive
//!   30-second threshold fires.
//! - P2: Idle Phase Aggressive Coalescing (IPAC) — Progressively stretches
//!   the idle resync interval after sustained inactivity to reduce forced
//!   redraw CPU spikes.
//! - P4: Memory Pressure Adaptive Reclaim (MPAR) — Hints the kernel to
//!   reclaim stale frame buffer pages during idle, smoothing RSS step-downs.
//! - P5: Endurance Health Score (EHS) — A single 0–100 metric tracking
//!   memory stability, frame jitter, and context switch rate.
//!
//! P3 (Context Switch Batching) is handled at the Terminal level via its
//! existing BufWriter; no additional code is needed here.
//!
//! All subsystems are zero-allocation, single-threaded, and backward-compatible
//! with the existing architecture invariants.

pub(crate) use crate::constants::*;

// ────────────────────────────────────────────────────────────────────────────
// P1: Phase-Aware Adaptive Pacing — re-exported from central_control_power_dragon
// ────────────────────────────────────────────────────────────────────────────
//
// PhasePredictor + local_secs_since_midnight migrated to
// `crate::central_control_power_dragon::phase_predictor` (Phase 2
// consolidation). Re-exported here via the `pub(crate) use crate::constants::*`
// glob (constants.rs re-exports dragon_power, which re-exports phase_predictor).
// Existing `use super::adaptive::*` imports in event_loop.rs continue to
// resolve without call-site changes. Once all consumers migrate to importing
// directly from `crate::central_control_power_dragon::*`, this shim will
// be removed.

// ────────────────────────────────────────────────────────────────────────────
// P2 + P4: Re-exported from central_control_power_dragon::reclaim_state
// ────────────────────────────────────────────────────────────────────────────
//
// adaptive_resync_interval (P2 IPAC) + hint_reclaim_pages (P4 MPAR) +
// ReclaimState struct migrated to
// `crate::central_control_power_dragon::reclaim_state` (Phase 2
// consolidation). Re-exported here via the `pub(crate) use crate::constants::*`
// glob (constants.rs re-exports dragon_power, which re-exports reclaim_state).
// Existing `use super::adaptive::{...}` imports in event_loop.rs continue to
// resolve without call-site changes.

// ────────────────────────────────────────────────────────────────────────────
// P5: Endurance Health Score — re-exported from central_control_power_dragon
// ────────────────────────────────────────────────────────────────────────────
//
// EnduranceHealth struct + impl + Default migrated to
// `crate::central_control_power_dragon::endurance_health` (Phase 2
// consolidation). Re-exported here via the `pub(crate) use crate::constants::*`
// glob (constants.rs re-exports dragon_power, which re-exports
// endurance_health). Existing `use super::adaptive::{...}` imports in
// event_loop.rs continue to resolve without call-site changes.

// ────────────────────────────────────────────────────────────────────────────
// Performance Self-Healer (P1 + P2) — re-exported from central_control_power_dragon
// ────────────────────────────────────────────────────────────────────────────
//
// SelfHealAction enum + PerformanceSelfHealer struct + impl + Default
// migrated to `crate::central_control_power_dragon::self_healer` (Phase 2
// consolidation). Re-exported here via the `pub(crate) use crate::constants::*`
// glob (constants.rs re-exports dragon_power, which re-exports self_healer).
// Existing `use super::adaptive::{...}` imports in event_loop.rs continue to
// resolve without call-site changes.
//
// All P1/P2/P4/P5 behavior code has now been migrated to submodules of
// central_control_power_dragon/. This file is a thin re-export shim only.

#[cfg(test)]
mod tests {
    // All tests migrated to submodules of central_control_power_dragon/:
    //  - phase_predictor.rs (5 tests: P1 PhasePredictor)
    //  - reclaim_state.rs   (5 tests: P2 resync_interval + P4 ReclaimState)
    //  - endurance_health.rs (5 tests: P5 EnduranceHealth)
    //  - self_healer.rs     (15 tests: P1+P2 PerformanceSelfHealer)
    //
    // Each submodule's #[cfg(test)] block has direct access to private
    // fields (active_start_ema, rss_idx, high_pressure_since, etc.) so
    // the tests can assert on internal state without exposing it through
    // public getters.
    //
    // This empty tests module is kept so future adaptive-specific tests
    // (if any are added) have a natural home here.
}
