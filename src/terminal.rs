// Copyright (c) 2026 rezky_nightky

use std::io::{stdout, Result, Stdout, Write};
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
use crate::constants::{DIRTY_THRESHOLD_RATIO, MAX_TERMINAL_COLS, MAX_TERMINAL_LINES, SHUTDOWN_TIMEOUT_SECS};
use crate::frame::Frame;

/// Dirty threshold ratio: if dirty cells >= total/N, do full redraw.
/// (centralized in constants.rs, imported above)
struct LastFrame {
    width: u16,
    height: u16,
    cells: Vec<Cell>,
}

impl LastFrame {
    fn new(width: u16, height: u16) -> Self {
        let len = width as usize * height as usize;
        Self {
            width,
            height,
            cells: vec![
                crate::cell::Cell {
                    ch: ' ',
                    fg: None,
                    bg: None,
                    bold: false,
                };
                len
            ],
        }
    }
}

pub struct Terminal {
    stdout: Stdout,
    last: Option<LastFrame>,
    run_buf: String,
    /// Reusable buffer for full-redraw row batching (avoids per-frame allocation).
    row_buf: String,
    row_dirty: Vec<Vec<usize>>,
    touched_rows: Vec<u16>,
    mouse_capture_enabled: bool,
    /// Set to `true` after flush completes; the force-exit watchdog checks
    /// this and skips `process::exit` when cleanup finished normally.
    shutdown_complete: Arc<AtomicBool>,
}

