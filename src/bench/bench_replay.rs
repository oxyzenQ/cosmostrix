// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! The deterministic replay oracle — byte-identical render sequences,
//! pinned by blake3 (night-frontier-Z1, The Proof Moat 1a).
//!
//! The fixed-seed benchmark already renders headlessly on a virtual
//! screen, but nothing pinned the CONTENT: the sim clock mixes real
//! `Instant::now()` reads (state anchors set at construction/reset)
//! with the virtual `sim_now` ladder, so two runs of the same seed
//! could diverge by microseconds that flip spawn boundaries, RNG
//! reseed rungs, or the 10s density-noise bucket. This module kills
//! that clock mixing ([`crate::cloud::Cloud::anchor_virtual_clock`])
//! and pins the render contract itself:
//!
//! same seed + same invocation + same virtual screen
//!   => the frame sequence hashes byte-identically, run over run.
//!
//! One blake3 digest is computed for EVERY rendered frame
//! ([`crate::bench::frame_digest::frame_digest`], canonical cell
//! serialization), folded into one sequence digest
//! ([`frame_digest::fold_sequence`]), and reported:
//!
//! - `replay: frame=<k> hash=b3:<hex>` — one line per
//!   [`REPLAY_HASH_EVERY_N_FRAMES`] frames (the audit trail: CI logs
//!   show WHICH frame diverged, not just that something did);
//! - `replay-digest: b3:<hex>` — the folded sequence digest, the one
//!   number a CI job compares across two runs (or two hosts).
//!
//! Determinism scope, stated honestly: the sequence pins the core
//! render path (spawn lanes, movement, color, phosphor, climate drift)
//! for a fixed configuration. Inputs that legitimately
//! change the render (scene, style, screen size, fps, density,
//! charset, palette) are echoed on the header block, so a comparison
//! is only meaningful between identical headers. Machine state that
//! would break the contract is force-disabled here (Crystal Dragon
//! ambient polling reads the host CPU/clock). Message overlays,
//! cinematic ghost events, and click effects are not part of the
//! replay surface (the headless loop never arms them; ghost events
//! additionally age on wall-clock `.elapsed()` by design).

use std::time::{Duration, Instant};

use crate::app::CloudConfig;
use crate::constants::{MIN_TERMINAL_COLS, MIN_TERMINAL_LINES, RNG_INITIAL_SEED};
use crate::frame::Frame;
use crate::output::println_safe;

use super::frame_digest::{fold_sequence, frame_digest, hex32};
use super::validate_bench_scene;

/// Print one `replay:` hash line every N rendered frames.
///
/// 25 at the default 60 fps = one line every ~0.42 virtual seconds —
/// dense enough for CI logs to localize a divergence, sparse enough
/// that a 300-frame default run prints 12 lines.
pub(crate) const REPLAY_HASH_EVERY_N_FRAMES: u64 = 25;

/// Default frame count when `--bench-frames` is not given.
///
/// 300 frames at 60 fps = 5 virtual seconds — long enough to cross
/// spawn cadences, phosphor decay windows, and the reseed ladder's
/// short rungs; short enough for a CI budget of well under a second.
pub(crate) const REPLAY_DEFAULT_FRAMES: u64 = 300;

/// Hard ceiling on the replay sequence length.
///
/// `--bench-frames` is a count, and a typo (`--bench-frames
/// 999999999999`) must cost minutes, not days. The clamp is printed
/// so a truncated run can never masquerade as the requested length
/// (the same honesty contract as the benchmark watchdog).
pub(crate) const REPLAY_MAX_FRAMES: u64 = 100_000;

/// The pinned virtual screen — the replay law.
///
/// Unlike [`crate::bench_helpers::bench_dimensions`], this NEVER
/// queries the terminal, the environment, or the host: a replay
/// digest is only comparable between runs that rendered the same
/// rectangle, so the default is a compile-time constant. An explicit
/// `--screen-size WxH` overrides it deterministically (same input,
/// same hash — the header block echoes the rectangle so comparisons
/// stay honest).
const REPLAY_DEFAULT_COLS: u16 = 120;
const REPLAY_DEFAULT_LINES: u16 = 40;

/// Everything the replay computed — returned unparsed so tests can
/// assert on the digests directly, and the printer can stay dumb.
pub(crate) struct ReplayReport {
    pub scene: String,
    pub style: &'static str,
    pub cols: u16,
    pub lines: u16,
    pub frames: u64,
    pub frames_requested: u64,
    pub target_fps: f64,
    pub base_density: f32,
    pub charset_len: usize,
    /// blake3 digest of EVERY rendered frame, in render order.
    pub frame_digests: Vec<[u8; 32]>,
    /// The folded sequence digest (the CI-compared value).
    pub sequence_digest: [u8; 32],
}

