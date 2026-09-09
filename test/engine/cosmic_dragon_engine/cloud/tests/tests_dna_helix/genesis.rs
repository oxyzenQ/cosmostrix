// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Genesis contracts (NIGHT-research-7 part 3, law 0): the birth
//! sequence the style entry replays — soup, ladder, windup,
//! steady. Pinned at both levels: the genome's timeline (the
//! phase classification, the two fronts, the held rotation, the
//! fork gate, the exact steady seam) and the orchestration (the
//! molecule hidden through the soup, the top-down materialization
//! with the fresh-write light, the thick primordial broth, the
//! re-entry replay, the resize keeps-steady and bench
//! fast-forward contracts).

use super::*;

use crate::cloud::dna_helix::genesis::{
    genesis_phase, genesis_radius_growth, genesis_total_secs, ladder_front, windup_front,
    GenesisPhase,
};
use crate::cloud::dna_helix::helix::{DnaGenome, DnaRandom, ForkPhase};
use crate::constants::{
    DNA_CHARGE_MAX, DNA_GENESIS_LADDER_SECS, DNA_GENESIS_SOUP_SECS, DNA_GENESIS_WINDUP_SECS,
    DNA_ROT_RATE, DNA_R_MIN,
};
use rand::{distr::Uniform, rngs::StdRng, SeedableRng};

fn make_genome(cols: u16, lines: u16) -> DnaGenome {
    let mut g = DnaGenome::new();
    g.reset(cols, lines);
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
fn genesis_timeline_classifies_the_four_phases() {
    // Law 0's classifier: the phase windows partition the timeline
    // in order, and the total is the sum of the three windows.
    let total = genesis_total_secs();
    let expected = DNA_GENESIS_SOUP_SECS + DNA_GENESIS_LADDER_SECS + DNA_GENESIS_WINDUP_SECS;
    assert!(
        (total - expected).abs() < 1e-6,
        "total drifted: {total} vs {expected}"
    );
    assert_eq!(genesis_phase(0.0), GenesisPhase::Soup);
    assert_eq!(
        genesis_phase(DNA_GENESIS_SOUP_SECS - 0.01),
        GenesisPhase::Soup
    );
    assert_eq!(genesis_phase(DNA_GENESIS_SOUP_SECS), GenesisPhase::Ladder);
    let ladder_end = DNA_GENESIS_SOUP_SECS + DNA_GENESIS_LADDER_SECS;
    assert_eq!(genesis_phase(ladder_end - 0.01), GenesisPhase::Ladder);
    assert_eq!(genesis_phase(ladder_end), GenesisPhase::Windup);
    assert_eq!(genesis_phase(total - 0.01), GenesisPhase::Windup);
    assert_eq!(genesis_phase(total), GenesisPhase::Steady);
    assert_eq!(genesis_phase(total + 100.0), GenesisPhase::Steady);
}

#[test]
fn genesis_genome_clock_completes_and_flips_the_flag() {
    // The clock advances on the sim dt, clamps at the total and
    // flips the formed flag exactly once — a long spike dt cannot
    // overshoot, further advance is a no-op on the genesis state.
    let mut g = make_genome(80, 40);
    assert!(!g.formed_for_test(), "fresh construction is unborn");
    assert_eq!(g.genesis_t_for_test(), 0.0);
    advance_genome(&mut g, 1.0);
    assert!((g.genesis_t_for_test() - 1.0).abs() < 1e-5);
    assert!(!g.formed_for_test());
    // One huge tick: the clamp holds, the flag flips.
    advance_genome(&mut g, 100.0);
    assert!(
        (g.genesis_t_for_test() - genesis_total_secs()).abs() < 1e-5,
        "clock overshot the total"
    );
    assert!(g.formed_for_test(), "the flag never flipped");
    let t = g.genesis_t_for_test();
    advance_genome(&mut g, 1.0);
    assert_eq!(g.genesis_t_for_test(), t, "the clock moved past steady");
}

#[test]
fn genesis_soup_phase_hides_the_molecule_and_gates_absorption() {
    // Through the primordial dwell: the molecule draws nothing
    // (the draw gate), no rung is built, and the falling soup is
    // NOT absorbed — the primordial rain falls through to the
    // floor (law 0's absorption gate). The soup itself draws.
    let mut cloud = make_dna_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 50, 16);
    assert!(
        !cloud.dna_helix_rain.genome_for_test().formed_for_test(),
        "still the primordial dwell at ~1 s"
    );
    assert!(
        !cloud.dna_helix_rain.genome_for_test().molecule_visible(),
        "the molecule drew during the primordial dwell"
    );
    assert_eq!(
        cloud.dna_helix_rain.absorptions_for_test(),
        0,
        "a drop was absorbed before any rung was built"
    );
    assert!(
        !cloud.dna_helix_rain.drawn_cells_for_test().is_empty(),
        "the soup itself must draw (the sky is the scene)"
    );
}

