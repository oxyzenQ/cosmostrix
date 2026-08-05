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

use super::super::ghost_events::{CinematicEvent, EventCtx};

// v30.1 — Bug #11 regression fix.
// Previously held fullwidth CJK ideographs ('雨','雷','電','風','雲','闇','光'),
// all EAW=Wide (width=2). The frame buffer has no per-cell width metadata,
// so a width=2 char advances the terminal cursor by 2 while the renderer
// tracks 1 — every subsequent cell in the row shifts right by 1, matching
// the original Bug #11 (commit c1843fe) symptom: rain rows "shift right"
// near the ghost, normalize after force_draw_everything fires.
//
// Fix: halfwidth Katakana (U+FF66-U+FF9D, EAW=Halfwidth, width=1).
// Preserves the "kanji ghost" aesthetic while satisfying the 1-char-1-cell
// invariant enforced by sanitize_message_text / charset_custom / build_chars.
pub(crate) const GHOST_CHARS: &[char] = &['ｱ', 'ｲ', 'ｳ', 'ｴ', 'ｵ', 'ｶ', 'ｷ', 'ｸ', 'ｹ', 'ｺ'];
const GHOST_FADE_IN_FRAC: f32 = 0.2;
const GHOST_FADE_OUT_FRAC: f32 = 0.3;

pub(crate) struct GhostEvent {
    col: u16,
    line: u16,
    ch: char,
    spawn_time: Instant,
    duration: Duration,
}

impl GhostEvent {
    pub(crate) fn new(col: u16, line: u16, now: Instant) -> Self {
        let mut rng = rand::rng();
        let idx = rng.random_range(0..GHOST_CHARS.len());
        let duration_var = 2000 + rng.random_range(0..2000);
        let ch = GHOST_CHARS[idx];
        // Bug #11 regression guard: every char written to the frame buffer
        // MUST have unicode_width::width() == Some(1). Wide chars (CJK
        // ideographs, emoji) advance the terminal cursor by 2 columns while
        // the renderer tracks only 1, desyncing every subsequent cell in the
        // row. See the GHOST_CHARS doc comment above for the full rationale.
        //
        // Import is scoped inside the function so release builds (where
        // debug_assert! is compiled out) don't pay the unused-import warning.
        #[cfg(debug_assertions)]
        {
            use unicode_width::UnicodeWidthChar;
            debug_assert!(
                UnicodeWidthChar::width(ch) == Some(1),
                "GHOST_CHARS entry {ch:?} must be width=1 (Bug #11 regression guard)"
            );
        }
        Self {
            col: col.max(1),
            line: line.max(1),
            ch,
            spawn_time: now,
            duration: Duration::from_millis(duration_var as u64),
        }
    }
}

impl CinematicEvent for GhostEvent {
    fn is_finished(&self) -> bool {
        self.spawn_time.elapsed() >= self.duration
    }

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

        let (br, bg, bb) = ctx.ghost_base_color;

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
}
