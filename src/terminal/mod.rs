// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Terminal abstraction layer for Cosmostrix.
//!
//! Provides raw mode, alternate screen management, optional mouse capture, and the
//! core diff-based ANSI rendering pipeline.
//!
//! ## Output Strategy
//!
//! The terminal uses a 64 KiB buffered writer to batch an entire frame's
//! ANSI commands into a single `write()` syscall. Within each frame, the
//! renderer uses run-length encoding: consecutive cells sharing the same
//! style (foreground, background, bold) are batched into a single string
//! buffer, minimizing the number of `SetForegroundColor` / `SetBackgroundColor`
//! commands.
//!
//! For differential (non-full) redraws, dirty cells are grouped by row,
//! sorted, and scanned for contiguous runs of matching style. This produces
//! minimal cursor movement and style-change overhead.
//!
//! ## Terminal Safety
//!
//! A RAII [`Terminal`] guard ensures the alternate screen, raw mode, and
//! cursor visibility are always restored on drop — including panic unwinding.
//! A fork-based SIGKILL guard (Linux) provides a last-resort safety net
//! for cases where the process is killed with signal 9.

#[cfg(unix)]
use std::io::{stdin, IsTerminal};
use std::io::{stdout, BufWriter, Result, Stdout, Write};
#[cfg(unix)]
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[cfg(unix)]
use std::fs::File;

/// Global flag set by the panic hook when it has already restored the
/// terminal (called `restore_terminal_best_effort()`). Terminal::drop
/// checks this flag and skips its own cleanup if the terminal was already
/// restored — otherwise the BufWriter's pending frame data would be
/// flushed to the MAIN screen (after LeaveAlternateScreen), leaking
/// partially-rendered rain onto the user's terminal.
///
/// ## Why this exists (v16 audit)
///
/// On Windows (and most Unix terminals), the alternate screen buffer
/// captures BOTH stdout AND stderr. When a panic occurs:
///   1. Panic hook runs → prints message to stderr (trapped in alt screen)
///   2. Unwind starts → Terminal::drop runs → LeaveAlternateScreen
///   3. The panic message (in the alt screen buffer) is DISCARDED
///   4. User sees a "silent exit" — no error, no crash message
///
/// Fix: the panic hook restores the terminal BEFORE printing, so the
/// message goes to the main screen. This flag prevents Terminal::drop
/// from double-restoring AND from flushing stale rain data to the main
/// screen.
pub(crate) static TERMINAL_RESTORED_BY_PANIC: AtomicBool = AtomicBool::new(false);

use crossterm::{
    cursor, event,
    style::{Attribute, Color, ResetColor, SetAttribute},
    terminal as crossterm_terminal, ExecutableCommand, QueueableCommand,
};

use crate::cell::Cell;
use crate::color_cache::ColorCache;
use crate::constants::{
    MAX_TERMINAL_COLS, MAX_TERMINAL_LINES, MIN_TERMINAL_COLS, MIN_TERMINAL_LINES,
    RENDER_COMBINED_FLUSH_INIT_CAP, RENDER_ROW_BUF_INIT_CAP, RENDER_RUN_BUF_INIT_CAP,
    SHUTDOWN_TIMEOUT_SECS,
};
use crate::sgr_format::write_sgr_colors_buf;
use crate::termdetect::TerminalCaps;
use crate::tier2::{should_backpressure, should_ris_reset, ByteWindow};

// ── dragon-fight split: sub-modules ──────────────────────────────────────
// Extracted from this file to keep mod.rs under the 1500-LOC cap and isolate
// concerns. See each module's docs for its responsibility.
mod draw;
mod last_frame;
#[cfg(test)]
mod p5_tests;

use last_frame::LastFrame;

/// Buffer size for stdout BufWriter (256 KiB). Large enough to batch an
/// entire frame's ANSI commands into a single `write()` syscall during
/// interactive mode. Sized at 256 KiB (up from 64 KiB) so that even
/// dense dirty-all frames (4.8K cells × ~30 bytes/cell ANSI ≈ 140 KB)
/// fit without spilling a second syscall.
const STDOUT_BUF_CAPACITY: usize = 256 * 1024;

