// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-lts-3 restart-consistency tests: the 'r' shortkey must
//! behave exactly like a fresh startup (the owner's report: the
//! restarted black hole "is not a real start from zero, very
//! different from startup").
//!
//! Three contracts:
//! 1. The birth choreography re-arms on restart for the four
//!    choreographed families (black hole formation, DNA genesis,
//!    quasar ignition, neural genesis) — the same re-entry
//!    contract scene switches already honor.
//! 2. The fresh-start state: the deterministic RNG stream
//!    re-seeds (every launch replays the same rolls), the time
//!    anchor re-captures, and the pause/resume easing clears.
//! 3. Dynamic screen size: every style resets and resumes live,
//!    in-bounds content on both a resize up and a resize down.

use std::time::{Duration, Instant};

use rand::distr::Distribution;
use rand::{rngs::StdRng, SeedableRng};

use super::make_cloud;
use crate::cloud::Cloud;
use crate::frame::Frame;
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};

fn make_style_cloud(style: RainStyle, cols: u16, lines: u16) -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::Mono,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Green,
        style,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.set_droplet_density(0.9);
    cloud.set_chars_per_sec(18.0);
    cloud.set_max_sim_delta(Duration::from_millis(17));
    cloud.reset(cols, lines);
    cloud.clear_redraw_flags_for_test();
    cloud
}

fn run_frames(cloud: &mut Cloud, frame: &mut Frame, frames: u32, step_ms: u64) {
    let start = Instant::now();
    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    for idx in 0..frames {
        let now = start + Duration::from_millis(idx as u64 * step_ms);
        cloud.rain_at(frame, now);
        frame.clear_dirty();
    }
}

/// The exact 'r' shortkey sequence from interactive/input.rs.
fn press_r(cloud: &mut Cloud, frame: &mut Frame) {
    cloud.restart_from_zero(frame.width, frame.height);
    cloud.restart_message_typewriter();
}

fn visible_cells(cloud: &Cloud, frame: &Frame) -> usize {
    (0..(cloud.cols as usize * cloud.lines as usize))
        .filter(|&i| {
            let cell = frame.cell_at_index_ref(i);
            cell.fg.is_some() || cell.ch != ' '
        })
        .count()
}

// ── Contract 1: the birth choreography re-arms on restart ────────

/// The owner's exact repro: a formed black hole, restarted via 'r',
/// must re-form from the singularity dot — the same sequence a
/// fresh startup plays (the resize path deliberately keeps the
/// steady state; the restart path must NOT).
#[test]
fn restart_replays_the_black_hole_formation() {
    let (cols, lines) = (120, 40);
    let mut cloud = make_style_cloud(RainStyle::BlackHole, cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);

    // Drive the formation to completion (220 frames x 16 ms = 3.52 s
    // > the ~3.1 s sequence, the tests_black_hole steady recipe).
    run_frames(&mut cloud, &mut frame, 220, 16);
    assert!(
        cloud.black_hole_rain.formed_for_test(),
        "precondition: the hole finished forming"
    );

    press_r(&mut cloud, &mut frame);
    assert!(
        !cloud.black_hole_rain.formed_for_test(),
        "'r' must re-arm the formation (the owner's bug: the restarted hole popped in already formed)"
    );
    assert!(
        (cloud.black_hole_rain.formation_t_for_test()).abs() < f32::EPSILON,
        "'r' must rewind the formation clock to zero"
    );

    // The replay begins with the seed dot: a few frames in, the
    // drawn-cell set is the dot (+ the collapse cross at most).
    run_frames(&mut cloud, &mut frame, 30, 16);
    let drawn = cloud.black_hole_rain.drawn_cells_for_test();
    assert!(
        !drawn.is_empty() && drawn.len() <= 5,
        "the restart replay must begin with the seed phase (got {} cells)",
        drawn.len()
    );
    let cx = (cols - 1) / 2;
    let cy = (lines - 1) / 2;
    assert!(
        drawn.iter().any(|c| c.col == cx && c.line == cy),
        "the reborn seed must sit at the viewport center"
    );
}