/// Compute the replay sequence for a resolved bench configuration.
///
/// Pure computation: no printing, no I/O, no terminal interaction —
/// the in-process determinism test calls this twice and asserts the
/// digest vectors are identical, which is exactly the property the
/// CI job asserts across two processes of the built binary.
pub(crate) fn compute_replay(cfg: &CloudConfig) -> ReplayReport {
    // Strict-validate the bench scene BEFORE any allocation (same
    // fail-fast law as run_benchmark: a typo fails loudly, it does not
    // silently fall back).
    validate_bench_scene(cfg);

    let frames_requested = cfg
        .bench_frames
        .unwrap_or(REPLAY_DEFAULT_FRAMES)
        .clamp(1, REPLAY_MAX_FRAMES);

    // Pinned virtual screen: explicit CLI size wins, otherwise the
    // compile-time constants. Deliberately NOT bench_dimensions() —
    // that helper falls back to env vars and the real terminal size,
    // which would make the digest host-dependent.
    let (w, h) = match cfg.screen_size {
        Some((rw, rh)) => (
            rw.clamp(MIN_TERMINAL_COLS, crate::constants::BENCH_MAX_COLS),
            rh.clamp(MIN_TERMINAL_LINES, crate::constants::BENCH_MAX_LINES),
        ),
        None => (REPLAY_DEFAULT_COLS, REPLAY_DEFAULT_LINES),
    };

    let density = crate::effective_density(cfg.base_density, w, cfg.density_auto);

    let mut cloud = cfg.create_cloud(density);
    // reset_bench clamps to BENCH_MAX_COLS/LINES (8K UHD), mirroring
    // Frame::new_bench — the replay may exceed the interactive cap the
    // same way the benchmark does.
    cloud.reset_bench(w, h);
    // T1.1: keep realloc counters clean (parity with run_benchmark).
    cloud.enable_stuck_cell_sweep = false;
    // Silent arena — a replay log is a digest log, not a verbose dump.
    cloud.set_verbose(false);
    // Crystal Dragon ambient polling reads host CPU/clock state — a
    // machine-dependent palette would break the byte contract. Climate
    // drift (luminance/saturation/hue modulation inside the render
    // path) still runs: it is seeded from the fixed launch RNG.
    cloud.crystal_dragon = false;
    // No component timing: the timing probes read the real clock, and
    // the replay needs nothing from them.

    let target_period = Duration::from_secs_f64(1.0 / cfg.target_fps);
    cloud.set_max_sim_delta(target_period);

    // THE DETERMINISM LAW: every wall-clock-derived state anchor moves
    // onto the virtual clock's origin, so every `elapsed` the engine
    // computes during the sequence is exact `k * period` arithmetic.
    let origin = Instant::now();
    cloud.anchor_virtual_clock(origin);

    let mut frame = Frame::new_bench(w, h, cloud.palette.bg);
    // One scratch buffer, one allocation: steady-state hashing reuses
    // the capacity (the canonical serializer clears, never reallocates
    // beyond growth).
    let mut scratch = Vec::with_capacity((w as usize) * (h as usize) * 16 + 32);

    let mut sim_now = origin;
    let mut digests = Vec::with_capacity(frames_requested as usize);
    for i in 0..frames_requested {
        sim_now += target_period;
        cloud.rain_at(&mut frame, sim_now);
        digests.push(frame_digest(&frame, i + 1, &mut scratch));
        frame.clear_dirty();
    }

    let style = cloud.rain_style;
    ReplayReport {
        scene: cfg.scene_name.clone(),
        style: style.as_str(),
        cols: frame.width,
        lines: frame.height,
        frames: digests.len() as u64,
        frames_requested,
        target_fps: cfg.target_fps,
        base_density: cfg.base_density,
        charset_len: cloud.chars.len(),
        sequence_digest: fold_sequence(&digests),
        frame_digests: digests,
    }
}

/// night-frontier-Z9-D: print the in-process state-digest canary line
/// the soak pass law parses (leg 5). Text report only - the --json wire
/// contract stays one JSON document. A DRIFT verdict is a measurement,
/// not an exit failure: the enforcement point is the soak lane, the
/// same division as the RSS law.
pub(crate) fn print_state_digest_line(t0: &[u8; 32], end: &[u8; 32]) {
    let verdict = if t0 == end { "IDENTICAL" } else { "DRIFT" };
    println_safe!(
        "state-digest: t0=b3:{} end=b3:{} verdict={}",
        hex32(t0),
        hex32(end),
        verdict
    );
}

/// Run the replay oracle and print the parseable report.
pub(crate) fn run_replay(cfg: &CloudConfig) -> std::io::Result<()> {
    let report = compute_replay(cfg);

    println_safe!("Replay:");
    println_safe!("  scene: {}", report.scene);
    println_safe!("  style: {}", report.style);
    println_safe!("  cols: {}", report.cols);
    println_safe!("  lines: {}", report.lines);
    println_safe!(
        "  frames: {} (requested {})",
        report.frames,
        report.frames_requested
    );
    println_safe!("  target_fps: {:.3}", report.target_fps);
    println_safe!("  density: {:.4}", report.base_density);
    println_safe!("  charset_cells: {}", report.charset_len);
    println_safe!("  hash_every: {REPLAY_HASH_EVERY_N_FRAMES}");
    // The seed is compile-time pinned (the reseed ladder derives every
    // later stream from it counter-based), so the value identifies the
    // sequence as surely as the invocation does.
    println_safe!("  seed: {RNG_INITIAL_SEED:#x} (pinned at compile time)");
    println_safe!("  clock: virtual (anchored origin; zero wall-clock residue)");

    for (idx, digest) in report.frame_digests.iter().enumerate() {
        let frame_no = (idx + 1) as u64;
        if frame_no.is_multiple_of(REPLAY_HASH_EVERY_N_FRAMES) || frame_no == report.frames {
            println_safe!("  replay: frame={frame_no} hash=b3:{}", hex32(digest));
        }
    }
    println_safe!("  replay-digest: b3:{}", hex32(&report.sequence_digest));
    Ok(())
}

#[cfg(test)]
#[path = "../../test/bench/replay_tests.rs"]
mod replay_tests;