pub(crate) struct Terminal {
    stdout: BufWriter<Stdout>,
    last: Option<LastFrame>,
    run_buf: String,
    /// Reusable buffer for full-redraw row batching (avoids per-frame allocation).
    row_buf: String,
    /// Flat reusable buffer for diff-redraw: holds dirty cell indices
    /// sorted by index (= row-major order), replacing the previous
    /// `Vec<Vec<usize>>` nested structure. Single allocation, better
    /// cache locality, no per-row Vec pointer chasing.
    dirty_flat: Vec<usize>,
    /// Direct ANSI byte buffer for hot-path commands.
    ///
    /// Bypasses crossterm's `.queue()` which incurs per-call overhead:
    /// trait dispatch + Adapter struct + fmt::Write machinery + heap
    /// String allocation for Rgb colors (`format!("2;{r};{g};{b}")`)
    /// and SetAttribute (`sgr()` returns String).
    ///
    /// This buffer accumulates raw ANSI bytes for color/cursor/print
    /// commands, then flushes once per frame via `write_all`. Eliminates
    /// ~170 heap allocs/frame (one per style change) + ~170 trait
    /// dispatch calls.
    ansi_buf: Vec<u8>,
    mouse_capture_enabled: bool,
    focus_change_enabled: bool,
    bracketed_paste_enabled: bool,
    raw_mode_enabled: bool,
    alternate_screen_enabled: bool,
    cursor_hidden: bool,
    line_wrap_disabled: bool,
    cleaned_up: bool,
    /// Set to `true` after flush completes; the force-exit watchdog checks
    /// this and skips `process::exit` when cleanup finished normally.
    shutdown_complete: Arc<AtomicBool>,
    /// Terminal protocol capabilities detected at startup.
    term_caps: TerminalCaps,
    /// Color byte cache for palette colors (built after palette is known).
    color_cache: Option<ColorCache>,
    /// Cumulative ANSI bytes flushed to stdout across all frames.
    /// Incremented in `flush_ansi()` by `ansi_buf.len()` before clearing.
    /// Used by `--perf-stats` to report average bytes/frame and total bandwidth.
    total_ansi_bytes: u64,
    /// Number of `flush_ansi()` calls (= number of frames drawn).
    /// Used with `total_ansi_bytes` to compute average bytes/frame.
    flush_count: u64,
    /// P3: reusable buffer for combining SYNC_START + ansi_buf + SYNC_END
    /// into a single write_all call. Only used when sync_output is enabled.
    /// Grows as needed, never shrinks. Avoids per-frame allocation.
    combined_flush_buf: Vec<u8>,
    /// P3: lazily-opened /dev/tty handle used as a one-shot recovery
    /// channel when the primary stdout fd breaks mid-run (SSH disconnect,
    /// terminal emulator crash, parent process death). Cached so multiple
    /// recovery attempts within the same shutdown window reuse the same
    /// fd instead of leaking handles.
    #[cfg(unix)]
    tty_fallback: Option<File>,
    /// P3: count of consecutive /dev/tty recoveries. Capped at
    /// STDOUT_FALLBACK_MAX_RECOVERIES to prevent a pathological loop
    /// when /dev/tty itself is broken (e.g., no controlling terminal
    /// under `setsid`). When the cap is exceeded, the original error
    /// propagates and the event loop exits via the normal error path.
    #[cfg(unix)]
    tty_recoveries: u32,
    /// Latency of the last `write_with_recovery` call, in nanoseconds.
    /// Read by the event loop to feed `perf_pressure` when writes are
    /// slow (e.g., VSCode's xterm.js falling behind over long runs).
    /// Zero until the first flush completes.
    last_write_ns: u64,
    /// Tier 2: cumulative ANSI bytes flushed since the last RIS reset.
    /// When this crosses `XTERMJS_RIS_RESET_BYTES`, `flush_ansi` emits
    /// an ESC c (RIS) to clear xterm.js's in-memory buffer. Reset to 0
    /// after each RIS emission. Tracked on all terminals (cheap), but
    /// only triggers a RIS when `xtermjs_host` is true.
    bytes_since_ris: u64,
    /// Tier 2: rolling window of per-frame byte counts, used to apply
    /// preemptive backpressure when the recent byte rate exceeds the
    /// budget. Only consulted when `term_caps.xtermjs_host` is true.
    byte_window: ByteWindow,
    /// Tier 2: # of flushes suppressed by byte-budget backpressure.
    /// Reported in `--perf-stats` exit summary. Reset only by restart.
    backpressure_skips: u64,
    /// Tier 2: # of RIS reset emissions. Reported in `--perf-stats`
    /// exit summary. Reset only by restart.
    ris_resets: u64,
    /// v30.6: true when the most recent `flush_ansi` suppressed the flush
    /// due to byte-budget backpressure. Reset on next successful write.
    /// The event loop injects a synthetic `write_overshoot` from this —
    /// otherwise suppression masks itself (no write → stale latency →
    /// no perf_pressure accumulation → self-healer never fires).
    last_flush_suppressed: bool,
}

