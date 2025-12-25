// Copyright (c) 2025 rezk_nightky

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

pub struct Terminal {
    stdout: Stdout,
    last: Option<Frame>,
}

impl Terminal {
    pub fn new() -> Result<Self> {
        let mut out = stdout();
        terminal::enable_raw_mode()?;
        out.execute(terminal::EnterAlternateScreen)?;
        out.execute(cursor::Hide)?;
        out.execute(terminal::Clear(terminal::ClearType::All))?;
        out.flush()?;
        Ok(Self {
            stdout: out,
            last: None,
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

    pub fn draw(&mut self, frame: &Frame) -> Result<()> {
        let mut cur_fg: Option<Color> = None;
        let mut cur_bg: Option<Color> = None;
        let mut cur_bold: bool = false;

        let needs_full_redraw = self
            .last
            .as_ref()
            .map(|l| l.width != frame.width || l.height != frame.height)
            .unwrap_or(true);

        if needs_full_redraw {
            self.stdout
                .queue(terminal::Clear(terminal::ClearType::All))?;
        }

        for y in 0..frame.height {
            for x in 0..frame.width {
                let idx = y as usize * frame.width as usize + x as usize;
                let cell = frame.cells[idx];
                let changed = if needs_full_redraw {
                    true
                } else {
                    self.last
                        .as_ref()
                        .and_then(|l| l.cells.get(idx).copied())
                        .map(|prev| prev != cell)
                        .unwrap_or(true)
                };

                if !changed {
                    continue;
                }

                self.stdout.queue(cursor::MoveTo(x, y))?;

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

                let mut buf = [0u8; 4];
                let s = cell.ch.encode_utf8(&mut buf);
                self.stdout.queue(Print(s))?;
            }
        }

        self.stdout.queue(SetAttribute(Attribute::Reset))?;
        self.stdout.queue(ResetColor)?;
        self.stdout.flush()?;

        self.last = Some(frame.clone());
        Ok(())
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.stdout.execute(SetAttribute(Attribute::Reset));
        let _ = self.stdout.execute(ResetColor);
        let _ = self.stdout.execute(cursor::Show);
        let _ = self.stdout.execute(terminal::LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
        let _ = self.stdout.flush();
    }
}

pub fn blank_cell(bg: Option<Color>) -> Cell {
    Cell {
        ch: ' ',
        fg: None,
        bg,
        bold: false,
    }
}
