// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Application configuration: CloudConfig struct and density calculation helpers.

use crate::cloud::Cloud;
use crate::config::IntroType;
use crate::constants::*;
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, MonolithSize, ShadingMode};

// --- CloudConfig struct for deduplicating cloud initialization ---

/// Aggregated configuration for creating and running a `Cloud` instance.
/// Collected from CLI args and config file, then passed to the interactive
/// loop or benchmark runner.
#[derive(Clone)]
pub struct CloudConfig {
    pub color_mode: ColorMode,
    pub shading_mode: ShadingMode,
    pub bold_mode: BoldMode,
    pub async_mode: bool,
    pub default_bg: bool,
    pub color_scheme: ColorScheme,
    /// Custom palette override (v16). When Some, the cloud uses this palette
    /// instead of the built-in palette from color_scheme. The color_scheme
    /// enum is still tracked for verbose output + cycling, but the actual
    /// colors come from this palette.
    pub custom_palette: Option<crate::palette::Palette>,
    /// Name of the active custom palette (for live reload). When Some,
    /// rebuild_cloud_config reloads the palette definition from config
    /// so editing colors-custom entries takes effect immediately.
    pub custom_palette_name: Option<String>,
    pub rain_style: RainStyle,
    /// Glitch enable flag, derived from `glitch_level != GlitchLevel::None`.
    /// Replaces the old `noglitch: bool` field (v30 simplify: --noglitch CLI
    /// flag removed; positive polarity is clearer and matches `cloud.glitchy`).
    pub glitch_enabled: bool,
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
    /// (FPS-F1): xterm.js host + 30 FPS cap, copied from `TerminalCaps`
    /// at startup so the event loop's live-reload path can re-apply the cap
    /// when the user edits `fps =` in config.toml. See `resolve_capped_fps`.
    pub(crate) xtermjs_host: bool,
    pub(crate) default_fps_cap: f64,
    pub duration: Option<f64>,
    pub duration_s: Option<f64>,
    pub bench_frames: Option<u64>,
    pub benchmark: bool,
    /// Optional benchmark duration override in seconds.
    /// When None, defaults to BENCHMARK_DURATION_SECS (5s).
    /// Resolved exclusively from --bench-duration (bare seconds or compound
    /// like 6s/30m/1h30m). The hidden --duration flag is interactive-mode
    /// only (auto-exit deadline) and is NOT consulted by the benchmark
    /// dispatcher.
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
    /// --bench-scene <NAME>: bench I/O scene selector. None = default
    /// (emit_cell_lean); Some("production-draw") routes the writer through
    /// the production Terminal::draw hot path (MoveTo per row + ColorCache
    /// SGR + BOLT bold escape) so the BOLT-backed production path is
    /// measurable. Requires --bench-io.
    pub bench_scene: Option<String>,
    /// --verbose flag: print diagnostic info to stderr.
    pub verbose: bool,
    pub density_auto: bool,
    pub base_density: f32,
    pub perf_stats: bool,
    pub screensaver: bool,
    pub intro: IntroType,
    pub mouse: bool,
    pub charset_preset: String,
    pub user_ranges: Vec<(char, char)>,
    pub def_ascii: bool,
    /// Crystal Dragon Engine: ambient intelligence for palette drift.
    pub crystal_dragon: bool,
    /// Optional per-column density map for monolith pillar placement.
    /// Parsed from scene-custom.<name>.density-map config field (CSV f64).
    /// None = uniform distribution (default).
    pub(crate) monolith_density_map: Option<&'static [f64]>,
    /// Path to the config file being watched for live reload.
    /// None = no watcher (CLI-only run, no config file).
    pub(crate) config_path_for_watcher: Option<std::path::PathBuf>,
    /// Resolved scene name for this session. Used to initialize the
    /// event loop's scene_name (for verbose output and interactive cycling).
    pub(crate) scene_name: String,
    /// Name of the active custom scene (set via `--scene-custom <name>`).
    /// When Some, `rebuild_cloud_config` looks up `[scene-custom.<name>]`
    /// in the new config and applies its fields on top of the base
    /// CloudConfig so live-edits to a custom scene take effect immediately.
    /// v20: custom scenes are first-class citizens — this field is the
    /// bridge that lets live reload track which custom scene is active.
    pub(crate) scene_custom_name: Option<String>,
    /// Bug 3 fix: tracks which CloudConfig fields were set explicitly via
    /// CLI flags (vs derived from config.toml or scene defaults).
    ///
    /// The priority contract is **CLI > config.toml > scene default**.
    /// At startup, `apply_config_and_runtime_defaults` records which fields
    /// the user set on the command line (via clap's `ValueSource::CommandLine`).
    /// On live reload, `rebuild_cloud_config` consults this tracker to skip
    /// applying config.toml values for fields the user explicitly pinned via
    /// CLI — preserving the CLI's authority across reloads.
    ///
    /// Without this tracker, a user running `cosmostrix -c green` would
    /// have their CLI `--color green` overridden the moment they edit
    /// `color = "snow"` in config.toml during live reload. That violates
    /// the priority contract.
    pub(crate) cli_explicit: CliExplicit,
    /// Ambient phase schedule — collected from `ambient.<HH-MM>` config keys
    /// by `crate::ambient::collect_ambient_schedule`. Empty = no ambient
    /// entries (scheduler thread idles). The event loop spawns an
    /// `AmbientSchedulerHandle` from this and reloads it on every
    /// live-reload (see `event_loop.rs`).
    pub(crate) ambient_schedule: crate::ambient::AmbientSchedule,
}

