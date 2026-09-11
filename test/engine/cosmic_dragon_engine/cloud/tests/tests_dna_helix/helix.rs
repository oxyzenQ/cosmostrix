// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Genome-level behavior contracts (NIGHT-research-7): the turn
//! (uniform rotation + the projected strand geometry), the pairing
//! (rung spans + Watson-Crick complementarity), the recency decay,
//! the replication fork's travel, dissolve window and
//! re-synthesis mutation, and the fork clock's re-arm cycle.

use crate::cloud::dna_helix::helix::{
    charge_level, rung_count_for_lines, BasePair, DnaGenome, DnaRandom, ForkPhase,
};
use crate::cloud::monolith::BrightnessLevel;
use crate::constants::{DNA_CHARGE_DECAY, DNA_CHARGE_MAX, DNA_ROT_RATE};
use rand::{distr::Uniform, rngs::StdRng, SeedableRng};

fn make_genome(cols: u16, lines: u16) -> DnaGenome {
    let mut g = DnaGenome::new();
    g.reset(cols, lines);
    g
}

/// The steady-state arm: a genome with the genesis fast-forwarded
/// (the laws 1-4 contracts below pin the formed molecule — the
/// birth sequence has its own file, genesis.rs; the black hole
/// tree's drive-past-formation pattern).
fn make_formed_genome(cols: u16, lines: u16) -> DnaGenome {
    let mut g = make_genome(cols, lines);
    g.fast_forward_genesis();
    g
}

fn advance_genome(g: &mut DnaGenome, secs: f32) {
    let (mut rng, chance) = fixed_random(42);
    let mut random = DnaRandom {
        rng: &mut rng,
        rand_chance: &chance,
    };
    g.advance(secs, &mut random);
}

fn fixed_random(seed: u64) -> (StdRng, Uniform<f32>) {
    (
        StdRng::seed_from_u64(seed),
        Uniform::new(0.0, 1.0).expect("unit interval"),
    )
}

#[test]
fn dna_genome_rung_count_follows_height() {
    // One rung every RUNG_STEP lines from line 1: a 40-line
    // viewport carries 19 rungs, a 5-line one 2, a 3-line one 1
    // (a rung at line 1 — the degenerate-terminal floor), a
    // 2-line one none (the strands alone).
    assert_eq!(rung_count_for_lines(40), 19);
    assert_eq!(rung_count_for_lines(5), 2);
    assert_eq!(rung_count_for_lines(3), 1);
    assert_eq!(rung_count_for_lines(2), 0);
    assert_eq!(rung_count_for_lines(0), 0);
    let g = make_genome(80, 40);
    assert_eq!(g.rung_count_for_test(), 19);
    // Rung lines never render past the floor.
    assert_eq!(g.rung_line(18), Some(37));
    let g2 = make_genome(80, 6);
    assert_eq!(g2.rung_line(1), Some(3));
    assert_eq!(g2.rung_line(2), None, "ordinal past the table");
}

#[test]
fn dna_rotation_advances_phase_uniformly() {
    // Law 1: the phase advances at the helical rate per sim-second
    // — the whole molecule turns as one body (the formed molecule;
    // the genesis holds the rotation through the soup and ladder,
    // pinned separately in genesis.rs).
    let mut g = make_formed_genome(80, 40);
    let phase0 = g.phase;
    advance_genome(&mut g, 1.0);
    assert!(
        (g.phase - phase0 - DNA_ROT_RATE).abs() < 1e-4,
        "phase advance drifted: {}",
        g.phase - phase0
    );
}