/// The DNA molecule: a formed helix, restarted, must re-transcribe
/// from the primordial soup (the genesis clock rewinds, the formed
/// flag re-arms).
#[test]
fn restart_replays_the_dna_genesis() {
    let (cols, lines) = (90, 30);
    let mut cloud = make_style_cloud(RainStyle::DnaHelix, cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);

    // 600 frames x 17 ms = 10.2 s — past the full genesis sequence
    // (soup -> ladder -> windup -> steady; the bench note caps the
    // sequence at ~60 percent of a 10 s window).
    run_frames(&mut cloud, &mut frame, 600, 17);
    assert!(
        cloud.dna_helix_rain.genome_for_test().formed_for_test(),
        "precondition: the molecule finished its genesis"
    );

    press_r(&mut cloud, &mut frame);
    assert!(
        !cloud.dna_helix_rain.genome_for_test().formed_for_test(),
        "'r' must re-arm the DNA genesis"
    );
    assert!(
        (cloud.dna_helix_rain.genome_for_test().genesis_t_for_test()).abs() < f32::EPSILON,
        "'r' must rewind the genesis clock to zero"
    );
}

/// The quasar engine: a lit engine, restarted, must re-ignite from
/// the cold cloud (the ignition clock rewinds, the lit flag
/// re-arms).
#[test]
fn restart_replays_the_quasar_ignition() {
    let (cols, lines) = (90, 30);
    let mut cloud = make_style_cloud(RainStyle::Quasar, cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);

    // 600 frames x 17 ms = 10.2 s — past the ignition sequence
    // (cold cloud -> disk condensation -> core lights -> jets).
    run_frames(&mut cloud, &mut frame, 600, 17);
    assert!(
        cloud.quasar_rain.lit_for_test(),
        "precondition: the engine finished igniting"
    );

    press_r(&mut cloud, &mut frame);
    assert!(
        !cloud.quasar_rain.lit_for_test(),
        "'r' must re-arm the quasar ignition"
    );
    assert!(
        (cloud.quasar_rain.ignition_t_for_test()).abs() < f32::EPSILON,
        "'r' must rewind the ignition clock to zero"
    );
}

/// The neural machine: a trained network, restarted, must re-run
/// the genesis (the genesis clock rewinds, the lit flag re-arms).
#[test]
fn restart_replays_the_neural_genesis() {
    let (cols, lines) = (90, 30);
    let mut cloud = make_style_cloud(RainStyle::Neural, cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);

    // 600 frames x 17 ms = 10.2 s — past the genesis sequence
    // (data falls -> layers build -> dendrites reach -> first
    // thought fires).
    run_frames(&mut cloud, &mut frame, 600, 17);
    assert!(
        cloud.neural_rain.lit_for_test(),
        "precondition: the machine finished its genesis"
    );

    press_r(&mut cloud, &mut frame);
    assert!(
        !cloud.neural_rain.lit_for_test(),
        "'r' must re-arm the neural genesis"
    );
    assert!(
        (cloud.neural_rain.genesis_t_for_test()).abs() < f32::EPSILON,
        "'r' must rewind the genesis clock to zero"
    );
}

// ── Contract 2: the fresh-start state ─────────────────────────────

