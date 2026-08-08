// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Core simulation engine for Cosmostrix — atmospheric rendering pipeline.
//!
//! Key systems: **DrawCtx** (read-only renderer snapshot for per-frame
//! callbacks), **DropletSpawner** (3 parallax layers, see `spawn.rs`),
//! **GhostEventScheduler** (ghost-kanji events, see `ghost_events.rs`),
//! **LivingRain** (wind-gust drift, see `living_rain.rs`).
//!
//! On color-scheme change, new droplets inherit the new palette while
//! existing droplets keep their old colors until they age out —
//! transition smoothed via Phase 8 hue-preserving chroma shader
//! (see `chroma/shaders/transition.rs`).

pub(crate) mod ecosystem;
pub(crate) mod events;
mod ghost_events;
mod living_rain;
mod monolith;
mod monolith_glyphs;
#[cfg(test)]
mod monolith_tests;
mod phosphor;
mod rain;
mod render;
mod runtime_controls;
mod scene_runtime;
mod spawn;
mod state;

#[cfg(test)]
mod tests;

pub(super) use render::{CharLoc, DrawCtx};

use std::time::{Duration, Instant};

use bitvec::prelude::BitVec;
use crossterm::style::Color;
use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
    SeedableRng,
};
use smallvec::SmallVec;

use crate::cell::Cell;
use crate::constants::*;
use crate::droplet::Droplet;
use crate::frame::Frame;
use crate::palette::{build_palette, Palette};
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorPipeline, ColorScheme, MonolithSize, ShadingMode};

use ecosystem::{
    BehaviorProfile, ColorEcosystem, EntropyDrift, ProfileParams, RendererMemory, StorytellingState,
};
use monolith::MonolithRain;
use state::{AnomalyZone, ColumnStatus, MsgChr};

use ghost_events::GhostEventScheduler;
use render::FlashWave;

#[derive(Clone, Copy, Debug)]
pub(super) struct QuantumParticle {
    pub active: bool,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub birth: Instant,
    pub ch: char,
    /// Palette body color at spawn (crossfade on palette switch).
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[allow(private_interfaces, clippy::struct_excessive_bools)]
pub struct Cloud {
    pub(super) lines: u16,
    pub(super) cols: u16,

    pub(super) palette: Palette,
    pub(super) color_mode: ColorMode,
    /// v30.3: cached `ColorPipeline::detect(color_mode)`.
    pub(super) color_pipeline: ColorPipeline,
    pub(super) rain_style: RainStyle,
    monolith_size: MonolithSize,

    pub(super) shading_distance: bool,
    pub(super) bold_mode: BoldMode,

    pub(super) async_mode: bool,
    pub(super) raining: bool,
    pub(super) pause: bool,

    pub(super) droplet_density: f32,
    pub(super) monolith_density_map: Option<&'static [f64]>,
    pub(super) droplets_per_sec: f32,
    pub(super) chars_per_sec: f32,

    pub(super) glitchy: bool,
    pub(super) glitch_pct: f32,
    pub(super) glitch_low_ms: u16,
    pub(super) glitch_high_ms: u16,

    pub(super) short_pct: f32,
    pub(super) die_early_pct: f32,
    pub(super) linger_low_ms: u16,
    pub(super) linger_high_ms: u16,

    pub(super) max_droplets_per_column: u8,

    pub(super) droplets: Vec<Droplet>,
    pub(super) monolith_rain: MonolithRain,

    pub(super) chars: Vec<char>,
    pub(super) char_pool: Vec<char>,
    pub(super) previous_char_pool: Vec<char>,
    pub(super) char_pool_is_binary: bool,
    pub(super) charset_transition_start: Option<Instant>,
    pub(super) glitch_pool: Vec<char>,
    pub(super) glitch_pool_idx: usize,

    pub(super) glitch_map: BitVec,
    pub(super) color_map: Vec<u8>,

    pub(super) edge_fade_lut: Vec<f32>,

    /// Phase D (hot-path): precomputed per-column hue-coherence perturbation
    /// LUT. Built once per frame in `rain_at` from the time phase
    /// (`COLUMN_COHERENCE_FREQ` rad/s). Read by index in the shader hot path
    /// (`ShaderCtx::column_coherence_lut[col]`) instead of calling
    /// `column_coherence_perturbation(phase, col)` per cell. Saves
    /// ~65-130M cycles/sec at 60 FPS on a 200-col viewport.
    /// Stored on Cloud (not built fresh each frame) to avoid per-frame heap
    /// allocation. Length kept in sync with `cols` via the resize check in
    /// `rain_at`.
    pub(super) column_coherence_lut: Vec<i32>,

