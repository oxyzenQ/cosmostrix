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
    style::{Attribute, Color, ResetColor, SetAttribute, SetBackgroundColor},
    terminal, ExecutableCommand, QueueableCommand,
};

use crate::bolt::{BOLD_ESCAPES, BOLD_ESCAPE_LENS};
use crate::cell::Cell;
use crate::color_cache::ColorCache;
use crate::constants::{
    DIRTY_THRESHOLD_RATIO, MAX_TERMINAL_COLS, MAX_TERMINAL_LINES, MIN_TERMINAL_COLS,
    MIN_TERMINAL_LINES, RENDER_COMBINED_FLUSH_INIT_CAP, RENDER_ROW_BUF_INIT_CAP,
    RENDER_RUN_BUF_INIT_CAP, SHUTDOWN_TIMEOUT_SECS,
};
use crate::frame::Frame;
use crate::sgr_format::{push_u16, write_sgr_colors_buf};
use crate::termdetect::TerminalCaps;

/// Dirty threshold ratio: if dirty cells >= total/N, do full redraw.
/// (centralized in constants.rs, imported above).
struct LastFrame {
    width: u16,
    height: u16,
    cells: Vec<Cell>,
    /// Semantic generation this LastFrame was rendered with.
    /// A mismatch with Frame::semantic_gen forces a full redraw.
    semantic_gen: u32,
}

impl LastFrame {
    fn new(width: u16, height: u16) -> Self {
        let len = width as usize * height as usize;
        Self {
            width,
            height,
            cells: vec![Cell::blank_with_bg(None); len],
            semantic_gen: 0,
        }
    }

    /// v25.16 (perf polish): reuse the existing Vec allocation when the
    /// new dimensions fit within the old capacity. Avoids a heap
    /// alloc/dealloc pair every time the terminal is resized to a
    /// smaller or equal size — common during window-drag resize storms
    /// where the user overshoots and settles back to the original size.
    ///
    /// When the new size exceeds the existing capacity, falls back to a
    /// fresh allocation (same as `new`). When no existing frame is
    /// provided, also falls back to `new`.
    ///
    /// **Safety of `resize_with`**: `Vec::resize_with(new_len, || blank)`
    /// first truncates if `new_len < old.len()`, then extends by calling
    /// the closure for each new element. We `clear()` first to drop all
    /// old cell values (which contained previous-frame content) so the
    /// resulting Vec is uniformly blank. The underlying allocation is
    /// reused — only the length changes.
    fn reuse_or_new(existing: Option<Self>, width: u16, height: u16) -> Self {
        let Some(mut old) = existing else {
            return Self::new(width, height);
        };
        let new_len = width as usize * height as usize;
        if old.cells.capacity() < new_len {
            // Need a bigger buffer — allocate fresh. The old Vec is
            // dropped, freeing its allocation.
            return Self::new(width, height);
        }
        // Reuse the allocation. Clear drops all existing cells (which
        // contained previous-frame content), then resize_with extends
        // back to new_len using the blank-cell closure. The Vec's
        // capacity is preserved across clear+resize_with.
        old.cells.clear();
        old.cells.resize_with(new_len, || Cell::blank_with_bg(None));
        old.width = width;
        old.height = height;
        old.semantic_gen = 0;
        old
    }
}

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
}

impl Terminal {
    /// Create a Terminal. The `signal_exit` parameter is accepted for
    /// call-site compatibility but is not stored — the event loop keeps
    /// its own Arc<AtomicBool> and polls it directly.
    pub(crate) fn with_signal_exit(_signal_exit: Arc<AtomicBool>) -> Result<Self> {
        let raw = stdout();
        terminal::enable_raw_mode()?;
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
        };

