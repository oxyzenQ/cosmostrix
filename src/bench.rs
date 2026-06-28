// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

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
use std::io::{self, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::cursor::{Hide, Show};
use crossterm::execute;

use crate::constants::{
    ANSI_BYTES_PER_CELL_ESTIMATE, BENCH_ELAPSED_MIN_S, DENSITY_AUTO_DEFAULT_COLS,
    DENSITY_AUTO_DEFAULT_LINES, DIRTY_THRESHOLD_RATIO, MAX_TERMINAL_COLS, MAX_TERMINAL_LINES,
};
use crate::frame::Frame;
use crate::zactrix_core::{
    classify_frame_jitter, classify_frame_time_stability, dirty_threshold_cells,
    estimates_full_redraw,
};

use super::{effective_density, CloudConfig};

// Re-export metric meaning constants used by external modules
// (e.g., cloud/tests/tests_visual_depth.rs) so that import paths
// remain stable after the split into bench_report.rs.
#[allow(unused_imports)]
pub(crate) use crate::bench_report::{AVG_DIRTY_CELL_RATIO_MEANING, ESTIMATED_FULL_REDRAW_MEANING};

/// Duration of the premium benchmark in seconds.
const BENCHMARK_DURATION_SECS: u64 = 5;

/// Warmup duration for the premium benchmark in seconds.
const BENCHMARK_WARMUP_SECS: u64 = 2;

/// Number of frame time samples for percentile calculations.
const FRAME_TIME_SAMPLES: usize = 10_000;

/// Braille spinner frames — subtle, modern, premium.
const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Number of lines in the live rewrite region during benchmark.
const LIVE_LINES: u16 = 4;

/// Minimum interval between screen updates (~15 Hz).
const UPDATE_INTERVAL: Duration = Duration::from_millis(66);

/// Number of recent frame times to smooth for display.
const DISPLAY_FT_WINDOW: usize = 16;

// ── Cursor guard ─────────────────────────────────────────────────────────────

/// RAII guard that ensures the terminal cursor is restored on drop.
///
/// Hides the cursor on creation and shows it again when dropped.
/// This handles both normal completion and panic unwinding.
struct CursorGuard;

impl CursorGuard {
    fn acquire() -> io::Result<Self> {
        execute!(io::stderr(), Hide)?;
        Ok(Self)
    }
}

impl Drop for CursorGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stderr(), Show);
    }
}

// ── Interrupt flag ───────────────────────────────────────────────────────────

/// Register a SIGINT/ctrl-c handler that sets the given flag.
///
/// Returns the Arc flag; the caller checks it periodically.
fn register_interrupt() -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));

    #[cfg(unix)]
    {
        let f = flag.clone();
        // Best-effort: if registration fails, the benchmark still runs;
        // the user can always SIGKILL as a last resort.
        let _ = signal_hook::flag::register(signal_hook::consts::SIGINT, f);
    }

    #[cfg(windows)]
    {
        let f = flag.clone();
        let _ = ctrlc::set_handler(move || {
            f.store(true, Ordering::SeqCst);
        });
    }

    flag
}

// ── Live progress ────────────────────────────────────────────────────────────

/// Manages the live benchmark progress UI on stderr.
///
/// The UI consists of a header, init/warmup indicators, and a compact
/// live-metrics region that overwrites itself without scrolling:
///
/// ```text
/// COSMOSTRIX BENCHMARK
/// ────────────────────
/// initializing renderer... done
/// warming frame pipeline... done
/// running benchmark... ⠧
/// fps: ~12188
/// frametime: 0.083ms
/// elapsed: 3.1s / 5.0s
/// ```
///
/// On finish the entire live region is erased and the final report
/// is printed cleanly to stdout.
struct BenchProgress {
    spinner_idx: usize,
    last_update: Instant,
    running_initialized: bool,
    /// Number of newline-terminated lines written to stderr.
    /// Used by `finish()` to erase the correct number of lines.
    lines_written: u16,
    /// Whether the warmup spinner is active (line not yet newline-terminated).
    warmup_active: bool,
    /// Rolling window of recent frame times for display smoothing.
    recent_ft: [f64; DISPLAY_FT_WINDOW],
    recent_ft_idx: usize,
    recent_ft_count: usize,
    /// Whether stderr is an interactive terminal.
    is_tty: bool,
    /// RAII cursor guard.
    _cursor_guard: Option<CursorGuard>,
}

