// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Phosphor Ghost — kanji characters that appear on dim rain cells with
//! fade-in/out animation. Ghosts render before rain so droplets partially
//! overwrite them, creating a layered depth effect.

use std::time::{Duration, Instant};

use crossterm::style::Color;
use rand::Rng;

use crate::cell::Cell;
use crate::frame::Frame;

use super::super::atmospheric_events::{AtmosphericEvent, EventCtx, EventState};

const GHOST_CHARS: &[char] = &['雨', '雷', '電', '風', '雲', '闇', '光'];
const GHOST_FADE_IN_FRAC: f32 = 0.2;
const GHOST_FADE_OUT_FRAC: f32 = 0.3;
const GHOST_BASE_COLOR: (u8, u8, u8) = (18, 22, 18);

pub(crate) struct GhostEvent {
    col: u16,
    line: u16,
    ch: char,
    spawn_time: Instant,
    duration: Duration,
}

impl GhostEvent {
    pub fn new(col: u16, line: u16, now: Instant) -> Self {
        let mut rng = rand::rng();
        let idx = rng.random_range(0..GHOST_CHARS.len());
        let duration_var = 2000 + rng.random_range(0..2000);
        Self {
            col: col.max(1),
            line: line.max(1),
            ch: GHOST_CHARS[idx],
            spawn_time: now,
            duration: Duration::from_millis(duration_var as u64),
        }
    }
}

impl AtmosphericEvent for GhostEvent {
    fn state(&self) -> EventState {
        EventState::Active
    }
    fn is_finished(&self) -> bool {
        self.spawn_time.elapsed() >= self.duration
    }
    fn phase_durations_ms(&self) -> (u64, u64) {
        (2000, 2000)
    }
    fn memory_footprint(&self) -> usize {
        128
    }
    fn update(&mut self, _now: Instant) {}

    fn render(&self, ctx: &EventCtx, frame: &mut Frame) {
        let elapsed = self.spawn_time.elapsed().as_secs_f32();
        let total = self.duration.as_secs_f32();
        let progress = (elapsed / total).clamp(0.0, 1.0);

        let opacity = if progress < GHOST_FADE_IN_FRAC {
            progress / GHOST_FADE_IN_FRAC
        } else if progress > (1.0 - GHOST_FADE_OUT_FRAC) {
            (1.0 - progress) / GHOST_FADE_OUT_FRAC
        } else {
            1.0
        };

        let (br, bg, bb) = GHOST_BASE_COLOR;

        let r = (br as f32 * opacity) as u8;
        let g = (bg as f32 * opacity) as u8;
        let b = (bb as f32 * opacity) as u8;
        if r == 0 && g == 0 && b == 0 {
            return;
        }

        if self.col >= ctx.cols || self.line >= ctx.lines {
            return;
        }

        let Some(idx) = frame.index(self.col, self.line) else {
            return;
        };
        let cell = frame.cell_at_index(idx);

        // Only draw on dim cells (don't overwrite bright rain)
        let (cr, cg, cb) = match cell.fg {
            Some(Color::Rgb { r, g, b }) => (r as f32, g as f32, b as f32),
            _ => return,
        };
        let brightness = cr * 0.299 + cg * 0.587 + cb * 0.114;
        if brightness < 80.0 {
            frame.set_force(
                self.col,
                self.line,
                Cell {
                    ch: self.ch,
                    fg: Some(Color::Rgb { r, g, b }),
                    ..cell
                },
            );
        }
    }

    fn is_pre_rain(&self) -> bool {
        true
    }

    fn seed_phosphor(
        &self,
        _phosphor: &mut [u8],
        _phosphor_base_fg: &mut [Option<Color>],
        _phosphor_base_ch: &mut [char],
        _cols: u16,
        _lines: u16,
    ) {
    }
}
