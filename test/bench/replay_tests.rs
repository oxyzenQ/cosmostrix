// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! Replay oracle tests (night-frontier-Z1, Proof Moat 1a).
//!
//! The core law: the SAME invocation must produce a byte-identical
//! frame sequence, run over run, process over process. These tests
//! pin that property in-process (two independent `compute_replay`
//! runs, two independent `Cloud` instances, two independent RNG
//! streams from the same launch seed); the CI replay-oracle step pins
//! the same property across two processes of the built binary by
//! diffing the `replay-digest:` lines.

use super::*;

use crate::app::CloudConfig;
use std::time::Instant;

/// Minimal replay-shaped CloudConfig — the same literal shape as the
/// bench harness configs, tuned for the oracle: fixed 120x40 screen,
/// monolith style, fixed 60 fps, no scene override (the pinned default
/// surface the CI job exercises).
fn make_replay_config(frames: u64, screen: Option<(u16, u16)>) -> CloudConfig {
    CloudConfig {
        color_mode: crate::runtime::ColorMode::Mono,
        shading_mode: crate::runtime::ShadingMode::Random,
        bold_mode: crate::runtime::BoldMode::Off,
        async_mode: false,
        default_bg: true,
        color_scheme: crate::runtime::ColorScheme::Green,
        custom_palette: None,
        custom_palette_name: None,
        rain_style: crate::rain_style::RainStyle::Monolith,
        glitch_enabled: false,
        glitch_level: crate::config::GlitchLevel::None,
        glitch_pct: 0.0,
        glitch_low: 0,
        glitch_high: 0,
        linger_low: 0,
        linger_high: 0,
        short_pct: 0.0,
        die_early_pct: 0.0,
        max_dpc: 1,
        density: 0.8,
        speed: 8.0,
        monolith_size: crate::runtime::MonolithSize::Normal,
        chars: vec!['0', '1'],
        message: None,
        message_border: true,
        msg_fill_style: crate::msg_fill_style::MsgFillStyle::Engrave,
        target_fps: 60.0,
        xtermjs_host: false,
        default_fps_cap: 240.0,
        duration_s: None,
        bench_frames: Some(frames),
        benchmark: false,
        bench_duration: None,
        save_baseline: None,
        compare_baseline: None,
        bench_io: false,
        bench_all: false,
        bench_scene: None,
        bench_cosmetics: false,
        screen_size: screen,
        color_tune: crate::color_tune::ColorTune::IDENTITY,
        json: false,
        verbose: false,
        density_auto: false,
        base_density: 0.8,
        perf_stats: false,
        screensaver: false,
        intro: crate::intro_style::IntroType::None,
        intro_color: None,
        mouse: false,
        charset_preset: String::from("binary"),
        user_ranges: vec![],
        def_ascii: true,
        crystal_dragon: false,
        power_dragon: true,
        msg_mode: true,
        effects_enabled: false,
        config_path_for_watcher: None,
        scene_name: "monolith".to_string(),
        scene_custom_name: None,
        scene_custom_config_owned: false,
        cli_explicit: crate::app::CliExplicit::default(),
        ambient_schedule: crate::crystal_dragon_engine::ambient::AmbientSchedule::default(),
        ambient_snapback_secs: None,
        crystal_dragon_secs: None,
    }
}

/// THE LAW: two runs of the same invocation produce identical digest
/// vectors. The runs construct independent Clouds (fresh RNG from the
/// pinned launch seed, fresh anchored clock), so any wall-clock
/// residue, HashMap iteration order, or RNG stream leak shows up as a
/// digest divergence here.
#[test]
fn replay_sequence_is_byte_identical_across_runs() {
    let a = compute_replay(&make_replay_config(60, None));
    let b = compute_replay(&make_replay_config(60, None));

    assert_eq!(a.frames, 60);
    assert_eq!(a.frame_digests.len(), b.frame_digests.len());
    for (i, (da, db)) in a
        .frame_digests
        .iter()
        .zip(b.frame_digests.iter())
        .enumerate()
    {
        assert_eq!(
            da,
            db,
            "frame {} diverged between identical runs — the replay oracle is not deterministic",
            i + 1
        );
    }
    assert_eq!(a.sequence_digest, b.sequence_digest);
}

/// The digest is input-sensitive: a different pinned screen changes
/// the sequence (the header block echoes the rectangle so CI never
/// compares apples to oranges).
#[test]
fn replay_digest_depends_on_the_pinned_inputs() {
    let a = compute_replay(&make_replay_config(30, None));
    let b = compute_replay(&make_replay_config(30, Some((100, 30))));

    assert_eq!(a.cols, REPLAY_DEFAULT_COLS);
    assert_eq!(a.lines, REPLAY_DEFAULT_LINES);
    assert_eq!(b.cols, 100);
    assert_eq!(b.lines, 30);
    assert_ne!(
        a.sequence_digest, b.sequence_digest,
        "a different virtual screen must produce a different digest (the oracle must have teeth)"
    );
}

