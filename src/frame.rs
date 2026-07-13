// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

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
//! A separate `Vec<u8>` provides O(1) dirty checks without scanning the full
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

// Note: dirty_map is now Vec<u8> (1 byte per cell) instead of BitVec.
// Trade-off: 8x more memory (4.8 KiB vs 600 B for 120x40) but the partial-clear
// hot path becomes a simple indexed byte store (no bit math) and the full-clear
// is a memset that the compiler auto-vectorizes. BitVec's per-bit .set() has
// overhead from read-modify-write on the containing byte.

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
    dirty_map: Vec<u8>,
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
            dirty_map: vec![0u8; len],
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

    /// Returns the current frame generation counter.
    /// Cells whose `cell_gen` matches this value were written in the current frame.
    #[must_use]
    #[inline]
    pub fn current_gen(&self) -> u32 {
        self.gen
    }

    /// Returns the generation counter for a cell at the given flat index.
    /// Useful for checking whether a cell was written in the current frame
    /// without incurring the blank-fallback logic of `cell_at_index_ref`.
    #[must_use]
    #[inline]
    pub fn cell_gen_at_index(&self, i: usize) -> u32 {
        // Dragon egg #6: direct indexing. Caller is expected to pass a valid
        // index (from dirty_indices() or index()). Using .get().copied().unwrap_or(0)
        // adds Option alloc + unwrap_or branching. Direct indexing is a single load.
        // If i is out of bounds, this panics (same as cells[i] would) — which is
        // the correct behavior for a bug in the caller.
        self.cell_gen[i]
    }

    #[must_use]
    pub fn dirty_indices(&self) -> &[usize] {
        &self.dirty
    }

    pub fn clear_dirty(&mut self) {
        if self.dirty_all {
            self.dirty_all = false;
            // Full Vec<u8> reset — compiler auto-vectorizes to wide SIMD stores
            // (AVX2: 32 bytes/store). For 120x40=4800 cells this is 150 stores
            // vs BitVec's 75 byte stores with bit-masking overhead.
            self.dirty_map.fill(0);
            self.dirty.clear();
            return;
        }

        // Partial clear: only clear cells that were marked dirty this frame.
        // Vec<u8> indexed store is a single byte mov; no read-modify-write.
        for &i in &self.dirty {
            self.dirty_map[i] = 0;
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
    // Test-facing accessor; renderer hot paths use `cell_at_index`.
    #[allow(dead_code)]
    pub fn get(&self, x: u16, y: u16) -> Option<&Cell> {
        self.index(x, y).map(|i| {
            // Dragon egg #7: direct indexing — i from index() is bounds-checked.
            if self.cell_gen[i] == self.gen {
                &self.cells[i]
            } else {
                &self.blank
            }
        })
    }

    #[must_use]
    #[inline]
    pub fn cell_at_index(&self, i: usize) -> Cell {
        // P3: direct indexing (caller bounds-checks via index() or dirty_indices)
        if self.cell_gen[i] == self.gen {
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
        // P3: direct indexing
        if self.cell_gen[i] == self.gen {
            &self.cells[i]
        } else {
            &self.blank
        }
    }

    #[inline]
    pub fn set(&mut self, x: u16, y: u16, cell: Cell) {
        if let Some(i) = self.index(x, y) {
            // P3 dragon egg: direct indexing instead of .get().copied() == Some().
            // The index() call above already bounds-checked i. Using direct
            // indexing here avoids the redundant bounds check in .get() and
            // the Option allocation in .copied().
            //
            // BEFORE: self.cell_gen.get(i).copied() == Some(self.gen)
            //   = bounds check + Option<u32> alloc + copied + Option comparison
            // AFTER: self.cell_gen[i] == self.gen
            //   = direct load + u32 comparison
            //
            // Saves ~2-3 cycles per set() call. At 50K FPS × ~300 dirty cells/frame
            // = 15M set() calls/sec, saves ~30-45M cycles/sec = ~10-15ms/sec.
            let gen_matches = self.cell_gen[i] == self.gen;
            let cur = if gen_matches {
                self.cells[i]
            } else {
                self.blank
            };
            if cur == cell {
                return;
            }

            self.cells[i] = cell;
            self.cell_gen[i] = self.gen;
            // Vec<u8> dirty check: direct byte load (no bit math).
            // Before (BitVec): read-modify-write on containing byte + mask.
            // After (Vec<u8>): single byte load + store.
            if !self.dirty_all && self.dirty_map[i] == 0 {
                self.dirty_map[i] = 1;
                self.dirty.push(i);
            }
        }
    }

    /// Force-set a cell without equality comparison.
    /// Used when the caller knows the cell content has changed
    /// (e.g., monolith previous_cell cleanup clearing known-drawn cells).
    /// Skips the 24-byte Cell equality check, saving ~10-15ns per call
    /// in the monolith render hot path.
    #[inline]
    pub fn set_force(&mut self, x: u16, y: u16, cell: Cell) {
        if let Some(i) = self.index(x, y) {
            // Dragon egg #1: direct indexing — index() already bounds-checked.
            self.cells[i] = cell;
            self.cell_gen[i] = self.gen;
            if !self.dirty_all && self.dirty_map[i] == 0 {
                self.dirty_map[i] = 1;
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
