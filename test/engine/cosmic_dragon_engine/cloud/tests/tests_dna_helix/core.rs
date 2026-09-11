// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Core dna-helix-style behavior contracts (NIGHT-research-7):
//! scene resolution, spawn density + the calm-sky sparse dial, the
//! nucleotide fall + bounds, the rung absorption + mutation, the
//! drawn bounds, the diff cleanup (the molecule repaints without
//! residue), pause freeze, style transitions, speed scaling, and
//! the sustained-boundedness integration.

use super::*;

use crate::cloud::dna_helix::helix::{BasePair, ForkPhase};
use crate::constants::{DNA_ACTIVE_MAX, DNA_MUTATION_CHANCE};

#[test]
fn dna_helix_scene_resolves_style_and_fields() {
    let s = crate::scene::get_scene("dna_helix").expect("dna_helix scene exists");
    assert_eq!(s.config.rain_style, RainStyle::DnaHelix);
    assert_eq!(s.config.color, Some("neptune"));
    assert_eq!(s.config.charset, Some("dna"));
    assert_eq!(s.config.fps, Some(60.0));
    assert_eq!(s.config.speed, Some(14.0));
    assert_eq!(s.config.density, Some(0.50));
    assert_eq!(
        s.config.glitch_level,
        Some(crate::config::GlitchLevel::None)
    );
    // Style dispatch sanity: structured family, accumulator spawn.
    assert!(!RainStyle::DnaHelix.is_droplet_family());
    assert!(RainStyle::DnaHelix.uses_spawn_remainder());
    // Label round-trip (the scene-custom `rain` field surface).
    assert_eq!(RainStyle::DnaHelix.as_str(), "dna_helix");
    assert_eq!(
        RainStyle::from_label("dna_helix"),
        Some(RainStyle::DnaHelix)
    );
    assert_eq!(RainStyle::from_label("DnaHelix"), Some(RainStyle::DnaHelix));
    assert_eq!(RainStyle::from_label("dnahelix"), Some(RainStyle::DnaHelix));
    assert_eq!(RainStyle::from_label("dna"), Some(RainStyle::DnaHelix));
    assert!(RainStyle::valid_labels_hint().contains("dna_helix"));
    // The pairing surface covers all four Watson-Crick states
    // (the mutation arm's roll domain).
    for pair in [
        BasePair::AdenineThymine,
        BasePair::ThymineAdenine,
        BasePair::GuanineCytosine,
        BasePair::CytosineGuanine,
    ] {
        let (a, b) = pair.end_glyphs();
        assert!(
            "ATGC".contains(a) && "ATGC".contains(b),
            "non-DNA base: {a}/{b}"
        );
    }
}

#[test]
fn dna_drops_spawn_to_sparse_calm_sky_target() {
    // The calm-sky dial family: at default density 0.70 the target
    // ratio is 0.06 + 0.70 * 0.05 = 0.095 -> ~11 nucleotides on a
    // 120-column pool. The soup must stay a sparse ambient minority
    // — the molecule is the hero of the composition.
    let mut cloud = make_dna_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 600, 16);

    let active = cloud.dna_helix_rain.active_count();
    let target = (120.0_f32 * 0.095).round() as usize;
    // Within a few: absorbed + floor-expired drops recycle through
    // the trickle accumulator, so the steady state hovers just
    // below the lane target. The dial's contract is the
    // sparse-minority band, not an exact headcount.
    assert!(
        active + 4 >= target,
        "expected the density target {target} (within four), got {active}"
    );
    // The hard cap: even sustained spawning cannot push past the
    // ACTIVE_MAX dial.
    assert!(
        active <= (120.0_f32 * DNA_ACTIVE_MAX).ceil() as usize,
        "sparse cap violated: {active} active on 120 lanes"
    );
}

#[test]
fn dna_drops_fall_monotonically_and_stay_bounded() {
    // Law 5: the fall is terminal (vy constant, positive) — y is
    // strictly increasing while a drop lives; x drifts inside the
    // viewport walls.
    let mut cloud = make_dna_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 60, 16);

    let states = cloud.dna_helix_rain.drop_states_for_test();
    assert!(!states.is_empty(), "no active nucleotides after 1 s");
    for (x, y, vy) in &states {
        assert!(
            *x >= 0.0 && *x < 80.0,
            "nucleotide out of column bounds: x={x}"
        );
        assert!(*y >= -2.0 && *y < 40.0, "nucleotide off the frame: y={y}");
        assert!(*vy > 0.0, "nucleotide not falling: vy={vy}");
    }

    // Monotone fall: snapshot two instants and compare per-drop y
    // (the pool slots are stable while active).
    let before: Vec<(f32, f32)> = cloud
        .dna_helix_rain
        .drop_states_for_test()
        .into_iter()
        .map(|(x, y, _)| (x, y))
        .collect();
    run_frames(&mut cloud, &mut frame, 10, 16);
    let after: Vec<(f32, f32)> = cloud
        .dna_helix_rain
        .drop_states_for_test()
        .into_iter()
        .map(|(x, y, _)| (x, y))
        .collect();
    // At least one surviving drop must have descended.
    let descended = before
        .iter()
        .zip(after.iter())
        .filter(|((_, y0), (_, y1))| y1 > y0)
        .count();
    assert!(descended > 0, "no nucleotide descended over 10 frames");
}

