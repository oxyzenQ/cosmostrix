// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! Differential + full-redraw ANSI render path for [`Terminal`].
//!
//! Extracted from `terminal/mod.rs` in the dragon-fight branch to keep the
//! main module under the 800-LOC cap and isolate the perf-critical render
//! path. This is the single hottest function in cosmostrix — every frame
//! goes through `Terminal::draw`.
//!
//! ## Two paths
//!
//! 1. Full redraw (≥12.5% cells dirty, dim/semantic change, or an
//!    unknown shadow — see `force_full_emit`): row-RLE
//!    pass over all cells, accumulates into `ansi_buf`, one `write_all`.
//! 2. Differential (<12.5% cells dirty): flat sorted dirty-index
//!    iteration, contiguous-run detection, per-run MoveTo + SGR batching.
//!
//! The crossover threshold is `DIRTY_THRESHOLD_RATIO` (currently 8 = 12.5%).
//! See `cosmic_dragon::egg::threshold_sweep` for the benchmark that tuned it.

use std::io::{Result, Write};

use crossterm::style::Color;

use crate::bolt::{BOLD_ESCAPE_LENS, BOLD_ESCAPES};
use crate::constants::DIRTY_THRESHOLD_RATIO;
use crate::frame::Frame;
use crate::sgr_format::{push_u16, write_sgr_colors_buf};

