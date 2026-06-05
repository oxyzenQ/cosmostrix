// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

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
            self.monolith_rain.reset(self.cols, self.full_width);
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
    pub(super) fn apply_glitch_level_runtime(&mut self, level: GlitchLevel) {
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
}
