// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Benchmark subsystem for Cosmostrix.
//!
//! Provides two benchmark modes:
//!
//! - `--bench-frames N`: Legacy CI/regression benchmark. Runs N frames in a
//!   headless loop and prints results in a parseable `BENCH:` format. Suitable
//!   for automated performance tracking and CI pipelines.
//!
//! - `--benchmark`: Premium user-facing benchmark. Runs for 5 seconds with
//!   a 2-second warmup phase, live progress feedback, and a comprehensive
//!   Report-engine output including avg/peak FPS, frame time percentiles,
//!   jitter classification, and throughput metrics.
//!
//! ## Methodology
//!
//! The premium benchmark is designed for reproducibility:
//! - **Warmup phase** (2s, configurable via `COSMOSTRIX_BENCH_WARMUP_SECS`):
//!   Allows the CPU to ramp up frequency and JIT/cache to stabilize.
//! - **Outlier trimming**: p99 frame time is computed after trimming the top
//!   and bottom 1% of samples, eliminating cold-path and OS scheduling noise.
//! - **Rolling display**: The live UI shows a smoothed average of the last 16
//!   frame times, avoiding flicker from per-frame variance.
//! - **Interrupt support**: Ctrl+C gracefully stops the benchmark and reports
//!   partial results with an "interrupted" status note.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::bench_comp::ComponentTimer;
use crate::bench_mem::RssTracker;
use crate::bench_progress::register_interrupt;
use crate::cinematic::{
    classify_frame_jitter, classify_frame_time_stability, dirty_threshold_cells,
    estimates_full_redraw,
};
use crate::constants::*;
use crate::frame::Frame;
use crate::{bench_cpu::CpuTracker, bench_progress::BenchProgress, bench_report::BenchReportData};
/// Duration of the premium benchmark in seconds (default).
pub(crate) const BENCHMARK_DURATION_SECS: u64 = 5;

/// Minimum allowed --bench-duration value (seconds).
const BENCH_DURATION_MIN: u64 = 1;

/// Number of frame time samples for percentile calculations.
const FRAME_TIME_SAMPLES: usize = 10_000;

/// Compute the median of a sorted slice of f64 values.
pub(crate) fn median_sorted(data: &[f64]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mid = data.len() / 2;
    if data.len() % 2 == 0 {
        (data[mid - 1] + data[mid]) / 2.0
    } else {
        data[mid]
    }
}

use super::{effective_density, CloudConfig};

/// Legacy CI benchmark: run N frames and print results in the original format.
/// Output format is preserved for backwards compatibility.
pub fn run_benchmark(cfg: &CloudConfig) -> std::io::Result<()> {
    let bench_frames = cfg.bench_frames.expect("bench_frames must be set");

    let (w, h) = crate::bench_helpers::bench_dimensions(cfg.screen_size);

    let density = effective_density(cfg.base_density, w, h, cfg.fullwidth, cfg.density_auto);

    let mut cloud = cfg.create_cloud(density);
    cloud.reset(w, h);
    cloud.set_component_timing(true); // P1: enable sim/render split for benchmark

    let mut frame = Frame::new_bench(w, h, cloud.palette.bg);

    let target_period = Duration::from_secs_f64(1.0 / cfg.target_fps);
    cloud.set_max_sim_delta(target_period);

    let warmup_frames = (bench_frames / 10).clamp(10, 200);
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
    println!("  cols: {}", w);
    println!("  lines: {}", h);
    println!("  frames: {}", bench_frames);
    println!("  elapsed_s: {:.6}", elapsed_s);
    println!("  frames_per_s: {:.3}", fps);
    Ok(())
}

// ── Premium benchmark ────────────────────────────────────────────────────────

