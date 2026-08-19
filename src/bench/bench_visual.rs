// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Visual objective metrics — computed from frame buffer during benchmark.
//!
//! Phase 6 of DeepSeek benchmark restructuring plan.
//!
//! Samples the frame every N frames to compute:
//! - Shannon entropy of dirty cell distribution per column
//! - Gini coefficient of dirty cell density inequality
//! - Color transition smoothness (average RGB delta between frames)
//!
//! All metrics are cross-platform (computed from Cell data, not OS APIs).

use crossterm::style::Color;

use crate::frame::Frame;

/// Visual metrics accumulated over the benchmark run.
#[derive(Debug, Clone, Default)]
pub(crate) struct VisualMetrics {
    pub frame_entropy_bits: f64,
    pub density_gini: f64,
    pub color_transition_delta_avg: f64,
    pub samples: u32,
}

/// Accumulator for visual metrics — call sample() every N frames, finalize() at end.
pub(crate) struct VisualSampler {
    entropy_sum: f64,
    gini_sum: f64,
    color_delta_sum: f64,
    color_delta_count: u64,
    samples: u32,
    /// Strategy B: hybrid O(D) approach for color transition tracking.
    ///
    /// `prev_cells` is indexed by flat idx → O(1) lookup (no search).
    /// `prev_dirty_bits` avoids ghost comparisons (a cell that wasn't dirty
    /// in the previous sample has bit=0, so we skip it — no false delta).
    /// `prev_dirty_indices` remembers which indices were dirty, so we can
    /// clear only those bits on the next sample (O(D) clear, not O(W*H)).
    ///
    /// This supersedes A'' (which used `Vec<(usize, Cell)>` + linear scan
    /// → O(D²) blowup). It also beats the original `Vec<Cell>` full-grid
    /// copy (O(W*H) per sample) by only updating dirty cells: O(D).
    ///
    /// For storm scene (D=1778, W*H=3232): O(D) = 1778 ops vs A'' O(D²) =
    /// 1.58M ops vs original O(W*H) = 3232 ops. Strategy B wins on all
    /// scenes where D < W*H (i.e., always, except full-redraw frames).
    prev_cells: Vec<crate::cell::Cell>,
    prev_dirty_bits: Vec<u8>,
    prev_dirty_indices: Vec<usize>,
    sample_interval: u32,
    frame_counter: u32,
    // Reusable scratch buffers for per-sample column-distribution analysis.
    // Hoisted out of `sample()` so they are allocated once and `clear()`-ed
    // per sample instead of `vec![...]`-allocated every sample. This keeps
    // the benchmark's `alloc_calls_per_frame` metric honest — without this,
    // the visual sampler alone contributes ~0.2 allocs/frame of noise that
    // does not reflect the real rendering hot path.
    col_counts: Vec<u32>,
    sorted_counts: Vec<u32>,
}

impl VisualSampler {
    pub(crate) fn new(sample_interval: u32) -> Self {
        Self {
            entropy_sum: 0.0,
            gini_sum: 0.0,
            color_delta_sum: 0.0,
            color_delta_count: 0,
            samples: 0,
            prev_cells: Vec::new(),
            prev_dirty_bits: Vec::new(),
            prev_dirty_indices: Vec::new(),
            // BD-01: guard against 0 — would panic at `frame_counter % 0`
            // in `sample()`. `max(1)` makes the constructor fail-safe for
            // any future caller (current callers pass 10).
            sample_interval: sample_interval.max(1),
            frame_counter: 0,
            col_counts: Vec::new(),
            sorted_counts: Vec::new(),
        }
    }

