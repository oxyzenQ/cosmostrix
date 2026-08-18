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
};
use crate::constants::*;
use crate::frame::Frame;
use crate::theme::canonical_name_for_scheme;
use crate::{bench_cpu::CpuTracker, bench_progress::BenchProgress, bench_report::BenchReportData};

// Re-export bench validation helpers + consts from bench_helpers.rs.
// Pure functions moved out to keep this file under the LOC cap.
pub(crate) use crate::bench_helpers::{
    median_sorted, resolve_bench_duration, validate_bench_scene,
};
#[cfg(test)]
pub(crate) use crate::bench_helpers::{validate_bench_scene_str, VALID_BENCH_SCENES};

/// Number of frame time samples for percentile calculations.
const FRAME_TIME_SAMPLES: usize = 10_000;

use super::{effective_density, CloudConfig};

/// Minimum plausible frame time in milliseconds. Samples below this are
/// clock artifacts (TSC read returning the same value, kernel rounding
/// down, etc.) and must be discarded — a real frame requires at least
/// 2× `Instant::now()` calls (~40-100ns total) plus actual renderer work,
/// so anything below 1µs is physically impossible.
///
/// At this floor, the maximum representable `peak_fps` is 1,000,000 —
/// still well above any real renderer throughput, but eliminates absurd
/// values like 3,584,229 FPS that percentile trimming alone cannot
/// remove when outlier clusters exceed 1% of samples.
const PEAK_FPS_MIN_FRAME_MS: f64 = 0.001; // 1µs

/// Compute `peak_fps` from a sorted (ascending) slice of frame-time samples
/// in milliseconds.
///
/// Returns the FPS derived from the **fastest plausible** frame time —
/// the smallest sample that survives BOTH:
///   1. p1 percentile trim (skip fastest 1% to absorb sparse outliers), AND
///   2. absolute floor of `PEAK_FPS_MIN_FRAME_MS` (skip clock artifacts
///      that cluster densely enough to survive percentile trimming).
///
/// # Why both p1 AND a floor?
///
/// The original p1-only fix (commit 6b093f1) assumed outliers were sparse
/// (< 1% of samples). On FreeBSD's fast path this assumption fails: when
/// `Instant::elapsed()` returns sub-microsecond deltas for hundreds of
/// consecutive frames (TSC read hits the same cycle, or kernel clock
/// rounds down), the outlier cluster exceeds 1% and survives the p1 trim.
/// This produced absurd values like 3,584,229 FPS even AFTER the p1 fix.
///
/// The absolute floor catches what percentile trimming cannot: any sample
/// below 1µs is a clock artifact regardless of how many there are. This
/// is a physics-based filter, not a statistical one — it cannot over-trim
/// real frame times because no real frame completes in < 1µs.
///
/// # Zero-sample handling
///
/// Samples equal to `0.0` (clock resolution coarser than frame time) are
/// also skipped by the `> PEAK_FPS_MIN_FRAME_MS` filter. If ALL samples
/// are below the floor, we fall back to `0.0` (honest "not measurable"
/// answer for coarse-clock systems).
///
/// # Arguments
///
/// * `sorted_ft` - Frame times in ms, sorted ascending. Caller is
///   responsible for sorting (this fn does NOT re-sort, to avoid O(n log n)
///   on every call when the caller already has a sorted slice for p99/p95).
/// * `count` - Number of valid samples in `sorted_ft` (may be less than
///   `sorted_ft.len()` if the buffer is a fixed-size array partially filled).
///
/// # Returns
///
/// `peak_fps` in frames-per-second. `0.0` if no samples exceed the floor,
/// or if `count == 0`.
pub(crate) fn compute_peak_fps(sorted_ft: &[f64], count: usize) -> f64 {
    if count == 0 {
        return 0.0;
    }
    let p1_count = (count as f64 * 0.01) as usize;
    let p1_idx = p1_count.min(count.saturating_sub(1));
    // Scan p1+ range for the first sample above the absolute floor.
    // This skips BOTH zero samples AND sub-microsecond clock artifacts
    // that survive percentile trimming on FreeBSD's fast path.
    let min_ft = sorted_ft[p1_idx..count]
        .iter()
        .copied()
        .find(|&t| t > PEAK_FPS_MIN_FRAME_MS)
        .unwrap_or(0.0);
    if min_ft > 0.0 {
        1000.0 / min_ft
    } else {
        0.0
    }
}