/// The sequence length is part of the committed value: the same frame
/// content over a different frame count folds to a different digest
/// (fold_sequence headers the count), so a truncated run can never
/// reuse a longer run's pinned vector.
#[test]
fn sequence_length_is_bound_into_the_fold() {
    let a = compute_replay(&make_replay_config(30, None));
    let b = compute_replay(&make_replay_config(31, None));

    // The first 30 frames are the same sequence...
    assert_eq!(a.frame_digests, b.frame_digests[..30]);
    // ...but the folds differ because the length header differs.
    assert_ne!(a.sequence_digest, b.sequence_digest);
}

/// anchor_virtual_clock moves EVERY wall-clock-derived state field
/// onto the caller's origin — the mechanism the replay law rests on.
/// Field-by-field (the struct has no PartialEq): every Instant anchor
/// must equal the origin exactly after the call.
#[test]
fn anchor_moves_every_wall_clock_state_field() {
    use crate::cloud::Cloud;

    let mut cloud = Cloud::new(
        crate::runtime::ColorMode::Mono,
        crate::runtime::ShadingMode::Random,
        crate::runtime::BoldMode::Off,
        false,
        true,
        crate::runtime::ColorScheme::Green,
        crate::rain_style::RainStyle::Monolith,
    );
    cloud.reset(60, 20);

    // Simulate the real-clock drift the anchor kills: the reset path
    // just stamped real `Instant::now()` into the state fields.
    let origin = Instant::now();

    cloud.anchor_virtual_clock(origin);

    assert_eq!(cloud.last_glitch_time, origin);
    assert_eq!(cloud.next_glitch_time, origin);
    assert_eq!(cloud.last_spawn_time, origin);
    assert_eq!(cloud.last_reseed_time, origin);
    assert_eq!(cloud.last_phosphor_time, origin);
    assert_eq!(cloud.start_anchor, origin);
}

/// The canonical serializer is stable and sensitive: re-serializing
/// the same frame yields the same digest; flipping ONE bit of ONE
/// cell (a bold flag here) changes it. This is what makes the pinned
/// engine-oracle vector (Proof Moat 1c) a real proof — a modified
/// engine cannot keep the vector without drawing the same frame.
#[test]
fn frame_digest_is_stable_and_input_sensitive() {
    use crate::frame::Frame;

    let mut frame = Frame::new(60, 20, None);
    frame.clear_dirty();

    let mut scratch = Vec::new();
    let d1 = frame_digest(&frame, 7, &mut scratch);
    let d2 = frame_digest(&frame, 7, &mut scratch);
    assert_eq!(d1, d2, "same frame state must serialize identically");

    // Same geometry, different frame number → different digest (the
    // position in the sequence is part of the committed bytes).
    let d3 = frame_digest(&frame, 8, &mut scratch);
    assert_ne!(d1, d3);

    // One cell flips one flag → different digest.
    frame.cells[0].bold = true;
    let d4 = frame_digest(&frame, 7, &mut scratch);
    assert_ne!(d1, d4);

    // One cell changes its glyph → different digest.
    let mut frame2 = Frame::new(60, 20, None);
    frame2.clear_dirty();
    frame2.cells[4] = crate::cell::Cell {
        ch: 'X',
        fg: None,
        bg: None,
        bold: false,
    };
    let d5 = frame_digest(&frame2, 7, &mut scratch);
    assert_ne!(d1, d5);
}

/// The fold commits to length AND order: reordered or truncated
/// digest sequences fold differently.
#[test]
fn fold_binds_length_and_order() {
    let d = [[1u8; 32], [2u8; 32], [3u8; 32]];
    let whole = fold_sequence(&d);
    let reordered = fold_sequence(&[d[1], d[0], d[2]]);
    let truncated = fold_sequence(&d[..2]);

    assert_ne!(whole, reordered);
    assert_ne!(whole, truncated);
}

/// The soak's in-process state-digest canary (night-frontier-Z9-D)
/// computes the SAME digest the cross-process anchor computes. The
/// soak harness chains its fresh-process `--bench-replay` anchors to
/// the soaked bench process's in-process `state-digest:` pair (the
/// pass law's leg 1b), which is only meaningful if the premium bench's
/// config surface (benchmark mode, wet I/O, a pinned duration) cannot
/// move the pinned sequence. If a future bench field leaks into the
/// replay surface, this test fails first and the harness must be
/// re-derived on purpose.
#[test]
fn soak_state_digest_canary_matches_the_cross_process_anchor() {
    // The premium bench's config surface is the replay-shaped literal
    // with the soak invocation's fields set: benchmark mode on, a
    // pinned duration (the hosted profile's 150 minutes), wet I/O on,
    // and no explicit frame count (the oracle's 300-frame default).
    let mut bench_cfg = make_replay_config(REPLAY_DEFAULT_FRAMES, None);
    bench_cfg.benchmark = true;
    bench_cfg.bench_duration = Some(150 * 60);
    bench_cfg.bench_io = true;

    let in_process = compute_replay(&bench_cfg);
    let anchor = compute_replay(&make_replay_config(REPLAY_DEFAULT_FRAMES, None));

    assert_eq!(
        in_process.sequence_digest, anchor.sequence_digest,
        "the soak's in-process canary digest must equal the cross-process \
         anchor digest - the bench config surface moved the pinned sequence"
    );
}