    pub(super) droplet_free_list: Vec<usize>,

    pub(super) col_stat: Vec<ColumnStatus>,

    pub(super) mt: StdRng,

    pub(super) rand_chance: Uniform<f32>,
    pub(super) rand_line: Uniform<u16>,
    pub(super) rand_cpidx: Uniform<u16>,
    pub(super) rand_len: Uniform<u16>,
    pub(super) rand_col: Uniform<u16>,
    pub(super) rand_glitch_ms: Uniform<u16>,
    pub(super) rand_linger_ms: Uniform<u16>,
    pub(super) rand_speed: Uniform<f32>,

    pub(super) last_glitch_time: Instant,
    pub(super) next_glitch_time: Instant,
    pub(super) last_spawn_time: Instant,
    /// v30 Hinnant: process anchor captured at `Cloud::new()`, inherited
    /// across live-reload. Replaces `now.elapsed()` in `rain_at()`.
    pub(super) start_anchor: Instant,
    pub(super) spawn_remainder: f32,
    pub(super) pause_time: Option<Instant>,

    pub(super) resume_blend: f32,
    pub(super) resume_start: Option<Instant>,
    /// Starting resume_blend for the acceleration ramp (triple-tap 'p').
    pub(super) resume_blend_start: f32,

    pub(super) pause_start: Option<Instant>,

    pub(super) force_draw_everything: bool,

    pub(super) semantic_invalidate: bool,

    pub(super) frames_since_full_redraw: u64,

    /// P4: frame counter for stuck-cell sweep (gated on enable_stuck_cell_sweep).
    pub(super) frames_since_stuck_sweep: u64,

    pub(super) perf_pressure: f32,
    pub(super) max_sim_delta: Duration,

    pub(super) shading_mode: ShadingMode,

    pub(super) message: Vec<MsgChr>,
    pub(super) message_text: Option<String>,
    pub(super) message_border: bool,
    pub(super) message_start_time: Option<Instant>,
    pub(super) color_scheme: ColorScheme,
    pub(super) default_background: bool,
    scene_name: String,

    pub(super) palette_table: [Option<Palette>; MAX_PALETTE_SLOTS],

    pub(super) active_palette_slot: u8,

    pub(super) transition_start: Option<Instant>,

    pub(super) column_palette_slot: Vec<u8>,

    pub mouse_col: u16,

    pub mouse_line: u16,

    pub mouse_enabled: bool,

    pub(super) flash_waves: [FlashWave; MOUSE_FLASH_POOL_SIZE],

    pub(super) quantum_particles: Vec<QuantumParticle>,
    /// Active quantum particle count (incremental, O(1) early-out).
    pub(super) quantum_active_count: usize,

    pub(super) last_reseed_time: Instant,

    pub(super) phosphor: Vec<u8>,
    pub(super) phosphor_base_fg: Vec<Option<Color>>,
    pub(super) phosphor_base_ch: Vec<char>,
    pub(super) phosphor_layer: Vec<u8>,
    pub(super) phosphor_fresh: BitVec,
    pub(super) phosphor_in_active: BitVec,
    pub(super) last_phosphor_time: Instant,
    pub(super) last_quantum_update_time: Instant,
    pub(super) phosphor_active: SmallVec<[usize; 256]>,
    pub(super) phosphor_last_fresh: SmallVec<[usize; 256]>,
    pub(super) crt_vignette_candidates: Vec<(u16, u16, f32)>, // T1.1-real: hoisted scratch (was per-frame SmallVec)

    pub(super) anomaly_zones: Vec<AnomalyZone>,

    pub(super) profile: BehaviorProfile,
    pub(super) profile_current: ProfileParams,
    pub(super) profile_target: ProfileParams,
    pub(super) profile_transition_start: Option<Instant>,

    pub(super) color_ecosystem: ColorEcosystem,
    pub(super) entropy_drift: EntropyDrift,
    pub(super) memory: RendererMemory,
    pub(super) storytelling: StorytellingState,

