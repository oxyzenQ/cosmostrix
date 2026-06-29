// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Phosphor persistence, anomaly handling, and atmospheric frame effects.

use std::time::Instant;

use crossterm::style::Color;
use rand::distr::Distribution;

use crate::cell::Cell;
use crate::constants::*;
use crate::palette;

use super::state::{AnomalyKind, AnomalyZone};
use super::Cloud;

#[inline]
fn captured_phosphor_energy(line: u16, lines: u16) -> u8 {
    let bottom_dist = lines.saturating_sub(line).saturating_sub(1);
    if bottom_dist >= EDGE_FADE_ROWS {
        return 255;
    }

    let taper_steps = EDGE_FADE_ROWS.saturating_sub(1).saturating_sub(bottom_dist) as u8;
    PHOSPHOR_EDGE_ENERGY_CAP.saturating_sub(taper_steps * PHOSPHOR_EDGE_ROW_TAPER)
}

impl Cloud {
    /// Phosphor persistence post-process: fade cells not refreshed by a
    /// droplet this frame, creating CRT-style afterglow.
    ///
    /// ## Performance optimization (v5.0.4)
    ///
    /// Pass 1 now scans dirty-cell indices when the dirty list is populated,
    /// falling back to full-grid scan only when dirty_all is set (e.g. after
    /// clear_with_bg). This eliminates ~95% of redundant scans in the common
    /// case where only dirty cells need phosphor capture.
    pub(super) fn phosphor_decay_pass(
        &mut self,
        frame: &mut crate::frame::Frame,
        elapsed_sec: f32,
    ) {
        let total = (self.cols as usize) * (self.lines as usize);
        if total == 0 || self.phosphor.len() != total {
            return;
        }

        // Skip phosphor under high performance pressure
        if self.perf_pressure > 0.7 {
            return;
        }

        let bg = self.palette.bg;
        let lines = self.lines;
        let frame_width = frame.width;

        // Pre-build blank cell for phosphor clear operations
        let blank_cell = Cell {
            ch: ' ',
            fg: None,
            bg,
            bold: false,
        };

        // Pass 1: Mark cells currently drawn by droplets as fresh.
        self.phosphor_fresh.fill(false);
        let current_gen = frame.current_gen();
        let mut tracked_fresh: smallvec::SmallVec<[usize; 256]> = smallvec::SmallVec::new();

        // OPTIMIZED: use dirty-index scan when available, full-grid as fallback.
        if frame.is_dirty_all() {
            // Full-grid scan: clear_with_bg emptied the dirty list.
            for line in 0..lines {
                for col in 0..self.cols {
                    let fidx = line as usize * frame_width as usize + col as usize;
                    let is_current_gen = frame.cell_gen_at_index(fidx) == current_gen;
                    if is_current_gen {
                        let cell = frame.cell_at_index_ref(fidx);
                        if cell.fg.is_some() {
                            let pidx = col as usize * lines as usize + line as usize;
                            self.phosphor_fresh.set(pidx, true);
                            self.phosphor[pidx] = captured_phosphor_energy(line, lines);
                            self.phosphor_base_fg[pidx] = cell.fg;
                            self.phosphor_base_ch[pidx] = cell.ch;
                            tracked_fresh.push(pidx);
                        } else if cell.ch != ' ' {
                            let pidx = col as usize * lines as usize + line as usize;
                            self.phosphor_fresh.set(pidx, true);
                            self.phosphor[pidx] = captured_phosphor_energy(line, lines);
                            self.phosphor_base_ch[pidx] = cell.ch;
                            tracked_fresh.push(pidx);
                        }
                    }
                }
            }
        } else {
            // Dirty-index scan: only iterate recently-drawn cells.
            for &dirty_idx in frame.dirty_indices() {
                let col = (dirty_idx % frame_width as usize) as u16;
                let line = (dirty_idx / frame_width as usize) as u16;
                if line >= lines || col >= self.cols {
                    continue;
                }
                let is_current_gen = frame.cell_gen_at_index(dirty_idx) == current_gen;
                if is_current_gen {
                    let cell = frame.cell_at_index_ref(dirty_idx);
                    if cell.fg.is_some() {
                        let pidx = col as usize * lines as usize + line as usize;
                        self.phosphor_fresh.set(pidx, true);
                        self.phosphor[pidx] = captured_phosphor_energy(line, lines);
                        self.phosphor_base_fg[pidx] = cell.fg;
                        self.phosphor_base_ch[pidx] = cell.ch;
                        tracked_fresh.push(pidx);
                    } else if cell.ch != ' ' {
                        let pidx = col as usize * lines as usize + line as usize;
                        self.phosphor_fresh.set(pidx, true);
                        self.phosphor[pidx] = captured_phosphor_energy(line, lines);
                        self.phosphor_base_ch[pidx] = cell.ch;
                        tracked_fresh.push(pidx);
                    }
                }
            }
        }

        // Pass 2: Update phosphor_layer from active droplets AND protect
        // active trail cells from phosphor decay.
        for d in &self.droplets {
            if d.bound_col == u16::MAX || !d.is_alive {
                continue;
            }
            let start = d.tail_put_line.map(|v| v.saturating_add(1)).unwrap_or(0);
            for line in start..=d.head_put_line {
                if line >= lines {
                    break;
                }
                let pidx = d.bound_col as usize * lines as usize + line as usize;
                if pidx < self.phosphor_layer.len() {
                    self.phosphor_layer[pidx] = d.layer;
                }
                if pidx < self.phosphor_fresh.len()
                    && !self.phosphor_fresh.get(pidx).is_some_and(|b| *b)
                {
                    self.phosphor_fresh.set(pidx, true);
                    self.phosphor[pidx] = captured_phosphor_energy(line, lines);
                    let fidx = line as usize * frame_width as usize + d.bound_col as usize;
                    let cell = frame.cell_at_index_ref(fidx);
                    if cell.fg.is_some() {
                        self.phosphor_base_fg[pidx] = cell.fg;
                        self.phosphor_base_ch[pidx] = cell.ch;
                    } else if cell.ch != ' ' {
                        self.phosphor_base_ch[pidx] = cell.ch;
                    }
                    tracked_fresh.push(pidx);
                }
            }
        }

        // Track newly active phosphor cells (dedup to prevent unbounded growth).
        for &pidx in &tracked_fresh {
            if !self.phosphor_active.contains(&pidx) {
                self.phosphor_active.push(pidx);
            }
        }

        // PERF(v10): Precompute per-frame decay factors for all (layer, bottom)
        // combinations.  There are PARALLAX_LAYERS (3) × 2 (normal/bottom) = 6
        // unique exp() values per frame.  Precomputing eliminates one exp() call
        // per decaying phosphor cell — typically 500-2000+ calls/frame.
        // Index: [layer * 2 + is_bottom]
        let base_decay = PHOSPHOR_DECAY_RATE * elapsed_sec;
        let bottom_base_decay = base_decay * PHOSPHOR_BOTTOM_DECAY_MULT;
        let mut decay_exp_factors = [1.0f32; PARALLAX_LAYERS * 2];
        for (i, &lm) in PHOSPHOR_LAYER_DECAY_MULT.iter().enumerate() {
            decay_exp_factors[i * 2] = (-base_decay * lm).exp();
            decay_exp_factors[i * 2 + 1] = (-bottom_base_decay * lm).exp();
        }

        // Pass 3: Decay non-fresh cells with phosphor energy.
        // OPTIMIZED: iterate only active phosphor cells instead of full grid.
        let mut i = 0;
        while i < self.phosphor_active.len() {
            let pidx = self.phosphor_active[i];
            if pidx >= total {
                self.phosphor_active.swap_remove(i);
                continue;
            }

            if self.phosphor_fresh.get(pidx).is_some_and(|b| *b) {
                i += 1;
                continue;
            }

            if self.phosphor[pidx] == 0 {
                self.phosphor_active.swap_remove(i);
                continue;
            }

            let col = (pidx / lines as usize) as u16;
            let line = (pidx % lines as usize) as u16;
            let fidx = line as usize * frame_width as usize + col as usize;

            let is_blank_current_gen = frame.cell_gen_at_index(fidx) == current_gen
                && frame.cell_at_index_ref(fidx).fg.is_none();

            if is_blank_current_gen {
                self.phosphor[pidx] = PHOSPHOR_TAIL_RESIDUAL;
                i += 1;
                continue;
            }

            if self.phosphor[pidx] == 255 {
                self.phosphor[pidx] = PHOSPHOR_TAIL_RESIDUAL;
            } else {
                // PERF(v10): Use precomputed exp() factor instead of per-cell exp() call.
                let layer = self.phosphor_layer[pidx] as usize;
                let layer_clamped = layer.min(PARALLAX_LAYERS - 1);
                let bottom_dist = lines.saturating_sub(line).saturating_sub(1);
                let is_bottom = (bottom_dist < PHOSPHOR_BOTTOM_ROWS) as usize;
                let factor = decay_exp_factors[layer_clamped * 2 + is_bottom];
                let new_energy = (self.phosphor[pidx] as f32 * factor) as u8;
                self.phosphor[pidx] = new_energy;
            }

            if self.phosphor[pidx] <= PHOSPHOR_DEAD_THRESHOLD {
                self.phosphor[pidx] = 0;
                self.phosphor_base_fg[pidx] = None;
                self.phosphor_base_ch[pidx] = '\0';
                self.phosphor_active.swap_remove(i);
                frame.set(col, line, blank_cell);
                continue;
            }

            if self.phosphor[pidx] < PHOSPHOR_GLYPH_THRESHOLD {
                self.phosphor_base_ch[pidx] = '\0';
                if let Some(base_fg) = self.phosphor_base_fg[pidx] {
                    let factor = self.phosphor[pidx] as f32 / 255.0;
                    let ghost_fg = if let Some((r, g, b)) = palette::decode_color(base_fg) {
                        palette::apply_brightness_rgb(r, g, b, factor)
                    } else {
                        base_fg
                    };
                    frame.set(
                        col,
                        line,
                        Cell {
                            ch: ' ',
                            fg: Some(ghost_fg),
                            bg,
                            bold: false,
                        },
                    );
                }
                i += 1;
                continue;
            }

            if let Some(base_fg) = self.phosphor_base_fg[pidx] {
                let factor = self.phosphor[pidx] as f32 / 255.0;
                let ghost_fg = if let Some((r, g, b)) = palette::decode_color(base_fg) {
                    palette::apply_brightness_rgb(r, g, b, factor)
                } else {
                    base_fg
                };
                let ghost_ch = self.phosphor_base_ch[pidx];
                frame.set(
                    col,
                    line,
                    Cell {
                        ch: if ghost_ch == '\0' { ' ' } else { ghost_ch },
                        fg: Some(ghost_fg),
                        bg,
                        bold: false,
                    },
                );
            } else if self.phosphor_base_ch[pidx] != '\0' {
                let factor = self.phosphor[pidx] as f32 / 255.0;
                let ghost_ch = self.phosphor_base_ch[pidx];
                let ghost_fg = self.palette.colors.first().copied().map(|c| {
                    if let Some((r, g, b)) = palette::decode_color(c) {
                        palette::apply_brightness_rgb(r, g, b, factor * 0.6)
                    } else {
                        c
                    }
                });
                frame.set(
                    col,
                    line,
                    Cell {
                        ch: ghost_ch,
                        fg: ghost_fg,
                        bg,
                        bold: false,
                    },
                );
            }

            i += 1;
        }
    }