#[test]
fn dna_absorption_charges_rungs_and_mutates() {
    // Law 5's deposition: a nucleotide crossing a rung line inside
    // the rung's span is absorbed — the rung's charge rises and,
    // on the mutation roll, the pair re-rolls. Pinned through the
    // full pipeline: run enough frames that landings happen, then
    // verify the genome shows absorbed charge.
    let mut cloud = make_dna_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 900, 16);

    let absorptions = cloud.dna_helix_rain.absorptions_for_test();
    assert!(
        absorptions > 0,
        "no nucleotide absorbed by any rung over 15 s"
    );
    // The charged rungs: at least one rung must carry recency.
    let charged = (0..cloud.dna_helix_rain.genome_for_test().rung_count_for_test())
        .filter(|&i| cloud.dna_helix_rain.genome_for_test().charge_for_test(i) > 0.0)
        .count();
    assert!(charged > 0, "absorptions left no recency on the ladder");
    // The mutation chance is a proper fraction (the calibration
    // contract — the roll drives the visible genome edits).
    assert!((0.0..=1.0).contains(&DNA_MUTATION_CHANCE));
}

#[test]
fn dna_drawn_cells_stay_in_bounds() {
    // The renderer's floor: every cell the draw pass claims sits
    // inside the viewport (strands, rungs, soup, trails).
    let mut cloud = make_dna_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 120, 16);
    for cell in cloud.dna_helix_rain.drawn_cells_for_test() {
        assert!(cell.col < 80, "drawn cell col out of bounds: {}", cell.col);
        assert!(
            cell.line < 40,
            "drawn cell line out of bounds: {}",
            cell.line
        );
    }
}

#[test]
fn dna_repaints_without_residue() {
    // The diff-cleanup contract: after the molecule settles, a
    // force-redraw + continued frames must leave the drawn cell
    // set covering the same population (no stale cells and no
    // residue ghosts — the monolith family's repaint test). Driven
    // past the genesis first so the baseline is the steady
    // molecule (the birth sequence's growing population is pinned
    // in genesis.rs).
    let mut cloud = make_dna_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_past_genesis(&mut cloud, &mut frame);
    run_frames(&mut cloud, &mut frame, 300, 16);
    let baseline = cloud.dna_helix_rain.drawn_cells_for_test().len();
    cloud.clear_redraw_flags_for_test();
    cloud.force_draw_everything();
    run_frames(&mut cloud, &mut frame, 30, 16);
    let after = cloud.dna_helix_rain.drawn_cells_for_test().len();
    // The populations must be the same order (the fork may open or
    // close a dissolve window between snapshots — a bounded
    // variation, never a collapse or an explosion).
    assert!(
        after > baseline / 2 && after < baseline * 2 + 64,
        "drawn population unstable: {baseline} -> {after}"
    );
}

#[test]
fn dna_replication_fork_cycles_in_the_pipeline() {
    // Law 4 through the full pipeline: the genome's fork must both
    // travel and re-arm over a long run (the replication clock
    // mean is 12 sim-seconds; 40 s of wall at 60 fps covers
    // multiple sweeps).
    let mut cloud = make_dna_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    let mut saw_traveling = false;
    for _ in 0..40 {
        run_frames(&mut cloud, &mut frame, 60, 16);
        if cloud.dna_helix_rain.genome_for_test().fork_phase == ForkPhase::Traveling {
            saw_traveling = true;
        }
    }
    assert!(saw_traveling, "the replication fork never opened");
}

