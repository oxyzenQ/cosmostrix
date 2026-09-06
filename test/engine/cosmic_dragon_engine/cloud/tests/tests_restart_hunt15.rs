// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-hunter-15 regression tests: the 'r' shortkey restart on glyph
//! rain left the previous rain stuck on screen (owner report 2026-09-06,
//! post-e58f8b8).
//!
//! Symptom: pressing 'r' on a glyph-style run did not clear the rain —
//! the screen appeared frozen for a moment, then rain resumed falling
//! ON TOP of the old residue, which stayed forever ("ada bekas rain
//! sebelumnya yang stuck"). Every other style cleared and restarted
//! from the top as documented.
//!
//! Root cause: the 'r' handler calls `cloud.reset()` +
//! `cloud.force_draw_everything()`. `reset_with_bounds` armed
//! `semantic_invalidate` only for STRUCTURED styles; for the droplet
//! family (glyph) the force flag was consumed by the HUNT-25 resync
//! path (`frame.force_repaint()` — re-emit current content, NO clear),
//! so the frame kept the pre-restart glyphs while the simulation state
//! (droplet pool, phosphor arrays) was wiped: no droplet owned those
//! cells anymore and the phosphor decay system no longer tracked them,
//! so nothing could ever blank them. Pre-HUNT-25 the glyph force branch
//! called `frame.clear_with_bg` (which cleared the screen on restart,
//! alongside the resync mass-dump bug HUNT-25 fixed); HUNT-25's
//! force_repaint swap removed the restart clear as collateral damage.
//!
//! Fix: `reset_with_bounds` arms `semantic_invalidate` for EVERY style —
//! a hard reset is a semantic event (the frame content no longer
//! represents truth), so rain_at's semantic branch runs
//! `frame.invalidate_semantic(bg)` (full logical clear + generation
//! bump + terminal LastFrame resync) before the force branch. A bare
//! resync (idle resync, stuck sweep, P2 mitigation — force flag WITHOUT
//! a reset) keeps the HUNT-25 non-perturbing force_repaint contract.

use std::time::{Duration, Instant};

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
    cloud.reset(cols, lines);
    cloud.clear_redraw_flags_for_test();
    cloud
}

/// Draw several frames of live content so the screen holds real rain
/// cells before the restart. Returns the flat indices of all non-blank
/// cells (the residue-to-be).
fn draw_content_and_collect(cloud: &mut Cloud, frame: &mut Frame, frames: u32) -> Vec<usize> {
    // Mirror the flux test harness's production semantics (see
    // tests_flux/mod.rs run_frames): the sim cap is always armed in
    // production, and a 1 s spawn backlog primes the pools. 17 ms >
    // FLUX_SIM_DT so the flux solver takes exactly one step per frame.
    let step_ms = 17u64;
    cloud.set_max_sim_delta(Duration::from_millis(step_ms));
    let start = Instant::now();
    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    for idx in 0..frames {
        cloud.rain_at(frame, start + Duration::from_millis(idx as u64 * step_ms));
        frame.clear_dirty();
    }
    (0..(cloud.cols as usize * cloud.lines as usize))
        .filter(|&i| {
            let cell = frame.cell_at_index_ref(i);
            cell.fg.is_some() || cell.ch != ' '
        })
        .collect()
}

/// The exact 'r' shortkey sequence from interactive/input.rs.
fn press_r(cloud: &mut Cloud, frame: &mut Frame) {
    cloud.reset(frame.width, frame.height);
    cloud.force_draw_everything();
    cloud.restart_message_typewriter();
}

