// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Runtime property setters and semantic invalidation.
//!
//! Provides the setter methods that modify Cloud state at runtime:
//! color scheme, speed, density, shading, glitch, pause, and performance
//! tuning. Also contains speed sanitization helpers.

use std::time::Duration;

use rand::distr::{Distribution, Uniform};

use crate::constants::*;
use crate::rain_style::RainStyle;
use crate::runtime::{ColorScheme, MonolithSize, ShadingMode};

use super::Cloud;

/// Clamp a chars-per-sec value to style-specific bounds.
/// Monolith style has a lower maximum speed than glyph styles.
pub(crate) fn sanitize_speed_for_style(cps: f32, rain_style: RainStyle) -> f32 {
    let cps = if cps.is_finite() {
        cps.max(RUNTIME_SPEED_MIN)
    } else {
        RUNTIME_SPEED_MIN
    };
    let max = if matches!(rain_style, RainStyle::Monolith) {
        MONOLITH_EFFECTIVE_SPEED_MAX
    } else {
        RUNTIME_SPEED_MAX
    };
    cps.min(max)
}

impl Cloud {
    pub fn set_color_scheme(&mut self, scheme: ColorScheme) {
        self.color_scheme = scheme;
        use crate::palette::build_palette;
        let new_palette = build_palette(scheme, self.color_mode, self.default_background);
        self.apply_new_palette(new_palette);
    }

    /// Set a custom palette directly (v16 --colors-custom path).
    ///
    /// This bypasses the `ColorScheme` enum entirely — the palette is
    /// injected directly from user config. The `color_scheme` field stays
    /// unchanged (for verbose display / cycling), but the actual colors
    /// come from the provided palette.
    ///
    /// The palette transition wave (cinematic top-to-bottom cascade) works
    /// identically to `set_color_scheme` — the old streams keep their birth
    /// palette below the wave line, and the new palette propagates visually.
    pub fn set_palette(&mut self, palette: crate::palette::Palette) {
        self.apply_new_palette(palette);
    }

    /// Internal: apply a new palette with the transition wave effect.
    ///
    /// Shared between `set_color_scheme` (built-in themes) and `set_palette`
    /// (custom themes). Handles:
    /// - Palette slot rotation (circular buffer for cross-fade)
    /// - Color map regeneration (per-cell random index into new palette)
    /// - Column slot reset (all columns adopt new palette for spawning)
    /// - Transition start time (for wave animation)
    /// - Monolith draw history + phosphor reset
    fn apply_new_palette(&mut self, new_palette: crate::palette::Palette) {
        // Advance to next palette slot (circular buffer)
        let next_slot = ((self.active_palette_slot as usize + 1) % MAX_PALETTE_SLOTS) as u8;
        self.palette_table[next_slot as usize] = Some(new_palette.clone());
        self.active_palette_slot = next_slot;

        // Update the convenience palette reference
        self.palette = new_palette;

        // Regenerate color map for the new palette size
        self.fill_color_map();

        // Start transition: all columns adopt the new palette immediately
        // for spawn purposes. The visual wave is row-based (top-to-bottom)
        // driven by color_wave_line_at(), not column-based delays.
        for slot in self.column_palette_slot.iter_mut() {
            *slot = self.active_palette_slot;
        }
        self.transition_start = Some(std::time::Instant::now());

        // v16: Force full redraw when palette changes so the background
        // fills the entire screen (including borders). Without this, cells
        // that were never written to (edges, bottom rows) keep their old
        // background, causing visible "gap" lines around the rain area.
        self.force_draw_everything = true;
        self.semantic_invalidate = true;

        if matches!(self.rain_style, RainStyle::Monolith) {
            self.monolith_rain.clear_draw_history();
            self.reset_phosphor_state();
        }

        // v16: force_draw_everything is set above so the background
        // fills the whole screen on palette change. The transition wave
        // still works because force_draw clears to the new bg first,
        // then rain cells are written on top.
    }

    pub fn set_async(&mut self, on: bool) {
        self.async_mode = on;
        self.set_column_speeds();
        self.update_droplet_speeds();
    }

    pub fn set_chars_per_sec(&mut self, cps: f32) {
        self.chars_per_sec = sanitize_speed_for_style(cps, self.rain_style);
        self.recalc_droplets_per_sec();
        self.set_column_speeds();
        self.update_droplet_speeds();
    }

    pub fn set_monolith_size(&mut self, size: MonolithSize) {
        self.monolith_size = size;
        if matches!(self.rain_style, RainStyle::Monolith) {
            self.monolith_rain.clear_draw_history();
            self.reset_phosphor_state();
            self.semantic_invalidate = true;
        }
    }

    pub fn set_droplet_density(&mut self, density: f32) {
        self.droplet_density = density;
        self.recalc_droplets_per_sec();
    }

    pub fn set_glitchy(&mut self, on: bool) {
        self.glitchy = on;
        self.fill_glitch_map();
        if on {
            let now = std::time::Instant::now();
            self.last_glitch_time = now;
            let ms = self.rand_glitch_ms.sample(&mut self.mt) as u64;
            self.next_glitch_time = now + Duration::from_millis(ms);
        }
        self.semantic_invalidate = true;
    }

    /// Current glitch intensity (0.0 = off, 0.25 = intense).
    #[must_use]
    pub fn glitch_pct(&self) -> f32 {
        self.glitch_pct
    }

    pub fn set_glitch_pct(&mut self, pct: f32) {
        self.glitch_pct = pct;
        self.fill_glitch_map();
    }

    pub fn set_glitch_times(&mut self, low_ms: u16, high_ms: u16) {
        self.glitch_low_ms = low_ms;
        self.glitch_high_ms = high_ms;
        let (lo, hi) = if low_ms <= high_ms {
            (low_ms, high_ms)
        } else {
            (high_ms, low_ms)
        };
        self.rand_glitch_ms =
            Uniform::new_inclusive(lo, hi).expect("rand_glitch_ms: lo <= hi after swap");
    }

    pub fn set_linger_times(&mut self, low_ms: u16, high_ms: u16) {
        self.linger_low_ms = low_ms;
        self.linger_high_ms = high_ms;
        let (lo, hi) = if low_ms <= high_ms {
            (low_ms, high_ms)
        } else {
            (high_ms, low_ms)
        };
        self.rand_linger_ms =
            Uniform::new_inclusive(lo, hi).expect("rand_linger_ms: lo <= hi after swap");
    }

    pub fn set_max_droplets_per_column(&mut self, v: u8) {
        self.max_droplets_per_column = v;
    }

    pub fn set_perf_pressure(&mut self, p: f32) {
        self.perf_pressure = p.clamp(0.0, 1.0);
    }

    pub fn set_max_sim_delta(&mut self, d: Duration) {
        self.max_sim_delta = d;
    }

    pub fn set_shading_mode(&mut self, sm: ShadingMode) {
        self.shading_mode = sm;
        self.shading_distance = matches!(sm, ShadingMode::DistanceFromHead);
        if matches!(self.rain_style, RainStyle::Monolith) {
            self.monolith_rain.clear_draw_history();
            self.reset_phosphor_state();
        }
        // Shading mode is a renderer semantic mutation — invalidate the
        // Terminal's LastFrame cache to prevent stale shading artifacts.
        self.semantic_invalidate = true;
    }

    pub fn force_draw_everything(&mut self) {
        self.force_draw_everything = true;
    }
}