/// Per-field record of which CloudConfig fields were set via CLI.
/// Used by `rebuild_cloud_config` to enforce the
/// **CLI > config.toml > scene default** priority contract across
/// live reloads.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CliExplicit {
    pub color: bool,
    pub charset: bool,
    pub speed: bool,
    pub density: bool,
    pub fps: bool,
    pub scene: bool,
    pub glitch_level: bool,
    /// Track whether `--crystal-dragon` was set on CLI (intent
    /// preservation: CLI flag wins over config.toml on live reload).
    pub crystal_dragon: bool,
}

impl CloudConfig {
    /// (FPS-F1): resolve `target_fps` for live-reload, re-applying
    /// the xterm.js 30 FPS cap. Without this, a user in VSCode could edit
    /// `fps = 240` in config.toml and resurrect the multi-hour OOM crash
    /// Tier 2 was designed to prevent. Native terminals have
    /// `default_fps_cap = 240.0` (effectively uncapped — startup clamps
    /// to [1, 240]). Falls back to `fallback_fps` if `self.target_fps`
    /// is ≤ 0.
    pub(crate) fn resolve_capped_fps(&self, fallback_fps: f64) -> f64 {
        let raw = if self.target_fps > 0.0 {
            self.target_fps
        } else {
            fallback_fps.max(1.0)
        };
        if self.xtermjs_host && raw > self.default_fps_cap {
            crate::lr_trace!(
                "live-reload: xterm.js host — capping fps {:.1}→{:.0}",
                raw,
                self.default_fps_cap,
            );
            self.default_fps_cap
        } else {
            raw
        }
    }

    pub fn create_cloud(&self, density: f32) -> Cloud {
        let mut cloud = Cloud::new(
            self.color_mode,
            self.shading_mode,
            self.bold_mode,
            self.async_mode,
            self.default_bg,
            self.color_scheme,
            self.rain_style,
        );

        cloud.glitchy = self.glitch_enabled;
        cloud.set_glitch_pct(self.glitch_pct / 100.0);
        cloud.set_glitch_times(self.glitch_low, self.glitch_high);
        cloud.set_linger_times(self.linger_low, self.linger_high);
        cloud.short_pct = self.short_pct / 100.0;
        cloud.die_early_pct = self.die_early_pct / 100.0;
        cloud.set_max_droplets_per_column(self.max_dpc);

        // Effective runtime values = base values (atmosphere engine eliminated
        // 2026-08-05 at commit 07b44b5; default modulation was always identity
        // even when it existed, so this assignment is unchanged in behavior).
        cloud.set_droplet_density(density);
        cloud.set_chars_per_sec(self.speed);
        cloud.set_monolith_size(self.monolith_size);

        cloud.init_chars(self.chars.clone());
        cloud.reset(DENSITY_AUTO_DEFAULT_COLS, DENSITY_AUTO_DEFAULT_LINES);

        // v16: Apply custom palette AFTER cloud.reset() to guarantee no
        // initialization code overwrites it. set_palette handles color_map
        // regen + transition wave + monolith reset internally.
        if let Some(ref custom) = self.custom_palette {
            cloud.set_palette(custom.clone());
        }

        // Apply --color-tune (if non-identity) to the palette AFTER custom
        // palette injection. This lets users tune custom palettes the same
        // way as built-in ones.
        crate::color_tune::apply_tune_to_palette(
            &mut cloud.palette,
            self.color_mode,
            &self.color_tune,
        );

        // v14 Peak Monolith: apply per-column density map if set.
        // This sculpts pillar formation — columns with weight 0.0 never spawn,
        // 1.0 always spawn. Enables artistic compositions (twin towers, clusters).
        if let Some(map) = self.monolith_density_map {
            cloud.set_monolith_density_map(Some(map));
        }

        // v17 mastery: hover/click visual effects are ALWAYS ON (--mouse flag
        // deleted). Mouse reporting is also always on (terminal-level, blocks
        // text selection). cloud.mouse_enabled now always true.
        cloud.mouse_enabled = true;

        // Crystal Dragon Engine: when enabled, activates the point-based
        // temperature group system for palette drift.
        cloud.crystal_dragon = self.crystal_dragon;
        // crystal_dragon_sensor and crystal_dragon_control are already
        // initialized in Cloud::new() with default config. Future CLI
        // flags can override crystal_dragon_control here.

        // v30 strengthen (Bug #4): if a custom palette is active, drift's
        // set_color_scheme would overwrite the user's custom palette with a
        // built-in one (silent data loss). Track this so the rain loop can
        // suppress palette drift while still allowing climate drift (which
        // only modulates rendering params, not the palette itself).
        cloud.custom_palette_active = self.custom_palette.is_some();

        // v30 strengthen (Bug #5): store color_tune on Cloud so that
        // set_color_scheme can re-apply it after rebuilding the palette.
        // Without this, the first palette drift would silently drop the
        // user's --color-tune settings.
        cloud.color_tune = self.color_tune;

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
            shading_mode: self.shading_mode,
            bold_mode: self.bold_mode,
            async_mode: self.async_mode,
            default_bg: self.default_bg,
            color_scheme: self.color_scheme,
            custom_palette: self.custom_palette.clone(),
            custom_palette_name: self.custom_palette_name.clone(),
            rain_style: self.rain_style,
            glitch_enabled: self.glitch_enabled,
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
            xtermjs_host: self.xtermjs_host,
            default_fps_cap: self.default_fps_cap,
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
            bench_scene: None,
            verbose: false,
            density_auto: self.density_auto,
            base_density: self.base_density,
            perf_stats: false,
            screensaver: false,
            intro: IntroType::None,
            mouse: false,
            charset_preset: self.charset_preset.clone(),
            user_ranges: self.user_ranges.clone(),
            def_ascii: self.def_ascii,
            crystal_dragon: self.crystal_dragon,
            monolith_density_map: self.monolith_density_map,
            config_path_for_watcher: None, // watcher only for interactive, not benchmark
            scene_name: self.scene_name.clone(),
            scene_custom_name: self.scene_custom_name.clone(),
            cli_explicit: self.cli_explicit,
            ambient_schedule: self.ambient_schedule.clone(),
        }
    }
}

