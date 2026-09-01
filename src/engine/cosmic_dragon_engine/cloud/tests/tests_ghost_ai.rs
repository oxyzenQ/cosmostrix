// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Dragon Engine v2 depth-verify: Ghost AI pressure-scaled spawning.
//!
//! The v2 merge (d55442d) shipped pressure-scaled ghost spawn probability
//! (ghosts as a living system health indicator) but ZERO tests for it.
//! These tests are the missing proof that the ghost AI is real and working:
//! frequent ghosts at calm, zero ghosts at the perf gate, and the linear
//! ramp keeps mid-pressure spawns alive.

use std::time::Instant;

use super::super::ghost_events::GhostEventScheduler;
use crate::constants::{EVENT_PERF_GATE, GHOST_SPAWN_CHANCE_PER_TICK};

/// Drive the scheduler for `ticks` frames at a fixed pressure and return
/// how many ghost events are active afterwards.
fn run_ticks(pressure: f32, ticks: usize) -> usize {
    let mut scheduler = GhostEventScheduler::new(Instant::now());
    scheduler.enable_events();
    for _ in 0..ticks {
        scheduler.evaluate_triggers(pressure, 80, 24, false, false);
    }
    scheduler.ghost_count()
}

#[test]
fn ghost_ai_hard_gate_blocks_spawn_under_extreme_pressure() {
    // Above the perf gate the hard gate returns before any RNG work —
    // ghosts are a health indicator, and a saturated system shows none.
    let count = run_ticks(0.9, 2_000);
    assert_eq!(count, 0, "no ghosts may spawn above EVENT_PERF_GATE");
}

#[test]
fn ghost_ai_zero_spawn_chance_at_exact_gate_pressure() {
    // At exactly EVENT_PERF_GATE the linear ramp hits 0.0: the hard gate
    // (p > gate) does not fire, but the pressure factor does. This pins
    // the ramp endpoint — the "busy system" boundary.
    let count = run_ticks(EVENT_PERF_GATE, 5_000);
    assert_eq!(
        count, 0,
        "spawn chance at exactly the gate must be 0 (linear ramp endpoint)"
    );
}

#[test]
fn ghost_ai_spawns_during_calm_pressure() {
    // At 0% pressure the full GHOST_SPAWN_CHANCE_PER_TICK (0.003) applies.
    // P(no spawn in 5_000 ticks) = 0.997^5000 ≈ 3e-7 — deterministic enough.
    let count = run_ticks(0.0, 5_000);
    assert!(
        count >= 1,
        "calm system must spawn ghosts (full chance per tick = {GHOST_SPAWN_CHANCE_PER_TICK})"
    );
}

#[test]
fn ghost_ai_still_spawns_at_mid_pressure() {
    // Halfway to the gate the chance is halved (0.0015/tick) but non-zero:
    // the ramp must be a smooth slope, not a cliff that dies mid-range.
    // P(no spawn in 10_000 ticks) ≈ e^-15 ≈ 3e-7.
    let count = run_ticks(EVENT_PERF_GATE * 0.5, 10_000);
    assert!(
        count >= 1,
        "mid-pressure must keep spawning (linear ramp midpoint alive)"
    );
}

#[test]
fn ghost_ai_paused_never_spawns() {
    // Pause gate: ghosts respect the frozen-frame contract.
    let mut scheduler = GhostEventScheduler::new(Instant::now());
    scheduler.enable_events();
    for _ in 0..5_000 {
        scheduler.evaluate_triggers(0.0, 80, 24, true, false);
    }
    assert_eq!(
        scheduler.ghost_count(),
        0,
        "paused cloud must not spawn ghosts"
    );
}

#[test]
fn ghost_ai_transition_never_spawns() {
    // Scene-transition gate: no ghosts mid-transition (visual noise).
    let mut scheduler = GhostEventScheduler::new(Instant::now());
    scheduler.enable_events();
    for _ in 0..5_000 {
        scheduler.evaluate_triggers(0.0, 80, 24, false, true);
    }
    assert_eq!(
        scheduler.ghost_count(),
        0,
        "in-transition cloud must not spawn ghosts"
    );
}
