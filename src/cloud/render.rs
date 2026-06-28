// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! DrawCtx — read-only drawing context passed to Droplet::draw.

use std::time::Instant;

use bitvec::prelude::BitSlice;
use crossterm::style::Color;

use crate::constants::*;
use crate::runtime::BoldMode;

/// Precomputed exponential decay lookup table for trail brightness.
/// Maps 256 normalized distances → exp(-TRAIL_EXPONENTIAL_K * t).
/// Eliminates ~3,000 exp() calls per frame in shading_distance mode.
static TRAIL_EXP_LUT: std::sync::LazyLock<[f32; 256]> = std::sync::LazyLock::new(|| {
    let mut lut = [0.0f32; 256];
    for (i, entry) in lut.iter_mut().enumerate() {
        let t = i as f32 / 255.0;
        *entry = (-(TRAIL_EXPONENTIAL_K as f32) * t).exp();
    }
    lut
});

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharLoc {
    Middle,
    Tail,
    Head,
}

/// Read-only drawing context passed to `Droplet::draw` to avoid borrowing
/// the entire `Cloud` (which would conflict with the mutable droplet loop).
pub struct DrawCtx<'a> {
    pub lines: u16,
    pub full_width: bool,
    pub shading_distance: bool,
    pub bg: Option<Color>,

    pub color_mode: crate::runtime::ColorMode,
    pub bold_mode: BoldMode,
    pub glitchy: bool,

    pub last_glitch_time: Instant,
    pub next_glitch_time: Instant,
    /// Precomputed 1.0 / glitch duration for multiply (avoids per-cell division).
    pub glitch_inv_between: f64,

    /// Per-slot palette color arrays for generation-based rendering.
    /// Index by droplet's `palette_slot` to resolve its birth palette.
    pub palette_slices: [&'a [Color]; MAX_PALETTE_SLOTS],

    /// Which palette slot is the currently active (latest) one.
    /// Used for transition glow effects on new-generation streams.
    pub active_palette_slot: u8,

    /// Whether a palette transition is currently in progress.
    /// When true, new-generation streams get enhanced visual effects.
    pub transitioning: bool,

    pub color_map: &'a [u8],
    pub glitch_map: &'a BitSlice,
    pub char_pool: &'a [char],
    pub previous_char_pool: &'a [char],
    pub charset_wave_line: Option<f32>,

    /// Color transition wave line: during a palette transition, rows above
    /// this value use the new (active) palette; rows below use their birth
    /// palette. Sweeps from 0 to lines+1 over COLOR_TRANSITION_DURATION_MS,
    /// creating a top-to-bottom wave that matches the charset transition.
    pub color_wave_line: Option<f32>,

    /// Mouse cursor column (u16::MAX if no mouse).
    pub mouse_col: u16,
    /// Mouse cursor line (u16::MAX if no mouse).
    pub mouse_line: u16,
    /// Flash effect click column.
    pub flash_col: u16,
    /// Flash effect click line.
    pub flash_line: u16,
    /// Flash effect start time (None if no active flash).
    pub flash_time: Option<Instant>,
    /// Cached result of pool_is_binary check, computed once per DrawCtx
    /// construction to avoid per-cell iteration of the char pool.
    pub pool_is_binary: bool,
}