impl Terminal {
    /// Create a Terminal. The `signal_exit` parameter is accepted for
    /// call-site compatibility but is not stored — the event loop keeps
    /// its own Arc<AtomicBool> and polls it directly.
    pub(crate) fn with_signal_exit(_signal_exit: Arc<AtomicBool>) -> Result<Self> {
        let raw = stdout();
        crossterm_terminal::enable_raw_mode()?;
        let out = BufWriter::with_capacity(STDOUT_BUF_CAPACITY, raw);
        let term_caps = crate::termdetect::detect();
        if term_caps.sync_output {
            // Enable synchronized output at the terminal level.
            // The terminal now expects ESC[?2026h / ESC[?2026l framing
            // around each logical update batch.  We wrap entire frames
            // in the draw method.
            let _ = out.get_ref().write_all(crate::termdetect::SYNC_START);
        }
        let mut term = Self {
            stdout: out,
            last: None,
            run_buf: {
                let mut s = String::new();
                s.reserve(RENDER_RUN_BUF_INIT_CAP);
                s
            },
            row_buf: String::with_capacity(RENDER_ROW_BUF_INIT_CAP),
            dirty_flat: Vec::new(),
            ansi_buf: Vec::with_capacity(STDOUT_BUF_CAPACITY),
            mouse_capture_enabled: false,
            focus_change_enabled: false,
            bracketed_paste_enabled: false,
            raw_mode_enabled: true,
            alternate_screen_enabled: false,
            cursor_hidden: false,
            line_wrap_disabled: false,
            cleaned_up: false,
            shutdown_complete: Arc::new(AtomicBool::new(false)),
            term_caps,
            color_cache: None,
            total_ansi_bytes: 0,
            flush_count: 0,
            combined_flush_buf: Vec::with_capacity(RENDER_COMBINED_FLUSH_INIT_CAP),
            #[cfg(unix)]
            tty_fallback: None,
            #[cfg(unix)]
            tty_recoveries: 0,
            last_write_ns: 0,
            bytes_since_ris: 0,
            byte_window: ByteWindow::with_capacity(
                crate::constants::XTERMJS_BYTE_BUDGET_WINDOW_FRAMES as usize,
            ),
            backpressure_skips: 0,
            ris_resets: 0,
            last_flush_suppressed: false,
        };

        let init_res: Result<()> = (|| {
            let out = &mut term.stdout;
            out.execute(crossterm_terminal::EnterAlternateScreen)?;
            term.alternate_screen_enabled = true;
            out.execute(cursor::Hide)?;
            term.cursor_hidden = true;
            if out.execute(crossterm_terminal::DisableLineWrap).is_ok() {
                term.line_wrap_disabled = true;
            }
            if out.execute(event::EnableBracketedPaste).is_ok() {
                term.bracketed_paste_enabled = true;
            }
            out.execute(SetAttribute(Attribute::Reset))?;
            out.execute(ResetColor)?;
            out.execute(crossterm_terminal::Clear(
                crossterm_terminal::ClearType::All,
            ))?;
            out.flush()?;
            Ok(())
        })();
        if let Err(e) = init_res {
            term.cleanup_terminal();
            return Err(e);
        }
        Ok(term)
    }

    pub(crate) fn size(&self) -> Result<(u16, u16)> {
        let (w, h) = crossterm_terminal::size()?;
        // Clamp to prevent OOM from misreported terminal sizes
        let w = w.min(MAX_TERMINAL_COLS);
        let h = h.min(MAX_TERMINAL_LINES);
        // Floor to prevent degenerate rendering in tiny terminals
        let w = w.max(MIN_TERMINAL_COLS);
        let h = h.max(MIN_TERMINAL_LINES);
        Ok((w, h))
    }

    pub(crate) fn poll_event(timeout: std::time::Duration) -> Result<bool> {
        event::poll(timeout)
    }

    pub(crate) fn read_event() -> Result<event::Event> {
        event::read()
    }

    /// Flush the ANSI buffer to stdout via a single write_all call.
    /// When synchronized output is supported, wraps the frame in
    /// `ESC[?2026h` / `ESC[?2026l` markers for tear-free rendering.
    ///
    /// Also accumulates `ansi_buf.len()` into `total_ansi_bytes` and
    /// increments `flush_count` for `--perf-stats` reporting. The sync
    /// wrapper bytes (12 bytes total when sync_output is enabled) are
    /// NOT counted — only the actual frame content.
    ///
    /// P3 optimization: when sync_output is enabled, combine the 3
    /// write_all calls (SYNC_START + ansi_buf + SYNC_END) into a single
    /// write_all via a combined buffer. This reduces syscalls from 3 to 1
    /// per frame. At 60 FPS, saves 120 syscalls/second (3→1 × 60).
    /// Each syscall is ~1µs, so saves ~120µs/second of syscall overhead.
    ///
    /// The combined buffer is reused across frames (self.combined_flush_buf)
    /// to avoid per-frame allocation. It grows as needed but never shrinks.
    #[inline]
    fn flush_ansi(&mut self) -> Result<()> {
        if self.ansi_buf.is_empty() {
            return Ok(());
        }
        // Accumulate encoding stats BEFORE clearing the buffer.
        // Only count frame content, not sync wrappers.
        let frame_bytes = self.ansi_buf.len() as u64;
        self.total_ansi_bytes += frame_bytes;
        self.flush_count += 1;

        // Tier 2 -- RIS reset (xterm.js hosts only).
        //
        // Check BEFORE backpressure: a RIS emission is itself ~20 bytes
        // (RIS + re-enter-alternate-screen + cursor hide + SGR mouse
        // mode), negligible vs the threshold, so even if we are about
        // to suppress this flush for backpressure, the RIS still fires
        // and resets xterm.js's buffer.
        if self.term_caps.xtermjs_host {
            let cumulative = self.bytes_since_ris + frame_bytes;
            if should_ris_reset(cumulative, self.bytes_since_ris) {
                self.emit_ris_reset()?;
                // bytes_since_ris was reset inside emit_ris_reset; the
                // upcoming flush's frame_bytes will be added below.
            }
        }

        // Tier 2 -- byte-budget backpressure (xterm.js hosts only).
        // If the rolling window exceeds the budget, suppress this flush.
        // Rain state still advances (event loop calls cloud.rain_at()
        // BEFORE term.draw()), so the user sees a brief stutter rather
        // than a permanent desync. The 0 byte count is pushed into the
        // window so the budget recovers as old frames age out.
        if self.term_caps.xtermjs_host {
            let window_sum = self.byte_window.sum();
            if should_backpressure(window_sum, self.bytes_since_ris) {
                self.backpressure_skips += 1;
                self.byte_window.push(0);
                // v30.6: signal backpressure so the event loop injects
                // a synthetic write_overshoot (otherwise suppression
                // masks itself: no write → stale latency → no
                // perf_pressure → self-healer never fires).
                self.last_flush_suppressed = true;
                self.ansi_buf.clear();
                return Ok(());
            }
        }

        // Extract ansi_buf so write_with_recovery can borrow `*self`
        // mutably for the recovery path. The Vec's allocation is preserved
        // across take + restore -- zero per-frame alloc cost.
        let mut ansi_buf = std::mem::take(&mut self.ansi_buf);

        let write_result = if self.term_caps.sync_output {
            // P3: combine SYNC_START + ansi_buf + SYNC_END into one write.
            // Reuse the combined buffer to avoid per-frame allocation.
            let mut combined = std::mem::take(&mut self.combined_flush_buf);
            combined.clear();
            combined.extend_from_slice(crate::termdetect::SYNC_START);
            combined.extend_from_slice(&ansi_buf);
            combined.extend_from_slice(crate::termdetect::SYNC_END);
            let r = self.write_with_recovery(&combined);
            // Restore the combined buffer for reuse next frame.
            self.combined_flush_buf = combined;
            r
        } else {
            self.write_with_recovery(&ansi_buf)
        };

        match write_result {
            Ok(()) => {
                // Success: clear ansi_buf (preserves allocation for reuse).
                ansi_buf.clear();
                self.ansi_buf = ansi_buf;
                // Tier 2: record this frame's byte contribution.
                if self.term_caps.xtermjs_host {
                    self.byte_window.push(frame_bytes);
                    self.bytes_since_ris += frame_bytes;
                }
                // v30.6: clear backpressure flag (this flush went through).
                self.last_flush_suppressed = false;
                Ok(())
            }
            Err(e) => {
                // Failure: restore ansi_buf so the next flush retries the
                // same data (matches pre-P3 semantics).
                self.ansi_buf = ansi_buf;
                Err(e)
            }
        }
    }