#[test]
fn genesis_ladder_materializes_rungs_top_down_with_fresh_light() {
    // Early ladder (~0.7 s into the window): the assembly wave has
    // written the top rungs (built, stamped to max charge, the
    // pair rolled) and not yet reached the bottom ones (unbuilt,
    // zero charge).
    let mut g = make_genome(80, 40);
    let t = DNA_GENESIS_SOUP_SECS + 0.7;
    advance_genome(&mut g, t);
    assert_eq!(genesis_phase(g.genesis_t_for_test()), GenesisPhase::Ladder);
    // The front at 0.7 s into a 2.6 s window: ~10.8 of 40 lines —
    // rungs at lines 1, 3, 5, 7, 9 built; the rung at line 37 not.
    for built in 0..5 {
        assert!(g.rung_built(built), "rung {built} below the front");
        assert!(
            g.charge_for_test(built) > 0.0,
            "the written rung {built} carries no fresh light"
        );
    }
    assert!(!g.rung_built(18), "the bottom rung built ahead of the wave");
    assert_eq!(g.charge_for_test(18), 0.0, "unbuilt rung carries charge");
}

#[test]
fn genesis_ladder_is_flat_face_on_with_growing_radius() {
    // The pre-molecule: ONE angle for every line (the flat
    // ladder, face-on — sin at the max so the rungs span their
    // widest), the radius growing fast out of the axis (cubic
    // ease-out: the spine reads as a spine only in the first
    // moments of the window, then splits into the strands).
    let mut g = make_genome(80, 40);
    let t = DNA_GENESIS_SOUP_SECS + 1.2;
    advance_genome(&mut g, t);
    let flat = g.strand_angle(0.0);
    for line in [1.0, 10.0, 20.0, 39.0] {
        assert!(
            (g.strand_angle(line) - flat).abs() < 1e-5,
            "the ladder is not flat at line {line}"
        );
    }
    let growth = genesis_radius_growth(t);
    assert!(
        growth > 0.0 && growth < 1.0,
        "radius growth not mid-ramp: {growth}"
    );
    // The face-on read: the rung span is the full growing width.
    let (left, right) = g.rung_span(0).expect("span");
    let expected = 2.0 * g.effective_radius(1.0);
    assert!(
        (right - left - expected).abs() < 1e-4,
        "the face-on span must be the full width: {} vs {expected}",
        right - left
    );
    // Early in the window the spine sits BELOW the steady
    // legibility floor (the floor is lifted through the growth —
    // a clamped spine would never read as the single seed line).
    let mut g2 = make_genome(80, 40);
    let t_early = DNA_GENESIS_SOUP_SECS + 0.1;
    advance_genome(&mut g2, t_early);
    let r = g2.effective_radius(20.0);
    assert!(
        r > 0.0 && r < DNA_R_MIN,
        "the growing spine must sit below the steady floor: {r}"
    );
}

