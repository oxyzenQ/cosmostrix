// Copyright (c) 2026 rezky_nightky

//! Terminal cell representation.
//!
//! A `Cell` is the atomic unit of the frame buffer — a single terminal
//! position containing a character, foreground/background colors, and a
//! bold flag. Cells are copied by value (~24 bytes) throughout the renderer.

use crossterm::style::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
}

impl Cell {
    #[must_use]
    pub fn blank_with_bg(bg: Option<Color>) -> Self {
        Self {
            ch: ' ',
            fg: None,
            bg,
            bold: false,
        }
    }
}
