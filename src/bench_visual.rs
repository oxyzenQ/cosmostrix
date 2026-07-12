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
pub struct VisualMetrics {
    pub frame_entropy_bits: f64,
    pub density_gini: f64,
    pub color_transition_delta_avg: f64,
    pub samples: u32,
}

/// Accumulator for visual metrics — call sample() every N frames, finalize() at end.
pub struct VisualSampler {
    entropy_sum: f64,
    gini_sum: f64,
    color_delta_sum: f64,
    color_delta_count: u64,
    samples: u32,
    prev_cells: Vec<crate::cell::Cell>,
    sample_interval: u32,
    frame_counter: u32,
}

impl VisualSampler {
    pub fn new(sample_interval: u32) -> Self {
        Self {
            entropy_sum: 0.0,
            gini_sum: 0.0,
            color_delta_sum: 0.0,
            color_delta_count: 0,
            samples: 0,
            prev_cells: Vec::new(),
            sample_interval,
            frame_counter: 0,
        }
    }

    /// Call every frame. Only samples every N frames to reduce overhead.
    pub fn sample(&mut self, frame: &Frame) {
        self.frame_counter += 1;
        if self.frame_counter % self.sample_interval != 0 {
            return;
        }

        let dirty = frame.dirty_indices();
        if dirty.is_empty() && !frame.is_dirty_all() {
            return;
        }

        let width = frame.width as usize;
        let height = frame.height as usize;

        // Count dirty cells per column
        let mut col_counts = vec![0u32; width];
        if frame.is_dirty_all() {
            col_counts.fill(height as u32);
        } else {
            for &idx in dirty {
                let col = idx % width;
                col_counts[col] += 1;
            }
        }

        // 1. Shannon entropy of column distribution
        let total: u32 = col_counts.iter().sum();
        if total > 0 {
            let mut entropy = 0.0;
            for &count in &col_counts {
                if count > 0 {
                    let p = count as f64 / total as f64;
                    entropy -= p * p.log2();
                }
            }
            self.entropy_sum += entropy;
        }

        // 2. Gini coefficient
        let mut sorted = col_counts.clone();
        sorted.sort_unstable();
        let n = sorted.len() as f64;
        let sum: u32 = sorted.iter().sum();
        if sum > 0 && n > 0.0 {
            let mut weighted_sum = 0.0;
            for (i, &val) in sorted.iter().enumerate() {
                weighted_sum += (i as f64 + 1.0) * val as f64;
            }
            let gini = (2.0 * weighted_sum) / (n * sum as f64) - (n + 1.0) / n;
            self.gini_sum += gini.max(0.0);
        }

        // 3. Color transition smoothness
        if !self.prev_cells.is_empty() {
            let mut delta_sum = 0.0;
            let mut delta_count = 0u32;
            for &idx in dirty {
                let cur = frame.cell_at_index(idx);
                if idx < self.prev_cells.len() {
                    let prev = &self.prev_cells[idx];
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

        // Save current cells for next frame comparison
        if self.prev_cells.len() != width * height {
            self.prev_cells = vec![crate::cell::Cell::blank_with_bg(None); width * height];
        }
        for i in 0..width * height {
            self.prev_cells[i] = frame.cell_at_index(i);
        }

        self.samples += 1;
    }

    /// Finalize and return averaged metrics.
    pub fn finalize(self) -> VisualMetrics {
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