/// The RNG stream rewinds to the construction zero on restart:
/// Cloud::new seeds StdRng from RNG_INITIAL_SEED, so every launch
/// draws from the same deterministic stream. A restart cannot sit
/// at the exact same stream POSITION as a cold launch (the
/// construction path — pool builds, charset setup — consumes
/// thousands of draws a restart does not replay, and the count
/// varies with config), so the honest equivalence is the STREAM,
/// not the position. Three properties pin it:
/// (1) the rewind really happens — the post-restart draws are not
///     the burned stream's continuation;
/// (2) every restart rewinds to the same zero — 'r' twice replays
///     the identical post-restart draws (deterministic replay);
/// (3) the post-restart draws live inside the freshly-seeded
///     construction stream (same RNG_INITIAL_SEED family).
#[test]
fn restart_reseeds_the_rng_stream_from_zero() {
    let mut cloud = make_style_cloud(RainStyle::Monolith, 40, 12);
    // Burn the stream far past any launch position.
    for _ in 0..1_000 {
        let _: f32 = cloud.rand_chance.sample(&mut cloud.mt);
    }
    // (1) Capture the burned stream's continuation first.
    let continuation: Vec<f32> = (0..16)
        .map(|_| cloud.rand_chance.sample(&mut cloud.mt))
        .collect();

    // Restart: the stream must NOT continue where the burn left it.
    let (w, h) = (cloud.cols, cloud.lines);
    cloud.restart_from_zero(w, h);
    let first: Vec<f32> = (0..16)
        .map(|_| cloud.rand_chance.sample(&mut cloud.mt))
        .collect();
    assert_ne!(
        first, continuation,
        "the restart must rewind the RNG, not continue the burned stream"
    );

    // (3) The construction stream: a generous window search for the
    // restart's draws inside the freshly-seeded reference (the
    // reset path's own setup draws offset the restart window;
    // 65536 draws covers it with orders of magnitude to spare).
    let dist = cloud.rand_chance;
    let mut reference = StdRng::seed_from_u64(crate::constants::RNG_INITIAL_SEED);
    let launch_stream: Vec<f32> = (0..65_536).map(|_| dist.sample(&mut reference)).collect();
    assert!(
        launch_stream.windows(16).any(|win| win == first.as_slice()),
        "the post-restart draws must live in the RNG_INITIAL_SEED launch stream"
    );

    // (2) The same zero every time: burn again, restart again, the
    // replay is identical.
    for _ in 0..500 {
        let _: f32 = cloud.rand_chance.sample(&mut cloud.mt);
    }
    cloud.restart_from_zero(w, h);
    let second: Vec<f32> = (0..16)
        .map(|_| cloud.rand_chance.sample(&mut cloud.mt))
        .collect();
    assert_eq!(
        first, second,
        "every restart must rewind to the same deterministic zero"
    );
}

/// The time anchor re-captures and the pause/resume easing clears:
/// a restart mid-pause must come back unpaused at full rate, the
/// same state a fresh launch starts in.
#[test]
fn restart_clears_pause_and_recaptures_the_anchor() {
    let mut cloud = make_style_cloud(RainStyle::Glyph, 40, 12);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 10, 17);

    // Simulate a paused session (the 'p' family state).
    let pause_instant = Instant::now();
    cloud.pause = true;
    cloud.pause_start = Some(pause_instant);
    cloud.pause_time = Some(pause_instant);
    cloud.resume_blend = 0.3;
    cloud.resume_start = Some(pause_instant);
    cloud.resume_blend_start = 0.05;
    let anchor_before = cloud.start_anchor;

    press_r(&mut cloud, &mut frame);

    assert!(!cloud.pause, "'r' must come back unpaused");
    assert!(cloud.pause_start.is_none(), "'r' must clear pause_start");
    assert!(cloud.pause_time.is_none(), "'r' must clear pause_time");
    assert!(cloud.resume_start.is_none(), "'r' must clear resume_start");
    assert_eq!(cloud.resume_blend, 1.0, "'r' must restore full rate");
    assert_eq!(cloud.resume_blend_start, 0.0);
    assert!(
        cloud.start_anchor >= anchor_before,
        "'r' must recapture the time anchor (a fresh launch anchors at construction)"
    );
}

