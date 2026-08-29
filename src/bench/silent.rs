// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Silent benchmark measurement — extracted from `bench/mod.rs` to
//! keep that file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns `run_premium_benchmark_silent()` — the internal measurement
//! loop (no output) that runs warmup + measurement frames + collects
//! all metrics into BenchReportData. Called by run_premium_benchmark
//! and run_benchmark_capture.

use std::time::{Duration, Instant};

use crate::bench_comp::ComponentTimer;
use crate::cinematic::dirty_threshold_cells;
use crate::constants::*;
use crate::frame::Frame;
use crate::theme::canonical_name_for_scheme;
use crate::{
    bench_helpers::resolve_bench_duration, bench_report::BenchReportData, effective_density,
    CloudConfig,
};

use super::premium::FRAME_TIME_SAMPLES;
use super::validate_bench_scene;

/// Internal: run benchmark measurement and return data (no output).
pub(crate) fn run_premium_benchmark_silent(cfg: &CloudConfig) -> std::io::Result<BenchReportData> {
    // Strict-validate --bench-scene so typos are rejected even when called
    // via run_benchmark_capture (used by --bench-all).
    validate_bench_scene(cfg);

    let bench_duration_secs = crate::ux::or_exit(resolve_bench_duration(cfg.bench_duration));

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
    cloud.set_component_timing(true);
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

    // Phase 2: wet I/O
    // T2.1: pass palette so BenchIoWriter can build a ColorCache and mirror
    // the production Terminal::draw() fast path (pre-formatted SGR bytes).
    // --bench-scene production-draw routes the writer through the production
    // draw path (mirrors Terminal::draw instead of emit_cell_lean).
    let bench_scene_production = cfg
        .bench_scene
        .as_deref()
        .map(|s| s == "production-draw")
        .unwrap_or(false);
    let mut io_writer = if cfg.bench_io {
        crate::bench_io::BenchIoWriter::with_palette(&cloud.palette)
    } else {
        None
    };

    // perf counters must be opened before warmup (fd + ENABLE ioctl), but
    // the baseline read is taken AFTER warmup so the delta only covers the
    // measurement window — matching total_elapsed_s and total_frames scope.
    let perf_handle = crate::bench_perf::open_counters();
    let mut visual_sampler = crate::bench_visual::VisualSampler::new(10);

    // Warmup
    let warmup_end =
        Instant::now() + Duration::from_secs(crate::bench_helpers::bench_warmup_secs());
    let mut sim_now = Instant::now();
    while Instant::now() < warmup_end {
        sim_now += target_period;
        cloud.rain_at(&mut frame, sim_now);
        if let Some(ref mut io) = io_writer {
            if bench_scene_production {
                io.write_frame_production(&frame);
            } else {
                io.write_frame(&frame);
            }
        }
        frame.clear_dirty();
    }

    // Baseline snapshots (scope = measurement phase only)
    // Taken AFTER warmup so the delta excludes warmup cycles/energy/allocs,
    // matching total_elapsed_s and total_frames which also exclude warmup.
    let alloc_before = crate::alloc_trace::AllocSnapshot::now();
    let energy_before = crate::bench_energy::EnergySnapshot::now();
    let perf_before = perf_handle.as_ref().map(|h| h.read()).unwrap_or_default();

    // Measurement
    let start = Instant::now();
    let bench_end = start + Duration::from_secs(bench_duration_secs);
    let mut frame_times: [f64; FRAME_TIME_SAMPLES] = [0.0; FRAME_TIME_SAMPLES];
    let mut ft_index = 0;
    let mut total_frames = 0u64;
    let mut drawn_frames = 0u64;
    let mut total_drawn_cells = 0u64;
    let mut max_dirty_cells = 0u64;
    let mut dirty_all_frames = 0u64;
    let mut components = ComponentTimer::new();

    let total_cells = (w as usize) * (h as usize);
    let dirty_threshold = dirty_threshold_cells(total_cells, DIRTY_THRESHOLD_RATIO);

    while Instant::now() < bench_end {
        sim_now += target_period;
        let frame_start = Instant::now();
        cloud.rain_at(&mut frame, sim_now);

        let sim_ms = cloud.last_sim_ms();
        let render_ms = cloud.last_render_ms();

        let is_dirty_all = frame.is_dirty_all();
        let dirty_len = frame.dirty_indices().len();
        let did_draw = is_dirty_all || dirty_len > 0;
        let dirty_count = if is_dirty_all { total_cells } else { dirty_len };
        if did_draw {
            drawn_frames += 1;
            total_drawn_cells += dirty_count as u64;
        }
        max_dirty_cells = max_dirty_cells.max(dirty_count as u64);
        if is_dirty_all {
            dirty_all_frames += 1;
        }

        if let Some(ref mut io) = io_writer {
            if bench_scene_production {
                io.write_frame_production(&frame);
            } else {
                io.write_frame(&frame);
            }
        }
        visual_sampler.sample(&frame);
        frame.clear_dirty();

        let frame_time_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
        let io_ms = (frame_time_ms - sim_ms - render_ms).max(0.0);
        components.record(sim_ms, render_ms, io_ms);

        if ft_index < FRAME_TIME_SAMPLES {
            frame_times[ft_index] = frame_time_ms;
            ft_index += 1;
        }
        total_frames += 1;
    }

    let total_elapsed_s = start.elapsed().as_secs_f64().max(BENCH_ELAPSED_MIN_S);
    let elapsed_s = total_elapsed_s;

    // Finalize collectors
    let terminal_io = io_writer.map(|io| io.finalize(total_elapsed_s));
    let alloc_after = crate::alloc_trace::AllocSnapshot::now();
    let energy_after = crate::bench_energy::EnergySnapshot::now();
    let perf_after = perf_handle.as_ref().map(|h| h.read()).unwrap_or_default();
    let visual_metrics = visual_sampler.finalize();

    let mut alloc_metrics = alloc_after.delta(&alloc_before);
    alloc_metrics.alloc_calls_per_frame = if total_frames > 0 {
        alloc_metrics.alloc_calls as f64 / total_frames as f64
    } else {
        0.0
    };
    alloc_metrics.dealloc_calls_per_frame = if total_frames > 0 {
        alloc_metrics.dealloc_calls as f64 / total_frames as f64
    } else {
        0.0
    };
    alloc_metrics.read_proc_heap();

    let energy_metrics = energy_after.delta(
        &energy_before,
        total_elapsed_s,
        total_frames,
        total_drawn_cells,
    );
    let perf_metrics = perf_after.delta(&perf_before);

    // Compute summary metrics
    let avg_fps = (total_frames as f64) / elapsed_s;
    // peak_fps: derived from the minimum non-zero frame time.
    // On fast systems, some frames complete within a single clock tick
    // (elapsed = 0.0). The silent capture path does not collect frame_times,
    // so peak_fps is always 0.0 here.
    let peak_fps = 0.0; // not measured: silent capture has no frame_times array
                        // v50 LTS audit fix: previously this was
                        // `perf_work_sum_s * 1000.0 / total_frames`, where `perf_work_sum_s`
                        // was the sum of per-frame `frame_start.elapsed()` measurements. That
                        // had the same FreeBSD `clock_gettime` syscall bias as the visible
                        // path: the ~10 µs/frame of loop bookkeeping was missed, so
                        // `avg_frame_time` came out ~28 % low and inconsistent with
                        // `avg_fps`. The dead `perf_work_sum_s` / `perf_work_max_s` collectors
                        // and the redundant second `frame_start.elapsed()` call per frame
                        // were removed at the same time — they were never read by any
                        // downstream consumer and only added measurement overhead.
    let avg_frame_time = if total_frames > 0 {
        elapsed_s * 1000.0 / total_frames as f64
    } else {
        0.0
    };
    let avg_dirty_cells_per_frame = if total_frames > 0 {
        total_drawn_cells as f64 / total_frames as f64
    } else {
        0.0
    };
    let (avg_sim_ms, avg_render_ms, avg_io_ms, _max_sim, _max_render, _max_io) =
        components.finalize();

    let enrichment = crate::bench_config_enrichment::compute_config_enrichment(cfg);
    let report_data = BenchReportData {
        was_interrupted: false,
        w,
        h,
        color_mode: cfg.color_mode,
        target_fps: cfg.target_fps,
        density: cfg.density,
        speed: cfg.speed,
        scene: cfg.scene_name.clone(),
        color_scheme_name: canonical_name_for_scheme(cfg.color_scheme)
            .unwrap_or("unknown")
            .to_string(),
        charset_preset: cfg.charset_preset.clone(),
        glyph_count: cfg.chars.len(),
        rain_style: cfg.rain_style.as_str(),
        monolith_size: cfg.monolith_size.as_str(),
        bold_mode: format!("{:?}", cfg.bold_mode),
        shading_mode: format!("{:?}", cfg.shading_mode),
        color_mode_label: enrichment.color_mode_label,
        custom_palette_name: enrichment.custom_palette_name,
        custom_palette_bg_hex: enrichment.custom_palette_bg_hex,
        color_bg_label: enrichment.color_bg_label,
        color_tune_summary: enrichment.color_tune_summary,
        async_mode: enrichment.async_mode,
        glitch_enabled: enrichment.glitch_enabled,
        glitch_level: enrichment.glitch_level,
        glitch_pct: enrichment.glitch_pct,
        color_pipeline: enrichment.color_pipeline,
        chroma_in_benchmark: enrichment.chroma_in_benchmark,
        power_dragon: enrichment.power_dragon,
        crystal_dragon: enrichment.crystal_dragon,
        msg_mode: enrichment.msg_mode,
        intro: enrichment.intro,
        no_effects: enrichment.no_effects,
        avg_fps,
        peak_fps,
        avg_frame_time,
        // v30 strengthen (audit): the silent capture path (used by --bench-all)
        // does not compute percentile/jitter/stability
        // metrics because those require the full frame_times array + sort,
        // and the callers only read avg_fps + ns/cell + alloc/visual metrics.
        // The previous code hardcoded `jitter_classification: "low"` and
        // `frame_time_stability: "excellent"` — misleading quality verdicts
        // that asserted "excellent" stability without actually measuring it.
        // Now we honestly label these as "not measured (silent capture)" so
        // any future caller that reads these fields knows they're placeholders.
        p99_frame_time: 0.0,
        p95_frame_time: 0.0,
        max_frame_time: 0.0,
        p99_9_frame_time: 0.0,
        jitter_classification: "not measured (silent capture)",
        median_fps: 0.0, // not measured: requires sorted frame_times
        frame_time_stability: "not measured (silent capture)",
        jitter_std: 0.0,
        active_frame_ratio: if total_frames > 0 {
            (dirty_all_frames as f64 / total_frames as f64) * 100.0
        } else {
            100.0
        },
        avg_dirty_cells_per_frame,
        max_dirty_cells,
        avg_dirty_cell_ratio_percent: if total_cells > 0 {
            avg_dirty_cells_per_frame / total_cells as f64 * 100.0
        } else {
            0.0
        },
        dirty_all_frames,
        dirty_threshold,
        logical_cells_per_frame: total_cells as u64,
        render_ns_per_cell: if avg_dirty_cells_per_frame > 0.0 {
            avg_render_ms * 1_000_000.0 / avg_dirty_cells_per_frame
        } else {
            0.0
        },
        io_ns_per_cell: if avg_dirty_cells_per_frame > 0.0 {
            avg_io_ms * 1_000_000.0 / avg_dirty_cells_per_frame
        } else {
            0.0
        },
        // v50 LTS audit fix (Issue 1 residue): same fix as the visible
        // capture path — use `avg_frame_time` (full per-frame wall-clock
        // cost including bookkeeping) instead of `(sim+render+io)`.
        // See the visible-path comment above for full explanation.
        total_ns_per_cell: if avg_dirty_cells_per_frame > 0.0 {
            (avg_frame_time * 1_000_000.0) / avg_dirty_cells_per_frame
        } else {
            0.0
        },
        terminal_io,
        energy: Some(energy_metrics),
        perf: Some(perf_metrics),
        allocator: Some(alloc_metrics),
        visual: Some(visual_metrics),
        glyphs_per_second_theoretical: 0,
        dirty_glyphs_per_second: 0,
        ansi_bytes_per_second: 0,
        active_streams_avg: 0,
        total_drawn_cells,
        elapsed_s,
        total_frames,
        drawn_frames,
        peak_rss_kb: None,
        avg_rss_kb: None,
        rss_samples: 0,
        rss_supported: false,
        avg_cpu_percent: None,
        peak_cpu_percent: None,
        cpu_samples: 0,
        cpu_supported: false,
        rusage_delta: None,
        env: crate::envstat::EnvSnapshot::collect(),
        avg_sim_ms,
        avg_render_ms,
        avg_io_ms,
        max_sim_ms: 0.0,
        max_render_ms: 0.0,
        max_io_ms: 0.0,
        first_half_fps: None,
        second_half_fps: None,
        fps_drift_percent: None,
        bench_duration_secs,
    };

    Ok(report_data)
}
