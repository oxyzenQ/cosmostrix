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
//!   the 24-byte `Cell` copy per dirty cell.
//!
//! ## A' upgrade — RLE batching (production parity)
//!
//! The previous `write_frame` emitted one SGR + one char per dirty cell,
//! producing ~5× more ANSI bytes than the production `Terminal::draw()`
//! path which batches contiguous same-style cells into a single SGR +
//! a character run. Because `io_ms` in the benchmark loop is computed
//! as a residual bucket that includes `write_frame()` time, this
//! over-production falsely surfaced as an "85.5% IO bottleneck".
//!
//! The new `write_frame` ports the RLE batching logic from
//! `terminal.rs:835-941`:
//! 1. Sort dirty indices (row-major order, same as production).
//! 2. Scan contiguous runs of identical (fg, bg, bold) cells.
//! 3. Emit ONE SGR sequence per style change + ONE `extend_from_slice`
//!    for the entire character run.
//! 4. Track `cur_bold` (only emit `\x1b[1m` / `\x1b[22m` on toggle).
//!
//! Output is byte-for-byte identical to what `Terminal::draw()` would
//! emit for the same dirty set (modulo MoveTo cursor commands, which
//! are not needed when writing to `/dev/null`). Expected impact:
//! `bytes_written` drops ~5×, `io_share` drops from ~85% to ~40-50%,
//! `avg_fps` rises from ~47K to ~70-80K.

use std::io::{BufWriter, Write};
use std::time::Instant;

use crate::cell::Cell;
use crate::color_cache::ColorCache;
use crate::frame::Frame;
use crate::sgr_format::write_sgr_colors_buf;

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
    /// T2.1: SGR cache hit counter — incremented each time `ColorCache::sgr_for_cell`
    /// returned a pre-formatted byte slice. Misses fall back to `write_sgr_colors_buf`.
    pub sgr_cache_hits: u64,
    pub sgr_cache_misses: u64,
}

impl TerminalIoMetrics {
    /// Write bandwidth in MB/s (1 MB = 1,048,576 bytes).
    #[must_use]
    pub fn bandwidth_mbps(&self) -> f64 {
        if self.elapsed_secs > 0.0 {
            (self.bytes_written as f64 / 1_048_576.0) / self.elapsed_secs
        } else {
            0.0
        }
    }

    /// Average write latency in microseconds per write call.
    #[must_use]
    pub fn avg_latency_us(&self) -> f64 {
        if self.write_calls > 0 {
            (self.total_write_ns as f64 / self.write_calls as f64) / 1000.0
        } else {
            0.0
        }
    }