/// The default scene (cinematic glyph rain) restarts like a fresh
/// launch: the pool re-fills through natural spawn (no warm-start
/// ramp — that belongs to style transitions, not launches), and
/// the restarted rain draws the same first-second content shape.
#[test]
fn restart_cinematic_glyph_refills_like_a_fresh_launch() {
    let (cols, lines) = (60, 25);
    let mut cloud = make_cloud();
    cloud.reset(cols, lines);
    cloud.clear_redraw_flags_for_test();
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);

    // Draw a full screen of rain, then restart.
    run_frames(&mut cloud, &mut frame, 120, 17);
    let before = visible_cells(&cloud, &frame);
    assert!(before > 0, "precondition: glyph rain drew content");

    press_r(&mut cloud, &mut frame);

    // The first post-restart frame holds no residue (hunt15's
    // contract) and the pool refills over the following second —
    // the same shape a fresh launch shows one second in.
    run_frames(&mut cloud, &mut frame, 90, 17);
    let after = visible_cells(&cloud, &frame);
    assert!(
        after > 0,
        "the restarted glyph rain must refill (the fresh-launch fill)"
    );
    assert!(
        cloud.glyph_entry_time.is_none(),
        "a launch carries no transition ramp"
    );
}

// ── Contract 3: dynamic screen size for every style ───────────────

/// Every one of the fourteen styles survives a resize up and a
/// resize down: the reset rebuilds for the new viewport, the rain
/// resumes live in-bounds content, nothing panics. (The resize
/// path is the resize-semantics reset — the steady state is kept
/// by design; this pins the dynamic-size support the owner
/// directed the audit to verify for every type rain.)
#[test]
fn resize_up_and_down_keeps_every_style_live() {
    for style in [
        RainStyle::Glyph,
        RainStyle::Monolith,
        RainStyle::Vortex,
        RainStyle::Flux,
        RainStyle::Lorenz,
        RainStyle::Dragon,
        RainStyle::Physarum,
        RainStyle::BlackHole,
        RainStyle::Aeolian,
        RainStyle::SolarFlare,
        RainStyle::DnaHelix,
        RainStyle::Murmuration,
        RainStyle::Quasar,
        RainStyle::Neural,
    ] {
        // Start at 60x25 and establish live content.
        let mut cloud = make_style_cloud(style, 60, 25);
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        run_frames(&mut cloud, &mut frame, 120, 17);
        assert!(
            visible_cells(&cloud, &frame) > 0,
            "{style:?}: precondition — style drew visible content at 60x25"
        );

        // Resize UP (the event-loop sequence: cloud.reset + fresh
        // Frame, mirroring event_loop.rs's pending_resize apply).
        cloud.reset(120, 45);
        frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        run_frames(&mut cloud, &mut frame, 120, 17);
        assert!(
            visible_cells(&cloud, &frame) > 0,
            "{style:?}: rain must resume live content after the resize up to 120x45"
        );

        // Resize DOWN: the geometry must re-anchor to the smaller
        // viewport with no out-of-bounds residue (Frame indexing is
        // bounds-checked; a family writing stale geometry panics or
        // misplaces content here).
        cloud.reset(48, 16);
        frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        run_frames(&mut cloud, &mut frame, 120, 17);
        assert!(
            visible_cells(&cloud, &frame) > 0,
            "{style:?}: rain must resume live content after the resize down to 48x16"
        );
        assert_eq!(cloud.cols, 48, "{style:?}: cols track the viewport");
        assert_eq!(cloud.lines, 16, "{style:?}: lines track the viewport");
    }
}

/// The resize path must NOT replay the birth choreography (the
/// documented steady-state contract — distinct from 'r', which
/// must): a formed black hole stays formed across a resize.
#[test]
fn resize_keeps_the_steady_state_distinct_from_restart() {
    let mut cloud = make_style_cloud(RainStyle::BlackHole, 120, 40);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 220, 16);
    assert!(cloud.black_hole_rain.formed_for_test());

    cloud.reset(100, 30);
    assert!(
        cloud.black_hole_rain.formed_for_test(),
        "a pure resize must keep the steady state (the documented resize contract)"
    );

    // And the restart still re-arms after the resize.
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    press_r(&mut cloud, &mut frame);
    assert!(
        !cloud.black_hole_rain.formed_for_test(),
        "'r' after a resize must still replay the formation"
    );
}
