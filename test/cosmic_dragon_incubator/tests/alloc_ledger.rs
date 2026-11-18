// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! The zero-alloc ledger — the lean frame path is an allocation-free
//! contract, enforced by the counting global allocator
//! (night-frontier-Z2, Kernel Floor 2b).
//!
//! The README's Philosophy section says the lean path reaches
//! "0.0 allocs/frame (measured)". A number that is only measured in a
//! benchmark window can regress silently between releases; this module
//! turns the measurement into a CONTRACT. The counting global allocator
//! ([`crate::alloc_trace::TraceAlloc`], wired in `src/main.rs`) counts
//! every alloc/dealloc/realloc with per-thread attribution — exactly
//! the mechanism the bench uses — and the lock suite here asserts the
//! delta is EXACTLY ZERO across a pinned steady-state render window:
//!
//!   one ledger frame = `Cloud::rain_at` (the sim + diff step)
//!                    + `BenchIoWriter::write_frame` (the lean
//!                      per-dirty-cell emission the `--bench-scene lean`
//!                      window measures)
//!                    + `Frame::clear_dirty`.
//!
//! A regression (any new per-frame `Vec`, `String`, `Box`, or pool
//! spill) moves the counter, and this suite goes red in CI on the
//! commit that introduced it — the ledger has teeth, proven by the
//! positive-control test at the bottom.
//!
//! Honest scope, stated once: the window covers the LEAN path (sim +
//! diff + lean emission), the same scope the 0.0 claim was measured on.
//! The production-draw I/O path (`Terminal::draw` through the real
//! stdout) carries ~1.1 allocs/frame by design and is NOT this
//! contract; startup, config parse, palette rebuilds, and message
//! overlays are one-shot or interactive surfaces, also not this
//! contract. The window runs on the anchored virtual clock (the Z1
//! replay law) so the ledger measures the path, not the wall clock.

use std::time::{Duration, Instant};

use crate::alloc_trace::AllocSnapshot;
use crate::bench::bench_io::BenchIoWriter;
use crate::cloud::Cloud;
use crate::frame::Frame;

/// The pinned ledger screen: the replay-oracle rectangle (Z1). Same
/// rationale — never the env, never the host terminal.
const LEDGER_COLS: u16 = 120;
const LEDGER_LINES: u16 = 40;

/// Warmup frames before the ledger window opens. Above the bench
/// warmup ceiling (BENCH_WARMUP_MAX_FRAMES = 200 is the bench's
/// largest window; the spawn ramp at 120x40 settles well inside it),
/// so every pool, cache, and SmallVec spill has reached its
/// steady-state capacity before the counter is read.
const LEDGER_WARMUP_FRAMES: u64 = 200;

/// Measured frames under the ledger window. 180 at the anchored 60 fps
/// ladder = 3 virtual seconds of steady-state rain; long enough to
/// cross spawn/reseed boundaries, short enough for the suite budget.
const LEDGER_FRAMES: u64 = 180;

