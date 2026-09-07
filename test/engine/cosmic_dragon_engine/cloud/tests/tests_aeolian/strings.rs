// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The invented-math contracts of the aeolian string field
//! (NIGHT-special-2, laws 1-4): the L1 contraction proof, the
//! urgency conduction (packet propagation + bright-outruns-dim +
//! FPS invariance), the wall reflection, the self-similar decay,
//! and the interference knot read.

use crate::cloud::aeolian::strings::AeolianStrings;
use crate::constants::{AEOLIAN_KNOT_LEVEL, AEOLIAN_WALL_REFLECT};

/// Peak column of the RIGHTWARD channel on a string (argmax of
/// |chan_r|) — the rightward packet's observable position. The
/// combined field holds two mirrored packets (a symmetric pluck),
/// so tests tracking travel direction must read the channel,
/// not the sum.
fn right_peak_position(field: &AeolianStrings, string: usize, cols: usize) -> usize {
    let mut best = 0;
    let mut best_a = -1.0;
    for x in 0..cols {
        let (r, _) = field.channels_for_test(string, x);
        let a = r.abs();
        if a > best_a {
            best_a = a;
            best = x;
        }
    }
    best
}

#[test]
fn law1_pluck_propagates_both_directions() {
    // A symmetric pluck splits into two packets racing away from
    // the impact column: after ~1 s of conduction the field must
    // have mass on BOTH sides of the origin, and the origin column
    // itself must have drained (the impact site goes silent).
    let mut field = AeolianStrings::new();
    field.reset(120, 40);
    field.pluck(0, 60, 3.0);

    let dt = 0.05;
    for _ in 0..20 {
        field.advance(dt);
    }

    let left = (0..60)
        .map(|x| field.combined(0, x))
        .fold(0.0_f32, f32::max);
    let right = (61..120)
        .map(|x| field.combined(0, x))
        .fold(0.0_f32, f32::max);
    assert!(left > 0.05, "leftward packet missing (combined={left})");
    assert!(right > 0.05, "rightward packet missing (combined={right})");
    // Symmetric injection: the two sides stay in balance (the wall
    // is far away at 120 columns).
    assert!(
        (left - right).abs() / left.max(right) < 0.35,
        "asymmetric split: left={left} right={right}"
    );
}

#[test]
fn law1_urgency_bright_packet_outruns_dim() {
    // The core of the invention: conduction rate grows with
    // amplitude, so a strong packet's crest travels measurably
    // farther than a weak one in the same time. Two strings on one
    // field, identical origin, different pluck amplitudes.
    let mut field = AeolianStrings::new();
    field.reset(200, 34); // two strings, far walls
    field.pluck(0, 100, 4.0); // bright packet
    field.pluck(1, 100, 0.4); // dim packet

    let dt = 0.016;
    for _ in 0..60 {
        field.advance(dt);
    }

    let bright_peak = right_peak_position(&field, 0, 200);
    let dim_peak = right_peak_position(&field, 1, 200);
    assert!(
        bright_peak > dim_peak + 10,
        "urgency law violated: bright peak {bright_peak} vs dim peak {dim_peak}"
    );
}

#[test]
fn law1_fps_invariant_peak_position() {
    // Rate-based physics: half the dt for double the steps must
    // land the packet at the same place (within the 1-cell
    // quantization of the lattice). This is the frame-rate
    // invariance contract — the weave renders identically at 30,
    // 60 or 144 fps.
    let mut a = AeolianStrings::new();
    a.reset(200, 40);
    a.pluck(0, 100, 3.0);
    let mut b = AeolianStrings::new();
    b.reset(200, 40);
    b.pluck(0, 100, 3.0);

    for _ in 0..60 {
        a.advance(0.016);
    }
    for _ in 0..120 {
        b.advance(0.008);
    }

    let pa = right_peak_position(&a, 0, 200);
    let pb = right_peak_position(&b, 0, 200);
    assert!(
        pa.abs_diff(pb) <= 2,
        "fps-dependent packet position: 60-step={pa} 120-step={pb}"
    );
}