    /// Effective write FPS = write_calls / elapsed_secs.
    #[must_use]
    pub fn effective_write_fps(&self) -> f64 {
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
/// A': now performs RLE batching identical to `Terminal::draw()` so the
/// benchmark I/O path produces the same byte stream as production rendering.
pub(crate) struct BenchIoWriter {
    writer: BufWriter<std::fs::File>,
    ansi_buf: Vec<u8>,
    /// A': reusable character-run buffer (mirrors `Terminal::run_buf`).
    /// Accumulates UTF-8 bytes for a contiguous same-style run, flushed
    /// to `ansi_buf` in a single `extend_from_slice` when style changes.
    run_buf: String,
    /// A': reusable dirty-index scratch buffer. We sort a copy of the
    /// dirty list (rather than mutating `Frame`'s internal SmallVec) so
    /// the benchmark path stays read-only with respect to the frame.
    dirty_flat: Vec<usize>,
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
    pub(crate) fn with_palette(palette: &crate::chroma::palette::Palette) -> Option<Self> {
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
            run_buf: String::with_capacity(256),
            dirty_flat: Vec::new(),
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
    /// A': now performs RLE batching — contiguous same-style dirty cells are
    /// grouped into a single SGR + a character run, mirroring the production
    /// `Terminal::draw()` diff path. This drops `bytes_written` ~5× and
    /// removes the false "IO bottleneck" from the benchmark report.
    pub(crate) fn write_frame(&mut self, frame: &Frame) {
        self.ansi_buf.clear();
        self.run_buf.clear();

        let dirty = frame.dirty_indices();
        if dirty.is_empty() && !frame.is_dirty_all() {
            return;
        }

        // Current SGR state — only emit on change (mirrors Terminal::draw).
        let mut cur_fg: Option<crossterm::style::Color> = None;
        let mut cur_bg: Option<crossterm::style::Color> = None;
        let mut cur_bold = false;
        let cache_ref = self.color_cache.as_ref();
        let width_usize = frame.width as usize;

        if frame.is_dirty_all() {
            // Full redraw: iterate all cells in row-major order. Group
            // contiguous same-style runs across row boundaries (the
            // production full-redraw path in terminal.rs:713-758 also
            // flushes per-row, but for the bench writer we don't emit
            // MoveTo commands since the target is /dev/null — flushing
            // across rows is fine and produces the minimal byte count).
            let total = width_usize * (frame.height as usize);
            for idx in 0..total {
                let cell = frame.cell_at_index_ref(idx);
                self.write_cell_rle(
                    cell,
                    &mut cur_fg,
                    &mut cur_bg,
                    &mut cur_bold,
                    cache_ref,
                );
            }
        } else {
            // Diff path: sort dirty indices (row-major) and scan contiguous
            // runs. This mirrors terminal.rs:798-941 production logic:
            //   1. Sort dirty indices (already in row-major order if push
            //      order is preserved, but sort_unstable guarantees it).
            //   2. Walk the sorted array, detecting contiguous horizontal
            //      runs (idx1 == idx0 + 1 && same row) with identical style.
            //   3. Accumulate chars into run_buf; flush on style change or
            //      run break.
            self.dirty_flat.clear();
            self.dirty_flat.extend(dirty.iter().copied());
            self.dirty_flat.sort_unstable();

            let mut i = 0usize;
            while i < self.dirty_flat.len() {
                let idx0 = self.dirty_flat[i];
                let cell0_ref = frame.cell_at_index_ref(idx0);
                let fg0 = cell0_ref.fg;
                let bg0 = cell0_ref.bg;
                let bold0 = cell0_ref.bold;

                self.run_buf.clear();
                self.run_buf.push(cell0_ref.ch);

                // Extend run with contiguous same-style cells on the same row.
                let mut j = i + 1;
                while j < self.dirty_flat.len() {
                    let idx1 = self.dirty_flat[j];
                    // Must be the next column on the same row (contiguous).
                    if idx1 != self.dirty_flat[j - 1] + 1 {
                        break;
                    }
                    if idx1 / width_usize != idx0 / width_usize {
                        break;
                    }
                    let cell1_ref = frame.cell_at_index_ref(idx1);
                    if cell1_ref.fg != fg0
                        || cell1_ref.bg != bg0
                        || cell1_ref.bold != bold0
                    {
                        break;
                    }
                    self.run_buf.push(cell1_ref.ch);
                    j += 1;
                }

                // Emit SGR if style changed since the previous run.
                let style_changed = fg0 != cur_fg || bg0 != cur_bg;
                if style_changed {
                    Self::emit_sgr(
                        cache_ref,
                        &mut self.ansi_buf,
                        &mut self.metrics.sgr_cache_hits,
                        &mut self.metrics.sgr_cache_misses,
                        fg0,
                        bg0,
                    );
                    cur_fg = fg0;
                    cur_bg = bg0;
                }

                if bold0 != cur_bold {
                    if bold0 {
                        self.ansi_buf.extend_from_slice(b"\x1b[1m");
                    } else {
                        self.ansi_buf.extend_from_slice(b"\x1b[22m");
                    }
                    cur_bold = bold0;
                }

                // Flush the character run in one call.
                self.ansi_buf.extend_from_slice(self.run_buf.as_bytes());

                i = j;
            }
        }

        // Flush any trailing run (only relevant for the full-redraw path,
        // which accumulates into run_buf without per-row flush).
        if !self.run_buf.is_empty() {
            self.ansi_buf.extend_from_slice(self.run_buf.as_bytes());
            self.run_buf.clear();
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

    /// A': full-redraw helper — emit one cell, accumulating into `run_buf`
    /// and flushing on style change. Mirrors terminal.rs:723-754.
    #[inline]
    fn write_cell_rle(
        &mut self,
        cell: &Cell,
        cur_fg: &mut Option<crossterm::style::Color>,
        cur_bg: &mut Option<crossterm::style::Color>,
        cur_bold: &mut bool,
        cache_ref: Option<&ColorCache>,
    ) {
        // On any style change, flush the pending run and emit new SGR.
        let color_changed = cell.fg != *cur_fg || cell.bg != *cur_bg;
        if color_changed && !self.run_buf.is_empty() {
            self.ansi_buf.extend_from_slice(self.run_buf.as_bytes());
            self.run_buf.clear();
        }
        if color_changed {
            Self::emit_sgr(
                cache_ref,
                &mut self.ansi_buf,
                &mut self.metrics.sgr_cache_hits,
                &mut self.metrics.sgr_cache_misses,
                cell.fg,
                cell.bg,
            );
            *cur_fg = cell.fg;
            *cur_bg = cell.bg;
        }
        if cell.bold != *cur_bold {
            if !self.run_buf.is_empty() {
                self.ansi_buf.extend_from_slice(self.run_buf.as_bytes());
                self.run_buf.clear();
            }
            if cell.bold {
                self.ansi_buf.extend_from_slice(b"\x1b[1m");
            } else {
                self.ansi_buf.extend_from_slice(b"\x1b[22m");
            }
            *cur_bold = cell.bold;
        }
        self.run_buf.push(cell.ch);
    }

    /// Emit SGR color bytes for (fg, bg) into the ANSI buffer.
    /// Mirrors `Terminal::emit_sgr` (terminal.rs:550-563) — uses the color
    /// cache when available, falling back to on-the-fly formatting.
    #[inline]
    fn emit_sgr(
        cache: Option<&ColorCache>,
        buf: &mut Vec<u8>,
        sgr_hits: &mut u64,
        sgr_misses: &mut u64,
        fg: Option<crossterm::style::Color>,
        bg: Option<crossterm::style::Color>,
    ) {
        if let Some(cache) = cache {
            if let Some(cached) = cache.sgr_for_cell(fg, bg) {
                buf.extend_from_slice(cached);
                *sgr_hits += 1;
                return;
            }
        }
        write_sgr_colors_buf(buf, fg, bg);
        *sgr_misses += 1;
    }

    /// Finalize and return collected metrics.
    pub(crate) fn finalize(mut self, elapsed_secs: f64) -> TerminalIoMetrics {
        // Final flush
        let _ = self.writer.flush();
        self.metrics.elapsed_secs = elapsed_secs;
        self.metrics
    }
}
