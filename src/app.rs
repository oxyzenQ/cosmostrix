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
    pub message_border: bool,
    pub target_fps: f64,
    pub duration: Option<f64>,
    pub duration_s: Option<f64>,
    pub bench_frames: Option<u64>,
    pub benchmark: bool,
    /// Optional benchmark duration override in seconds.
    /// When None, defaults to BENCHMARK_DURATION_SECS (5s).
    /// Resolved from --bench-duration (bare seconds) OR --duration (compound: 6s/1h30m).
    pub bench_duration: Option<u64>,
    /// Parsed --screen-size WxH value. None means dynamic (use terminal size).
    /// When set, benchmark uses this fixed size; interactive renders to fixed virtual size.
    pub screen_size: Option<(u16, u16)>,
    /// Parsed --color-tune value. None means no tune (identity).
    pub color_tune: crate::color_tune::ColorTune,
    /// Output benchmark report as JSON (--json flag).
    pub json: bool,
    /// --save-baseline PATH: save benchmark JSON to file
    pub save_baseline: Option<String>,
    /// --compare-baseline PATH: compare against saved baseline
    pub compare_baseline: Option<String>,
    /// --bench-io: wet terminal I/O benchmark (write to /dev/null)
    pub bench_io: bool,
    /// --bench-all: run scaling benchmark across multiple sizes
    pub bench_all: bool,
    /// --verbose flag: print diagnostic info to stderr.
    pub verbose: bool,
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
    /// Optional per-column density map for monolith pillar placement.
    /// Parsed from scene-custom.<name>.density-map config field (CSV f64).
    /// None = uniform distribution (legacy behavior).
    pub(crate) monolith_density_map: Option<&'static [f64]>,
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

        // Apply --color-tune (if non-identity) to the palette AFTER Cloud::new
        // builds it. This turns the 43 fixed themes into 43 × ∞ by letting
        // users adjust saturation/brightness at load time without editing
        // source code.
        crate::color_tune::apply_tune_to_palette(
            &mut cloud.palette,
            self.color_mode,
            &self.color_tune,
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

        // v14 Peak Monolith: apply per-column density map if set.
        // This sculpts pillar formation — columns with weight 0.0 never spawn,
        // 1.0 always spawn. Enables artistic compositions (twin towers, clusters).
        if let Some(map) = self.monolith_density_map {
            cloud.set_monolith_density_map(Some(map));
        }

        // Mouse interaction is opt-in (--mouse flag). Default: disabled for
        // terminal safety (avoids mouse escape sequence leaks on crash).
        cloud.mouse_enabled = self.mouse;

        // Color drift: disabled by default. When off, autonomous palette drift
        // from ColorEcosystem is suppressed so that explicit CLI/config/profile
        // color remains sticky across the entire session.
        cloud.auto_color_drift = self.auto_color_drift;

        if let Some(msg) = &self.message {
            cloud.set_message_border(self.message_border);
            cloud.set_message(msg);
        }

        cloud
    }

    /// Clone the config for scaling benchmark (bench-all).
    /// Only copies fields needed for benchmark, not interactive-only fields.
    pub fn clone_config(&self) -> Self {
        Self {
            color_mode: self.color_mode,
            fullwidth: self.fullwidth,
            shading_mode: self.shading_mode,
            bold_mode: self.bold_mode,
            async_mode: self.async_mode,
            default_bg: self.default_bg,
            color_scheme: self.color_scheme,
            rain_style: self.rain_style,
            noglitch: self.noglitch,
            glitch_pct: self.glitch_pct,
            glitch_low: self.glitch_low,
            glitch_high: self.glitch_high,
            linger_low: self.linger_low,
            linger_high: self.linger_high,
            short_pct: self.short_pct,
            die_early_pct: self.die_early_pct,
            max_dpc: self.max_dpc,
            density: self.density,
            speed: self.speed,
            monolith_size: self.monolith_size,
            chars: self.chars.clone(),
            message: self.message.clone(),
            message_border: self.message_border,
            target_fps: self.target_fps,
            duration: self.duration,
            duration_s: self.duration_s,
            bench_frames: self.bench_frames,
            benchmark: self.benchmark,
            bench_duration: self.bench_duration,
            screen_size: self.screen_size,
            color_tune: self.color_tune,
            json: false,
            save_baseline: None,
            compare_baseline: None,
            bench_io: false,
            bench_all: false,
            verbose: false,
            density_auto: self.density_auto,
            base_density: self.base_density,
            perf_stats: false,
            screensaver: false,
            mouse: false,
            charset_preset: self.charset_preset.clone(),
            user_ranges: self.user_ranges.clone(),
            def_ascii: self.def_ascii,
            auto_color_drift: self.auto_color_drift,
            atmosphere_modulation: self.atmosphere_modulation,
            atmosphere_mode: self.atmosphere_mode,
            monolith_density_map: self.monolith_density_map,
        }
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