    pub(super) glyph_entry_time: Option<Instant>,

    pub(super) auto_color_drift: bool,
    /// v30 Bug #4: true when --colors-custom active → suppress palette drift.
    pub(super) custom_palette_active: bool,
    /// v30 Bug #5: color_tune stored on Cloud so set_color_scheme re-applies it.
    pub(super) color_tune: crate::color_tune::ColorTune,

    pub(super) event_manager: GhostEventScheduler,

    pub(super) gust: living_rain::GustState,

    pub(super) last_sim_ms: f64,
    pub(super) last_render_ms: f64,
    pub(super) enable_component_timing: bool,
    /// T1.1: gate for stuck-cell sweep (default true; benchmark sets false).
    pub(crate) enable_stuck_cell_sweep: bool,
    /// Gate diagnostic stderr logs. Set from cfg.verbose.
    pub(super) verbose: bool,
    /// Total stuck cells cleared across all sweeps.
    pub(super) stuck_cells_cleared_total: u64,
    /// Total sweeps that found at least one stuck cell.
    pub(super) stuck_sweeps_with_clears: u64,
}

impl Cloud {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        color_mode: ColorMode,
        shading_mode: ShadingMode,
        bold_mode: BoldMode,
        async_mode: bool,
        default_background: bool,
        color_scheme: ColorScheme,
        rain_style: RainStyle,
    ) -> Self {
        let now = Instant::now();
        let mt = StdRng::seed_from_u64(RNG_INITIAL_SEED);

        Self {
            lines: 25,
            cols: 80,
            palette: build_palette(color_scheme, color_mode, default_background),
            color_mode,
            color_pipeline: ColorPipeline::detect(color_mode),
            rain_style,
            monolith_size: MonolithSize::Normal,
            shading_distance: matches!(shading_mode, ShadingMode::DistanceFromHead),
            bold_mode,
            async_mode,
            raining: true,
            pause: false,
            droplet_density: 1.0,
            monolith_density_map: None,
            droplets_per_sec: 5.0,
            chars_per_sec: 8.0,
            glitchy: true,
            glitch_pct: 0.1,
            glitch_low_ms: 300,
            glitch_high_ms: 400,
            short_pct: 0.5,
            die_early_pct: 0.3333333,
            linger_low_ms: 1,
            linger_high_ms: 3000,
            max_droplets_per_column: 3,
            droplets: Vec::new(),
            monolith_rain: MonolithRain::new(),
            chars: Vec::new(),
            char_pool: Vec::new(),
            previous_char_pool: Vec::new(),
            char_pool_is_binary: false,
            charset_transition_start: None,
            glitch_pool: Vec::new(),
            glitch_pool_idx: 0,
            glitch_map: BitVec::new(),
            color_map: Vec::new(),
            edge_fade_lut: Vec::new(),
            // Phase D: preallocated — rain_at resizes+fills per frame.
            column_coherence_lut: Vec::new(),
            droplet_free_list: Vec::new(),
            col_stat: Vec::new(),
            mt,
            rand_chance: Uniform::new(0.0, 1.0).expect("rand_chance: [0,1) always valid"),
            rand_line: Uniform::new_inclusive(0, 23).expect("rand_line: [0,23] always valid"),
            rand_cpidx: Uniform::new_inclusive(0, MAX_CHAR_POOL_IDX)
                .expect("rand_cpidx: [0,2047] always valid"),
            rand_len: Uniform::new_inclusive(1, 23).expect("rand_len: [1,23] always valid"),
            rand_col: Uniform::new_inclusive(0, 79).expect("rand_col: [0,79] always valid"),
            rand_glitch_ms: Uniform::new_inclusive(300, 400)
                .expect("rand_glitch_ms: [300,400] always valid"),
            rand_linger_ms: Uniform::new_inclusive(1, 3000)
                .expect("rand_linger_ms: [1,3000] always valid"),
            rand_speed: Uniform::new_inclusive(0.3333333, 1.0)
                .expect("rand_speed: [0.33,1.0] always valid"),
            last_glitch_time: now,
            next_glitch_time: now + Duration::from_millis(300),
            last_spawn_time: now,
            start_anchor: now,
            spawn_remainder: 0.0,
            pause_time: None,
            resume_blend: 1.0,
            resume_start: None,
            resume_blend_start: 0.0,
            pause_start: None,
            force_draw_everything: false,
            semantic_invalidate: false,
            frames_since_full_redraw: 0,
            frames_since_stuck_sweep: 0,
            perf_pressure: 0.0,
            max_sim_delta: Duration::from_millis(0),
            shading_mode,
            message: Vec::new(),
            message_text: None,
            message_border: false,
            message_start_time: None,
            color_scheme,
            default_background,
            scene_name: String::new(),
            palette_table: [None, None, None, None],
            active_palette_slot: 0,
            transition_start: None,
            column_palette_slot: Vec::new(),
            mouse_col: u16::MAX,
            mouse_line: u16::MAX,
            mouse_enabled: false,
            flash_waves: [FlashWave {
                active: false,
                col: u16::MAX,
                line: u16::MAX,
                birth: now,
            }; MOUSE_FLASH_POOL_SIZE],
            quantum_particles: vec![
                QuantumParticle {
                    active: false,
                    x: 0.0,
                    y: 0.0,
                    vx: 0.0,
                    vy: 0.0,
                    birth: now,
                    ch: '*',
                    r: QUANTUM_BRAND_PURPLE_R,
                    g: QUANTUM_BRAND_PURPLE_G,
                    b: QUANTUM_BRAND_PURPLE_B,
                };
                QUANTUM_RIPPLE_POOL_SIZE
            ],
            quantum_active_count: 0,
            last_reseed_time: now,
            phosphor: Vec::new(),
            phosphor_base_fg: Vec::new(),
            phosphor_base_ch: Vec::new(),
            phosphor_layer: Vec::new(),
            phosphor_fresh: BitVec::new(),
            phosphor_in_active: BitVec::new(),
            phosphor_active: SmallVec::new(),
            phosphor_last_fresh: SmallVec::new(),
            crt_vignette_candidates: Vec::with_capacity(128),
            last_phosphor_time: now,
            last_quantum_update_time: now,
            anomaly_zones: Vec::new(),
            profile: BehaviorProfile::Monolith,
            profile_current: BehaviorProfile::Monolith.params(),
            profile_target: BehaviorProfile::Monolith.params(),
            profile_transition_start: None,
            color_ecosystem: ColorEcosystem::new(now),
            entropy_drift: EntropyDrift::new(now),
            memory: RendererMemory::new(now),
            storytelling: StorytellingState::new(now),
            glyph_entry_time: None,
            auto_color_drift: AUTO_COLOR_DRIFT_DEFAULT,
            // v30 strengthen: overridden in app.rs create_cloud.
            custom_palette_active: false,
            color_tune: crate::color_tune::ColorTune::IDENTITY,
            event_manager: GhostEventScheduler::new(now),
            gust: living_rain::GustState::new(now),
            last_sim_ms: 0.0,
            last_render_ms: 0.0,
            enable_component_timing: false,
            enable_stuck_cell_sweep: true, // T1.1: default on; benchmark disables via setter
            verbose: false,
            stuck_cells_cleared_total: 0,
            stuck_sweeps_with_clears: 0,
        }
    }

