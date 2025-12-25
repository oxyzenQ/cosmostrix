// Copyright (c) 2025 rezk_nightky

use crate::cell::Cell;

#[derive(Clone, Debug)]
pub struct Frame {
    pub width: u16,
    pub height: u16,
    pub cells: Vec<Cell>,
}

impl Frame {
    pub fn new(width: u16, height: u16, bg: Option<crossterm::style::Color>) -> Self {
        let len = width as usize * height as usize;
        Self {
            width,
            height,
            cells: vec![Cell::blank_with_bg(bg); len],
        }
    }

    pub fn clear_with_bg(&mut self, bg: Option<crossterm::style::Color>) {
        for cell in &mut self.cells {
            *cell = Cell::blank_with_bg(bg);
        }
    }

    pub fn index(&self, x: u16, y: u16) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(y as usize * self.width as usize + x as usize)
    }

    pub fn get(&self, x: u16, y: u16) -> Option<&Cell> {
        self.index(x, y).map(|i| &self.cells[i])
    }

    pub fn set(&mut self, x: u16, y: u16, cell: Cell) {
        if let Some(i) = self.index(x, y) {
            self.cells[i] = cell;
        }
    }
}