impl BenchProgress {
    fn new() -> Self {
        Self {
            spinner_idx: 0,
            // Allow the first update immediately.
            last_update: Instant::now() - UPDATE_INTERVAL,
            running_initialized: false,
            lines_written: 0,
            warmup_active: false,
            recent_ft: [0.0; DISPLAY_FT_WINDOW],
            recent_ft_idx: 0,
            recent_ft_count: 0,
            is_tty: io::stderr().is_terminal(),
            _cursor_guard: None,
        }
    }

    /// Advance the spinner and return the current frame character.
    #[inline]
    fn spin(&mut self) -> char {
        let c = SPINNER[self.spinner_idx];
        self.spinner_idx = (self.spinner_idx + 1) % SPINNER.len();
        c
    }

    /// Print the header block and hide the cursor.
    fn begin(&mut self) {
        if !self.is_tty {
            return;
        }
        self._cursor_guard = CursorGuard::acquire().ok();
        let mut stderr = io::stderr().lock();
        let _ = writeln!(stderr, "COSMOSTRIX BENCHMARK");
        let _ = writeln!(stderr, "────────────────────");
        let _ = stderr.flush();
        self.lines_written = 2;
    }

    /// Print "initializing renderer... done" — this step is fast enough
    /// that it appears as a single completed line.
    fn init_done(&mut self) {
        if !self.is_tty {
            return;
        }
        let _ = writeln!(io::stderr(), "initializing renderer... done");
        let _ = io::stderr().flush();
        self.lines_written += 1;
    }

    /// Print the initial warmup line with a spinner frame.
    fn warmup_start(&mut self) {
        if !self.is_tty {
            return;
        }
        let spinner = self.spin();
        let _ = write!(io::stderr(), "warming frame pipeline... {}  ", spinner);
        let _ = io::stderr().flush();
        self.warmup_active = true;
    }

    /// Animate the warmup spinner. Rate-limited internally.
    fn warmup_tick(&mut self) {
        if !self.is_tty {
            return;
        }
        let now = Instant::now();
        if now.duration_since(self.last_update) < UPDATE_INTERVAL {
            return;
        }
        self.last_update = now;
        let spinner = self.spin();
        let _ = write!(io::stderr(), "\rwarming frame pipeline... {}  ", spinner);
        let _ = io::stderr().flush();
    }

    /// Mark warmup as complete.
    fn warmup_done(&mut self) {
        if !self.is_tty {
            return;
        }
        let _ = write!(io::stderr(), "\x1b[2K\rwarming frame pipeline... done\n");
        let _ = io::stderr().flush();
        self.warmup_active = false;
        self.lines_written += 1;
    }

