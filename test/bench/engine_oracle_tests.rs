// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! Engine oracle tests (night-frontier-Z1, Proof Moat 1c).
//!
//! The pinned vector is the moat's tooth: any engine change that
//! alters even one cell of the pinned 16-frame sequence fails these
//! tests (and the `sig` D4 row, and the `--doctor` line) until the
//! vector is deliberately regenerated — the same version-bound law
//! the provenance anchor runs under.

use super::*;

use std::time::Instant;

/// THE PIN: the current engine must recompute the compile-time vector.
/// A failing test here means the render changed (deliberately or not)
/// — regenerate the constant only as a deliberate, documented act.
#[test]
fn pinned_vector_matches_the_current_engine() {
    match run_engine_oracle() {
        Ok(digest) => assert_eq!(digest, ENGINE_ORACLE_DIGEST),
        Err(divergence) => panic!(
            "the engine no longer renders the pinned sequence — regenerate \
             ENGINE_ORACLE_DIGEST deliberately if this change is intended: {divergence}"
        ),
    }
}

/// The oracle is deterministic: two renders of the pinned sequence in
/// the same process (independent Clouds, independent RNG streams from
/// the launch seed, independently anchored clocks) fold identically.
#[test]
fn oracle_is_deterministic_across_runs() {
    let a = compute_engine_digest();
    let b = compute_engine_digest();
    assert_eq!(a, b);
    assert_eq!(a, ENGINE_ORACLE_DIGEST);
}

/// The fold is structural: the pinned digest equals a manual fold over
/// the 16 per-frame digests of the pinned sequence, rendered here
/// frame by frame — the same sequence, the same serializer, the same
/// length-and-order header.
#[test]
fn pinned_vector_equals_the_manual_fold_of_the_sequence() {
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
    let origin = Instant::now();
    cloud.anchor_virtual_clock(origin);
    cloud.crystal_dragon = false;
    cloud.set_max_sim_delta(ORACLE_PERIOD);

    let mut frame = Frame::new(ORACLE_COLS, ORACLE_LINES, cloud.palette.bg);
    let mut scratch = Vec::new();
    let mut sim_now = origin;
    let mut digests = Vec::with_capacity(ORACLE_FRAMES as usize);
    for i in 0..ORACLE_FRAMES {
        sim_now += ORACLE_PERIOD;
        cloud.rain_at(&mut frame, sim_now);
        digests.push(frame_digest(&frame, i + 1, &mut scratch));
        frame.clear_dirty();
    }

    assert_eq!(digests.len(), ORACLE_FRAMES as usize);
    assert_eq!(fold_sequence(&digests), ENGINE_ORACLE_DIGEST);
}

/// The pinned vector is not degenerate: it is not the zero digest (a
/// placeholder pin would make every comparison vacuously informative)
/// and no shorter-sequence fold can reuse it (the sequence length is
/// bound into the value).
#[test]
fn pinned_vector_is_not_degenerate() {
    assert_ne!(ENGINE_ORACLE_DIGEST, [0u8; 32]);

    // A one-frame fold and a two-frame fold cannot equal the pinned
    // 16-frame vector: the fold headers the sequence length, so a
    // truncated or padded sequence never reuses the pinned value.
    assert_ne!(fold_sequence(&[ENGINE_ORACLE_DIGEST]), ENGINE_ORACLE_DIGEST);
    assert_ne!(
        fold_sequence(&[ENGINE_ORACLE_DIGEST, ENGINE_ORACLE_DIGEST]),
        ENGINE_ORACLE_DIGEST
    );
}
