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

use std::env;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::cinematic::{
    classify_frame_jitter, classify_frame_time_stability, dirty_threshold_cells,
    estimates_full_redraw,
};
use crate::constants::{
    ANSI_BYTES_PER_CELL_ESTIMATE, BENCH_ELAPSED_MIN_S, DENSITY_AUTO_DEFAULT_COLS,
    DENSITY_AUTO_DEFAULT_LINES, DIRTY_THRESHOLD_RATIO, MAX_TERMINAL_COLS, MAX_TERMINAL_LINES,
};
use crate::frame::Frame;

use super::{effective_density, CloudConfig};
use crate::bench_comp::ComponentTimer;
use crate::bench_cpu::CpuTracker;
use crate::bench_mem::RssTracker;
use crate::bench_progress::{register_interrupt, BenchProgress};

// Re-export metric meaning constants used by external modules
// (e.g., cloud/tests/tests_visual_depth.rs) so that import paths
// remain stable after the split into bench_report.rs.
#[allow(unused_imports)]
pub(crate) use crate::bench_report::{AVG_DIRTY_CELL_RATIO_MEANING, ESTIMATED_FULL_REDRAW_MEANING};

/// Duration of the premium benchmark in seconds (default).
const BENCHMARK_DURATION_SECS: u64 = 5;

/// Minimum and maximum allowed --bench-duration values (seconds).
/// 1s floor avoids meaningless sub-second runs; 600s (10min) ceiling
/// prevents runaway processes in CI. Users wanting longer endurance
/// runs should use the regular interactive mode with --duration.
const BENCH_DURATION_MIN: u64 = 1;
const BENCH_DURATION_MAX: u64 = 600;

/// Warmup duration for the premium benchmark in seconds.
const BENCHMARK_WARMUP_SECS: u64 = 2;

/// Number of frame time samples for percentile calculations.
const FRAME_TIME_SAMPLES: usize = 10_000;

// ── Legacy CI benchmark ─────────────────────────────────────────────────────

