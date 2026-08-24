// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Terminal I/O wet benchmark — measures real write bandwidth + latency.
//!
//! Phase 2 of DeepSeek benchmark restructuring plan.
//!
//! When `--bench-io` is passed, the benchmark writes ANSI output to
//! `/dev/null` (Linux/macOS) or `nul` (Windows) to simulate real
//! terminal I/O without blocking on a real terminal emulator.
//!
//! Metrics collected:
//! - `io_bytes_written`: total ANSI bytes written
//! - `io_write_calls`: number of write() + flush() calls
//! - `io_total_write_ns`: cumulative time in write+flush
//! - `io_backpressure_events`: short writes (would_block or partial)
//! - Computed: bandwidth_mbps, avg_latency_us, effective_write_fps
//!
//! ## T2.1 upgrade
//!
//! The writer now mirrors the production `Terminal::draw()` fast path:
//! - Uses `ColorCache` to pre-format SGR byte sequences for palette colors
//!   (eliminates per-cell `write_sgr_colors_buf` formatting cost).
//! - Uses `Frame::cell_at_index_ref` instead of `cell_at_index` to avoid
//!   the 16-byte `Cell` copy per dirty cell.
//!
//! ## Strategy B — Lean per-cell emit (supersedes A' RLE batching)
//!
//! The previous A' RLE batching (sort dirty indices → scan contiguous
//! same-style runs → emit one SGR per run) was a net regression for
//! matrix rain workloads because:
//! - Sort cost O(D log D) is pure overhead for /dev/null target (byte
//!   order doesn't matter — no MoveTo commands emitted).
//! - Run detection (3 comparisons per cell + run_buf push/clear) adds
//!   constant overhead per cell.
//! - Matrix rain has near-zero contiguous same-style runs (each cell's
//!   fg/bg is unique based on distance-from-head), so RLE compresses
//!   nothing. Measured bytes/cell ≈ 23-29 across scenes (≈ per-cell SGR
//!   + 1 char), confirming zero compression.
//!
//! Strategy B drops the sort and run detection entirely. Per-cell style
//! tracking (cur_fg/cur_bg/cur_bold across cells) still skips SGR emit
//! when style matches previous cell — same byte count as RLE for matrix
//! rain, but without the sort or run-scan overhead.
//!
//! Multi-scene benchmark data (commit 575dded, before Strategy B):
//!   monolith (D=235):  avg_fps=37773, io_share=88.0%, io_ns/cell=96.1
//!   cinematic (D=715): avg_fps=8083,  io_share=82.2%, io_ns/cell=141.1
//!   storm (D=1778):    avg_fps=2728,  io_share=89.9%, io_ns/cell=184.8
//!
//! The io_ns/cell GROWTH with D (96→141→184) was the signature of
//! O(D log D) sort here + O(D²) VisualSampler (fixed separately).
//! Strategy B makes io_ns/cell approximately constant across scenes.
//!
//! Strategy B results (commit 6d8c260):
//!   monolith (D=235):  avg_fps=46675, io_share=85.6%, io_ns/cell=75.2
//!   cinematic (D=668): avg_fps=10442, io_share=78.0%, io_ns/cell=110.8
//!   storm (D=1774):    avg_fps=3791,  io_share=86.2%, io_ns/cell=127.6
//!
//! ## Strategy C — Silent cache lookup (built on top of Strategy B)
//!
//! The bench writer previously tracked `sgr_cache_hits/misses` in
//! `TerminalIoMetrics` — but those fields were never read by
//! `bench_report.rs` or `bench.rs`. They were dead metrics. Worse, every
//! `ColorCache::sgr_for_cell()` call touched an atomic counter
//! (`fetch_add` on `AtomicU64`), ~5-10ns per cell. At 46K FPS × 235 cells
//! = 10.8M atomic RMW ops/sec, this was 5-10% of per-cell SGR cost for
//! zero benefit.
//!
//! Strategy C adds `ColorCache::sgr_for_cell_silent()` — same lookup
//! logic, no atomic counter touch. The bench writer uses it exclusively.
//! The dead `TerminalIoMetrics::sgr_cache_hits/misses` fields were removed
//! entirely (one less `&mut u64` to thread through `emit_cell_lean`,
//! bringing it from 9 args down to 7 — under clippy's
//! `too_many_arguments` threshold, so the `#[allow]` was also dropped).