use super::emit_diff::emit_sgr;
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
            // Scrollback-safe: avoid Clear(ClearType::All) inside the
            // alternate screen — \x1b[2J can set an internal flag on
            // VTE-based terminals that clears the main screen's scrollback
            // when LeaveAlternateScreen is later called. Instead, set the
            // bg color and let the full redraw below overwrite every cell.
            // (do_full_redraw will be true when needs_clear is true, so
            // every cell gets written regardless.)
            if let Some(bg) = frame.blank.bg {
                // task-17 contract extended to the canvas paint
                // (night-frontier-Z3a deepening): the blanket bg must
                // ride the same emission-boundary quantizer the cells
                // ride. The deepened bg-form probe caught one indexed
                // 48;5 SGR per session leaking through this path on
                // dumb/16 lanes while the fg side was quantized - the
                // blanket was the last raw crossterm color emission.
                // None quantizer = truecolor session: the raw color is
                // already the session's wire space. Mono mode resolves
                // the bg to Reset (default background - never paint on
                // dumb terminals), Classic16 resolves it to a named
                // classic color.
                let bg = match self.sgr_quantizer.as_mut() {
                    Some(q) => q.quantize_bg(Some(bg)),
                    None => Some(bg),
                };
                if let Some(bg) = bg {
                    // The blanket rides the repo's own classic-code
                    // writer, NOT crossterm's Display: crossterm renders
                    // NAMED colors in the indexed 48;5;N form (Black bg
                    // = ESC[48;5;0m), which the deepened bg-form probe
                    // caught as one indexed SGR per non-truecolor
                    // session even after the quantizer armed correctly
                    // (the debug trace proved it: armed=true, bg=Black,
                    // and the wire still carried 48;5;0m). The cells
                    // ride sgr_format's classic wire (task-17); the
                    // canvas paint rides the same wire. fg rides 39
                    // (default) - the byte shape of the cache's reset
                    // entry. The 16-byte scratch Vec replaces the old
                    // to_string() allocation, same once-per-needs_clear
                    // window, and total_ansi_bytes counts the exact
                    // queued bytes so the syscall-floor byte-identity
                    // row stays kernel-exact.
                    let mut buf = Vec::with_capacity(16);
                    write_sgr_colors_buf(&mut buf, None, Some(bg));
                    self.stdout.write_all(&buf)?;
                    self.total_ansi_bytes += buf.len() as u64;
                }
            }
        }

        let can_reuse_last = !needs_full_redraw && self.last.is_some();
        // NIGHT-hunter-34: a freshly (re)initialized shadow knows nothing
        // about the physical screen — force the full-redraw path so the
        // cell-skip below cannot treat "frame blank == shadow blank" as
        // "nothing to emit" while the terminal still shows residue.
        // (Needs_full_redraw already covers the reset triggers — this OR
        // keeps the invariant airtight even if a write error interrupts
        // a draw after the shadow reset but before the flag is cleared.)
        let force_emit_all = self.last.as_ref().is_some_and(|l| l.force_full_emit);
        let total_cells = frame.width as usize * frame.height as usize;
        let dirty_count = frame.dirty_indices().len();
        let dirty_is_large =
            total_cells > 0 && dirty_count >= (total_cells / DIRTY_THRESHOLD_RATIO);
        let do_full_redraw =
            !can_reuse_last || frame.is_dirty_all() || dirty_is_large || force_emit_all;

        // ── Idle-frame fast path (v30 Cosmic Dragon) ──
        //
        // Skip the entire render body when no cells changed this frame AND
        // the last frame is reusable (no dim/semantic change). Protects all
        // `term.draw()` callers (event loop, intro, future) from doing useless
        // work on idle frames — the event loop already gates on `did_draw`,
        // but the intro's `end_frame` (intro_style/mod.rs) calls `draw()`
        // unconditionally.
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
            // NIGHT-hunter-34: re-read the flag AFTER the potential reset
            // above — reuse_or_new arms it on the fresh shadow, and the
            // pre-reset read (the do_full_redraw OR) only covers the
            // preserved-shadow case (a write error interrupted a previous
            // draw between the reset and the flag clear).
            let force_emit_all = force_emit_all || last.force_full_emit;

            // S-master-HUNT-27: cell-level skip in the full-redraw path.
            //
            // Previously this loop emitted EVERY cell unconditionally
            // (SGR + char for each of the WxH cells). On an idle resync
            // (dirty_all set by force_repaint, no actual content change),
            // this re-emitted the entire screen as a ~50KB ANSI burst
            // every 20s — the owner-reported "glitch rain shift" that
            // survived 4 fix attempts (HUNT-23 through HUNT-26) because
            // all of them fixed cloud-side state but never touched this
            // emit path.
            //
            // The fix: compare each cell against last.cells[idx] (the
            // previous frame's screen state). Unchanged cells are
            // skipped entirely — no SGR, no char, no row_buf push. The
            // MoveTo is deferred until the first changed cell in each
            // row, so a row with zero changes emits zero bytes. On a
            // stable screen (the idle resync case), the entire full
            // redraw emits zero content bytes (just the final \x1b[0m
            // reset), making the resync truly invisible.
            //
            // When a skip breaks the contiguous char run, row_buf is
            // flushed (accumulated chars emitted at the cursor's natural
            // position), and the next changed cell emits a MoveTo to
            // reposition. This preserves the RLE-style char accumulation
            // for runs of changed cells while skipping unchanged ones.
            //
            // First-full-redraw safety: when needs_new_last is true
            // (resize/semantic change), last.cells is freshly allocated
            // with blank cells. Every non-blank cell is "changed", so
            // no skip happens — the first full redraw emits everything
            // as before.
            let row_buf = &mut self.row_buf;
            let ansi_buf = &mut self.ansi_buf;
            row_buf.clear();
            ansi_buf.clear();
            // Pre-reserve if terminal grew since last frame
            let need_cap = frame.width as usize * 4;
            if row_buf.capacity() < need_cap {
                row_buf.reserve(need_cap - row_buf.capacity());
            }

            let width_usize = frame.width as usize;
            // NIGHT-ultimate-1: the diff path debug-asserts the largest
            // dirty index (below); the full-redraw path indexes
            // `last.cells[idx]` per cell with only an invariant comment.
            // Promote that invariant to a debug_assert so debug builds
            // catch a resize that reallocates `frame` without
            // reallocating `last` (the same class of drift the diff-path
            // assert guards). O(1), elided in release.
            debug_assert!(
                last.cells.len() == width_usize * frame.height as usize,
                "last_frame dims must match frame dims — resize reallocates both"
            );
            for y in 0..frame.height {
                row_buf.clear();
                // HUNT-27: defer MoveTo until the first changed cell.
                // need_move starts true so the first changed cell (if any)
                // positions the cursor. A row with zero changed cells
                // emits zero bytes.
                let mut need_move = true;

                for x in 0..frame.width {
                    let idx = y as usize * width_usize + x as usize;
                    let cell = frame.cell_at_index(idx);

                    // HUNT-27: skip unchanged cells — EXCEPT while the
                    // shadow is in the unknown state (NIGHT-hunter-34:
                    // fresh reset claims blank bg=None cells, but the
                    // physical screen can hold intro rain, old-scene
                    // glyphs, or a black/custom background fill; with
                    // color-bg=default-background the skip left them
                    // on screen forever). force_emit_all emits every
                    // cell once, then the shadow is exact again.
                    if !force_emit_all && cell == last.cells[idx] {
                        // Flush accumulated row_buf before the skip —
                        // the contiguous char run ends here, and the
                        // cursor won't advance past this cell.
                        if !row_buf.is_empty() {
                            ansi_buf.extend_from_slice(row_buf.as_bytes());
                            row_buf.clear();
                        }
                        need_move = true;
                        continue;
                    }

                    // Changed cell — emit MoveTo if needed (first
                    // changed cell in the row, or after a skip).
                    if need_move {
                        ansi_buf.push(0x1b);
                        ansi_buf.push(b'[');
                        push_u16(ansi_buf, y + 1);
                        ansi_buf.push(b';');
                        push_u16(ansi_buf, x + 1);
                        ansi_buf.push(b'H');
                        need_move = false;
                    }

                    // Flush row_buf on any style change
                    let style_changed =
                        cell.fg != cur_fg || cell.bg != cur_bg || cell.bold != cur_bold;
                    if style_changed && !row_buf.is_empty() {
                        ansi_buf.extend_from_slice(row_buf.as_bytes());
                        row_buf.clear();
                    }

                    // Combined fg+bg SGR — cached when possible
                    // task-17: quantized at the emission boundary when the
                    // session is not truecolor (see emit_sgr).
                    let color_changed = cell.fg != cur_fg || cell.bg != cur_bg;
                    if color_changed {
                        emit_sgr(
                            self.sgr_quantizer.as_mut(),
                            self.color_cache.as_ref(),
                            ansi_buf,
                            cell.fg,
                            cell.bg,
                        );
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
            // HUNT-23: flush through flush_stdout_timed so the (potentially
            // blocking) syscall latency feeds last_write_ns.
            ansi_buf.extend_from_slice(b"\x1b[0m");
            // NIGHT-hunter-34: every cell was compared against (and, when
            // needed, emitted into) the shadow this pass — the physical
            // screen state is known again. Clear the unknown flag so the
            // HUNT-27 skip resumes on the next full redraw.
            last.force_full_emit = false;
            self.flush_ansi()?;
            self.flush_stdout_timed()?;

            frame.clear_dirty();
            return Ok(());
        }

        let last = self.last.as_mut().expect("checked above");

        // night-frontier-Z9-C: the differential body moved VERBATIM to
        // `emit_diff::encode_diff_emission` — the emission-minimality
        // certificate's seam. It was previously fused behind a live
        // stdout here, so no test could execute the exact shipped
        // solver; now the certificate drives this exact code against an
        // independent law-DP reference per push. Byte-for-byte behavior
        // is preserved: same statements, same order, and the debug
        // assert below ties the returned accounting to the queued
        // bytes on every debug frame.
        let stats = super::emit_diff::encode_diff_emission(
            frame,
            &mut last.cells,
            self.sgr_quantizer.as_mut(),
            self.color_cache.as_ref(),
            &mut self.ansi_buf,
            &mut self.run_buf,
            &mut self.dirty_flat,
        );
        debug_assert_eq!(
            stats.bytes,
            self.ansi_buf.len(),
            "the diff emission accounting must equal the queued bytes"
        );

        // Flush all buffered ANSI bytes in one write_all (the reset
        // rides inside the encoded frame; see emit_diff). HUNT-23:
        // flush through flush_stdout_timed so the (potentially blocking)
        // syscall latency feeds last_write_ns.
        self.flush_ansi()?;
        self.flush_stdout_timed()?;
        frame.clear_dirty();
        Ok(())
    }
}