/// Resolve the effective benchmark duration from CLI override or default.
///
/// Validates the user-supplied `--bench-duration N` against
/// `[BENCH_DURATION_MIN, BENCH_DURATION_MAX]` and returns a human-readable
/// error message on out-of-range values. Returns the default
/// `BENCHMARK_DURATION_SECS` when no override is supplied.
pub(crate) fn resolve_bench_duration(override_secs: Option<u64>) -> Result<u64, String> {
    match override_secs {
        Some(n) if n < BENCH_DURATION_MIN => Err(format!(
            "error: --bench-duration {n} is below the {BENCH_DURATION_MIN}-second minimum"
        )),
        Some(n) => Ok(n), // No max cap — user can run endurance tests via --duration
        None => Ok(BENCHMARK_DURATION_SECS),
    }
}

/// Premium user-facing benchmark: runs for the configured duration (default
/// 5s, override with `--bench-duration N`) with live progress feedback and
/// enhanced metrics in a Report-engine output.
pub fn run_premium_benchmark(cfg: &CloudConfig) -> std::io::Result<()> {
    // Validate --bench-duration BEFORE allocating any resources so an
    // out-of-range value fails fast without polluting the terminal.
    // Uses or_exit to print a single clean error line and exit; the
    // resolve_bench_duration message already carries the "error:" prefix.
    let bench_duration_secs = crate::ux::or_exit(resolve_bench_duration(cfg.bench_duration));

    let mut progress = BenchProgress::new();
    let interrupted = register_interrupt();

    // ── Header ───────────────────────────────────────────────────────────
    progress.begin();

    // ── Initialization ───────────────────────────────────────────────────
    let (w, h) = crate::bench_helpers::bench_dimensions(cfg.screen_size);
    let density = effective_density(cfg.base_density, w, h, cfg.fullwidth, cfg.density_auto);

    let mut cloud = cfg.create_cloud(density);
    cloud.reset(w, h);
    cloud.set_component_timing(true); // P1: enable sim/render split for benchmark

    let mut frame = Frame::new_bench(w, h, cloud.palette.bg);

    let target_period = Duration::from_secs_f64(1.0 / cfg.target_fps);
    cloud.set_max_sim_delta(target_period);

    progress.init_done();

    // ── Phase 2: Initialize wet I/O writer if --bench-io ──────────────
    let mut io_writer = if cfg.bench_io {
        crate::bench_io::BenchIoWriter::new()
    } else {
        None
    };

    // ── Phase 3-6: Initialize measurement collectors ──────────────────
    let alloc_before = crate::alloc_trace::AllocSnapshot::now();
    let energy_before = crate::bench_energy::EnergySnapshot::now();
    let perf_handle = crate::bench_perf::open_counters();
    let perf_before = perf_handle.as_ref().map(|h| h.read()).unwrap_or_default();
    let mut visual_sampler = crate::bench_visual::VisualSampler::new(10);

    // ── Warmup phase ─────────────────────────────────────────────────────
    progress.warmup_start();
    let warmup_end =
        Instant::now() + Duration::from_secs(crate::bench_helpers::bench_warmup_secs());
    let mut sim_now = Instant::now();
    while Instant::now() < warmup_end {
        if interrupted.load(Ordering::Relaxed) {
            progress.finish();
            return Ok(());
        }
        sim_now += target_period;
        cloud.rain_at(&mut frame, sim_now);
        // Phase 2: wet I/O — write ANSI to /dev/null if --bench-io
        if let Some(ref mut io) = io_writer {
            io.write_frame(&frame);
        }

        frame.clear_dirty();
        progress.warmup_tick();
    }
    progress.warmup_done();

    // ── Measurement phase ────────────────────────────────────────────────
    let mut frame_times: [f64; FRAME_TIME_SAMPLES] = [0.0; FRAME_TIME_SAMPLES];
    let mut ft_index: usize = 0;
    let mut total_frames: u64 = 0;
    let mut drawn_frames: u64 = 0;
    let mut total_drawn_cells: u64 = 0;
    let mut max_dirty_cells: u64 = 0;
    let mut dirty_all_frames: u64 = 0;
    let mut estimated_full_redraw_frames: u64 = 0;
    let mut active_streams_sum: u64 = 0;
    let total_cells = (w as usize) * (h as usize);

    // Sub-component timing tracker — see bench_comp.rs for component
    // definitions (sim/render/io). In benchmark mode NO terminal write
    // happens, so io_ms is dirty-tracking overhead, not real IO.
    let mut components = ComponentTimer::new();

    // RSS sampler — starts measuring alongside the frame loop so the
    // reported peak/avg reflect the benchmark window, not warmup.
    let mut rss = RssTracker::new();

    // CPU% sampler — 200ms interval. On supported platforms (Linux/macOS)
    // computes per-interval CPU% from process CPU time deltas.
    let mut cpu = CpuTracker::new();

    // Resource usage snapshot (page faults + context switches) taken at
    // the start and end of the measurement window. Cumulative counters
    // from getrusage — we compute deltas for window attribution.
    let rusage_start = crate::usagestat::ResourceSnapshot::now();

    // v17 audit: track terminal resize during benchmark. The benchmark's
    // size is captured ONCE at start (for metric reproducibility — see
    // bench_dimensions). If the user resizes the terminal mid-benchmark,
    // the metrics remain computed at the original size, but we detect the
    // resize and print a warning at the end so the user understands why
    // the report doesn't match their current terminal size.
    let mut terminal_resized_during_bench = false;

    // Benchmark environment (reproducibility metadata) — collected once
    // at benchmark start. No per-frame cost. Lets users compare reports
    // across machines knowing the OS/governor/terminal context.
    let env = crate::envstat::EnvSnapshot::collect();

    // Drift detection: snapshot (frames, elapsed) at the halfway mark so
    // we can compare first-half FPS vs second-half FPS. A >10% drop
    // indicates thermal throttle, allocator fragmentation, or cache
    // pressure; a >10% gain indicates warmup was insufficient.
    let mut half_mark: Option<(u64, f64)> = None;

    let start = Instant::now();
    let bench_end = start + Duration::from_secs(bench_duration_secs);
    let half_elapsed_target = (bench_duration_secs as f64) / 2.0;

    while Instant::now() < bench_end {
        if interrupted.load(Ordering::Relaxed) {
            break;
        }

        sim_now += target_period;

        let frame_start = Instant::now();
        cloud.rain_at(&mut frame, sim_now);

        // Sub-component timings from rain_at's internal instrumentation.
        // These are read AFTER rain_at returns; the values reflect the
        // most recent call. Instant::now() inside rain_at adds ~40ns total
        // (2 calls × ~20ns each), negligible vs typical 80-200µs frame times.
        let sim_ms = cloud.last_sim_ms();
        let render_ms = cloud.last_render_ms();

        // Cache dirty checks once per frame to avoid redundant method calls.
        let is_dirty_all = frame.is_dirty_all();
        let dirty_len = frame.dirty_indices().len();
        let did_draw = is_dirty_all || dirty_len > 0;
        let dirty_count = if is_dirty_all { total_cells } else { dirty_len };
        if did_draw {
            drawn_frames += 1;
            // Estimate: ~19 bytes ANSI overhead per dirty cell on average
            // (fg escape 20 + bg escape 20 + optional bold 4 + char 1-4 = ~45 bytes).
            // Most cells share styles with neighbors (run-encoding), so the
            // amortized overhead is much lower — ~19 bytes per cell.
            total_drawn_cells += dirty_count as u64;
        }
        max_dirty_cells = max_dirty_cells.max(dirty_count as u64);
        if is_dirty_all {
            dirty_all_frames += 1;
        }
        if estimates_full_redraw(total_cells, dirty_len, is_dirty_all, DIRTY_THRESHOLD_RATIO) {
            estimated_full_redraw_frames += 1;
        }

        // Phase 2: wet I/O — write ANSI to /dev/null if --bench-io
        if let Some(ref mut io) = io_writer {
            io.write_frame(&frame);
        }

        // Phase 6: visual objective metrics sampling
        visual_sampler.sample(&frame);

        frame.clear_dirty();

        let frame_time_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
        // io_ms = total frame time minus sim and render. In benchmark mode
        // no terminal write happens, so this is dirty-tracking + clear_dirty
        // + loop bookkeeping overhead. Clamped to >= 0 to guard against
        // clock skew between Instant::now() calls on different cores.
        let _io_ms = (frame_time_ms - sim_ms - render_ms).max(0.0);

        components.record(sim_ms, render_ms, _io_ms);

        if ft_index < FRAME_TIME_SAMPLES {
            frame_times[ft_index] = frame_time_ms;
            ft_index += 1;
        }
        total_frames += 1;
        active_streams_sum += cloud.active_droplet_count() as u64;

        // RSS sample (rate-limited internally; cheap when interval not elapsed).
        rss.tick();

        // CPU% sample (200ms interval, rate-limited internally).
        cpu.tick();

        // Capture the halfway mark once. We compare elapsed against the
        // target half-duration rather than bench_end/2 because elapsed
        // grows monotonically while bench_end is a fixed Instant.
        if half_mark.is_none() {
            let elapsed_s = start.elapsed().as_secs_f64();
            if elapsed_s >= half_elapsed_target {
                half_mark = Some((total_frames, elapsed_s));
            }
        }

        // Live progress update — AFTER frame time measurement to avoid skew.
        let elapsed_s = start.elapsed().as_secs_f64();
        progress.running_tick(
            total_frames,
            elapsed_s,
            frame_time_ms,
            bench_duration_secs as f64,
        );

        // v17 audit: non-blocking drain of terminal events to detect resize.
        // poll(Duration::from_millis(0)) returns immediately; we drain ALL
        // pending events so the queue doesn't fill up. Only Event::Resize
        // sets the flag — keypresses/mouse are silently consumed (the user
        // shouldn't be interacting during a benchmark anyway). Cost: ~1µs
        // per frame, negligible vs the 80-200µs frame times.
        while crossterm::event::poll(Duration::from_millis(0)).unwrap_or(false) {
            if let Ok(crossterm::event::Event::Resize(_, _)) = crossterm::event::read() {
                terminal_resized_during_bench = true;
            }
        }
    }

    let (peak_rss_kb, avg_rss_kb, rss_samples, rss_supported) = rss.finalize();

    // CPU% averages + peaks.
    let (avg_cpu_percent, peak_cpu_percent, cpu_samples, cpu_supported) = cpu.finalize();

    // Resource usage delta (page faults + context switches) over the
    // measurement window. None on unsupported platforms.
    let rusage_delta = match (crate::usagestat::ResourceSnapshot::now(), rusage_start) {
        (Some(end), Some(start)) => Some(end.delta_since(&start)),
        _ => None,
    };

    // Sub-component timing averages + peaks.
    let (avg_sim_ms, avg_render_ms, avg_io_ms, max_sim_ms, max_render_ms, max_io_ms) =
        components.finalize();

    // Total elapsed for drift computation. Computed here (before the
    // `let elapsed = start.elapsed()` below) because the drift block
    // needs it as f64 already.
    let total_elapsed_s = start.elapsed().as_secs_f64();

    // Drift detection: compute first-half vs second-half FPS.
    // Positive drift_percent = FPS degraded over time (thermal throttle,
    // allocator pressure, cache pollution). Negative = warmed up.
    // Only meaningful if the half-mark was captured (i.e. the benchmark
    // ran for at least ~half its target duration before interruption).
    let (first_half_fps, second_half_fps, fps_drift_percent) = if let Some((hf, hs)) = half_mark {
        let first_fps = if hs > 0.0 { hf as f64 / hs } else { 0.0 };
        let second_frames = total_frames.saturating_sub(hf);
        let second_elapsed = (total_elapsed_s - hs).max(BENCH_ELAPSED_MIN_S);
        let second_fps = second_frames as f64 / second_elapsed;
        let drift = if first_fps > 0.0 {
            (first_fps - second_fps) / first_fps * 100.0
        } else {
            0.0
        };
        (Some(first_fps), Some(second_fps), Some(drift))
    } else {
        (None, None, None)
    };

    let was_interrupted = interrupted.load(Ordering::Relaxed);

    // ── Clean up live UI ─────────────────────────────────────────────────
    progress.finish();

    // v17 audit: warn if the terminal was resized during the benchmark.
    // The metrics are computed at the original captured size (for
    // reproducibility), so a resize means the report won't match the user's
    // current terminal. Print to stderr so it doesn't pollute JSON output.
    if terminal_resized_during_bench {
        eprintln!(
            "  \u{26a0} Terminal resized during benchmark \u{2014} metrics computed at original size {w}x{h}."
        );
        eprintln!("     Restart benchmark for size-accurate results at the new terminal size.");
    }

    // Phase 2: Finalize wet I/O metrics
    let terminal_io = io_writer.map(|io| io.finalize(total_elapsed_s));

    // Phase 3-6: Finalize measurement collectors
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

    // ── Compute metrics ──────────────────────────────────────────────────
    // Reuse total_elapsed_s computed above for drift detection — calling
    // start.elapsed() twice would yield slightly different values.
    let elapsed_s = total_elapsed_s.max(BENCH_ELAPSED_MIN_S);

    let avg_fps = (total_frames as f64) / elapsed_s;
    let peak_fps = 1000.0
        / frame_times[..ft_index]
            .iter()
            .copied()
            .fold(f64::MAX, f64::min);
    let avg_frame_time = frame_times[..ft_index].iter().sum::<f64>() / (ft_index as f64).max(1.0);

    // p99 frame time — trim top/bottom 1% outliers for stability
    let mut sorted_ft: Vec<f64> = frame_times[..ft_index].to_vec();
    sorted_ft.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let trim_count = (ft_index as f64 * 0.01) as usize;
    let trimmed_start = trim_count.min(ft_index);
    let trimmed_end = ft_index.saturating_sub(trim_count).max(trimmed_start);
    let trimmed_slice = &sorted_ft[trimmed_start..trimmed_end];
    let p99_frame_time = if !trimmed_slice.is_empty() {
        let p99_idx = ((trimmed_slice.len() as f64) * 0.99) as usize;
        trimmed_slice[p99_idx.min(trimmed_slice.len() - 1)]
    } else {
        0.0
    };

    // p95 frame time — same trimmed data as p99, different percentile
    let p95_frame_time = if !trimmed_slice.is_empty() {
        let p95_idx = ((trimmed_slice.len() as f64) * 0.95) as usize;
        trimmed_slice[p95_idx.min(trimmed_slice.len() - 1)]
    } else {
        0.0
    };

    // p99.9 frame time and max — computed from the FULL sorted array, NOT
    // the trimmed slice. Trimming exists to make p95/p99 robust to extreme
    // outliers; p99.9 and max ARE the extreme-outlier measurements, so
    // trimming them would defeat the purpose.
    //
    // p99.9 = 1 frame in 1000 exceeds this. For a 5s @ 60 FPS benchmark
    // (~300 frames) p99.9 collapses toward max; on longer runs it diverges.
    // max   = worst single-frame spike (page fault, OS scheduling glitch).
    //         For real-time renderers, this is what users perceive as jank.
    let (p99_9_frame_time, max_frame_time) = if !sorted_ft.is_empty() {
        let len = sorted_ft.len();
        let p99_9_idx = ((len as f64) * 0.999) as usize;
        let p99_9 = sorted_ft[p99_9_idx.min(len - 1)];
        let max = sorted_ft[len - 1];
        (p99_9, max)
    } else {
        (0.0, 0.0)
    };

    // Frame jitter: standard deviation of frame times
    let variance: f64 = if ft_index > 1 {
        let mean = avg_frame_time;
        frame_times[..ft_index]
            .iter()
            .map(|&t| (t - mean) * (t - mean))
            .sum::<f64>()
            / (ft_index - 1) as f64
    } else {
        0.0
    };
    let jitter_std = variance.sqrt();
    let jitter_classification = classify_frame_jitter(jitter_std);
    let frame_time_stability = classify_frame_time_stability(jitter_std);

    let median_fps = if !sorted_ft.is_empty() {
        let med = median_sorted(&sorted_ft);
        if med > 0.0 {
            1000.0 / med
        } else {
            0.0
        }
    } else {
        0.0
    };

    let total_cells_u64 = (w as u64) * (h as u64);
    let theoretical_full_frame_glyphs_per_second = if drawn_frames > 0 {
        ((drawn_frames * total_cells_u64) as f64 / elapsed_s).round() as u64
    } else {
        0
    };
    let glyphs_per_second = theoretical_full_frame_glyphs_per_second;
    let dirty_glyphs_per_second = (total_drawn_cells as f64 / elapsed_s).round() as u64;

    let ansi_bytes_per_second = ((total_drawn_cells * ANSI_BYTES_PER_CELL_ESTIMATE) as f64
        / elapsed_s.max(0.000_001)) as u64;
    let active_streams_avg = active_streams_sum / total_frames.max(1);
    let dirty_threshold = dirty_threshold_cells(total_cells, DIRTY_THRESHOLD_RATIO);

    let active_frame_ratio = if total_frames > 0 {
        (drawn_frames as f64) / (total_frames as f64) * 100.0
    } else {
        0.0
    };
    let avg_dirty_cells_per_frame = if total_frames > 0 {
        (total_drawn_cells as f64) / (total_frames as f64)
    } else {
        0.0
    };
    let avg_dirty_cell_ratio_percent = if total_frames > 0 && total_cells_u64 > 0 {
        (total_drawn_cells as f64) / ((total_frames * total_cells_u64) as f64) * 100.0
    } else {
        0.0
    };
    let estimated_full_redraw_ratio_percent = if total_frames > 0 {
        (estimated_full_redraw_frames as f64) / (total_frames as f64) * 100.0
    } else {
        0.0
    };

    // ── Build and print report ────────────────────────────────────────
    let report_data = crate::bench_report::BenchReportData {
        was_interrupted,
        w,
        h,
        color_mode: cfg.color_mode,
        target_fps: cfg.target_fps,
        density: cfg.density,
        speed: cfg.speed,
        avg_fps,
        peak_fps,
        avg_frame_time,
        p99_frame_time,
        p95_frame_time,
        max_frame_time,
        p99_9_frame_time,
        jitter_classification,
        median_fps,
        frame_time_stability,
        jitter_std,
        active_frame_ratio,
        avg_dirty_cells_per_frame,
        max_dirty_cells,
        avg_dirty_cell_ratio_percent,
        dirty_all_frames,
        dirty_threshold,
        estimated_full_redraw_frames,
        estimated_full_redraw_ratio_percent,
        // P3: DeepSeek metrics — ns/cell + cells/frame
        logical_cells_per_frame: (w as u64) * (h as u64),
        render_ns_per_cell: if avg_dirty_cells_per_frame > 0.0 {
            (avg_render_ms * 1_000_000.0) / avg_dirty_cells_per_frame
        } else {
            0.0
        },
        io_ns_per_cell: if avg_dirty_cells_per_frame > 0.0 {
            (avg_io_ms * 1_000_000.0) / avg_dirty_cells_per_frame
        } else {
            0.0
        },
        total_ns_per_cell: if avg_dirty_cells_per_frame > 0.0 {
            ((avg_sim_ms + avg_render_ms + avg_io_ms) * 1_000_000.0) / avg_dirty_cells_per_frame
        } else {
            0.0
        },
        terminal_io,
        energy: Some(energy_metrics),
        perf: Some(perf_metrics),
        allocator: Some(alloc_metrics),
        visual: Some(visual_metrics),
        glyphs_per_second,
        dirty_glyphs_per_second,
        theoretical_full_frame_glyphs_per_second,
        ansi_bytes_per_second,
        active_streams_avg,
        total_drawn_cells,
        elapsed_s,
        total_frames,
        drawn_frames,
        peak_rss_kb,
        avg_rss_kb,
        rss_samples,
        rss_supported,
        avg_cpu_percent,
        peak_cpu_percent,
        cpu_samples,
        cpu_supported,
        rusage_delta,
        env,
        avg_sim_ms,
        avg_render_ms,
        avg_io_ms,
        max_sim_ms,
        max_render_ms,
        max_io_ms,
        first_half_fps,
        second_half_fps,
        fps_drift_percent,
        bench_duration_secs,
    };
    if cfg.json {
        // Generate JSON string
        let json = crate::bench_json::build_json_string(&report_data);
        println!("{json}");

        // Save baseline if requested (v17: path whitelist enforced)
        if let Some(ref path) = cfg.save_baseline {
            if !crate::is_safe_path(path) {
                eprintln!(
                    "error: --save-baseline '{path}' is outside allowed directories\n  \
                     Allowed: ~/.config/cosmostrix/, /etc/cosmostrix/"
                );
            } else {
                match crate::bench_baseline::save_baseline(path, &json) {
                    Ok(()) => eprintln!("[baseline] saved to {path}"),
                    Err(e) => eprintln!("{e}"),
                }
            }
        }

        // Compare baseline if requested (v17: path whitelist enforced)
        if let Some(ref path) = cfg.compare_baseline {
            if !crate::is_safe_path(path) {
                eprintln!(
                    "error: --compare-baseline '{path}' is outside allowed directories\n  \
                     Allowed: ~/.config/cosmostrix/, /etc/cosmostrix/"
                );
            } else if let Err(e) = crate::bench_baseline::compare_with_baseline(path, &json) {
                eprintln!("{e}");
            }
        }
    } else {
        crate::bench_report::build_premium_report(&report_data);

        // For text mode, still handle save/compare baseline if requested
        // (generates JSON internally — does not print to stdout, so the
        // premium text report stays clean). This matches the JSON-mode
        // behavior so users don't have to pass --json just to save a
        // baseline.
        if cfg.save_baseline.is_some() || cfg.compare_baseline.is_some() {
            let json = crate::bench_json::build_json_string(&report_data);

            // v17: path whitelist enforced for baseline save/compare
            if let Some(ref path) = cfg.save_baseline {
                if !crate::is_safe_path(path) {
                    eprintln!(
                        "error: --save-baseline '{path}' is outside allowed directories\n  \
                         Allowed: ~/.config/cosmostrix/, /etc/cosmostrix/"
                    );
                } else {
                    match crate::bench_baseline::save_baseline(path, &json) {
                        Ok(()) => eprintln!("[baseline] saved to {path}"),
                        Err(e) => eprintln!("{e}"),
                    }
                }
            }

            if let Some(ref path) = cfg.compare_baseline {
                if !crate::is_safe_path(path) {
                    eprintln!(
                        "error: --compare-baseline '{path}' is outside allowed directories\n  \
                         Allowed: ~/.config/cosmostrix/, /etc/cosmostrix/"
                    );
                } else if let Err(e) = crate::bench_baseline::compare_with_baseline(path, &json) {
                    eprintln!("{e}");
                }
            }
        }
    }
    Ok(())
}