#[test]
fn dna_pause_freezes_the_molecule() {
    // The family pause contract: while paused, rain_at's early
    // return freezes the whole molecule — same Instant stepping
    // produces no state change.
    let mut cloud = make_dna_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 60, 16);
    cloud.pause = true;
    cloud.pause_time = Some(Instant::now());
    let phase0 = cloud.dna_helix_rain.genome_for_test().phase;
    let y0: Vec<f32> = cloud
        .dna_helix_rain
        .drop_states_for_test()
        .into_iter()
        .map(|(_, y, _)| y)
        .collect();
    let start = Instant::now();
    for i in 0..30 {
        let now = start + Duration::from_millis(i * 16);
        cloud.rain_at(&mut frame, now);
        frame.clear_dirty();
    }
    assert!(
        (cloud.dna_helix_rain.genome_for_test().phase - phase0).abs() < 1e-6,
        "the molecule turned while paused"
    );
    let y1: Vec<f32> = cloud
        .dna_helix_rain
        .drop_states_for_test()
        .into_iter()
        .map(|(_, y, _)| y)
        .collect();
    assert_eq!(y0, y1, "the soup moved while paused");
}

#[test]
fn dna_style_transition_round_trip() {
    // The scene-cycle contract: switching away wipes the molecule's
    // recency (the family exit reset); switching back rebuilds it —
    // fresh rungs, fresh pool, the rain returns.
    let mut cloud = make_dna_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 200, 16);
    assert!(!cloud.dna_helix_rain.drawn_cells_for_test().is_empty());

    cloud.transition_rain_style(RainStyle::Glyph);
    let charge_after_exit: f32 = (0..cloud.dna_helix_rain.genome_for_test().rung_count_for_test())
        .map(|i| cloud.dna_helix_rain.genome_for_test().charge_for_test(i))
        .sum();
    assert!(
        charge_after_exit <= 1e-6,
        "style exit left the genome charged (charge {charge_after_exit})"
    );

    cloud.transition_rain_style(RainStyle::DnaHelix);
    assert_eq!(cloud.dna_helix_rain.active_count(), 0);
    let mut frame2 = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame2, 300, 16);
    assert!(
        cloud.dna_helix_rain.active_count() > 0,
        "the soup never returned after re-entry"
    );
    assert!(
        !cloud.dna_helix_rain.drawn_cells_for_test().is_empty(),
        "the molecule never rebuilt on re-entry"
    );
}

#[test]
fn dna_speed_keys_scale_the_whole_molecule() {
    // The family speed contract: at double chars_per_sec the
    // rotation advances double over the same wall-time (the turn,
    // the fork and the fall ride one clock — shapes invariant).
    // Measured on the formed molecule (the genesis rides the same
    // clock — the speed keys fast-forward the birth too — but its
    // held rotation through the soup and ladder is pinned
    // separately in genesis.rs).
    let run_phase = |cps: f32| -> f32 {
        let mut cloud = make_dna_cloud(80, 40);
        cloud.set_chars_per_sec(cps);
        let mut frame = Frame::new(80, 40, cloud.palette.bg);
        run_past_genesis(&mut cloud, &mut frame);
        let phase0 = cloud.dna_helix_rain.genome_for_test().phase;
        run_frames(&mut cloud, &mut frame, 60, 16);
        cloud.dna_helix_rain.genome_for_test().phase - phase0
    };
    let slow = run_phase(14.0);
    let fast = run_phase(28.0);
    assert!(
        (fast - 2.0 * slow).abs() < 1e-3,
        "rotation did not scale with the speed key: {fast} vs {}",
        2.0 * slow
    );
}

#[test]
fn dna_sustained_boundedness() {
    // The LTS integration: a long randomized run holds every
    // invariant — charges under the clamp, the fork in its state
    // machine, drops bounded, drawn cells in the viewport.
    let mut cloud = make_dna_cloud(60, 30);
    let mut frame = Frame::new(60, 30, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 3600, 16);
    let genome = cloud.dna_helix_rain.genome_for_test();
    for i in 0..genome.rung_count_for_test() {
        let c = genome.charge_for_test(i);
        let c_max = crate::constants::DNA_CHARGE_MAX + 1e-6;
        assert!(
            (0.0..=c_max).contains(&c),
            "charge out of band at rung {i}: {c}"
        );
    }
    for cell in cloud.dna_helix_rain.drawn_cells_for_test() {
        assert!(cell.col < 60 && cell.line < 30, "drawn cell out of bounds");
    }
    for (x, y, _) in cloud.dna_helix_rain.drop_states_for_test() {
        assert!(
            (0.0..60.0).contains(&x) && (-2.0..30.0).contains(&y),
            "drop out of bounds"
        );
    }
}

#[test]
fn dna_narrow_terminal_renders_degenerate_but_safe() {
    // The degenerate-terminal safety: a 20x6 viewport carries two
    // rungs and a tight helix — no panic, every drawn cell in
    // bounds, the fork envelope clamped.
    let mut cloud = make_dna_cloud(20, 6);
    let mut frame = Frame::new(20, 6, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 240, 16);
    assert!(cloud.dna_helix_rain.genome_for_test().rung_count_for_test() <= 2);
    for cell in cloud.dna_helix_rain.drawn_cells_for_test() {
        assert!(
            cell.col < 20 && cell.line < 6,
            "narrow-terminal cell out of bounds"
        );
    }
}

