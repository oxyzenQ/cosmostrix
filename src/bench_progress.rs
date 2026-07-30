// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Live progress UI for the premium benchmark.
//!
//! Extracted from `bench.rs` to keep that file under its LOC guard.
//! Manages the stderr-based progress display: header, init/warmup
//! indicators, and a compact live-metrics region that overwrites itself
//! without scrolling. On finish the entire live region is erased and the
//! final report is printed cleanly to stdout.
//!
//! Also hosts the cross-platform SIGINT/Ctrl-C registration helper used
//! to gracefully interrupt the benchmark mid-run.

use std::io::{self, IsTerminal, Write};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::cursor::{Hide, Show};
use crossterm::execute;

/// Braille spinner frames — subtle, modern, premium.
const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Number of lines in the live rewrite region during benchmark.
const LIVE_LINES: u16 = 4;

/// Minimum interval between screen updates (~15 Hz).
const UPDATE_INTERVAL: Duration = Duration::from_millis(66);

/// Minimum interval between silent spinner updates (4 Hz — calm, not spammy).
///
/// The silent spinner is a single-line loading indicator shown in
/// non-verbose mode. It updates at 4 Hz (every 250ms) which is calm
/// enough to feel professional yet alive enough to communicate progress.
const SILENT_UPDATE_INTERVAL: Duration = Duration::from_millis(250);

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
/// Returns the Arc flag; the caller checks it periodically. Best-effort:
/// if registration fails, the benchmark still runs; the user can always
/// SIGKILL as a last resort.
pub(crate) fn register_interrupt() -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));

    #[cfg(unix)]
    {
        let f = flag.clone();
        let _ = signal_hook::flag::register(signal_hook::consts::SIGINT, f);
    }

    #[cfg(windows)]
    {
        let f = flag.clone();
        let _ = ctrlc::set_handler(move || {
            f.store(true, std::sync::atomic::Ordering::SeqCst);
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
///
/// ## Silent spinner mode (default)
///
/// By default (`verbose == false` AND stderr is a TTY), a single-line
/// loading spinner is shown:
///
/// ```text
/// ⠋ Running COSMOSTRIX BENCHMARK... 2.3s
/// ```
///
/// This spinner appears once on a single line, updates at 4 Hz (calm,
/// not spammy), and is erased when the benchmark completes — leaving
/// only the final report on stdout. Pass `verbose = true` (from
/// `cfg.verbose`, i.e., `cosmostrix --benchmark --verbose`) to show the
/// full multi-line progress UI with header, warmup indicators, and live
/// metrics for debugging.
///
/// When stderr is NOT a TTY (piped/redirected), all progress output is
/// suppressed to avoid ANSI pollution — only the final report prints.
pub(crate) struct BenchProgress {
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
    /// Whether verbose progress UI is enabled (from `cfg.verbose`).
    /// When true AND `is_tty`, the full multi-line progress UI is shown
    /// (header, warmup indicators, live metrics). When false, a single-
    /// line silent spinner is shown instead (see `silent_active`).
    verbose: bool,
    /// Whether the silent single-line spinner is currently displayed.
    /// Active in non-verbose mode when stderr is a TTY. Shows:
    /// `⠋ Running COSMOSTRIX BENCHMARK... X.Xs` updated at 4 Hz.
    silent_active: bool,
    /// When the silent spinner started (for elapsed display).
    silent_start: Option<Instant>,
    /// RAII cursor guard.
    _cursor_guard: Option<CursorGuard>,
}

impl BenchProgress {
    pub(crate) fn new(verbose: bool) -> Self {
        Self {
            spinner_idx: 0,
            // Allow the first update immediately.
            // Use checked_sub to avoid panic if Instant epoch is very close
            // to now (theoretically possible in containers/VMs at boot).
            last_update: Instant::now()
                .checked_sub(UPDATE_INTERVAL)
                .unwrap_or_else(Instant::now),
            running_initialized: false,
            lines_written: 0,
            warmup_active: false,
            recent_ft: [0.0; DISPLAY_FT_WINDOW],
            recent_ft_idx: 0,
            recent_ft_count: 0,
            is_tty: io::stderr().is_terminal(),
            verbose,
            silent_active: false,
            silent_start: None,
            _cursor_guard: None,
        }
    }

    /// Whether the progress UI should be displayed at all.
    /// Requires both `--verbose` (user opt-in) AND an interactive stderr
    /// (so piping benchmark output to a file is always silent).
    #[inline]
    fn should_display(&self) -> bool {
        self.verbose && self.is_tty
    }

    /// Whether the silent single-line spinner should be shown.
    /// Active when: non-verbose mode AND interactive stderr. Verbose
    /// mode uses the full progress UI instead; piped output stays
    /// completely silent to avoid ANSI escape pollution in files.
    #[inline]
    fn should_display_silent(&self) -> bool {
        !self.verbose && self.is_tty
    }

    /// Advance the spinner and return the current frame character.
    #[inline]
    fn spin(&mut self) -> char {
        let c = SPINNER[self.spinner_idx];
        self.spinner_idx = (self.spinner_idx + 1) % SPINNER.len();
        c
    }

    /// Print the header block and hide the cursor.
    ///
    /// In verbose mode: prints the multi-line header. In silent mode:
    /// prints the initial single-line spinner. Both acquire the cursor
    /// guard to hide the cursor during the benchmark.
    pub(crate) fn begin(&mut self) {
        if self.should_display() {
            self._cursor_guard = CursorGuard::acquire().ok();
            let mut stderr = io::stderr().lock();
            // v17: purple header for benchmark title
            let _ = write!(stderr, "{}", crate::output::brand_bold_open());
            let _ = writeln!(stderr, "COSMOSTRIX BENCHMARK{}", crate::output::reset());
            let _ = writeln!(stderr, "────────────────────");
            let _ = stderr.flush();
            self.lines_written = 2;
            return;
        }
        if self.should_display_silent() {
            self._cursor_guard = CursorGuard::acquire().ok();
            self.silent_start = Some(Instant::now());
            self.silent_active = true;
            self.last_update = Instant::now();
            let spinner = self.spin();
            let _ = write!(io::stderr(), "{spinner} Running COSMOSTRIX BENCHMARK...");
            let _ = io::stderr().flush();
        }
    }

    /// Print "initializing renderer... done" — this step is fast enough
    /// that it appears as a single completed line.
    pub(crate) fn init_done(&mut self) {
        if !self.should_display() {
            return;
        }
        let _ = writeln!(io::stderr(), "initializing renderer... done");
        let _ = io::stderr().flush();
        self.lines_written += 1;
    }

    /// Print the initial warmup line with a spinner frame.
    pub(crate) fn warmup_start(&mut self) {
        if !self.should_display() {
            return;
        }
        let spinner = self.spin();
        let _ = write!(io::stderr(), "warming frame pipeline... {}  ", spinner);
        let _ = io::stderr().flush();
        self.warmup_active = true;
    }

    /// Animate the warmup spinner. Rate-limited internally.
    /// In silent mode, delegates to `silent_tick()` to keep the single
    /// loading line alive during the warmup phase.
    pub(crate) fn warmup_tick(&mut self) {
        if self.should_display() {
            let now = Instant::now();
            if now.duration_since(self.last_update) < UPDATE_INTERVAL {
                return;
            }
            self.last_update = now;
            let spinner = self.spin();
            let _ = write!(io::stderr(), "\rwarming frame pipeline... {}  ", spinner);
            let _ = io::stderr().flush();
            return;
        }
        self.silent_tick();
    }

    /// Mark warmup as complete.
    pub(crate) fn warmup_done(&mut self) {
        if !self.should_display() {
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
    pub(crate) fn running_tick(
        &mut self,
        total_frames: u64,
        elapsed_s: f64,
        frame_time_ms: f64,
        duration_s: f64,
    ) {
        if !self.should_display() {
            // Silent mode: update the single loading line, ignore live
            // metrics (those are verbose-only diagnostics).
            let _ = (total_frames, elapsed_s, frame_time_ms, duration_s);
            self.silent_tick();
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

    /// Update the silent single-line spinner (non-verbose mode).
    ///
    /// Rate-limited to 4 Hz — calm enough to feel professional, alive
    /// enough to communicate progress. Rewrites the line in place with
    /// `\r` so no scrolling occurs. Shows elapsed time since `begin()`.
    fn silent_tick(&mut self) {
        if !self.silent_active {
            return;
        }
        let now = Instant::now();
        if now.duration_since(self.last_update) < SILENT_UPDATE_INTERVAL {
            return;
        }
        self.last_update = now;
        let elapsed = self
            .silent_start
            .map(|s| s.elapsed().as_secs_f64())
            .unwrap_or(0.0);
        let spinner = self.spin();
        let _ = write!(
            io::stderr(),
            "\r{spinner} Running COSMOSTRIX BENCHMARK... {elapsed:.1}s"
        );
        let _ = io::stderr().flush();
    }

    /// Clear the entire live progress region and restore the terminal.
    ///
    /// After this call the terminal is left in a clean state with the
    /// cursor positioned where the benchmark output originally started.
    /// The final report should then be printed to **stdout**.
    pub(crate) fn finish(&mut self) {
        if self.should_display() {
            // Verbose path — clear the multi-line live region.
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
            return;
        }

        // Silent path — clear the single spinner line.
        if self.silent_active {
            let _ = write!(io::stderr(), "\r\x1b[2K");
            let _ = io::stderr().flush();
            self.silent_active = false;
            self._cursor_guard = None;
        }
    }
}
