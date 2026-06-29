// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Lightning atmospheric event — precomputed bolt path with optional branches.
//!
//! The bolt path is fully computed at spawn time and stored in pre-allocated
//! buffers. Rendering iterates the precomputed path with zero per-frame
//! allocation. Decay is handled via phosphor integration.

use std::time::Instant;

use crossterm::style::Color;
use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::cell::Cell;
use crate::constants::*;
use crate::frame::Frame;

use super::super::atmospheric_events::{AtmosphericEvent, EventCtx, EventState};
use super::helpers::{apply_brightness_mult, apply_white_blend};

/// Phase within the lightning lifecycle.
#[derive(Clone, Copy, Debug, PartialEq)]
enum LightningPhase {
    /// 0-50ms: bolt appears at peak intensity.
    Strike,
    /// 50-200ms: secondary glow, branch visibility, flash.
    Flash,
    /// 200-700ms: phosphor-powered fade (no per-frame rendering).
    Decay,
    /// Event complete.
    Finished,
}

/// A single lightning bolt with optional branches.
///
/// All path data is precomputed at spawn. Rendering iterates the stored
/// path arrays — no simulation, no recomputation, no per-frame allocation.
pub(crate) struct LightningEvent {
    /// Current lifecycle phase.
    phase: LightningPhase,
    /// When the current phase began.
    phase_start: Instant,
    /// When the event was spawned.
    spawn_time: Instant,
    /// Overall intensity multiplier (0.0-2.0, default 1.0).
    intensity: f32,
    /// Terminal dimensions at spawn time.
    cols: u16,
    lines: u16,

    // ── Precomputed bolt path ──
    /// Main bolt: (col, line) pairs from top to bottom.
    main_bolt: Vec<(u16, u16)>,
    /// Bolt characters for each main bolt segment.
    bolt_chars: Vec<char>,
    /// Branches: each branch is a list of (col, line) pairs.
    branches: Vec<Vec<(u16, u16)>>,
    /// Flash cells: (col, line, falloff_factor) for cells within flash radius.
    flash_cells: Vec<(u16, u16, f32)>,
    /// Last palette color, captured at spawn for phosphor seeding.
    last_palette_color: Option<Color>,
}

impl LightningEvent {
    /// Create a new lightning event with paths precomputed.
    pub fn new(
        cols: u16,
        lines: u16,
        rng: &mut StdRng,
        intensity: f32,
        palette_color: Option<Color>,
    ) -> Self {
        let now = Instant::now();
        let mut event = Self {
            phase: LightningPhase::Strike,
            phase_start: now,
            spawn_time: now,
            intensity: intensity.clamp(0.1, 2.0),
            cols,
            lines,
            main_bolt: Vec::new(),
            bolt_chars: Vec::new(),
            branches: Vec::new(),
            flash_cells: Vec::new(),
            last_palette_color: palette_color,
        };
        event.generate_paths(rng);
        event
    }