    /// Tier 2: emit an RIS (Reset to Initial State) sequence -- ESC c.
    ///
    /// This forces xterm.js to clear its in-memory scrollback buffer,
    /// preventing the unbounded growth that leads to V8 OOM (SIGTRAP)
    /// over multi-hour runs. After the RIS, we re-enter the alternate
    /// screen and re-hide the cursor -- RIS in some terminals exits the
    /// alternate screen and resets cursor visibility, which would leave
    /// the user with a scrambled display.
    ///
    /// Cost: ~20 bytes of ANSI (RIS + alternate screen + cursor hide +
    /// SGR mouse mode). Fires at most every ~7 seconds under sustained
    /// max load (50 MB threshold / 7 MB/sec rate), so the per-frame
    /// amortized cost is ~3 bytes -- negligible.
    ///
    /// After emission, `bytes_since_ris` and `byte_window` are reset
    /// since the buffer they were tracking has been nuked.
    fn emit_ris_reset(&mut self) -> Result<()> {
        // Build the reset sequence: RIS + re-enter alternate screen +
        // re-hide cursor. RIS alone is sufficient for xterm.js (its
        // implementation preserves alternate screen state), but the
        // extra bytes are cheap insurance against stricter terminals
        // (e.g., a future xterm.js host that fully resets on RIS).
        //
        // Sequence breakdown:
        //   ESC c            -- RIS (Reset to Initial State)
        //   ESC [ ? 1049 h   -- Enter alternate screen (DECSET 1049)
        //   ESC [ ? 25 l     -- Hide cursor (DECTCEM off)
        //   ESC [ ? 1006 h   -- Re-enable SGR mouse mode (in case RIS
        //                      reset it -- xterm.js preserves this, but
        //                      other hosts might not)
        const RIS_RECOVERY: &[u8] = b"\x1bc\x1b[?1049h\x1b[?25l\x1b[?1006h";

        self.write_with_recovery(RIS_RECOVERY)?;
        self.ris_resets += 1;
        self.bytes_since_ris = 0;
        self.byte_window.reset();
        Ok(())
    }

    /// P3: write a buffer to stdout, attempting a /dev/tty fallback when
    /// the primary fd is broken mid-run (SSH disconnect, terminal crash,
    /// parent death). On a recoverable error:
    ///
    ///   1. Lazily open `/dev/tty` (Unix) or `CONOUT$` (Windows).
    ///   2. Write the buffer to the fallback handle.
    ///   3. Set `GRACEFUL_SHUTDOWN` so the main loop exits cleanly via
    ///      the normal shutdown path (Terminal::drop still runs).
    ///   4. Bump `tty_recoveries`. If it exceeds
    ///      `STDOUT_FALLBACK_MAX_RECOVERIES`, propagate the original
    ///      error — /dev/tty itself is likely broken too.
    ///
    /// Zero per-frame overhead in the steady state: the happy path is a
    /// single `write_all` on the BufWriter. Fallback only fires on error.
    #[inline]
    fn write_with_recovery(&mut self, buf: &[u8]) -> Result<()> {
        // Time the write so the event loop can detect slow downstream
        // terminals. Instant::now() is ~20ns — negligible vs the write.
        let start = std::time::Instant::now();
        let result = self.stdout.write_all(buf);
        self.last_write_ns = start.elapsed().as_nanos() as u64;
        match result {
            Ok(()) => Ok(()),
            Err(e) => self.recover_to_tty(buf, e),
        }
    }