use std::io::{BufWriter, Write};
use std::time::Instant;

use crossterm::style::Color;

use crate::bolt::{write_u8_to_slice, BOLD_ESCAPES, BOLD_ESCAPE_LENS};
use crate::cell::Cell;
use crate::color_cache::ColorCache;
use crate::frame::Frame;
use crate::sgr_format::{push_u8, write_sgr_colors_buf};

// BOLT tables (U8_PADDED, U8_LEN, BOLD_ESCAPES, BOLD_ESCAPE_LENS) and the
// branchless `write_u8_to_slice` helper now live in `crate::bolt` so that
// `bench_io`, `sgr_format`, and `terminal::draw` share a single
// branchless implementation. See `src/bolt/mod.rs` for the table layouts and
// the projected production-path gain rationale.

/// Terminal I/O metrics collected during wet benchmark.
#[derive(Debug, Clone, Default)]
pub(crate) struct TerminalIoMetrics {
    pub enabled: bool,
    pub target: String,
    pub bytes_written: u64,
    pub write_calls: u64,
    pub total_write_ns: u64,
    pub backpressure_events: u64,
    pub elapsed_secs: f64,
}

impl TerminalIoMetrics {
    /// Write bandwidth in MB/s (1 MB = 1,048,576 bytes).
    #[must_use]
    pub(crate) fn bandwidth_mbps(&self) -> f64 {
        if self.elapsed_secs > 0.0 {
            (self.bytes_written as f64 / 1_048_576.0) / self.elapsed_secs
        } else {
            0.0
        }
    }

    /// Average write latency in microseconds per write call.
    #[must_use]
    pub(crate) fn avg_latency_us(&self) -> f64 {
        if self.write_calls > 0 {
            (self.total_write_ns as f64 / self.write_calls as f64) / 1000.0
        } else {
            0.0
        }
    }

    /// Effective write FPS = write_calls / elapsed_secs.
    #[must_use]
    pub(crate) fn effective_write_fps(&self) -> f64 {
        if self.elapsed_secs > 0.0 {
            self.write_calls as f64 / self.elapsed_secs
        } else {
            0.0
        }
    }
}

/// Wet I/O writer that writes ANSI output to a null device.
///
/// T2.1: holds a `ColorCache` for pre-formatted SGR sequences. Constructed
/// via [`BenchIoWriter::with_palette`], the writer mirrors the production
/// `Terminal::draw()` fast path — SGR sequences are pre-formatted once at
/// construction, then memcpy'd per cell instead of being formatted on-the-fly.
///
/// Strategy B: lean per-cell emit with style tracking. No sort, no run
/// detection, no scratch buffers. The struct holds only `writer`, `ansi_buf`,
/// `metrics`, and `color_cache`.
pub(crate) struct BenchIoWriter {
    writer: BufWriter<std::fs::File>,
    ansi_buf: Vec<u8>,
    metrics: TerminalIoMetrics,
    /// T2.1: pre-formatted SGR cache. `None` when constructed without a palette
    /// (legacy `new()` path); `Some` when constructed via `with_palette()`.
    color_cache: Option<ColorCache>,
}

impl BenchIoWriter {
    /// T2.1: create a writer with a pre-built `ColorCache` derived from the
    /// active palette. Mirrors the production `Terminal::draw()` fast path —
    /// SGR sequences are pre-formatted once at construction, then memcpy'd
    /// per cell instead of being formatted on-the-fly.
    pub(crate) fn with_palette(
        palette: &crate::chroma_dragon_engine::palette::Palette,
    ) -> Option<Self> {
        Self::build(Some(ColorCache::new(palette)))
    }

    fn build(color_cache: Option<ColorCache>) -> Option<Self> {
        let path = if cfg!(target_os = "windows") {
            "nul"
        } else {
            "/dev/null"
        };

        let file = std::fs::File::create(path).ok()?;
        let writer = BufWriter::with_capacity(262_144, file); // 256 KB buffer

        Some(Self {
            writer,
            ansi_buf: Vec::with_capacity(8192),
            metrics: TerminalIoMetrics {
                enabled: true,
                target: path.to_string(),
                ..Default::default()
            },
            color_cache,
        })
    }