    /// Generate the main bolt path, branches, and flash cells.
    fn generate_paths(&mut self, rng: &mut StdRng) {
        if self.cols < 4 || self.lines < 4 {
            return;
        }

        let chance = Uniform::new(0.0f32, 1.0f32).expect("chance [0,1) always valid");

        // ── Main bolt path ──
        let max_wander = ((self.cols as f32) * LIGHTNING_WANDER_FRACTION) as u16;
        let center = self.cols / 2;
        let start_col = if max_wander > 0 {
            let offset = ((chance.sample(rng) * max_wander as f32 * 2.0) as i32)
                .saturating_sub(max_wander as i32);
            (center as i32 + offset).clamp(0, (self.cols - 1) as i32) as u16
        } else {
            center
        };

        let mut col = start_col;
        let mut prev_col = col;
        let mut line: u16 = 0;
        let zigzag_avg = LIGHTNING_ZIGZAG_AVG.max(1);

        while line < self.lines && self.main_bolt.len() < LIGHTNING_PATH_CAPACITY {
            self.main_bolt.push((col, line));

            // Direction change every ~zigzag_avg rows
            if (line % zigzag_avg) == 0 || col == prev_col {
                let dir: i16 = if chance.sample(rng) < 0.5 { -1 } else { 1 };
                let step = (chance.sample(rng) * LIGHTNING_HSTEP_MAX as f32).ceil() as i16;
                let new_col = (col as i16 + dir * step).clamp(0, (self.cols - 1) as i16) as u16;
                prev_col = col;
                col = new_col;
            }

            let dcol = col as i16 - prev_col as i16;
            self.bolt_chars
                .push(super::helpers::bolt_char_for_step(dcol, 1, rng));

            let vstep = LIGHTNING_VSTEP_MIN as f32
                + chance.sample(rng) * (LIGHTNING_VSTEP_MAX - LIGHTNING_VSTEP_MIN) as f32;
            line = line.saturating_add(vstep.ceil() as u16);
        }

        // ── Branches ──
        if self.main_bolt.len() >= 5 {
            let num_branches = if chance.sample(rng) < 0.4 {
                0
            } else if chance.sample(rng) < 0.35 {
                1
            } else if chance.sample(rng) < 0.2 {
                2
            } else {
                3
            };

            for _ in 0..num_branches {
                let branch_root =
                    (chance.sample(rng) * (self.main_bolt.len() as f32 * 0.6)) as usize;
                if let Some(&(root_col, root_line)) = self.main_bolt.get(branch_root) {
                    let max_branch_len = self.main_bolt.len() - branch_root;
                    let branch_len =
                        ((0.3 + chance.sample(rng) * 0.4) * max_branch_len as f32) as usize;
                    let branch_len = branch_len.clamp(3, LIGHTNING_BRANCH_CAPACITY);

                    let mut branch: Vec<(u16, u16)> = Vec::new();
                    let mut bcol = root_col;
                    let mut bline = root_line;

                    let branch_dir: i16 = if chance.sample(rng) < 0.5 { -1 } else { 1 };

                    for _ in 0..branch_len {
                        let step = (chance.sample(rng) * 3.0).ceil() as i16;
                        bcol = (bcol as i16 + branch_dir * step).clamp(0, (self.cols - 1) as i16)
                            as u16;
                        bline = bline
                            .saturating_add((1.0 + chance.sample(rng) * 2.0).ceil() as u16)
                            .min(self.lines.saturating_sub(1));
                        branch.push((bcol, bline));
                    }
                    self.branches.push(branch);
                }
            }
        }

        // ── Flash cells ──
        if LIGHTNING_FLASH_RADIUS > 0 {
            let flash_col_min = self
                .main_bolt
                .iter()
                .map(|&(c, _)| c)
                .min()
                .unwrap_or(0)
                .saturating_sub(LIGHTNING_FLASH_RADIUS);
            let flash_col_max = self
                .main_bolt
                .iter()
                .map(|&(c, _)| c)
                .max()
                .unwrap_or(self.cols.saturating_sub(1))
                .saturating_add(LIGHTNING_FLASH_RADIUS)
                .min(self.cols.saturating_sub(1));
            let flash_line_min = 0u16;
            let flash_line_max = self
                .main_bolt
                .last()
                .map(|&(_, l)| l)
                .unwrap_or(self.lines.saturating_sub(1))
                .saturating_add(LIGHTNING_FLASH_RADIUS)
                .min(self.lines.saturating_sub(1));

            for fc in flash_col_min..=flash_col_max {
                for fl in flash_line_min..=flash_line_max {
                    if self.flash_cells.len() >= LIGHTNING_FLASH_CAPACITY {
                        break;
                    }
                    // Find minimum distance to any bolt segment
                    let mut min_dist = f32::MAX;
                    for &(bc, bl) in &self.main_bolt {
                        let dx = fc as f32 - bc as f32;
                        let dy = fl as f32 - bl as f32;
                        let dist = (dx * dx + dy * dy).sqrt();
                        if dist < min_dist {
                            min_dist = dist;
                        }
                    }
                    if min_dist <= LIGHTNING_FLASH_RADIUS as f32 {
                        let sigma = LIGHTNING_FLASH_SIGMA;
                        let closest_falloff = if sigma > 0.0 {
                            (-(min_dist * min_dist) / (2.0 * sigma * sigma)).exp()
                        } else {
                            1.0
                        };
                        self.flash_cells.push((fc, fl, closest_falloff));
                    }
                }
            }
        }
    }