#[test]
fn dna_strands_mirror_and_cross() {
    // Law 1's projection: strand B mirrors strand A around the
    // axis (x_B = 2cx - x_A, depth negated). Over a full turn each
    // strand sweeps the full radius band and the pair passes
    // through the crossing (same column, opposite depth).
    // Phase 0: the crossings land ON integer lines (y = 0, 11, 22,
    // 33 at the 22-line turn) — the phase-pinned geometry the
    // crossing assertions read (the formed genome otherwise carries
    // the face-on birth presentation, whose crossings fall between
    // the lines).
    let mut g = make_formed_genome(80, 40);
    g.plant_phase_for_test(0.0);
    let cx = g.center_x();
    let mut crossed = false;
    for line in 0..40 {
        // strand_pair: one projection read per line (strand A plus
        // the mirror — the draw pass's contract, the same values
        // the former separate strand_a + strand_b reads returned).
        let ((ax, ad), (bx, bd)) = g.strand_pair(line as f32);
        assert!((bx - (2.0 * cx - ax)).abs() < 1e-3, "strand B not mirrored");
        assert!((bd + ad).abs() < 1e-3, "strand B depth not negated");
        // Both strands stay inside the viewport margins.
        assert!((0.0..80.0).contains(&ax), "strand A out of bounds: {ax}");
        assert!((0.0..80.0).contains(&bx), "strand B out of bounds: {bx}");
        // Depth is a cosine — always in [-1, 1].
        assert!((-1.0..=1.0).contains(&ad));
        if (ax - bx).abs() < 0.5 {
            crossed = true;
        }
    }
    assert!(crossed, "no crossing found in a 40-line viewport");
}

#[test]
fn dna_rung_spans_breathe_with_the_turn() {
    // Law 2: the rung span is the projection between the strands
    // — wide at the lateral swing, tight at the crossings. Over
    // the full height the span set must contain both wide and
    // tight members, and every span stays inside the viewport.
    // Phase 0 lands the crossings on rung lines (the deterministic
    // alignment; see the mirror test's note).
    let mut g = make_formed_genome(80, 40);
    g.plant_phase_for_test(0.0);
    let mut widths = Vec::new();
    for idx in 0..g.rung_count_for_test() {
        let (l, r) = g.rung_span(idx).expect("rung span");
        assert!(l >= 0.0 && r < 80.0, "rung span out of bounds: {l}..{r}");
        widths.push(r - l);
    }
    let max = widths.iter().cloned().fold(0.0_f32, f32::max);
    let min = widths.iter().cloned().fold(f32::MAX, f32::min);
    assert!(max > 10.0, "no wide (lateral) rung found: max {max}");
    assert!(min < 3.0, "no tight (crossing) rung found: min {min}");
}

#[test]
fn dna_base_pairs_always_complementary() {
    // Law 2's identity: the end glyphs are Watson-Crick pairs —
    // A bonds T, G bonds C, never A-A or G-T.
    for pair in [
        BasePair::AdenineThymine,
        BasePair::ThymineAdenine,
        BasePair::GuanineCytosine,
        BasePair::CytosineGuanine,
    ] {
        let (a, b) = pair.end_glyphs();
        let complementary = matches!((a, b), ('A', 'T') | ('T', 'A') | ('G', 'C') | ('C', 'G'));
        assert!(complementary, "non-Watson-Crick pair: {a}-{b}");
    }
    // from_roll covers all four states (the mutation arm).
    assert_eq!(BasePair::from_roll(0.1), BasePair::AdenineThymine);
    assert_eq!(BasePair::from_roll(0.3), BasePair::ThymineAdenine);
    assert_eq!(BasePair::from_roll(0.6), BasePair::GuanineCytosine);
    assert_eq!(BasePair::from_roll(0.9), BasePair::CytosineGuanine);
}

#[test]
fn dna_charge_decays_exponentially_and_stays_bounded() {
    // Law 3: a planted charge decays toward zero; the absorb path
    // hard-clamps at the max (bounded by construction).
    let mut g = make_genome(80, 40);
    g.plant_charge_for_test(0, DNA_CHARGE_MAX);
    advance_genome(&mut g, 1.0);
    let expected = DNA_CHARGE_MAX * (-DNA_CHARGE_DECAY).exp();
    assert!(
        (g.charge_for_test(0) - expected).abs() < 1e-4,
        "charge decay drifted: {} vs {expected}",
        g.charge_for_test(0)
    );
    // The clamp: repeated absorption cannot exceed the max.
    for _ in 0..20 {
        g.absorb(0, 5.0, false, 0.0);
    }
    assert!(
        g.charge_for_test(0) <= DNA_CHARGE_MAX + 1e-6,
        "charge clamp violated: {}",
        g.charge_for_test(0)
    );
}