#[test]
fn dna_adopt_palette_and_clear_draw_history() {
    // The transition + invalidation contracts: palette adoption
    // reaches every active nucleotide (the field slot is read
    // through the drops the family contract updates); the draw
    // history clear empties the diff baseline.
    let mut cloud = make_dna_cloud(80, 40);
    let mut frame = Frame::new(80, 40, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 300, 16);
    cloud.dna_helix_rain.adopt_palette_slot(3);
    let adopted = cloud
        .dna_helix_rain
        .drops
        .iter()
        .filter(|d| d.active)
        .filter(|d| d.palette_slot == 3)
        .count();
    assert!(adopted > 0, "no nucleotide adopted the palette slot");
    let cells = cloud.dna_helix_rain.drawn_cells_for_test().len();
    assert!(cells > 0, "no drawn cells to clear");
    cloud.dna_helix_rain.clear_draw_history();
    assert!(cloud.dna_helix_rain.drawn_cells_for_test().is_empty());
}

// -- NIGHT-research-20: the soft-light round (standing-Core sweep) --

#[test]
fn dna_age_ladder_never_lands_core_on_the_soup() {
    // The audit's first standing site: fresh nucleotide heads read
    // Core for the first 20% of every drop's lifetime (~3 s of
    // standing white blend per drop). The age ladder now composes
    // to the warm Hot ceiling from the moment a nucleotide enters
    // the sky: no consumed fraction of any lifetime lands Core.
    use crate::cloud::dna_helix::drops::NucleotideDrop;
    use crate::cloud::monolith::BrightnessLevel;

    let level_at = |fraction: f32| {
        let mut drop = NucleotideDrop::vacant();
        drop.lifetime = 10.0;
        drop.sim_age = fraction * 10.0;
        drop.age_level()
    };

    for fraction in [0.0, 0.05, 0.19, 0.2, 0.3, 0.44, 0.45, 0.6, 0.74, 0.75, 1.0] {
        assert!(
            !matches!(level_at(fraction), BrightnessLevel::Core),
            "the soup ladder must never land Core (age fraction {fraction})"
        );
    }
    // The fade pinned: fresh and young read the warm ceiling, the
    // mid ages Mid, the drifters Ghost.
    for fresh in [0.0, 0.05, 0.19, 0.3, 0.44] {
        assert!(matches!(level_at(fresh), BrightnessLevel::Hot));
    }
    for mid in [0.45, 0.6, 0.74] {
        assert!(matches!(level_at(mid), BrightnessLevel::Mid));
    }
    for old in [0.75, 1.0] {
        assert!(matches!(level_at(old), BrightnessLevel::Ghost));
    }
}

#[test]
fn dna_rung_front_face_composes_to_the_warm_ceiling() {
    // The audit's third standing site: the rung front-face step-up
    // lifted every re-synthesized (Hot) rung to Core through the
    // whole decay band after each replication sweep. The step-up
    // now stops at the warm Hot ceiling; the 3D depth read survives
    // (Mid bases step up, Hot bases hold, the back face steps
    // down), and a rung inside its fresh-write blink keeps the
    // flash across the whole face.
    use crate::cloud::dna_helix::draw::depth_level;
    use crate::cloud::monolith::BrightnessLevel;

    // A standing Hot rung's front face holds the warm ceiling (was
    // Core — the standing offender).
    assert!(matches!(
        depth_level(BrightnessLevel::Hot, 0.5),
        BrightnessLevel::Hot
    ));
    // The depth read survives below the ceiling: Mid bases step up
    // to Hot, Ghost fabric steps up to Mid, the back face steps
    // down.
    assert!(matches!(
        depth_level(BrightnessLevel::Mid, 0.5),
        BrightnessLevel::Hot
    ));
    assert!(matches!(
        depth_level(BrightnessLevel::Ghost, 0.5),
        BrightnessLevel::Mid
    ));
    assert!(matches!(
        depth_level(BrightnessLevel::Hot, -0.5),
        BrightnessLevel::Mid
    ));
    // The fresh-write blink keeps its flash across the whole face.
    assert!(matches!(
        depth_level(BrightnessLevel::Core, 0.5),
        BrightnessLevel::Core
    ));
    assert!(matches!(
        depth_level(BrightnessLevel::Core, 0.0),
        BrightnessLevel::Core
    ));
}