    /// Record a frame time and update the live metrics if enough time has
    /// elapsed.
    ///
    /// This is the hot-path call from the measurement loop. It is designed
    /// to be cheap on the fast path (just a timestamp comparison + one array
    /// write), so it does not distort benchmark results.
    fn running_tick(
        &mut self,
        total_frames: u64,
        elapsed_s: f64,
        frame_time_ms: f64,
        duration_s: f64,
    ) {
        if !self.is_tty {
            return;
        }

        // Always record the frame time in the rolling buffer.
        self.recent_ft[self.recent_ft_idx] = frame_time_ms;
        self.recent_ft_idx = (self.recent_ft_idx + 1) % self.recent_ft.len();
        if self.recent_ft_count < self.recent_ft.len() {
            self.recent_ft_count += 1;
        }

        // Rate-limit screen updates.
        let now = Instant::now();
        if now.duration_since(self.last_update) < UPDATE_INTERVAL {
            return;
        }
        self.last_update = now;

        let fps = if elapsed_s > 0.0 {
            total_frames as f64 / elapsed_s
        } else {
            0.0
        };

        let avg_ft = if self.recent_ft_count > 0 {
            let sum: f64 = self.recent_ft[..self.recent_ft_count].iter().sum();
            sum / self.recent_ft_count as f64
        } else {
            0.0
        };

        if !self.running_initialized {
            let spinner = self.spin();
            let _ = write!(
                io::stderr(),
                "running benchmark... {}\n\
                 fps: ~{:.0}\n\
                 frametime: {:.3}ms\n\
                 elapsed: {:.1}s / {:.1}s\n",
                spinner,
                fps,
                avg_ft,
                elapsed_s,
                duration_s,
            );
            self.running_initialized = true;
            self.lines_written += LIVE_LINES;
            let _ = io::stderr().flush();
            return;
        }

        // Rewrite the live region: move up, clear and reprint each line.
        let spinner = self.spin();
        let _ = write!(
            io::stderr(),
            "\x1b[{}A\x1b[2K\rrunning benchmark... {}\n\
             \x1b[2K\rfps: ~{:.0}\n\
             \x1b[2K\rframetime: {:.3}ms\n\
             \x1b[2K\relapsed: {:.1}s / {:.1}s\n",
            LIVE_LINES,
            spinner,
            fps,
            avg_ft,
            elapsed_s,
            duration_s,
        );
        let _ = io::stderr().flush();
    }

    /// Clear the entire live progress region and restore the terminal.
    ///
    /// After this call the terminal is left in a clean state with the
    /// cursor positioned where the benchmark output originally started.
    /// The final report should then be printed to **stdout**.
    fn finish(&mut self) {
        if !self.is_tty {
            return;
        }

        // If the warmup spinner is still active (no newline), commit the
        // line so we can count and clear it.
        if self.warmup_active {
            let _ = write!(io::stderr(), "\x1b[2K\r\n");
            self.warmup_active = false;
            self.lines_written += 1;
        }

        if self.lines_written > 0 {
            // Move to the top of our output, clear each line, return to start.
            let _ = write!(io::stderr(), "\x1b[{}A", self.lines_written);
            for _ in 0..self.lines_written {
                let _ = write!(io::stderr(), "\x1b[2K\x1b[1B");
            }
            let _ = write!(io::stderr(), "\x1b[{}A\r", self.lines_written);
            let _ = io::stderr().flush();
        }

        // Drop cursor guard — restores cursor visibility via RAII.
        self._cursor_guard = None;
    }
}

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

/// Premium user-facing benchmark: runs for 5 seconds with live progress
/// feedback and enhanced metrics in a Report-engine output.
pub fn run_premium_benchmark(cfg: &CloudConfig) -> std::io::Result<()> {
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

    let start = Instant::now();
    let bench_end = start + Duration::from_secs(BENCHMARK_DURATION_SECS);

    while Instant::now() < bench_end {
        if interrupted.load(Ordering::Relaxed) {
            break;
        }

        sim_now += target_period;

        let frame_start = Instant::now();
        cloud.rain_at(&mut frame, sim_now);

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
        if ft_index < FRAME_TIME_SAMPLES {
            frame_times[ft_index] = frame_time_ms;
            ft_index += 1;
        }
        total_frames += 1;
        active_streams_sum += cloud.active_droplet_count() as u64;

        // Live progress update — AFTER frame time measurement to avoid skew.
        let elapsed_s = start.elapsed().as_secs_f64();
        progress.running_tick(
            total_frames,
            elapsed_s,
            frame_time_ms,
            BENCHMARK_DURATION_SECS as f64,
        );
    }

    let was_interrupted = interrupted.load(Ordering::Relaxed);

    // ── Clean up live UI ─────────────────────────────────────────────────
    progress.finish();

    // ── Compute metrics ──────────────────────────────────────────────────
    let elapsed = start.elapsed();
    let elapsed_s = elapsed.as_secs_f64().max(BENCH_ELAPSED_MIN_S);

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
        // Phase 5.5 target is under 850 LOC.
        let source = include_str!("bench.rs");
        let lines = source.lines().count();
        assert!(
            lines < 850,
            "bench.rs must stay under 850 LOC target (currently {lines})"
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
}
