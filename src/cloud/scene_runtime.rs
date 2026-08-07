// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Runtime scene switching and rain style transitions.
//!
//! Handles the logic for switching between scenes (monolith, matrix, signal)
//! at runtime, including rain style transitions, glyph warm-starting, and
//! scene-managed value application (color, charset, speed, density, glitch).

use std::time::Instant;

use rand::distr::Distribution;

use crate::config::GlitchLevel;
use crate::rain_style::RainStyle;

use super::Cloud;

impl Cloud {
    /// Apply a runtime scene switch. Updates rain_style, color, charset,
    /// speed, density, and glitch-level from the scene config.
    ///
    /// If the scene specifies a value for a parameter, it is applied.
    /// If the scene does not specify a value (None), the current state
    /// is preserved. This means runtime scene cycling always applies
    /// scene-managed values; explicit CLI overrides set at startup are
    /// not tracked at runtime.
    ///
    /// Returns the charset preset name used (scene's or current).
    pub fn apply_scene_runtime(
        &mut self,
        scene_name: &str,
        current_charset_preset: &str,
        user_ranges: &[(char, char)],
        def_ascii: bool,
    ) -> String {
        use crate::charset::{build_chars, charset_from_str};
        use crate::cli::parse_color_scheme;
        use crate::scene;

        let Some(scene_info) = scene::get_scene(scene_name) else {
            return current_charset_preset.to_string();
        };
        self.scene_name = scene_name.to_string();

        let new_style = scene_info.config.rain_style;
        if self.rain_style != new_style {
            self.transition_rain_style(new_style);
        }

        // Apply scene color if specified
        if let Some(color_name) = scene_info.config.color {
            if let Ok(scheme) = parse_color_scheme(color_name) {
                self.set_color_scheme(scheme);
            }
        }

        // Apply scene charset if specified
        let charset_name: &str = scene_info.config.charset.unwrap_or(current_charset_preset);
        let charset_owned = charset_name.to_string();
        if let Ok(cs) = charset_from_str(charset_name, def_ascii) {
            let chars = build_chars(cs, user_ranges, def_ascii);
            self.transition_chars(chars);
        }

        // Apply speed
        if let Some(speed) = scene_info.config.speed {
            self.set_chars_per_sec(speed);
        }

        // Apply density
        if let Some(density) = scene_info.config.density {
            self.set_droplet_density(density);
        }

        // Apply glitch level
        if let Some(glitch) = scene_info.config.glitch_level {
            self.apply_glitch_level_runtime(glitch);
        }

        self.semantic_invalidate = true;
        self.force_draw_everything = true;
        self.last_spawn_time = Instant::now();
        // Only reset spawn debt for monolith; glyph warm-start sets its own.
        if matches!(self.rain_style, RainStyle::Monolith) {
            self.spawn_remainder = 0.0;
        }

        charset_owned
    }

    /// Transition to a different rain style, clearing all state for
    /// both the old and new style to prevent ghosting or residue.
    /// For glyph styles, the droplet pool is re-allocated and warm-started
    /// so the first post-switch frame has visible content immediately.
    pub(super) fn transition_rain_style(&mut self, new_style: RainStyle) {
        if matches!(self.rain_style, RainStyle::Monolith) {
            self.monolith_rain.clear_draw_history();
        }
        self.rain_style = new_style;
        if matches!(new_style, RainStyle::Monolith) {
            self.monolith_rain.reset(self.cols);
            self.droplets.clear();
            self.spawn_remainder = 0.0;
            self.glyph_entry_time = None;
        } else {
            // Re-allocate glyph droplet pool and warm-start so the
            // first post-switch frame has visible rain immediately,
            // preventing the blank-screen bug on monolith→glyph switch.
            self.ensure_glyph_pool_and_warm_start();
        }
        self.reset_phosphor_state();
        self.semantic_invalidate = true;
        self.force_draw_everything = true;
        self.last_spawn_time = Instant::now();
    }

