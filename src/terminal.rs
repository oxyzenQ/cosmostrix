// Copyright (c) 2026 rezky_nightky

use std::io::{stdout, Result, Stdout, Write};

use crossterm::{
    cursor, event,
    style::{
        Attribute, Color, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
    },
    terminal, ExecutableCommand, QueueableCommand,
};

use crate::cell::Cell;
use crate::frame::Frame;

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
    row_dirty: Vec<Vec<usize>>,
    touched_rows: Vec<u16>,
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
            row_dirty: Vec::new(),
            touched_rows: Vec::new(),
        })
    }

    pub fn size(&self) -> Result<(u16, u16)> {
        terminal::size()
    }

    pub fn poll_event(timeout: std::time::Duration) -> Result<bool> {
        event::poll(timeout)
    }

    pub fn read_event() -> Result<event::Event> {
        event::read()
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
        let dirty_is_large = total_cells > 0 && dirty_count >= (total_cells / 3);
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

            for y in 0..frame.height {
                self.stdout.queue(cursor::MoveTo(0, y))?;
                for x in 0..frame.width {
                    let idx = y as usize * frame.width as usize + x as usize;
                    let cell = frame.cell_at_index(idx);

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

                    self.stdout.queue(Print(cell.ch))?;

                    last.cells[idx] = cell;
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
        for r in &mut self.row_dirty {
            r.clear();
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
        let _ = self.stdout.execute(SetAttribute(Attribute::Reset));
        let _ = self.stdout.execute(ResetColor);
        let _ = self.stdout.execute(cursor::Show);
        let _ = self.stdout.execute(terminal::EnableLineWrap);
        let _ = self.stdout.execute(terminal::LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
        let _ = self.stdout.flush();
    }
}

pub fn restore_terminal_best_effort() {
    let mut out = stdout();
    let _ = out.execute(SetAttribute(Attribute::Reset));
    let _ = out.execute(ResetColor);
    let _ = out.execute(cursor::Show);
    let _ = out.execute(terminal::EnableLineWrap);
    let _ = out.execute(terminal::LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();
    let _ = out.flush();
}

pub fn blank_cell(bg: Option<Color>) -> Cell {
    Cell {
        ch: ' ',
        fg: None,
        bg,
        bold: false,
    }
}