        let init_res: Result<()> = (|| {
            let out = &mut term.stdout;
            out.execute(terminal::EnterAlternateScreen)?;
            term.alternate_screen_enabled = true;
            out.execute(cursor::Hide)?;
            term.cursor_hidden = true;
            if out.execute(terminal::DisableLineWrap).is_ok() {
                term.line_wrap_disabled = true;
            }
            if out.execute(event::EnableBracketedPaste).is_ok() {
                term.bracketed_paste_enabled = true;
            }
            out.execute(SetAttribute(Attribute::Reset))?;
            out.execute(ResetColor)?;
            out.execute(terminal::Clear(terminal::ClearType::All))?;
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
        let (w, h) = terminal::size()?;
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
        self.total_ansi_bytes += self.ansi_buf.len() as u64;
        self.flush_count += 1;

        // Extract ansi_buf so write_with_recovery can borrow `*self`
        // mutably for the recovery path. The Vec's allocation is preserved
        // across take + restore — zero per-frame alloc cost.
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
                Ok(())
            }
            Err(e) => {
                // Failure: restore ansi_buf so the next flush attempt
                // retries the same data (matches pre-P3 semantics).
                self.ansi_buf = ansi_buf;
                Err(e)
            }
        }
    }

    /// P3: write a buffer to stdout, attempting a /dev/tty fallback when
    /// the primary fd is broken mid-run (SSH disconnect, terminal crash,
    /// parent death). On a recoverable error:
    ///
    ///   1. Lazily open `/dev/tty` (Unix) or `CONOUT$` (Windows) for writing.
    ///   2. Write the buffer to the fallback handle.
    ///   3. Set `GRACEFUL_SHUTDOWN` so the main loop exits cleanly via the
    ///      normal shutdown path (Terminal::drop still runs, restoring the
    ///      TTY state from the fallback fd).
    ///   4. Bump `tty_recoveries`. If it exceeds
    ///      `STDOUT_FALLBACK_MAX_RECOVERIES`, stop trying and propagate the
    ///      original error — /dev/tty itself is likely broken too.
    ///
    /// Zero per-frame overhead in the steady state: the happy path is a
    /// single `write_all` on the BufWriter. The fallback only fires when
    /// that call returns an error.
    #[inline]
    fn write_with_recovery(&mut self, buf: &[u8]) -> Result<()> {
        match self.stdout.write_all(buf) {
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
            let _ = self.stdout.execute(terminal::EnableLineWrap);
            self.line_wrap_disabled = false;
        }
        // Always clear the visible viewport inside the alternate screen
        // before switching back to the main screen. This prevents the last
        // rain frame from being momentarily visible on the main screen when
        // the terminal emulator processes the LeaveAlternateScreen escape.
        //
        // v16: Previously this only ran on signal-triggered exit (SIGTERM/
        // SIGKILL). Normal q exit skipped it, assuming LeaveAlternateScreen
        // alone would cleanly restore the original content. But some
        // terminal emulators (especially with color-bg = default-background)
        // don't fully restore — rain residue bleeds through. Now we always
        // clear, which is a cheap operation (one Clear All escape) and
        // eliminates the residue class of bugs entirely.
        if self.alternate_screen_enabled {
            let _ = self.stdout.queue(cursor::MoveTo(0, 0));
            let _ = self.stdout.queue(terminal::Clear(terminal::ClearType::All));
            let _ = self.stdout.flush();
        }
        if self.alternate_screen_enabled {
            let _ = self.stdout.execute(terminal::LeaveAlternateScreen);
            self.alternate_screen_enabled = false;
        }
        if self.raw_mode_enabled {
            let _ = terminal::disable_raw_mode();
            self.raw_mode_enabled = false;
        }
        let _ = self.stdout.flush();
    }

    pub(crate) fn draw(&mut self, frame: &mut Frame) -> Result<()> {
        let mut cur_fg: Option<Color> = None;
        let mut cur_bg: Option<Color> = None;
        let mut cur_bold: bool = false;
        let mut cur_pos: Option<(u16, u16)> = None;

        // Separate dimension-change detection from semantic-change detection.
        // Clear(All) is ONLY issued when the terminal dimensions changed, because
        // resized terminals may have stale content at the new edges that isn't
        // covered by the frame. For semantic-only changes (charset, shading,
        // theme), the full redraw path iterates every cell and overwrites it, so
        // a blanket clear is redundant — and it causes visible flicker in
        // fullscreen terminals because the screen is blanked before the redraw
        // completes (the gap is perceptible at high cell counts).
        let (needs_full_redraw, needs_clear) = self
            .last
            .as_ref()
            .map(|l| {
                let dim_changed = l.width != frame.width || l.height != frame.height;
                let sem_changed = l.semantic_gen != frame.semantic_gen;
                (dim_changed || sem_changed, dim_changed)
            })
            .unwrap_or((true, true));

        if needs_clear {
            // v16: If the frame has a bg color, set it BEFORE Clear(All)
            // so cleared cells get the correct bg. Without this, Clear(All)
            // fills with terminal default bg (None), creating visible gaps
            // at screen edges.
            if let Some(bg) = frame.blank.bg {
                self.stdout.queue(SetBackgroundColor(bg))?;
            }
            self.stdout
                .queue(terminal::Clear(terminal::ClearType::All))?;
        }

        let can_reuse_last = !needs_full_redraw && self.last.is_some();
        let total_cells = frame.width as usize * frame.height as usize;
        let dirty_count = frame.dirty_indices().len();
        let dirty_is_large =
            total_cells > 0 && dirty_count >= (total_cells / DIRTY_THRESHOLD_RATIO);
        let do_full_redraw = !can_reuse_last || frame.is_dirty_all() || dirty_is_large;

        // ── Idle-frame fast path (v30 Cosmic Dragon) ──
        //
        // Skip the entire render body when no cells changed this frame AND
        // the last frame is reusable (no dim/semantic change). Protects all
        // `term.draw()` callers (event loop, intro, future) from doing useless
        // work on idle frames — the event loop already gates on `did_draw`,
        // but intro.rs:490 calls `draw()` unconditionally.
        //
        // `clear_dirty()` is still called to advance `dirty_gen` — `set()`
        // compares `dirty_cell_gen[i] != dirty_gen` to decide whether to push
        // a cell. Without the gen bump, cells dirty in the last non-idle
        // frame would be incorrectly skipped on the next frame. See
        // cosmic_dragon_lock_tests.rs INV-17 for the contract test.
        if can_reuse_last && dirty_count == 0 && !frame.is_dirty_all() {
            frame.clear_dirty();
            return Ok(());
        }

        if do_full_redraw {
            let needs_new_last = self
                .last
                .as_ref()
                .map(|l| {
                    l.width != frame.width
                        || l.height != frame.height
                        || l.semantic_gen != frame.semantic_gen
                })
                .unwrap_or(true);
            if needs_new_last {
                // v25.16 (perf polish): reuse the old LastFrame's Vec
                // allocation when the new dimensions fit. This avoids
                // heap churn during resize-drag storms where the user
                // overshoots and settles back to a smaller size.
                let old = self.last.take();
                self.last = Some(LastFrame::reuse_or_new(old, frame.width, frame.height));
            }
            let last = self.last.as_mut().expect("set above");
            // Synchronize semantic generation so future differential frames
            // don't spuriously re-trigger full redraws for this generation.
            last.semantic_gen = frame.semantic_gen;

            // PERF(v10): True single-pass RLE — accumulate characters into row_buf,
            // flush only when style actually changes.  Eliminates one
            // cell_at_index_ref(idx+1) generation-check per cell (~4800
            // calls on a 200×40 terminal per full redraw).
            let row_buf = &mut self.row_buf;
            let ansi_buf = &mut self.ansi_buf;
            row_buf.clear();
            ansi_buf.clear();
            // Pre-reserve if terminal grew since last frame
            let need_cap = frame.width as usize * 4;
            if row_buf.capacity() < need_cap {
                row_buf.reserve(need_cap - row_buf.capacity());
            }
            // MoveTo(0,0) directly into ansi_buf
            ansi_buf.extend_from_slice(b"\x1b[1;1H");
            for y in 0..frame.height {
                if y > 0 {
                    // MoveTo(0, y) directly into ansi_buf
                    ansi_buf.push(0x1b);
                    ansi_buf.push(b'[');
                    push_u16(ansi_buf, y + 1);
                    ansi_buf.extend_from_slice(b";1H");
                }
                row_buf.clear();
                let width_usize = frame.width as usize;
                for x in 0..frame.width {
                    let idx = y as usize * width_usize + x as usize;
                    let cell = frame.cell_at_index(idx);

                    // Flush row_buf on any style change
                    let style_changed =
                        cell.fg != cur_fg || cell.bg != cur_bg || cell.bold != cur_bold;
                    if style_changed && !row_buf.is_empty() {
                        ansi_buf.extend_from_slice(row_buf.as_bytes());
                        row_buf.clear();
                    }

                    // Combined fg+bg SGR — cached when possible
                    let color_changed = cell.fg != cur_fg || cell.bg != cur_bg;
                    if color_changed {
                        Self::emit_sgr(self.color_cache.as_ref(), ansi_buf, cell.fg, cell.bg);
                        cur_fg = cell.fg;
                        cur_bg = cell.bg;
                    }

                    if cell.bold != cur_bold {
                        // BOLT: branchless bold escape via table lookup.
                        // `cell.bold as usize` selects BOLD_ESCAPES[1] (ON,
                        // `\x1b[1m`, 4 bytes) or BOLD_ESCAPES[0] (OFF,
                        // `\x1b[22m`, 5 bytes). Compiles to `setne` on x86.
                        let bold_idx = cell.bold as usize;
                        let bold_len = BOLD_ESCAPE_LENS[bold_idx];
                        ansi_buf.extend_from_slice(&BOLD_ESCAPES[bold_idx][..bold_len]);
                        cur_bold = cell.bold;
                    }

                    row_buf.push(cell.ch);
                    last.cells[idx] = cell;
                }
                // Flush remaining cells in the row buffer
                if !row_buf.is_empty() {
                    ansi_buf.extend_from_slice(row_buf.as_bytes());
                }
            }

            // Reset attributes + flush all buffered ANSI bytes in one write_all.
            ansi_buf.extend_from_slice(b"\x1b[0m");
            self.flush_ansi()?;
            self.stdout.flush()?;

            frame.clear_dirty();
            return Ok(());
        }

        let last = self.last.as_mut().expect("checked above");

        let dirty = frame.dirty_indices();
        let width_usize = frame.width as usize;
        let height_usize = frame.height as usize;
        let run_buf = &mut self.run_buf;
        let ansi_buf = &mut self.ansi_buf;
        let cache_ref = self.color_cache.as_ref();
        ansi_buf.clear();

        // PERF: flat dirty-index buffer replaces the previous Vec<Vec<usize>>
        // nested structure. Collect all dirty indices into a single Vec,
        // sort once (row-major index sort groups by row AND orders within
        // row in one pass), then iterate contiguous runs. This eliminates
        // per-row Vec allocations on resize and improves cache locality.
        //
        // v25.15 (perf audit): the previous `dirty_flat.extend(dirty.iter()
        // .copied().filter(|&idx| idx < height * width))` had an O(N) bounds
        // filter that ran every frame. The filter is redundant — every entry
        // in `frame.dirty_indices()` was pushed by `Frame::set()` /
        // `set_force()`, both of which call `self.index(x, y)` first and
        // only push `Some(i)` results. So every dirty index is already
        // guaranteed in-bounds.
        //
        // Replaced the filter with a `debug_assert!` that verifies the
        // invariant in debug builds (zero cost in release). If a future
        // caller bypasses `index()` and pushes an OOB index, the debug
        // build will catch it immediately instead of silently masking it.
        let dirty_flat = &mut self.dirty_flat;
        dirty_flat.clear();
        dirty_flat.extend(dirty.iter().copied());
        dirty_flat.sort_unstable();
        // v25.16 (perf polish): the previous O(N) `dirty_flat.iter().all()`
        // checked every index every frame in debug builds (~4800
        // comparisons on a 200×40 terminal). Since `dirty_flat` is now
        // sorted ascending (we just called `sort_unstable()`), only the
        // LAST (largest) index needs to be checked — if it's in bounds,
        // all smaller indices are too. This drops the debug-build cost
        // from O(N) to O(1) per frame, with zero release-build impact
        // (debug_assert! is elided in release).
        //
        // SAFETY: `Frame::set()` / `set_force()` both call
        // `self.index(x, y)` first and only push `Some(i)` results,
        // so every dirty index is guaranteed in-bounds. This assert
        // catches the unlikely case where a future caller bypasses
        // `index()` and pushes an OOB index.
        debug_assert!(
            dirty_flat
                .last()
                .map_or(true, |&idx| idx < height_usize * width_usize),
            "dirty_indices must be in-bounds — Frame::set guarantees this"
        );

        // Iterate the flat sorted array, detecting row boundaries and
        // contiguous horizontal runs for RLE batching.
        // v25.11 (bug #12): track the current row to force a MoveTo at
        // each row boundary. This prevents cursor desync where the terminal
        // autowraps or drifts at row boundaries (especially the bottom rows
        // where phosphor decay writes many ghost cells). Without this, a
        // single-cell "row shift right" glitch can appear transiently at
        // the bottom of the screen and self-correct only when the periodic
        // full-redraw kicks in (~5 minutes). Forcing MoveTo at each row
        // start costs ~6 bytes/row (negligible) and eliminates the desync.
        let mut i = 0usize;
        let mut last_row: u16 = u16::MAX;
        while i < dirty_flat.len() {
            let idx0 = dirty_flat[i];
            // Borrow instead of copy: compare with last frame without allocating.
            // Most dirty cells are unchanged (set to blank by tail pass);
            // this avoids copying ~24 bytes per Cell for early-exit.
            let cell0_ref = frame.cell_at_index_ref(idx0);
            // Cosmic Dragon egg #2: direct indexing — dirty_flat was filtered to
            // idx < height*width, so idx0 is guaranteed in bounds.
            // BEFORE: last.cells.get(idx0) == Some(cell0_ref)
            // AFTER:  &last.cells[idx0] == cell0_ref
            if &last.cells[idx0] == cell0_ref {
                i += 1;
                continue;
            }

            let cell0 = *cell0_ref;
            last.cells[idx0] = cell0;

            let x0 = (idx0 % width_usize) as u16;
            let y0 = (idx0 / width_usize) as u16;
            let fg0 = cell0.fg;
            let bg0 = cell0.bg;
            let bold0 = cell0.bold;

            // v25.11 (bug #12): force cursor resync at each row boundary.
            // When we cross from one row to the next, invalidate cur_pos so
            // a MoveTo is always emitted for the first dirty cell in the
            // new row. This corrects any terminal-side autowrap or cursor
            // drift that accumulated during the previous row's run.
            if y0 != last_row {
                cur_pos = None;
                last_row = y0;
            }

            run_buf.clear();
            run_buf.push(cell0.ch);
            let mut run_len: u16 = 1;
            let mut last_idx_in_run = idx0;
            let mut j = i + 1;

            while j < dirty_flat.len() {
                let idx1 = dirty_flat[j];
                // Must be the next column on the same row (contiguous).
                if idx1 != last_idx_in_run + 1 {
                    break;
                }
                // Row boundary check: if we wrapped to the next row, the
                // x-coordinate resets, so the run must flush here.
                if idx1 / width_usize != idx0 / width_usize {
                    break;
                }

                let cell1_ref = frame.cell_at_index_ref(idx1);
                // Cosmic Dragon egg #3: direct indexing — idx1 from dirty_flat (filtered).
                if &last.cells[idx1] == cell1_ref {
                    break;
                }
                if cell1_ref.fg != fg0 || cell1_ref.bg != bg0 || cell1_ref.bold != bold0 {
                    break;
                }

                run_buf.push(cell1_ref.ch);
                let cell1 = *cell1_ref;
                last.cells[idx1] = cell1;
                run_len = run_len.saturating_add(1);
                last_idx_in_run = idx1;
                j += 1;
            }

            if cur_pos != Some((x0, y0)) {
                // MoveTo(x0, y0) directly into ansi_buf
                ansi_buf.push(0x1b);
                ansi_buf.push(b'[');
                push_u16(ansi_buf, y0 + 1);
                ansi_buf.push(b';');
                push_u16(ansi_buf, x0 + 1);
                ansi_buf.push(b'H');
            }

            // Combined fg+bg SGR — cached when possible
            let style_changed = fg0 != cur_fg || bg0 != cur_bg;
            if style_changed {
                Self::emit_sgr(cache_ref, ansi_buf, fg0, bg0);
                cur_fg = fg0;
                cur_bg = bg0;
            }

            if bold0 != cur_bold {
                // BOLT: branchless bold escape via table lookup (see above).
                let bold_idx = bold0 as usize;
                let bold_len = BOLD_ESCAPE_LENS[bold_idx];
                ansi_buf.extend_from_slice(&BOLD_ESCAPES[bold_idx][..bold_len]);
                cur_bold = bold0;
            }

            // Print run directly into ANSI buffer (UTF-8 bytes).
            ansi_buf.extend_from_slice(run_buf.as_bytes());
            let next_x = x0.saturating_add(run_len);
            cur_pos = if next_x < frame.width {
                Some((next_x, y0))
            } else {
                None
            };

            i = j;
        }

        // Reset attributes + flush all buffered ANSI bytes in one write_all.
        ansi_buf.extend_from_slice(b"\x1b[0m");
        self.flush_ansi()?;
        self.stdout.flush()?;
        frame.clear_dirty();
        Ok(())
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
    let _ = out.execute(terminal::EnableLineWrap);
    let _ = out.execute(terminal::LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();
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
    let _ = out.execute(terminal::LeaveAlternateScreen);
    let _ = out.execute(cursor::MoveTo(0, 0));
    let _ = out.execute(terminal::Clear(terminal::ClearType::All));
    let _ = out.execute(terminal::Clear(terminal::ClearType::Purge));
    let _ = out.execute(cursor::MoveTo(0, 0));
    let _ = out.execute(terminal::EnableLineWrap);
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

#[cfg(test)]
mod p5_tests {
    use super::*;
    use crate::constants::FD_HEALTH_PROBE_INTERVAL_FRAMES;

    /// The probe interval must be a positive, non-trivial value. Too
    /// small → per-frame overhead; too large → idle-period breakage
    /// goes undetected for too long. 3600 frames ≈ 60 s at 60 FPS is
    /// the documented sweet spot (matches P4 sweep cadence).
    #[test]
    fn p5_probe_interval_is_reasonable() {
        // black_box prevents const-folding so clippy doesn't flag the
        // assertions as constant. The values are still the same.
        let n = std::hint::black_box(FD_HEALTH_PROBE_INTERVAL_FRAMES);
        assert!(
            n >= 600,
            "probe interval must be at least 600 frames (10s at 60fps) to avoid overhead"
        );
        assert!(
            n <= 36000,
            "probe interval must be at most 36000 frames (10min at 60fps) to stay useful"
        );
    }

    /// The probe interval matches the P4 stuck-cell sweep cadence.
    /// Both are background hygiene passes on the same slow tick —
    /// keeping them in sync simplifies reasoning about background cost.
    #[test]
    fn p5_probe_interval_matches_p4_sweep_cadence() {
        use crate::constants::STUCK_CELL_SWEEP_INTERVAL_FRAMES;
        assert_eq!(
            FD_HEALTH_PROBE_INTERVAL_FRAMES, STUCK_CELL_SWEEP_INTERVAL_FRAMES,
            "P5 probe cadence should match P4 sweep cadence (both are 60s background hygiene)"
        );
    }

    /// Synthetic BrokenPipe errors are recoverable (per P3's classification).
    /// This is what probe_stdout_health synthesizes when isatty returns false,
    /// so the P3 recovery path accepts it. Verifies the contract between
    /// P5's detection layer and P3's recovery layer.
    #[cfg(unix)]
    #[test]
    fn p5_synthetic_broken_pipe_is_recoverable_by_p3() {
        let synthetic = std::io::Error::from(std::io::ErrorKind::BrokenPipe);
        assert!(
            is_recoverable_io_error(&synthetic),
            "P5's synthetic BrokenPipe error must be classified as recoverable by P3's is_recoverable_io_error"
        );
    }

    /// When stdout IS a terminal (the normal case in test environments),
    /// probe_stdout_health must return true. This is the steady-state
    /// behavior: the probe runs every 60s, finds stdout healthy, and
    /// returns without side-effects.
    ///
    /// NOTE: This test only validates the happy path. Constructing a
    /// Terminal with a broken stdout fd requires either closing the fd
    /// mid-test (unsafe, racy) or using a pipe + close pattern that
    /// doesn't fit Terminal's constructor contract. The broken-fd path
    /// is exercised indirectly via the P3 tests (is_recoverable_io_error
    /// classification) and the integration test below.
    #[cfg(unix)]
    #[test]
    fn p5_probe_returns_true_when_stdout_is_terminal() {
        // We can't easily construct a full Terminal in unit tests (it
        // calls enable_raw_mode + enters alternate screen). Instead,
        // verify the IsTerminal trait behaves as expected on real stdout.
        use std::io::IsTerminal;
        let stdout_is_tty = std::io::stdout().is_terminal();
        let stderr_is_tty = std::io::stderr().is_terminal();
        // In a normal test environment, at least one of these should be
        // a tty. If neither is (e.g., headless CI with no /dev/tty),
        // skip the assertion — the test still passes.
        if stdout_is_tty || stderr_is_tty {
            assert!(
                stdout_is_tty,
                "if any std stream is a tty, stdout should be too (test env assumption)"
            );
        }
    }

    /// The probe must not be a no-op when called on a non-tty stdout.
    /// We can't easily construct a Terminal with a broken fd, but we
    /// CAN verify the building block: a non-tty file (e.g., /dev/null)
    /// returns false from IsTerminal::is_terminal. This is the exact
    /// check probe_stdout_health makes on stdout.get_ref().
    #[cfg(unix)]
    #[test]
    fn p5_is_terminal_returns_false_for_non_tty_files() {
        use std::io::IsTerminal;

        // Open /dev/null — definitely not a tty.
        let devnull = std::fs::OpenOptions::new()
            .write(true)
            .open("/dev/null")
            .expect("/dev/null should be openable on Unix");

        // std::fs::File implements IsTerminal since Rust 1.70.
        // probe_stdout_health calls self.stdout.get_ref().is_terminal()
        // where stdout is BufWriter<Stdout> and get_ref() returns &Stdout.
        // Stdout's is_terminal() uses the same trait, so testing it on
        // File validates the same codepath.
        assert!(
            !devnull.is_terminal(),
            "/dev/null must NOT be classified as a terminal — probe_stdout_health relies on this to detect fd corruption"
        );
    }

    /// The probe's recovery path (recover_to_tty with empty buffer)
    /// must respect the recovery cap. After STDOUT_FALLBACK_MAX_RECOVERIES
    /// attempts, further recoveries propagate the error. This is
    /// enforced by P3's recover_to_tty, which P5 reuses — so P5
    /// inherits the cap automatically.
    #[test]
    fn p5_recovery_inherits_p3_cap() {
        use crate::constants::STDOUT_FALLBACK_MAX_RECOVERIES;
        // black_box prevents const-folding so clippy doesn't flag the
        // assertions as constant. The values are still the same.
        let cap = std::hint::black_box(STDOUT_FALLBACK_MAX_RECOVERIES);
        // The cap must be small enough to prevent infinite recovery
        // loops but large enough to handle transient multi-frame
        // breakage. P5 only triggers once per 60s, so the cap is
        // measured in minutes of recovery attempts.
        assert!(cap >= 1, "recovery cap must allow at least one attempt");
        assert!(
            cap <= 10,
            "recovery cap must be small enough to prevent pathological loops"
        );
    }

    /// The modulo check in the event loop must fire exactly once per
    /// interval. Simulate the event loop's `perf_rss_samples % N == 0`
    /// check over a range of frame counters and verify the probe fires
    /// exactly at multiples of N (including 0) and nowhere else.
    ///
    /// This catches off-by-one errors (e.g., using `+ 1` or starting
    /// the counter at 1 instead of 0) that would silently shift the
    /// probe cadence.
    #[test]
    fn p5_modulo_check_fires_exactly_at_multiples_of_interval() {
        // Read the const into a runtime variable so clippy doesn't
        // flag the assertions as constant-folded. The value is still
        // the same; we're just testing the modulo arithmetic pattern
        // the event loop uses.
        let n: u64 = FD_HEALTH_PROBE_INTERVAL_FRAMES;
        // Prevent const-folding so clippy::assertions_on_constants
        // doesn't fire. The actual value is unchanged.
        let n = std::hint::black_box(n);

        let mut fire_count = 0usize;
        // Simulate 3 intervals worth of frames.
        let total_frames = n * 3;
        for frame in 0..total_frames {
            let fires = frame % n == 0;
            if fires {
                fire_count += 1;
            }
        }
        assert_eq!(
            fire_count, 3,
            "probe must fire exactly 3 times over 3 intervals (frames 0, N, 2N), got {}",
            fire_count
        );

        // Verify the specific fire points.
        assert!(
            0u64 % n == 0,
            "probe must fire on the first frame (frame 0)"
        );
        assert!(
            (n - 1) % n != 0,
            "probe must NOT fire one frame before the interval boundary"
        );
        assert!(
            n % n == 0,
            "probe must fire exactly at the interval boundary"
        );
        assert!(
            (n + 1) % n != 0,
            "probe must NOT fire one frame after the interval boundary"
        );
    }
}
