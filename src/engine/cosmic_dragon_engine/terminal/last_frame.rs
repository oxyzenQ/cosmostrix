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
    /// NIGHT-hunter-34: the physical screen state is UNKNOWN.
    ///
    /// Set to `true` by every constructor path (`new`, `reuse_or_new`)
    /// — both are only called when the previous shadow was discarded
    /// (first draw, resize, semantic reset). A freshly initialized
    /// shadow says "every cell is blank with bg=None", but the real
    /// terminal still holds whatever was last emitted (intro rain, the
    /// previous scene's glyphs, a `color-bg = "black"` fill). The
    /// HUNT-27 cell-skip in the full-redraw path must therefore emit
    /// EVERY cell at least once; `Terminal::draw` clears this flag
    /// after that emit, making the shadow trustworthy again.
    ///
    /// Why this was invisible under `color-bg = "black"`: the frame's
    /// blank cell carries `bg = Some(black)` while the fresh shadow
    /// claims `bg = None` — the cells differ, so the full redraw
    /// repainted everything by accident. Under
    /// `color-bg = "default-background"` the frame blank is also
    /// `bg = None`, the skip fires, and the physical residue survives
    /// every semantic event (scene switch 'x'/'X', restart 'r',
    /// live-reload rebuilds) — the owner's four NIGHT-hunter-34
    /// reproductions.
    ///
    /// The idle-resync zero-emit optimization (HUNT-27's actual target:
    /// `force_repaint` with no semantic change) is untouched — that
    /// path preserves the shadow, so `force_full_emit` stays false and
    /// the skip keeps working there.
    pub(crate) force_full_emit: bool,
}

impl LastFrame {
    pub(crate) fn new(width: u16, height: u16) -> Self {
        let len = width as usize * height as usize;
        Self {
            width,
            height,
            cells: vec![Cell::blank_with_bg(None); len],
            semantic_gen: 0,
            force_full_emit: true,
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
    ///
    /// NIGHT-hunter-34: both paths set `force_full_emit = true` — the
    /// discarded shadow's physical-screen knowledge is gone, so the
    /// next draw must re-emit every cell (see the field doc above).
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
        old.force_full_emit = true;
        old
    }
}