    pub fn set_message(&mut self, msg: &str) {
        self.message_text = Some(msg.to_string());
        // v25: delay typewriter 6s so the intro finishes first.
        self.message_start_time = Some(Instant::now() + Duration::from_secs(6));
        self.reset_message();
        self.force_draw_everything = true;
    }

    pub fn restart_message_typewriter(&mut self) {
        if self.message_text.is_some() {
            self.message_start_time = Some(Instant::now() + Duration::from_secs(6));
            self.force_draw_everything = true;
        }
    }

    pub fn set_message_border(&mut self, on: bool) {
        self.message_border = on;
        if self.message_text.is_some() {
            self.reset_message();
            self.force_draw_everything = true;
        }
    }

    pub fn enable_events(&mut self) {
        self.event_manager.enable_events();
    }

    pub fn set_mouse_position(&mut self, col: u16, line: u16) {
        self.mouse_col = col;
        self.mouse_line = line;
    }

    pub fn set_mouse_click(&mut self, col: u16, line: u16) {
        // v30 fix: bounded pool. Old design reset any in-flight wave on every
        // click. New design mirrors spawn_quantum_ripple: first inactive slot,
        // or evict OLDEST (smallest birth) if all active.
        let now = Instant::now();
        let mut slot = None;
        let mut oldest = (0usize, Instant::now());
        for (i, w) in self.flash_waves.iter_mut().enumerate() {
            if !w.active {
                slot = Some(i);
                break;
            }
            if i == 0 || w.birth < oldest.1 {
                oldest = (i, w.birth);
            }
        }
        let s = slot.unwrap_or(oldest.0);
        self.flash_waves[s] = FlashWave {
            active: true,
            col,
            line,
            birth: now,
        };
        self.spawn_quantum_ripple(col, line);
    }

