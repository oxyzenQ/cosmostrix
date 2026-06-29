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
//! - 0: Long — vertical bolt, slight jitter, 1-2 small branches, length 0.6-1.0
//! - 1: Short — vertical bolt stopped at 15-35% height, 0-1 branches
//! - 2: Diagonal — angled 15-45°, NOT vertical, tan(angle) drift every row
//! - 3: Fork — main stops at 30-50%, splits into 2-4 daughter branches (Y-shape)
//! - 4: Massive — wide zigzag, 5-10 branches, length 0.8-1.0, brightness ≥1.2
//! - 5: Sheet — 3-6 parallel vertical channels spread 8-25 cols
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
            0 => 0.85, // Long: normal
            1 => 0.75, // Short: dimmer (stopped mid-screen)
            2 => 0.95, // Diagonal: slightly bright
            3 => 1.0,  // Fork: normal, branches carry the drama
            4 => 1.25, // Massive: bright (min 1.2 per spec)
            _ => 0.7,  // Sheet: dimmer individual channels (the quantity creates impact)
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
    /// Each bolt family uses fundamentally different generation logic.
    fn generate_paths(&mut self, rng: &mut StdRng) {
        if self.cols < 4 || self.lines < 4 {
            return;
        }

        let chance = Uniform::new(0.0f32, 1.0f32).expect("chance [0,1) always valid");

        let center = self.cols / 2;
        let max_wander = ((self.cols as f32) * LIGHTNING_WANDER_FRACTION) as u16;
        let start_col = if max_wander > 0 {
            let offset = ((chance.sample(rng) * max_wander as f32 * 2.0) as i32)
                .saturating_sub(max_wander as i32);
            (center as i32 + offset).clamp(0, (self.cols - 1) as i32) as u16
        } else {
            center
        };

        match self.bolt_family {
            // ── Family 0: Long — vertical bolt, slight jitter ──
            0 => self.gen_long(rng, &chance, start_col),
            // ── Family 1: Short — forced 15-35% length ──
            1 => {
                self.length_pct = 0.15 + chance.sample(rng) * 0.20;
                self.gen_long(rng, &chance, start_col);
            }
            // ── Family 2: Diagonal — angled 15-45°, NOT vertical ──
            2 => self.gen_diagonal(rng, &chance, start_col),
            // ── Family 3: Fork — main stops mid-screen, branches split ──
            3 => self.gen_fork(rng, &chance, start_col),
            // ── Family 4: Massive — wide zigzag, many branches ──
            4 => self.gen_massive(rng, &chance, start_col),
            // ── Family 5: Sheet — parallel vertical channels ──
            _ => self.gen_sheet(rng, &chance),
        }

        // Flash cells computed after path generation
        self.compute_flash_cells(rng);
    }

    // ── Family 0/1: Long/Short — vertical bolt with slight jitter ──

    fn gen_long(&mut self, rng: &mut StdRng, chance: &Uniform<f32>, start_col: u16) {
        let target_line = ((self.lines as f32) * self.length_pct) as u16;
        let target_line = target_line.min(self.lines);

        let mut col = start_col;
        let mut prev_col = col;
        let mut line: u16 = 0;
        let zigzag_avg = 6u16 + (chance.sample(rng) * 4.0) as u16;

        while line < target_line && self.main_bolt.len() < LIGHTNING_PATH_CAPACITY {
            self.main_bolt.push((col, line));

            if (line % zigzag_avg) == 0 || col == prev_col {
                let dir: i16 = if chance.sample(rng) < 0.5 { -1 } else { 1 };
                let step = 1i16 + (chance.sample(rng) * 1.5).ceil() as i16; // hstep 1-2
                let new_col = (col as i16 + dir * step).clamp(0, (self.cols - 1) as i16) as u16;
                prev_col = col;
                col = new_col;
            }

            let dcol = col as i16 - prev_col as i16;
            self.bolt_chars
                .push(super::helpers::bolt_char_for_step(dcol, 1, rng));

            let vstep = 2.0 + chance.sample(rng) * 3.0;
            line = line.saturating_add(vstep.ceil() as u16);
        }

        // 1-2 small branches for Long family
        let branch_count = if self.bolt_family == 0 && chance.sample(rng) < 0.35 {
            1 + (chance.sample(rng) * 2.0) as usize
        } else {
            0
        };
        self.gen_branches(rng, chance, 0.15, 0.35, branch_count);
    }

    // ── Family 2: Diagonal — angled bolt, NOT vertical ──

    fn gen_diagonal(&mut self, rng: &mut StdRng, _chance: &Uniform<f32>, start_col: u16) {
        let target_line = ((self.lines as f32) * self.length_pct) as u16;
        let target_line = target_line.min(self.lines);

        // Angle 15-45 degrees
        let angle_deg = 15.0
            + rand::distr::Uniform::new(0.0f32, 30.0f32)
                .expect("[0,30) valid")
                .sample(rng);
        let angle_rad = angle_deg * std::f32::consts::PI / 180.0;
        let tan_angle = angle_rad.tan();

        // Direction: left or right
        let direction: f32 = if rand::distr::Uniform::new(0.0f32, 1.0f32)
            .expect("[0,1) valid")
            .sample(rng)
            < 0.5
        {
            -1.0
        } else {
            1.0
        };

        let mut col_f: f32 = start_col as f32;
        let mut line: u16 = 0;

        while line < target_line && self.main_bolt.len() < LIGHTNING_PATH_CAPACITY {
            let col = (col_f.round() as u16).clamp(0, self.cols.saturating_sub(1));
            self.main_bolt.push((col, line));

            // Diagonal character
            // Diagonal character
            let ch = if direction < 0.0 { '╲' } else { '╱' };
            self.bolt_chars.push(ch);

            // Move horizontally EVERY row by tan(angle) * vstep
            let vstep = 2.0
                + rand::distr::Uniform::new(0.0f32, 2.0f32)
                    .expect("[0,2) valid")
                    .sample(rng);
            col_f += direction * tan_angle * vstep;
            line = line.saturating_add(vstep.ceil() as u16);
        }

        // Diagonal: 0-1 small branches
        let branch_count = if rand::distr::Uniform::new(0.0f32, 1.0f32)
            .expect("[0,1) valid")
            .sample(rng)
            < 0.25
        {
            1
        } else {
            0
        };
        self.gen_branches(
            rng,
            &Uniform::new(0.0, 1.0).expect("valid"),
            0.15,
            0.30,
            branch_count,
        );
    }

    // ── Family 3: Fork — main stops at 30-50%, branches split (Y-shape) ──

    fn gen_fork(&mut self, rng: &mut StdRng, chance: &Uniform<f32>, start_col: u16) {
        let target_line = ((self.lines as f32) * self.length_pct) as u16;
        let target_line = target_line.min(self.lines);

        // Fork point: 30-50% of target
        let fork_frac = 0.30 + chance.sample(rng) * 0.20;
        let fork_line = (target_line as f32 * fork_frac) as u16;

        let mut col = start_col;
        let mut prev_col = col;
        let mut line: u16 = 0;
        let zigzag_avg = 4u16;

        // Draw main bolt up to fork point
        while line < fork_line && self.main_bolt.len() < LIGHTNING_PATH_CAPACITY {
            self.main_bolt.push((col, line));

            if (line % zigzag_avg) == 0 || col == prev_col {
                let dir: i16 = if chance.sample(rng) < 0.5 { -1 } else { 1 };
                let step = 1i16 + (chance.sample(rng) * 2.0).ceil() as i16;
                let new_col = (col as i16 + dir * step).clamp(0, (self.cols - 1) as i16) as u16;
                prev_col = col;
                col = new_col;
            }

            let dcol = col as i16 - prev_col as i16;
            self.bolt_chars
                .push(super::helpers::bolt_char_for_step(dcol, 1, rng));

            let vstep = 2.0 + chance.sample(rng) * 3.0;
            line = line.saturating_add(vstep.ceil() as u16);
        }

        // Now generate fork branches from fork_line downward
        let branch_count = 2 + (chance.sample(rng) * 3.0) as usize; // 2-4
        let remaining = target_line.saturating_sub(fork_line);

        for i in 0..branch_count {
            let angle_offset = -0.6 + (i as f32 / (branch_count - 1).max(1) as f32) * 1.2; // spread angles
            let branch_angle = std::f32::consts::FRAC_PI_4 * angle_offset; // ±45° spread
            let tan_ba = branch_angle.tan();

            let mut branch: Vec<(u16, u16)> = Vec::new();
            let mut bcol_f = col as f32;
            let mut bline = fork_line;

            while bline < target_line && branch.len() < LIGHTNING_BRANCH_CAPACITY {
                let bc = (bcol_f.round() as u16).clamp(0, self.cols.saturating_sub(1));
                branch.push((bc, bline));

                let vstep = 2.0 + chance.sample(rng) * 3.0;
                bcol_f += tan_ba * vstep;
                bline = bline.saturating_add(vstep.ceil() as u16);
            }

            if !branch.is_empty() {
                // Mark the fork point with a bright character
                if let Some(last_seg) = self.bolt_chars.last_mut() {
                    *last_seg = '┣'; // fork indicator
                }
                self.branches.push(branch);
            }
            let _ = remaining; // used for future tuning
        }
    }

    // ── Family 4: Massive — wide zigzag, many branches ──

    fn gen_massive(&mut self, rng: &mut StdRng, chance: &Uniform<f32>, start_col: u16) {
        self.length_pct = self.length_pct.max(0.8); // Force at least 80%
        let target_line = ((self.lines as f32) * self.length_pct) as u16;
        let target_line = target_line.min(self.lines);

        let mut col = start_col;
        let mut prev_col = col;
        let mut line: u16 = 0;
        let zigzag_avg = 3u16;

        while line < target_line && self.main_bolt.len() < LIGHTNING_PATH_CAPACITY {
            self.main_bolt.push((col, line));

            if (line % zigzag_avg) == 0 || col == prev_col {
                let dir: i16 = if chance.sample(rng) < 0.5 { -1 } else { 1 };
                let step = (chance.sample(rng) * 7.0).ceil() as i16 + 1; // hstep 1-8
                let new_col = (col as i16 + dir * step).clamp(0, (self.cols - 1) as i16) as u16;
                prev_col = col;
                col = new_col;
            }

            let dcol = col as i16 - prev_col as i16;
            self.bolt_chars
                .push(super::helpers::bolt_char_for_step(dcol, 1, rng));

            let vstep = 1.0 + chance.sample(rng) * 2.0;
            line = line.saturating_add(vstep.ceil() as u16);
        }

        // Many branches: 5-10
        let branch_count = 5 + (chance.sample(rng) * 6.0) as usize;
        self.gen_branches(rng, chance, 0.15, 0.50, branch_count);
    }

    // ── Family 5: Sheet — parallel vertical channels ──

    fn gen_sheet(&mut self, rng: &mut StdRng, chance: &Uniform<f32>) {
        self.length_pct = self.length_pct.max(0.6);
        let target_line = ((self.lines as f32) * self.length_pct) as u16;
        let target_line = target_line.min(self.lines);

        // Main bolt: center channel (light)
        let center_col = self.cols / 2;
        self.main_bolt.push((center_col, 0));
        self.bolt_chars.push('│');
        let mut line: u16 = 2;
        while line < target_line && self.main_bolt.len() < LIGHTNING_PATH_CAPACITY {
            self.main_bolt.push((center_col, line));
            self.bolt_chars.push('│');
            line = line.saturating_add(2);
        }

        // Parallel channels: 3-6
        let num_channels = 3 + (chance.sample(rng) * 4.0) as usize;
        let spread = 8u16 + (chance.sample(rng) * 17.0) as u16; // 8-25 cols

        for c in 0..num_channels {
            // Alternate sides from center
            let side_offset = if c % 2 == 0 {
                (c as u16 / 2 + 1) as i32
            } else {
                -((c as u16).div_ceil(2) as i32)
            };
            let channel_col = (center_col as i32
                + side_offset * spread as i32 / num_channels as i32)
                .clamp(0, (self.cols - 1) as i32) as u16;

            let mut channel: Vec<(u16, u16)> = Vec::new();
            let mut cline: u16 = (chance.sample(rng) * 6.0) as u16; // staggered start
            let ccol = channel_col;

            while cline < target_line && channel.len() < LIGHTNING_BRANCH_CAPACITY {
                // Minimal jitter
                let jitter = if chance.sample(rng) < 0.15 {
                    (chance.sample(rng) * 2.0).round() as i16 - 1
                } else {
                    0
                };
                let jc = (ccol as i16 + jitter).clamp(0, (self.cols - 1) as i16) as u16;
                channel.push((jc, cline));
                let vstep = 2.0 + chance.sample(rng) * 3.0;
                cline = cline.saturating_add(vstep.ceil() as u16);
            }

            if !channel.is_empty() {
                self.branches.push(channel);
            }
        }
    }

    // ── Shared: branch generation ──

    fn gen_branches(
        &mut self,
        rng: &mut StdRng,
        chance: &Uniform<f32>,
        len_min_frac: f32,
        len_max_frac: f32,
        count: usize,
    ) {
        if self.main_bolt.len() < 3 || count == 0 {
            return;
        }

        for _ in 0..count {
            let root_idx = (chance.sample(rng) * (self.main_bolt.len() as f32 * 0.6)) as usize;
            if let Some(&(root_col, root_line)) = self.main_bolt.get(root_idx) {
                let max_len = self.main_bolt.len() - root_idx;
                let branch_len = ((len_min_frac
                    + chance.sample(rng) * (len_max_frac - len_min_frac))
                    * max_len as f32) as usize;
                let branch_len = branch_len.clamp(2, LIGHTNING_BRANCH_CAPACITY);

                let mut branch: Vec<(u16, u16)> = Vec::new();
                let mut bcol = root_col;
                let mut bline = root_line;
                let branch_dir: i16 = if chance.sample(rng) < 0.5 { -1 } else { 1 };

                for _ in 0..branch_len {
                    let step = (chance.sample(rng) * 3.0).ceil() as i16;
                    bcol =
                        (bcol as i16 + branch_dir * step).clamp(0, (self.cols - 1) as i16) as u16;
                    bline = bline
                        .saturating_add((1.0 + chance.sample(rng) * 2.0).ceil() as u16)
                        .min(self.lines.saturating_sub(1));
                    branch.push((bcol, bline));
                }
                self.branches.push(branch);
            }
        }
    }

    // ── Shared: flash cell computation ──

    fn compute_flash_cells(&mut self, rng: &mut StdRng) {
        let _ = rng; // kept for future parameterization
        let flash_radius = match self.bolt_family {
            4 => LIGHTNING_FLASH_RADIUS + 6, // Massive: wider flash
            _ => LIGHTNING_FLASH_RADIUS,
        };

        if flash_radius == 0 || self.main_bolt.is_empty() {
            return;
        }

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
                    return;
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