#[test]
fn genesis_windup_zips_the_twist_from_the_top_with_a_flat_tail() {
    // Mid-windup: above the front the strand carries the full
    // steady law; below it the flat extension holds the front's
    // angle exactly (the seam is continuous — the identity, not an
    // approximation).
    let mut g = make_genome(80, 40);
    let t = DNA_GENESIS_SOUP_SECS + DNA_GENESIS_LADDER_SECS + 1.2;
    advance_genome(&mut g, t);
    assert_eq!(genesis_phase(g.genesis_t_for_test()), GenesisPhase::Windup);
    let front = windup_front(t, 40);
    assert!(
        front > 0.0 && front < 40.0,
        "front not mid-descent: {front}"
    );
    // Above the front: the steady law exactly.
    for line in [0.0, front * 0.5] {
        let expected = g.phase + line * g.twist_for_test();
        assert!(
            (g.strand_angle(line) - expected).abs() < 1e-5,
            "wound region off the steady law at {line}"
        );
    }
    // Below the front: the flat extension — every line holds the
    // front's angle, and the seam is exact.
    let seam = g.strand_angle(front);
    let tail = g.strand_angle(39.0);
    assert!(
        (tail - seam).abs() < 1e-5,
        "the flat tail does not hold the front's angle: {tail} vs {seam}"
    );
    // The twist gradient exists above the seam (the wound region
    // accumulates twist from the top down).
    let above = g.strand_angle(front * 0.5);
    assert!(
        (seam - above).abs() > 1e-3,
        "no twist gradient above the seam: {seam} vs {above}"
    );
}

#[test]
fn genesis_completion_matches_the_steady_law_exactly() {
    // The seam contract: the last windup frame and the first
    // steady frame evaluate to identical geometry — the final
    // front is the full height, so the steady formula is the
    // windup formula at every line. Driven to just past the total.
    let mut g = make_genome(80, 40);
    advance_genome(&mut g, genesis_total_secs() + 0.5);
    assert!(g.formed_for_test());
    for line in [0.0, 7.0, 20.0, 39.0] {
        let expected = g.phase + line * g.twist_for_test();
        assert!(
            (g.strand_angle(line) - expected).abs() < 1e-5,
            "steady law violated at {line}"
        );
    }
    // All rungs built, the radius back above the legibility floor.
    for idx in 0..g.rung_count_for_test() {
        assert!(g.rung_built(idx), "rung {idx} unbuilt in the steady state");
    }
    assert!(g.effective_radius(20.0) >= DNA_R_MIN);
}

#[test]
fn genesis_holds_the_rotation_through_soup_and_ladder() {
    // The face-on contract: the phase is frozen while the flat
    // pre-molecule assembles, and resumes with the windup (the
    // birth's stage lighting — a rotating flat ladder periodically
    // collapses edge-on to a single line). Stepped so each tick
    // lands strictly inside its window (the rotation check reads
    // the post-update phase — a tick that straddles the
    // ladder->windup boundary rotates for its whole dt, at most
    // one frame of rotation at the interactive 16 ms cadence,
    // invisible).
    let mut g = make_genome(80, 40);
    let phase0 = g.phase;
    advance_genome(&mut g, DNA_GENESIS_SOUP_SECS - 0.05);
    assert!(
        (g.phase - phase0).abs() < 1e-6,
        "the phase moved during the soup"
    );
    // Land exactly at the soup's end (the ladder begins).
    advance_genome(&mut g, 0.05);
    assert!(
        (g.phase - phase0).abs() < 1e-6,
        "the phase moved at the soup seam"
    );
    // The ladder window, stopping short of the windup.
    advance_genome(&mut g, DNA_GENESIS_LADDER_SECS - 0.05);
    assert!(
        (g.phase - phase0).abs() < 1e-6,
        "the phase moved during the ladder"
    );
    // The windup: the turn resumes at the helical rate.
    advance_genome(&mut g, 1.0);
    assert!(
        (g.phase - phase0 - DNA_ROT_RATE).abs() < 1e-4,
        "the turn did not resume with the windup: {}",
        g.phase - phase0
    );
}

#[test]
fn genesis_fork_waits_for_the_completed_genome() {
    // The replication gate: an armed fork with a spent clock stays
    // armed while the molecule is unborn (no replication before
    // the genome exists), and opens on the very next tick once
    // formed.
    let mut g = make_genome(80, 40);
    g.plant_fork_for_test(ForkPhase::Armed, 0.0, 0.001);
    advance_genome(&mut g, 0.01);
    assert_eq!(
        g.fork_phase,
        ForkPhase::Armed,
        "the fork opened before the genome existed"
    );
    assert!(!g.formed_for_test());
    g.fast_forward_genesis();
    advance_genome(&mut g, 0.01);
    assert_eq!(
        g.fork_phase,
        ForkPhase::Traveling,
        "the fork did not open on the completed genome"
    );
}

