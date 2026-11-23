// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! The production-draw zero-alloc ledger (INV-25, night-netrunner-Z1).
//!
//! INV-18 proved the LEAN window (sim + diff + `emit_cell_lean`)
//! allocates exactly zero bytes per steady-state frame. Its own doc
//! honestly named the remainder: "the production-draw I/O path
//! (Terminal::draw) carries ~1.1 allocs/frame by design and is NOT
//! this contract." This module closes that remainder for the window
//! the `--bench-scene production-draw` benchmark actually measures —
//! the lens extension of the Z11/Z13 family (the zelynic estate
//! law): the ceiling lowers in the same commit that closes a named
//! site.
//!
//! The named site, found by reading the mirror against its
//! production twin: `write_frame_production` allocated a fresh
//! `Vec::with_capacity(width * 4)` row buffer EVERY frame — a heap
//! event the real `Terminal::draw` does not carry (its row_buf and
//! ansi_buf are struct fields, warm since the first frame). The
//! benchmark was measuring an allocation its own production path
//! does not make — a bench-vs-production drift, closed by moving the
//! buffer into a struct field (`production_row_buf`) exactly like
//! the Terminal's own spelling.
//!
//! One ledger frame = `Cloud::rain_at` (the sim + diff step)
//!                     + `BenchIoWriter::write_frame_production`
//!                       (the production-draw mirror: MoveTo per row,
//!                       SGR through the cache/quantizer, BOLT bold,
//!                       RLE run batching)
//!                     + `Frame::clear_dirty`.
//!
//! Honest scope, stated once: this window covers the BENCH MIRROR of
//! the production draw path — the exact code `--bench-scene
//! production-draw` measures, warm buffers included. The real
//! `Terminal::draw` through live stdout additionally carries the
//! once-per-needs_clear bg-blanket scratch (a 16-byte window outside
//! every steady-state frame, by construction: the blanket only fires
//! on a dimension change) and the OS's own pipe buffering, neither
//! of which any in-process lens can or should pin. The claim this
//! ledger earns: the production-draw emission window allocates zero
//! heap bytes per steady-state frame, machine-pinned — the same
//! contract INV-18 holds for the lean window, now on both emission
//! paths the benchmark can name.

use std::time::{Duration, Instant};

use crate::alloc_trace::AllocSnapshot;
use crate::bench::bench_io::BenchIoWriter;

use super::alloc_ledger::{
    LEDGER_FRAMES, LEDGER_WARMUP_FRAMES, build_ledger_rig,
};

/// INV-25: the production-draw window performs EXACTLY ZERO heap
/// allocations per frame in steady state.
///
/// Same window shape as INV-18 (warmup excluded, per-thread
/// attribution so concurrent test threads cannot pollute the delta),
/// with the writer routed through `write_frame_production` — the
/// `--bench-scene production-draw` mirror. The first frame may warm
/// `production_row_buf` (a one-time capacity reservation, the same
/// warmup law the lean pools follow); the steady-state window must
/// be exactly flat: alloc == 0, dealloc == 0, realloc == 0.
#[test]
fn lock_inv25_production_draw_window_performs_zero_allocations() {
    let (mut cloud, mut frame, mut writer) = build_ledger_rig();
    let target_period = Duration::from_secs_f64(1.0 / 60.0);

    let mut sim_now = Instant::now();
    for _ in 0..LEDGER_WARMUP_FRAMES {
        sim_now += target_period;
        cloud.rain_at(&mut frame, sim_now);
        writer.write_frame_production(&frame);
        frame.clear_dirty();
    }

    // The window opens on the same per-thread counters INV-18 trusts.
    let before = AllocSnapshot::now();
    for _ in 0..LEDGER_FRAMES {
        sim_now += target_period;
        cloud.rain_at(&mut frame, sim_now);
        writer.write_frame_production(&frame);
        frame.clear_dirty();
    }
    let after = AllocSnapshot::now();

    let delta = after.delta(&before);
    assert_eq!(
        delta.alloc_calls, 0,
        "the production-draw window allocated {} times across {LEDGER_FRAMES} frames \
         ({} bytes) — the zero-alloc contract is broken on the production \
         emission path; a per-frame heap allocation (a fresh Vec, String, or \
         pool spill) landed in the sim/diff/production-emit path",
        delta.alloc_calls, delta.bytes_allocated_total
    );
    assert_eq!(
        delta.dealloc_calls, 0,
        "the production-draw window deallocated {} times across {LEDGER_FRAMES} \
         frames — steady-state buffers must neither grow nor shrink per frame",
        delta.dealloc_calls
    );
    assert_eq!(
        delta.realloc_calls, 0,
        "the production-draw window reallocated {} times across {LEDGER_FRAMES} \
         frames — capacity growth must be warmup-only, never per-frame",
        delta.realloc_calls
    );
    assert_eq!(
        delta.heap_retained_bytes, 0,
        "the production-draw window retained {} heap bytes — the steady-state \
         heap footprint must be flat",
        delta.heap_retained_bytes
    );
}

/// The ledger's teeth, the INV-18 positive-control law: the SAME
/// window shape with one intentional per-frame allocation must count
/// exactly LEDGER_FRAMES allocations. If this fires low, the ledger
/// is not measuring this thread's window and the zero-alloc contract
/// above is unenforceable.
#[test]
fn lock_inv25_ledger_counts_intentional_per_frame_allocations() {
    let (mut cloud, mut frame, mut writer) = build_ledger_rig();
    let target_period = Duration::from_secs_f64(1.0 / 60.0);

    let mut sim_now = Instant::now();
    for _ in 0..LEDGER_WARMUP_FRAMES {
        sim_now += target_period;
        cloud.rain_at(&mut frame, sim_now);
        writer.write_frame_production(&frame);
        frame.clear_dirty();
    }

    let before = AllocSnapshot::now();
    for _ in 0..LEDGER_FRAMES {
        sim_now += target_period;
        cloud.rain_at(&mut frame, sim_now);
        writer.write_frame_production(&frame);
        frame.clear_dirty();
        // The deliberate regression: one heap allocation per frame.
        let leak = vec![0u8; 64];
        std::hint::black_box(&leak);
    }
    let after = AllocSnapshot::now();

    let delta = after.delta(&before);
    assert_eq!(
        delta.alloc_calls, LEDGER_FRAMES,
        "the positive control must count exactly one allocation per frame \
         (counted {} over {LEDGER_FRAMES} frames) — if this fires low, the \
         ledger is not measuring this thread's window and the zero-alloc \
         contract is unenforceable",
        delta.alloc_calls
    );
}