    /// P3 helper: attempt to recover a failed stdout write by routing the
    /// buffer through /dev/tty. See `write_with_recovery` for the full
    /// contract. Returns the original error if recovery is not possible.
    #[cfg(unix)]
    fn recover_to_tty(&mut self, buf: &[u8], original_err: std::io::Error) -> Result<()> {
        use crate::constants::STDOUT_FALLBACK_MAX_RECOVERIES;
        // Non-recoverable errors (e.g., WriteZero, Interrupted) should
        // propagate unchanged. We only attempt /dev/tty when the primary
        // fd is observably broken.
        if !is_recoverable_io_error(&original_err) {
            return Err(original_err);
        }
        // Defensive cap: stop trying after N consecutive recoveries.
        if self.tty_recoveries >= STDOUT_FALLBACK_MAX_RECOVERIES {
            return Err(original_err);
        }
        // Lazily open /dev/tty on first recovery; cache for reuse.
        if self.tty_fallback.is_none() {
            self.tty_fallback = open_tty_fallback();
        }
        let Some(tty) = self.tty_fallback.as_mut() else {
            // No controlling terminal (e.g., `setsid` sandbox). Propagate
            // the original stdout error so the event loop exits normally.
            return Err(original_err);
        };
        // Best-effort write to /dev/tty. If this fails too, propagate the
        // original stdout error (more diagnostic than the tty error).
        if tty.write_all(buf).is_err() {
            return Err(original_err);
        }
        let _ = tty.flush();
        self.tty_recoveries += 1;
        // Signal the main loop to exit cleanly via the normal shutdown
        // path. This avoids racing on the broken stdout fd during cleanup.
        crate::interactive::request_graceful_shutdown();
        // AB-10 (rain-screen cleanliness): leave the alt screen BEFORE the
        // stderr write below. Without this, the notice leaks into the
        // rain matrix because the alt screen is still active (raw mode +
        // EnterAlternateScreen were sent at Terminal construction, and
        // stdout being broken doesn't unwind them). restore_terminal_best_effort
        // is idempotent — calling it here AND again later in the panic hook
        // or Terminal::drop is a no-op for the second call.
        restore_terminal_best_effort();
        // Broken-pipe-safe stderr notice — eprintln! would panic if stderr
        // is also broken (e.g., terminal fully gone), so use write_fmt.
        use std::io::Write as _;
        let _ = std::io::stderr().write_fmt(format_args!(
            "[terminal] stdout write failed ({}) — recovered via /dev/tty, exiting gracefully\n",
            original_err
        ));
        let _ = std::io::stderr().flush();
        Ok(())
    }

    /// P3 helper (Windows): /dev/tty fallback is Unix-only. On Windows,
    /// stdout corruption is rarer (the console API is more robust) and
    /// `CONOUT$` reopening requires unsafe Win32 calls. Propagate the
    /// original error for now; the watchdog still catches stuck loops.
    #[cfg(not(unix))]
    fn recover_to_tty(&mut self, _buf: &[u8], original_err: std::io::Error) -> Result<()> {
        Err(original_err)
    }

    /// P5: probe stdout fd health proactively.
    ///
    /// Called on a slow interval (`FD_HEALTH_PROBE_INTERVAL_FRAMES`,
    /// ≈60 s at 60 FPS) to detect fd corruption BEFORE a write fails.
    /// The reactive P3 path catches write failures during active
    /// rendering, but during idle periods (no redraws) stdout could
    /// break and we wouldn't notice until the next render attempt.
    /// This probe closes that window.
    ///
    /// On Unix: calls `isatty(stdout_fd)`. If it returns false (the fd
    /// is no longer connected to a terminal — e.g., SSH disconnect,
    /// terminal emulator crash, parent process death), reuses the P3
    /// recovery path by calling `recover_to_tty(b"", BrokenPipe)`.
    /// This routes the (empty) buffer through `/dev/tty`, sets
    /// `GRACEFUL_SHUTDOWN`, and logs to stderr — same exit semantics
    /// as a reactive recovery.
    ///
    /// On non-Unix: always returns `true`. Windows console handles
    /// don't fail the same way PTYs do, and the reactive P3 path
    /// (which on Windows just propagates the error) remains in effect.
    ///
    /// Returns `true` when stdout is healthy, `false` when corruption
    /// was detected and recovery was attempted. Callers should check
    /// `GRACEFUL_SHUTDOWN` after a `false` return and break the loop.
    pub(crate) fn probe_stdout_health(&mut self) -> bool {
        #[cfg(unix)]
        {
            use std::io::IsTerminal;
            if !self.stdout.get_ref().is_terminal() {
                // stdout is no longer a tty — synthesize a BrokenPipe
                // error and reuse the P3 recovery path. The empty
                // buffer means no data is written to /dev/tty (just
                // the side-effects: open /dev/tty, set GRACEFUL_SHUTDOWN,
                // log to stderr, bump tty_recoveries).
                let synthetic = std::io::Error::from(std::io::ErrorKind::BrokenPipe);
                let _ = self.recover_to_tty(b"", synthetic);
                return false;
            }
        }
        // Allow(dead_code) on non-Unix so the method body isn't flagged
        // for having no side-effects when the cfg(unix) block is gone.
        #[allow(unreachable_code)]
        true
    }