    #[must_use]
    pub fn color_scheme(&self) -> ColorScheme {
        self.color_scheme
    }

    #[must_use]
    pub fn rain_style(&self) -> RainStyle {
        self.rain_style
    }

    pub fn profile(&self) -> BehaviorProfile {
        self.profile
    }

    pub fn profile_name(&self) -> &'static str {
        self.profile.name()
    }

    #[must_use]
    pub fn droplet_count(&self) -> usize {
        self.droplets.len()
    }

    #[must_use]
    pub fn active_droplet_count(&self) -> usize {
        if matches!(self.rain_style, RainStyle::Monolith) {
            self.monolith_rain.active_count()
        } else {
            self.droplets.iter().filter(|d| d.is_alive).count()
        }
    }

    #[must_use]
    pub fn last_sim_ms(&self) -> f64 {
        self.last_sim_ms
    }

    #[must_use]
    pub fn last_render_ms(&self) -> f64 {
        self.last_render_ms
    }

    pub fn set_component_timing(&mut self, enabled: bool) {
        self.enable_component_timing = enabled;
    }

    /// Gate verbose cloud logging (stuck-cell-sweep). Set from cfg.verbose.
    pub fn set_verbose(&mut self, verbose: bool) {
        self.verbose = verbose;
    }

    /// Cumulative stuck-cell sweep stats for verbose summary.
    #[must_use]
    pub fn stuck_cell_stats(&self) -> (u64, u64) {
        (
            self.stuck_cells_cleared_total,
            self.stuck_sweeps_with_clears,
        )
    }

    pub fn set_monolith_density_map(&mut self, map: Option<&'static [f64]>) {
        self.monolith_density_map = map;
    }

    /// Phase D Bug #9: carry color_ecosystem + entropy_drift across live-reload
    /// (prevents brightness discontinuity when config is edited mid-session).
    /// v30: also carries `start_anchor` so time-varying phases stay continuous.
    pub fn inherit_ecosystem_state(&mut self, other: &Cloud) {
        self.color_ecosystem = other.color_ecosystem;
        self.entropy_drift = other.entropy_drift;
        self.start_anchor = other.start_anchor;
    }
    #[must_use]
    pub fn active_scene(&self) -> &str {
        &self.scene_name
    }
    #[must_use]
    pub fn hud_colors(&self) -> &[crossterm::style::Color] {
        &self.palette.colors
    }
    pub fn toggle_pause(&mut self) -> bool {
        // BRANCH 1: mid-deceleration → abort & resume. Capture current
        // resume_blend as ramp start (audit §8.4 — interpolate 0.4→1.0).
        if self.pause_start.is_some() {
            self.pause_start = None;
            self.pause = false;
            self.pause_time = None;
            self.resume_blend_start = self.resume_blend.max(0.1);
            self.resume_blend = self.resume_blend_start;
            self.resume_start = Some(Instant::now());
            return true;
        }
        // BRANCH 2: fully paused → unpause. Shift every last_*_time
        // forward by pause duration + visual-subsystem timestamps (§8.5).
        if self.pause {
            self.pause = false;
            if let Some(pt) = self.pause_time.take() {
                let now = Instant::now();
                let elapsed = now.saturating_duration_since(pt);
                self.last_spawn_time = now;
                self.spawn_remainder = 0.0;
                for d in &mut self.droplets {
                    if d.is_alive {
                        d.increment_time(elapsed);
                        d.last_time = Some(now);
                        // v30.2: randomize advance_remainder on resume (was 0).
                        // Wiping to 0 made all droplets cross the 1.0 row
                        // threshold in lockstep → asynchronous "loncat" pops
                        // during slow resume. Random jitter spreads the pops,
                        // matching apply_phase_jitter's per-droplet phase.
                        d.advance_remainder = self.rand_chance.sample(&mut self.mt);
                    }
                }
                // v30.2 §H10: shift monolith streams' last_time forward by
                // pause duration (was "safe by accident" via resume_blend=0).
                self.monolith_rain.shift_active_streams_last_time(elapsed);
                self.last_phosphor_time += elapsed;
                self.last_quantum_update_time += elapsed;
                self.last_glitch_time += elapsed;
                self.next_glitch_time += elapsed;
                self.last_reseed_time += elapsed;
                self.color_ecosystem.shift_in_time(elapsed);
                self.entropy_drift.last_tick += elapsed;
                self.memory.last_sample += elapsed;
                self.storytelling.last_tick += elapsed;
                if let Some(ref mut cd) = self.storytelling.cooldown_until {
                    *cd += elapsed;
                }
                if let Some(ref mut ts) = self.transition_start {
                    *ts += elapsed;
                }
                if let Some(ref mut pt) = self.profile_transition_start {
                    *pt += elapsed;
                }
                if let Some(ref mut ct) = self.charset_transition_start {
                    *ct += elapsed;
                }
                // §8.5: shift visual-subsystem timestamps so they don't
                // skip ahead on resume.
                if let Some(ref mut mt) = self.message_start_time {
                    *mt += elapsed;
                }
                if let Some(ref mut ge) = self.glyph_entry_time {
                    *ge += elapsed;
                }
                // v30 fix: shift ALL active flash wave births (was single slot).
                for w in &mut self.flash_waves {
                    if w.active {
                        w.birth += elapsed;
                    }
                }
                // v30 fix (MOUSE_EFFECTS_AUDIT.md bug fix): shift active
                // quantum particle births too. Without this, particles spawned
                // before pause instantly expire on unpause (their age = now -
                // birth includes the pause duration, exceeding their 0.8s
                // lifetime). Flash waves survived correctly because their
                // birth was shifted above — this makes particles consistent.
                for p in &mut self.quantum_particles {
                    if p.active {
                        p.birth += elapsed;
                    }
                }
                self.resume_blend_start = 0.0;
                self.resume_blend = 0.0;
                self.resume_start = Some(now);
                true
            } else {
                true
            }
        } else {
            // BRANCH 3: running → start deceleration. Clear stale
            // resume_start (audit §8.3 — rapid triple-tap state hygiene).
            self.pause_start = Some(Instant::now());
            self.resume_start = None;
            true
        }
    }

    #[cfg(test)]
    pub fn is_force_draw_everything(&self) -> bool {
        self.force_draw_everything
    }

    #[cfg(test)]
    pub fn is_semantic_invalidate(&self) -> bool {
        self.semantic_invalidate
    }

    #[cfg(test)]
    pub fn clear_redraw_flags_for_test(&mut self) {
        self.semantic_invalidate = false;
        self.force_draw_everything = false;
    }

    pub(super) fn reset_message(&mut self) {
        let Some(text) = self.message_text.as_deref() else {
            return;
        };

        let pad_x: u16 = 2;
        let pad_y: u16 = 1;

        let border: u16 = if self.message_border { 1 } else { 0 };

        let min_box_w = (2u16.saturating_mul(border))
            .saturating_add(2u16.saturating_mul(pad_x))
            .max(1);
        let min_box_h = (2u16.saturating_mul(border))
            .saturating_add(2u16.saturating_mul(pad_y))
            .max(1);
        if self.cols < min_box_w || self.lines < min_box_h {
            self.message.clear();
            return;
        }

        let max_content_w = self
            .cols
            .saturating_sub(2u16.saturating_mul(border))
            .saturating_sub(2u16.saturating_mul(pad_x))
            .max(1);
        let max_content_h = self
            .lines
            .saturating_sub(2u16.saturating_mul(border))
            .saturating_sub(2u16.saturating_mul(pad_y))
            .max(1);

        let mut content_lines: Vec<Vec<char>> = Vec::new();
        for raw_line in text.split('\n') {
            if content_lines.len() as u16 >= max_content_h {
                break;
            }

            let mut buf: Vec<char> = Vec::new();
            for ch in raw_line.chars() {
                if buf.len() >= max_content_w as usize {
                    content_lines.push(std::mem::take(&mut buf));
                    if content_lines.len() as u16 >= max_content_h {
                        break;
                    }
                }
                buf.push(ch);
            }

            if content_lines.len() as u16 >= max_content_h {
                break;
            }

            if raw_line.is_empty() {
                content_lines.push(Vec::new());
            } else if !buf.is_empty() {
                content_lines.push(buf);
            }
        }

        if content_lines.is_empty() {
            content_lines.push(Vec::new());
        }

        let mut content_w: u16 = 1;
        for l in &content_lines {
            content_w = content_w.max(l.len().min(max_content_w as usize) as u16);
        }
        let content_h: u16 = (content_lines.len().min(max_content_h as usize)) as u16;

        let box_w = content_w
            .saturating_add(2u16.saturating_mul(border))
            .saturating_add(2u16.saturating_mul(pad_x));
        let box_h = content_h
            .saturating_add(2u16.saturating_mul(border))
            .saturating_add(2u16.saturating_mul(pad_y));

        let start_col = (self.cols / 2).saturating_sub(box_w / 2);
        let start_line = (self.lines / 2).saturating_sub(box_h / 2);

        self.message.clear();

        for y in 0..box_h {
            let line = start_line.saturating_add(y);
            if line >= self.lines {
                continue;
            }

            for x in 0..box_w {
                let col = start_col.saturating_add(x);
                if col >= self.cols {
                    continue;
                }

                let mut ch = ' ';
                if border == 1 {
                    let is_top = y == 0;
                    let is_bottom = y + 1 == box_h;
                    let is_left = x == 0;
                    let is_right = x + 1 == box_w;
                    // v25 cinematic border: rounded corners + smooth lines.
                    ch = if is_top && is_left {
                        '╭'
                    } else if is_top && is_right {
                        '╮'
                    } else if is_bottom && is_left {
                        '╰'
                    } else if is_bottom && is_right {
                        '╯'
                    } else if is_top || is_bottom {
                        '─'
                    } else if is_left || is_right {
                        '│'
                    } else {
                        ' '
                    };
                }

                {
                    let content_start_y = border.saturating_add(pad_y);
                    let content_start_x = border.saturating_add(pad_x);

                    if y >= content_start_y
                        && y < content_start_y.saturating_add(content_h)
                        && x >= content_start_x
                        && x < content_start_x.saturating_add(content_w)
                    {
                        let inner_y = y - content_start_y;
                        let inner_x = x - content_start_x;

                        let li = inner_y as usize;
                        if let Some(line_chars) = content_lines.get(li) {
                            let line_len = line_chars.len().min(content_w as usize);
                            let left_pad = (content_w as usize)
                                .saturating_sub(line_len)
                                .saturating_div(2);
                            let ix = inner_x as usize;
                            if ix >= left_pad && ix < left_pad + line_len {
                                ch = line_chars[ix - left_pad];
                            }
                        }
                    }
                }

                self.message.push(MsgChr { line, col, val: ch });
            }
        }
    }

    fn draw_message(&self, frame: &mut Frame) {
        let bg = self.palette.bg;
        let fg = if self.color_mode == ColorMode::Mono {
            None
        } else {
            self.palette.colors.last().copied()
        };

        // Count total text (content) chars and border chars.
        let total_text: usize = self
            .message
            .iter()
            .filter(|mc| !is_border_char(mc.val))
            .count();
        let total_border: usize = self
            .message
            .iter()
            .filter(|mc| is_border_char(mc.val) && mc.val != ' ')
            .count();

        // v30 Hinnant: hoist start.elapsed() above the per-cell loop below
        // (was 1 syscall per revealed content cell, 50-200×/frame).
        let message_elapsed_ms: Option<usize> = self
            .message_start_time
            .map(|start| start.elapsed().as_millis() as usize);

        let reveal_count = if let Some(elapsed_ms) = message_elapsed_ms {
            let count = (elapsed_ms / 80).max(1);
            count.min(total_text.max(1))
        } else {
            usize::MAX
        };

        // v25 progressive border: border cells revealed clockwise,
        // lagging behind text reveal (cinematic effect).
        let text_progress = if total_text > 0 {
            reveal_count as f32 / total_text as f32
        } else {
            1.0
        };
        // Border progress = text_progress ^ 1.5 (ease-out).
        let border_progress = text_progress.powf(1.5);
        let border_show = (border_progress * total_border as f32).floor() as usize;

        let border_order = build_border_order(&self.message);

        let mut visible_border: std::collections::HashSet<usize> = std::collections::HashSet::new();
        for &idx in border_order.iter().take(border_show) {
            visible_border.insert(idx);
        }

        const FADE_IN_MS: usize = 100;
        const FADE_IN_START: f32 = 0.30;

        let mut content_idx = 0usize;
        for (idx, mc) in self.message.iter().enumerate() {
            let is_content = !is_border_char(mc.val);
            let is_visible_border = mc.val != ' ' && visible_border.contains(&idx);

            let (ch, cell_fg) = if is_content {
                if content_idx < reveal_count {
                    content_idx += 1;
                    let cell_fg =
                        if let (Some(elapsed_ms), Some(base_fg)) = (message_elapsed_ms, fg) {
                            let reveal_time_ms = content_idx * 80;
                            let age_ms = elapsed_ms.saturating_sub(reveal_time_ms);
                            if age_ms >= FADE_IN_MS {
                                fg
                            } else {
                                let progress = age_ms as f32 / FADE_IN_MS as f32;
                                let factor = FADE_IN_START + (1.0 - FADE_IN_START) * progress;
                                if let Some((r, g, b)) = crate::palette::decode_color(base_fg) {
                                    Some(crate::palette::apply_brightness_rgb(r, g, b, factor))
                                } else {
                                    fg
                                }
                            }
                        } else {
                            fg
                        };
                    (mc.val, cell_fg)
                } else {
                    (' ', None)
                }
            } else if is_visible_border {
                (mc.val, fg)
            } else {
                (' ', None)
            };

            frame.set_force(
                mc.col,
                mc.line,
                Cell {
                    ch,
                    fg: cell_fg,
                    bg,
                    bold: ch != ' ' && self.bold_mode != BoldMode::Off,
                },
            );
        }
    }
}

