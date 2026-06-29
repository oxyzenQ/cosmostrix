// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Lightning atmospheric event — precomputed bolt path with optional branches.
//!
//! The bolt path is fully computed at spawn time and stored in pre-allocated
//! buffers. Rendering iterates the precomputed path with zero per-frame
//! allocation. Decay is handled via phosphor integration.
//!
//! ## Bolt Families (v10.0.0 Phase 2D)
//!
//! - 0: Straight — minimal zigzag, length 0.4-0.9, brightness 0.8
//! - 1: Jagged — sharp zigzag, length 0.5-1.0, brightness 1.0
//! - 2: Forked — many branches, length 0.6-1.0, brightness 0.9
//! - 3: Broken — gaps, length 0.3-0.7, brightness 0.7
//! - 4: Ribbon — thick, gentle curves, length 0.5-1.0, brightness 1.1
//! - 5: Heavy — rare, thick, full-screen, length 0.8-1.0, brightness 1.3
//!
//! ## Return Strokes
//!
//! 25% chance of 1-2 secondary flashes after initial bolt, with a 40-80ms
//! dark gap between strokes at 60-80% brightness.

use std::time::{Duration, Instant};

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
    /// Return stroke dark gap (brief pause before secondary flash).
    ReturnStrokeDark,
    /// Return stroke flash (secondary peak at reduced brightness).
    ReturnStrokeFlash,
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

    // ── Bolt family (v10.0.0 Phase 2D) ──
    /// Bolt family index (0-5).
    bolt_family: u8,
    /// Target length as fraction of screen height.
    length_pct: f32,
    /// Per-family brightness multiplier.
    family_brightness: f32,

    // ── Return strokes (v10.0.0 Phase 2D) ──
    /// Total return strokes configured.
    return_stroke_count: u8,
    /// Return strokes completed so far.
    return_stroke_done: u8,
    /// True during a return stroke flash.
    #[allow(dead_code)]
    return_stroke_phase: bool,
    /// Dark gap end time (when the return flash should begin).
    return_stroke_dark_until: Option<Instant>,

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
    /// `bolt_family`: 0-5 family index
    /// `length_pct`: target length as fraction of screen height
    /// `return_strokes`: 0-2 return strokes after the initial bolt
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cols: u16,
        lines: u16,
        rng: &mut StdRng,
        intensity: f32,
        palette_color: Option<Color>,
        bolt_family: u8,
        length_pct: f32,
        return_strokes: u8,
    ) -> Self {
        let now = Instant::now();

        // Determine per-family brightness
        let family_brightness = match bolt_family {
            0 => 0.8,
            1 => 1.0,
            2 => 0.9,
            3 => 0.7,
            4 => 1.1,
            _ => 1.3,
        };

        let mut event = Self {
            phase: LightningPhase::Strike,
            phase_start: now,
            spawn_time: now,
            intensity: intensity.clamp(0.1, 2.0),
            cols,
            lines,
            bolt_family,
            length_pct: length_pct.clamp(0.1, 1.0),
            family_brightness,
            return_stroke_count: return_strokes.min(2),
            return_stroke_done: 0,
            return_stroke_phase: false,
            return_stroke_dark_until: None,
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
    /// Path generation is family-specific.
    fn generate_paths(&mut self, rng: &mut StdRng) {
        if self.cols < 4 || self.lines < 4 {
            return;
        }

        let chance = Uniform::new(0.0f32, 1.0f32).expect("chance [0,1) always valid");

        let max_wander = ((self.cols as f32) * LIGHTNING_WANDER_FRACTION) as u16;
        let center = self.cols / 2;
        let start_col = if max_wander > 0 {
            let offset = ((chance.sample(rng) * max_wander as f32 * 2.0) as i32)
                .saturating_sub(max_wander as i32);
            (center as i32 + offset).clamp(0, (self.cols - 1) as i32) as u16
        } else {
            center
        };

        // Per-family zigzag and step parameters
        let (zigzag_avg, hstep_max) = match self.bolt_family {
            0 => (8u16, 1i16),  // Straight: minimal changes
            1 => (2u16, 4i16),  // Jagged: frequent, sharp
            2 => (2u16, 5i16),  // Forked: like Jagged but wider
            3 => (6u16, 3i16),  // Broken: moderate but with gaps
            4 => (6u16, 2i16),  // Ribbon: gentle curves
            _ => (10u16, 1i16), // Heavy: slow, deliberate
        };

        // Calculate target line based on length_pct
        let target_line = ((self.lines as f32) * self.length_pct) as u16;
        let target_line = target_line.min(self.lines);

        let mut col = start_col;
        let mut prev_col = col;
        let mut line: u16 = 0;
        let zigzag_avg = zigzag_avg.max(1);

        // Pre-compute gap pattern for Broken family
        let gap_every = if self.bolt_family == 3 {
            // Gap every 4th-6th segment
            4 + (chance.sample(rng) * 3.0) as usize
        } else {
            0 // No gaps
        };
        let mut segment_index: usize = 0;

        while line < target_line && self.main_bolt.len() < LIGHTNING_PATH_CAPACITY {
            // Broken family: skip segments for gaps
            let should_skip = if self.bolt_family == 3 && gap_every > 0 {
                segment_index > 0 && segment_index % gap_every == 0
            } else {
                false
            };

            if !should_skip {
                self.main_bolt.push((col, line));
            }

            // Direction change
            if (line % zigzag_avg) == 0 || col == prev_col {
                let dir: i16 = if chance.sample(rng) < 0.5 { -1 } else { 1 };
                let step = (chance.sample(rng) * hstep_max as f32).ceil() as i16;
                // For Ribbon family, reduce step size further
                let step = if self.bolt_family == 4 {
                    step.max(1)
                } else {
                    step
                };
                let new_col = (col as i16 + dir * step).clamp(0, (self.cols - 1) as i16) as u16;
                prev_col = col;
                col = new_col;
            }

            // Character for this segment (or skip for broken family)
            if !should_skip {
                let dcol = col as i16 - prev_col as i16;
                self.bolt_chars
                    .push(super::helpers::bolt_char_for_step(dcol, 1, rng));
            }

            // Vertical step — per-family
            let vstep_min: f32 = if self.bolt_family == 5 {
                1.0 // Heavy: slow progression
            } else {
                LIGHTNING_VSTEP_MIN as f32
            };
            let vstep_max = match self.bolt_family {
                5 => 2.0, // Heavy: very slow
                4 => 4.0, // Ribbon: moderate
                0 => 5.0, // Straight: faster
                _ => LIGHTNING_VSTEP_MAX as f32,
            };
            let vstep = vstep_min + chance.sample(rng) * (vstep_max - vstep_min);
            line = line.saturating_add(vstep.ceil() as u16);
            segment_index += 1;
        }

        // ── Branches ──
        let branch_prob: f32;
        let min_branches: usize;
        let max_branches: usize;

        match self.bolt_family {
            2 => {
                // Forked: 50-80% branch probability, 2-4 branches
                branch_prob = 0.5 + chance.sample(rng) * 0.3;
                min_branches = 2;
                max_branches = 4;
            }
            4 => {
                // Ribbon: occasional branch
                branch_prob = 0.25;
                min_branches = 1;
                max_branches = 2;
            }
            _ => {
                branch_prob = 0.35;
                min_branches = 1;
                max_branches = 3;
            }
        }

        if self.main_bolt.len() >= 5 && chance.sample(rng) < branch_prob {
            let num_branches = min_branches
                + (chance.sample(rng) * (max_branches - min_branches + 1) as f32) as usize;
            let num_branches = num_branches.min(LIGHTNING_MAX_BRANCHES);

            for _ in 0..num_branches {
                let branch_root = if self.bolt_family == 2 {
                    // Forked: branches from various points
                    (chance.sample(rng) * (self.main_bolt.len() as f32 * 0.7)) as usize
                } else {
                    (chance.sample(rng) * (self.main_bolt.len() as f32 * 0.5)) as usize
                };
                if let Some(&(root_col, root_line)) = self.main_bolt.get(branch_root) {
                    let max_branch_len = self.main_bolt.len() - branch_root;
                    let branch_len = if self.bolt_family == 2 {
                        // Forked: longer branches
                        ((0.4 + chance.sample(rng) * 0.4) * max_branch_len as f32) as usize
                    } else {
                        ((0.2 + chance.sample(rng) * 0.3) * max_branch_len as f32) as usize
                    };
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
        let flash_radius = match self.bolt_family {
            5 => LIGHTNING_FLASH_RADIUS + 6, // Heavy: wider flash
            4 => LIGHTNING_FLASH_RADIUS + 3, // Ribbon: thicker
            _ => LIGHTNING_FLASH_RADIUS,
        };

        if flash_radius > 0 && !self.main_bolt.is_empty() {
            let flash_col_min = self
                .main_bolt
                .iter()
                .map(|&(c, _)| c)
                .min()
                .unwrap_or(0)
                .saturating_sub(flash_radius);
            let flash_col_max = self
                .main_bolt
                .iter()
                .map(|&(c, _)| c)
                .max()
                .unwrap_or(self.cols.saturating_sub(1))
                .saturating_add(flash_radius)
                .min(self.cols.saturating_sub(1));
            let flash_line_min = 0u16;
            let flash_line_max = self
                .main_bolt
                .last()
                .map(|&(_, l)| l)
                .unwrap_or(self.lines.saturating_sub(1))
                .saturating_add(flash_radius)
                .min(self.lines.saturating_sub(1));

            for fc in flash_col_min..=flash_col_max {
                for fl in flash_line_min..=flash_line_max {
                    if self.flash_cells.len() >= LIGHTNING_FLASH_CAPACITY {
                        break;
                    }
                    let mut min_dist = f32::MAX;
                    for &(bc, bl) in &self.main_bolt {
                        let dx = fc as f32 - bc as f32;
                        let dy = fl as f32 - bl as f32;
                        let dist = (dx * dx + dy * dy).sqrt();
                        if dist < min_dist {
                            min_dist = dist;
                        }
                    }
                    if min_dist <= flash_radius as f32 {
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
        let base_intensity = intensity * self.family_brightness;

        match self.phase {
            LightningPhase::Strike => {
                let strike_ms = (LIGHTNING_ACTIVE_MS as f32) * LIGHTNING_STRIKE_FRACTION;
                let elapsed = now.saturating_duration_since(self.phase_start).as_millis() as f32;
                let progress = (elapsed / strike_ms.max(1.0)).min(1.0);
                base_intensity * (-progress * 6.0).exp()
            }
            LightningPhase::Flash => {
                let active_ms = LIGHTNING_ACTIVE_MS as f32;
                let strike_ms = active_ms * LIGHTNING_STRIKE_FRACTION;
                let flash_ms = active_ms - strike_ms;
                let elapsed =
                    now.saturating_duration_since(self.phase_start).as_millis() as f32 - strike_ms;
                let progress = (elapsed / flash_ms.max(1.0)).min(1.0);
                let decay = (-progress * 2.0).exp();
                base_intensity * decay
            }
            LightningPhase::ReturnStrokeFlash => {
                // Return stroke at 60-80% of original brightness
                let return_brightness = 0.6 + (self.return_stroke_done as f32 * 0.1);
                let elapsed = now.saturating_duration_since(self.phase_start).as_millis() as f32;
                let strike_ms = (LIGHTNING_ACTIVE_MS as f32) * LIGHTNING_STRIKE_FRACTION * 0.6;
                let progress = (elapsed / strike_ms.max(1.0)).min(1.0);
                base_intensity * return_brightness * (-progress * 4.0).exp()
            }
            _ => 0.0,
        }
    }
}

impl AtmosphericEvent for LightningEvent {
    fn state(&self) -> EventState {
        match self.phase {
            LightningPhase::Strike
            | LightningPhase::Flash
            | LightningPhase::ReturnStrokeDark
            | LightningPhase::ReturnStrokeFlash => EventState::Active,
            LightningPhase::Decay => EventState::Decay,
            LightningPhase::Finished => EventState::Finished,
        }
    }

    fn is_finished(&self) -> bool {
        self.phase == LightningPhase::Finished
    }

    fn phase_durations_ms(&self) -> (u64, u64) {
        // Return strokes extend the active duration
        let extra_active = self.return_stroke_count as u64 * 120; // ~120ms per return stroke
        (LIGHTNING_ACTIVE_MS + extra_active, LIGHTNING_DECAY_MS)
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

        // Check return stroke transitions
        if self.return_stroke_count > 0 && self.return_stroke_done < self.return_stroke_count {
            match self.phase {
                LightningPhase::Strike | LightningPhase::Flash => {
                    // After active phase ends, transition to return stroke dark gap
                    if elapsed >= total_active_ms {
                        // Dark gap duration (40-80ms)
                        let dark_gap_ms = 40u128 + (self.return_stroke_done as u128 * 20);
                        self.phase = LightningPhase::ReturnStrokeDark;
                        self.phase_start = now;
                        self.return_stroke_dark_until =
                            Some(now + Duration::from_millis(dark_gap_ms as u64));
                        return;
                    }
                }
                LightningPhase::ReturnStrokeDark => {
                    if let Some(dark_until) = self.return_stroke_dark_until {
                        if now >= dark_until {
                            self.phase = LightningPhase::ReturnStrokeFlash;
                            self.phase_start = now;
                            self.return_stroke_done += 1;
                            self.return_stroke_dark_until = None;
                            return;
                        }
                    } else {
                        // Fallback: if dark_until was none, advance after some time
                        let dark_elapsed =
                            now.saturating_duration_since(self.phase_start).as_millis();
                        if dark_elapsed > 60 {
                            self.phase = LightningPhase::ReturnStrokeFlash;
                            self.phase_start = now;
                            self.return_stroke_done += 1;
                            return;
                        }
                    }
                    return; // Don't apply normal phase transitions during dark gap
                }
                LightningPhase::ReturnStrokeFlash => {
                    let flash_elapsed = now.saturating_duration_since(self.phase_start).as_millis();
                    let return_flash_ms = 60u128;
                    if flash_elapsed >= return_flash_ms {
                        if self.return_stroke_done < self.return_stroke_count {
                            // Another dark gap before next return stroke
                            let dark_gap_ms = 40u128 + (self.return_stroke_done as u128 * 20);
                            self.phase = LightningPhase::ReturnStrokeDark;
                            self.phase_start = now;
                            self.return_stroke_dark_until =
                                Some(now + Duration::from_millis(dark_gap_ms as u64));
                            return;
                        } else {
                            // All return strokes done, go to decay
                            self.phase = LightningPhase::Decay;
                            return;
                        }
                    }
                    return;
                }
                _ => {}
            }
        }

        // Normal phase transitions (no return strokes or after all return strokes done)
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
        if matches!(
            self.phase,
            LightningPhase::Finished | LightningPhase::Decay | LightningPhase::ReturnStrokeDark
        ) {
            // Don't render during dark gap or decay/finished
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
            let bolt_intensity = 1.0 - (i as f32 / self.main_bolt.len().max(1) as f32) * 0.3;

            // Per-family rendering width
            let extra_width: u16 = match self.bolt_family {
                4 | 5 => 2, // Ribbon and Heavy: thick (2-3 wide)
                _ => 1,     // Others: standard width
            };

            write_cell(col, line, ch, bolt_intensity, true);

            // Horizontal fuzz / width
            for w in 1..=extra_width {
                if col >= w {
                    write_cell(col - w, line, '┃', bolt_intensity * 0.4, false);
                }
                if col + w < self.cols {
                    write_cell(col + w, line, '┃', bolt_intensity * 0.4, false);
                }
            }
        }

        // ── Render branches ──
        let branch_brightness = match self.bolt_family {
            2 => LIGHTNING_BRANCH_BRIGHTNESS * 1.2, // Forked: brighter branches
            _ => LIGHTNING_BRANCH_BRIGHTNESS,
        };

        for branch in &self.branches {
            for &(col, line) in branch.iter() {
                write_cell(col, line, '╱', branch_brightness, false);
            }
        }

        // ── Render flash (background glow) ──
        let flash_phase = matches!(
            self.phase,
            LightningPhase::Flash | LightningPhase::ReturnStrokeFlash
        );
        if flash_phase {
            for &(col, line, falloff) in &self.flash_cells {
                if skip_msg(col, line) || col >= self.cols || line >= self.lines {
                    continue;
                }
                let flash_intensity = match self.bolt_family {
                    5 => LIGHTNING_FLASH_INTENSITY * 1.4, // Heavy: bigger glow
                    4 => LIGHTNING_FLASH_INTENSITY * 1.2, // Ribbon: wider glow
                    _ => LIGHTNING_FLASH_INTENSITY,
                };
                let glow = intensity * falloff * flash_intensity;
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
        if !matches!(
            self.phase,
            LightningPhase::Strike | LightningPhase::ReturnStrokeFlash
        ) {
            return 0.0;
        }
        let strike_ms = (LIGHTNING_ACTIVE_MS as f32) * LIGHTNING_STRIKE_FRACTION;
        let elapsed = now.saturating_duration_since(self.phase_start).as_millis() as f32;
        let progress = (elapsed / strike_ms.max(1.0)).min(1.0);
        // Per-family pulse strength
        let family_boost = match self.bolt_family {
            5 => 1.3,  // Heavy: strongest pulse
            4 => 1.15, // Ribbon: strong pulse
            _ => 1.0,
        };
        family_boost * (-progress * 4.0).exp()
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

        // Per-family phosphor energy
        let bolt_energy = match self.bolt_family {
            5 => seed_energy,                    // Heavy: full afterglow
            4 => seed_energy.saturating_sub(20), // Ribbon: slightly less
            3 => seed_energy.saturating_sub(40), // Broken: shorter afterglow
            _ => seed_energy,
        };

        // Seed phosphor for main bolt cells
        for &(col, line) in &self.main_bolt {
            let pidx = col as usize * lines as usize + line as usize;
            if pidx < total {
                phosphor[pidx] = phosphor[pidx].max(bolt_energy);
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
                phosphor[pidx] = bolt_energy / 3;
                if phosphor_base_fg[pidx].is_none() {
                    phosphor_base_fg[pidx] = self.last_palette_color;
                }
            }
        }
    }
}