    /// Determine the current brightness multiplier based on phase and elapsed time.
    fn phase_brightness(&self, now: Instant, intensity: f32) -> f32 {
        match self.phase {
            LightningPhase::Strike => {
                let strike_ms = (LIGHTNING_ACTIVE_MS as f32) * LIGHTNING_STRIKE_FRACTION;
                let elapsed = now.saturating_duration_since(self.phase_start).as_millis() as f32;
                // Instant peak, exponential decay: exp(-t × 6.0)
                // Frame 0 (t=0ms):   1.00 — instant full brightness
                // Frame 1 (t=17ms):  0.36 — sharp falloff
                // Frame 2 (t=33ms):  0.14 — rapid decay
                // Frame 3 (t=50ms):  0.05 — into flash phase
                let progress = (elapsed / strike_ms.max(1.0)).min(1.0);
                intensity * (-progress * 6.0).exp()
            }
            LightningPhase::Flash => {
                let active_ms = LIGHTNING_ACTIVE_MS as f32;
                let strike_ms = active_ms * LIGHTNING_STRIKE_FRACTION;
                let flash_ms = active_ms - strike_ms;
                let elapsed =
                    now.saturating_duration_since(self.phase_start).as_millis() as f32 - strike_ms;
                let progress = (elapsed / flash_ms.max(1.0)).min(1.0);
                // Exponential fade during flash phase
                let decay = (-progress * 2.0).exp();
                intensity * decay
            }
            _ => 0.0,
        }
    }
}

impl AtmosphericEvent for LightningEvent {
    fn state(&self) -> EventState {
        match self.phase {
            LightningPhase::Strike | LightningPhase::Flash => EventState::Active,
            LightningPhase::Decay => EventState::Decay,
            LightningPhase::Finished => EventState::Finished,
        }
    }

    fn is_finished(&self) -> bool {
        self.phase == LightningPhase::Finished
    }

    fn phase_durations_ms(&self) -> (u64, u64) {
        (LIGHTNING_ACTIVE_MS, LIGHTNING_DECAY_MS)
    }

    fn memory_footprint(&self) -> usize {
        self.main_bolt.capacity() * std::mem::size_of::<(u16, u16)>()
            + self.bolt_chars.capacity() * std::mem::size_of::<char>()
            + self
                .branches
                .iter()
                .map(|b| b.capacity() * std::mem::size_of::<(u16, u16)>())
                .sum::<usize>()
            + self.flash_cells.capacity() * std::mem::size_of::<(u16, u16, f32)>()
    }

    fn update(&mut self, now: Instant) {
        let total_active_ms = LIGHTNING_ACTIVE_MS as u128;
        let total_decay_ms = LIGHTNING_DECAY_MS as u128;
        let elapsed = now.saturating_duration_since(self.spawn_time).as_millis();

        self.phase = if elapsed < (total_active_ms * LIGHTNING_STRIKE_FRACTION as u128 / 100) {
            LightningPhase::Strike
        } else if elapsed < total_active_ms {
            LightningPhase::Flash
        } else if elapsed < total_active_ms + total_decay_ms {
            LightningPhase::Decay
        } else {
            LightningPhase::Finished
        };
    }

