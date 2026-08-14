// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Differential + full-redraw ANSI render path for [`Terminal`].
//!
//! Extracted from `terminal/mod.rs` in the dragon-fight branch to keep the
//! main module under the 1500-LOC cap and isolate the perf-critical render
//! path. This is the single hottest function in cosmostrix — every frame
//! goes through `Terminal::draw`.
//!
//! ## Two paths
//!
//! 1. **Full redraw** (≥12.5% cells dirty, or dim/semantic change): row-RLE
//!    pass over all cells, accumulates into `ansi_buf`, one `write_all`.
//! 2. **Differential** (<12.5% cells dirty): flat sorted dirty-index
//!    iteration, contiguous-run detection, per-run MoveTo + SGR batching.
//!
//! The crossover threshold is `DIRTY_THRESHOLD_RATIO` (currently 8 = 12.5%).
//! See `cosmic_dragon::egg::threshold_sweep` for the benchmark that tuned it.

use std::io::{Result, Write};

use crossterm::{
    style::{Color, SetBackgroundColor},
    terminal as crossterm_terminal, QueueableCommand,
};

use crate::bolt::{BOLD_ESCAPES, BOLD_ESCAPE_LENS};
use crate::constants::DIRTY_THRESHOLD_RATIO;
use crate::frame::Frame;
use crate::sgr_format::push_u16;

use super::{LastFrame, Terminal};