    /// Enable mouse capture so mouse events are reported.
    pub(crate) fn enable_mouse_capture(&mut self) -> Result<()> {
        self.stdout.execute(event::EnableMouseCapture)?;
        self.mouse_capture_enabled = true;
        self.stdout.execute(event::EnableFocusChange)?;
        self.focus_change_enabled = true;
        self.stdout.flush()?;
        Ok(())
    }

    /// Disable mouse capture.
    pub(crate) fn disable_mouse_capture(&mut self) -> Result<()> {
        if self.mouse_capture_enabled {
            self.stdout.execute(event::DisableMouseCapture)?;
            self.mouse_capture_enabled = false;
            // Keep the global signal-handler flag in sync so that signal
            // handlers don't issue a redundant DisableMouseCapture later.
            crate::interactive::clear_mouse_capture_flag();
        }
        if self.focus_change_enabled {
            self.stdout.execute(event::DisableFocusChange)?;
            self.focus_change_enabled = false;
        }
        self.stdout.flush()?;
        Ok(())
    }

    /// Set the color byte cache for this terminal session.
    /// Must be called after the palette is built and before the first draw.
    pub(crate) fn set_color_cache(&mut self, cache: ColorCache) {
        self.color_cache = Some(cache);
    }

    /// Return encoding statistics as `(total_ansi_bytes, flush_count, sgr_hits, sgr_misses)`.
    ///
    /// - `total_ansi_bytes`: cumulative ANSI bytes flushed to stdout across all frames.
    /// - `flush_count`: number of `flush_ansi()` calls (= number of frames drawn).
    /// - `sgr_hits`: number of `ColorCache::sgr_for_cell()` calls that returned `Some`.
    /// - `sgr_misses`: number that returned `None` (fell back to on-the-fly formatting).
    ///
    /// Used by the `--perf-stats` exit report to compute:
    ///   - average bytes/frame = total_ansi_bytes / flush_count
    ///   - total bandwidth = total_ansi_bytes / elapsed_seconds
    ///   - SGR cache hit rate = sgr_hits / (sgr_hits + sgr_misses)
    ///
    /// Returns `(0, 0, 0, 0)` if no color cache is set.
    #[must_use]
    pub(crate) fn encoding_stats(&self) -> (u64, u64, u64, u64) {
        let (hits, misses) = self
            .color_cache
            .as_ref()
            .map_or((0, 0), |c| c.cache_stats());
        (self.total_ansi_bytes, self.flush_count, hits, misses)
    }

    /// Tier 2: return Tier 2-specific stats for `--perf-stats` exit summary.
    ///
    /// - `backpressure_skips`: number of flushes suppressed by the byte-
    ///   budget backpressure path. Nonzero only when running inside an
    ///   xterm.js host AND the recent byte rate exceeded the per-window
    ///   budget -- typically during sustained full-redraw activity
    ///   (palette cycling, scene transitions, terminal resize storms).
    /// - `ris_resets`: number of ESC c (RIS) reset emissions. Each reset
    ///   clears ~50 MB of accumulated xterm.js buffer. Nonzero only on
    ///   xterm.js hosts.
    /// - `bytes_since_ris`: cumulative bytes flushed since the last RIS.
    ///   Useful for diagnosing whether Tier 2 is firing at the expected
    ///   cadence (around 50 MB / 7 sec under sustained max load).
    ///
    /// On native terminals all three fields are always 0.
    #[must_use]
    pub(crate) fn tier2_stats(&self) -> (u64, u64, u64) {
        (
            self.backpressure_skips,
            self.ris_resets,
            self.bytes_since_ris,
        )
    }

    /// Returns the latency (in nanoseconds) of the last `write_with_recovery`
    /// call. The event loop feeds this into `perf_pressure` so that slow
    /// downstream terminals (e.g., VSCode's xterm.js under multi-hour load)
    /// trigger the self-healer before the consumer OOMs.
    #[must_use]
    pub(crate) fn last_write_ns(&self) -> u64 {
        self.last_write_ns
    }

    /// v30.6: true when the most recent `flush_ansi` suppressed the flush
    /// due to byte-budget backpressure. Used by the event loop to inject
    /// a synthetic `write_overshoot` so the self-healer fires. Reset on
    /// next successful write.
    #[must_use]
    pub(crate) fn last_flush_suppressed(&self) -> bool {
        self.last_flush_suppressed
    }

    /// Emit SGR color bytes for (fg, bg) into the ANSI buffer.
    /// Uses the color cache when available, falling back to on-the-fly
    /// formatting via `write_sgr_colors_buf`.
    #[inline]
    fn emit_sgr(
        cache: Option<&ColorCache>,
        buf: &mut Vec<u8>,
        fg: Option<Color>,
        bg: Option<Color>,
    ) {
        if let Some(cache) = cache {
            if let Some(cached) = cache.sgr_for_cell(fg, bg) {
                buf.extend_from_slice(cached);
                return;
            }
        }
        write_sgr_colors_buf(buf, fg, bg);
    }