/// The pinned ledger config — the same literal shape as the bench
/// harness configs (see `test/bench/replay_tests.rs`), tuned to the
/// lean-path contract: monolith style, Green/Mono, binary charset,
/// fixed 60 fps, wet lean I/O on, no scene override.
fn make_ledger_config() -> crate::app::CloudConfig {
    crate::app::CloudConfig {
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
        bench_frames: None,
        benchmark: false,
        bench_duration: None,
        save_baseline: None,
        compare_baseline: None,
        bench_io: true,
        bench_all: false,
        bench_scene: None,
        bench_cosmetics: false,
        screen_size: Some((LEDGER_COLS, LEDGER_LINES)),
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

/// Build the pinned ledger rig: the cloud (bench-parity arms, Z1 clock
/// anchor) and its frame. Shared by both ledger tests so the contract
/// and its positive control measure the SAME path.
fn build_ledger_rig() -> (Cloud, Frame, BenchIoWriter) {
    let cfg = make_ledger_config();
    let density = crate::effective_density(cfg.base_density, LEDGER_COLS, cfg.density_auto);

    let mut cloud = cfg.create_cloud(density);
    // Bench parity (run_bench.rs): the same arms the 0.0 measurement
    // was taken under.
    cloud.reset_bench(LEDGER_COLS, LEDGER_LINES);
    cloud.set_component_timing(true);
    cloud.enable_stuck_cell_sweep = false;
    cloud.set_verbose(false);
    cloud.crystal_dragon = false;

    let target_period = Duration::from_secs_f64(1.0 / cfg.target_fps);
    cloud.set_max_sim_delta(target_period);

    // The Z1 determinism law: every wall-clock-derived state anchor
    // moves onto one origin, so the window is exact period arithmetic.
    let origin = Instant::now();
    cloud.anchor_virtual_clock(origin);

    let frame = Frame::new_bench(LEDGER_COLS, LEDGER_LINES, cloud.palette.bg);
    // Wet lean writer — the bench-parity emission target (/dev/null),
    // mirroring Terminal::draw's pre-formatted SGR fast path.
    let writer = BenchIoWriter::with_palette(&cloud.palette)
        .expect("/dev/null is writable on every ledger target");

    (cloud, frame, writer)
}

/// INV-18: the lean frame path performs EXACTLY ZERO heap
/// allocations per frame in steady state.
///
/// The window is the bench's own shape (warmup excluded, per-thread
/// attribution), raised to a lock-suite contract: alloc == 0,
/// dealloc == 0, realloc == 0 across LEDGER_FRAMES ledger frames. Any
/// new per-frame allocation — however small — trips this test and CI
/// goes red on the offending commit.
#[test]
fn lock_inv18_lean_frame_path_performs_zero_allocations() {
    let (mut cloud, mut frame, mut writer) = build_ledger_rig();
    let target_period = Duration::from_secs_f64(1.0 / 60.0);

    let mut sim_now = Instant::now();
    for _ in 0..LEDGER_WARMUP_FRAMES {
        sim_now += target_period;
        cloud.rain_at(&mut frame, sim_now);
        writer.write_frame(&frame);
        frame.clear_dirty();
    }

    // The ledger window opens: per-thread counters, so concurrent
    // test threads cannot pollute this delta (the FreeBSD CI incident
    // design, alloc_trace.rs).
    let before = AllocSnapshot::now();
    for _ in 0..LEDGER_FRAMES {
        sim_now += target_period;
        cloud.rain_at(&mut frame, sim_now);
        writer.write_frame(&frame);
        frame.clear_dirty();
    }
    let after = AllocSnapshot::now();

    let delta = after.delta(&before);
    assert_eq!(
        delta.alloc_calls, 0,
        "the lean frame path allocated {} times across {LEDGER_FRAMES} frames \
         ({} bytes) — the 0.0 allocs/frame contract is broken; a per-frame \
         heap allocation landed in the sim/diff/lean-emit path",
        delta.alloc_calls, delta.bytes_allocated_total
    );
    assert_eq!(
        delta.dealloc_calls, 0,
        "the lean frame path deallocated {} times across {LEDGER_FRAMES} \
         frames — steady-state pools must neither grow nor shrink per frame",
        delta.dealloc_calls
    );
    assert_eq!(
        delta.realloc_calls, 0,
        "the lean frame path reallocated {} times across {LEDGER_FRAMES} \
         frames — buffer growth must be warmup-only, never per-frame",
        delta.realloc_calls
    );
    assert_eq!(
        delta.heap_retained_bytes, 0,
        "the lean frame path retained {} heap bytes — the steady-state \
         heap footprint must be flat",
        delta.heap_retained_bytes
    );
}

/// The ledger's teeth: the SAME window shape with one intentional
/// per-frame allocation counts exactly LEDGER_FRAMES allocations.
///
/// A tripwire that cannot fire is decoration. This positive control
/// proves the counting allocator sees through this exact window
/// shape — if a future refactor silently disables counting (or the
/// window starts running on another thread), this test fails first.
#[test]
fn lock_inv18_ledger_counts_intentional_per_frame_allocations() {
    let (mut cloud, mut frame, mut writer) = build_ledger_rig();
    let target_period = Duration::from_secs_f64(1.0 / 60.0);

    let mut sim_now = Instant::now();
    for _ in 0..LEDGER_WARMUP_FRAMES {
        sim_now += target_period;
        cloud.rain_at(&mut frame, sim_now);
        writer.write_frame(&frame);
        frame.clear_dirty();
    }

    let before = AllocSnapshot::now();
    for _ in 0..LEDGER_FRAMES {
        sim_now += target_period;
        cloud.rain_at(&mut frame, sim_now);
        writer.write_frame(&frame);
        frame.clear_dirty();
        // The deliberate regression: one heap allocation per frame.
        let leak = vec![0u8; 64];
        std::hint::black_box(&leak);
    }
    let after = AllocSnapshot::now();

    let delta = after.delta(&before);
    assert_eq!(
        delta.alloc_calls, LEDGER_FRAMES,
        "the positive control must count exactly one allocation per frame \
         (counted {} over {LEDGER_FRAMES} frames) — if this fires low, the \
         ledger is not measuring this thread's window and the zero-alloc \
         contract is unenforceable",
        delta.alloc_calls
    );
}