/// THE bug: glyph restart must clear the old rain. Pre-fix, every cell
/// drawn before the restart survived it untouched (force_repaint
/// re-emitted the stale content; no droplet/phosphor tracking could
/// ever blank it again).
#[test]
fn hunt15_restart_glyph_clears_old_rain() {
    let mut cloud = make_cloud(); // Glyph family, 20x10
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    frame.clear_dirty();

    let drawn = draw_content_and_collect(&mut cloud, &mut frame, 12);
    assert!(!drawn.is_empty(), "precondition: rain drew glyphs");
    let gen_before = frame.current_gen();
    let semantic_gen_before = frame.semantic_gen;

    press_r(&mut cloud, &mut frame);
    // First post-restart frame: reset set last_spawn_time = now, so the
    // spawn budget is ~zero — no new droplets, nothing redrawn.
    cloud.rain_at(&mut frame, Instant::now());

    // 1. The content epoch must have been bumped (a logical clear via
    //    invalidate_semantic / clear_with_bg — NOT a force_repaint,
    //    which preserves the generation).
    assert_ne!(
        frame.current_gen(),
        gen_before,
        "restart must invalidate the frame content (force_repaint alone \
         re-emits the stale pre-restart rain)"
    );
    // 2. The terminal's LastFrame must be flagged for a full resync.
    assert_ne!(
        frame.semantic_gen, semantic_gen_before,
        "restart must bump semantic_gen so the terminal cache repaints"
    );
    // 3. No pre-restart cell may retain its old glyph.
    for &i in &drawn {
        let cell = frame.cell_at_index_ref(i);
        assert!(
            cell.fg.is_none() && cell.ch == ' ',
            "cell {i} still holds pre-restart residue (ch={:?}, fg={:?})",
            cell.ch,
            cell.fg
        );
    }
}

/// The restart contract for ALL seven styles: after 'r', no cell may
/// retain PRE-restart content unless the fresh simulation legitimately
/// rewrote it this frame. Glyph fails pre-fix; the six structured
/// styles passed via their clear_with_bg force branch — this pins them
/// so none of the seven can ever regress.
/// 120 frames (~2 s at 16 ms) at 60x25: flux ramps its mote pool under
/// production sim-cap semantics (no artificial backlog — see
/// tests_flux/core.rs flux_spawn_reaches_density_target).
#[test]
fn hunt15_restart_clears_old_rain_for_every_style() {
    for style in [
        RainStyle::Glyph,
        RainStyle::Monolith,
        RainStyle::Vortex,
        RainStyle::Flux,
        RainStyle::Lorenz,
        RainStyle::Dragon,
        RainStyle::Physarum,
    ] {
        let mut cloud = make_style_cloud(style, 60, 25);
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        frame.clear_dirty();

        let drawn = draw_content_and_collect(&mut cloud, &mut frame, 120);
        assert!(
            !drawn.is_empty(),
            "{style:?}: precondition — style drew visible content"
        );

        press_r(&mut cloud, &mut frame);
        cloud.rain_at(&mut frame, Instant::now());

        let residue = drawn.iter().any(|&i| {
            let cell = frame.cell_at_index_ref(i);
            (cell.fg.is_some() || cell.ch != ' ') && !frame.cell_written_this_frame(i)
        });
        assert!(
            !residue,
            "{style:?}: cells retain pre-restart content after 'r' — \
             restart must fully invalidate the frame"
        );
    }
}

/// A bare resync (force_draw_everything WITHOUT a reset) keeps the
/// HUNT-25 contract: existing content and phosphor state are preserved,
/// no generation bump. This is what separates a maintenance resync from
/// the semantic restart fixed above.
#[test]
fn hunt15_bare_resync_still_preserves_content() {
    let mut cloud = make_cloud();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    frame.clear_dirty();

    let _ = draw_content_and_collect(&mut cloud, &mut frame, 6);
    let drawn_before = (0..(cloud.cols as usize * cloud.lines as usize))
        .filter(|&i| {
            let cell = frame.cell_at_index_ref(i);
            cell.fg.is_some() || cell.ch != ' '
        })
        .count();
    assert!(drawn_before > 0);
    let gen_before = frame.current_gen();
    let semantic_gen_before = frame.semantic_gen;

    cloud.force_draw_everything();
    cloud.rain_at(&mut frame, Instant::now());

    // force_repaint path: content preserved, generation untouched.
    assert_eq!(frame.current_gen(), gen_before);
    assert_eq!(frame.semantic_gen, semantic_gen_before);
    let drawn_after = (0..(cloud.cols as usize * cloud.lines as usize))
        .filter(|&i| {
            let cell = frame.cell_at_index_ref(i);
            cell.fg.is_some() || cell.ch != ' '
        })
        .count();
    assert_eq!(
        drawn_after, drawn_before,
        "a bare resync must not clear or duplicate content"
    );
}
