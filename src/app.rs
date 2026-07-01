// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Application configuration: CloudConfig struct and density calculation helpers.

use crate::atmosphere_apply::{AtmosphereApplicationMode, AtmosphereRuntimeModulation};
use crate::cloud::Cloud;
use crate::constants::*;
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, MonolithSize, ShadingMode};

// --- CloudConfig struct for deduplicating cloud initialization ---

/// Aggregated configuration for creating and running a `Cloud` instance.
/// Collected from CLI args and config file, then passed to the interactive
/// loop or benchmark runner.
pub struct CloudConfig {
    pub color_mode: ColorMode,
    pub fullwidth: bool,
    pub shading_mode: ShadingMode,
    pub bold_mode: BoldMode,
    pub async_mode: bool,
    pub default_bg: bool,
    pub color_scheme: ColorScheme,
    pub rain_style: RainStyle,
    pub noglitch: bool,
    pub glitch_pct: f32,
    pub glitch_low: u16,
    pub glitch_high: u16,
    pub linger_low: u16,
    pub linger_high: u16,
    pub short_pct: f32,
    pub die_early_pct: f32,
    pub max_dpc: u8,
    pub density: f32,
    pub speed: f32,
    pub monolith_size: MonolithSize,
    pub chars: Vec<char>,
    pub message: Option<String>,
    pub message_no_border: bool,
    pub target_fps: f64,
    pub duration: Option<f64>,
    pub duration_s: Option<f64>,
    pub bench_frames: Option<u64>,
    pub benchmark: bool,
    pub density_auto: bool,
    pub base_density: f32,
    pub perf_stats: bool,
    pub screensaver: bool,
    pub mouse: bool,
    pub charset_preset: String,
    pub user_ranges: Vec<(char, char)>,
    pub def_ascii: bool,
    pub auto_color_drift: bool,
    /// Atmosphere modulation for the runtime seam. Default is identity (Disabled).
    /// Wired through derive_effective_runtime but identity by default.
    pub(crate) atmosphere_modulation: AtmosphereRuntimeModulation,
    /// Atmosphere application mode. Default is Disabled (identity).
    /// Reserved for future phases where non-identity modulation is gated.
    #[allow(dead_code)]
    pub(crate) atmosphere_mode: AtmosphereApplicationMode,
}

impl CloudConfig {
    pub fn create_cloud(&self, density: f32) -> Cloud {
        let mut cloud = Cloud::new(
            self.color_mode,
            self.fullwidth,
            self.shading_mode,
            self.bold_mode,
            self.async_mode,
            self.default_bg,
            self.color_scheme,
            self.rain_style,
        );

        cloud.glitchy = !self.noglitch;
        cloud.set_glitch_pct(self.glitch_pct / 100.0);
        cloud.set_glitch_times(self.glitch_low, self.glitch_high);
        cloud.set_linger_times(self.linger_low, self.linger_high);
        cloud.short_pct = self.short_pct / 100.0;
        cloud.die_early_pct = self.die_early_pct / 100.0;
        cloud.set_max_droplets_per_column(self.max_dpc);

        // Phase 5: Compute effective runtime values from base + atmosphere modulation.
        // Default modulation is identity, so effective values equal base values.
        let eff = crate::atmosphere_apply::derive_effective_runtime(
            self.speed,
            density,
            &self.atmosphere_modulation,
        );
        cloud.set_droplet_density(eff.density);
        cloud.set_chars_per_sec(eff.speed);
        cloud.set_monolith_size(self.monolith_size);

        cloud.init_chars(self.chars.clone());
        cloud.reset(DENSITY_AUTO_DEFAULT_COLS, DENSITY_AUTO_DEFAULT_LINES);

        // Mouse interaction is opt-in (--mouse flag). Default: disabled for
        // terminal safety (avoids mouse escape sequence leaks on crash).
        cloud.mouse_enabled = self.mouse;

        // Color drift: disabled by default. When off, autonomous palette drift
        // from ColorEcosystem is suppressed so that explicit CLI/config/profile
        // color remains sticky across the entire session.
        cloud.auto_color_drift = self.auto_color_drift;

        if let Some(msg) = &self.message {
            cloud.set_message_border(!self.message_no_border);
            cloud.set_message(msg);
        }

        cloud
    }
}

// --- Density calculation helpers ---

#[must_use]
pub fn auto_density_factor(cols: u16, lines: u16, fullwidth: bool) -> f32 {
    let eff_cols = if fullwidth {
        (cols / 2).max(1)
    } else {
        cols.max(1)
    } as f32;
    let eff_lines = lines.max(1) as f32;

    let area = eff_cols * eff_lines;
    let base = DENSITY_BASE_COLS * DENSITY_BASE_LINES;
    let factor = (area / base).sqrt();
    factor.clamp(DENSITY_AUTO_MIN, DENSITY_AUTO_MAX)
}

#[must_use]
pub fn effective_density(base: f32, cols: u16, lines: u16, fullwidth: bool, auto: bool) -> f32 {
    let base = base.clamp(DENSITY_CLAMP_MIN, DENSITY_CLAMP_MAX);
    if !auto {
        return base;
    }
    (base * auto_density_factor(cols, lines, fullwidth)).clamp(DENSITY_CLAMP_MIN, DENSITY_CLAMP_MAX)
}