#[test]
fn genesis_fast_forward_skips_to_the_cold_steady_molecule() {
    // The bench contract: the fast-forward parks the clock, flips
    // the flag, and writes NO assembly charges — the cold
    // post-reset molecule, exactly the pre-genesis bench profile
    // (regression comparability).
    let mut g = make_genome(80, 40);
    g.fast_forward_genesis();
    assert!(g.formed_for_test());
    assert!((g.genesis_t_for_test() - genesis_total_secs()).abs() < 1e-5);
    for idx in 0..g.rung_count_for_test() {
        assert!(g.rung_built(idx));
        assert_eq!(g.charge_for_test(idx), 0.0, "fast-forward wrote charge");
    }
}

#[test]
fn genesis_primordial_soup_runs_thick_then_thins() {
    // Law 0's dial: through the soup the active target runs the
    // genesis multiplier — the broth carries more nucleotides than
    // the whole steady target (the soup IS the scene while the
    // molecule is absent). ~2.1 sim-s in (still the thick window).
    let mut cloud = make_dna_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 110, 16);
    let active = cloud.dna_helix_rain.active_count();
    // The steady target on 120 lanes at density 0.70 is ~11; the
    // thick dial targets ~28. Above the steady target proves the
    // multiplier ran.
    assert!(
        active > 11,
        "the primordial broth is not thick: {active} active"
    );
    // And it thins: past the genesis the population settles back
    // under the steady target (the steady dial hands the sky back
    // — the spawn gate never lets active exceed the target, and
    // the thick-era drops have all floor-expired by now).
    run_past_genesis(&mut cloud, &mut frame);
    run_frames(&mut cloud, &mut frame, 400, 16);
    assert!(
        cloud.dna_helix_rain.active_count() <= 12,
        "the soup never thinned: {}",
        cloud.dna_helix_rain.active_count()
    );
}

#[test]
fn genesis_reentry_replays_the_sequence() {
    // The begin_formation contract: style entry re-arms the birth
    // sequence — a formed molecule replays the soup on re-entry
    // and completes again.
    let mut cloud = make_dna_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_past_genesis(&mut cloud, &mut frame);
    assert!(
        cloud.dna_helix_rain.genome_for_test().formed_for_test(),
        "the first pass never completed"
    );
    cloud.transition_rain_style(RainStyle::Glyph);
    cloud.transition_rain_style(RainStyle::DnaHelix);
    assert!(
        !cloud.dna_helix_rain.genome_for_test().formed_for_test(),
        "re-entry did not re-arm the genesis"
    );
    let mut frame2 = Frame::new(80, 40, cloud.palette.bg);
    run_past_genesis(&mut cloud, &mut frame2);
    assert!(
        cloud.dna_helix_rain.genome_for_test().formed_for_test(),
        "the replay never completed"
    );
    assert!(
        !cloud.dna_helix_rain.drawn_cells_for_test().is_empty(),
        "the reformed molecule never drew"
    );
}

#[test]
fn genesis_resize_keeps_the_steady_state() {
    // The resize contract (the black hole's formation contract):
    // a pure reset does NOT rewind the genesis — a formed molecule
    // stays formed through a viewport change, and an unborn one
    // keeps its clock (the formation continues on the new grid).
    let mut cloud = make_dna_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_past_genesis(&mut cloud, &mut frame);
    cloud.reset(100, 50);
    assert!(
        cloud.dna_helix_rain.genome_for_test().formed_for_test(),
        "a pure resize rewound the genesis"
    );
    // Mid-formation: the clock survives the resize.
    let mut cloud2 = make_dna_cloud(80, 40);
    let mut frame2 = Frame::new(80, 40, cloud2.palette.bg);
    run_frames(&mut cloud2, &mut frame2, 150, 16);
    let t_mid = cloud2.dna_helix_rain.genome_for_test().genesis_t_for_test();
    assert!(t_mid > 0.0);
    cloud2.reset(90, 45);
    let t_after = cloud2.dna_helix_rain.genome_for_test().genesis_t_for_test();
    assert!(
        (t_mid - t_after).abs() < 1e-5,
        "the resize snapped the genesis clock: {t_mid} -> {t_after}"
    );
}

