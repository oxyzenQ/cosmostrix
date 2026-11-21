// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! The differential-frame emission core (night-frontier-Z9-C).
//!
//! The steady-state diff solver lived fused inside [`super::Terminal::draw`]
//! behind a live stdout (`BufWriter<Stdout>` + `enable_raw_mode`), so no
//! test could execute the exact shipped algorithm — the emission
//! claim in the terminal module docs ("minimal cursor movement and
//! style-change overhead") was prose, not a provable contract. This
//! module is the certificate's seam: the diff-path body moved here
//! VERBATIM from `draw.rs` (same statements, same order, same bytes —
//! only `self.field` accesses became parameters and byte-class counters
//! were added, which never touch the emitted stream), and `draw.rs`
//! calls it. [`encode_diff_emission`] is pure computation over its
//! inputs: no I/O, no terminal, no clock — the emission-minimality
//! certificate (INNOVATIONS entry 57) drives THIS function — the code
//! the binary actually ships — against an independent law-DP reference
//! solver on adversarial grids, per push.
//!
//! The named law set this solver optimizes within (each law is a
//! correctness contract, not a perf choice):
//! 1. no overpaint — a cell whose content already matches the screen
//!    is never emitted (the HUNT-27 invisibility law: an idle resync
//!    must stay byte-silent);
//! 2. row-boundary cursor resync — the cursor position is dropped at
//!    every row change (the bug-12 law: terminal autowrap/drift must
//!    never desync the diff path);
//! 3. the bridge grammar — a style change bridges through the
//!    pre-formatted SGR forms (`write_sgr_colors_buf` + cache, the
//!    task-17 emission-boundary quantizer) and the BOLT bold escapes.

use crossterm::style::Color;

use crate::bolt::{BOLD_ESCAPE_LENS, BOLD_ESCAPES};
use crate::cell::Cell;
use crate::color_cache::ColorCache;
use crate::frame::Frame;
use crate::palette::SgrQuantizer;
use crate::sgr_format::{push_u16, write_sgr_colors_buf};

/// Byte-class accounting of one differential frame emission.
///
/// The classes are exhaustive by construction and sum to [`Self::bytes`]
/// (the emission ledger asserts the identity every frame): the payload
/// (the changed cells' UTF-8 glyphs) plus the control furniture (cursor
/// addresses, style bridges, bold toggles, the trailing reset).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DiffEmissionStats {
    /// Total bytes appended to `out` for this frame, including the
    /// trailing `\x1b[0m` reset.
    pub bytes: usize,
    /// UTF-8 glyph bytes painted — the payload.
    pub payload_bytes: usize,
    /// Cursor-address (`ESC[y;xH`) sequences emitted.
    pub moves: usize,
    /// Combined fg+bg SGR bridges emitted.
    pub sgrs: usize,
    /// Bold toggle escapes emitted.
    pub bolds: usize,
}

/// Emit SGR color bytes for (fg, bg) into the ANSI buffer.
/// Uses the color cache when available, falling back to on-the-fly
/// formatting via `write_sgr_colors_buf`.
///
/// Moved verbatim from `Terminal::emit_sgr` (terminal/mod.rs) so both
/// emission paths share one grammar. task-17: when a boundary quantizer
/// is present (non-truecolor session), (fg, bg) are quantized into the
/// session's wire space FIRST — the cache lookup then matches
/// wire-space palette entries and the fallback formats already-quantized
/// colors, so no `38;2` truecolor sequence can leave the boundary on a
/// Color16/Color256/Mono session.
#[inline]
pub(super) fn emit_sgr(
    quant: Option<&mut SgrQuantizer>,
    cache: Option<&ColorCache>,
    buf: &mut Vec<u8>,
    fg: Option<Color>,
    bg: Option<Color>,
) {
    let (fg, bg) = match quant {
        Some(q) => (q.quantize_fg(fg), q.quantize_bg(bg)),
        None => (fg, bg),
    };
    if let Some(cache) = cache
        && let Some(cached) = cache.sgr_for_cell(fg, bg)
    {
        buf.extend_from_slice(cached);
        return;
    }
    write_sgr_colors_buf(buf, fg, bg);
}

