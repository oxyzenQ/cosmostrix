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
        // v80.0.0-beta.2 HUD honesty: the tracked palette name is cleared
        // with the flag so the `clr:` HUD line falls back to the scheme
        // name the moment the custom palette stops being active.
        self.custom_palette_active = false;
        self.custom_palette_name = None;
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
    ///
    /// v80.0.0-beta.2 HUD honesty: `name` is the user-facing palette name
    /// surfaced on the `clr:` HUD line (tracked on the Cloud so runtime
    /// activation paths — ambient fire, scene-runtime custom-scene
    /// switch, live-reload rebuild — are as visible as the startup
    /// `--colors-custom` path). Callers without a meaningful name (unit
    /// tests constructing ad-hoc palettes) pass None.
    pub fn set_palette(&mut self, name: Option<&str>, palette: crate::palette::Palette) {
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
        self.custom_palette_name = name.map(str::to_string);
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
        } else if matches!(self.rain_style, RainStyle::Vortex) {
            self.vortex_rain.clear_draw_history();
            self.reset_phosphor_state();
        } else if matches!(self.rain_style, RainStyle::Dragon) {
            // NIGHT-research-5: dragon — structured-family sibling,
            // same draw-history clear on palette change.
            self.dragon_rain.clear_draw_history();
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
        // Vortex/Ripple have no monolith-size rendering dependency — the
        // field is stored for a later monolith switch (cheap no-op here).
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

    /// v50.0.0-beta.6: set terminal-aware phosphor tuning + speed multiplier.
    /// Called once at startup from event_loop after terminal detection.
    pub fn set_phosphor_tuning(&mut self, decay_mult: f32, ghost_cap: f32, speed_mult: f32) {
        self.phosphor_decay_mult = decay_mult.max(0.1);
        self.ghost_brightness_cap = ghost_cap.clamp(0.0, 1.0);
        self.speed_mult = speed_mult.max(0.1);
        // Re-derive droplet speeds with the new multiplier so existing
        // droplets immediately benefit from the terminal-aware speed.
        self.recalc_droplets_per_sec();
        self.update_droplet_speeds();
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
        } else if matches!(self.rain_style, RainStyle::Vortex) {
            self.vortex_rain.clear_draw_history();
            self.reset_phosphor_state();
        } else if matches!(self.rain_style, RainStyle::Dragon) {
            // NIGHT-research-5: dragon — structured-family sibling,
            // same draw-history clear on shading mode toggle.
            self.dragon_rain.clear_draw_history();
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
    /// Precondition: the caller must ensure that `prev_palette` is
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
    /// v80.0.0-alpha.1 (S-master-HUNT-6, owner bug 2026-09-03:
    /// "--crystal-dragon-secs 10m still drifts every 60s + burst drift in
    /// milliseconds after enabling via config"): the drift DECISION is now
    /// gated on the poll boundary — it may only run on a tick where the
    /// polling interval actually elapsed. The pre-HUNT-6 code rolled the
    /// dice EVERY FRAME (gated only by dwell), so the effective cadence was
    /// `min_dwell_secs` (60s) regardless of a slower `polling_secs`, and
    /// any live-reload rebuild (which resets `drift_active`) re-armed an
    /// immediate drift — the "flashy burst" the owner saw on every config
    /// save. With the poll gate, `polling_secs` is the true cadence
    /// governor (P=600 → one drift decision per 600s) and rebuilds cannot
    /// fire mid-cycle: the inherited `crystal_dragon_last_poll` keeps the
    /// boundary phase across reloads.
    ///
    /// First tick (`crystal_dragon_last_poll == None` — engine freshly
    /// activated): the sensor is polled for the baseline point, but NO
    /// drift decision runs — the clock is armed and the first decision
    /// comes one full `polling_secs` after activation. This kills the
    /// immediate-fire burst when crystal-dragon is enabled mid-session
    /// (config edit) and the startup flash: enabling the engine with a
    /// 10m cadence now means the first drift is visibly owed at +10m, not
    /// fired within ~100ms of the edit.
    ///
    /// The caller (rain.rs) applies the new scheme via `set_color_scheme`,
    /// which triggers the 300 ms OKLab wave transition via Chroma Dragon.
    pub(crate) fn crystal_dragon_tick(&mut self, now: std::time::Instant) -> Option<ColorScheme> {
        use crate::crystal_dragon_engine::point_system::{calc_v1_select, calc_v2_select};

        // Poll gate: a drift decision may only run on a tick where the
        // polling interval elapsed (or the arming tick — which polls the
        // sensor but returns None). `polled` is the cadence governor.
        let polled = match self.crystal_dragon_last_poll {
            None => {
                // Arming tick — engine just activated: sample the sensor
                // baseline, start the clock, decide nothing yet.
                self.crystal_dragon_last_poll = Some(now);
                self.crystal_dragon_sensor.poll(now);
                false
            }
            Some(last) => {
                let elapsed_since_poll = now.saturating_duration_since(last).as_secs_f32();
                if elapsed_since_poll >= self.crystal_dragon_control.polling_secs {
                    self.crystal_dragon_last_poll = Some(now);
                    self.crystal_dragon_sensor.poll(now);
                    true
                } else {
                    false
                }
            }
        };
        if !polled {
            return None;
        }

        // Dwell hysteresis: don't drift if we haven't dwelled in the
        // current theme long enough.
        let dwell = now
            .saturating_duration_since(self.crystal_dragon_sensor.theme_entered_at())
            .as_secs_f32();
        if dwell < self.crystal_dragon_control.min_dwell_secs {
            return None;
        }

        // Probabilistic gate: drift only with control.drift_chance probability.
        // S-master-1-v2: reads the CrystalDragonControl field (not the
        // CRYSTAL_DRAGON_DRIFT_CHANCE const) so the documented
        // "future config override" contract is real — the field is the
        // single runtime source of truth, the const only seeds the default.
        // S-master-HUNT-7: the shipped default is 1.0 (deterministic
        // boundary fire — the documented cadence contract). Fractional
        // values starve the cadence by 1/value poll cycles per drift; the
        // pre-HUNT-7 0.12 default made the engine sit silent for ~8.3
        // cadences while the HUD reported `crdr: on`. The dice stays as an
        // owner tuning surface only.
        let chance_dist = Uniform::new(0.0f32, 1.0f32).expect("chance_dist always valid");
        if chance_dist.sample(&mut self.mt) >= self.crystal_dragon_control.drift_chance {
            return None;
        }

        let current_point = self.crystal_dragon_sensor.current_point();

        // Dragon Engine v2: calc-v2 (pattern state machine with recency memory)
        // is the default. It applies a recency penalty to recently-selected
        // themes, preventing A→B→A oscillation and producing more varied
        // drift. Falls back to calc-v1 if the control config selects it.
        let new_scheme = match self.crystal_dragon_control.calc_method {
            crate::crystal_dragon_engine::crystal_dragon_control::CrystalDragonCalcMethod::Calc => {
                calc_v1_select(current_point, self.color_scheme, &mut self.mt)
            }
            crate::crystal_dragon_engine::crystal_dragon_control::CrystalDragonCalcMethod::CalcV2 => {
                calc_v2_select(
                    current_point,
                    self.color_scheme,
                    &self.crystal_dragon_drift_history,
                    &mut self.mt,
                )
            }
        };

        // Record the selection in drift history (for calc-v2 recency).
        if let Some(scheme) = new_scheme {
            self.crystal_dragon_drift_history.record(scheme);
            self.crystal_dragon_sensor.record_theme_transition(now);
        }

        new_scheme
    }
}