    fn cleanup_terminal(&mut self) {
        if self.cleaned_up {
            return;
        }
        self.cleaned_up = true;

        let _ = self.disable_mouse_capture();
        if self.bracketed_paste_enabled {
            let _ = self.stdout.execute(event::DisableBracketedPaste);
            self.bracketed_paste_enabled = false;
        }
        let _ = self.stdout.execute(SetAttribute(Attribute::Reset));
        let _ = self.stdout.execute(ResetColor);
        if self.cursor_hidden {
            let _ = self.stdout.execute(cursor::Show);
            self.cursor_hidden = false;
        }
        if self.line_wrap_disabled {
            let _ = self.stdout.execute(crossterm_terminal::EnableLineWrap);
            self.line_wrap_disabled = false;
        }
        // v31.1: REMOVED Clear(All) before LeaveAlternateScreen.
        //
        // v16 added MoveTo(0,0)+Clear(All) before LeaveAlternateScreen to
        // prevent rain residue from bleeding onto the main screen during the
        // buffer swap. However, \x1b[2J inside the alternate screen can
        // clear the main screen's scrollback on some terminal emulators
        // (VTE-based, some xterm-direct implementations). After
        // LeaveAlternateScreen, the user's entire terminal history was
        // gone — a far worse bug than a brief rain residue flash.
        //
        // LeaveAlternateScreen alone properly restores the main screen
        // buffer. The alternate screen content (including any rain residue)
        // is swapped out and becomes invisible. No pre-clear is needed.
        if self.alternate_screen_enabled {
            let _ = self
                .stdout
                .execute(crossterm_terminal::LeaveAlternateScreen);
            self.alternate_screen_enabled = false;
        }
        // v31.1: explicitly disable synchronized output (ESC[?2026l).
        // Each frame ends with SYNC_END, but if the last write failed or
        // was partial, sync mode could be stuck on — causing the terminal
        // to buffer all output invisibly after LeaveAlternateScreen.
        // TERMINAL_RESTORE_SEQUENCE includes this, but cleanup_terminal()
        // doesn't use that sequence. Belt-and-suspenders: always emit it.
        if self.term_caps.sync_output {
            let _ = self
                .stdout
                .write_all(crate::termdetect::SYNC_END);
            let _ = self.stdout.flush();
        }
        if self.raw_mode_enabled {
            let _ = crossterm_terminal::disable_raw_mode();
            self.raw_mode_enabled = false;
        }
        let _ = self.stdout.flush();
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        // Spawn a force-exit timer in case flush blocks.
        // The flag is set to `true` after flush completes; if the watchdog
        // sees the flag it skips `process::exit`, allowing normal shutdown
        // and SIGCONT recovery to proceed without being killed.
        //
        // The thread detaches and checks the flag after the timeout; if
        // shutdown already completed it simply returns without doing anything.
        let done = self.shutdown_complete.clone();
        let _ = std::thread::Builder::new()
            .name("cx-shutdown-guard".to_string())
            .spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(SHUTDOWN_TIMEOUT_SECS));
                if !done.load(std::sync::atomic::Ordering::Acquire) {
                    std::process::exit(0);
                }
            });

        // v16 audit: If the panic hook already restored the terminal
        // (TERMINAL_RESTORED_BY_PANIC is set), skip cleanup_terminal().
        // This prevents the BufWriter's pending frame data from being
        // flushed to the MAIN screen (after LeaveAlternateScreen), which
        // would leak partially-rendered rain onto the user's terminal.
        // The panic hook already called restore_terminal_best_effort(),
        // so the terminal is in a clean state.
        if !TERMINAL_RESTORED_BY_PANIC.load(std::sync::atomic::Ordering::Acquire) {
            self.cleanup_terminal();
        }
        self.shutdown_complete
            .store(true, std::sync::atomic::Ordering::Release);
    }
}

#[cold]
pub(crate) fn restore_terminal_best_effort() {
    let mut out = stdout();
    let _ = out.execute(event::DisableMouseCapture);
    let _ = out.execute(event::DisableFocusChange);
    let _ = out.execute(event::DisableBracketedPaste);
    let _ = out.write_all(TERMINAL_RESTORE_SEQUENCE.as_bytes());
    let _ = out.execute(SetAttribute(Attribute::Reset));
    let _ = out.execute(ResetColor);
    let _ = out.execute(cursor::Show);
    let _ = out.execute(crossterm_terminal::EnableLineWrap);
    let _ = out.execute(crossterm_terminal::LeaveAlternateScreen);
    let _ = crossterm_terminal::disable_raw_mode();
    let _ = out.flush();
}

/// Best-effort terminal restore sequence.
///
/// Disables all optional terminal modes that cosmostrix may have enabled:
/// - Mouse reporting (1000, 1002, 1003, 1006, 1015)
/// - Bracketed paste (2004)
/// - Focus events (1004)
/// - Alternate screen (1049)
/// - Synchronized output (2026) — added v15, prevents stuck sync mode
/// - Cursor hide (25 → show)
/// - SGR reset (0m)
///
/// Also resets:
/// - Scroll region to full screen (`\x1b[r`) — in case cosmostrix set one
/// - Character set to US ASCII (`\x1b(B`) — in case cosmostrix changed it
/// - Auto-wrap enabled (`\x1b[?7h`) — in case it was disabled
///
/// Does NOT clear screen or scrollback — that's the destructive
/// TERMINAL_RESET_SEQUENCE used only by --reset-terminal.
pub(crate) const TERMINAL_RESTORE_SEQUENCE: &str = "\x1b[0m\
     \x1b[?2026l\
     \x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l\x1b[?1015l\
     \x1b[?2004l\
     \x1b[?1004l\
     \x1b[?1049l\
     \x1b[r\
     \x1b(B\
     \x1b[?7h\
     \x1b[?25h\
     \x1b[0m";