// --- Density calculation helpers ---

/// Auto-density factor for the current terminal size.
///
/// v17 audit: the old formula was `sqrt(area / (80*25))` clamped to [0.5, 2.0].
/// This was conceptually wrong for cosmostrix's per-column density model:
///
///   - cosmostrix's `density` means "fraction of columns active" (glyph) or
///     "active lane ratio scale" (monolith). Both are inherently scale-
///     invariant quantities — a 200x60 terminal should have the SAME column
///     density as an 80x24 terminal, just with more columns.
///   - The old `sqrt(area)` formula double-counted width scaling (cols ×
///     density already scales with width) and added bogus height scaling
///     (more rows = longer droplet lifetime = fewer spawns needed, already
///     handled by recalc_droplets_per_sec).
///   - At 200x60, the old formula gave factor=2.0, so base_density=0.85
///     became effective=1.7 — 62% above the monolith ceiling (1.04), maxing
///     out the 35% active-lane cap on every non-trivial terminal.
///
/// The new formula is a **width-only dampener** for small terminals:
///
///   factor = clamp(cols / 80, 0.6, 1.0)
///
/// - At 80+ cols: factor = 1.0 (identity — no amplification, no reduction)
/// - At 48 cols: factor = 0.6 (small terminals get slightly sparser rain
///   to avoid over-saturation when each column is more visible)
/// - Never amplifies above 1.0 — the per-column model is already scale-
///   invariant, so amplification was always a bug.
///
/// .0-alpha.3: the legacy `--fullwidth` parameter (which doubled the
/// column stride for monolith streams) was removed. The `fullwidth` flag
/// is gone, so this function no longer needs a `fullwidth` parameter —
/// columns are always single-width (the Cosmic Dragon principle forbids
/// wide chars permanently; the charset is always single-width).
#[must_use]
pub fn auto_density_factor(cols: u16) -> f32 {
    let eff_cols = cols.max(1) as f32;
    // Width-only dampener: terminals smaller than 80 cols get slightly
    // sparser rain; 80+ cols get identity (factor=1.0). Never amplifies.
    let factor = eff_cols / DENSITY_BASE_COLS;
    factor.clamp(DENSITY_AUTO_MIN, 1.0)
}

/// Compute the effective droplet density for the current terminal.
///
/// When `auto` is true (user did NOT pass `--density` explicitly), the
/// base density is multiplied by `auto_density_factor()` — a width-only
/// dampener that never amplifies. When `auto` is false (user passed
/// `--density N`), the base is returned as-is (clamped to safe bounds).
///
/// See `auto_density_factor()` for the rationale on why the old
/// `sqrt(area)` amplifier was removed.
#[must_use]
pub fn effective_density(base: f32, cols: u16, auto: bool) -> f32 {
    let base = base.clamp(DENSITY_CLAMP_MIN, DENSITY_CLAMP_MAX);
    if !auto {
        return base;
    }
    (base * auto_density_factor(cols)).clamp(DENSITY_CLAMP_MIN, DENSITY_CLAMP_MAX)
}