#[test]
fn law2_wall_reflection_reverses_channel() {
    // A strong pluck AT the right wall: the rightward channel's
    // boundary outflow cannot leave (the instrument is closed) — it
    // re-enters the LEFT channel at the wall and travels back into
    // the field.
    let mut field = AeolianStrings::new();
    field.reset(120, 40);
    let wall = 119;
    field.pluck(0, wall, 3.0);

    field.advance(0.05);

    // After one tick the left channel at the wall carries the
    // reflected remainder (1 - p + REFL * p of the pluck, decayed).
    let (_, l_at_wall) = {
        let (r, l) = field.channels_for_test(0, wall);
        (r, l)
    };
    assert!(
        l_at_wall > 1.0,
        "reflection missing: left channel at wall is {l_at_wall}"
    );
    // And it travels inward over subsequent ticks.
    for _ in 0..10 {
        field.advance(0.05);
    }
    let inward = (100..wall)
        .map(|x| field.combined(0, x))
        .fold(0.0_f32, f32::max);
    assert!(
        inward > 0.1,
        "reflected packet never left the wall: {inward}"
    );
}

#[test]
fn law2_reflection_factor_is_lossy() {
    // The wall absorbs: total field amplitude after a wall bounce
    // can never exceed the pluck (REFL <= 1 contracts the L1 norm).
    let mut field = AeolianStrings::new();
    field.reset(40, 40);
    field.pluck(0, 39, 3.0); // pluck AT the wall
    let mut injected = 3.0_f32; // both channels -> total L1 6.0
    let _ = injected;
    injected = 6.0;

    for _ in 0..30 {
        field.advance(0.05);
    }
    let l1 = field.l1_norm_for_test();
    assert!(
        l1 <= injected,
        "wall bounce grew the field: l1={l1} injected={injected}"
    );
    assert!(
        l1 < injected * AEOLIAN_WALL_REFLECT,
        "wall did not absorb: l1={l1} (injected {injected})"
    );
}

#[test]
fn law3_l1_never_grows_without_plucks() {
    // The stability proof's observable: with NO injection, the L1
    // norm is strictly non-increasing every tick (conduction is an
    // exact interior transfer, reflection is lossy, decay shrinks).
    // Tolerance 1e-3: the norm itself is a 480-term f32 sum, so pure
    // rounding dust reaches a few 1e-5 — real growth (any broken
    // transfer term) is orders of magnitude larger.
    let mut field = AeolianStrings::new();
    field.reset(120, 40);
    field.pluck(0, 30, 2.5);
    field.pluck(0, 90, 1.5);
    field.pluck(1, 60, 3.0);

    let mut prev = field.l1_norm_for_test();
    assert!(prev > 0.0);
    for tick in 0..900 {
        field.advance(0.016);
        let now = field.l1_norm_for_test();
        assert!(
            now <= prev + 1e-3,
            "L1 grew at tick {tick}: {now} > {prev} (the sweep must be a contraction)"
        );
        prev = now;
    }
    // And decay wins in the long run: a silent field goes silent
    // (900 ticks x 0.016 = 14.4 sim-s > 4 half-lives; the initial
    // L1 is 4 x 7.0 = 28 with the profile, so the floor here is
    // ~0.7% of the birth energy).
    assert!(prev < 0.3, "field never decayed: {prev}");
}