/// Encode the differential emission for `frame` against the previous
/// screen (`last_cells`) into `out` — the shipped steady-state solver.
///
/// Moved verbatim from `Terminal::draw`'s differential branch (see the
/// module docs for the extraction contract). The caller keeps the flush
/// (`flush_ansi` + `flush_stdout_timed`) and the dirty-clear; this
/// function owns the byte stream: the flat sorted dirty iteration, the
/// unchanged-cell skip, the contiguous same-style runs, the deferred
/// `MoveTo`, the cached SGR bridges, the BOLT bold toggles, and the
/// trailing attribute reset.
pub(crate) fn encode_diff_emission(
    frame: &Frame,
    last_cells: &mut [Cell],
    mut quantizer: Option<&mut SgrQuantizer>,
    cache: Option<&ColorCache>,
    out: &mut Vec<u8>,
    run_buf: &mut String,
    dirty_flat: &mut Vec<usize>,
) -> DiffEmissionStats {
    let mut stats = DiffEmissionStats::default();

    let mut cur_fg: Option<Color> = None;
    let mut cur_bg: Option<Color> = None;
    let mut cur_bold: bool = false;
    let mut cur_pos: Option<(u16, u16)> = None;

    let last = last_cells;

    let width_usize = frame.width as usize;
    let height_usize = frame.height as usize;
    let cache_ref = cache;
    out.clear();

    // PERF: flat dirty-index buffer replaces the previous Vec<Vec<usize>>
    // nested structure. Collect all dirty indices into a single Vec,
    // sort once (row-major index sort groups by row AND orders within
    // row in one pass), then iterate contiguous runs. This eliminates
    // per-row Vec allocations on resize and improves cache locality.
    //
    // (perf audit): the previous O(N) bounds filter ran every frame and
    // was redundant — every entry in `frame.dirty_indices()` was pushed
    // by `Frame::set()` / `set_force()`, both of which call
    // `self.index(x, y)` first and only push `Some(i)` results. The
    // invariant is asserted in debug builds only (the LAST index gates
    // the whole array — the array is sorted ascending).
    dirty_flat.clear();
    dirty_flat.extend(frame.dirty_indices().iter().copied());
    dirty_flat.sort_unstable();
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
        if &last[idx0] == cell0_ref {
            i += 1;
            continue;
        }

        let cell0 = *cell0_ref;
        last[idx0] = cell0;

        let x0 = (idx0 % width_usize) as u16;
        let y0 = (idx0 / width_usize) as u16;
        let fg0 = cell0.fg;
        let bg0 = cell0.bg;
        let bold0 = cell0.bold;

        // (bug #12): force cursor resync at each row boundary.
        if y0 != last_row {
            cur_pos = None;
            last_row = y0;
        }

        run_buf.clear();
        run_buf.push(cell0.ch);
        stats.payload_bytes += cell0.ch.len_utf8();
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
            if &last[idx1] == cell1_ref {
                break;
            }
            if cell1_ref.fg != fg0 || cell1_ref.bg != bg0 || cell1_ref.bold != bold0 {
                break;
            }

            run_buf.push(cell1_ref.ch);
            stats.payload_bytes += cell1_ref.ch.len_utf8();
            let cell1 = *cell1_ref;
            last[idx1] = cell1;
            run_len = run_len.saturating_add(1);
            last_idx_in_run = idx1;
            j += 1;
        }

        if cur_pos != Some((x0, y0)) {
            // MoveTo(x0, y0) directly into out
            out.push(0x1b);
            out.push(b'[');
            push_u16(out, y0 + 1);
            out.push(b';');
            push_u16(out, x0 + 1);
            out.push(b'H');
            stats.moves += 1;
        }

        // Combined fg+bg SGR — cached when possible
        // task-17: quantized at the emission boundary when the
        // session is not truecolor (see emit_sgr).
        let style_changed = fg0 != cur_fg || bg0 != cur_bg;
        if style_changed {
            emit_sgr(quantizer.as_deref_mut(), cache_ref, out, fg0, bg0);
            cur_fg = fg0;
            cur_bg = bg0;
            stats.sgrs += 1;
        }

        if bold0 != cur_bold {
            // BOLT: branchless bold escape via table lookup (see above).
            let bold_idx = bold0 as usize;
            let bold_len = BOLD_ESCAPE_LENS[bold_idx];
            out.extend_from_slice(&BOLD_ESCAPES[bold_idx][..bold_len]);
            cur_bold = bold0;
            stats.bolds += 1;
        }

        // Print run directly into the ANSI buffer (UTF-8 bytes).
        out.extend_from_slice(run_buf.as_bytes());
        let next_x = x0.saturating_add(run_len);
        cur_pos = if next_x < frame.width {
            Some((next_x, y0))
        } else {
            None
        };

        i = j;
    }

    // Reset attributes; the caller flushes all buffered ANSI bytes in
    // one write_all (the syscall-floor byte-identity counts exactly
    // these queued bytes).
    out.extend_from_slice(b"\x1b[0m");
    stats.bytes = out.len();
    stats
}
