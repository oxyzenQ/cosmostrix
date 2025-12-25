// Copyright (c) 2025 rezk_nightky

use crossterm::style::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
}

impl Cell {
    pub fn blank_with_bg(bg: Option<Color>) -> Self {
        Self {
            ch: ' ',
            fg: None,
            bg,
            bold: false,
        }
    }
}
