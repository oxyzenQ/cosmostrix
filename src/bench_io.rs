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

use std::io::{BufWriter, Write};
use std::time::Instant;

use crate::cell::Cell;
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
pub(crate) struct BenchIoWriter {
    writer: BufWriter<std::fs::File>,
    ansi_buf: Vec<u8>,
    metrics: TerminalIoMetrics,
}

impl BenchIoWriter {
    /// Create a new writer targeting /dev/null (Unix) or nul (Windows).
    pub(crate) fn new() -> Option<Self> {
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
        })
    }

    /// Generate ANSI bytes from the frame's dirty cells and write to null device.
    /// Measures write time and tracks metrics.
    pub(crate) fn write_frame(&mut self, frame: &Frame) {
        self.ansi_buf.clear();

        // Generate ANSI for dirty cells (simplified: SGR + char per dirty cell).
        // This produces representative ANSI output similar to what Terminal::draw()
        // would emit, without the full RLE optimization (which is the renderer's
        // job — here we measure raw I/O bandwidth).
        let dirty = frame.dirty_indices();
        if dirty.is_empty() && !frame.is_dirty_all() {
            return;
        }

        let cur_fg_prev: Option<crossterm::style::Color> = None;
        let cur_bg_prev: Option<crossterm::style::Color> = None;
        let mut cur_fg = cur_fg_prev;
        let mut cur_bg = cur_bg_prev;

        if frame.is_dirty_all() {
            // Full redraw: iterate all cells
            let total = (frame.width as usize) * (frame.height as usize);
            for idx in 0..total {
                let cell = frame.cell_at_index(idx);
                self.emit_cell(&mut cur_fg, &mut cur_bg, &cell);
            }
        } else {
            // Diff: iterate only dirty cells
            for &idx in dirty {
                let cell = frame.cell_at_index(idx);
                self.emit_cell(&mut cur_fg, &mut cur_bg, &cell);
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

    /// Emit SGR + character for a single cell into ansi_buf.
    fn emit_cell(
        &mut self,
        cur_fg: &mut Option<crossterm::style::Color>,
        cur_bg: &mut Option<crossterm::style::Color>,
        cell: &Cell,
    ) {
        // SGR (only if color changed)
        if cell.fg != *cur_fg || cell.bg != *cur_bg {
            write_sgr_colors_buf(&mut self.ansi_buf, cell.fg, cell.bg);
            *cur_fg = cell.fg;
            *cur_bg = cell.bg;
        }

        // Bold toggle
        if cell.bold {
            self.ansi_buf.extend_from_slice(b"\x1b[1m");
        }

        // Character (UTF-8 encoded)
        let mut buf = [0u8; 4];
        let s = cell.ch.encode_utf8(&mut buf);
        self.ansi_buf.extend_from_slice(s.as_bytes());
    }

    /// Finalize and return collected metrics.
    pub(crate) fn finalize(mut self, elapsed_secs: f64) -> TerminalIoMetrics {
        // Final flush
        let _ = self.writer.flush();
        self.metrics.elapsed_secs = elapsed_secs;
        self.metrics
    }
}
