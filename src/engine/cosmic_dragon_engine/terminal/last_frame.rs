// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! [`LastFrame`] — double-buffered previous-frame cache for the diff pipeline.
//!
//! Extracted from `terminal/mod.rs` in the dragon-fight branch. Holds the
//! previous frame's rendered cells so `Terminal::draw` can compute the
//! differential (only dirty cells) on the next frame. The `reuse_or_new`
//! optimization avoids heap churn during resize-drag storms.

use crate::cell::Cell;

/// Previous-frame cell cache, used by the differential render path.
pub(crate) struct LastFrame {
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) cells: Vec<Cell>,
    /// Semantic generation this LastFrame was rendered with.
    /// A mismatch with `Frame::semantic_gen` forces a full redraw.
    pub(crate) semantic_gen: u32,
}

impl LastFrame {
    pub(crate) fn new(width: u16, height: u16) -> Self {
        let len = width as usize * height as usize;
        Self {
            width,
            height,
            cells: vec![Cell::blank_with_bg(None); len],
            semantic_gen: 0,
        }
    }

    /// (perf polish): reuse the existing Vec allocation when the
    /// new dimensions fit within the old capacity. Avoids a heap
    /// alloc/dealloc pair every time the terminal is resized to a
    /// smaller or equal size — common during window-drag resize storms
    /// where the user overshoots and settles back to the original size.
    ///
    /// When the new size exceeds the existing capacity, falls back to a
    /// fresh allocation (same as `new`). When no existing frame is
    /// provided, also falls back to `new`.
    ///
    /// Safety of `resize_with`: `Vec::resize_with(new_len, || blank)`
    /// first truncates if `new_len < old.len()`, then extends by calling
    /// the closure for each new element. We `clear()` first to drop all
    /// old cell values (which contained previous-frame content) so the
    /// resulting Vec is uniformly blank. The underlying allocation is
    /// reused — only the length changes.
    pub(crate) fn reuse_or_new(existing: Option<Self>, width: u16, height: u16) -> Self {
        let Some(mut old) = existing else {
            return Self::new(width, height);
        };
        let new_len = width as usize * height as usize;
        if old.cells.capacity() < new_len {
            // Need a bigger buffer — allocate fresh. The old Vec is
            // dropped, freeing its allocation.
            return Self::new(width, height);
        }
        // Reuse the allocation. Clear drops all existing cells (which
        // contained previous-frame content), then resize_with extends
        // back to new_len using the blank-cell closure. The Vec's
        // capacity is preserved across clear+resize_with.
        old.cells.clear();
        old.cells.resize_with(new_len, || Cell::blank_with_bg(None));
        old.width = width;
        old.height = height;
        old.semantic_gen = 0;
        old
    }
}