impl Terminal {
    /// Render the frame to stdout via the diff-based ANSI pipeline.
    ///
    /// See the module docs for the two-path strategy. This method owns the
    /// perf-critical hot path — every allocation here was audited during the
    /// Cosmic Dragon egg experiments (see `docs/archive/cosmic_dragon/FINDINGS.md`).
    pub(crate) fn draw(&mut self, frame: &mut Frame) -> Result<()> {
        let mut cur_fg: Option<Color> = None;
        let mut cur_bg: Option<Color> = None;
        let mut cur_bold: bool = false;
        let mut cur_pos: Option<(u16, u16)> = None;

        // Separate dimension-change detection from semantic-change detection.
        // Clear(All) is ONLY issued when the terminal dimensions changed, because
        // resized terminals may have stale content at the new edges that isn't
        // covered by the frame. For semantic-only changes (charset, shading,
        // theme), the full redraw path iterates every cell and overwrites it, so
        // a blanket clear is redundant — and it causes visible flicker in
        // fullscreen terminals because the screen is blanked before the redraw
        // completes (the gap is perceptible at high cell counts).
        let (needs_full_redraw, needs_clear) = self
            .last
            .as_ref()
            .map(|l| {
                let dim_changed = l.width != frame.width || l.height != frame.height;
                let sem_changed = l.semantic_gen != frame.semantic_gen;
                (dim_changed || sem_changed, dim_changed)
            })
            .unwrap_or((true, true));

        if needs_clear {
            // v16: If the frame has a bg color, set it BEFORE Clear(All)
            // so cleared cells get the correct bg. Without this, Clear(All)
            // fills with terminal default bg (None), creating visible gaps
            // at screen edges.
            if let Some(bg) = frame.blank.bg {
                self.stdout.queue(SetBackgroundColor(bg))?;
            }
            self.stdout.queue(crossterm_terminal::Clear(
                crossterm_terminal::ClearType::All,
            ))?;
        }

        let can_reuse_last = !needs_full_redraw && self.last.is_some();
        let total_cells = frame.width as usize * frame.height as usize;
        let dirty_count = frame.dirty_indices().len();
        let dirty_is_large =
            total_cells > 0 && dirty_count >= (total_cells / DIRTY_THRESHOLD_RATIO);
        let do_full_redraw = !can_reuse_last || frame.is_dirty_all() || dirty_is_large;

        // ── Idle-frame fast path (v30 Cosmic Dragon) ──
        //
        // Skip the entire render body when no cells changed this frame AND
        // the last frame is reusable (no dim/semantic change). Protects all
        // `term.draw()` callers (event loop, intro, future) from doing useless
        // work on idle frames — the event loop already gates on `did_draw`,
        // but intro.rs:490 calls `draw()` unconditionally.
        //
        // `clear_dirty()` is still called to advance `dirty_gen` — `set()`
        // compares `dirty_cell_gen[i] != dirty_gen` to decide whether to push
        // a cell. Without the gen bump, cells dirty in the last non-idle
        // frame would be incorrectly skipped on the next frame. See
        // cosmic_dragon_lock_tests.rs INV-17 for the contract test.
        if can_reuse_last && dirty_count == 0 && !frame.is_dirty_all() {
            frame.clear_dirty();
            return Ok(());
        }

        if do_full_redraw {
            let needs_new_last = self
                .last
                .as_ref()
                .map(|l| {
                    l.width != frame.width
                        || l.height != frame.height
                        || l.semantic_gen != frame.semantic_gen
                })
                .unwrap_or(true);
            if needs_new_last {
                // (perf polish): reuse the old LastFrame's Vec
                // allocation when the new dimensions fit. This avoids
                // heap churn during resize-drag storms where the user
                // overshoots and settles back to a smaller size.
                let old = self.last.take();
                self.last = Some(LastFrame::reuse_or_new(old, frame.width, frame.height));
            }
            let last = self.last.as_mut().expect("set above");
            // Synchronize semantic generation so future differential frames
            // don't spuriously re-trigger full redraws for this generation.
            last.semantic_gen = frame.semantic_gen;

            // PERF(v10): True single-pass RLE — accumulate characters into row_buf,
            // flush only when style actually changes.  Eliminates one
            // cell_at_index_ref(idx+1) generation-check per cell (~4800
            // calls on a 200×40 terminal per full redraw).
            let row_buf = &mut self.row_buf;
            let ansi_buf = &mut self.ansi_buf;
            row_buf.clear();
            ansi_buf.clear();
            // Pre-reserve if terminal grew since last frame
            let need_cap = frame.width as usize * 4;
            if row_buf.capacity() < need_cap {
                row_buf.reserve(need_cap - row_buf.capacity());
            }
            // MoveTo(0,0) directly into ansi_buf
            ansi_buf.extend_from_slice(b"\x1b[1;1H");
            for y in 0..frame.height {
                if y > 0 {
                    // MoveTo(0, y) directly into ansi_buf
                    ansi_buf.push(0x1b);
                    ansi_buf.push(b'[');
                    push_u16(ansi_buf, y + 1);
                    ansi_buf.extend_from_slice(b";1H");
                }
                row_buf.clear();
                let width_usize = frame.width as usize;
                for x in 0..frame.width {
                    let idx = y as usize * width_usize + x as usize;
                    let cell = frame.cell_at_index(idx);

                    // Flush row_buf on any style change
                    let style_changed =
                        cell.fg != cur_fg || cell.bg != cur_bg || cell.bold != cur_bold;
                    if style_changed && !row_buf.is_empty() {
                        ansi_buf.extend_from_slice(row_buf.as_bytes());
                        row_buf.clear();
                    }

                    // Combined fg+bg SGR — cached when possible
                    let color_changed = cell.fg != cur_fg || cell.bg != cur_bg;
                    if color_changed {
                        Self::emit_sgr(self.color_cache.as_ref(), ansi_buf, cell.fg, cell.bg);
                        cur_fg = cell.fg;
                        cur_bg = cell.bg;
                    }

                    if cell.bold != cur_bold {
                        // BOLT: branchless bold escape via table lookup.
                        // `cell.bold as usize` selects BOLD_ESCAPES[1] (ON,
                        // `\x1b[1m`, 4 bytes) or BOLD_ESCAPES[0] (OFF,
                        // `\x1b[22m`, 5 bytes). Compiles to `setne` on x86.
                        let bold_idx = cell.bold as usize;
                        let bold_len = BOLD_ESCAPE_LENS[bold_idx];
                        ansi_buf.extend_from_slice(&BOLD_ESCAPES[bold_idx][..bold_len]);
                        cur_bold = cell.bold;
                    }

                    row_buf.push(cell.ch);
                    last.cells[idx] = cell;
                }
                // Flush remaining cells in the row buffer
                if !row_buf.is_empty() {
                    ansi_buf.extend_from_slice(row_buf.as_bytes());
                }
            }

            // Reset attributes + flush all buffered ANSI bytes in one write_all.
            ansi_buf.extend_from_slice(b"\x1b[0m");
            self.flush_ansi()?;
            self.stdout.flush()?;

            frame.clear_dirty();
            return Ok(());
        }

        let last = self.last.as_mut().expect("checked above");

        let dirty = frame.dirty_indices();
        let width_usize = frame.width as usize;
        let height_usize = frame.height as usize;
        let run_buf = &mut self.run_buf;
        let ansi_buf = &mut self.ansi_buf;
        let cache_ref = self.color_cache.as_ref();
        ansi_buf.clear();

        // PERF: flat dirty-index buffer replaces the previous Vec<Vec<usize>>
        // nested structure. Collect all dirty indices into a single Vec,
        // sort once (row-major index sort groups by row AND orders within
        // row in one pass), then iterate contiguous runs. This eliminates
        // per-row Vec allocations on resize and improves cache locality.
        //
        // (perf audit): the previous `dirty_flat.extend(dirty.iter()
        // .copied().filter(|&idx| idx < height * width))` had an O(N) bounds
        // filter that ran every frame. The filter is redundant — every entry
        // in `frame.dirty_indices()` was pushed by `Frame::set()` /
        // `set_force()`, both of which call `self.index(x, y)` first and
        // only push `Some(i)` results. So every dirty index is already
        // guaranteed in-bounds.
        //
        // Replaced the filter with a `debug_assert!` that verifies the
        // invariant in debug builds (zero cost in release). If a future
        // caller bypasses `index()` and pushes an OOB index, the debug
        // build will catch it immediately instead of silently masking it.
        let dirty_flat = &mut self.dirty_flat;
        dirty_flat.clear();
        dirty_flat.extend(dirty.iter().copied());
        dirty_flat.sort_unstable();
        // (perf polish): the previous O(N) `dirty_flat.iter().all()`
        // checked every index every frame in debug builds (~4800
        // comparisons on a 200×40 terminal). Since `dirty_flat` is now
        // sorted ascending (we just called `sort_unstable()`), only the
        // LAST (largest) index needs to be checked — if it's in bounds,
        // all smaller indices are too. This drops the debug-build cost
        // from O(N) to O(1) per frame, with zero release-build impact
        // (debug_assert! is elided in release).
        //
        // SAFETY: `Frame::set()` / `set_force()` both call
        // `self.index(x, y)` first and only push `Some(i)` results,
        // so every dirty index is guaranteed in-bounds. This assert
        // catches the unlikely case where a future caller bypasses
        // `index()` and pushes an OOB index.
        debug_assert!(
            dirty_flat
                .last()
                .is_none_or(|&idx| idx < height_usize * width_usize),
            "dirty_indices must be in-bounds — Frame::set guarantees this"
        );

        // Iterate the flat sorted array, detecting row boundaries and
        // contiguous horizontal runs for RLE batching.
        // (bug #12): track the current row to force a MoveTo at
        // each row boundary. This prevents cursor desync where the terminal
        // autowraps or drifts at row boundaries (especially the bottom rows
        // where phosphor decay writes many ghost cells). Without this, a
        // single-cell "row shift right" glitch can appear transiently at
        // the bottom of the screen and self-correct only when the periodic
        // full-redraw kicks in (~5 minutes). Forcing MoveTo at each row
        // start costs ~6 bytes/row (negligible) and eliminates the desync.
        let mut i = 0usize;
        let mut last_row: u16 = u16::MAX;
        while i < dirty_flat.len() {
            let idx0 = dirty_flat[i];
            // Borrow instead of copy: compare with last frame without allocating.
            // Most dirty cells are unchanged (set to blank by tail pass);
            // this avoids copying ~24 bytes per Cell for early-exit.
            let cell0_ref = frame.cell_at_index_ref(idx0);
            // Cosmic Dragon egg #2: direct indexing — dirty_flat was filtered to
            // idx < height*width, so idx0 is guaranteed in bounds.
            // BEFORE: last.cells.get(idx0) == Some(cell0_ref)
            // AFTER:  &last.cells[idx0] == cell0_ref
            if &last.cells[idx0] == cell0_ref {
                i += 1;
                continue;
            }

            let cell0 = *cell0_ref;
            last.cells[idx0] = cell0;

            let x0 = (idx0 % width_usize) as u16;
            let y0 = (idx0 / width_usize) as u16;
            let fg0 = cell0.fg;
            let bg0 = cell0.bg;
            let bold0 = cell0.bold;

            // (bug #12): force cursor resync at each row boundary.
            // When we cross from one row to the next, invalidate cur_pos so
            // a MoveTo is always emitted for the first dirty cell in the
            // new row. This corrects any terminal-side autowrap or cursor
            // drift that accumulated during the previous row's run.
            if y0 != last_row {
                cur_pos = None;
                last_row = y0;
            }

            run_buf.clear();
            run_buf.push(cell0.ch);
            let mut run_len: u16 = 1;
            let mut last_idx_in_run = idx0;
            let mut j = i + 1;

            while j < dirty_flat.len() {
                let idx1 = dirty_flat[j];
                // Must be the next column on the same row (contiguous).
                if idx1 != last_idx_in_run + 1 {
                    break;
                }
                // Row boundary check: if we wrapped to the next row, the
                // x-coordinate resets, so the run must flush here.
                if idx1 / width_usize != idx0 / width_usize {
                    break;
                }

                let cell1_ref = frame.cell_at_index_ref(idx1);
                // Cosmic Dragon egg #3: direct indexing — idx1 from dirty_flat (filtered).
                if &last.cells[idx1] == cell1_ref {
                    break;
                }
                if cell1_ref.fg != fg0 || cell1_ref.bg != bg0 || cell1_ref.bold != bold0 {
                    break;
                }

                run_buf.push(cell1_ref.ch);
                let cell1 = *cell1_ref;
                last.cells[idx1] = cell1;
                run_len = run_len.saturating_add(1);
                last_idx_in_run = idx1;
                j += 1;
            }

            if cur_pos != Some((x0, y0)) {
                // MoveTo(x0, y0) directly into ansi_buf
                ansi_buf.push(0x1b);
                ansi_buf.push(b'[');
                push_u16(ansi_buf, y0 + 1);
                ansi_buf.push(b';');
                push_u16(ansi_buf, x0 + 1);
                ansi_buf.push(b'H');
            }

            // Combined fg+bg SGR — cached when possible
            let style_changed = fg0 != cur_fg || bg0 != cur_bg;
            if style_changed {
                Self::emit_sgr(cache_ref, ansi_buf, fg0, bg0);
                cur_fg = fg0;
                cur_bg = bg0;
            }

            if bold0 != cur_bold {
                // BOLT: branchless bold escape via table lookup (see above).
                let bold_idx = bold0 as usize;
                let bold_len = BOLD_ESCAPE_LENS[bold_idx];
                ansi_buf.extend_from_slice(&BOLD_ESCAPES[bold_idx][..bold_len]);
                cur_bold = bold0;
            }

            // Print run directly into ANSI buffer (UTF-8 bytes).
            ansi_buf.extend_from_slice(run_buf.as_bytes());
            let next_x = x0.saturating_add(run_len);
            cur_pos = if next_x < frame.width {
                Some((next_x, y0))
            } else {
                None
            };

            i = j;
        }

        // Reset attributes + flush all buffered ANSI bytes in one write_all.
        ansi_buf.extend_from_slice(b"\x1b[0m");
        self.flush_ansi()?;
        self.stdout.flush()?;
        frame.clear_dirty();
        Ok(())
    }
}