/// Check if a character is a border character (not content).
/// v25: includes rounded box-drawing chars.
#[inline]
fn is_border_char(ch: char) -> bool {
    matches!(
        ch,
        ' ' | '+' | '-' | '|' | '╭' | '╮' | '╰' | '╯' | '─' | '│'
    )
}

/// Build clockwise-ordered list of border cell indices: top-left → top →
/// top-right → right → bottom-right → bottom → bottom-left → left.
fn build_border_order(message: &[MsgChr]) -> Vec<usize> {
    if message.is_empty() {
        return Vec::new();
    }
    // Find bounding box of border cells.
    let mut min_line = u16::MAX;
    let mut max_line = 0u16;
    let mut min_col = u16::MAX;
    let mut max_col = 0u16;
    for mc in message {
        if is_border_char(mc.val) && mc.val != ' ' {
            min_line = min_line.min(mc.line);
            max_line = max_line.max(mc.line);
            min_col = min_col.min(mc.col);
            max_col = max_col.max(mc.col);
        }
    }
    if min_line == u16::MAX {
        return Vec::new();
    }

    // Collect border cells in clockwise order.
    let mut order: Vec<usize> = Vec::new();
    // 1. Top edge: left→right (includes corners)
    for col in min_col..=max_col {
        for (idx, mc) in message.iter().enumerate() {
            if mc.line == min_line && mc.col == col && is_border_char(mc.val) && mc.val != ' ' {
                order.push(idx);
            }
        }
    }
    // 2. Right edge: top+1 to bottom-1 (corners already added)
    for line in (min_line + 1)..max_line {
        for (idx, mc) in message.iter().enumerate() {
            if mc.line == line && mc.col == max_col && is_border_char(mc.val) && mc.val != ' ' {
                order.push(idx);
            }
        }
    }
    // 3. Bottom edge: left→right (includes corners)
    for col in min_col..=max_col {
        for (idx, mc) in message.iter().enumerate() {
            if mc.line == max_line && mc.col == col && is_border_char(mc.val) && mc.val != ' ' {
                order.push(idx);
            }
        }
    }
    // 4. Left edge: bottom-1 to top+1 (corners already added)
    for line in ((min_line + 1)..max_line).rev() {
        for (idx, mc) in message.iter().enumerate() {
            if mc.line == line && mc.col == min_col && is_border_char(mc.val) && mc.val != ' ' {
                order.push(idx);
            }
        }
    }
    order
}