#[test]
fn law3_decay_erodes_but_l1_bound_holds_under_sustained_plucking() {
    // The integration form of the L1 proof: hammer the field with
    // continuous injection for thousands of ticks; the norm must
    // plateau at the analytic steady state injection_rate / decay
    // (each pluck adds 2x amplitude to the L1 — one per channel —
    // and one fires every 3 ticks).
    let mut field = AeolianStrings::new();
    field.reset(120, 40);
    let per_pluck = 2.0_f32;
    let dt = 0.016;
    // Each pluck injects L1 = 4 x per_pluck: the three-cell profile
    // carries (1 + 0.5 + 0.5) x amplitude per channel, and the
    // pluck fills BOTH channels.
    let injection_rate = (4.0 * per_pluck) / (3.0 * dt);
    let steady_state = injection_rate / crate::constants::AEOLIAN_STRING_DECAY;
    let mut max_l1 = 0.0_f32;
    for tick in 0..4000 {
        if tick % 3 == 0 {
            // Continuous weather: a pluck every third tick.
            field.pluck(0, (tick * 37) % 120, per_pluck);
        }
        field.advance(dt);
        max_l1 = max_l1.max(field.l1_norm_for_test());
    }
    // The bound: the field never exceeds the analytic steady state
    // by more than 20% (transient overshoot tolerance).
    assert!(
        max_l1 < steady_state * 1.2,
        "field runaway under sustained plucking: max L1 {max_l1} vs steady {steady_state}"
    );
    assert!(max_l1 > per_pluck, "field never rang: max L1 {max_l1}");
}

#[test]
fn interference_knot_reads_where_packets_cross() {
    // Two counter-propagating packets on one string: where they
    // pass through each other, BOTH channels are strong at the same
    // cells — the knot read (min channel amplitude) must exceed the
    // knot threshold somewhere between the two origins.
    let mut field = AeolianStrings::new();
    field.reset(200, 40);
    field.pluck(0, 40, 3.5);
    field.pluck(0, 160, 3.5);

    let dt = 0.016;
    let mut best_knot = 0.0_f32;
    // The two packets close on each other at ~75 cells/sim-s across
    // a 120-cell gap: the crossing window opens around tick 100 and
    // lasts until they pass through (~tick 200).
    for _ in 0..220 {
        field.advance(dt);
        for x in 50..150 {
            best_knot = best_knot.max(field.knot(0, x));
        }
    }
    assert!(
        best_knot > AEOLIAN_KNOT_LEVEL,
        "no interference knot between crossing packets (best={best_knot})"
    );
}

#[test]
fn slope_query_points_uphill() {
    // The drop-steering gradient: a pulse's slope is positive on
    // the left flank (field rising toward the crest) and negative
    // on the right flank. Pluck near the LEFT wall so the leftward
    // twin dies at the wall and the read stays one-sided.
    let mut field = AeolianStrings::new();
    field.reset(200, 40);
    field.pluck(0, 30, 3.0);
    // One small step so the packet has flanks (the three-cell pluck
    // profile spreads into a proper pulse within a few ticks).
    for _ in 0..4 {
        field.advance(0.016);
    }
    // A one-sided read: plucked near the LEFT wall so the leftward
    // twin absorbs at the wall quickly — the field around the
    // rightward packet is unconfounded by counter-travel.
    let peak = right_peak_position(&field, 0, 200);
    assert!(peak > 30, "rightward packet did not move: peak={peak}");
    let left_slope = field.slope(0, peak.saturating_sub(2));
    let right_slope = field.slope(0, (peak + 2).min(199));
    assert!(
        left_slope > 0.0 && right_slope < 0.0,
        "slope misread: left={left_slope} right={right_slope} at peak {peak}"
    );
}

#[test]
fn silent_field_draws_nothing_and_queries_zero() {
    // The invisible instrument: a fresh field is silent everywhere
    // (combined = 0, knot = 0) — the architecture exists only where
    // the rain has played it.
    let field = AeolianStrings::new();
    let mut field = field;
    field.reset(80, 40);
    for x in 0..80 {
        assert_eq!(field.combined(0, x), 0.0);
        assert_eq!(field.knot(0, x), 0.0);
        assert_eq!(field.slope(0, x), 0.0);
    }
    assert_eq!(field.l1_norm_for_test(), 0.0);
}
