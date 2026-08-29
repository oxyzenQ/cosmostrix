// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Phosphor persistence, anomaly handling, and atmospheric frame effects.

use std::time::Instant;

use crossterm::style::Color;
use rand::distr::Distribution;

use crate::cell::Cell;
use crate::constants::*;
use crate::palette;
use crate::rain_style::RainStyle;

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

/// Phosphor ghost brightness helper. Shared by A18 (sub-threshold
/// ghost), A19 (main ghost trail), and A20 (orphan trail fallback).
///
/// Routes through the chroma engine when `is_chroma` is true (calls
/// `palette::apply_brightness_rgb`), falls back to
/// `chroma::legacy::scale_rgb` otherwise. Same equation both paths
/// (parity contract in `chroma/legacy.rs`); the branch exists for
/// audit symmetry with the A1-A17 sites in `droplet.rs`.
///
/// If `color` cannot be decoded (e.g. `Color::Reset`), returns `color`
/// unchanged -- matching the pre-extraction behavior at every call site.
#[inline]
fn phosphor_ghost_brightness(color: Color, factor: f32, is_chroma: bool) -> Color {
    if let Some((r, g, b)) = palette::decode_color(color) {
        if is_chroma {
            palette::apply_brightness_rgb(r, g, b, factor)
        } else {
            let (nr, ng, nb) = crate::chroma_dragon_engine::legacy::scale_rgb(r, g, b, factor);
            Color::Rgb {
                r: nr,
                g: ng,
                b: nb,
            }
        }
    } else {
        color
    }
}

