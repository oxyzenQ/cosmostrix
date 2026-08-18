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
    /// Switch the active color scheme and start a palette transition wave.
    ///
    /// This method ALWAYS applies the scheme — it rebuilds the palette,
    /// re-randomizes the per-cell color map, resets column palette slots,
    /// starts the 300ms transition wave, and clears stale monolith draw
    /// history / phosphor state.
    ///
    /// Callers that need a same-scheme no-op guard (e.g. scene cycling,
    /// where `cinematic` and `monolith` both use `neon-purple`) must
    /// compare `self.color_scheme()` themselves before calling. The guard
    /// lives at the call site (`apply_builtin_scene_runtime`,
    /// `apply_custom_scene_runtime`) so that direct callers (tests, `c`
    /// key, live config reload) retain the full cleanup behavior even
    /// when the scheme is unchanged.
    pub fn set_color_scheme(&mut self, scheme: ColorScheme) {
        self.color_scheme = scheme;
        // (Color-#1): switching to a builtin scheme means a custom
        // palette (if any was loaded) is no longer the source of truth.
        // Without this clear, the `custom_palette_active` flag would stay
        // true after `--colors-custom X` + 'c' cycle, falsely blocking
        // --crystal-dragon forever (the drift gate at rain.rs reads
        // this flag). Note: the 'c' cycle path already calls this fn.
        self.custom_palette_active = false;
        use crate::palette::build_palette;
        let mut new_palette = build_palette(scheme, self.color_mode, self.default_background);
        // v30 strengthen (Bug #5): re-apply color_tune after palette rebuild.
        // Without this, the first palette drift would silently drop the
        // user's --color-tune settings (sat/bright/head/body/tail). The
        // tune is stored on Cloud at construction time (see app.rs).
        // Identity tune is a no-op (all multipliers are 1.0).
        crate::color_tune::apply_tune_to_palette(
            &mut new_palette,
            self.color_mode,
            &self.color_tune,
        );
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
        // (Color-#1): mark custom_palette_active so the drift gate
        // (rain.rs:923 `!custom_palette_active && !ambient_palette_locked`)
        // correctly suppresses palette drift while a custom palette is
        // loaded at runtime (e.g. ambient fires a scene with
        // `colors-custom = "morning_brand"`). Without this set, drift
        // would silently overwrite the custom palette after the ambient
        // lock clears — the exact "silent data loss" bug v30 strengthen
        // (Bug #4) was supposed to prevent (it only covered the
        // startup-time --colors-custom case, not the runtime ambient fire).
        self.custom_palette_active = true;
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

    /// Read-only accessor for the current droplet density multiplier.
    /// v50 (2026-08-17) HUD expansion: feeds the `dsty:` HUD line so the
    /// owner can see the actual density value while adjusting it via
    /// `[` / `]` keys — previously the value was invisible to the user.
    /// Returns the sanitized value (set_droplet_density writes the raw
    /// value, density itself is not sanitized — only speed is).
    pub fn droplet_density(&self) -> f32 {
        self.droplet_density
    }

    /// Read-only accessor for the current chars-per-second speed.
    /// v50 (2026-08-17) HUD expansion: feeds the `sped:` HUD line so the
    /// owner can see the actual speed value while adjusting it via `↑`
    /// / `↓` keys — previously the value was invisible to the user.
    /// Returns the sanitized value (set_chars_per_sec applies
    /// `sanitize_speed_for_style` for the active rain style).
    pub fn chars_per_sec(&self) -> f32 {
        self.chars_per_sec
    }

    pub fn set_glitchy(&mut self, on: bool) {
        self.glitchy = on;
        // (Glitch-BUG6): when disabling glitch, clear in-flight anomaly
        // zones to match the Glitch-P0 fix in apply_glitch_level_runtime.
        // Without this, a future code path that uses set_glitchy(false) to
        // disable glitch at runtime would leave LuminanceSurge /
        // GlyphCorruption / PulseWave anomalies active for up to
        // ANOMALY_DURATION_SECS (1.5s) after — the exact bug Glitch-P0 fixed
        // for the scene-switch path. Currently set_glitchy is test-only, but
        // this future-proofs the API.
        if !on {
            self.anomaly_zones.clear();
        }
        self.fill_glitch_map();
        if on {
            let now = std::time::Instant::now();
            self.last_glitch_time = now;
            let ms = self.rand_glitch_ms.sample(&mut self.mt) as u64;
            self.next_glitch_time = now + Duration::from_millis(ms);
        }
        self.semantic_invalidate = true;
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

    /// AB-11: set the aggressive-throttle flag. When true, `rain_at()` uses
    /// steeper spawn-scale + disables glitches — WITHOUT touching the user's
    /// color/charset/density/speed/glitch_level. Called by the self-healer
    /// via the event loop on sustained high/low CPU pressure.
    pub fn set_aggressive_throttle(&mut self, on: bool) {
        self.aggressive_throttle = on;
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

    /// Start a 300ms palette transition wave from a previous palette.
    ///
    /// Used by live config reload when the color scheme changes: the Cloud
    /// rebuild already installed the new palette in slot 0, but without this
    /// call the transition would be an instant jump (`transition_start = None`).
    ///
    /// This method:
    /// 1. Stores `prev_palette` in the circular-buffer slot BEFORE the
    ///    active slot, so the shader can read both old and new palettes
    ///    during the 300ms wave.
    /// 2. Sets `transition_start = Some(now)` to activate the wave.
    /// 3. Sets `force_draw_everything` + `semantic_invalidate` so the
    ///    first frame redraws everything under the new palette.
    ///
    /// **Precondition**: the caller must ensure that `prev_palette` is
    /// genuinely different from `self.palette` (same-scheme no-op guard
    /// is the caller's responsibility, matching the contract of
    /// `set_color_scheme`).
    pub fn start_transition_from_previous_palette(
        &mut self,
        prev_palette: crate::palette::Palette,
    ) {
        // Compute the "previous" slot in the circular buffer: one step
        // backward from the active slot. This is where the shader reads
        // the old palette during the transition wave (see rain.rs:451).
        let prev_slot =
            ((self.active_palette_slot as usize + MAX_PALETTE_SLOTS - 1) % MAX_PALETTE_SLOTS) as u8;
        self.palette_table[prev_slot as usize] = Some(prev_palette);

        // Activate the 300ms top-to-bottom wave transition.
        self.transition_start = Some(std::time::Instant::now());

        // Force full redraw so the new background fills the entire screen
        // (matching apply_new_palette's behavior).
        self.force_draw_everything = true;
        self.semantic_invalidate = true;
    }

    // ── Crystal Dragon Engine ──────────────────────────────────────────

    /// Tick the Crystal Dragon engine and maybe return a new color scheme.
    ///
    /// Polls the sensor (CPU or CLOCK) if the polling interval has elapsed,
    /// then probabilistically selects a new color theme from the temperature
    /// group (Cold/Medium/Hot) matching the current point.
    ///
    /// Returns `Some(new_scheme)` if a drift event should occur, or `None`
    /// if the engine decides to stay on the current theme this tick.
    ///
    /// The caller (rain.rs) applies the new scheme via `set_color_scheme`,
    /// which triggers the 300 ms OKLab wave transition via Chroma Dragon.
    pub(crate) fn crystal_dragon_tick(&mut self, now: std::time::Instant) -> Option<ColorScheme> {
        use crate::crystal_dragon_engine::crystal_dragon_control::CRYSTAL_DRAGON_DRIFT_CHANCE;
        use crate::crystal_dragon_engine::point_system::calc_v1_select;

        // Check if the polling interval has elapsed.
        let elapsed_since_poll = match self.crystal_dragon_last_poll {
            None => {
                // First poll — initialize timestamp and poll immediately.
                self.crystal_dragon_last_poll = Some(now);
                self.crystal_dragon_sensor.poll(now);
                0.0
            }
            Some(last) => now.saturating_duration_since(last).as_secs_f32(),
        };

        if elapsed_since_poll >= self.crystal_dragon_control.polling_secs {
            self.crystal_dragon_last_poll = Some(now);
            self.crystal_dragon_sensor.poll(now);
        }

        // Dwell hysteresis: don't drift if we haven't dwelled in the
        // current theme long enough.
        let dwell = now
            .saturating_duration_since(self.crystal_dragon_sensor.theme_entered_at())
            .as_secs_f32();
        if dwell < self.crystal_dragon_control.min_dwell_secs {
            return None;
        }

        // Probabilistic gate: drift only with CRYSTAL_DRAGON_DRIFT_CHANCE probability.
        let chance_dist = Uniform::new(0.0f32, 1.0f32).expect("chance_dist always valid");
        if chance_dist.sample(&mut self.mt) >= CRYSTAL_DRAGON_DRIFT_CHANCE {
            return None;
        }

        // calc-v1: probabilistic weighted theme selection.
        let current_point = self.crystal_dragon_sensor.current_point();
        let new_scheme = calc_v1_select(current_point, self.color_scheme, &mut self.mt);

        if new_scheme.is_some() {
            self.crystal_dragon_sensor.record_theme_transition(now);
        }

        new_scheme
    }
}
