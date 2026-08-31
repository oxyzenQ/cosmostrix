// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Runtime scene switching and rain style transitions.
//!
//! Handles the logic for switching between scenes (monolith, matrix, signal)
//! at runtime, including rain style transitions, glyph warm-starting, and
//! scene-managed value application (color, charset, speed, density, glitch).
//!
//! `apply_scene_runtime` now accepts an optional `cfg` parameter
//! (via the `_with_cfg` variant) so it can resolve custom scenes by looking
//! up `[scene-custom.<name>]` blocks. When the named scene is a custom
//! scene, the runtime applies the block's `base-scene` defaults first
//! (rain_style + scene-managed fields), then the block's own overrides.
//! Built-in scene names take the fast path (no cfg lookup needed).

use std::collections::HashMap;
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
    ///
    /// This method only handles built-in scenes. For custom scenes
    /// (referenced via `--scene-custom` or ambient entries), use
    /// [`Cloud::apply_scene_runtime_with_cfg`] which can resolve
    /// `[scene-custom.<name>]` blocks. The interactive scene-cycle keys
    /// (`x`) only cycles through built-in scenes (`SCENE_ORDER`), so
    /// they can safely call this method directly.
    pub fn apply_scene_runtime(
        &mut self,
        scene_name: &str,
        current_charset_preset: &str,
        user_ranges: &[(char, char)],
        def_ascii: bool,
    ) -> String {
        // Try built-in scene first; if not found, return current charset
        // (no-op). Custom scenes are NOT resolved here — callers that need
        // custom-scene support should use apply_scene_runtime_with_cfg.
        let Some(scene_info) = crate::scene::get_scene(scene_name) else {
            return current_charset_preset.to_string();
        };
        self.apply_builtin_scene_runtime(
            scene_name,
            scene_info.config,
            current_charset_preset,
            user_ranges,
            def_ascii,
        )
    }

    /// Apply a runtime scene switch with custom-scene support.
    ///
    /// Like [`Cloud::apply_scene_runtime`] but also resolves custom scenes
    /// via `[scene-custom.<name>]` blocks in `cfg`. When `scene_name` is a
    /// custom scene:
    ///
    /// 1. Look up `scene-custom.<name>` block.
    /// 2. If `base-scene = <built-in>` is set, apply that built-in scene's
    ///    rain_style + color/charset/speed/density/glitch defaults first.
    /// 3. Apply the custom block's own overrides (color/charset/speed/
    ///    density/glitch-level) on top.
    ///
    /// Built-in scene names take the fast path (same as
    /// `apply_scene_runtime`). Unknown scenes (neither built-in nor a
    /// defined custom block) are a no-op (return current charset preset).
    pub fn apply_scene_runtime_with_cfg(
        &mut self,
        scene_name: &str,
        current_charset_preset: &str,
        user_ranges: &[(char, char)],
        def_ascii: bool,
        cfg: &HashMap<String, String>,
    ) -> String {
        // Fast path: built-in scene.
        if let Some(scene_info) = crate::scene::get_scene(scene_name) {
            return self.apply_builtin_scene_runtime(
                scene_name,
                scene_info.config,
                current_charset_preset,
                user_ranges,
                def_ascii,
            );
        }
        // Custom scene path: look up block, apply base-scene defaults then
        // custom overrides.
        self.apply_custom_scene_runtime(
            scene_name,
            current_charset_preset,
            user_ranges,
            def_ascii,
            cfg,
        )
    }

    /// Apply a built-in scene's `SceneConfig` to the live Cloud state.
    /// Shared by `apply_scene_runtime` (built-in fast path) and
    /// `apply_scene_runtime_with_cfg` (base-scene inheritance layer).
    fn apply_builtin_scene_runtime(
        &mut self,
        scene_name: &str,
        config: crate::scene::SceneConfig,
        current_charset_preset: &str,
        user_ranges: &[(char, char)],
        def_ascii: bool,
    ) -> String {
        use crate::charset::{build_chars, charset_from_str};
        use crate::cli::parse_color_scheme;

        self.scene_name = scene_name.to_string();

        let new_style = config.rain_style;
        if self.rain_style != new_style {
            self.transition_rain_style(new_style);
        }

        // Apply scene color if specified.
        // Guard: skip set_color_scheme when the scene's color matches the
        // current scheme. This prevents a spurious 300ms palette transition
        // wave when cycling between scenes that share the same palette
        // (e.g. cinematic → monolith, both neon-purple). Without this guard,
        // the transition wave destabilizes apply_quantum_ripple's blend base
        // (cell.fg is mid-transition old/new palette mix), producing
        // inconsistent click effect colors — the "snow ice vs spark fire" bug.
        if let Some(color_name) = config.color {
            if let Ok(scheme) = parse_color_scheme(color_name) {
                if scheme != self.color_scheme {
                    self.set_color_scheme(scheme);
                }
            }
        }

        // Apply scene charset if specified.
        // Guard: skip transition_chars when the preset matches the current
        // one — cinematic and monolith both use "zen", so cycling between
        // them must NOT start a charset transition wave (same root cause as
        // the color-scheme guard above).
        let charset_name: &str = config.charset.unwrap_or(current_charset_preset);
        let charset_owned = charset_name.to_string();
        if charset_name != current_charset_preset {
            if let Ok(cs) = charset_from_str(charset_name, def_ascii) {
                let chars = build_chars(cs, user_ranges, def_ascii);
                self.transition_chars(chars);
            }
        }

        // Apply speed
        if let Some(speed) = config.speed {
            self.set_chars_per_sec(speed);
        }

        // Apply density
        if let Some(density) = config.density {
            self.set_droplet_density(density);
        }

        // Apply glitch level
        if let Some(glitch) = config.glitch_level {
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

    /// Apply a custom scene (from `[scene-custom.<name>]` block) at runtime.
    ///
    /// Step 1: apply base-scene's defaults (rain_style + color/charset/speed/
    /// density/glitch) if `base-scene = <built-in>` is set.
    /// Step 2: apply the custom block's own overrides (color/charset/speed/
    /// density/glitch-level). Fields not set in the block retain the base
    /// scene's values (or current Cloud state if no base-scene).
    ///
    /// `fps`, `monolith-size`, `color-bg`, `density-map` are NOT applied at
    /// runtime — they live on the event loop / Cloud construction path.
    fn apply_custom_scene_runtime(
        &mut self,
        scene_name: &str,
        current_charset_preset: &str,
        user_ranges: &[(char, char)],
        def_ascii: bool,
        cfg: &HashMap<String, String>,
    ) -> String {
        use crate::charset::{build_chars, charset_from_str};
        use crate::cli::parse_color_scheme;
        use clap::ValueEnum;

        let custom_scenes = crate::scene_custom::collect_custom_scenes(cfg);
        let normalized = scene_name.trim().to_ascii_lowercase();
        let Some(custom) = custom_scenes.get(&normalized) else {
            // Unknown scene — no-op.
            return current_charset_preset.to_string();
        };

        self.scene_name = scene_name.to_string();
        let mut charset_preset = current_charset_preset.to_string();

        // Step 1: apply base-scene defaults (if any).
        if let Some(base_name) = custom.base_scene.as_deref() {
            if let Some(base_info) = crate::scene::get_scene(base_name) {
                let base_cfg = base_info.config;
                // rain_style
                let new_style = base_cfg.rain_style;
                if self.rain_style != new_style {
                    self.transition_rain_style(new_style);
                }
                // color — guard: skip set_color_scheme when the base-scene's
                // color matches the current scheme (same rationale as the
                // built-in path: avoid spurious palette transition wave).
                if let Some(color_name) = base_cfg.color {
                    if let Ok(scheme) = parse_color_scheme(color_name) {
                        if scheme != self.color_scheme {
                            self.set_color_scheme(scheme);
                        }
                    }
                }
                // charset — guard: skip transition_chars when the base-scene's
                // charset matches the current preset (same rationale as the
                // built-in path: avoid spurious charset transition wave).
                let base_charset = base_cfg.charset.unwrap_or(current_charset_preset);
                charset_preset = base_charset.to_string();
                if base_charset != current_charset_preset {
                    if let Ok(cs) = charset_from_str(base_charset, def_ascii) {
                        let chars = build_chars(cs, user_ranges, def_ascii);
                        self.transition_chars(chars);
                    }
                }
                // speed
                if let Some(speed) = base_cfg.speed {
                    self.set_chars_per_sec(speed);
                }
                // density
                if let Some(density) = base_cfg.density {
                    self.set_droplet_density(density);
                }
                // glitch
                if let Some(glitch) = base_cfg.glitch_level {
                    self.apply_glitch_level_runtime(glitch);
                }
            }
        } else {
            // No base-scene — transition rain_style to Glyph (custom scenes
            // default to Glyph when no base-scene is set, matching the
            // construction-time rain_style_for_custom_scene fallback).
            if !matches!(self.rain_style, RainStyle::Glyph) {
                self.transition_rain_style(RainStyle::Glyph);
            }
        }

        // Step 2: apply custom block overrides.
        // color (built-in scheme via `color`, OR custom palette via `colors-custom`)
        // (Color-#2): add `if scheme != self.color_scheme` guard matching
        // the built-in path (line 142-146). Without this, an ambient fire of a
        // custom scene whose `color` matches the current scheme triggers a
        // spurious 300ms palette transition wave — the "snow ice vs spark fire"
        // bug. The guard at line 234-237 (Step 1 base-scene) already has this
        // check; this fix makes Step 2 consistent.
        if let Some(color_name) = &custom.color {
            if let Ok(scheme) = parse_color_scheme(color_name) {
                if scheme != self.color_scheme {
                    self.set_color_scheme(scheme);
                }
            } else if let Ok(palette) = crate::colors_custom::load_custom_palette(cfg, color_name) {
                self.set_palette(palette);
            }
        }
        // `colors-custom` — explicit custom palette name. Applied
        // only if `color` wasn't set (avoids last-writer-wins confusion).
        if custom.color.is_none() {
            if let Some(palette_name) = &custom.colors_custom {
                if let Ok(palette) = crate::colors_custom::load_custom_palette(cfg, palette_name) {
                    self.set_palette(palette);
                }
            }
        }
        // charset (built-in via `charset`, OR custom via `charset-custom`)
        if let Some(charset_name) = &custom.charset {
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
        // `charset-custom` — explicit custom charset name. Applied
        // only if `charset` wasn't set.
        if custom.charset.is_none() {
            if let Some(charset_name) = &custom.charset_custom {
                if let Some(custom_chars) =
                    crate::charset_custom::load_custom_charset_if_matches(cfg, charset_name)
                {
                    charset_preset = charset_name.clone();
                    self.transition_chars(custom_chars);
                }
            }
        }
        // speed
        if let Some(speed_str) = &custom.speed {
            if let Ok(speed) = speed_str.trim().parse::<f32>() {
                self.set_chars_per_sec(speed);
            }
        }
        // density
        if let Some(density_str) = &custom.density {
            if let Ok(density) = density_str.trim().parse::<f32>() {
                self.set_droplet_density(density);
            }
        }
        // glitch-level
        if let Some(glitch_str) = &custom.glitch_level {
            if let Ok(level) = GlitchLevel::from_str(glitch_str, true) {
                self.apply_glitch_level_runtime(level);
            }
        }
        // bold (0=Off, 1=Random, 2=All) — matches --bold CLI.
        // (CLI-V-5): tighten to reject values > 2 (was silently Random).
        // testconf rejects bold=99 with an error; the startup top-level path
        // rejects via parse_canonical_u8_range. This makes the runtime scene
        // path consistent: unknown values are silently ignored (mode unchanged)
        // rather than silently coerced to Random. Uses a labeled block so we
        // skip just the assignment, NOT the rest of the function (return would
        // wrongly skip shadingmode/async-mode/etc. fields below).
        if let Some(bold_str) = &custom.bold {
            if let Ok(n) = bold_str.trim().parse::<u8>() {
                'bold: {
                    let mode = match n {
                        0 => crate::runtime::BoldMode::Off,
                        1 => crate::runtime::BoldMode::Random,
                        2 => crate::runtime::BoldMode::All,
                        _ => break 'bold,
                    };
                    self.bold_mode = mode;
                }
            }
        }
        // shadingmode (0=Random, 1=DistanceFromHead).
        // (CLI-V-5): tighten to reject values > 1 (was silently Random).
        if let Some(shading_str) = &custom.shading_mode {
            if let Ok(n) = shading_str.trim().parse::<u8>() {
                'shading: {
                    let sm = match n {
                        0 => crate::runtime::ShadingMode::Random,
                        1 => crate::runtime::ShadingMode::DistanceFromHead,
                        _ => break 'shading,
                    };
                    self.set_shading_mode(sm);
                }
            }
        }
        // async-mode (true/false).
        if let Some(async_str) = &custom.async_mode {
            let on = matches!(
                async_str.trim().to_ascii_lowercase().as_str(),
                "true" | "1" | "yes" | "on"
            );
            self.set_async(on);
        }
        // Note: fps, density-map are not runtime-applicable — they are
        // construction-time only. monolith-size and color-bg are forbidden
        // in scene-custom blocks per  owner contract.

        self.semantic_invalidate = true;
        self.force_draw_everything = true;
        self.last_spawn_time = Instant::now();
        if matches!(self.rain_style, RainStyle::Monolith) {
            self.spawn_remainder = 0.0;
        }

        charset_preset
    }

    /// Transition to a different rain style, clearing all state for
    /// both the old and new style to prevent ghosting or residue.
    /// For glyph styles, the droplet pool is re-allocated and warm-started
    /// so the first post-switch frame has visible content immediately.
    pub(crate) fn transition_rain_style(&mut self, new_style: RainStyle) {
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
        // ME-03..ME-05 (mouse-effect state leak fix): clear mouse-click /
        // quantum-ripple state on scene switch. Stale flash waves + quantum
        // particles from the previous scene would otherwise render in the
        // new scene with the previous palette's snapshot color for up to
        // 1.8s (flash) / 0.8s (quantum). Refreshing the timing anchors
        // (`last_phosphor_time`, `last_quantum_update_time`) prevents the
        // first post-switch frame from computing a stale `dt` that
        // teleports particles.
        for w in &mut self.flash_waves {
            w.active = false;
        }
        for p in &mut self.quantum_particles {
            p.active = false;
        }
        self.quantum_active_count = 0;
        let now = Instant::now();
        self.last_spawn_time = now;
        self.last_phosphor_time = now;
        self.last_quantum_update_time = now;
        self.semantic_invalidate = true;
        self.force_draw_everything = true;
    }

    /// Apply glitch level parameters directly at runtime.
    ///
    /// Public so `apply_custom_scene_runtime` (this file) can apply a
    /// scene-custom or base-scene's glitch-level at runtime. Idempotent in
    /// the sense that calling with the same level twice is safe (it resets
    /// glitch timing, which is a minor side effect — callers should still
    /// gate with an "if changed" check to avoid needless resets every 30s).
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
        // ME-06 (mouse-effect state leak fix): clear anomaly_zones
        // unconditionally, not just when turning glitch OFF. For
        // cinematic→monolith (both glitch_level=Subtle, on=true), the
        // previous scene's in-flight anomaly events (LuminanceSurge /
        // GlyphCorruption / PulseWave) would otherwise continue to apply
        // for up to 1.5s into the new scene — visible as leftover visual
        // corruption flicker. Cloud::reset() (Space key) already clears
        // anomaly_zones; this matches that behavior for scene switches.
        self.anomaly_zones.clear();
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
    /// simplified to a single scene-name field. The entry's `scene`
    /// is resolved via [`Cloud::apply_scene_runtime_with_cfg`], which handles
    /// both built-in scenes (fast path) and custom scenes (looks up
    /// `[scene-custom.<name>]` block, applies `base-scene` defaults first,
    /// then the block's own overrides).
    ///
    /// Called by the event loop when the ambient scheduler thread fires a
    /// phase boundary. Returns the charset preset name used (scene's or
    /// current).
    pub fn apply_ambient_entry(
        &mut self,
        entry: &crate::crystal_dragon_engine::ambient::AmbientEntry,
        current_charset_preset: &str,
        user_ranges: &[(char, char)],
        def_ascii: bool,
        cfg: &std::collections::HashMap<String, String>,
    ) -> String {
        self.apply_scene_runtime_with_cfg(
            &entry.scene,
            current_charset_preset,
            user_ranges,
            def_ascii,
            cfg,
        )
    }
}