/// Destructive terminal reset sequence (used by --reset-terminal).
///
/// Includes everything in TERMINAL_RESTORE_SEQUENCE plus:
/// - Cursor home (`\x1b[H`)
/// - Clear screen (`\x1b[2J`)
/// - Clear scrollback (`\x1b[3J`)
/// - Cursor home again (after clear)
///
/// This is the "nuclear option" — it wipes the visible screen AND the
/// scrollback buffer. Use only when the terminal is in a broken state
/// (e.g., after SIGKILL left cosmostrix's alternate screen + raw mode
/// active and the user can't see their shell prompt).
pub(crate) const TERMINAL_RESET_SEQUENCE: &str = "\x1b[0m\
     \x1b[?2026l\
     \x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l\x1b[?1015l\
     \x1b[?2004l\
     \x1b[?1004l\
     \x1b[?1049l\
     \x1b[r\
     \x1b(B\
     \x1b[?7h\
     \x1b[?25h\
     \x1b[H\x1b[2J\x1b[3J\x1b[H\
     \x1b[0m";

/// Emergency terminal reset — the "nuclear option" for broken terminals.
///
/// Called by `--reset-terminal`. This is the most aggressive recovery
/// path, used when cosmostrix (or any terminal app) was killed with
/// SIGKILL and left the terminal in a broken state:
/// - Alternate screen still active (rain visible, shell hidden)
/// - Raw mode still on (no echo, can't type commands)
/// - Mouse reporting still on (clicks do weird things)
/// - Cursor hidden
/// - Synchronized output stuck (terminal buffers output)
///
/// Recovery sequence (defense-in-depth, 5 layers):
///
/// 1. **ANSI restore sequence** — disables all optional modes
/// 2. **ANSI reset sequence** — clears screen + scrollback + cursor home
/// 3. **crossterm commands** — LeaveAlternateScreen, Clear, Show cursor
/// 4. **stty sane** — restores terminal line discipline (raw mode off)
/// 5. **reset** — external utility that does a full terminal reset
///    (clears screen, resets modes, restores tabs, etc.)
///
/// Each layer is best-effort — failures are silently ignored because
/// the terminal may be in a state where some operations don't work.
/// The goal is maximum recovery probability, not perfection.
pub(crate) fn reset_terminal_emergency() {
    // Layer 1: ANSI restore sequence (disable all optional modes)
    restore_terminal_best_effort();

    let mut out = stdout();

    // Layer 2: ANSI reset sequence (clear screen + scrollback)
    let _ = out.write_all(TERMINAL_RESET_SEQUENCE.as_bytes());
    let _ = out.flush();

    // Layer 3: crossterm commands (redundant with ANSI but belt-and-suspenders)
    let _ = out.execute(SetAttribute(Attribute::Reset));
    let _ = out.execute(ResetColor);
    let _ = out.execute(cursor::Show);
    let _ = out.execute(crossterm_terminal::LeaveAlternateScreen);
    let _ = out.execute(cursor::MoveTo(0, 0));
    let _ = out.execute(crossterm_terminal::Clear(
        crossterm_terminal::ClearType::All,
    ));
    let _ = out.execute(crossterm_terminal::Clear(
        crossterm_terminal::ClearType::Purge,
    ));
    let _ = out.execute(cursor::MoveTo(0, 0));
    let _ = out.execute(crossterm_terminal::EnableLineWrap);
    let _ = out.flush();

    // Layer 4+5: external utilities (Unix only)
    #[cfg(unix)]
    {
        if stdin().is_terminal() || stdout().is_terminal() {
            // stty sane: restores terminal line discipline (raw mode off,
            // echo on, canonical mode on, etc.). This is critical after
            // SIGKILL because raw mode persists in the kernel's termios
            // state — ANSI escapes alone cannot fix it.
            let _ = Command::new("stty").arg("sane").status();

            // reset: full external terminal reset utility. Clears screen,
            // resets modes, restores tab stops, sends terminal init string.
            // May not exist on all systems (embedded, minimal containers)
            // — failure is silently ignored.
            let _ = Command::new("reset").status();

            // tput reset: alternative if 'reset' not available. Uses
            // terminfo database to send the appropriate reset sequences.
            // Some systems have tput but not reset.
            let _ = Command::new("tput").arg("reset").status();
        }
    }
}

#[must_use]
pub(crate) fn blank_cell(bg: Option<Color>) -> Cell {
    Cell {
        ch: ' ',
        fg: None,
        bg,
        bold: false,
    }
}

// ── P3: stdout /dev/tty fallback helpers ─────────────────────────────────────
//
// v30: extracted to `terminal_tty.rs` to keep this file under the 1500-LOC
// guard. Re-exported here so existing call sites in this file (`recover_to_tty`
// at line ~402, ~411) keep working without a path change. External callers
// (event_loop.rs) already use `crate::terminal::is_terminal_gone` — that path
// still resolves via this re-export.
// `is_terminal_gone` is cross-platform (used by event_loop.rs + intro drain).
// `is_recoverable_io_error` and `open_tty_fallback` are Unix-only (gated in
// terminal_tty.rs) — only re-export them on Unix so Windows compiles cleanly.
pub(crate) use crate::terminal_tty::is_terminal_gone;
#[cfg(unix)]
pub(crate) use crate::terminal_tty::{is_recoverable_io_error, open_tty_fallback};