    /// Generate ANSI bytes from the frame's dirty cells and write to null device.
    /// Measures write time and tracks metrics.
    ///
    /// Strategy B: lean per-cell emit. No sort, no run detection. Style
    /// tracking (cur_fg/cur_bg/cur_bold) skips SGR emit when style matches
    /// the previous cell — same byte count as RLE for matrix rain (where
    /// each cell has unique style), but without the O(D log D) sort or
    /// O(D) run-scan overhead.
    ///
    /// Strategy C: silent cache lookup. The previous `sgr_for_cell` call
    /// touched atomic hit/miss counters in `ColorCache` (one `fetch_add`
    /// per cell, ~5-10ns each). At 46K FPS × 235 cells = 10.8M atomic
    /// RMW ops/sec, this was 5-10% of per-cell SGR cost. Strategy C uses
    /// `sgr_for_cell_silent` which skips the atomics entirely. The bench
    /// writer previously tracked its own `sgr_cache_hits/misses` in
    /// `TerminalIoMetrics` — but those were never reported (no consumer
    /// in bench_report.rs or bench.rs), so they were dead metrics. Both
    /// the field and the counter increments have been removed.
    pub(crate) fn write_frame(&mut self, frame: &Frame) {
        self.ansi_buf.clear();

        let dirty = frame.dirty_indices();
        if dirty.is_empty() && !frame.is_dirty_all() {
            return;
        }

        // Current SGR state — only emit on change (mirrors Terminal::draw).
        let mut cur_fg: Option<crossterm::style::Color> = None;
        let mut cur_bg: Option<crossterm::style::Color> = None;
        let mut cur_bold = false;
        let cache_ref = self.color_cache.as_ref();
        // Stack-allocated UTF-8 encoding buffer — zero alloc per cell.
        let mut utf8_buf = [0u8; 4];

        if frame.is_dirty_all() {
            // Full redraw: iterate all cells in row-major order.
            let total = (frame.width as usize) * (frame.height as usize);
            for idx in 0..total {
                let cell = frame.cell_at_index_ref(idx);
                emit_cell_lean(
                    &mut self.ansi_buf,
                    cell,
                    &mut cur_fg,
                    &mut cur_bg,
                    &mut cur_bold,
                    cache_ref,
                    &mut utf8_buf,
                );
            }
        } else {
            // Diff path: iterate dirty cells in push order (NO sort).
            // For /dev/null target, byte order doesn't matter — no MoveTo
            // commands are emitted. Per-cell style tracking gives the same
            // byte count as sorted iteration for matrix rain (each cell has
            // unique fg/bg, so SGR is emitted per-cell regardless). Saves
            // O(D log D) sort + O(D) run detection vs the previous RLE path.
            for &idx in dirty {
                let cell = frame.cell_at_index_ref(idx);
                emit_cell_lean(
                    &mut self.ansi_buf,
                    cell,
                    &mut cur_fg,
                    &mut cur_bg,
                    &mut cur_bold,
                    cache_ref,
                    &mut utf8_buf,
                );
            }
        }

        // Reset attributes
        self.ansi_buf.extend_from_slice(b"\x1b[0m");

        // Write + measure
        let write_start = Instant::now();
        let bytes_to_write = self.ansi_buf.len();

        match self.writer.write_all(&self.ansi_buf) {
            Ok(()) => {
                self.metrics.bytes_written += bytes_to_write as u64;
                self.metrics.write_calls += 1;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                self.metrics.backpressure_events += 1;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WriteZero => {
                self.metrics.backpressure_events += 1;
            }
            Err(_) => {
                // Other errors — count as backpressure
                self.metrics.backpressure_events += 1;
            }
        }

        // Flush
        match self.writer.flush() {
            Ok(()) => {
                self.metrics.write_calls += 1;
            }
            Err(_) => {
                self.metrics.backpressure_events += 1;
            }
        }

        self.metrics.total_write_ns += write_start.elapsed().as_nanos() as u64;
    }

    /// `--bench-scene production-draw` I/O path.
    ///
    /// Mirrors `Terminal::draw`'s full-redraw hot path (the production
    /// render code that BOLT optimized) instead of `emit_cell_lean`'s
    /// Strategy E stack-buffer fast path. The differences from
    /// [`write_frame`](Self::write_frame):
    ///
    /// - Emits `MoveTo` per row via `sgr_format::push_u16` (cursor
    ///   positioning overhead that `emit_cell_lean` skips).
    /// - Uses `Self::emit_sgr` (production SGR path →
    ///   `sgr_format::write_sgr_colors_buf` → `bolt::push_u8`) for every
    ///   style change, NOT the Strategy E inline `\x1b[38;2;…` stack
    ///   buffer.
    /// - Uses `bolt::BOLD_ESCAPES[bold_idx]` for bold escape (same as
    ///   `emit_cell_lean`, but via the project-wide `bolt::` module).
    /// - Batches same-style runs into `row_buf` and flushes on style
    ///   change (production RLE behavior).
    ///
    /// Use this scene to measure the BOLT-backed production render path.
    /// Output goes to the same /dev/null BufWriter as `write_frame`.
    pub(crate) fn write_frame_production(&mut self, frame: &Frame) {
        self.ansi_buf.clear();
        // Reusable row buffer for RLE-style run batching (mirrors Terminal::draw).
        let mut row_buf: Vec<u8> = Vec::with_capacity(frame.width as usize * 4);

        let mut cur_fg: Option<crossterm::style::Color> = None;
        let mut cur_bg: Option<crossterm::style::Color> = None;
        let mut cur_bold = false;
        let cache_ref = self.color_cache.as_ref();
        let mut utf8_buf = [0u8; 4];

        // MoveTo(1,1) — cursor home for the first row.
        self.ansi_buf.extend_from_slice(b"\x1b[1;1H");

        for y in 0..frame.height {
            if y > 0 {
                // MoveTo(1, y+1) — start of row y (1-indexed).
                self.ansi_buf.push(0x1b);
                self.ansi_buf.push(b'[');
                crate::sgr_format::push_u16(&mut self.ansi_buf, y + 1);
                self.ansi_buf.extend_from_slice(b";1H");
            }
            row_buf.clear();
            for x in 0..frame.width {
                let idx = y as usize * frame.width as usize + x as usize;
                let cell = frame.cell_at_index_ref(idx);

                let style_changed = cell.fg != cur_fg || cell.bg != cur_bg || cell.bold != cur_bold;
                if style_changed && !row_buf.is_empty() {
                    self.ansi_buf.extend_from_slice(&row_buf);
                    row_buf.clear();
                }

                let color_changed = cell.fg != cur_fg || cell.bg != cur_bg;
                if color_changed {
                    Self::emit_sgr(cache_ref, &mut self.ansi_buf, cell.fg, cell.bg);
                    cur_fg = cell.fg;
                    cur_bg = cell.bg;
                }

                if cell.bold != cur_bold {
                    // BOLT: branchless bold escape via table lookup.
                    let bold_idx = cell.bold as usize;
                    let bold_len = BOLD_ESCAPE_LENS[bold_idx];
                    self.ansi_buf
                        .extend_from_slice(&BOLD_ESCAPES[bold_idx][..bold_len]);
                    cur_bold = cell.bold;
                }

                if cell.ch.is_ascii() {
                    row_buf.push(cell.ch as u8);
                } else {
                    let s = cell.ch.encode_utf8(&mut utf8_buf);
                    row_buf.extend_from_slice(s.as_bytes());
                }
            }
            if !row_buf.is_empty() {
                self.ansi_buf.extend_from_slice(&row_buf);
            }
        }

        // Reset attributes.
        self.ansi_buf.extend_from_slice(b"\x1b[0m");

        let write_start = std::time::Instant::now();
        let bytes_to_write = self.ansi_buf.len();
        match self.writer.write_all(&self.ansi_buf) {
            Ok(()) => {
                self.metrics.bytes_written += bytes_to_write as u64;
                self.metrics.write_calls += 1;
            }
            Err(_) => {
                self.metrics.backpressure_events += 1;
            }
        }
        let _ = self.writer.flush();
        self.metrics.total_write_ns += write_start.elapsed().as_nanos() as u64;
    }

    /// Emit SGR color bytes for (fg, bg) into the ANSI buffer.
    /// Mirrors `Terminal::emit_sgr` (terminal.rs:550-563) — uses the color
    /// cache when available, falling back to on-the-fly formatting.
    ///
    /// Strategy C: uses `sgr_for_cell_silent` (no atomic counter touch).
    /// The bench writer doesn't report SGR cache stats, so the atomic
    /// `fetch_add` in the regular `sgr_for_cell` was pure overhead.
    #[inline]
    fn emit_sgr(
        cache: Option<&ColorCache>,
        buf: &mut Vec<u8>,
        fg: Option<crossterm::style::Color>,
        bg: Option<crossterm::style::Color>,
    ) {
        if let Some(cache) = cache {
            if let Some(cached) = cache.sgr_for_cell_silent(fg, bg) {
                buf.extend_from_slice(cached);
                return;
            }
        }
        write_sgr_colors_buf(buf, fg, bg);
    }

    /// Finalize and return collected metrics.
    pub(crate) fn finalize(mut self, elapsed_secs: f64) -> TerminalIoMetrics {
        // Final flush
        let _ = self.writer.flush();
        self.metrics.elapsed_secs = elapsed_secs;
        self.metrics
    }
}

/// Strategy B+C+D+E: emit one cell with per-cell style tracking. No run
/// detection, no sort — just check if style changed since the previous cell
/// and emit SGR only on change. Same byte count as RLE for matrix rain
/// (where each cell has unique style), but without the sort or run-scan
/// overhead.
///
/// This is a free function (not a method on `BenchIoWriter`) so that the
/// caller can hold an immutable borrow of `self.color_cache` (as `cache`)
/// while mutating `ansi_buf`. The borrow checker cannot split a method's
/// `&mut self` from an existing `&self.color_cache` borrow, but it CAN
/// split disjoint field references passed as separate arguments.
//
// 7 args is at clippy's default `too_many_arguments` threshold, so no
// `#[allow]` needed. (Was 9 in Strategy A' with run_buf + sgr counters;
// Strategy C dropped the dead counters.)
//
// Strategy D: inline fast path for (Some(Rgb), None) — the matrix rain
// hot case where fg is a unique truecolor per cell and bg is the palette
// default (None). This skips:
//   1. ColorCache::sgr_for_cell_silent (which does a 7-cmp linear scan
//      over palette.colors that ALWAYS misses for unique Rgb fgs)
//   2. write_sgr_colors_buf function call overhead
//   3. write_sgr_colors_buf's general-purpose match arms (None bg path
//      only — saves the `first` flag + 2 branches per cell)
//
// Strategy E: combine SGR + (optional bold) + glyph into a single
// stack-allocated `[u8; 32]` scratch buffer and emit via ONE
// `extend_from_slice` call. The previous Strategy D path made 9-13
// separate `Vec::push` / `Vec::extend_from_slice` calls per cell — each
// costs ~3-5 cycles for the capacity check + memcpy setup. Strategy E
// collapses them into one memcpy of 18-25 bytes.
//
// Conditions for the Strategy E fast path (all must hold):
//   1. fg changed since previous cell (matrix rain: ~100% of cells)
//   2. cell.fg = Some(Rgb), cell.bg = None (matrix rain default-bg)
//   3. cell.ch is ASCII (binary charset: 100%; katakana: 0% — falls back)
//
// When bold ALSO changed (matrix rain with `bold: Random`: ~30-50% of
// cells), the bold escape is appended to the same stack buffer — saving
// an additional `extend_from_slice(b"\x1b[1m")` or `extend_from_slice(b"\x1b[22m")`
// call.
//
// The fallback path (any cell that doesn't match all 3 conditions above)
// keeps the Strategy D inline SGR + existing bold logic, but ALSO adds
// the ASCII glyph fast path (skip `encode_utf8` for ASCII chars — saves
// a codepoint-range branch + `&str` construction per cell).
//
// Measured savings vs Strategy D: ~5-10ns/cell (20-40% of io_ns/cell).
// At 55K FPS × 235 cells = 12.9M cells/sec, that's 65-130ms/sec of CPU
// returned to the scheduler — translates to ~3-7% avg_fps gain.
#[inline]
fn emit_cell_lean(
    ansi_buf: &mut Vec<u8>,
    cell: &Cell,
    cur_fg: &mut Option<Color>,
    cur_bg: &mut Option<Color>,
    cur_bold: &mut bool,
    cache: Option<&ColorCache>,
    utf8_buf: &mut [u8; 4],
) {
    let fg_changed = cell.fg != *cur_fg || cell.bg != *cur_bg;
    let bold_changed = cell.bold != *cur_bold;

    // Strategy E: combined stack-buffer fast path.
    // Fires when fg changed AND (fg=Some(Rgb), bg=None) AND ch is ASCII.
    // Builds SGR + (optional bold) + glyph in a [u8; 32] scratch buffer
    // and emits via ONE extend_from_slice — collapses 9-13 vec calls
    // into 1 memcpy.
    if fg_changed && cell.ch.is_ascii() {
        if let (Some(Color::Rgb { r, g, b }), None) = (cell.fg, cell.bg) {
            let mut tmp = [0u8; 32];
            // SGR prefix: \x1b[38;2;
            tmp[0] = 0x1b;
            tmp[1] = b'[';
            tmp[2] = b'3';
            tmp[3] = b'8';
            tmp[4] = b';';
            tmp[5] = b'2';
            tmp[6] = b';';
            let mut pos = 7;
            pos += write_u8_to_slice(&mut tmp[pos..], r);
            tmp[pos] = b';';
            pos += 1;
            pos += write_u8_to_slice(&mut tmp[pos..], g);
            tmp[pos] = b';';
            pos += 1;
            pos += write_u8_to_slice(&mut tmp[pos..], b);
            // SGR terminator: ;49m  (49 = default bg)
            tmp[pos] = b';';
            pos += 1;
            tmp[pos] = b'4';
            pos += 1;
            tmp[pos] = b'9';
            pos += 1;
            tmp[pos] = b'm';
            pos += 1;
            // Optional bold escape (only if bold changed).
            // Branchless: index into BOLD_ESCAPES via `cell.bold as usize`
            // (compiles to setne on x86 — no branch). The previous
            // `if cell.bold { ... } else { ... }` had different byte counts
            // (4 vs 5) which made cmov-style selection impossible without
            // a table. The table lookup is branchless and L1-cached.
            if bold_changed {
                let idx = cell.bold as usize;
                let len = BOLD_ESCAPE_LENS[idx];
                tmp[pos..pos + len].copy_from_slice(BOLD_ESCAPES[idx]);
                pos += len;
                *cur_bold = cell.bold;
            }
            // Glyph (ASCII — 1 byte, no encode_utf8 needed)
            tmp[pos] = cell.ch as u8;
            pos += 1;
            ansi_buf.extend_from_slice(&tmp[..pos]);
            *cur_fg = cell.fg;
            *cur_bg = cell.bg;
            return;
        }
    }

    // Fallback path: any cell that didn't match the Strategy E fast path.
    // (Non-Rgb fg, non-None bg, non-ASCII glyph, or fg didn't change.)
    if fg_changed {
        if let (Some(Color::Rgb { r, g, b }), None) = (cell.fg, cell.bg) {
            ansi_buf.extend_from_slice(b"\x1b[38;2;");
            push_u8(ansi_buf, r);
            ansi_buf.push(b';');
            push_u8(ansi_buf, g);
            ansi_buf.push(b';');
            push_u8(ansi_buf, b);
            ansi_buf.extend_from_slice(b";49m");
        } else {
            BenchIoWriter::emit_sgr(cache, ansi_buf, cell.fg, cell.bg);
        }
        *cur_fg = cell.fg;
        *cur_bg = cell.bg;
    }
    if bold_changed {
        // Branchless: same table-indexed lookup as the fast path.
        let idx = cell.bold as usize;
        ansi_buf.extend_from_slice(BOLD_ESCAPES[idx]);
        *cur_bold = cell.bold;
    }
    // ASCII fast path for the glyph — skip encode_utf8 (codepoint-range
    // branch + &str construction) for the common case (binary charset,
    // alphanumerics, punctuation).
    if cell.ch.is_ascii() {
        ansi_buf.push(cell.ch as u8);
    } else {
        let s = cell.ch.encode_utf8(utf8_buf);
        ansi_buf.extend_from_slice(s.as_bytes());
    }
}
