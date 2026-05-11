// Copyright (c) 2026 rezky_nightky

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

    /// Approximate perceived luminance as a 0-255 value.
    /// Used by Differential Glyph Intelligence to decide if a color change
    /// is perceptually significant enough to warrant a redraw.
    /// Uses the standard ITU-R BT.601 luma formula.
    #[must_use]
    pub fn luminance(&self) -> u8 {
        let (r, g, b) = match self.fg {
            Some(Color::Rgb { r, g, b }) => (r, g, b),
            Some(Color::AnsiValue(v)) => ansi256_to_rgb(v),
            Some(Color::White) => (255, 255, 255),
            Some(Color::Grey) => (192, 192, 192),
            Some(Color::DarkGrey) => (128, 128, 128),
            Some(Color::Black) => (0, 0, 0),
            Some(c) => named_color_rgb(c),
            None => match self.bg {
                Some(Color::Rgb { r, g, b }) => (r, g, b),
                Some(Color::AnsiValue(v)) => ansi256_to_rgb(v),
                _ => (0, 0, 0),
            },
        };
        // ITU-R BT.601: Y = 0.299R + 0.587G + 0.114B
        let luma = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
        luma.round().clamp(0.0, 255.0) as u8
    }
}

/// Decode a 256-color ANSI index to approximate (r, g, b).
fn ansi256_to_rgb(v: u8) -> (u8, u8, u8) {
    if v < 16 {
        const ANSI16: [(u8, u8, u8); 16] = [
            (0, 0, 0), (128, 0, 0), (0, 128, 0), (128, 128, 0),
            (0, 0, 128), (128, 0, 128), (0, 128, 128), (192, 192, 192),
            (128, 128, 128), (255, 0, 0), (0, 255, 0), (255, 255, 0),
            (0, 0, 255), (255, 0, 255), (0, 255, 255), (255, 255, 255),
        ];
        ANSI16[v as usize]
    } else if v < 232 {
        const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
        let v = v - 16;
        (LEVELS[(v / 36) as usize], LEVELS[((v % 36) / 6) as usize], LEVELS[(v % 6) as usize])
    } else {
        let gray = 8 + 10 * (v - 232);
        (gray, gray, gray)
    }
}

/// Approximate RGB for named 8/16 colors.
fn named_color_rgb(c: Color) -> (u8, u8, u8) {
    match c {
        Color::DarkRed => (128, 0, 0),
        Color::DarkGreen => (0, 128, 0),
        Color::DarkYellow => (128, 128, 0),
        Color::DarkBlue => (0, 0, 128),
        Color::DarkMagenta => (128, 0, 128),
        Color::DarkCyan => (0, 128, 128),
        Color::Red => (255, 0, 0),
        Color::Green => (0, 255, 0),
        Color::Yellow => (255, 255, 0),
        Color::Blue => (0, 0, 255),
        Color::Magenta => (255, 0, 255),
        Color::Cyan => (0, 255, 255),
        _ => (0, 0, 0),
    }
}