/// Legacy CI benchmark: run N frames and print results in the original format.
/// Output format is preserved for backwards compatibility.
pub(crate) fn run_benchmark(cfg: &CloudConfig) -> std::io::Result<()> {
    // Strict-validate --bench-scene BEFORE any allocation so an invalid
    // value (typo like "leanax") fails fast instead of silently falling
    // back to the default lean path. Honesty contract: no hidden behavior.
    validate_bench_scene(cfg);

    let bench_frames = cfg.bench_frames.expect("bench_frames must be set");

    let (w, h) = crate::bench_helpers::bench_dimensions(cfg.screen_size);

    let density = effective_density(cfg.base_density, w, cfg.density_auto);

    let mut cloud = cfg.create_cloud(density);
    cloud.reset(w, h);
    cloud.set_component_timing(true); // P1: enable sim/render split for benchmark
    cloud.enable_stuck_cell_sweep = false; // T1.1: keep realloc counters clean in benchmark
    cloud.set_verbose(cfg.verbose); // silent arena unless --verbose
                                    // v30 strengthen: drift spike protection — palette drift in benchmark
                                    // mode corrupts p99/max metrics with palette-rebuild cost. Disable
                                    // here so benchmarks are deterministic. Climate drift (luminance/
                                    // saturation/hue modulation) still runs because it is deterministic
                                    // (fixed RNG seed) and has no rebuild cost.
    cloud.auto_color_drift = false;

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
/// Premium user-facing benchmark: runs for the configured duration (default
/// 5s, override with `--bench-duration N`) with live progress feedback and
/// enhanced metrics in a Report-engine output.
pub(crate) fn run_premium_benchmark(cfg: &CloudConfig) -> std::io::Result<()> {
    // Strict-validate --bench-scene BEFORE any allocation so an invalid
    // value (typo like "leanax" or "production-drawmadadadaxa") fails fast
    // instead of silently falling back to the default lean path.
    // Honesty contract: no hidden behavior, no silent fallback.
    validate_bench_scene(cfg);

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
    let density = effective_density(cfg.base_density, w, cfg.density_auto);

    let mut cloud = cfg.create_cloud(density);
    cloud.reset(w, h);
    cloud.set_component_timing(true); // P1: enable sim/render split for benchmark
    cloud.enable_stuck_cell_sweep = false; // T1.1: keep realloc counters clean in benchmark
    cloud.set_verbose(cfg.verbose); // silent arena unless --verbose
                                    // v30 strengthen: drift spike protection — palette drift in benchmark
                                    // mode corrupts p99/max metrics with palette-rebuild cost. Disable
                                    // here so benchmarks are deterministic. Climate drift (luminance/
                                    // saturation/hue modulation) still runs because it is deterministic
                                    // (fixed RNG seed) and has no rebuild cost.
    cloud.auto_color_drift = false;

    let mut frame = Frame::new_bench(w, h, cloud.palette.bg);

    let target_period = Duration::from_secs_f64(1.0 / cfg.target_fps);
    cloud.set_max_sim_delta(target_period);

    progress.init_done();

    // ── Phase 2: Initialize wet I/O writer if --bench-io ──────────────
    // T2.1: pass palette so BenchIoWriter can build a ColorCache and mirror
    // the production Terminal::draw() fast path (pre-formatted SGR bytes).
    // --bench-scene production-draw also requires --bench-io (the writer
    // is what gets routed through the production path).
    let bench_scene_production = cfg
        .bench_scene
        .as_deref()
        .map(|s| s == "production-draw")
        .unwrap_or(false);
    if bench_scene_production && !cfg.bench_io {
        crate::ux::or_exit::<(), String>(Err(
            "error: --bench-scene production-draw requires --bench-io to be set \
             (it routes the BenchIoWriter through the production draw path)"
                .to_string(),
        ));
    }
    let mut io_writer = if cfg.bench_io {
        crate::bench_io::BenchIoWriter::with_palette(&cloud.palette)
    } else {
        None
    };

    // ── Phase 3-6: Initialize measurement collectors ──────────────────
    // perf counters must be opened before warmup (fd + ENABLE ioctl), but
    // the baseline read is taken AFTER warmup so the delta only covers the
    // measurement window — matching total_elapsed_s and total_frames scope.
    let perf_handle = crate::bench_perf::open_counters();
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
        // Phase 2: wet I/O — write ANSI to /dev/null if --bench-io.
        // --bench-scene production-draw routes through write_frame_production
        // (mirrors Terminal::draw full-redraw path) instead of the default
        // emit_cell_lean fast path.
        if let Some(ref mut io) = io_writer {
            if bench_scene_production {
                io.write_frame_production(&frame);
            } else {
                io.write_frame(&frame);
            }
        }

        frame.clear_dirty();
        progress.warmup_tick();
    }
    progress.warmup_done();

    // ── Baseline snapshots (scope = measurement phase only) ─────────────
    // Taken AFTER warmup so the delta excludes warmup cycles/energy/allocs,
    // matching total_elapsed_s and total_frames which also exclude warmup.
    let alloc_before = crate::alloc_trace::AllocSnapshot::now();
    let energy_before = crate::bench_energy::EnergySnapshot::now();
    let perf_before = perf_handle.as_ref().map(|h| h.read()).unwrap_or_default();

    // ── Measurement phase ────────────────────────────────────────────────
    let mut frame_times: [f64; FRAME_TIME_SAMPLES] = [0.0; FRAME_TIME_SAMPLES];
    let mut ft_index: usize = 0;
    let mut total_frames: u64 = 0;
    let mut drawn_frames: u64 = 0;
    let mut total_drawn_cells: u64 = 0;
    let mut max_dirty_cells: u64 = 0;
    let mut dirty_all_frames: u64 = 0;
    let mut active_streams_sum: u64 = 0;
    // bolt: rate-limit cloud.active_droplet_count() sampling. For non-Monolith
    // scenes (cinematic, storm) this is O(N) per call — iterates
    // self.droplets filtering is_alive. At 58K FPS with ~2K droplets (storm),
    // that's 116M filter+count iterations/sec — pure waste for a metric that
    // changes slowly (droplets spawn/die on ~100ms timescale, not per-frame).
    // Sampling every 64 frames cuts the call rate by 64×; for a 10s bench at
    // 58K FPS that's still 906 samples — statistically representative for the
    // reported integer average. The first frame is always sampled so short
    // benches still report a non-zero value.
    let mut streams_samples: u64 = 0;
    let total_cells = (w as usize) * (h as usize);

    // Sub-component timing tracker — see bench_comp.rs for component
    // definitions (sim/render/io). io_ms is a residual bucket: when
    // --bench-io is active it includes ANSI writes to /dev/null via
    // BenchIoWriter; otherwise it captures VisualSampler sampling +
    // clear_dirty + loop bookkeeping.
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

    // Rate-limit terminal event polling to 10Hz. crossterm::event::poll
    // allocates 3 buffers per call (file descriptor set + internal state),
    // which at 70K FPS benchmark throughput becomes 210K allocs/sec — the
    // dominant allocation source in the benchmark loop. Resize detection
    // only needs human-response latency (~100ms); polling every frame is
    // pure waste. Interactive mode is unaffected — the bench loop runs at
    // 1000×+ the interactive framerate, so 10Hz here ≈ 0.1Hz-equivalent
    // CPU load vs interactive.
    let mut last_poll = Instant::now();
    let poll_interval = Duration::from_millis(100);

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

        // Phase 2: wet I/O — write ANSI to /dev/null if --bench-io.
        // Routes through write_frame_production when --bench-scene
        // production-draw is set (mirrors Terminal::draw's hot path).
        if let Some(ref mut io) = io_writer {
            if bench_scene_production {
                io.write_frame_production(&frame);
            } else {
                io.write_frame(&frame);
            }
        }

        // Phase 6: visual objective metrics sampling
        visual_sampler.sample(&frame);

        frame.clear_dirty();

        let frame_time_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
        // io_ms = total frame time minus sim and render. This is a residual
        // bucket that captures: (1) BenchIoWriter::write_frame() when
        // --bench-io is active (writes ANSI to /dev/null), (2) VisualSampler
        // sampling cost (amortized over N frames), (3) dirty-tracking
        // clear_dirty() + loop bookkeeping. When --bench-io is NOT passed,
        // only (2) and (3) contribute — clear_dirty is O(1) via generation
        // bump, so io_ms is dominated by loop bookkeeping in that case.
        // Clamped to >= 0 to guard against clock skew between Instant::now()
        // calls on different cores.
        let io_ms = (frame_time_ms - sim_ms - render_ms).max(0.0);

        components.record(sim_ms, render_ms, io_ms);

        if ft_index < FRAME_TIME_SAMPLES {
            frame_times[ft_index] = frame_time_ms;
            ft_index += 1;
        }
        total_frames += 1;
        // bolt: sample active streams every 64 frames (bitmask mod, cheap).
        // total_frames starts at 1 after the first increment, so the condition
        // `total_frames == 1 || total_frames & 63 == 0` triggers on frame 1,
        // then frames 64, 128, 192, ... — guarantees ≥1 sample for any bench
        // length, and ~906 samples for a typical 10s @ 58K FPS run.
        if total_frames == 1 || total_frames & 63 == 0 {
            active_streams_sum += cloud.active_droplet_count() as u64;
            streams_samples += 1;
        }

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
        // Rate-limited terminal event drain (see comment above the loop).
        if last_poll.elapsed() >= poll_interval {
            last_poll = Instant::now();
            while crossterm::event::poll(Duration::from_millis(0)).unwrap_or(false) {
                if let Ok(crossterm::event::Event::Resize(_, _)) = crossterm::event::read() {
                    terminal_resized_during_bench = true;
                }
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

    // Verbose-only stuck-cell sweep summary (single line, not per-sweep spam).
    // The sweep itself ran silently during the benchmark; this is the
    // aggregated diagnostic. Suppressed in non-verbose mode.
    if cfg.verbose {
        let (cleared, sweeps) = cloud.stuck_cell_stats();
        if cleared > 0 {
            eprintln!(
                "[stuck-cell-sweep] healed {cleared} cell(s) across {sweeps} sweep(s) during benchmark"
            );
        }
    }

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
    // v50 LTS audit fix: previously this was `sum(frame_times) / ft_index`,
    // which measured only the `frame_start`→`clear_dirty` interval per
    // frame. That interval misses loop bookkeeping between samples:
    // 4–5 `Instant::now()` calls, `components.record`, `ft_index` update,
    // `total_frames += 1`, the active-streams sample, `progress.tick`,
    // and the non-blocking event poll. On Linux these add ~1–2 µs/frame
    // (vDSO, ~20 ns/call); on FreeBSD `Instant::now()` is a real
    // `clock_gettime` syscall (~2 µs/call), so the missing overhead
    // reaches ~10 µs/frame — about 28 % of a 0.040 ms true interval on
    // the FreeBSD 5 s run. The result was `avg_frame_time` reporting
    // 0.0287 ms while `1000 / avg_fps` reported 0.0398 ms, an
    // inconsistency visible to any user who cross-checks the two.
    //
    // LTS fix: derive `avg_frame_time` from the SAME wall-clock interval
    // that backs `avg_fps`. This guarantees `avg_frame_time ≈ 1000 /
    // avg_fps` (within f64 rounding) on every platform, which is the
    // contract users expect. The `frame_times[]` array is still
    // collected — percentiles (p95/p99/p99.9), `max_frame_time`, and
    // `jitter_std` all come from it and are unaffected because they are
    // computed from per-frame deltas (a constant bookkeeping overhead
    // shifts the mean but not the shape of the distribution).
    let avg_frame_time = if total_frames > 0 {
        elapsed_s * 1000.0 / total_frames as f64
    } else {
        0.0
    };

    // p99 frame time — trim top/bottom 1% outliers for stability
    let mut sorted_ft: Vec<f64> = frame_times[..ft_index].to_vec();
    sorted_ft.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // peak_fps: derived from the p1 (1st percentile) frame time — the
    // fastest 1% of frames, NOT the absolute single minimum.
    //
    // Why not min? On fast systems (FreeBSD, high-frequency clocks), the
    // absolute minimum sample is dominated by outliers: a single frame
    // preempted by the OS scheduler mid-bench, or a cache-aligned lucky
    // hit, can finish in ~280ns while the average is ~33,000ns (118×
    // ratio). Reporting 1000/0.000279 = 3,584,229 FPS is mathematically
    // correct but statistically meaningless — it does not represent any
    // sustainable throughput the engine can actually deliver.
    //
    // p1 mirrors the trimming philosophy already used for p99/p95 (which
    // trim 1% from each tail). The fastest 1% of frames is a robust
    // "best-case sustained" estimate: it ignores the single most extreme
    // outlier while still capturing the genuine fast-path performance.
    //
    // Zero-sample handling: on systems where the clock resolution is
    // coarser than the frame time, `Instant::elapsed()` returns 0.0 for
    // some samples. We skip those by scanning for the first non-zero entry
    // at or above the p1 index. If ALL samples in the p1+ range are zero,
    // we fall back to 0.0 (same honest "not measurable" answer as before).
    //
    // v50 LTS stabilization: this fixes the "absurd peak_fps" reports on
    // FreeBSD (3.5M FPS) and tightens the Linux reports (57K → closer to
    // 2× avg, which is the expected ceiling for a healthy renderer).
    let peak_fps = compute_peak_fps(&sorted_ft, ft_index);
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
    // v50 LTS audit fix (Issue 2): the field previously named
    // `glyphs_per_second` was misleading — the name implied "actual
    // glyphs rendered per second", but the value is the theoretical
    // upper bound (full-frame cell count × active-frame rate). The
    // actual rendered throughput is `dirty_glyphs_per_second` (often
    // 10–20× lower because the diff engine skips clean cells).
    // Renamed to `glyphs_per_second_theoretical` so the name itself
    // documents the semantics. The redundant
    // `theoretical_full_frame_glyphs_per_second` field (which held the
    // exact same value and only appeared in the premium report) was
    // removed at the same time to eliminate the duplicate.
    let glyphs_per_second_theoretical = if drawn_frames > 0 {
        ((drawn_frames * total_cells_u64) as f64 / elapsed_s).round() as u64
    } else {
        0
    };
    let dirty_glyphs_per_second = (total_drawn_cells as f64 / elapsed_s).round() as u64;

    // v50 LTS audit fix (Issue 3): ansi_bytes_per_second is an ESTIMATE,
    // not a measured value. It multiplies `total_drawn_cells` by the
    // constant `ANSI_BYTES_PER_CELL_ESTIMATE` (19 bytes/cell, see
    // constants.rs). The 19-byte figure accounts for SGR reset + fg/bg
    // escapes + 1 char, amortized by ~0.65 run-compression. Real ANSI
    // output varies by color mode (TrueColor ≈ 3× Color16) and run
    // length, so this is a rough throughput indicator only. The basis
    // note is emitted alongside the field in the premium report.
    let ansi_bytes_per_second = ((total_drawn_cells * ANSI_BYTES_PER_CELL_ESTIMATE) as f64
        / elapsed_s.max(0.000_001)) as u64;
    let active_streams_avg = active_streams_sum / streams_samples.max(1);
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

    // ── Build and print report ────────────────────────────────────────
    // compute CONFIG-enrichment fields once so the struct literal
    // below stays readable. See `compute_config_enrichment` for derivation.
    let enrichment = crate::bench_config_enrichment::compute_config_enrichment(cfg);
    let report_data = crate::bench_report::BenchReportData {
        was_interrupted,
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
        auto_color_drift: enrichment.auto_color_drift,
        color_pipeline: enrichment.color_pipeline,
        chroma_in_benchmark: enrichment.chroma_in_benchmark,
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
        glyphs_per_second_theoretical,
        dirty_glyphs_per_second,
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
    // BL-02 (Dragon Hunt v3): dedup — hoist json_opt above the json/text
    // branch so save/compare baseline logic runs once (was duplicated
    // verbatim in both arms).
    let json_opt: Option<String> =
        if cfg.json || cfg.save_baseline.is_some() || cfg.compare_baseline.is_some() {
            Some(crate::bench_json::build_json_string(&report_data))
        } else {
            None
        };

    if cfg.json {
        // Print JSON to stdout (only in --json mode).
        if let Some(ref json) = json_opt {
            println!("{json}");
        }
    } else {
        crate::bench_report::build_premium_report(&report_data);
    }

    // Save baseline if requested (v17: path whitelist enforced).
    // For text mode, the JSON was generated above so users don't have to
    // pass --json just to save a baseline.
    if let (Some(path), Some(json)) = (cfg.save_baseline.as_ref(), json_opt.as_ref()) {
        if !crate::is_safe_path(path) {
            eprintln!(
                "error: --save-baseline '{path}' is outside allowed directories\n  \
                 Allowed: ~/.config/cosmostrix/, /etc/cosmostrix/"
            );
        } else {
            match crate::bench_baseline::save_baseline(path, json) {
                Ok(()) => eprintln!("[baseline] saved to {path}"),
                Err(e) => eprintln!("{e}"),
            }
        }
    }

    // Compare baseline if requested (v17: path whitelist enforced).
    if let (Some(path), Some(json)) = (cfg.compare_baseline.as_ref(), json_opt.as_ref()) {
        if !crate::is_safe_path(path) {
            eprintln!(
                "error: --compare-baseline '{path}' is outside allowed directories\n  \
                 Allowed: ~/.config/cosmostrix/, /etc/cosmostrix/"
            );
        } else if let Err(e) = crate::bench_baseline::compare_with_baseline(path, json) {
            eprintln!("{e}");
        }
    }
    Ok(())
}

/// Run benchmark and return the report data without printing.
/// Used by --bench-all scaling automation.
pub(crate) fn run_benchmark_capture(
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
    // Strict-validate --bench-scene so typos are rejected even when called
    // via run_benchmark_capture (used by --bench-all).
    validate_bench_scene(cfg);

    let bench_duration_secs = crate::ux::or_exit(resolve_bench_duration(cfg.bench_duration));

    let (w, h) = crate::bench_helpers::bench_dimensions(cfg.screen_size);
    let density = effective_density(cfg.base_density, w, cfg.density_auto);

    let mut cloud = cfg.create_cloud(density);
    cloud.reset(w, h);
    cloud.set_component_timing(true);
    cloud.enable_stuck_cell_sweep = false; // T1.1: keep realloc counters clean in benchmark
    cloud.set_verbose(cfg.verbose); // silent arena unless --verbose
                                    // v30 strengthen: drift spike protection — palette drift in benchmark
                                    // mode corrupts p99/max metrics with palette-rebuild cost. Disable
                                    // here so benchmarks are deterministic. Climate drift (luminance/
                                    // saturation/hue modulation) still runs because it is deterministic
                                    // (fixed RNG seed) and has no rebuild cost.
    cloud.auto_color_drift = false;

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
        auto_color_drift: enrichment.auto_color_drift,
        color_pipeline: enrichment.color_pipeline,
        chroma_in_benchmark: enrichment.chroma_in_benchmark,
        avg_fps,
        peak_fps,
        avg_frame_time,
        // v30 strengthen (audit): the silent capture path (used by --bench-all
        // and --bench-tune) does not compute percentile/jitter/stability
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

#[cfg(test)]
#[path = "bench_tests.rs"]
mod bench_tests;