#[test]
fn dna_charge_ladder_reads_the_recency() {
    // The family's BrightnessLevel carries no PartialEq (the
    // monolith enum is match-only) — the ladder pins use matches!.
    //
    // NIGHT-research-20 re-pin: the old contract pinned the wrong
    // direction (charge 2.0 must equal Core) — that was the
    // replication-window rung holding every written rung Core-white
    // for ~1.6 s of its decay. The wake now reads the warm Hot
    // ceiling; Core survives only inside the fresh-write blink
    // (charge above the blink bound, the ~0.35 s flash at the
    // write moment itself).
    assert!(matches!(charge_level(0.0), BrightnessLevel::Ghost));
    assert!(matches!(charge_level(0.3), BrightnessLevel::Mid));
    assert!(matches!(charge_level(0.8), BrightnessLevel::Hot));
    // The settled wake (2.0, the old Core band) reads the warm
    // ceiling, never Core.
    assert!(matches!(charge_level(2.0), BrightnessLevel::Hot));
    // The blink band: a freshly-written rung (charge near max)
    // flashes Core — the write moment.
    assert!(matches!(
        charge_level(crate::constants::DNA_CHARGE_LEVEL_BLINK + 1.0e-4),
        BrightnessLevel::Core
    ));
    assert!(matches!(
        charge_level(DNA_CHARGE_MAX),
        BrightnessLevel::Core
    ));
}

#[test]
fn dna_fresh_write_blink_expires_under_decay() {
    // The blink is a moment, not a state: a freshly-written rung
    // (charge at max) reads Core at the write instant, and after
    // half a sim-second of law-3 decay the charge has fallen past
    // the blink bound — the rung reads the warm Hot ceiling while
    // its wake cools (the retired replication-window rung kept it
    // Core for ~1.6 s).
    let mut g = make_genome(80, 40);
    g.plant_charge_for_test(0, DNA_CHARGE_MAX);
    assert!(matches!(
        charge_level(g.charge_for_test(0)),
        BrightnessLevel::Core
    ));
    advance_genome(&mut g, 0.5);
    let charge = g.charge_for_test(0);
    assert!(
        charge <= crate::constants::DNA_CHARGE_LEVEL_BLINK,
        "half a second of decay must carry the charge past the blink bound (got {charge})"
    );
    assert!(matches!(charge_level(charge), BrightnessLevel::Hot));
}

#[test]
fn dna_fork_travels_down_and_re_arms() {
    // Law 4: an armed fork opens after the clock, travels to the
    // floor at the fork rate, then re-arms with a fresh clock.
    let mut g = make_formed_genome(80, 40);
    g.plant_fork_for_test(ForkPhase::Armed, 0.0, 0.001);
    advance_genome(&mut g, 0.01);
    assert_eq!(g.fork_phase, ForkPhase::Traveling);
    // Travel: fork_y advances at the fork rate.
    let y0 = g.fork_y;
    advance_genome(&mut g, 0.5);
    assert!(g.fork_y > y0, "fork did not travel: {y0} -> {}", g.fork_y);
    // Full sweep: the floor is 39 lines; at 8 lines/s plus the
    // envelope margins the fork exits within ~7 sim-seconds.
    advance_genome(&mut g, 8.0);
    assert_eq!(
        g.fork_phase,
        ForkPhase::Armed,
        "fork did not re-arm after the sweep"
    );
}

