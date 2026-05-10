// Copyright (c) 2026 rezky_nightky

use smallvec::SmallVec;

use crate::cell::Cell;
use crate::constants::DIRTY_CAPACITY_DIVISOR;
use bitvec::prelude::BitVec;

/// Inline capacity for dirty indices SmallVec (64 usize = 512 bytes on stack).
/// Covers small terminals without heap allocation; spills to heap for large frames.
const DIRTY_INLINE_CAPACITY: usize = 64;

#[derive(Clone, Debug)]
pub struct Frame {
    pub width: u16,
    pub height: u16,
    pub cells: Vec<Cell>,
    gen: u32,
    cell_gen: Vec<u32>,
    blank: Cell,
    dirty_all: bool,
    dirty_map: BitVec,
    dirty: SmallVec<[usize; DIRTY_INLINE_CAPACITY]>,
}

impl Frame {
    pub fn new(width: u16, height: u16, bg: Option<crossterm::style::Color>) -> Self {
        let len = width as usize * height as usize;
        let blank = Cell::blank_with_bg(bg);
        let gen = 1u32;
        Self {
            width,
            height,
            cells: vec![blank; len],
            gen,
            cell_gen: vec![gen; len],
            blank,
            dirty_all: true,
            dirty_map: BitVec::repeat(false, len),
            dirty: SmallVec::with_capacity(len / DIRTY_CAPACITY_DIVISOR),
        }
    }

    pub fn clear_with_bg(&mut self, bg: Option<crossterm::style::Color>) {
        self.blank = Cell::blank_with_bg(bg);
        self.gen = self.gen.wrapping_add(1);
        if self.gen == 0 {
            self.cell_gen.fill(0);
            self.gen = 1;
        }
        self.dirty_all = true;
        self.dirty.clear();
    }

    #[must_use]
    pub fn is_dirty_all(&self) -> bool {
        self.dirty_all
    }

    #[must_use]
    pub fn dirty_indices(&self) -> &[usize] {
        &self.dirty
    }

    #[allow(dead_code)]
    pub fn sort_dirty(&mut self) {
        if self.dirty_all || self.dirty.len() <= 1 {
            return;
        }
        self.dirty.sort_unstable();
    }

    pub fn clear_dirty(&mut self) {
        if self.dirty_all {
            self.dirty_all = false;
            self.dirty_map.fill(false);
            self.dirty.clear();
            return;
        }

        for &i in &self.dirty {
            if let Some(mut v) = self.dirty_map.get_mut(i) {
                *v = false;
            }
        }
        self.dirty.clear();
    }

    #[must_use]
    pub fn index(&self, x: u16, y: u16) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(y as usize * self.width as usize + x as usize)
    }

    #[must_use]
    #[allow(dead_code)]
    pub fn get(&self, x: u16, y: u16) -> Option<&Cell> {
        self.index(x, y).map(|i| {
            if self.cell_gen.get(i).copied() == Some(self.gen) {
                &self.cells[i]
            } else {
                &self.blank
            }
        })
    }

    #[must_use]
    pub fn cell_at_index(&self, i: usize) -> Cell {
        if self.cell_gen.get(i).copied() == Some(self.gen) {
            self.cells[i]
        } else {
            self.blank
        }
    }

    pub fn set(&mut self, x: u16, y: u16, cell: Cell) {
        if let Some(i) = self.index(x, y) {
            let cur = if self.cell_gen.get(i).copied() == Some(self.gen) {
                self.cells[i]
            } else {
                self.blank
            };
            if cur == cell {
                return;
            }

            self.cells[i] = cell;
            if let Some(v) = self.cell_gen.get_mut(i) {
                *v = self.gen;
            }
            if !self.dirty_all && self.dirty_map.get(i).map_or(true, |b| !*b) {
                self.dirty_map.set(i, true);
                self.dirty.push(i);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_with_bg_makes_cells_effectively_blank() {
        let mut f = Frame::new(2, 2, None);
        f.set(
            0,
            0,
            Cell {
                ch: 'x',
                fg: None,
                bg: None,
                bold: false,
            },
        );
        assert_eq!(f.get(0, 0).unwrap().ch, 'x');
        f.clear_with_bg(None);
        assert_eq!(f.get(0, 0).unwrap().ch, ' ');
    }
}
