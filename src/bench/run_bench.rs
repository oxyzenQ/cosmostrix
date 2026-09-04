// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Legacy CI benchmark — extracted from `bench/mod.rs` to keep that
//! file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns `run_benchmark()` — the N-frame headless benchmark with
//! parseable BENCH: format output for CI/regression tracking.

use crate::output::println_safe;
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
    // 24h hard ceiling for the frames loop (S-master-HUNT-5, owner
    // security mandate 2026-09-03): `--bench-frames` is a frame COUNT —
    // it cannot be range-checked against the 24h policy at parse time
    // (the wall-clock cost per frame is only knowable at runtime), so
    // the loop itself carries the ceiling as a watchdog. Checked every
    // 4096 frames (one `Instant::elapsed` per 4096 iterations ≈ ns-scale
    // amortized — invisible next to the render work). A `--bench-frames
    // 999999999999` typo now costs at most one day instead of ~190
    // years of held CPU. Same policy value as
    // `cli::cli_parse::DURATION_MAX_SECS` (86400) — duplicated as a
    // literal here because the const is f64-typed for the parser
    // grammar; keeping one policy number in two forms beats a cross-
    // module cast dependency in the bench hot path.
    const FRAMES_WATCHDOG_INTERVAL: u64 = 4096;
    let ceiling = Duration::from_secs(86_400);
    let mut frames_run: u64 = 0;
    let mut hit_ceiling = false;
    for _ in 0..bench_frames {
        if frames_run.is_multiple_of(FRAMES_WATCHDOG_INTERVAL) && start.elapsed() >= ceiling {
            hit_ceiling = true;
            break;
        }
        sim_now += target_period;
        cloud.rain_at(&mut frame, sim_now);
        frame.clear_dirty();
        frames_run += 1;
    }
    let elapsed_s = start.elapsed().as_secs_f64().max(BENCH_ELAPSED_MIN_S);
    // Honesty contract: when the watchdog fires, the report must say so
    // (a truncated frame count without disclosure would look like a
    // completed run — silently corrupting the FPS denominator).
    if hit_ceiling {
        println_safe!(
            "  watchdog: frames loop stopped at the 24h (86400s) hard ceiling after {frames_run}/{bench_frames} frames \
             (cosmostrix caps every time-scale run at one day; re-run with a realistic --bench-frames count)"
        );
    }
    let reported_frames = if hit_ceiling {
        frames_run
    } else {
        bench_frames
    };
    let fps = (reported_frames as f64) / elapsed_s;

    println_safe!("BENCH:");
    println_safe!("  scene: {}", cfg.scene_name);
    // disclose that monolith is the default + how to override, so
    // CI logs and human users can interpret FPS numbers correctly.
    println_safe!(
        "  scene_note: default is 'monolith' (peak throughput); override with --scene <name>"
    );
    if cfg.scene_name != "monolith" {
        println_safe!(
            "  disclaimer: scene '{}' is not peak-throughput; compare with 'monolith' for headline FPS",
            cfg.scene_name
        );
    }
    println_safe!("  cols: {}", w);
    println_safe!("  lines: {}", h);
    println_safe!(
        "  frames: {}",
        if hit_ceiling {
            frames_run
        } else {
            bench_frames
        }
    );
    println_safe!("  elapsed_s: {:.3}", elapsed_s);
    println_safe!("  frames_per_s: {:.3}", fps);
    Ok(())
}