    /// Apply glitch level parameters directly at runtime.
    ///
    /// Public so `event_loop.rs` can apply `adaptive-custom.glitch-level` at
    /// scheduled time points. Scene runtime also calls this when a scene
    /// specifies `glitch-level`. Idempotent in the sense that calling with
    /// the same level twice is safe (it resets glitch timing, which is a
    /// minor side effect — callers should still gate with an "if changed"
    /// check to avoid needless resets every 30s).
    pub fn apply_glitch_level_runtime(&mut self, level: GlitchLevel) {
        let (on, pct, lo, hi, short, rip) = match level {
            GlitchLevel::None => (false, 0.0, 300u16, 400u16, 0.5f32, 0.3333333f32),
            GlitchLevel::Subtle => (true, 0.03, 200, 300, 0.6, 0.45),
            GlitchLevel::Default => (true, 0.10, 300, 400, 0.5, 0.3333333),
            GlitchLevel::Intense => (true, 0.25, 500, 800, 0.3, 0.2),
        };
        self.glitchy = on;
        self.glitch_pct = pct;
        self.glitch_low_ms = lo;
        self.glitch_high_ms = hi;
        self.short_pct = short;
        self.die_early_pct = rip;
        if on {
            self.fill_glitch_map();
            let now = Instant::now();
            self.last_glitch_time = now;
            let ms = self.rand_glitch_ms.sample(&mut self.mt) as u64;
            self.next_glitch_time = now + std::time::Duration::from_millis(ms);
        } else {
            self.glitch_map.clear();
        }
        self.force_draw_everything = true;
    }

    /// Apply an ambient phase entry at runtime — instant switch (no blend).
    ///
    /// Called by the event loop when the ambient scheduler thread fires a
    /// phase boundary. Reads the entry's fields and applies them with
    /// **sticky semantics**: any `None` field is skipped (previous value
    /// retained). This matches the archived `adaptive-custom` contract.
    ///
    /// Order of application:
    /// 1. `scene` (if Some and different from current) → calls
    ///    `apply_scene_runtime` which handles rain_style transition,
    ///    glyph pool realloc, and applies scene-managed defaults.
    /// 2. `color` (if Some) → resolves built-in OR `colors-custom.<name>`,
    ///    then `set_color_scheme` (built-in) or `set_palette` (custom).
    /// 3. `charset` (if Some) → resolves built-in OR `charset-custom.<name>`,
    ///    then `transition_chars`.
    /// 4. `speed` (if Some) → `set_chars_per_sec`.
    /// 5. `density` (if Some) → `set_droplet_density`.
    /// 6. `glitch_level` (if Some) → `apply_glitch_level_runtime`.
    ///
    /// `fps` is NOT applied here — it lives in the event loop's
    /// `target_period`, not on Cloud. The caller is responsible for
    /// updating `target_period` when an entry's `fps` field changes.
    ///
    /// Returns the charset preset name used (entry's or current).
    pub fn apply_ambient_entry(
        &mut self,
        entry: &crate::ambient::AmbientEntry,
        current_charset_preset: &str,
        user_ranges: &[(char, char)],
        def_ascii: bool,
        cfg: &std::collections::HashMap<String, String>,
    ) -> String {
        use crate::charset::{build_chars, charset_from_str};
        use crate::cli::parse_color_scheme;

        // 1. Scene switch (if specified). Reuse apply_scene_runtime which
        //    handles rain_style transition + glyph warm-start + scene-managed
        //    defaults. Skip if scene is None (sticky).
        let mut charset_preset = current_charset_preset.to_string();
        if let Some(scene_name) = &entry.scene {
            charset_preset =
                self.apply_scene_runtime(scene_name, &charset_preset, user_ranges, def_ascii);
        }

        // 2. Color (if specified). Built-in scheme OR colors-custom.<name>.
        if let Some(color_name) = &entry.color {
            if let Ok(scheme) = parse_color_scheme(color_name) {
                self.set_color_scheme(scheme);
            } else if let Ok(palette) = crate::colors_custom::load_custom_palette(cfg, color_name) {
                self.set_palette(palette);
            }
        }

        // 3. Charset (if specified). Built-in OR charset-custom.<name>.
        if let Some(charset_name) = &entry.charset {
            if let Some(custom_chars) =
                crate::charset_custom::load_custom_charset_if_matches(cfg, charset_name)
            {
                charset_preset = charset_name.clone();
                self.transition_chars(custom_chars);
            } else if let Ok(charset) = charset_from_str(charset_name, def_ascii) {
                charset_preset = charset_name.clone();
                let chars = build_chars(charset, user_ranges, def_ascii);
                self.transition_chars(chars);
            }
        }

        // 4. Speed (if specified).
        if let Some(speed) = entry.speed {
            self.set_chars_per_sec(speed);
        }

        // 5. Density (if specified).
        if let Some(density) = entry.density {
            self.set_droplet_density(density);
        }

        // 6. Glitch level (if specified).
        if let Some(glitch_str) = &entry.glitch_level {
            use clap::ValueEnum;
            if let Ok(level) = GlitchLevel::from_str(glitch_str, true) {
                self.apply_glitch_level_runtime(level);
            }
        }

        self.semantic_invalidate = true;
        self.force_draw_everything = true;

        charset_preset
    }
}