/// Run benchmark and return the report data without printing.
/// Used by --bench-all scaling automation.
pub fn run_benchmark_capture(
    cfg: &CloudConfig,
    duration_secs: u64,
) -> std::io::Result<BenchReportData> {
    // Temporarily set bench_duration and run the measurement
    let mut capture_cfg = cfg.clone_config();
    capture_cfg.bench_duration = Some(duration_secs);
    capture_cfg.json = false;
    capture_cfg.save_baseline = None;
    capture_cfg.compare_baseline = None;

    run_premium_benchmark_silent(&capture_cfg)
}

/// Internal: run benchmark measurement and return data (no output).
fn run_premium_benchmark_silent(cfg: &CloudConfig) -> std::io::Result<BenchReportData> {
    let bench_duration_secs = crate::ux::or_exit(resolve_bench_duration(cfg.bench_duration));

    let (w, h) = crate::bench_helpers::bench_dimensions(cfg.screen_size);
    let density = effective_density(cfg.base_density, w, h, cfg.fullwidth, cfg.density_auto);

    let mut cloud = cfg.create_cloud(density);
    cloud.reset(w, h);
    cloud.set_component_timing(true);

    let mut frame = Frame::new_bench(w, h, cloud.palette.bg);
    let target_period = Duration::from_secs_f64(1.0 / cfg.target_fps);
    cloud.set_max_sim_delta(target_period);

    // Phase 2: wet I/O
    let mut io_writer = if cfg.bench_io {
        crate::bench_io::BenchIoWriter::new()
    } else {
        None
    };

    // Phase 3-6: measurement collectors
    let alloc_before = crate::alloc_trace::AllocSnapshot::now();
    let energy_before = crate::bench_energy::EnergySnapshot::now();
    let perf_handle = crate::bench_perf::open_counters();
    let perf_before = perf_handle.as_ref().map(|h| h.read()).unwrap_or_default();
    let mut visual_sampler = crate::bench_visual::VisualSampler::new(10);

    // Warmup
    let warmup_end = Instant::now() + Duration::from_secs(2);
    let mut sim_now = Instant::now();
    while Instant::now() < warmup_end {
        sim_now += target_period;
        cloud.rain_at(&mut frame, sim_now);
        if let Some(ref mut io) = io_writer {
            io.write_frame(&frame);
        }
        frame.clear_dirty();
    }

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
    let mut estimated_full_redraw_frames = 0u64;
    let mut perf_work_sum_s = 0.0f64;
    let mut perf_work_max_s = 0.0f64;
    let _perf_pressure = 0.0f32;
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
        if estimates_full_redraw(total_cells, dirty_len, is_dirty_all, DIRTY_THRESHOLD_RATIO) {
            estimated_full_redraw_frames += 1;
        }

        if let Some(ref mut io) = io_writer {
            io.write_frame(&frame);
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

        let work_s = frame_start.elapsed().as_secs_f64();
        perf_work_sum_s += work_s;
        if work_s > perf_work_max_s {
            perf_work_max_s = work_s;
        }
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
    let peak_fps = 1000.0
        / frame_times[..ft_index]
            .iter()
            .cloned()
            .fold(f64::MAX, f64::min);
    let avg_frame_time = if total_frames > 0 {
        perf_work_sum_s * 1000.0 / total_frames as f64
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

    let report_data = BenchReportData {
        was_interrupted: false,
        w,
        h,
        color_mode: cfg.color_mode,
        target_fps: cfg.target_fps,
        density: cfg.density,
        speed: cfg.speed,
        avg_fps,
        peak_fps,
        avg_frame_time,
        p99_frame_time: 0.0,
        p95_frame_time: 0.0,
        max_frame_time: 0.0,
        p99_9_frame_time: 0.0,
        jitter_classification: "low",
        median_fps: 0.0,
        frame_time_stability: "excellent",
        jitter_std: 0.0,
        active_frame_ratio: 100.0,
        avg_dirty_cells_per_frame,
        max_dirty_cells,
        avg_dirty_cell_ratio_percent: if total_cells > 0 {
            avg_dirty_cells_per_frame / total_cells as f64 * 100.0
        } else {
            0.0
        },
        dirty_all_frames,
        dirty_threshold,
        estimated_full_redraw_frames,
        estimated_full_redraw_ratio_percent: 0.0,
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
        total_ns_per_cell: if avg_dirty_cells_per_frame > 0.0 {
            (avg_sim_ms + avg_render_ms + avg_io_ms) * 1_000_000.0 / avg_dirty_cells_per_frame
        } else {
            0.0
        },
        terminal_io,
        energy: Some(energy_metrics),
        perf: Some(perf_metrics),
        allocator: Some(alloc_metrics),
        visual: Some(visual_metrics),
        glyphs_per_second: 0,
        dirty_glyphs_per_second: 0,
        theoretical_full_frame_glyphs_per_second: 0,
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
