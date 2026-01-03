// Copyright (c) 2026 rezky_nightky

use std::time::{Duration, Instant};

use crate::cloud::{CharLoc, DrawCtx};
use crate::frame::Frame;

#[derive(Clone, Debug)]
pub struct Droplet {
    pub is_alive: bool,
    pub is_head_crawling: bool,
    pub is_tail_crawling: bool,

    pub bound_col: u16,
    pub head_put_line: u16,
    pub head_cur_line: u16,

    pub tail_put_line: Option<u16>,
    pub tail_cur_line: u16,

    pub end_line: u16,
    pub char_pool_idx: u16,
    pub length: u16,
    pub chars_per_sec: f32,

    pub advance_remainder: f32,

    pub last_time: Option<Instant>,
    pub head_stop_time: Option<Instant>,
    pub time_to_linger: Duration,
}

impl Droplet {
    pub fn new() -> Self {
        Self {
            is_alive: false,
            is_head_crawling: false,
            is_tail_crawling: false,
            bound_col: u16::MAX,
            head_put_line: 0,
            head_cur_line: 0,
            tail_put_line: None,
            tail_cur_line: 0,
            end_line: u16::MAX,
            char_pool_idx: u16::MAX,
            length: u16::MAX,
            chars_per_sec: 0.0,

            advance_remainder: 0.0,

            last_time: None,
            head_stop_time: None,
            time_to_linger: Duration::from_millis(0),
        }
    }

    pub fn activate(&mut self, now: Instant) {
        self.is_alive = true;
        self.is_head_crawling = true;
        self.is_tail_crawling = true;
        self.advance_remainder = 0.0;
        self.last_time = Some(now);
    }

    pub fn increment_time(&mut self, delta: Duration) {
        if let Some(t) = self.last_time.as_mut() {
            *t += delta;
        }
        if let Some(t) = self.head_stop_time.as_mut() {
            *t += delta;
        }
    }

    pub fn advance(&mut self, now: Instant, lines: u16) -> bool {
        let Some(last) = self.last_time else {
            self.last_time = Some(now);
            return false;
        };

        let elapsed = now.saturating_duration_since(last);
        let elapsed_sec = elapsed.as_secs_f32();
        let delta = (self.chars_per_sec * elapsed_sec).max(0.0);
        let total = self.advance_remainder + delta;
        let whole = total.floor();
        self.advance_remainder = total - whole;
        let chars_advanced = whole as u16;
        if chars_advanced == 0 {
            self.last_time = Some(now);
            return false;
        }

        if self.is_head_crawling {
            self.head_put_line = self.head_put_line.saturating_add(chars_advanced);
            if self.head_put_line > self.end_line {
                self.head_put_line = self.end_line;
            }

            if self.head_put_line == self.end_line {
                self.is_head_crawling = false;
                if self.head_stop_time.is_none() {
                    self.head_stop_time = Some(now);
                    if self.time_to_linger > Duration::from_millis(0) {
                        self.is_tail_crawling = false;
                    }
                }
            }
        }

        if self.is_tail_crawling
            && (self.head_put_line >= self.length || self.head_put_line >= self.end_line)
        {
            let next_tail = match self.tail_put_line {
                Some(v) => v.saturating_add(chars_advanced),
                None => chars_advanced,
            };

            let mut next_tail = next_tail;
            if next_tail > self.end_line {
                next_tail = self.end_line;
            }
            self.tail_put_line = Some(next_tail);

            let thresh_line = lines / 4;
            if self.tail_cur_line <= thresh_line && next_tail > thresh_line {
                self.last_time = Some(now);
                return true;
            }
        }

        if !self.is_tail_crawling {
            if let Some(stop) = self.head_stop_time {
                if now.saturating_duration_since(stop) >= self.time_to_linger {
                    self.is_tail_crawling = true;
                }
            }
        }

        if self.tail_put_line == Some(self.head_put_line) {
            self.is_alive = false;
        }

        self.last_time = Some(now);
        false
    }

    fn is_head_bright(&self, now: Instant) -> bool {
        if self.is_head_crawling {
            return true;
        }
        if let Some(stop) = self.head_stop_time {
            return now.saturating_duration_since(stop) <= Duration::from_millis(100);
        }
        false
    }

    pub fn draw(
        &mut self,
        ctx: &DrawCtx<'_>,
        frame: &mut Frame,
        now: Instant,
        draw_everything: bool,
    ) {
        let bg = ctx.bg;

        let mut start_line = 0u16;
        if let Some(tp) = self.tail_put_line {
            for line in self.tail_cur_line..=tp {
                frame.set(self.bound_col, line, crate::terminal::blank_cell(bg));
            }
            self.tail_cur_line = tp;
            start_line = tp.saturating_add(1);
        }

        for line in start_line..=self.head_put_line {
            if line >= ctx.lines {
                break;
            }

            let is_glitched = ctx.is_glitched(line, self.bound_col);
            let val = ctx.get_char(line, self.char_pool_idx);

            let mut loc = CharLoc::Middle;
            if self.tail_put_line.is_some() && Some(line) == self.tail_put_line.map(|v| v + 1) {
                loc = CharLoc::Tail;
            }
            if line == self.head_put_line && self.is_head_bright(now) {
                loc = CharLoc::Head;
            }

            if matches!(loc, CharLoc::Middle)
                && line < self.head_cur_line
                && !is_glitched
                && line != self.end_line
                && !ctx.shading_distance
                && !draw_everything
            {
                continue;
            }

            let (fg, bold) = ctx.get_attr(
                line,
                self.bound_col,
                val,
                loc,
                now,
                self.head_put_line,
                self.length,
            );

            frame.set(
                self.bound_col,
                line,
                crate::cell::Cell {
                    ch: val,
                    fg,
                    bg,
                    bold,
                },
            );

            if ctx.full_width && self.bound_col + 1 < frame.width {
                frame.set(
                    self.bound_col + 1,
                    line,
                    crate::cell::Cell {
                        ch: ' ',
                        fg: None,
                        bg,
                        bold: false,
                    },
                );
            }
        }

        self.head_cur_line = self.head_put_line;
    }
}
