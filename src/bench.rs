// Copyright (c) 2026 rezky_nightky

//! Headless benchmark runners for Cosmostrix.
//!
//! Two modes:
//! - `--bench-frames N`: CI/regression benchmark, prints legacy `BENCH:` format.
//! - `--benchmark`: Premium user-facing 5-second benchmark with live progress
//!   and Report engine output.

use std::env;
use std::io::{self, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::cursor::{Hide, Show};
use crossterm::execute;

use crate::constants::{
    BENCH_ELAPSED_MIN_S, DENSITY_AUTO_DEFAULT_COLS, DENSITY_AUTO_DEFAULT_LINES, MAX_TERMINAL_COLS,
    MAX_TERMINAL_LINES,
};
use crate::diagnostics;
use crate::frame::Frame;
use crate::renderer_info;
use crate::report::Report;

use super::{effective_density, CloudConfig};

/// Duration of the premium benchmark in seconds.
const BENCHMARK_DURATION_SECS: u64 = 5;

/// Warmup duration for the premium benchmark in seconds.
const BENCHMARK_WARMUP_SECS: u64 = 1;

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
                spinner, fps, avg_ft, elapsed_s, duration_s,
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
            LIVE_LINES, spinner, fps, avg_ft, elapsed_s, duration_s,
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
    let warmup_end = Instant::now() + Duration::from_secs(BENCHMARK_WARMUP_SECS);
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
    let mut total_ansi_bytes: u64 = 0;
    let mut active_streams_sum: u64 = 0;

    let start = Instant::now();
    let bench_end = start + Duration::from_secs(BENCHMARK_DURATION_SECS);

    while Instant::now() < bench_end {
        if interrupted.load(Ordering::Relaxed) {
            break;
        }

        sim_now += target_period;

        let frame_start = Instant::now();
        cloud.rain_at(&mut frame, sim_now);

        let did_draw = frame.is_dirty_all() || !frame.dirty_indices().is_empty();
        if did_draw {
            drawn_frames += 1;
            let dirty_count = if frame.is_dirty_all() {
                (w as usize) * (h as usize)
            } else {
                frame.dirty_indices().len()
            };
            total_ansi_bytes += (dirty_count as u64) * 20;
        }

        frame.clear_dirty();

        let frame_time_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
        if ft_index < FRAME_TIME_SAMPLES {
            frame_times[ft_index] = frame_time_ms;
            ft_index += 1;
        }
        total_frames += 1;
        active_streams_sum += cloud.droplet_count() as u64;

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

    // p99 frame time
    let mut sorted_ft: Vec<f64> = frame_times[..ft_index].to_vec();
    sorted_ft.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p99_frame_time = if ft_index > 0 {
        sorted_ft[(((ft_index as f64) * 0.99) as usize).min(ft_index - 1)]
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
    let jitter_classification = if jitter_std < 0.5 {
        "low"
    } else if jitter_std < 2.0 {
        "medium"
    } else {
        "high"
    };

    let total_cells = (w as u64) * (h as u64);
    let glyphs_per_second = if drawn_frames > 0 {
        ((drawn_frames * total_cells) as f64 / elapsed_s).round() as u64
    } else {
        0
    };

    let ansi_bytes_per_second = (total_ansi_bytes as f64 / elapsed_s).round() as u64;
    let active_streams_avg = active_streams_sum / total_frames.max(1);

    let draw_ratio = if total_frames > 0 {
        (drawn_frames as f64) / (total_frames as f64) * 100.0
    } else {
        0.0
    };

    // ── Build report ─────────────────────────────────────────────────────
    let cpu = diagnostics::detect_cpu_info();
    let ri = renderer_info::renderer_info(cfg.color_mode);

    let mut r = Report::new("COSMOSTRIX BENCHMARK");

    if was_interrupted {
        r.section("STATUS").advice("interrupted — results are partial");
    }

    {
        let s = r.section("SYSTEM");
        s.field("variant", cpu.variant);
        s.field("optimization", &diagnostics::feature_string(&cpu.features));
        s.field("build", cpu.build_variant);
    }

    {
        let s = r.section("RENDERER");
        s.field("backend", ri.backend);
        s.field("pacing", ri.pacing);
        s.field("frame_strategy", ri.frame_strategy);
        s.field("color_depth", ri.color_depth);
    }

    {
        let s = r.section("CONFIG");
        s.field("cols", &w.to_string());
        s.field("lines", &h.to_string());
        s.field("target_fps", &format!("{:.1}", cfg.target_fps));
        s.field("density", &format!("{:.2}", cfg.density));
    }

    {
        let s = r.section("PERFORMANCE");
        s.field("avg_fps", &format!("{:.1}", avg_fps));
        s.field("peak_fps", &format!("{:.1}", peak_fps));
        s.field("avg_frame_time", &format!("{:.3}ms", avg_frame_time));
        s.field("p99_frame_time", &format!("{:.3}ms", p99_frame_time));
        s.field("frame_jitter", jitter_classification);
        s.field("draw_ratio", &format!("{:.1}%", draw_ratio));
    }

    {
        let s = r.section("THROUGHPUT");
        s.field("glyphs_per_second", &glyphs_per_second.to_string());
        s.field("ansi_bytes_per_second", &ansi_bytes_per_second.to_string());
        s.field("active_streams_avg", &active_streams_avg.to_string());
    }

    {
        let s = r.section("TIMING");
        s.field("elapsed", &format!("{:.3}s", elapsed_s));
        s.field("total_frames", &total_frames.to_string());
        s.field("drawn_frames", &drawn_frames.to_string());
    }

    // Final report goes to stdout — clean, pipeable.
    r.print();
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
