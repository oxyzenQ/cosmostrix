// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Dragon Engine v2 depth-verify: adaptive phosphor ("the rain breathes
//! with your CPU").
//!
//! The v2 merge (d55442d) shipped the adaptive decay factor (0.8 at idle
//! ramping to 1.0 at pressure 0.30, composing with the 1.2x pressure boost)
//! but ZERO tests for it. These tests are the missing proof: seed one stale
//! phosphor cell with identical energy, run one decay pass at different
//! pressures, and compare what survives.
//!
//! Setup notes (why the fixture looks unusual):
//! - The cell is seeded directly into `phosphor` + `phosphor_active` with
//!   the fresh bit clear, so Pass 3's exponential decay branch is reached
//!   without needing a real droplet to move away first.
//! - `frame.clear_with_bg()` bumps the frame generation WITHOUT rewriting
//!   cell generations, so the seeded cell is not current-gen: this avoids
//!   the `is_blank_current_gen` fast path (which would force the cell to
//!   PHOSPHOR_TAIL_RESIDUAL and mask the adaptive difference).

use std::time::Instant;

use super::make_cloud;
use crate::constants::{
    PHOSPHOR_BOTTOM_DECAY_MULT, PHOSPHOR_DECAY_RATE, PHOSPHOR_LAYER_DECAY_MULT,
};
use crate::frame::Frame;

/// Seed one stale phosphor cell at (col=0, line=5) with `energy` and run a
/// single 50 ms decay pass at `pressure`. Returns the surviving energy.
fn decayed_energy(pressure: f32, energy: u8) -> u8 {
    let mut cloud = make_cloud();
    cloud.perf_pressure = pressure;

    let pidx = 5usize; // col 0, line 5 (not bottom, not edge-fade)
    cloud.phosphor[pidx] = energy;
    cloud.phosphor_layer[pidx] = 0; // layer 0 — the fastest layer
    cloud.phosphor_active.push(pidx);
    cloud.phosphor_in_active.set(pidx, true);
    assert!(!cloud.phosphor_fresh[pidx], "seeded cell must start stale");

    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    frame.clear_with_bg(cloud.palette.bg); // bump gen: seeded cell is now stale-gen
    cloud.phosphor_decay_pass(&mut frame, 0.05);

    cloud.phosphor[pidx]
}

#[test]
fn phosphor_v2_adaptive_decay_slower_at_idle_than_loaded() {
    // Identical seed, identical elapsed: the idle trail (adaptive 0.8,
    // no boost) must retain MORE energy than the loaded trail (adaptive
    // 1.0, pressure boost 1.2). This is the "trails breathe with CPU"
    // contract — beautiful at calm, short under load.
    let idle = decayed_energy(0.0, 200);
    let loaded = decayed_energy(0.35, 200);
    assert!(
        idle > loaded,
        "idle trails must decay slower: idle energy {idle} vs loaded {loaded}"
    );
}

#[test]
fn phosphor_v2_adaptive_ramp_is_monotonic() {
    // The 0.8 -> 1.0 linear ramp means energy retention decreases
    // monotonically across idle -> mid -> loaded.
    let idle = decayed_energy(0.0, 200);
    let mid = decayed_energy(0.15, 200); // adaptive 0.9
    let loaded = decayed_energy(0.35, 200); // adaptive 1.0 + boost 1.2
    assert!(idle > mid, "idle {idle} must beat mid {mid}");
    assert!(mid > loaded, "mid {mid} must beat loaded {loaded}");
}

#[test]
fn phosphor_v2_skip_gate_above_high_threshold() {
    // M1 hysteresis gate (pre-v2, pinned here as the envelope boundary):
    // above PHOSPHOR_SKIP_HIGH the whole pass is skipped — energy is
    // untouched. The adaptive ramp only lives BELOW the pressure-boost
    // threshold; this test pins the upper envelope.
    let skipped = decayed_energy(0.75, 200);
    assert_eq!(
        skipped, 200,
        "pressure 0.75 > PHOSPHOR_SKIP_HIGH must skip the decay pass entirely"
    );
}

#[test]
fn phosphor_v2_adaptive_does_not_resurrect_dead_cells() {
    // Sanity: a dead cell (energy 0) stays dead at idle — the adaptive
    // factor slows decay, it never re-energizes.
    let idle = decayed_energy(0.0, 0);
    assert_eq!(idle, 0);
}

/// Cross-check against the documented math: at idle the decay factor is
/// PHOSPHOR_DECAY_RATE * 0.8 (adaptive) * elapsed * layer0_mult, so a
/// 200-energy cell after one 50 ms pass must equal the same product the
/// production code computes (this 20x10 fixture puts every row inside the
/// PHOSPHOR_BOTTOM_ROWS zone, so the bottom multiplier applies too).
/// Locking the exact value keeps future "tuning" from silently changing
/// the breathing amplitude (owner-visible visual contract).
#[test]
fn phosphor_v2_idle_decay_matches_documented_math() {
    let idle = decayed_energy(0.0, 200);
    let expected = (200.0f32
        * (-(PHOSPHOR_DECAY_RATE
            * 0.8
            * PHOSPHOR_LAYER_DECAY_MULT[0]
            * PHOSPHOR_BOTTOM_DECAY_MULT
            * 0.05))
            .exp()) as u8;
    assert_eq!(
        idle, expected,
        "idle decay must match the documented formula"
    );
    // And the amplitude must be owner-visible: 200 -> ~67, a clear drop.
    assert!(
        idle > 50 && idle < 90,
        "idle energy {idle} out of expected band"
    );
}

#[test]
fn phosphor_v2_last_phosphor_time_untouched_by_pass() {
    // Contract note: the pass itself never reads or writes
    // last_phosphor_time (the caller owns frame pacing). Pin it so the
    // adaptive factor cannot silently become time-dependent.
    let mut cloud = make_cloud();
    cloud.perf_pressure = 0.0;
    let t0 = Instant::now();
    cloud.last_phosphor_time = t0;
    let pidx = 5usize;
    cloud.phosphor[pidx] = 200;
    cloud.phosphor_active.push(pidx);
    cloud.phosphor_in_active.set(pidx, true);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    frame.clear_with_bg(cloud.palette.bg);
    cloud.phosphor_decay_pass(&mut frame, 0.05);
    assert_eq!(
        cloud.last_phosphor_time, t0,
        "decay pass must not mutate the caller-owned pacing timestamp"
    );
}
