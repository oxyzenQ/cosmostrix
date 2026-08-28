// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Legacy CI benchmark — extracted from `bench/mod.rs` to keep that
//! file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns `run_benchmark()` — the N-frame headless benchmark with
//! parseable BENCH: format output for CI/regression tracking.

use std::time::{Duration, Instant};

use crate::constants::*;
use crate::frame::Frame;
use crate::{effective_density, CloudConfig};

use super::validate_bench_scene;

pub(crate) fn run_benchmark(cfg: &CloudConfig) -> std::io::Result<()> {
    // Strict-validate --bench-scene BEFORE any allocation so an invalid
    // value (typo like "leanax") fails fast instead of silently falling
    // back to the default lean path. Honesty contract: no hidden behavior.
    validate_bench_scene(cfg);

    let bench_frames = cfg.bench_frames.expect("bench_frames must be set");

    let (w, h) = crate::bench_helpers::bench_dimensions(cfg.screen_size);

    let density = effective_density(cfg.base_density, w, cfg.density_auto);

    let mut cloud = cfg.create_cloud(density);
    // reset_bench clamps to BENCH_MAX_COLS/LINES (8K UHD), mirroring
    // Frame::new_bench — the benchmark intentionally exceeds the
    // interactive 1024x500 safety cap. Triple-engine LTS audit LOW-2:
    // routing through the interactive reset() previously produced a
    // hybrid state (rain spawning at raw bench width while glitch/color
    // coverage stopped at the interactive cap).
    cloud.reset_bench(w, h);
    cloud.set_component_timing(true); // P1: enable sim/render split for benchmark
    cloud.enable_stuck_cell_sweep = false; // T1.1: keep realloc counters clean in benchmark
    cloud.set_verbose(cfg.verbose); // silent arena unless --verbose
    cloud.crystal_dragon = false; // drift spike protection: palette drift in benchmark
                                  // mode corrupts p99/max metrics with palette-rebuild cost.
                                  // Climate drift (luminance/saturation/hue modulation) still runs
                                  // because it is deterministic (fixed RNG seed) and has no
                                  // rebuild cost.

    let mut frame = Frame::new_bench(w, h, cloud.palette.bg);

    let target_period = Duration::from_secs_f64(1.0 / cfg.target_fps);
    cloud.set_max_sim_delta(target_period);

    let warmup_frames = (bench_frames / BENCH_WARMUP_DIVISOR)
        .clamp(BENCH_WARMUP_MIN_FRAMES, BENCH_WARMUP_MAX_FRAMES);
    let mut sim_now = Instant::now();

    for _ in 0..warmup_frames {
        sim_now += target_period;
        cloud.rain_at(&mut frame, sim_now);
        frame.clear_dirty();
    }

    let start = Instant::now();
    for _ in 0..bench_frames {
        sim_now += target_period;
        cloud.rain_at(&mut frame, sim_now);
        frame.clear_dirty();
    }
    let elapsed_s = start.elapsed().as_secs_f64().max(BENCH_ELAPSED_MIN_S);
    let fps = (bench_frames as f64) / elapsed_s;

    println!("BENCH:");
    println!("  scene: {}", cfg.scene_name);
    // disclose that monolith is the default + how to override, so
    // CI logs and human users can interpret FPS numbers correctly.
    println!("  scene_note: default is 'monolith' (peak throughput); override with --scene <name>");
    if cfg.scene_name != "monolith" {
        println!(
            "  disclaimer: scene '{}' is not peak-throughput; compare with 'monolith' for headline FPS",
            cfg.scene_name
        );
    }
    println!("  cols: {}", w);
    println!("  lines: {}", h);
    println!("  frames: {}", bench_frames);
    println!("  elapsed_s: {:.3}", elapsed_s);
    println!("  frames_per_s: {:.3}", fps);
    Ok(())
}