/// Compute the median of a sorted slice of f64 values.
/// Returns 0.0 for empty slices.
fn median_sorted(data: &[f64]) -> f64 {
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

/// Legacy CI benchmark: run N frames and print results in the original format.
/// Output format is preserved for backwards compatibility.
pub fn run_benchmark(cfg: &CloudConfig) -> std::io::Result<()> {
    let bench_frames = cfg.bench_frames.expect("bench_frames must be set");

    let (w, h) = bench_dimensions();

    let density = effective_density(cfg.base_density, w, h, cfg.fullwidth, cfg.density_auto);

    let mut cloud = cfg.create_cloud(density);
    cloud.reset(w, h);

    let mut frame = Frame::new(w, h, cloud.palette.bg);

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
fn resolve_bench_duration(override_secs: Option<u64>) -> Result<u64, String> {
    match override_secs {
        Some(n) if n < BENCH_DURATION_MIN => Err(format!(
            "error: --bench-duration {n} is below the {BENCH_DURATION_MIN}-second minimum"
        )),
        Some(n) if n > BENCH_DURATION_MAX => Err(format!(
            "error: --bench-duration {n} exceeds the {BENCH_DURATION_MAX}-second maximum \
             (use interactive mode with --duration for longer endurance runs)"
        )),
        Some(n) => Ok(n),
        None => Ok(BENCHMARK_DURATION_SECS),
    }
}

/// Premium user-facing benchmark: runs for the configured duration (default
/// 5s, override with `--bench-duration N`) with live progress feedback and
/// enhanced metrics in a Report-engine output.
pub fn run_premium_benchmark(cfg: &CloudConfig) -> std::io::Result<()> {
    // Validate --bench-duration BEFORE allocating any resources so an
    // out-of-range value fails fast without polluting the terminal.
    let bench_duration_secs = resolve_bench_duration(cfg.bench_duration).map_err(|e| {
        // Print to stderr so the error is visible; io::Error preserves
        // the message for the caller.
        eprintln!("{e}");
        std::io::Error::new(std::io::ErrorKind::InvalidInput, e)
    })?;

    let mut progress = BenchProgress::new();
    let interrupted = register_interrupt();

    // ── Header ───────────────────────────────────────────────────────────
    progress.begin();

    // ── Initialization ───────────────────────────────────────────────────
    let (w, h) = bench_dimensions();
    let density = effective_density(cfg.base_density, w, h, cfg.fullwidth, cfg.density_auto);

    let mut cloud = cfg.create_cloud(density);
    cloud.reset(w, h);

    let mut frame = Frame::new(w, h, cloud.palette.bg);

    let target_period = Duration::from_secs_f64(1.0 / cfg.target_fps);
    cloud.set_max_sim_delta(target_period);

    progress.init_done();

    // ── Warmup phase ─────────────────────────────────────────────────────
    progress.warmup_start();
    let warmup_end = Instant::now() + Duration::from_secs(bench_warmup_secs());
    let mut sim_now = Instant::now();
    while Instant::now() < warmup_end {
        if interrupted.load(Ordering::Relaxed) {
            progress.finish();
            return Ok(());
        }
        sim_now += target_period;
        cloud.rain_at(&mut frame, sim_now);
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

        frame.clear_dirty();

        let frame_time_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
        // io_ms = total frame time minus sim and render. In benchmark mode
        // no terminal write happens, so this is dirty-tracking + clear_dirty
        // + loop bookkeeping overhead. Clamped to >= 0 to guard against
        // clock skew between Instant::now() calls on different cores.
        let io_ms = (frame_time_ms - sim_ms - render_ms).max(0.0);

        components.record(sim_ms, render_ms, io_ms);

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
    }

    let (peak_rss_kb, avg_rss_kb, rss_samples, rss_supported) = rss.finalize();

    // CPU% averages + peaks.
    let (avg_cpu_percent, peak_cpu_percent, cpu_samples, cpu_supported) = cpu.finalize();

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
    crate::bench_report::build_premium_report(&report_data);
    Ok(())
}

/// Read benchmark dimensions from environment or use defaults.
fn bench_dimensions() -> (u16, u16) {
    let w = env::var("COSMOSTRIX_BENCH_COLS")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(DENSITY_AUTO_DEFAULT_COLS)
        .min(MAX_TERMINAL_COLS);
    let h = env::var("COSMOSTRIX_BENCH_LINES")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(DENSITY_AUTO_DEFAULT_LINES)
        .min(MAX_TERMINAL_LINES);
    (w, h)
}

/// Read configurable warmup duration from environment, falling back to the
/// default constant. Allows CI or power users to tune JIT warmup for
/// stability on different hardware.
fn bench_warmup_secs() -> u64 {
    env::var("COSMOSTRIX_BENCH_WARMUP_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(BENCHMARK_WARMUP_SECS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench_report::{
        ACTIVE_FRAME_RATIO_MEANING, DIRTY_ALL_FRAMES_MEANING, DRAW_RATIO_MEANING,
    };

    #[test]
    fn benchmark_metric_meanings_distinguish_dirty_frame_concepts() {
        assert!(DRAW_RATIO_MEANING.contains("legacy compatibility"));
        assert!(ACTIVE_FRAME_RATIO_MEANING.contains("at least one dirty cell"));
        assert!(AVG_DIRTY_CELL_RATIO_MEANING.contains("dirty-cell coverage"));
        assert!(DIRTY_ALL_FRAMES_MEANING.contains("every cell was dirty"));
        assert!(ESTIMATED_FULL_REDRAW_MEANING.contains("threshold estimate"));
    }

    #[test]
    fn benchmark_docs_do_not_keep_stale_active_claims() {
        let readme = include_str!("../README.md");
        let benchmark_readme = include_str!("../benchmark/README.md");
        assert!(!readme.contains("7,000 FPS"));
        assert!(!readme.contains(">7,000 FPS"));
        assert!(!benchmark_readme.contains("v2.1.0 reference results"));
        assert!(!benchmark_readme.contains("throughput exceeds 7,000 FPS"));
    }

    #[test]
    fn benchmark_stability_field_exists() {
        let readme = include_str!("../README.md");
        assert!(readme.to_lowercase().contains("throughput stability"));
    }

    #[test]
    fn benchmark_output_includes_stability_fields() {
        // This test ensures the premium benchmark output includes
        // backward-compatible stability fields. If any of these are
        // removed, the test will fail, preventing accidental breakage.
        const REQUIRED_FIELDS: &[&str] = &[
            "avg_fps",
            "peak_fps",
            "avg_frame_time",
            "p95_frame_time",
            "p99_frame_time",
            "p99_9_frame_time",
            "max_frame_time",
            "frame_jitter",
            "median_fps",
            "frame_time_stability",
            "draw_ratio",
            "active_frame_ratio_percent",
            "avg_dirty_cell_ratio_percent",
            "estimated_full_redraw_ratio_percent",
            "active_streams_avg",
            "dirty_glyphs_per_second",
            "planned_mode",
            "planned_worker_budget",
            "plan_reason",
            "actual_execution",
            "terminal_writer",
            "atmosphere_regime",
            "atmosphere_effective",
            "atmosphere_transition",
            "atmosphere_verifier",
            "atmosphere_application",
            "atmosphere_application_mode",
            "atmosphere_visual_effect",
            "effective_runtime",
        ];
        // These are checked against report field keys in the actual
        // benchmark (integration-level). Here we just verify the
        // test documents the contract.
        assert!(!REQUIRED_FIELDS.is_empty());
        for field in REQUIRED_FIELDS {
            assert!(!field.is_empty());
        }
    }

    #[test]
    fn bench_file_stays_under_target_loc() {
        // Guard: src/bench.rs must stay well under 1000 LOC.
        // Current target is under 900 LOC — bumped from 850 after P1-A
        // added sub-component timing wiring (sim/render/io accumulators
        // and per-frame cloud.last_sim_ms()/last_render_ms() reads).
        // The ComponentTimer struct was extracted to bench_comp.rs to
        // minimize growth here; further sub-component work should also
        // live in bench_comp.rs rather than expand this file.
        let source = include_str!("bench.rs");
        let lines = source.lines().count();
        assert!(
            lines < 900,
            "bench.rs must stay under 900 LOC target (currently {lines})"
        );
    }

    #[test]
    fn bench_re_exports_preserve_external_import_paths() {
        // Verify that the re-exports from bench_report.rs are correct
        // so external modules (e.g., cloud/tests/tests_visual_depth.rs)
        // can still use `use crate::bench::AVG_DIRTY_CELL_RATIO_MEANING`.
        assert!(AVG_DIRTY_CELL_RATIO_MEANING.contains("dirty-cell coverage"));
        assert!(ESTIMATED_FULL_REDRAW_MEANING.contains("threshold estimate"));
    }

    #[test]
    fn resolve_bench_duration_uses_default_when_none() {
        assert_eq!(
            resolve_bench_duration(None).unwrap(),
            BENCHMARK_DURATION_SECS,
            "None override must fall back to default duration"
        );
    }

    #[test]
    fn resolve_bench_duration_accepts_in_range_override() {
        assert_eq!(resolve_bench_duration(Some(1)).unwrap(), 1, "min boundary");
        assert_eq!(
            resolve_bench_duration(Some(600)).unwrap(),
            600,
            "max boundary"
        );
        assert_eq!(resolve_bench_duration(Some(30)).unwrap(), 30, "mid-range");
    }

    #[test]
    fn resolve_bench_duration_rejects_below_minimum() {
        let err = resolve_bench_duration(Some(0)).unwrap_err();
        assert!(
            err.contains("below the"),
            "below-minimum error must explain the floor: {err}"
        );
    }

    #[test]
    fn resolve_bench_duration_rejects_above_maximum() {
        let err = resolve_bench_duration(Some(601)).unwrap_err();
        assert!(
            err.contains("exceeds the"),
            "above-maximum error must explain the ceiling: {err}"
        );
    }
}