    fn render(&self, ctx: &EventCtx, frame: &mut Frame) {
        if matches!(self.phase, LightningPhase::Finished | LightningPhase::Decay) {
            return;
        }

        let intensity = self.phase_brightness(ctx.now, self.intensity);
        if intensity <= 0.01 {
            return;
        }
        let bg = ctx.bg;

        // Helper: skip message box cells
        let skip_msg = |col: u16, line: u16| -> bool {
            if let Some((mx, my, mw, mh)) = ctx.message_bounds {
                col >= mx && col < mx + mw && line >= my && line < my + mh
            } else {
                false
            }
        };

        // Helper: write a cell with intensity-based color
        let mut write_cell = |col: u16, line: u16, ch: char, base_intensity: f32, is_core: bool| {
            if skip_msg(col, line) || col >= self.cols || line >= self.lines {
                return;
            }
            let effective = (intensity * base_intensity).clamp(0.0, 1.0);
            if effective < 0.02 {
                return;
            }

            // Get base color from palette
            let Some(&base_color) = ctx.palette_colors.last() else {
                return;
            };
            let Some((mut r, mut g, mut b)) = crate::palette::decode_color(base_color) else {
                return;
            };

            // Apply intensity dimming
            (r, g, b) = apply_brightness_mult(r, g, b, effective);

            // Core boost for main bolt center
            if is_core && effective > 0.3 {
                (r, g, b) = apply_white_blend(r, g, b, LIGHTNING_CORE_BOOST * effective);
            }

            frame.set_force(
                col,
                line,
                Cell {
                    ch,
                    fg: Some(Color::Rgb { r, g, b }),
                    bg,
                    bold: effective > 0.5,
                },
            );
        };

        // ── Render main bolt ──
        for (i, &(col, line)) in self.main_bolt.iter().enumerate() {
            let ch = self.bolt_chars.get(i).copied().unwrap_or('│');
            // Main bolt at full brightness, tapering slightly toward bottom
            let bolt_intensity = 1.0 - (i as f32 / self.main_bolt.len().max(1) as f32) * 0.3;
            write_cell(col, line, ch, bolt_intensity, true);

            // Horizontal fuzz: adjacent cells at reduced brightness
            if col > 0 {
                write_cell(col - 1, line, '┃', bolt_intensity * 0.5, false);
            }
            if col + 1 < self.cols {
                write_cell(col + 1, line, '┃', bolt_intensity * 0.5, false);
            }
        }

        // ── Render branches ──
        for branch in &self.branches {
            for &(col, line) in branch.iter() {
                write_cell(col, line, '╱', LIGHTNING_BRANCH_BRIGHTNESS, false);
            }
        }

        // ── Render flash (background glow) ──
        if self.phase == LightningPhase::Flash {
            for &(col, line, falloff) in &self.flash_cells {
                if skip_msg(col, line) || col >= self.cols || line >= self.lines {
                    continue;
                }
                let glow = intensity * falloff * LIGHTNING_FLASH_INTENSITY;
                if glow < 0.015 {
                    continue;
                }
                let Some(&base_color) = ctx.palette_colors.last() else {
                    continue;
                };
                let Some((r, g, b)) = crate::palette::decode_color(base_color) else {
                    continue;
                };
                let (r, g, b) = apply_white_blend(r, g, b, glow.clamp(0.0, 1.0));

                frame.set_force(
                    col,
                    line,
                    Cell {
                        ch: ' ',
                        fg: Some(Color::Rgb { r, g, b }),
                        bg,
                        bold: false,
                    },
                );
            }
        }
    }

    fn pulse_factor(&self, now: Instant) -> f32 {
        if !matches!(self.phase, LightningPhase::Strike) {
            return 0.0;
        }
        let strike_ms = (LIGHTNING_ACTIVE_MS as f32) * LIGHTNING_STRIKE_FRACTION;
        let elapsed = now.saturating_duration_since(self.phase_start).as_millis() as f32;
        let progress = (elapsed / strike_ms.max(1.0)).min(1.0);
        // Sharp pulse: instant peak, rapid decay.
        // t=0ms: 1.0  t=17ms: 0.22  t=33ms: 0.05
        (-progress * 4.0).exp()
    }

    fn seed_phosphor(
        &self,
        phosphor: &mut [u8],
        phosphor_base_fg: &mut [Option<Color>],
        phosphor_base_ch: &mut [char],
        cols: u16,
        lines: u16,
    ) {
        let total = (cols as usize) * (lines as usize);
        if phosphor.len() != total || total == 0 {
            return;
        }
        let seed_energy = EVENT_PHOSPHOR_SEED_ENERGY;

        // Seed phosphor for main bolt cells
        for &(col, line) in &self.main_bolt {
            let pidx = col as usize * lines as usize + line as usize;
            if pidx < total {
                phosphor[pidx] = phosphor[pidx].max(seed_energy);
                if phosphor_base_fg[pidx].is_none() {
                    phosphor_base_fg[pidx] = self.last_palette_color;
                }
                phosphor_base_ch[pidx] = '│';
            }
        }

        // Seed phosphor for flash cells (lower energy)
        for &(col, line, _falloff) in &self.flash_cells {
            let pidx = col as usize * lines as usize + line as usize;
            if pidx < total && phosphor[pidx] == 0 {
                phosphor[pidx] = seed_energy / 3;
                if phosphor_base_fg[pidx].is_none() {
                    phosphor_base_fg[pidx] = self.last_palette_color;
                }
            }
        }
    }
}