    /// Call every frame. Only samples every N frames to reduce overhead.
    pub(crate) fn sample(&mut self, frame: &Frame) {
        self.frame_counter += 1;
        if !self.frame_counter.is_multiple_of(self.sample_interval) {
            return;
        }

        let dirty = frame.dirty_indices();
        if dirty.is_empty() && !frame.is_dirty_all() {
            return;
        }

        let width = frame.width as usize;
        let height = frame.height as usize;
        let total = width * height;

        // Resize on first sample or terminal resize. `prev_dirty_indices`
        // is cleared to avoid referencing stale indices into the old array.
        if self.prev_cells.len() != total {
            self.prev_cells
                .resize(total, crate::cell::Cell::blank_with_bg(None));
            self.prev_dirty_bits.resize(total, 0);
            self.prev_dirty_indices.clear();
        }

        // Count dirty cells per column. Reuse the hoisted `col_counts`
        // buffer — `clear()` preserves capacity, `resize()` only allocates
        // if the width grew (terminal resize). Steady state: zero allocs.
        self.col_counts.clear();
        self.col_counts.resize(width, 0u32);
        if frame.is_dirty_all() {
            self.col_counts.fill(height as u32);
        } else {
            for &idx in dirty {
                let col = idx % width;
                self.col_counts[col] += 1;
            }
        }

        // 1. Shannon entropy of column distribution
        let total_dirty: u32 = self.col_counts.iter().sum();
        if total_dirty > 0 {
            let mut entropy = 0.0;
            for &count in &self.col_counts {
                if count > 0 {
                    let p = count as f64 / total_dirty as f64;
                    entropy -= p * p.log2();
                }
            }
            self.entropy_sum += entropy;
        }

        // 2. Gini coefficient. Reuse `sorted_counts` buffer — clone into
        // it (capacity preserved), then sort in place. Steady state: zero
        // allocs.
        self.sorted_counts.clear();
        self.sorted_counts.extend_from_slice(&self.col_counts);
        self.sorted_counts.sort_unstable();
        let n = self.sorted_counts.len() as f64;
        let sum: u32 = self.sorted_counts.iter().sum();
        if sum > 0 && n > 0.0 {
            let mut weighted_sum = 0.0;
            for (i, &val) in self.sorted_counts.iter().enumerate() {
                weighted_sum += (i as f64 + 1.0) * val as f64;
            }
            let gini = (2.0 * weighted_sum) / (n * sum as f64) - (n + 1.0) / n;
            self.gini_sum += gini.max(0.0);
        }

        // 3. Color transition smoothness — Strategy B: O(D) using direct
        // index lookup into prev_cells, gated by prev_dirty_bits to skip
        // cells that weren't dirty in the previous sample (no ghost delta).
        if !self.prev_dirty_indices.is_empty() {
            let mut delta_sum = 0.0;
            let mut delta_count = 0u32;
            for &idx in dirty {
                // idx is guaranteed < total by Frame's dirty list invariant.
                // prev_dirty_bits[idx] == 1 means this cell was dirty in the
                // previous sample, so prev_cells[idx] holds a meaningful
                // previous state to compare against.
                if self.prev_dirty_bits[idx] == 1 {
                    let prev = &self.prev_cells[idx];
                    let cur = frame.cell_at_index_ref(idx);
                    let d = color_delta(&prev.fg, &cur.fg);
                    if d > 0.0 {
                        delta_sum += d;
                        delta_count += 1;
                    }
                }
            }
            if delta_count > 0 {
                self.color_delta_sum += delta_sum / delta_count as f64;
                self.color_delta_count += 1;
            }
        }

        // Strategy B: update prev state in O(D) (only dirty cells), not
        // O(W*H) (full grid copy). Clear old dirty bits, set new ones.
        for &idx in &self.prev_dirty_indices {
            self.prev_dirty_bits[idx] = 0;
        }
        self.prev_dirty_indices.clear();

        if frame.is_dirty_all() {
            // Full redraw: all cells are dirty. O(W*H) update is unavoidable
            // here (every cell changed), but this path is rare (1-21 frames
            // per benchmark run based on scene).
            self.prev_dirty_indices.reserve(total);
            for idx in 0..total {
                self.prev_cells[idx] = frame.cell_at_index(idx);
                self.prev_dirty_bits[idx] = 1;
                self.prev_dirty_indices.push(idx);
            }
        } else {
            self.prev_dirty_indices.reserve(dirty.len());
            for &idx in dirty {
                self.prev_cells[idx] = frame.cell_at_index(idx);
                self.prev_dirty_bits[idx] = 1;
                self.prev_dirty_indices.push(idx);
            }
        }

        self.samples += 1;
    }

    /// Finalize and return averaged metrics.
    pub(crate) fn finalize(self) -> VisualMetrics {
        let n = self.samples.max(1) as f64;
        VisualMetrics {
            frame_entropy_bits: self.entropy_sum / n,
            density_gini: self.gini_sum / n,
            color_transition_delta_avg: if self.color_delta_count > 0 {
                self.color_delta_sum / self.color_delta_count as f64
            } else {
                0.0
            },
            samples: self.samples,
        }
    }
}

/// Euclidean distance between two Option<Color> values.
fn color_delta(a: &Option<Color>, b: &Option<Color>) -> f64 {
    match (a, b) {
        (
            Some(Color::Rgb {
                r: r1,
                g: g1,
                b: b1,
            }),
            Some(Color::Rgb {
                r: r2,
                g: g2,
                b: b2,
            }),
        ) => {
            let dr = ((*r1) as f64 - (*r2) as f64).abs();
            let dg = ((*g1) as f64 - (*g2) as f64).abs();
            let db = ((*b1) as f64 - (*b2) as f64).abs();
            (dr * dr + dg * dg + db * db).sqrt()
        }
        _ => 0.0,
    }
}