    /// Spawn a rare anomaly zone at a random position.
    pub(super) fn spawn_anomaly(&mut self, now: Instant) {
        if self.anomaly_zones.len() >= ANOMALY_MAX_ZONES {
            return;
        }
        if self.cols == 0 || self.lines == 0 {
            return;
        }

        let col = self.rand_col.sample(&mut self.mt);
        let line = self.rand_line.sample(&mut self.mt);
        let radius = 3 + (self.rand_chance.sample(&mut self.mt) * 5.0) as u16; // 3-8

        let kind_roll = self.rand_chance.sample(&mut self.mt);
        let kind = if kind_roll < 0.4 {
            AnomalyKind::LuminanceSurge
        } else if kind_roll < 0.75 {
            AnomalyKind::GlyphCorruption
        } else {
            AnomalyKind::PulseWave
        };

        self.anomaly_zones.push(AnomalyZone {
            col,
            line,
            radius,
            kind,
            start_time: now,
        });
    }

    /// Apply active anomaly zone effects to the frame (post-processing).
    pub(super) fn apply_anomalies(&mut self, frame: &mut crate::frame::Frame, now: Instant) {
        if self.anomaly_zones.is_empty() {
            return;
        }

        let bg = self.palette.bg;
        let cols = self.cols;
        let lines = self.lines;
        let width = frame.width;

        for zone in &self.anomaly_zones {
            let elapsed = now.saturating_duration_since(zone.start_time).as_secs_f32();
            if elapsed >= ANOMALY_DURATION_SECS {
                continue;
            }

            let progress = elapsed / ANOMALY_DURATION_SECS; // 0..1
            let fade = 1.0 - progress; // fades out over duration

            match zone.kind {
                AnomalyKind::LuminanceSurge => {
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
                                let brightened = palette::blend_toward_white(fg, intensity);
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

                            // Use deterministic hash for stable corruption per cell
                            let hash = ((col as u32).wrapping_mul(2654435761)
                                ^ (line as u32).wrapping_mul(2246822519))
                                >> 31;
                            if (hash as f32 / 2.0) > ANOMALY_CORRUPTION_CHANCE * fade {
                                continue;
                            }

                            let fidx = line as usize * width as usize + col as usize;
                            let cell = frame.cell_at_index_ref(fidx);
                            if cell.fg.is_some() && !self.glitch_pool.is_empty() {
                                let cell_owned = frame.cell_at_index(fidx);
                                let glitch_idx = (col as usize + line as usize + elapsed as usize)
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
                                    let brightened = palette::blend_toward_white(fg, intensity);
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

    /// Apply global atmospheric effects to the frame.
    /// OPTIMIZED (v5.0.4): scans only dirty-cell indices instead of full O(w×h) grid.
    pub(super) fn apply_atmospheric_frame_effects(
        &self,
        frame: &mut crate::frame::Frame,
        now: Instant,
    ) {
        let luminance = self.color_ecosystem.luminance_climate;
        let saturation = self.color_ecosystem.saturation_climate;
        let instability = self.memory.instability_pressure;
        let persistence = self.memory.persistence_richness;
        let emergent = self.storytelling.active_effects(now);
        let profile = self.profile_current;

        // Skip if all modifiers are neutral
        let needs_luminance = (luminance - 1.0).abs() > 0.01
            || emergent.luminance_boost > 0.0
            || profile.luminance_offset.abs() > 0.01;
        let needs_saturation = (saturation - 1.0).abs() > 0.01;
        let needs_persistence = persistence.abs() > 0.01;

        if !needs_luminance && !needs_saturation && !needs_persistence {
            return;
        }

        // Collect dirty indices first to release immutable borrow before frame.set()
        let dirty_indices: smallvec::SmallVec<[usize; 256]> =
            frame.dirty_indices().iter().copied().collect();

        // Apply to dirty cells only (O(dirty) not O(w×h))
        // PERF(v10): Decode color to RGB once, chain all effects on raw tuples,
        // encode once at the end.  Eliminates 2-3 redundant color_to_rgb()
        // match+destructure cycles per cell when multiple effects are active.
        let bg = self.palette.bg;
        for &dirty_idx in &dirty_indices {
            let col = (dirty_idx % frame.width as usize) as u16;
            let line = (dirty_idx / frame.width as usize) as u16;
            if line >= self.lines || col >= self.cols {
                continue;
            }
            let cell = frame.cell_at_index(dirty_idx);
            if let Some(fg) = cell.fg {
                // Single decode — all effects operate on raw (r, g, b)
                let Some((mut r, mut g, mut b)) = palette::decode_color(fg) else {
                    continue;
                };

                if needs_luminance {
                    let total_lum = luminance + profile.luminance_offset + emergent.luminance_boost;
                    if total_lum < 1.0 {
                        let f = total_lum.clamp(0.0, 1.0);
                        let fi = (f * 256.0) as i32;
                        r = ((r as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
                        g = ((g as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
                        b = ((b as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
                    } else if total_lum > 1.0 {
                        let boost = (total_lum - 1.0).clamp(0.0, 0.3);
                        let wf = (boost * 256.0) as i32;
                        r = (r as i32 + ((255 - r as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                        g = (g as i32 + ((255 - g as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                        b = (b as i32 + ((255 - b as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                    }
                }

                if needs_saturation && saturation < 1.0 {
                    let f = saturation.clamp(0.0, 1.0);
                    let gray = ((r as u16 + g as u16 + b as u16) / 3) as u8;
                    // Inline lerp: gray + (channel - gray) * f  (8.8 fixed-point)
                    let ti = (f * 256.0) as i32;
                    r = (gray as i32 + ((r as i32 - gray as i32) * ti + 128) / 256).clamp(0, 255)
                        as u8;
                    g = (gray as i32 + ((g as i32 - gray as i32) * ti + 128) / 256).clamp(0, 255)
                        as u8;
                    b = (gray as i32 + ((b as i32 - gray as i32) * ti + 128) / 256).clamp(0, 255)
                        as u8;
                }

                if needs_persistence && persistence > 0.0 {
                    let wf = ((persistence * 0.3).clamp(0.0, 1.0) * 256.0) as i32;
                    r = (r as i32 + ((255 - r as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                    g = (g as i32 + ((255 - g as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                    b = (b as i32 + ((255 - b as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                }

                if instability > 0.15 {
                    let hash = (col as u32).wrapping_mul(2654435761)
                        ^ (line as u32).wrapping_mul(2246822519)
                        ^ (now.elapsed().as_secs() as u32);
                    if hash % 1000 < (instability * 50.0) as u32 {
                        let wf = ((instability * 0.1).clamp(0.0, 1.0) * 256.0) as i32;
                        r = (r as i32 + ((255 - r as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                        g = (g as i32 + ((255 - g as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                        b = (b as i32 + ((255 - b as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                    }
                }

                frame.set(
                    col,
                    line,
                    crate::cell::Cell {
                        ch: cell.ch,
                        fg: Some(Color::Rgb { r, g, b }),
                        bg,
                        bold: cell.bold,
                    },
                );
            }
        }
    }
}
