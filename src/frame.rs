// Copyright (c) 2026 rezky_nightky

//! Differential frame buffer with generation-based dirty tracking.
//!
//! The frame buffer is the central data structure between simulation and
//! output. It stores a 2D grid of [`Cell`] values and tracks which cells
//! have changed since the last draw call.
//!
//! ## Dirty Tracking Strategy
//!
//! Each cell carries a *generation counter* alongside its content. When a cell
//! is written, its generation is updated to match the current frame generation.
//! A separate [`BitVec`] provides O(1) dirty checks without scanning the full
//! grid. Dirty indices are collected into a [`SmallVec`] with 64 inline slots,
//! covering small terminals without heap allocation.
//!
//! The generation system allows cells to be "logically cleared" without
//! physically overwriting them — [`clear_with_bg`] bumps the generation,
//! making all previous cells appear blank without a full buffer zeroing.

use smallvec::SmallVec;

use crate::cell::Cell;
use crate::constants::{
    DIRTY_CAPACITY_CAP, DIRTY_CAPACITY_DIVISOR, MAX_TERMINAL_COLS, MAX_TERMINAL_LINES,
    MIN_TERMINAL_COLS, MIN_TERMINAL_LINES,
};
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
    /// Semantic generation counter: incremented when the renderer's semantic
    /// identity changes (charset switch, shading mode toggle, theme change).
    /// The Terminal's LastFrame cache tracks this value — a mismatch forces
    /// a full redraw regardless of cell-level diff results, eliminating stale
    /// glyph residue from semantic mutations.
    pub semantic_gen: u32,
}

impl Frame {
    pub fn new(width: u16, height: u16, bg: Option<crossterm::style::Color>) -> Self {
        // Safety clamp: prevent OOM from absurd terminal sizes
        let width = width.clamp(MIN_TERMINAL_COLS, MAX_TERMINAL_COLS);
        let height = height.clamp(MIN_TERMINAL_LINES, MAX_TERMINAL_LINES);
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
            dirty: SmallVec::with_capacity((len / DIRTY_CAPACITY_DIVISOR).min(DIRTY_CAPACITY_CAP)),
            semantic_gen: 0,
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

    /// Invalidate the renderer's semantic identity. Call when charset,
    /// shading mode, or theme changes. Increments `semantic_gen` and
    /// performs a full logical clear via `clear_with_bg`, ensuring the
    /// Terminal's differential renderer detects the semantic change and
    /// forces a complete redraw — eliminating stale glyph residue from
    /// the previous renderer configuration.
    pub fn invalidate_semantic(&mut self, bg: Option<crossterm::style::Color>) {
        self.semantic_gen = self.semantic_gen.wrapping_add(1);
        self.clear_with_bg(bg);
    }

    #[must_use]
    pub fn is_dirty_all(&self) -> bool {
        self.dirty_all
    }

    #[must_use]
    pub fn dirty_indices(&self) -> &[usize] {
        &self.dirty
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
    #[inline]
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
    #[inline]
    pub fn cell_at_index(&self, i: usize) -> Cell {
        if self.cell_gen.get(i).copied() == Some(self.gen) {
            self.cells[i]
        } else {
            self.blank
        }
    }

    /// Borrow a cell by index without copying. Use when you only need to
    /// inspect the cell (e.g., check `fg.is_some()`) rather than move it.
    #[must_use]
    #[inline]
    pub fn cell_at_index_ref(&self, i: usize) -> &Cell {
        if self.cell_gen.get(i).copied() == Some(self.gen) {
            &self.cells[i]
        } else {
            &self.blank
        }
    }

    #[inline]
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

    #[test]
    fn top_row_glyph_to_blank_is_dirty() {
        let mut f = Frame::new(4, 4, None);
        f.clear_dirty();

        f.set(
            2,
            0,
            Cell {
                ch: 'x',
                fg: None,
                bg: None,
                bold: false,
            },
        );
        assert_eq!(f.dirty_indices(), &[2]);

        f.clear_dirty();
        f.set(2, 0, Cell::blank_with_bg(None));

        assert_eq!(f.dirty_indices(), &[2]);
        assert_eq!(f.get(2, 0).unwrap().ch, ' ');
    }
}
