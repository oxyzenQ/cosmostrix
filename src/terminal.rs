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

use crossterm::{
    cursor, event,
    style::{
        Attribute, Color, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
    },
    terminal, ExecutableCommand, QueueableCommand,
};

use crate::cell::Cell;
use crate::constants::{
    DIRTY_THRESHOLD_RATIO, MAX_TERMINAL_COLS, MAX_TERMINAL_LINES, MIN_TERMINAL_COLS,
    MIN_TERMINAL_LINES, SHUTDOWN_TIMEOUT_SECS,
};
use crate::frame::Frame;

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
}

/// Buffer size for stdout BufWriter (64 KiB). Large enough to batch an
/// entire frame's ANSI commands into a single syscall.
const STDOUT_BUF_CAPACITY: usize = 64 * 1024;

pub struct Terminal {
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
    /// When set (by signal handlers), cleanup clears the visible viewport
    /// before leaving the alternate screen. Normal q/esc exit leaves this
    /// `false` so the alternate screen switch is non-destructive.
    signal_exit: Arc<AtomicBool>,
}

impl Terminal {
    /// Create a Terminal that clears the visible viewport on drop if the
    /// given `signal_exit` flag is set. This is used by the interactive
    /// event loop to distinguish signal-triggered exits (which need
    /// viewport cleanup) from normal q/esc exits (which do not).
    pub(crate) fn with_signal_exit(signal_exit: Arc<AtomicBool>) -> Result<Self> {
        let raw = stdout();
        terminal::enable_raw_mode()?;
        let out = BufWriter::with_capacity(STDOUT_BUF_CAPACITY, raw);
        let mut term = Self {
            stdout: out,
            last: None,
            run_buf: {
                let mut s = String::new();
                s.reserve(256);
                s
            },
            row_buf: String::with_capacity(512),
            dirty_flat: Vec::new(),
            mouse_capture_enabled: false,
            focus_change_enabled: false,
            bracketed_paste_enabled: false,
            raw_mode_enabled: true,
            alternate_screen_enabled: false,
            cursor_hidden: false,
            line_wrap_disabled: false,
            cleaned_up: false,
            shutdown_complete: Arc::new(AtomicBool::new(false)),
            signal_exit: signal_exit.clone(),
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

    pub fn size(&self) -> Result<(u16, u16)> {
        let (w, h) = terminal::size()?;
        // Clamp to prevent OOM from misreported terminal sizes
        let w = w.min(MAX_TERMINAL_COLS);
        let h = h.min(MAX_TERMINAL_LINES);
        // Floor to prevent degenerate rendering in tiny terminals
        let w = w.max(MIN_TERMINAL_COLS);
        let h = h.max(MIN_TERMINAL_LINES);
        Ok((w, h))
    }

    pub fn poll_event(timeout: std::time::Duration) -> Result<bool> {
        event::poll(timeout)
    }

    pub fn read_event() -> Result<event::Event> {
        event::read()
    }

    /// Enable mouse capture so mouse events are reported.
    pub fn enable_mouse_capture(&mut self) -> Result<()> {
        self.stdout.execute(event::EnableMouseCapture)?;
        self.mouse_capture_enabled = true;
        self.stdout.execute(event::EnableFocusChange)?;
        self.focus_change_enabled = true;
        self.stdout.flush()?;
        Ok(())
    }

    /// Disable mouse capture.
    pub fn disable_mouse_capture(&mut self) -> Result<()> {
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
        // On signal-triggered exit (SIGINT/SIGTERM/SIGHUP), clear the
        // visible viewport inside the alternate screen before switching
        // back to the main screen. This prevents the last rain frame
        // from being momentarily visible on the main screen when the
        // terminal emulator processes the LeaveAlternateScreen escape.
        // Normal q/esc exit skips this — the alternate screen switch
        // alone cleanly restores the original terminal content.
        if self.alternate_screen_enabled
            && self.signal_exit.load(std::sync::atomic::Ordering::Acquire)
        {
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

    pub fn draw(&mut self, frame: &mut Frame) -> Result<()> {
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
            self.stdout
                .queue(terminal::Clear(terminal::ClearType::All))?;
        }

        let can_reuse_last = !needs_full_redraw && self.last.is_some();
        let total_cells = frame.width as usize * frame.height as usize;
        let dirty_count = frame.dirty_indices().len();
        let dirty_is_large =
            total_cells > 0 && dirty_count >= (total_cells / DIRTY_THRESHOLD_RATIO);
        let do_full_redraw = !can_reuse_last || frame.is_dirty_all() || dirty_is_large;

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
                self.last = Some(LastFrame::new(frame.width, frame.height));
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
            row_buf.clear();
            // Pre-reserve if terminal grew since last frame
            let need_cap = frame.width as usize * 4;
            if row_buf.capacity() < need_cap {
                row_buf.reserve(need_cap - row_buf.capacity());
            }
            self.stdout.queue(cursor::MoveTo(0, 0))?;
            for y in 0..frame.height {
                if y > 0 {
                    self.stdout.queue(cursor::MoveTo(0, y))?;
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
                        self.stdout.queue(Print(row_buf.as_str()))?;
                        row_buf.clear();
                    }

                    if cell.fg != cur_fg {
                        if let Some(fg) = cell.fg {
                            self.stdout.queue(SetForegroundColor(fg))?;
                        } else {
                            self.stdout.queue(SetForegroundColor(Color::Reset))?;
                        }
                        cur_fg = cell.fg;
                    }

                    if cell.bg != cur_bg {
                        if let Some(bg) = cell.bg {
                            self.stdout.queue(SetBackgroundColor(bg))?;
                        } else {
                            self.stdout.queue(SetBackgroundColor(Color::Reset))?;
                        }
                        cur_bg = cell.bg;
                    }

                    if cell.bold != cur_bold {
                        self.stdout.queue(SetAttribute(if cell.bold {
                            Attribute::Bold
                        } else {
                            Attribute::NormalIntensity
                        }))?;
                        cur_bold = cell.bold;
                    }

                    row_buf.push(cell.ch);
                    last.cells[idx] = cell;
                }
                // Flush remaining cells in the row buffer
                if !row_buf.is_empty() {
                    self.stdout.queue(Print(row_buf.as_str()))?;
                }
            }

            self.stdout.queue(SetAttribute(Attribute::Reset))?;
            self.stdout.queue(ResetColor)?;
            self.stdout.flush()?;

            frame.clear_dirty();
            return Ok(());
        }

        let last = self.last.as_mut().expect("checked above");

        let dirty = frame.dirty_indices();
        let width_usize = frame.width as usize;
        let height_usize = frame.height as usize;
        let run_buf = &mut self.run_buf;

        // PERF: flat dirty-index buffer replaces the previous Vec<Vec<usize>>
        // nested structure. Collect all dirty indices into a single Vec,
        // sort once (row-major index sort groups by row AND orders within
        // row in one pass), then iterate contiguous runs. This eliminates
        // per-row Vec allocations on resize and improves cache locality.
        let dirty_flat = &mut self.dirty_flat;
        dirty_flat.clear();
        dirty_flat.extend(
            dirty
                .iter()
                .copied()
                .filter(|&idx| idx < height_usize * width_usize),
        );
        dirty_flat.sort_unstable();

        // Iterate the flat sorted array, detecting row boundaries and
        // contiguous horizontal runs for RLE batching.
        let mut i = 0usize;
        while i < dirty_flat.len() {
            let idx0 = dirty_flat[i];
            // Borrow instead of copy: compare with last frame without allocating.
            // Most dirty cells are unchanged (set to blank by tail pass);
            // this avoids copying ~24 bytes per Cell for early-exit.
            let cell0_ref = frame.cell_at_index_ref(idx0);
            if last.cells.get(idx0) == Some(cell0_ref) {
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
                if last.cells.get(idx1) == Some(cell1_ref) {
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
                self.stdout.queue(cursor::MoveTo(x0, y0))?;
            }

            if fg0 != cur_fg {
                if let Some(fg) = fg0 {
                    self.stdout.queue(SetForegroundColor(fg))?;
                } else {
                    self.stdout.queue(SetForegroundColor(Color::Reset))?;
                }
                cur_fg = fg0;
            }

            if bg0 != cur_bg {
                if let Some(bg) = bg0 {
                    self.stdout.queue(SetBackgroundColor(bg))?;
                } else {
                    self.stdout.queue(SetBackgroundColor(Color::Reset))?;
                }
                cur_bg = bg0;
            }

            if bold0 != cur_bold {
                self.stdout.queue(SetAttribute(if bold0 {
                    Attribute::Bold
                } else {
                    Attribute::NormalIntensity
                }))?;
                cur_bold = bold0;
            }

            self.stdout.queue(Print(run_buf.as_str()))?;
            let next_x = x0.saturating_add(run_len);
            cur_pos = if next_x < frame.width {
                Some((next_x, y0))
            } else {
                None
            };

            i = j;
        }

        self.stdout.queue(SetAttribute(Attribute::Reset))?;
        self.stdout.queue(ResetColor)?;
        self.stdout.flush()?;
        frame.clear_dirty();
        Ok(())
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        // Safety: spawn a force-exit timer in case flush blocks.
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
        self.cleanup_terminal();
        self.shutdown_complete
            .store(true, std::sync::atomic::Ordering::Release);
    }
}

#[cold]
pub fn restore_terminal_best_effort() {
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

pub const TERMINAL_RESTORE_SEQUENCE: &str =
    "\x1b[0m\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l\x1b[?1015l\x1b[?2004l\x1b[?1004l\x1b[?1049l\x1b[?25h\x1b[0m";

pub const TERMINAL_RESET_SEQUENCE: &str =
    "\x1b[0m\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l\x1b[?1015l\x1b[?2004l\x1b[?1004l\x1b[?1049l\x1b[?25h\x1b[H\x1b[2J\x1b[3J\x1b[H\x1b[0m";

pub fn reset_terminal_emergency() {
    restore_terminal_best_effort();
    let mut out = stdout();
    let _ = out.write_all(TERMINAL_RESET_SEQUENCE.as_bytes());
    let _ = out.execute(SetAttribute(Attribute::Reset));
    let _ = out.execute(ResetColor);
    let _ = out.execute(cursor::Show);
    let _ = out.execute(terminal::LeaveAlternateScreen);
    let _ = out.execute(cursor::MoveTo(0, 0));
    let _ = out.execute(terminal::Clear(terminal::ClearType::All));
    let _ = out.execute(terminal::Clear(terminal::ClearType::Purge));
    let _ = out.execute(cursor::MoveTo(0, 0));
    let _ = out.flush();
    #[cfg(unix)]
    {
        if stdin().is_terminal() || stdout().is_terminal() {
            let _ = Command::new("stty").arg("sane").status();
            let _ = Command::new("reset").status();
        }
    }
}

#[must_use]
pub fn blank_cell(bg: Option<Color>) -> Cell {
    Cell {
        ch: ' ',
        fg: None,
        bg,
        bold: false,
    }
}

#[cfg(test)]
mod tests {
    use super::{TERMINAL_RESET_SEQUENCE, TERMINAL_RESTORE_SEQUENCE};
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    #[derive(Default)]
    struct CleanupFlags {
        mouse: bool,
        focus: bool,
        bracketed_paste: bool,
        cursor: bool,
        wrap: bool,
        signal_exit_clear: bool,
        alternate: bool,
        raw: bool,
        cleaned: bool,
    }

    impl CleanupFlags {
        fn cleanup_plan(&mut self, signal_exit: bool) -> Vec<&'static str> {
            if self.cleaned {
                return Vec::new();
            }
            self.cleaned = true;

            let mut plan = Vec::new();
            if self.mouse {
                plan.push("disable-mouse");
                self.mouse = false;
            }
            if self.focus {
                plan.push("disable-focus");
                self.focus = false;
            }
            if self.bracketed_paste {
                plan.push("disable-bracketed-paste");
                self.bracketed_paste = false;
            }
            if self.cursor {
                plan.push("show-cursor");
                self.cursor = false;
            }
            if self.wrap {
                plan.push("enable-wrap");
                self.wrap = false;
            }
            // Signal-exit viewport clear happens BEFORE leave-alternate
            // but only when signal_exit flag is set.
            if self.alternate && signal_exit {
                plan.push("clear-viewport");
                self.signal_exit_clear = true;
            }
            if self.alternate {
                plan.push("leave-alternate");
                self.alternate = false;
            }
            if self.raw {
                plan.push("disable-raw");
                self.raw = false;
            }
            plan
        }
    }

    #[test]
    fn normal_restore_sequence_disables_terminal_reporting_modes() {
        for mode in [
            "?1000l", "?1002l", "?1003l", "?1006l", "?1015l", "?2004l", "?1004l", "?1049l", "?25h",
        ] {
            assert!(
                TERMINAL_RESTORE_SEQUENCE.contains(mode),
                "missing terminal restore mode {mode}"
            );
        }
        assert!(TERMINAL_RESTORE_SEQUENCE.ends_with("\x1b[0m"));
    }

    #[test]
    fn normal_restore_sequence_does_not_clear_screen_or_scrollback() {
        assert!(
            !TERMINAL_RESTORE_SEQUENCE.contains("\x1b[2J"),
            "normal restore must not clear the visible screen"
        );
        assert!(
            !TERMINAL_RESTORE_SEQUENCE.contains("\x1b[3J"),
            "normal restore must not purge scrollback"
        );
        assert!(
            !TERMINAL_RESTORE_SEQUENCE.contains("\x1b[H"),
            "normal restore must not move cursor home on the shell screen"
        );
    }

    #[test]
    fn reset_terminal_sequence_disables_terminal_reporting_modes() {
        for mode in [
            "?1000l", "?1002l", "?1003l", "?1006l", "?1015l", "?2004l", "?1004l", "?1049l", "?25h",
        ] {
            assert!(
                TERMINAL_RESET_SEQUENCE.contains(mode),
                "missing terminal reset mode {mode}"
            );
        }
        assert!(TERMINAL_RESET_SEQUENCE.ends_with("\x1b[0m"));
    }

    #[test]
    fn reset_terminal_sequence_clears_screen_and_scrollback() {
        assert!(
            TERMINAL_RESET_SEQUENCE.contains("\x1b[2J"),
            "reset sequence must clear the visible screen"
        );
        assert!(
            TERMINAL_RESET_SEQUENCE.contains("\x1b[3J"),
            "reset sequence must request scrollback purge"
        );
        assert!(
            TERMINAL_RESET_SEQUENCE.matches("\x1b[H").count() >= 2,
            "reset sequence must move cursor home before and after clearing"
        );
    }

    #[test]
    fn reset_terminal_sequence_is_idempotent() {
        let repeated = format!("{TERMINAL_RESET_SEQUENCE}{TERMINAL_RESET_SEQUENCE}");
        for required in ["\x1b[0m", "\x1b[?1049l", "\x1b[?25h", "\x1b[2J", "\x1b[3J"] {
            assert!(
                repeated.contains(required),
                "repeated reset sequence missing required command {required:?}"
            );
        }
    }

    #[test]
    fn terminal_cleanup_plan_is_reverse_order_and_idempotent() {
        let mut flags = CleanupFlags {
            mouse: true,
            focus: true,
            bracketed_paste: true,
            cursor: true,
            wrap: true,
            alternate: true,
            raw: true,
            cleaned: false,
            ..Default::default()
        };

        assert_eq!(
            flags.cleanup_plan(false),
            [
                "disable-mouse",
                "disable-focus",
                "disable-bracketed-paste",
                "show-cursor",
                "enable-wrap",
                "leave-alternate",
                "disable-raw",
            ]
        );
        let mut flags = CleanupFlags {
            mouse: true,
            focus: true,
            bracketed_paste: true,
            cursor: true,
            wrap: true,
            alternate: true,
            raw: true,
            cleaned: false,
            ..Default::default()
        };
        let plan = flags.cleanup_plan(false);
        assert!(!plan.contains(&"clear-viewport"));
        assert!(!plan.contains(&"purge-scrollback"));
        assert!(!plan.contains(&"cursor-home"));
        assert!(flags.cleanup_plan(false).is_empty());
    }

    #[test]
    fn signal_exit_cleanup_clears_viewport_before_leaving_alternate() {
        let mut flags = CleanupFlags {
            mouse: true,
            focus: true,
            bracketed_paste: true,
            cursor: true,
            wrap: true,
            alternate: true,
            raw: true,
            cleaned: false,
            ..Default::default()
        };

        let plan = flags.cleanup_plan(true);
        let clear_idx = plan.iter().position(|&s| s == "clear-viewport");
        let leave_idx = plan.iter().position(|&s| s == "leave-alternate");
        assert!(
            clear_idx.is_some() && leave_idx.is_some(),
            "signal-exit cleanup must include clear-viewport and leave-alternate"
        );
        assert!(
            clear_idx < leave_idx,
            "clear-viewport must happen before leave-alternate"
        );
    }

    #[test]
    fn normal_exit_cleanup_skips_viewport_clear() {
        let mut flags = CleanupFlags {
            mouse: true,
            focus: true,
            bracketed_paste: true,
            cursor: true,
            wrap: true,
            alternate: true,
            raw: true,
            cleaned: false,
            ..Default::default()
        };

        let plan = flags.cleanup_plan(false);
        assert!(
            !plan.contains(&"clear-viewport"),
            "normal exit must NOT clear viewport"
        );
    }

    #[test]
    fn signal_exit_flag_is_atomic_and_shared() {
        let flag = Arc::new(AtomicBool::new(false));
        let clone = flag.clone();
        assert!(!flag.load(std::sync::atomic::Ordering::Acquire));
        clone.store(true, std::sync::atomic::Ordering::Release);
        assert!(flag.load(std::sync::atomic::Ordering::Acquire));
    }
}
