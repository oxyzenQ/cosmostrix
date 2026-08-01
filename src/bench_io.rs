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
                    &mut self.metrics.sgr_cache_hits,
                    &mut self.metrics.sgr_cache_misses,
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
                    &mut self.metrics.sgr_cache_hits,
                    &mut self.metrics.sgr_cache_misses,
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

/// Strategy B: emit one cell with per-cell style tracking. No run detection,
/// no sort — just check if style changed since the previous cell and emit
/// SGR only on change. Same byte count as RLE for matrix rain (where each
/// cell has unique style), but without the sort or run-scan overhead.
///
/// This is a free function (not a method on `BenchIoWriter`) so that the
/// caller can hold an immutable borrow of `self.color_cache` (as `cache`)
/// while mutating `ansi_buf` and the SGR hit/miss counters. The borrow
/// checker cannot split a method's `&mut self` from an existing
/// `&self.color_cache` borrow, but it CAN split disjoint field references
/// passed as separate arguments.
//
// 9 args exceeds clippy's default `too_many_arguments` threshold (7).
// Intentional: disjoint `&mut` field references needed for borrow-splitting
// (a &mut self method would reintroduce the E0502 conflict with the
// immutable cache_ref borrow). Same convention as live_config.rs:349,
// verbose.rs:57, cloud/render.rs:279, etc.
#[allow(clippy::too_many_arguments)]
#[inline]
fn emit_cell_lean(
    ansi_buf: &mut Vec<u8>,
    sgr_hits: &mut u64,
    sgr_misses: &mut u64,
    cell: &Cell,
    cur_fg: &mut Option<crossterm::style::Color>,
    cur_bg: &mut Option<crossterm::style::Color>,
    cur_bold: &mut bool,
    cache: Option<&ColorCache>,
    utf8_buf: &mut [u8; 4],
) {
    if cell.fg != *cur_fg || cell.bg != *cur_bg {
        BenchIoWriter::emit_sgr(cache, ansi_buf, sgr_hits, sgr_misses, cell.fg, cell.bg);
        *cur_fg = cell.fg;
        *cur_bg = cell.bg;
    }
    if cell.bold != *cur_bold {
        if cell.bold {
            ansi_buf.extend_from_slice(b"\x1b[1m");
        } else {
            ansi_buf.extend_from_slice(b"\x1b[22m");
        }
        *cur_bold = cell.bold;
    }
    let s = cell.ch.encode_utf8(utf8_buf);
    ansi_buf.extend_from_slice(s.as_bytes());
}