#[test]
fn dna_fork_resynthesizes_rungs_it_passes() {
    // Law 4's mutation: every rung the fork center crosses gets
    // the charge reset to max, the pair re-rolled. Enter above the
    // screen (the real entry contract) and travel one sim-second:
    // the fork reaches line ~3, crossing the rungs at lines 1 and
    // 3 (ordinals 0 and 1).
    let mut g = make_formed_genome(80, 40);
    g.plant_fork_for_test(ForkPhase::Traveling, -5.0, 0.0);
    advance_genome(&mut g, 1.0);
    let charged = (0..g.rung_count_for_test())
        .filter(|&i| g.charge_for_test(i) > 0.0)
        .count();
    assert!(
        charged >= 2,
        "no rung re-synthesized by the fork: {charged}"
    );
    // Travel past the mid rungs and check they carry charge.
    advance_genome(&mut g, 2.0);
    let mid_charged = (0..10).filter(|&i| g.charge_for_test(i) > 0.0).count();
    assert!(mid_charged > 0, "the passed rungs carry no recency");
    // The mutation arm: a full sweep must leave at least one pair
    // re-rolled off the vacant default (AdenineThymine).
    advance_genome(&mut g, 4.0);
    let rolled = (0..g.rung_count_for_test())
        .filter(|&i| g.pair_for_test(i) != Some(BasePair::AdenineThymine))
        .count();
    assert!(rolled > 0, "the fork never mutated a pair");
}

#[test]
fn dna_fork_dissolves_the_window_rungs() {
    // Law 4's dissolve window: rungs within the fork's gap are
    // dissolved (not drawn) while the fork travels; far from the
    // fork they stay paired.
    let mut g = make_genome(80, 40);
    g.plant_fork_for_test(ForkPhase::Traveling, 10.0, 0.0);
    // A rung near the fork line 10 (rung ordinal 4-5).
    let near = g.rung_dissolved(4) || g.rung_dissolved(5);
    assert!(near, "no rung dissolved near the fork");
    // Far below (rung at line 33, ordinal 16) stays paired.
    assert!(!g.rung_dissolved(16), "far rung dissolved");
    // An armed genome dissolves nothing.
    g.plant_fork_for_test(ForkPhase::Armed, 10.0, 5.0);
    assert!(!g.rung_dissolved(4));
}

#[test]
fn dna_fork_bow_peaks_at_the_fork() {
    // Law 4's Y: the radius scale peaks at the fork center and
    // decays with distance (a Gaussian envelope). 1.0 far away.
    let mut g = make_formed_genome(80, 40);
    g.plant_fork_for_test(ForkPhase::Traveling, 20.0, 0.0);
    let at_fork = g.radius_scale(20.0);
    let near = g.radius_scale(17.0);
    let far = g.radius_scale(2.0);
    assert!(at_fork > near, "bow does not peak at the fork");
    assert!(near > far, "bow does not decay with distance");
    // The Gaussian tail at 18 lines from a sigma-7 fork still
    // reads ~3% — the envelope decays, it never snaps.
    assert!(far < 1.05, "far radius scale too fat: {far}");
    // Armed: no bow at all.
    g.plant_fork_for_test(ForkPhase::Armed, 20.0, 5.0);
    assert!((g.radius_scale(20.0) - 1.0).abs() < 1e-6);
    // The effective radius never leaves the viewport margins.
    g.plant_fork_for_test(ForkPhase::Traveling, 20.0, 0.0);
    for line in 0..40 {
        let r = g.effective_radius(line as f32);
        assert!(
            r >= 0.0 && g.center_x() + r < 80.0,
            "bowed radius out of bounds: {r}"
        );
    }
}

#[test]
fn dna_genome_reset_wipes_charges_but_keeps_geometry() {
    // The family reset contract: a dormant molecule must not carry
    // painted recency into the next entry (charges wiped, fork
    // re-armed) — but the geometry (twist, center, radius) is
    // rebuilt for the viewport.
    let mut g = make_genome(80, 40);
    g.plant_charge_for_test(0, DNA_CHARGE_MAX);
    let phase = g.phase;
    g.reset(80, 40);
    assert_eq!(g.charge_for_test(0), 0.0, "charge survived the reset");
    assert_eq!(g.fork_phase, ForkPhase::Armed);
    assert_eq!(g.phase, phase, "phase snapped on same-size reset");
    assert_eq!(g.rung_count_for_test(), 19);
}
