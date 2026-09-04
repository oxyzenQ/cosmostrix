// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Terminal abstraction layer for cosmostrix.
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

use std::io::{stdout, BufWriter, Result, Stdout, Write};
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
    terminal as crossterm_terminal, ExecutableCommand,
};

use crate::color_cache::ColorCache;
use crate::constants::{
    MAX_TERMINAL_COLS, MAX_TERMINAL_LINES, MIN_TERMINAL_COLS, MIN_TERMINAL_LINES,
    RENDER_COMBINED_FLUSH_INIT_CAP, RENDER_ROW_BUF_INIT_CAP, RENDER_RUN_BUF_INIT_CAP,
    SHUTDOWN_TIMEOUT_SECS,
};
use crate::sgr_format::write_sgr_colors_buf;
use crate::termdetect::TerminalCaps;
use crate::tier2::ByteWindow;

// ── dragon-fight split: sub-modules ──────────────────────────────────────
// Extracted from this file to keep mod.rs under the 800-LOC cap and isolate
// concerns. See each module's docs for its responsibility.
mod cleanup;
mod draw;
mod io_recovery;
mod last_frame;
#[cfg(test)]
#[path = "../../../../test/engine/cosmic_dragon_engine/terminal/p5_tests.rs"]
mod p5_tests;
mod restore;

// Newly relocated from src/ root (audit M4). Re-exported as `pub(crate)`
// so the 7 existing `crate::terminal_tty::Foo` / `crate::sgr_format::Foo` /
// `crate::tier2::Foo` call sites continue to resolve via the
// `pub(crate) use terminal::{...};` re-export in main.rs.
pub(crate) mod sgr_format;
pub(crate) mod terminal_tty;
pub(crate) mod tier2;