impl Terminal {
    pub fn new() -> Result<Self> {
        let mut out = stdout();
        terminal::enable_raw_mode()?;
        let init_res: Result<()> = (|| {
            out.execute(terminal::EnterAlternateScreen)?;
            out.execute(cursor::Hide)?;
            let _ = out.execute(terminal::DisableLineWrap);
            out.execute(SetAttribute(Attribute::Reset))?;
            out.execute(ResetColor)?;
            out.execute(terminal::Clear(terminal::ClearType::All))?;
            out.flush()?;
            Ok(())
        })();
        if let Err(e) = init_res {
            let _ = out.execute(SetAttribute(Attribute::Reset));
            let _ = out.execute(ResetColor);
            let _ = out.execute(cursor::Show);
            let _ = out.execute(terminal::EnableLineWrap);
            let _ = out.execute(terminal::LeaveAlternateScreen);
            let _ = terminal::disable_raw_mode();
            let _ = out.flush();
            return Err(e);
        }
        Ok(Self {
            stdout: out,
            last: None,
            run_buf: {
                let mut s = String::new();
                s.reserve(64);
                s
            },
            row_buf: String::with_capacity(512),
            row_dirty: Vec::new(),
            touched_rows: Vec::new(),
            mouse_capture_enabled: false,
            shutdown_complete: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn size(&self) -> Result<(u16, u16)> {
        let (w, h) = terminal::size()?;
        // Clamp to prevent OOM from misreported terminal sizes
        let w = w.min(MAX_TERMINAL_COLS);
        let h = h.min(MAX_TERMINAL_LINES);
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
        self.stdout.flush()?;
        self.mouse_capture_enabled = true;
        Ok(())
    }

    /// Disable mouse capture.
    pub fn disable_mouse_capture(&mut self) -> Result<()> {
        if self.mouse_capture_enabled {
            self.stdout.execute(event::DisableMouseCapture)?;
            self.stdout.flush()?;
            self.mouse_capture_enabled = false;
            // Keep the global signal-handler flag in sync so that signal
            // handlers don't issue a redundant DisableMouseCapture later.
            crate::interactive::clear_mouse_capture_flag();
        }
        Ok(())
    }

    pub fn draw(&mut self, frame: &mut Frame) -> Result<()> {
        let mut cur_fg: Option<Color> = None;
        let mut cur_bg: Option<Color> = None;
        let mut cur_bold: bool = false;
        let mut cur_pos: Option<(u16, u16)> = None;

        let needs_full_redraw = self
            .last
            .as_ref()
            .map(|l| l.width != frame.width || l.height != frame.height)
            .unwrap_or(true);

        if needs_full_redraw {
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
                .map(|l| l.width != frame.width || l.height != frame.height)
                .unwrap_or(true);
            if needs_new_last {
                self.last = Some(LastFrame::new(frame.width, frame.height));
            }
            let last = self.last.as_mut().expect("set above");

            // Reuse the persistent row_buf to avoid per-frame allocation
            let row_buf = &mut self.row_buf;
            row_buf.clear();
            // Pre-reserve if terminal grew since last frame
            let need_cap = frame.width as usize * 4;
            if row_buf.capacity() < need_cap {
                row_buf.reserve(need_cap - row_buf.capacity());
            }
            for y in 0..frame.height {
                self.stdout.queue(cursor::MoveTo(0, y))?;
                row_buf.clear();
                let width_usize = frame.width as usize;
                for x in 0..frame.width {
                    let idx = y as usize * width_usize + x as usize;
                    let cell = frame.cell_at_index(idx);

                    // Peek ahead: if next cell has different style, flush buffer first
                    let next_differs = (x + 1 >= frame.width) || {
                        let next_cell = frame.cell_at_index(idx + 1);
                        next_cell.fg != cell.fg
                            || next_cell.bg != cell.bg
                            || next_cell.bold != cell.bold
                    };

                    if cell.fg != cur_fg {
                        if !row_buf.is_empty() {
                            self.stdout.queue(Print(row_buf.as_str()))?;
                            row_buf.clear();
                        }
                        if let Some(fg) = cell.fg {
                            self.stdout.queue(SetForegroundColor(fg))?;
                        } else {
                            self.stdout.queue(SetForegroundColor(Color::Reset))?;
                        }
                        cur_fg = cell.fg;
                    }

                    if cell.bg != cur_bg {
                        if !row_buf.is_empty() {
                            self.stdout.queue(Print(row_buf.as_str()))?;
                            row_buf.clear();
                        }
                        if let Some(bg) = cell.bg {
                            self.stdout.queue(SetBackgroundColor(bg))?;
                        } else {
                            self.stdout.queue(SetBackgroundColor(Color::Reset))?;
                        }
                        cur_bg = cell.bg;
                    }

                    if cell.bold != cur_bold {
                        if !row_buf.is_empty() {
                            self.stdout.queue(Print(row_buf.as_str()))?;
                            row_buf.clear();
                        }
                        self.stdout.queue(SetAttribute(if cell.bold {
                            Attribute::Bold
                        } else {
                            Attribute::NormalIntensity
                        }))?;
                        cur_bold = cell.bold;
                    }

                    row_buf.push(cell.ch);
                    last.cells[idx] = cell;

                    if next_differs && !row_buf.is_empty() {
                        self.stdout.queue(Print(row_buf.as_str()))?;
                        row_buf.clear();
                    }
                }
                // Flush any remaining cells in the row buffer
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
        let run_buf = &mut self.run_buf;

        if self.row_dirty.len() != frame.height as usize {
            self.row_dirty = vec![Vec::new(); frame.height as usize];
        }
        self.touched_rows.clear();

        for &idx in dirty {
            let y = (idx / width_usize) as u16;
            if y >= frame.height {
                continue;
            }
            let b = &mut self.row_dirty[y as usize];
            if b.is_empty() {
                self.touched_rows.push(y);
            }
            b.push(idx);
        }

        self.touched_rows.sort_unstable();
        self.touched_rows.dedup();

        for y0 in self.touched_rows.iter().copied() {
            let b = &mut self.row_dirty[y0 as usize];
            if b.len() > 1 {
                b.sort_unstable();
            }
            let mut i = 0usize;
            while i < b.len() {
                let idx0 = b[i];
                let cell0 = frame.cell_at_index(idx0);
                if last.cells.get(idx0).copied() == Some(cell0) {
                    i += 1;
                    continue;
                }

                last.cells[idx0] = cell0;

                let x0 = (idx0 % width_usize) as u16;
                let fg0 = cell0.fg;
                let bg0 = cell0.bg;
                let bold0 = cell0.bold;

                run_buf.clear();
                run_buf.push(cell0.ch);
                let mut run_len: u16 = 1;
                let mut last_idx_in_run = idx0;
                let mut j = i + 1;

                while j < b.len() {
                    let idx1 = b[j];
                    if idx1 != last_idx_in_run + 1 {
                        break;
                    }

                    let cell1 = frame.cell_at_index(idx1);
                    if last.cells.get(idx1).copied() == Some(cell1) {
                        break;
                    }
                    if cell1.fg != fg0 || cell1.bg != bg0 || cell1.bold != bold0 {
                        break;
                    }

                    run_buf.push(cell1.ch);
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
            b.clear();
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
        let _ = self.disable_mouse_capture();
        let _ = self.stdout.execute(SetAttribute(Attribute::Reset));
        let _ = self.stdout.execute(ResetColor);
        let _ = self.stdout.execute(cursor::Show);
        let _ = self.stdout.execute(terminal::EnableLineWrap);
        let _ = self.stdout.execute(terminal::LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();

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
        let _ = self.stdout.flush();
        self.shutdown_complete
            .store(true, std::sync::atomic::Ordering::Release);
    }
}

pub fn restore_terminal_best_effort() {
    let mut out = stdout();
    let _ = out.execute(event::DisableMouseCapture);
    let _ = out.execute(SetAttribute(Attribute::Reset));
    let _ = out.execute(ResetColor);
    let _ = out.execute(cursor::Show);
    let _ = out.execute(terminal::EnableLineWrap);
    let _ = out.execute(terminal::LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();
    let _ = out.flush();
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