impl DrawCtx<'_> {
    #[inline]
    fn is_bright(&self, now: Instant) -> bool {
        if now < self.last_glitch_time || self.glitch_inv_between <= 0.0 {
            return false;
        }
        let since = now
            .saturating_duration_since(self.last_glitch_time)
            .as_nanos() as f64;
        since * self.glitch_inv_between <= GLITCH_BRIGHT_RATIO
    }

    #[inline]
    fn is_dim(&self, now: Instant) -> bool {
        if now > self.next_glitch_time {
            return true;
        }
        if self.glitch_inv_between <= 0.0 {
            return true;
        }
        let since = now
            .saturating_duration_since(self.last_glitch_time)
            .as_nanos() as f64;
        since * self.glitch_inv_between >= GLITCH_DIM_RATIO
    }

    #[inline]
    pub fn is_glitched(&self, line: u16, col: u16) -> bool {
        if !self.glitchy {
            return false;
        }
        let idx = col as usize * self.lines as usize + line as usize;
        self.glitch_map.get(idx).is_some_and(|b| *b)
    }

    #[inline]
    pub fn get_char(&self, line: u16, col: u16, char_pool_idx: u16) -> char {
        let pool = if self.charset_uses_previous_pool(line, col) {
            self.previous_char_pool
        } else {
            self.char_pool
        };
        let _len = pool.len().max(1);
        // OPTIMIZED: use bitmask instead of modulo (CHAR_POOL_SIZE is power of 2)
        let idx = ((char_pool_idx as usize) + (line as usize)) & (CHAR_POOL_SIZE - 1);
        pool.get(idx).copied().unwrap_or('0')
    }

    #[inline]
    pub fn charset_transitioning(&self) -> bool {
        self.charset_wave_line.is_some()
    }

    #[inline]
    fn charset_uses_previous_pool(&self, line: u16, col: u16) -> bool {
        let Some(wave_line) = self.charset_wave_line else {
            return false;
        };
        if self.previous_char_pool.is_empty() {
            return false;
        }

        let jitter =
            (((line as u32).wrapping_mul(17) ^ (col as u32).wrapping_mul(31)) % 3) as f32 * 0.18;
        (line as f32) > wave_line + jitter
    }

    /// During a color transition, returns whether a cell at (line, col) should
    /// use its birth (previous) palette rather than the new (active) palette.
    /// Rows below the wave line use the old palette; rows above use the new.
    /// This creates a top-to-bottom cascade matching the charset transition.
    #[inline]
    pub fn color_uses_previous_palette(&self, palette_slot: u8, line: u16, col: u16) -> bool {
        let Some(wave_line) = self.color_wave_line else {
            return false;
        };
        // Only applies to droplets that still carry the old palette slot
        if palette_slot == self.active_palette_slot {
            return false;
        }
        // Jitter for organic edge (same pattern as charset wave)
        let jitter =
            (((line as u32).wrapping_mul(13) ^ (col as u32).wrapping_mul(29)) % 3) as f32 * 0.15;
        (line as f32) > wave_line + jitter
    }

    // Attribute calculation is the renderer's convergence point for palette,
    // position, glyph, transition, and head-state signals.
    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub fn get_attr(
        &self,
        palette_slot: u8,
        line: u16,
        col: u16,
        val: char,
        loc: CharLoc,
        now: Instant,
        head_put_line: u16,
        length: u16,
    ) -> (Option<Color>, bool) {
        // Resolve this stream's palette from the generation table.
        // During a color transition, cells above the wave line adopt the new
        // (active) palette even if the droplet was born with the old one,
        // creating a visible top-to-bottom cascade.
        let effective_slot = if self.color_uses_previous_palette(palette_slot, line, col) {
            palette_slot // Below wave: keep birth palette
        } else {
            self.active_palette_slot // Above wave or no transition: use new palette
        };
        let palette_colors = if (effective_slot as usize) < MAX_PALETTE_SLOTS {
            self.palette_slices[effective_slot as usize]
        } else {
            // Fallback: use active palette for invalid slots
            self.palette_slices[self.active_palette_slot as usize]
        };

        let mut bold = false;
        if self.bold_mode == BoldMode::Random {
            bold = (((line as u32) ^ (val as u32)) % 2) == 1;
        }

        let idx = col as usize * self.lines as usize + line as usize;
        let mut color_idx = self.color_map.get(idx).copied().unwrap_or(0) as i32;

        if self.shading_distance {
            let last = palette_colors.len().saturating_sub(1) as u64;
            let dist = head_put_line.saturating_sub(line) as f64;
            let len = length.max(1) as f64;

            // Exponential decay: brightness = exp(-k * distance/length)
            let normalized_dist = (dist / len).clamp(0.0, 1.0);
            // OPTIMIZED: use precomputed LUT instead of exp() per cell
            let lut_idx = (normalized_dist * 255.0) as usize;
            let brightness = TRAIL_EXP_LUT[lut_idx.min(255)];
            let mut v = ((brightness * last as f32).round() as u64).min(last);

            // Bloom: cells right behind head get extra brightness
            if dist < HEAD_BLOOM_CELLS as f64 {
                v = (v + 1).min(last);
            }

            color_idx = v as i32;
        }

        if self.glitchy && self.glitch_map.get(idx).is_some_and(|b| *b) {
            if self.is_bright(now) {
                color_idx += 1;
                bold = true;
            } else if self.is_dim(now) {
                color_idx -= 1;
                bold = false;
            }
        }

        let last = palette_colors.len().saturating_sub(1) as i32;
        match loc {
            CharLoc::Tail => {
                color_idx = 0;
                bold = false;
            }
            CharLoc::Head => {
                color_idx = last;
                bold = true;
            }
            CharLoc::Middle => {
                color_idx = color_idx.clamp(0, last.max(0));
            }
        }

        match self.bold_mode {
            BoldMode::Off => bold = false,
            BoldMode::All => bold = true,
            BoldMode::Random => {}
        }

        let fg = if self.color_mode == crate::runtime::ColorMode::Mono {
            None
        } else {
            palette_colors.get(color_idx as usize).copied()
        };

        (fg, bold)
    }
}