// v50.0.0-beta.7 LOC refactor: restore_terminal_best_effort +
// reset_terminal_emergency + blank_cell + TERMINAL_*_SEQUENCE constants
// extracted to restore.rs. Re-exported here so all existing call sites
// resolve unchanged. The constants are used externally via
// 'crate::terminal::TERMINAL_*' but not within mod.rs itself, hence
// the allow(unused_imports).
#[allow(unused_imports)]
pub(crate) use restore::{
    blank_cell, reset_terminal_emergency, restore_terminal_best_effort, TERMINAL_RESET_SEQUENCE,
    TERMINAL_RESTORE_SEQUENCE,
};

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
    /// True when the kitty keyboard protocol has been pushed (CSI >1u)
    /// and must be popped (CSI <1u) at cleanup. Set in `with_signal_exit`
    /// when `term_caps.kitty_keyboard` is true AND the push succeeds.
    /// The pop is emitted BEFORE LeaveAlternateScreen so the kitty
    /// protocol stack is unwound while still on the alt screen — same
    /// ordering pattern as SYNC_END before LeaveAlternateScreen (the
    /// kitty protocol is alt-screen-independent, but grouping it with
    /// the other alt-screen-scoped teardown steps keeps the cleanup
    /// invariants simple: "everything optional is disabled before we
    /// leave the alt screen").
    kitty_keyboard_enabled: bool,
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
    /// Latency of the last DRAWN frame's terminal writes, in nanoseconds:
    /// the content `write_all` (spillover syscalls for frames larger than
    /// the BufWriter capacity) plus the final `flush()` syscall
    /// (HUNT-23 — the flush is where a saturated PTY actually blocks).
    /// Read by the event loop to feed `perf_pressure` and the
    /// output-drain backoff when writes are slow (e.g., VSCode's xterm.js
    /// falling behind over long runs, or CPU-rendered terminals that
    /// cannot drain fullscreen ANSI rates). Zero until the first flush
    /// completes. Stale on frames that do not draw — the event loop's
    /// post-draw accounting gates the overshoot computation on `did_draw`.
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
    /// true when the most recent `flush_ansi` suppressed the flush
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
        // v50 scrollback fix: do NOT emit SYNC_START (\x1b[?2026h) here.
        //
        // This used to write SYNC_START directly to `out.get_ref()` (the
        // underlying Stdout) BEFORE EnterAlternateScreen was called below.
        // That meant \x1b[?2026h landed on the MAIN screen, not the alt
        // screen. Sync mode then stayed open on the main screen for the
        // entire session — closed only by the SYNC_END in cleanup_terminal(),
        // which ran AFTER LeaveAlternateScreen.
        //
        // On VTE-based terminals (GNOME Terminal, xfce4-terminal), Alacritty,
        // and some xterm configs, having \x1b[?2026l (sync end) arrive on the
        // main screen AFTER \x1b[?1049l (leave alt screen) causes the terminal
        // to apply the buffered "sync frame" to the main screen — which can
        // trigger scrollback destruction. The terminal interprets the
        // leave-alt-screen switch as a content update buffered by sync mode,
        // and "displaying" it wipes the scrollback.
        //
        // Each frame's flush_ansi() already wraps content in
        // SYNC_START + frame + SYNC_END on the ALT screen. No global
        // sync open is needed at init — the first frame's SYNC_START
        // handles it correctly on the alt screen.
        //
        // The matching fix in cleanup_terminal() moves SYNC_END to BEFORE
        // LeaveAlternateScreen, so sync mode is closed on the alt screen
        // (where it was opened by the last frame) before switching back
        // to the (untouched) main screen.
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
            kitty_keyboard_enabled: false,
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
            // Only enter alternate screen if the terminal supports it.
            // The Linux virtual console (TERM=linux) DOES support the
            // alternate screen buffer (\x1b[?1049h) via vt.c since kernel
            // 2.6.x — entering it saves the main screen state (incl.
            // scrollback), leaving it restores the main screen intact.
            // This is what preserves TTY history (e.g. `echo hello`) on
            // quit. Only `dumb` terminals and an unset TERM lack alt
            // screen support — on those, cosmostrix runs on the main
            // screen directly (scrollback is preserved by not clearing).
            if term.term_caps.has_alternate_screen {
                out.execute(crossterm_terminal::EnterAlternateScreen)?;
                term.alternate_screen_enabled = true;
            }
            out.execute(cursor::Hide)?;
            term.cursor_hidden = true;
            if out.execute(crossterm_terminal::DisableLineWrap).is_ok() {
                term.line_wrap_disabled = true;
            }
            if out.execute(event::EnableBracketedPaste).is_ok() {
                term.bracketed_paste_enabled = true;
            }
            // Kitty keyboard protocol: push DISAMBIGUATE_ESCAPE_CODES so
            // the terminal reports the FULL modifier bitfield on every
            // keypress (1=SHIFT, 2=ALT, 4=CONTROL, 8=SUPER, 16=HYPER,
            // 32=META). Without this, terminals fall back to legacy
            // escape sequences that ONLY encode SHIFT/ALT/CONTROL —
            // Super/Hyper/Meta are silently stripped, reaching cosmostrix
            // as `Char('c')` with `KeyModifiers::NONE`. That made Super+C
            // indistinguishable from bare 'c', bypassing the modifier
            // allowlist in `input.rs::is_unmodified_or_shift()`.
            //
            // Only pushed when `term_caps.kitty_keyboard` is true
            // (detected via KITTY_KEYBOARD_TERMINALS list — see
            // termdetect.rs). On terminals that don't support the
            // protocol, the push would emit literal `CSI >1u` characters
            // into the input stream, polluting it.
            //
            // The push is paired with a pop in `cleanup_terminal()`
            // before LeaveAlternateScreen. crossterm 0.29's
            // PushKeyboardEnhancementFlags returns Err(Unsupported) on
            // Windows — `let _ =` silently ignores that case.
            if term.term_caps.kitty_keyboard
                && out
                    .execute(event::PushKeyboardEnhancementFlags(
                        event::KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES,
                    ))
                    .is_ok()
            {
                term.kitty_keyboard_enabled = true;
            }
            out.execute(SetAttribute(Attribute::Reset))?;
            out.execute(ResetColor)?;
            // Scrollback-safe init: do NOT issue Clear(All) or fill every
            // cell of the alternate screen here. On VTE-based terminals
            // (GNOME Terminal, xfce4-terminal, etc.), any operation that
            // writes to the entire alternate screen buffer — including
            // \x1b[2J or a space-fill loop covering every cell — can set
            // an internal VTE flag that clears the main screen's scrollback
            // when LeaveAlternateScreen is later called. The first frame's
            // full redraw will paint every cell anyway, so no init clear is
            // needed. Skipping it entirely guarantees scrollback safety.
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

    /// true when the most recent `flush_ansi` suppressed the flush
    /// due to byte-budget backpressure. Used by the event loop to inject
    /// a synthetic `write_overshoot` so the self-healer fires. Reset on
    /// next successful write.
    #[must_use]
    pub(crate) fn last_flush_suppressed(&self) -> bool {
        self.last_flush_suppressed
    }

    /// v50.0.0-beta.6: access terminal caps for phosphor + speed tuning.
    /// Returns (phosphor_decay_mult, ghost_brightness_cap, speed_mult).
    #[must_use]
    pub(crate) fn phosphor_tuning(&self) -> (f32, f32, f32) {
        (
            self.term_caps.phosphor_decay_mult,
            self.term_caps.ghost_brightness_cap,
            self.term_caps.speed_mult,
        )
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

// ── P3: stdout /dev/tty fallback helpers ─────────────────────────────────────
//
// v30: extracted to `terminal_tty.rs` to keep this file under the 800-LOC
// guard. Re-exported here so existing call sites in this file (`recover_to_tty`
// at line ~402, ~411) keep working without a path change. External callers
// (event_loop.rs) already use `crate::terminal::is_terminal_gone` — that path
// still resolves via this re-export.
// `is_terminal_gone` is cross-platform (used by event_loop.rs + intro drain).
// `is_recoverable_io_error` and `open_tty_fallback` are Unix-only (gated in
// terminal_tty.rs) — also re-exported here so p5_tests.rs can use them via
// `use super::*` glob. The `recover_to_tty` method now lives in
// `io_recovery.rs` and imports them directly.
#[cfg(all(unix, test))]
pub(crate) use crate::terminal_tty::is_recoverable_io_error;
pub(crate) use crate::terminal_tty::is_terminal_gone;
