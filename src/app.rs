// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Application configuration: CloudConfig struct, config file defaults
//! integration, and density calculation helpers.

use crate::cloud::Cloud;
use crate::config::Args;
use crate::constants::*;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};
use crate::validation::{validate_f32_range, validate_f64_range, validate_u8_range};

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
        );

        cloud.glitchy = !self.noglitch;
        cloud.set_glitch_pct(self.glitch_pct / 100.0);
        cloud.set_glitch_times(self.glitch_low, self.glitch_high);
        cloud.set_linger_times(self.linger_low, self.linger_high);
        cloud.short_pct = self.short_pct / 100.0;
        cloud.die_early_pct = self.die_early_pct / 100.0;
        cloud.set_max_droplets_per_column(self.max_dpc);
        cloud.set_droplet_density(density);
        cloud.set_chars_per_sec(self.speed);

        cloud.init_chars(self.chars.clone());
        cloud.reset(DENSITY_AUTO_DEFAULT_COLS, DENSITY_AUTO_DEFAULT_LINES);

        // Mouse interaction is opt-in (--mouse flag). Default: disabled for
        // terminal safety (avoids mouse escape sequence leaks on crash).
        cloud.mouse_enabled = self.mouse;

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

// --- Config file defaults integration ---

/// Apply config file defaults to CLI args that were not explicitly provided.
///
/// All numeric values are validated against the same ranges enforced for CLI
/// arguments, so a malformed config file cannot cause panics (e.g. fps=0
/// leading to division-by-zero) or out-of-range behaviour.
pub(super) fn apply_config_defaults(matches: &clap::ArgMatches, args: &mut Args) {
    use clap::parser::ValueSource;

    let cfg = crate::configfile::load_config_file();
    if cfg.is_empty() {
        return;
    }

    // Only override args that were not explicitly provided (still at default)
    let apply = |key: &str, matches: &clap::ArgMatches| -> Option<String> {
        if matches.value_source(key) == Some(ValueSource::DefaultValue) {
            cfg.get(key).cloned()
        } else {
            None
        }
    };

    if let Some(v) = apply("color", matches) {
        args.color = v;
    }
    if let Some(v) = apply("charset", matches) {
        args.charset = v;
    }
    if let Some(v) = apply("fps", matches) {
        match v.parse::<f64>() {
            Ok(n) if n.is_finite() => {
                if let Ok(f) = validate_f64_range("config fps", n, 1.0, 240.0) {
                    args.fps = f;
                } else {
                    eprintln!("config: ignoring invalid fps={v} (min 1 max 240)");
                }
            }
            _ => eprintln!("config: ignoring unparseable fps='{v}' (expected a number)"),
        }
    }
    if let Some(v) = apply("speed", matches) {
        match v.parse::<f32>() {
            Ok(n) if n.is_finite() => {
                if let Ok(f) = validate_f32_range("config speed", n, 0.001, 1000.0) {
                    args.speed = f;
                } else {
                    eprintln!("config: ignoring invalid speed={v} (min 0.001 max 1000)");
                }
            }
            _ => eprintln!("config: ignoring unparseable speed='{v}' (expected a number)"),
        }
    }
    if let Some(v) = apply("density", matches) {
        match v.parse::<f32>() {
            Ok(n) if n.is_finite() => {
                if let Ok(f) =
                    validate_f32_range("config density", n, DENSITY_CLAMP_MIN, DENSITY_CLAMP_MAX)
                {
                    args.density = f;
                } else {
                    eprintln!("config: ignoring invalid density={v} (min {DENSITY_CLAMP_MIN} max {DENSITY_CLAMP_MAX})");
                }
            }
            _ => eprintln!("config: ignoring unparseable density='{v}' (expected a number)"),
        }
    }
    if let Some(v) = apply("bold", matches) {
        match v.parse::<u8>() {
            Ok(n) => {
                if let Ok(valid) = validate_u8_range("config bold", n, 0, 2) {
                    args.bold = valid;
                } else {
                    eprintln!("config: ignoring invalid bold={v} (min 0 max 2)");
                }
            }
            _ => eprintln!("config: ignoring unparseable bold='{v}' (expected integer 0-2)"),
        }
    }
    if let Some(v) = apply("shadingmode", matches) {
        match v.parse::<u8>() {
            Ok(n) => {
                if let Ok(valid) = validate_u8_range("config shadingmode", n, 0, 1) {
                    args.shading_mode = valid;
                } else {
                    eprintln!("config: ignoring invalid shadingmode={v} (min 0 max 1)");
                }
            }
            _ => eprintln!("config: ignoring unparseable shadingmode='{v}' (expected integer 0-1)"),
        }
    }
    if let Some(v) = apply("glitchpct", matches) {
        match v.parse::<f32>() {
            Ok(n) if n.is_finite() => {
                if let Ok(f) = validate_f32_range("config glitchpct", n, 0.0, 100.0) {
                    args.glitch_pct = f;
                } else {
                    eprintln!("config: ignoring invalid glitchpct={v} (min 0 max 100)");
                }
            }
            _ => eprintln!("config: ignoring unparseable glitchpct='{v}' (expected a number)"),
        }
    }
    if let Some(v) = apply("shortpct", matches) {
        match v.parse::<f32>() {
            Ok(n) if n.is_finite() => {
                if let Ok(f) = validate_f32_range("config shortpct", n, 0.0, 100.0) {
                    args.shortpct = f;
                } else {
                    eprintln!("config: ignoring invalid shortpct={v} (min 0 max 100)");
                }
            }
            _ => eprintln!("config: ignoring unparseable shortpct='{v}' (expected a number)"),
        }
    }
    if let Some(v) = apply("rippct", matches) {
        match v.parse::<f32>() {
            Ok(n) if n.is_finite() => {
                if let Ok(f) = validate_f32_range("config rippct", n, 0.0, 100.0) {
                    args.rippct = f;
                } else {
                    eprintln!("config: ignoring invalid rippct={v} (min 0 max 100)");
                }
            }
            _ => eprintln!("config: ignoring unparseable rippct='{v}' (expected a number)"),
        }
    }
    if let Some(v) = apply("maxdpc", matches) {
        match v.parse::<u8>() {
            Ok(n) => {
                if let Ok(valid) = validate_u8_range("config maxdpc", n, 1, 3) {
                    args.max_droplets_per_column = valid;
                } else {
                    eprintln!("config: ignoring invalid maxdpc={v} (min 1 max 3)");
                }
            }
            _ => eprintln!("config: ignoring unparseable maxdpc='{v}' (expected integer 1-3)"),
        }
    }
}