/// Anomaly halo blend helper. Shared by A21 (`LuminanceSurge` halo)
/// and A22 (`PulseWave` halo).
///
/// Routes through the chroma engine when `is_chroma` is true (calls
/// `palette::blend_toward_bg` or `palette::blend_toward_white`), falls
/// back to `chroma::legacy::blend_toward_rgb` or
/// `chroma::legacy::blend_toward_white` otherwise. Same equation both
/// paths; the branch exists for audit symmetry with the A1-A20 sites.
///
/// When `halo_target` is `Some`, blends `fg` toward the target color.
/// When `None`, blends `fg` toward pure white (the pre-Phase-6
/// fallback for degenerate palette edge cases).
#[inline]
pub(super) fn anomaly_halo_blend(
    fg: Color,
    halo_target: Option<Color>,
    intensity: f32,
    is_chroma: bool,
) -> Color {
    if is_chroma {
        match halo_target {
            Some(t) => palette::blend_toward_bg(fg, t, intensity),
            None => palette::blend_toward_white(fg, intensity),
        }
    } else {
        let (r, g, b) = palette::decode_color(fg).unwrap_or((0, 0, 0));
        let (nr, ng, nb) = match halo_target {
            Some(t) => {
                let (tr, tg, tb) = palette::decode_color(t).unwrap_or((255, 255, 255));
                crate::chroma_dragon_engine::legacy::blend_toward_rgb(
                    r, g, b, tr, tg, tb, intensity,
                )
            }
            None => crate::chroma_dragon_engine::legacy::blend_toward_white(r, g, b, intensity),
        };
        Color::Rgb {
            r: nr,
            g: ng,
            b: nb,
        }
    }
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
    pub(crate) fn phosphor_decay_pass(
        &mut self,
        frame: &mut crate::frame::Frame,
        elapsed_sec: f32,
    ) {
        let total = (self.cols as usize) * (self.lines as usize);
        if total == 0 || self.phosphor.len() != total {
            return;
        }

        // M1 (internal independent QA): phosphor decay pressure gate with
        // hysteresis. Previously a hard-cut at >0.7 caused the CRT afterglow
        // effect to strobe on/off when pressure fluctuated around the
        // threshold. Now: skip when pressure > PHOSPHOR_SKIP_HIGH (0.70),
        // and stay skipped until pressure drops below PHOSPHOR_SKIP_LOW
        // (0.50). The `phosphor_skipped` field on Cloud tracks the current
        // state across frames — once skipped, it only resumes after pressure
        // drops well below the trigger.
        let should_skip = if self.phosphor_skipped {
            self.perf_pressure > PHOSPHOR_SKIP_LOW
        } else {
            self.perf_pressure > PHOSPHOR_SKIP_HIGH
        };
        if should_skip {
            self.phosphor_skipped = true;
            return;
        }
        self.phosphor_skipped = false;

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
        // PERF: incrementally clear only the bits that were set last frame
        // (saved in `phosphor_last_fresh`), instead of O(W×H) `fill(false)`.
        // For 200×60 terminal: 1,536 bytes memset → ~200 bit clears.
        for &pidx in &self.phosphor_last_fresh {
            if pidx < self.phosphor_fresh.len() {
                self.phosphor_fresh.set(pidx, false);
            }
        }
        let current_gen = frame.current_gen();
        // Reuse the heap capacity from last frame's `phosphor_last_fresh`
        // instead of allocating a fresh SmallVec every frame. The
        // `mem::take` + `clear()` pattern preserves any heap capacity the
        // SmallVec grew on a previous frame, so steady-state per-frame
        // allocation drops to ZERO after the first spill. Without this,
        // each frame allocates a new SmallVec (1 alloc + 1 dealloc) once
        // the fresh-cell count exceeds the 256-element inline capacity —
        // which happens at ~80×24 and larger, exactly the sizes where the
        // scaling audit showed 3-5 allocs/frame.
        //
        // We take the field (replacing it with the empty default) and use
        // it as our working buffer for this frame; at the end of the
        // function we leave it in place (no re-assignment) so the
        // capacity carries forward to the next frame.
        let mut tracked_fresh = std::mem::take(&mut self.phosphor_last_fresh);
        tracked_fresh.clear();

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
        //
        // PERF: skip entirely for Monolith scenes. The Monolith renderer
        // keeps `self.droplets` cleared (see spawn.rs:33 — Monolith path
        // calls `self.droplets.clear()` in reset()), so this loop would
        // iterate an empty Vec every frame — a no-op with the per-iteration
        // branch overhead. Skipping saves a Vec::iter() setup + zero-length
        // iterator state machine per frame. Monolith has its own dedicated
        // spine phosphor cleanup via `clear_spine_phosphor()` (called from
        // rain.rs:519-529), so this Pass 2 protection is structurally
        // unnecessary for that scene family.
        if !matches!(self.rain_style, RainStyle::Monolith) {
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
                    if pidx < self.phosphor_fresh.len() && !self.phosphor_fresh[pidx] {
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
        }

        // Track newly active phosphor cells (dedup via BitVec for O(1) check).
        // PERF: previously used `phosphor_active.contains(&pidx)` which is
        // O(N) linear scan per fresh cell — 5,000-100,000 wasted ops/frame.
        // The BitVec membership check is O(1) and eliminates the bottleneck.
        for &pidx in &tracked_fresh {
            // Cosmic Dragon egg #9: direct BitVec indexing — pidx from tracked_fresh
            // was pushed after bounds-check in the loop above.
            if !self.phosphor_in_active[pidx] {
                self.phosphor_active.push(pidx);
                self.phosphor_in_active.set(pidx, true);
            }
        }

        // Save tracked_fresh for next frame's incremental phosphor_fresh
        // clear. Since we `mem::take`-d the field at the top of the
        // function and reused it as our working buffer, we just move it
        // back into place here — no allocation, capacity carries forward.
        self.phosphor_last_fresh = tracked_fresh;

        // PERF(v10): Precompute per-frame decay factors for all (layer, bottom)
        // combinations.  There are PARALLAX_LAYERS (3) × 2 (normal/bottom) = 6
        // unique exp() values per frame.  Precomputing eliminates one exp() call
        // per decaying phosphor cell — typically 500-2000+ calls/frame.
        // Index: [layer * 2 + is_bottom]
        //
        // v50.0.0-beta.6: apply terminal-aware phosphor_decay_mult for
        // cross-terminal visual consistency. High-perf terminals keep 1.0
        // (current behavior). Standard/VTE terminals get 1.3 (faster decay).
        //
        // PERF-3: under aggressive_throttle (VTE fullscreen lag), boost decay
        // further so phosphor cells die faster — fewer dirty cells per frame
        // = less ANSI throughput = VTE can keep up. This is the "berbekas"
        // (stale trails) fix: the trailing afterglow is what overwhelms VTE
        // at high cell counts.
        //
        // Two-tier boost with hysteresis (prevents oscillation — owner
        // reported lag returning after a few seconds):
        //   1. Immediate: perf_pressure > 0.30 → boost 1.2x. Stays active
        //      until pressure drops below 0.15 (hysteresis deadband). The
        //      deadband prevents the boost from toggling on/off rapidly when
        //      pressure fluctuates around the threshold — which was causing
        //      the "lag hilang, beberapa detik kembali lagi" oscillation.
        //   2. Sustained: aggressive_throttle (self-healer fired after 30s)
        //      → boost 1.5x (stronger, for persistent overload).
        // The two compose multiplicatively for a max boost of 1.8x.
        let should_boost = if self.phosphor_pressure_boost_active {
            self.perf_pressure > 0.15 // hysteresis: release below 0.15
        } else {
            self.perf_pressure > 0.30 // hysteresis: trigger above 0.30
        };
        self.phosphor_pressure_boost_active = should_boost;
        let pressure_boost = if should_boost { 1.2 } else { 1.0 };
        let throttle_boost = if self.aggressive_throttle { 1.5 } else { 1.0 };
        let base_decay = PHOSPHOR_DECAY_RATE
            * self.phosphor_decay_mult
            * pressure_boost
            * throttle_boost
            * elapsed_sec;
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
                self.phosphor_in_active.set(pidx, false);
                continue;
            }

            // Cosmic Dragon egg #10: direct BitVec indexing — pidx from phosphor_active
            // was pushed after bounds-check.
            if self.phosphor_fresh[pidx] {
                i += 1;
                continue;
            }

            if self.phosphor[pidx] == 0 {
                self.phosphor_active.swap_remove(i);
                self.phosphor_in_active.set(pidx, false);
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
                self.phosphor_in_active.set(pidx, false);
                frame.set(col, line, blank_cell);
                continue;
            }

            // v50.0.0-beta.6: ghost brightness cap — kill dim ghosts early
            // on terminals where sub-pixel rendering makes them too visible.
            // When ghost_brightness_cap > 0.0, cells with energy below
            // cap * 255 are treated as dead (prevents long-tail perception
            // on VTE-based terminals like gnome-console).
            if self.ghost_brightness_cap > 0.0
                && (self.phosphor[pidx] as f32) < self.ghost_brightness_cap * 255.0
            {
                self.phosphor[pidx] = 0;
                self.phosphor_base_fg[pidx] = None;
                self.phosphor_base_ch[pidx] = '\0';
                self.phosphor_active.swap_remove(i);
                self.phosphor_in_active.set(pidx, false);
                frame.set(col, line, blank_cell);
                continue;
            }

            if self.phosphor[pidx] < PHOSPHOR_GLYPH_THRESHOLD {
                self.phosphor_base_ch[pidx] = '\0';
                if let Some(base_fg) = self.phosphor_base_fg[pidx] {
                    let factor = self.phosphor[pidx] as f32 / 255.0;
                    // (chroma audit, A18): sub-threshold ghost
                    // brightness -- shared helper routes through chroma
                    // engine when active, chroma::legacy::scale_rgb
                    // otherwise. Matches the A1-A17 is_chroma() branch
                    // pattern used in droplet.rs.
                    let ghost_fg =
                        phosphor_ghost_brightness(base_fg, factor, self.color_pipeline.is_chroma());
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
                // (chroma audit, A19): main ghost brightness (visible
                // trail) -- shared helper, same as A18. The branch exists
                // for audit symmetry with the A1-A17 sites in droplet.rs.
                let ghost_fg =
                    phosphor_ghost_brightness(base_fg, factor, self.color_pipeline.is_chroma());
                let ghost_ch = self.phosphor_base_ch[pidx];
                // Trail character cycling: 2% chance per decay step to
                // mutate the trail character to a new random glyph. This
                // makes the rain feel "alive" throughout the trail, not
                // just at the head — matching the film Matrix effect
                // where background characters subtly shift.
                let ghost_ch = if ghost_ch != '\0'
                    && !self.char_pool.is_empty()
                    && self.rand_chance.sample(&mut self.mt) < TRAIL_CYCLE_PROBABILITY
                {
                    let new_ch = self.char_pool
                        [self.rand_cpidx.sample(&mut self.mt) as usize % self.char_pool.len()];
                    self.phosphor_base_ch[pidx] = new_ch;
                    new_ch
                } else {
                    ghost_ch
                };
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
                // (chroma audit, A20): orphan trail fallback (rare
                // path -- no base_fg stored, derive from palette's first
                // stop). Shared helper, same as A18/A19; factor is
                // multiplied by 0.6 to dim the orphan trail relative to
                // a tracked trail.
                let is_chroma = self.color_pipeline.is_chroma();
                let ghost_fg = self
                    .palette
                    .colors
                    .first()
                    .copied()
                    .map(|c| phosphor_ghost_brightness(c, factor * 0.6, is_chroma));
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
    pub(crate) fn spawn_anomaly(&mut self, now: Instant) {
        // PERF-4 strengthen: --no-effects gate. Anomaly zones (LuminanceSurge,
        // GlyphCorruption, PulseWave) are visually disruptive post-process
        // particle-like effects. Without this gate they continued to spawn
        // under --no-effects — a partial-disable leak. Early-return here
        // ensures no new zones are pushed; existing zones fade out on their
        // own expiry tick (rain.rs::update_anomaly_zones retains by expiry).
        if !self.effects_enabled {
            return;
        }
        if self.anomaly_zones.len() >= ANOMALY_MAX_ZONES {
            return;
        }
        if self.cols == 0 || self.lines == 0 {
            return;
        }

        let col = self.rand_col.sample(&mut self.mt);
        let line = self.rand_line.sample(&mut self.mt);
        let radius = 3 + (self.rand_chance.sample(&mut self.mt) * 5.0) as u16; // 3..=7 (rand excludes 1.0, so *5 yields [0,5), +3 → 3..=7)

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
}