#[test]
fn genesis_bench_fast_forwards_the_sequence() {
    // The bench contract: reset_bench hands the bench loop the
    // formed molecule (one-shot choreography is not steady-state
    // throughput — the Z-6 critical-path precedent).
    let mut cloud = make_dna_cloud(80, 40);
    cloud.reset_bench(80, 40);
    assert!(
        cloud.dna_helix_rain.genome_for_test().formed_for_test(),
        "the bench genesis did not fast-forward"
    );
}

#[test]
fn genesis_sequence_draws_within_bounds_throughout() {
    // The whole birth sequence, frame by frame: every drawn cell
    // sits inside the viewport at every phase boundary (the soup's
    // rain, the spine, the splitting strands, the winding ladder).
    let mut cloud = make_dna_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    let start = Instant::now();
    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    // 500 frames x 16 ms at 14 cps = ~9.7 sim-s: the full
    // sequence plus a second of steady state.
    for idx in 0..500u64 {
        let now = start + Duration::from_millis(idx * 16);
        cloud.rain_at(&mut frame, now);
        for cell in cloud.dna_helix_rain.drawn_cells_for_test() {
            assert!(
                cell.col < 80 && cell.line < 40,
                "genesis cell out of bounds at frame {idx}: {}:{}",
                cell.col,
                cell.line
            );
        }
        frame.clear_dirty();
    }
    assert!(
        cloud.dna_helix_rain.genome_for_test().formed_for_test(),
        "the sequence never completed over 9.7 sim-s"
    );
}

#[test]
fn genesis_fronts_travel_monotonically_to_the_full_height() {
    // The two fronts are monotone, reach the full height exactly
    // at their window's end, and hold it after (the closed-form
    // contracts the crossing tests ride on).
    let lines = 40u16;
    let ladder_end = DNA_GENESIS_SOUP_SECS + DNA_GENESIS_LADDER_SECS;
    let total = genesis_total_secs();
    let mut prev = 0.0f32;
    let mut t = 0.0f32;
    while t <= ladder_end + 0.001 {
        let front = ladder_front(t, lines);
        assert!(front >= prev, "the assembly front went backwards at {t}");
        prev = front;
        t += 0.1;
    }
    assert!(
        (ladder_front(ladder_end, lines) - lines as f32).abs() < 1e-5,
        "the assembly front never reached the floor"
    );
    assert_eq!(ladder_front(total + 5.0, lines), lines as f32);
    assert!((ladder_front(0.0, lines) - 0.0).abs() < 1e-6);
    assert!((windup_front(ladder_end, lines) - 0.0).abs() < 1e-6);
    assert!(
        (windup_front(total, lines) - lines as f32).abs() < 1e-5,
        "the twist front never reached the floor"
    );
}

#[test]
fn genesis_assembly_charge_respects_the_clamp() {
    // The assembly wave writes law-3 charges (the fresh-write
    // light): every written rung sits at the max clamp on the tick
    // it is written, and the trail decays under law 3 while the
    // wave travels on. Stepped so the top rungs are written a full
    // window before the bottom ones (the first-written rung reads
    // dimmer than the last-written one — the wave's wake).
    let mut g = make_genome(80, 40);
    // Early ladder: the top rungs are written, at the clamp.
    advance_genome(&mut g, DNA_GENESIS_SOUP_SECS + 0.5);
    let early = g.charge_for_test(0);
    assert!(early > 0.0, "the wave wrote no early light");
    assert!(early <= DNA_CHARGE_MAX + 1e-6, "the clamp was violated");
    // The rest of the window: the wave reaches the floor — the
    // first-written rung has decayed under the traveling wave.
    advance_genome(&mut g, DNA_GENESIS_LADDER_SECS);
    let first = g.charge_for_test(0);
    let last = g.charge_for_test(18);
    assert!(last > 0.0, "the wave wrote no light at the floor: {last}");
    assert!(
        first < last,
        "the trail does not decay behind the wave: {first} vs {last}"
    );
    assert!(
        first > 0.0,
        "the early light fully decayed within the window"
    );
    assert!(last <= DNA_CHARGE_MAX + 1e-6, "the clamp was violated");
}
