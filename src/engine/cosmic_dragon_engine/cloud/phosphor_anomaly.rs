// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Phosphor anomaly effects + stuck-cell sweep — extracted from
//! `cloud/phosphor.rs` to keep that file under the 800-LOC cap.

use std::time::Instant;

use crate::cell::Cell;
use crate::chroma_dragon_engine::post::anomaly::{anomaly_halo_target, AnomalyHaloMode};
use crate::constants::*;

use super::phosphor::anomaly_halo_blend;
use super::state::AnomalyKind;

impl super::Cloud {
    /// Apply active anomaly zone effects to the frame (post-processing).
    ///
    /// Phase 6 (Chroma Dragon — palette-aware anomaly halos): the
    /// LuminanceSurge and PulseWave branches now derive their halo
    /// target color from the active palette via
    /// `chroma::post::anomaly::anomaly_halo_target`, instead of
    /// hardcoding pure white. This extends Phase 3-I's "palette-aware
    /// ghost" pattern to anomaly halos:
    ///
    /// - **LuminanceSurge** → lifts cells toward the palette's brightest
    ///   stop (`palette.colors.last()`). On a NeonRed theme, the surge
    ///   becomes a "lift toward bright red" rather than "lift toward
    ///   white" — preserving palette coherence.
    /// - **PulseWave** → lifts cells toward a hue-cycled palette stop
    ///   (`(elapsed * ANOMALY_HALO_CYCLE_RATE) % palette.len()`). The
    ///   expanding ring's target color cycles through palette stops as
    ///   it expands, giving PulseWave a distinct visual identity from
    ///   LuminanceSurge.
    ///
    /// When `anomaly_halo_target` returns `None` (empty palette or
    /// `Color::Reset` selected stop — rare edge cases), the branches
    /// fall back to `blend_toward_white`, preserving pre-Phase-6
    /// behavior for those degenerate cases.
    pub(crate) fn apply_anomalies(&mut self, frame: &mut crate::frame::Frame, now: Instant) {
        if self.anomaly_zones.is_empty() {
            return;
        }

        let bg = self.palette.bg;
        let cols = self.cols;
        let lines = self.lines;
        let width = frame.width;
        let palette_colors = &self.palette.colors;

        for zone in &self.anomaly_zones {
            let elapsed = now.saturating_duration_since(zone.start_time).as_secs_f32();
            if elapsed >= ANOMALY_DURATION_SECS {
                continue;
            }

            let progress = elapsed / ANOMALY_DURATION_SECS; // 0..1
            let fade = 1.0 - progress; // fades out over duration

            match zone.kind {
                AnomalyKind::LuminanceSurge => {
                    // Phase 6: derive the halo target from the palette's
                    // brightest stop instead of hardcoding pure white.
                    let halo_target = anomaly_halo_target(
                        palette_colors,
                        AnomalyHaloMode::LuminanceSurge,
                        elapsed,
                    );
                    let r = zone.radius as i16;
                    let r_sq = (zone.radius as f32) * (zone.radius as f32);
                    for col_off in -r..=r {
                        for line_off in -r..=r {
                            let c = zone.col as i16 + col_off;
                            let l = zone.line as i16 + line_off;
                            if c < 0 || l < 0 {
                                continue;
                            }
                            let col = c as u16;
                            let line = l as u16;
                            if col >= cols || line >= lines {
                                continue;
                            }

                            // PERF(v10): Compare dist_sq against r_sq to avoid sqrt()
                            // for cells outside the circle (~30% of bounding box).
                            let dist_sq = (col_off * col_off + line_off * line_off) as f32;
                            if dist_sq > r_sq {
                                continue;
                            }

                            let dist = dist_sq.sqrt();

                            let falloff = 1.0 - dist / zone.radius as f32;
                            let intensity = ANOMALY_LUMINANCE_INTENSITY * falloff * fade;

                            let fidx = line as usize * width as usize + col as usize;
                            let cell = frame.cell_at_index(fidx);
                            if let Some(fg) = cell.fg {
                                // (chroma audit, A21): LuminanceSurge
                                // halo -- shared helper routes through chroma
                                // engine when active, chroma::legacy fallback
                                // otherwise. Matches the A1-A20 pattern.
                                let brightened = anomaly_halo_blend(
                                    fg,
                                    halo_target,
                                    intensity,
                                    self.color_pipeline.is_chroma(),
                                );
                                frame.set(
                                    col,
                                    line,
                                    Cell {
                                        ch: cell.ch,
                                        fg: Some(brightened),
                                        bg,
                                        bold: cell.bold,
                                    },
                                );
                            }
                        }
                    }
                }
                AnomalyKind::GlyphCorruption => {
                    let r = zone.radius as i16;
                    for col_off in -r..=r {
                        for line_off in -r..=r {
                            let c = zone.col as i16 + col_off;
                            let l = zone.line as i16 + line_off;
                            if c < 0 || l < 0 {
                                continue;
                            }
                            let col = c as u16;
                            let line = l as u16;
                            if col >= cols || line >= lines {
                                continue;
                            }

                            // CC-02: use the full u32 hash normalized to
                            // [0, 1) so ANOMALY_CORRUPTION_CHANCE * fade is
                            // actually respected. Previously `>> 31` extracted
                            // only the top bit, collapsing the rate to ~50%
                            // always-corrupt at any fade (the `fade` parameter
                            // was effectively ignored). Mirrors the climate
                            // hash pattern at chroma_dragon_engine/post/climate/mod.rs.
                            let hash = (col as u32).wrapping_mul(2654435761)
                                ^ (line as u32).wrapping_mul(2246822519);
                            if (hash as f32 / (u32::MAX as f32 + 1.0))
                                > ANOMALY_CORRUPTION_CHANCE * fade
                            {
                                continue;
                            }

                            let fidx = line as usize * width as usize + col as usize;
                            let cell = frame.cell_at_index_ref(fidx);
                            if cell.fg.is_some() && !self.glitch_pool.is_empty() {
                                let cell_owned = frame.cell_at_index(fidx);
                                let glitch_idx =
                                    (col as usize + line as usize + (elapsed * 8.0) as usize)
                                        % self.glitch_pool.len();
                                frame.set(
                                    col,
                                    line,
                                    Cell {
                                        ch: self.glitch_pool[glitch_idx],
                                        fg: cell_owned.fg,
                                        bg,
                                        bold: cell_owned.bold,
                                    },
                                );
                            }
                        }
                    }
                }
                AnomalyKind::PulseWave => {
                    // Phase 6: derive the halo target from a hue-cycled
                    // palette stop — the ring's target color cycles
                    // through palette stops as it expands.
                    let halo_target =
                        anomaly_halo_target(palette_colors, AnomalyHaloMode::PulseWave, elapsed);
                    let wave_radius = progress * zone.radius as f32 * 2.0;
                    let ring_width = 2.0;
                    let ring_outer = wave_radius + ring_width;
                    let ring_outer_sq = ring_outer * ring_outer;
                    let ring_inner_sq = (wave_radius - ring_width).max(0.0).powi(2);
                    let r2 = (zone.radius as i16) * 2;
                    for col_off in -r2..=r2 {
                        for line_off in -r2..=r2 {
                            let c = zone.col as i16 + col_off;
                            let l = zone.line as i16 + line_off;
                            if c < 0 || l < 0 {
                                continue;
                            }
                            let col = c as u16;
                            let line = l as u16;
                            if col >= cols || line >= lines {
                                continue;
                            }

                            // PERF(v10): Reject via dist_sq before computing sqrt.
                            let dist_sq = (col_off * col_off + line_off * line_off) as f32;
                            if dist_sq > ring_outer_sq || dist_sq < ring_inner_sq {
                                continue;
                            }

                            let dist = dist_sq.sqrt();
                            let ring_dist = (dist - wave_radius).abs();
                            if ring_dist < ring_width {
                                let t = 1.0 - ring_dist / ring_width;
                                let intensity = 0.2 * t * fade;
                                let fidx = line as usize * width as usize + col as usize;
                                let cell = frame.cell_at_index(fidx);
                                if let Some(fg) = cell.fg {
                                    // (chroma audit, A22): PulseWave
                                    // halo -- shared helper (same as A21),
                                    // palette-derived hue-cycled target.
                                    let brightened = anomaly_halo_blend(
                                        fg,
                                        halo_target,
                                        intensity,
                                        self.color_pipeline.is_chroma(),
                                    );
                                    frame.set(
                                        col,
                                        line,
                                        Cell {
                                            ch: cell.ch,
                                            fg: Some(brightened),
                                            bg,
                                            bold: cell.bold,
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// P4: periodic stuck-cell sweep (debug mode only).
    ///
    /// Scans the frame buffer for cells that hold a visible glyph at the
    /// current generation but are NOT covered by any active droplet's
    /// tail_put_line..=head_put_line range AND have zero phosphor energy.
    /// These represent dirty-tracking edge cases that the phosphor system
    /// (which only handles cells with `phosphor[i] > 0`) cannot reach.
    ///
    /// When stuck cells are found, they are force-cleared (set to blank)
    /// and cumulative counters (`stuck_cells_cleared_total`,
    /// `stuck_sweeps_with_clears`) are incremented silently. The
    /// benchmark prints a single summary line at the end via
    /// `cloud.stuck_cell_stats()` in verbose mode — no per-sweep spam.
    /// The sweep is capped at `STUCK_CELL_MAX_PER_SWEEP` cells per pass
    /// to avoid pathological clearing.
    ///
    /// ## Gating
    ///
    /// T1.1: independent gate added. The sweep now short-circuits when
    /// `enable_stuck_cell_sweep` is false (independent of `enable_component_timing`).
    /// Default true for interactive runs; set to false in benchmark mode
    /// (`bench.rs`) so the sweep's Vec growth does not pollute realloc
    /// counters. The body still respects `enable_component_timing` as a
    /// second short-circuit (kept for backwards compatibility with `--perf-stats`).
    /// The sweep also short-circuits when a message box is active
    /// (its overlay cells would be false positives).
    ///
    /// ## Cost
    ///
    /// O(W×H + droplets) per sweep. At 200×60 with ~100 active droplets,
    /// ≈12,100 ops every 60 s ≈ 200 ops/s — negligible.
    pub(crate) fn stuck_cell_sweep(&mut self, frame: &mut crate::frame::Frame) {
        // T1.1: independent gate. Default true; benchmark sets to false
        // so the sweep's Vec growth (droplet_ranges SmallVec + dirty-list
        // churn from set_force) does not pollute realloc counters.
        if !self.enable_stuck_cell_sweep {
            return;
        }
        // Legacy gate: still respect enable_component_timing (preserves
        // the pre-T1.1 behavior where --perf-stats toggled the sweep).
        if !self.enable_component_timing {
            return;
        }
        // Skip when a message box is active — overlay cells would trigger
        // false positives (they're written this frame, have fg, but no
        // droplet covers them by design).
        if !self.message.is_empty() {
            return;
        }

        self.frames_since_stuck_sweep += 1;
        if self.frames_since_stuck_sweep < STUCK_CELL_SWEEP_INTERVAL_FRAMES {
            return;
        }
        self.frames_since_stuck_sweep = 0;

        let total = (self.cols as usize) * (self.lines as usize);
        if total == 0 || self.phosphor.len() != total {
            return;
        }

        let width = self.cols;
        let current_gen = frame.current_gen();
        let blank_cell = Cell::blank_with_bg(self.palette.bg);

        // Pre-compute each active droplet's visible trail range so the
        // inner cell loop is a cheap O(droplets) check, not an O(droplets
        // × cells) nested scan.
        //
        // A droplet covers (col, line) iff:
        //   bound_col == col
        //   AND line in [visible_start, head_put_line]
        // where visible_start = tail_put_line.map_or(0, |t| t+1)
        let mut droplet_ranges: smallvec::SmallVec<[(u16, u16, u16); 128]> =
            smallvec::SmallVec::new();
        for d in &self.droplets {
            if !d.is_alive {
                continue;
            }
            let visible_start = d.tail_put_line.map_or(0, |t| t.saturating_add(1));
            if visible_start > d.head_put_line {
                continue;
            }
            droplet_ranges.push((d.bound_col, visible_start, d.head_put_line));
        }

        let mut stuck_count: usize = 0;
        for i in 0..total {
            // Cell must have been written this frame (gen matches).
            if frame.cell_gen_at_index(i) != current_gen {
                continue;
            }
            // Cell must have a visible glyph (fg set).
            let cell = frame.cell_at_index_ref(i);
            if cell.fg.is_none() {
                continue;
            }
            // Phosphor must NOT be tracking this cell — that's the gap
            // the sweep is designed to catch.
            if self.phosphor[i] != 0 {
                continue;
            }
            // Check if any active droplet covers (col, line).
            let col = (i % width as usize) as u16;
            let line = (i / width as usize) as u16;
            let covered = droplet_ranges
                .iter()
                .any(|&(bc, vs, he)| bc == col && line >= vs && line <= he);
            if covered {
                continue;
            }
            // Stuck cell found — force-clear it.
            frame.set_force(col, line, blank_cell);
            stuck_count += 1;
            if stuck_count >= STUCK_CELL_MAX_PER_SWEEP {
                break;
            }
        }

        if stuck_count > 0 {
            // Accumulate counters silently — a single summary line is
            // printed by the benchmark at the end (verbose mode only)
            // via `cloud.stuck_cell_stats()`. This replaces the per-sweep
            // stderr spam that flooded verbose benchmark output.
            self.stuck_cells_cleared_total += stuck_count as u64;
            self.stuck_sweeps_with_clears += 1;
            // Force a full redraw next frame so the cleared cells are
            // actually emitted to the terminal (they were "fresh" this
            // frame, so the diff renderer would otherwise skip them).
            self.force_draw_everything = true;
        }
    }
}
