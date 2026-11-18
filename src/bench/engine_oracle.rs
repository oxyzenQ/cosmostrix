// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! The embedded engine oracle — the binary proves its own render
//! contract before it signs anything (night-frontier-Z1, Proof Moat
//! 1c).
//!
//! The lock suites live in CI; the artifact a buyer runs carried no
//! engine proof at all — until now. This module renders a PINNED
//! synthetic sequence in-memory (a 60x20 monolith, 16 frames at 60
//! fps on the anchored virtual clock, exactly the replay-oracle
//! machinery of entry 32), folds the per-frame blake3 digests into
//! one sequence digest, and compares it against a compile-time pinned
//! vector. `sig` reports the result as depth D4 (engine-oracle) and
//! `--doctor` surfaces the same verdict line — the depth matrix rises
//! from an identity proof (who built this) to a capability proof
//! (what this binary actually draws).
//!
//! Cost discipline, stated honestly: milliseconds, single-threaded,
//! no terminal I/O, no threads, no blocking. The only heap footprint
//! is the pinned 60x20 synthetic arena (the render needs its state)
//! plus one reused scratch buffer — the digest fold itself is
//! allocation-free on top of that buffer.
//!
//! Binding, stated honestly: the vector pins the pinned-toolchain
//! release lane (the exact shape the release ships). A build from a
//! different profile or toolchain that renders the pinned sequence
//! differently reports engine-oracle FAIL — that is the moat's tooth:
//! `sig` certifies the release-lane render contract, and a binary
//! that fails it is not the certified artifact regardless of how
//! pristine its identity layers are. The vector is regenerated with
//! the pinned toolchain when the engine legitimately changes (the
//! same version-bound law as the provenance anchor and the replay
//! digests); the regeneration command lives in the module tests.

use std::time::{Duration, Instant};

use crate::engine::cosmic_dragon_engine::cloud::Cloud;
use crate::frame::Frame;
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};

use super::frame_digest::{fold_sequence, frame_digest, hex32};

/// The pinned synthetic sequence length.
///
/// 16 frames at 60 fps = 0.27 virtual seconds — long enough to cross
/// spawn cadence boundaries and phosphor decay steps, short enough
/// that `sig` and `--doctor` stay millisecond-scale.
pub(crate) const ORACLE_FRAMES: u64 = 16;

/// The pinned virtual screen for the oracle sequence.
const ORACLE_COLS: u16 = 60;
const ORACLE_LINES: u16 = 20;

/// The pinned frame period (60 fps — the canonical virtual clock).
///
/// `Duration::from_nanos` is the const-fn form of
/// `Duration::from_secs_f64(1.0 / 60.0)`: 1/60 s rounds to exactly
/// 16,666,667 ns, the same value the replay path computes at runtime,
/// so the oracle's clock arithmetic matches the replay law byte-for-byte.
const ORACLE_PERIOD: Duration = Duration::from_nanos(16_666_667);

/// The compile-time pinned engine vector (Proof Moat 1c).
///
/// blake3 fold over the 16-frame pinned sequence, rendered by the
/// pinned toolchain on the release lane. Empirically cross-verified
/// (night-frontier-Z1): the debug-glibc build and the
/// pro-linux-amd64-v3-musl release lane recompute the SAME digest
/// (IEEE float semantics hold across profile, libc, and target-cpu),
/// so one vector serves both the test build and the shipped shape.
/// Regeneration law: render the sequence with the pinned toolchain
/// (the release lane), then update this constant — the module test
/// `pinned_vector_matches_the_current_engine` fails until the
/// constant matches, so a stale vector can never pass CI silently.
pub(crate) const ENGINE_ORACLE_DIGEST: [u8; 32] = [
    0xd0, 0x0a, 0x46, 0x0a, 0x91, 0x19, 0xe0, 0xa3, 0xfd, 0xa7, 0xc3, 0x34, 0x7a, 0x19, 0x1a, 0xb3,
    0xc2, 0xc8, 0xad, 0x23, 0x33, 0xad, 0x0d, 0x25, 0xd6, 0x9a, 0x9a, 0xc7, 0x4e, 0x24, 0x6c, 0x4a,
];

/// Render the pinned sequence and return its folded digest.
///
/// Deterministic by construction: the fixed launch seed drives the
/// RNG, [`Cloud::anchor_virtual_clock`] kills every wall-clock
/// anchor, and the pinned configuration fixes everything else. The
/// oracle forces the Crystal Dragon off (ambient polling reads host
/// state — a machine-dependent palette would break the contract).
pub(crate) fn compute_engine_digest() -> [u8; 32] {
    let mut cloud = Cloud::new(
        ColorMode::TrueColor,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Green,
        RainStyle::Monolith,
    );
    cloud.set_droplet_density(0.75);
    cloud.set_chars_per_sec(8.0);
    cloud.set_monolith_size(crate::runtime::MonolithSize::Normal);
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(ORACLE_COLS, ORACLE_LINES);
    // Determinism law (entry 32): one origin instant, every state
    // anchor rewritten onto it, frames driven by exact k * period.
    let origin = Instant::now();
    cloud.anchor_virtual_clock(origin);
    cloud.crystal_dragon = false;
    cloud.set_max_sim_delta(ORACLE_PERIOD);

    let mut frame = Frame::new(ORACLE_COLS, ORACLE_LINES, cloud.palette.bg);
    let mut scratch =
        Vec::with_capacity((ORACLE_COLS as usize) * (ORACLE_LINES as usize) * 16 + 32);

    let mut sim_now = origin;
    let mut digests = Vec::with_capacity(ORACLE_FRAMES as usize);
    for i in 0..ORACLE_FRAMES {
        sim_now += ORACLE_PERIOD;
        cloud.rain_at(&mut frame, sim_now);
        digests.push(frame_digest(&frame, i + 1, &mut scratch));
        frame.clear_dirty();
    }
    fold_sequence(&digests)
}

/// Run the oracle: render the pinned sequence and compare against the
/// compile-time vector. `Ok` carries the recomputed digest; `Err`
/// names the divergence (recomputed vs pinned) for the depth row.
pub(crate) fn run_engine_oracle() -> Result<[u8; 32], String> {
    let recomputed = compute_engine_digest();
    if recomputed == ENGINE_ORACLE_DIGEST {
        Ok(recomputed)
    } else {
        Err(format!(
            "recomputed b3:{} does not match the pinned lane vector b3:{}",
            hex32(&recomputed),
            hex32(&ENGINE_ORACLE_DIGEST)
        ))
    }
}

#[cfg(test)]
#[path = "../../test/bench/engine_oracle_tests.rs"]
mod engine_oracle_tests;
