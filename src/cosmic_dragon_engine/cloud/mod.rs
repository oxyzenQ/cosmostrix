// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Core simulation engine for Cosmostrix — atmospheric rendering pipeline.
//! Key systems: **DrawCtx** (read-only renderer snapshot for per-frame
//! callbacks), **DropletSpawner** (3 parallax layers, see `spawn.rs`),
//! **GhostEventScheduler** (ghost-kanji events, see `ghost_events.rs`),
//! **LivingRain** (wind-gust drift, see `living_rain.rs`).
//! On color-scheme change, new droplets inherit the new palette while
//! existing droplets keep their old colors until they age out —
//! transition smoothed via Phase 8 hue-preserving chroma shader
//! (see `chroma/shaders/transition.rs`).

mod border;
// Newly relocated from src/ root (audit M12). Re-exported as `pub(crate)`
// so the 11 existing `crate::cinematic::Foo` and
// `crate::brightness_factors::Foo` call sites continue to resolve via the
// `pub(crate) use cloud::{...};` re-export in main.rs.
pub(crate) mod brightness_factors;
pub(crate) mod cinematic;
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
mod rain_post;
mod render;
mod runtime_controls;
mod scene_runtime;
mod spawn;
mod state;

#[cfg(test)]
mod tests;

pub(crate) use render::{CharLoc, DrawCtx};

use border::is_border_char;

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
pub(crate) struct QuantumParticle {
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
    // v50 (2026-08-17) trail particles masterclass effect: ring-buffer
    // of the last QUANTUM_RIPPLE_TRAIL_LEN positions, rendered with
    // diminishing brightness + cycled color (from C7). The trail is
    // pushed every frame in apply_quantum_ripple BEFORE the position
    // update, creating a streaking "comet trail" behind the moving
    // particle.
    //
    // Layout: trail_x[i] / trail_y[i] store the i-th most recent past
    // position. trail_count is the number of valid entries (0..=TRAIL_LEN).
    // The trail is rendered oldest-first so the most recent past position
    // (closest to the current particle) is drawn LAST and overrides any
    // older trail cells with its (brighter) value.
    pub trail_x: [f32; QUANTUM_RIPPLE_TRAIL_LEN],
    pub trail_y: [f32; QUANTUM_RIPPLE_TRAIL_LEN],
    pub trail_count: u8,
}

#[allow(private_interfaces, clippy::struct_excessive_bools)]
pub struct Cloud {
    pub(crate) lines: u16,
    pub(crate) cols: u16,

    pub(crate) palette: Palette,
    pub(crate) color_mode: ColorMode,
    /// cached `ColorPipeline::detect(color_mode)`.
    pub(crate) color_pipeline: ColorPipeline,
    pub(crate) rain_style: RainStyle,
    monolith_size: MonolithSize,

    pub(crate) shading_distance: bool,
    pub(crate) bold_mode: BoldMode,

    pub(crate) async_mode: bool,
    pub(crate) raining: bool,
    pub(crate) pause: bool,

    pub(crate) droplet_density: f32,
    pub(crate) monolith_density_map: Option<&'static [f64]>,
    pub(crate) droplets_per_sec: f32,
    pub(crate) chars_per_sec: f32,

    pub(crate) glitchy: bool,
    pub(crate) glitch_pct: f32,
    pub(crate) glitch_low_ms: u16,
    pub(crate) glitch_high_ms: u16,

    pub(crate) short_pct: f32,
    pub(crate) die_early_pct: f32,
    pub(crate) linger_low_ms: u16,
    pub(crate) linger_high_ms: u16,

    pub(crate) max_droplets_per_column: u8,

    pub(crate) droplets: Vec<Droplet>,
    pub(crate) monolith_rain: MonolithRain,

    pub(crate) chars: Vec<char>,
    pub(crate) char_pool: Vec<char>,
    pub(crate) previous_char_pool: Vec<char>,
    pub(crate) char_pool_is_binary: bool,
    pub(crate) charset_transition_start: Option<Instant>,
    pub(crate) glitch_pool: Vec<char>,
    pub(crate) glitch_pool_idx: usize,

    pub(crate) glitch_map: BitVec,
    pub(crate) color_map: Vec<u8>,

    pub(crate) edge_fade_lut: Vec<f32>,
    /// Pre-baked 2D vignette factor LUT (flat: `line * cols + col`).
    /// Eliminates per-cell sqrt + smoothstep in Droplet::draw hot path.
    /// Rebuilt on resize alongside edge_fade_lut. ~27-48 KiB.
    pub(crate) vignette_lut: Vec<f32>,
    /// (cols, lines) used to build vignette_lut — skip rebuild if unchanged.
    pub(crate) vignette_lut_dims: (u16, u16),

    /// Phase D (hot-path): per-column hue-coherence perturbation LUT.
    /// Built once per frame in `rain_at`. Replaces per-cell
    /// `column_coherence_perturbation(phase, col)` (saves ~65-130M
    /// cycles/sec at 60 FPS on a 200-col viewport).
    pub(crate) column_coherence_lut: Vec<i32>,

    pub(crate) droplet_free_list: Vec<usize>,

    pub(crate) col_stat: Vec<ColumnStatus>,

    pub(crate) mt: StdRng,

    pub(crate) rand_chance: Uniform<f32>,
    pub(crate) rand_line: Uniform<u16>,
    pub(crate) rand_cpidx: Uniform<u16>,
    pub(crate) rand_len: Uniform<u16>,
    pub(crate) rand_col: Uniform<u16>,
    pub(crate) rand_glitch_ms: Uniform<u16>,
    pub(crate) rand_linger_ms: Uniform<u16>,
    pub(crate) rand_speed: Uniform<f32>,

    pub(crate) last_glitch_time: Instant,
    pub(crate) next_glitch_time: Instant,
    pub(crate) last_spawn_time: Instant,
    /// v30 Hinnant: process anchor captured at `Cloud::new()`, inherited
    /// across live-reload. Replaces `now.elapsed()` in `rain_at()`.
    pub(crate) start_anchor: Instant,
    pub(crate) spawn_remainder: f32,
    pub(crate) pause_time: Option<Instant>,
    pub(crate) resume_blend: f32,
    pub(crate) resume_start: Option<Instant>,
    /// Starting resume_blend for the acceleration ramp (triple-tap 'p').
    pub(crate) resume_blend_start: f32,
    pub(crate) pause_start: Option<Instant>,
    pub(crate) force_draw_everything: bool,
    pub(crate) semantic_invalidate: bool,
    pub(crate) frames_since_full_redraw: u64,

    /// P4: frame counter for stuck-cell sweep (gated on enable_stuck_cell_sweep).
    pub(crate) frames_since_stuck_sweep: u64,
    pub(crate) perf_pressure: f32,
    /// AB-11: aggressive throttle flag (steeper spawn-scale, no glitches).
    pub(crate) aggressive_throttle: bool,
    /// M1: hysteresis state for phosphor decay skip (prevents strobing).
    pub(crate) phosphor_skipped: bool,
    pub(crate) max_sim_delta: Duration,

    pub(crate) shading_mode: ShadingMode,

    pub(crate) message: Vec<MsgChr>,
    pub(crate) message_text: Option<String>,
    pub(crate) message_border: bool,
    pub(crate) message_start_time: Option<Instant>,
    /// BN-01/02 (Dragon Hunt v3): hoisted clockwise border-cell index list.
    /// Rebuilt only in `reset_message` (rare — once per `--message` invocation
    /// or border toggle). `draw_message` borrows this instead of calling
    /// `build_border_order` per frame (was O((W+H)×N) per frame; now O(1) borrow).
    pub(crate) border_order: Vec<usize>,
    pub(crate) color_scheme: ColorScheme,
    pub(crate) default_background: bool,
    scene_name: String,

    pub(crate) palette_table: [Option<Palette>; MAX_PALETTE_SLOTS],

    pub(crate) active_palette_slot: u8,

    pub(crate) transition_start: Option<Instant>,

    pub(crate) column_palette_slot: Vec<u8>,

    pub mouse_col: u16,

    pub mouse_line: u16,

    pub mouse_enabled: bool,

    pub(crate) flash_waves: [FlashWave; MOUSE_FLASH_POOL_SIZE],

    pub(crate) quantum_particles: Vec<QuantumParticle>,
    /// Active quantum particle count (incremental, O(1) early-out).
    pub(crate) quantum_active_count: usize,

    pub(crate) last_reseed_time: Instant,

    pub(crate) phosphor: Vec<u8>,
    pub(crate) phosphor_base_fg: Vec<Option<Color>>,
    pub(crate) phosphor_base_ch: Vec<char>,
    pub(crate) phosphor_layer: Vec<u8>,
    pub(crate) phosphor_fresh: BitVec,
    pub(crate) phosphor_in_active: BitVec,
    pub(crate) last_phosphor_time: Instant,
    pub(crate) last_quantum_update_time: Instant,
    pub(crate) phosphor_active: SmallVec<[usize; 256]>,
    pub(crate) phosphor_last_fresh: SmallVec<[usize; 256]>,
    pub(crate) crt_vignette_candidates: Vec<(u16, u16, f32)>, // T1.1-real: hoisted scratch (was per-frame SmallVec)

    pub(crate) anomaly_zones: Vec<AnomalyZone>,

    // Profile identity — currently always Monolith. Retained for future
    // profile selector (Void, Neural, etc.) which will read this field.
    #[allow(dead_code)]
    pub(crate) profile: BehaviorProfile,
    pub(crate) profile_current: ProfileParams,
    pub(crate) profile_target: ProfileParams,
    pub(crate) profile_transition_start: Option<Instant>,

    pub(crate) color_ecosystem: ColorEcosystem,
    pub(crate) entropy_drift: EntropyDrift,
    pub(crate) memory: RendererMemory,
    pub(crate) storytelling: StorytellingState,

    pub(crate) glyph_entry_time: Option<Instant>,

    /// Crystal Dragon Engine: ambient intelligence for palette drift.
    /// Point-based temperature group system (Cold/Medium/Hot) +
    /// calc-v1 probabilistic weighted theme selection.
    pub(crate) crystal_dragon: bool,
    /// Crystal Dragon sensor state (CPU/CLOCK polling + point tracking).
    pub(crate) crystal_dragon_sensor: crate::crystal_dragon_engine::CrystalDragonSensor,
    /// Crystal Dragon control config (polling interval, drift chance, etc.).
    pub(crate) crystal_dragon_control: crate::crystal_dragon_engine::CrystalDragonControl,
    /// Last Crystal Dragon poll timestamp. None until first poll.
    pub(crate) crystal_dragon_last_poll: Option<std::time::Instant>,
    /// v30 Bug #4: true when --colors-custom active → suppress palette drift.
    pub(crate) custom_palette_active: bool,
    /// v30 Bug #5: color_tune stored on Cloud so set_color_scheme re-applies it.
    pub(crate) color_tune: crate::color_tune::ColorTune,
    /// true when ambient asserted palette → suppress Crystal Dragon palette drift
    /// replacement (climate drift still runs). Cleared by `c`/`C`/`x`.
    /// See docs/audits/AMBIENT_SCHEDULER_AUDIT.md §1.3.
    pub(crate) ambient_palette_locked: bool,
    /// true when user overrode scene/color/charset (`x`/`c`/`s`/`C`/`S`)
    /// or Crystal Dragon picked new palette since last ambient fire. Prevents
    /// event-loop dedup from skipping day-boundary refire. Cleared by
    /// ambient fire (scheduler, `a` key, startup).
    pub(crate) user_override_since_ambient: bool,

    pub(crate) event_manager: GhostEventScheduler,

    pub(crate) gust: living_rain::GustState,

    pub(crate) last_sim_ms: f64,
    pub(crate) last_render_ms: f64,
    pub(crate) enable_component_timing: bool,
    /// T1.1: gate for stuck-cell sweep (default true; benchmark sets false).
    pub(crate) enable_stuck_cell_sweep: bool,
    /// Gate diagnostic stderr logs. Set from cfg.verbose.
    pub(crate) verbose: bool,
    /// Total stuck cells cleared across all sweeps.
    pub(crate) stuck_cells_cleared_total: u64,
    /// Total sweeps that found at least one stuck cell.
    pub(crate) stuck_sweeps_with_clears: u64,
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
            vignette_lut: Vec::new(),
            vignette_lut_dims: (0, 0),
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
            aggressive_throttle: false,
            phosphor_skipped: false,
            max_sim_delta: Duration::from_millis(0),
            shading_mode,
            message: Vec::new(),
            message_text: None,
            message_border: false,
            message_start_time: None,
            border_order: Vec::new(),
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
                    trail_x: [0.0; QUANTUM_RIPPLE_TRAIL_LEN],
                    trail_y: [0.0; QUANTUM_RIPPLE_TRAIL_LEN],
                    trail_count: 0,
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
            crystal_dragon: false,
            crystal_dragon_sensor: crate::crystal_dragon_engine::CrystalDragonSensor::new(
                now,
                crate::crystal_dragon_engine::CrystalDragonControl::default(),
            ),
            crystal_dragon_control: crate::crystal_dragon_engine::CrystalDragonControl::default(),
            crystal_dragon_last_poll: None,
            // v30 strengthen: overridden in app.rs create_cloud.
            custom_palette_active: false,
            color_tune: crate::color_tune::ColorTune::IDENTITY,
            // ambient-harmony flags start false (set by ambient fire,
            // cleared by user override x/c/s).
            ambient_palette_locked: false,
            user_override_since_ambient: false,
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
        // v30 fix: bounded pool. Mirrors spawn_quantum_ripple: first inactive
        // slot, or evict OLDEST (smallest birth) if all active.
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
        // Crystal Dragon sensor state survives live reload.
        self.crystal_dragon_sensor = other.crystal_dragon_sensor;
        self.crystal_dragon_control = other.crystal_dragon_control;
        self.crystal_dragon_last_poll = other.crystal_dragon_last_poll;
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
                        // randomize advance_remainder on resume (was 0,
                        // caused lockstep "loncat" pops). Jitter spreads them,
                        // matching apply_phase_jitter's per-droplet phase.
                        d.advance_remainder = self.rand_chance.sample(&mut self.mt);
                    }
                }
                // §H10: shift monolith streams' last_time forward by
                // pause duration (was "safe by accident" via resume_blend=0).
                self.monolith_rain.shift_active_streams_last_time(elapsed);
                self.last_phosphor_time += elapsed;
                self.last_quantum_update_time += elapsed;
                self.last_glitch_time += elapsed;
                self.next_glitch_time += elapsed;
                self.last_reseed_time += elapsed;
                self.color_ecosystem.shift_in_time(elapsed);
                self.crystal_dragon_sensor.shift_in_time(elapsed);
                if let Some(ref mut cd) = self.crystal_dragon_last_poll {
                    *cd += elapsed;
                }
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
                // v30 fix: shift active quantum particle births too. Without
                // this, particles spawned before pause instantly expire on
                // unpause (age includes pause duration, exceeding 0.8s life).
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

    pub(crate) fn reset_message(&mut self) {
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
            self.border_order.clear();
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

        // BN-01/02 (Dragon Hunt v3): rebuild the cached border order here
        // (rare — only fires on `--message` set or border toggle) so
        // draw_message can borrow it instead of recomputing per frame.
        self.border_order = border::build_border_order(&self.message);
    }

    fn draw_message(&self, frame: &mut Frame) {
        let bg = self.palette.bg;
        // BC-01..05 (border chroma dragon): per-cell gradient sweeping the
        // active palette's chroma colors clockwise around the message box.
        // `palette.colors` IS the chroma gradient output (OKLab polar
        // interpolation applied at build time). On 'c'/'C' the gradient pops
        // to the new palette instantly (UI overlay semantics, no wave).
        let palette_colors = &self.palette.colors;
        let palette_n = palette_colors.len();
        let content_fg = if self.color_mode == ColorMode::Mono {
            None
        } else {
            palette_colors.last().copied()
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

        // BN-01/02 (Dragon Hunt v3): use hoisted `border_order` (rebuilt in
        // reset_message) instead of recomputing per frame. Replaces the
        // O((W+H)×N) `build_border_order` call + the per-frame HashSet
        // allocation with a Vec<bool> bit-set lookup.
        let border_order = &self.border_order;
        let mut visible_border: Vec<bool> = vec![false; self.message.len()];
        for &idx in border_order.iter().take(border_show) {
            if idx < visible_border.len() {
                visible_border[idx] = true;
            }
        }

        // BC-02 (border chroma gradient): precompute per-cell gradient color
        // for visible border cells. Maps clockwise border position i to
        // gradient t = i/total_border, then interpolates between adjacent
        // palette stops (instead of picking the discrete rounded index).
        //
        // v50 (2026-08-17) smooth gradient fix: the previous implementation
        // rounded `t * palette_last` to the nearest integer palette index,
        // which produced discrete palette-stop blocks with visible "gaps"
        // between transitions (e.g. white→red jumped from a white block to
        // a red block with no in-between). The owner explicitly flagged:
        // "border message color was already using chroma dragon but owner
        // still sees gap color between transition like white+red should be
        // smooth gradient like rain color using chroma dragon white+semi
        // red+red". The new implementation linearly interpolates between
        // palette[pos] and palette[pos+1] using the fractional remainder
        // `frac` (linear sRGB blend via `chroma::legacy::blend_toward_rgb`),
        // so adjacent border cells get smoothly-varying interpolated colors
        // — matching the rain color's per-cell chroma dragon sweep.
        //
        // On 'c'/'C' keypress the gradient pops to the new palette
        // instantly (UI overlay semantics, no wave); the interpolation is
        // recomputed every frame from the current palette so palette
        // changes reflect on the very next draw.
        let mut border_gradient: Vec<Option<Color>> = vec![None; self.message.len()];
        if palette_n > 0 && self.color_mode != ColorMode::Mono {
            let total_border_f = total_border.max(1) as f32;
            for (i, &idx) in border_order.iter().take(border_show).enumerate() {
                if idx >= border_gradient.len() {
                    continue;
                }
                let t = i as f32 / total_border_f;
                border_gradient[idx] = interpolate_palette_color(palette_colors, t);
            }
        }

        const FADE_IN_MS: usize = 100;
        const FADE_IN_START: f32 = 0.30;

        let mut content_idx = 0usize;
        for (idx, mc) in self.message.iter().enumerate() {
            let is_content = !is_border_char(mc.val);
            let is_visible_border = mc.val != ' ' && visible_border[idx];

            let (ch, cell_fg) = if is_content {
                if content_idx < reveal_count {
                    content_idx += 1;
                    let cell_fg = if let (Some(elapsed_ms), Some(base_fg)) =
                        (message_elapsed_ms, content_fg)
                    {
                        let reveal_time_ms = content_idx * 80;
                        let age_ms = elapsed_ms.saturating_sub(reveal_time_ms);
                        if age_ms >= FADE_IN_MS {
                            content_fg
                        } else {
                            let progress = age_ms as f32 / FADE_IN_MS as f32;
                            let factor = FADE_IN_START + (1.0 - FADE_IN_START) * progress;
                            // A23: chroma first, legacy::scale_rgb fallback.
                            if let Some((r, g, b)) = crate::palette::decode_color(base_fg) {
                                Some(if self.color_pipeline.is_chroma() {
                                    crate::palette::apply_brightness_rgb(r, g, b, factor)
                                } else {
                                    let (nr, ng, nb) =
                                        crate::chroma_dragon_engine::legacy::scale_rgb(
                                            r, g, b, factor,
                                        );
                                    Color::Rgb {
                                        r: nr,
                                        g: ng,
                                        b: nb,
                                    }
                                })
                            } else {
                                content_fg
                            }
                        }
                    } else {
                        content_fg
                    };
                    (mc.val, cell_fg)
                } else {
                    (' ', None)
                }
            } else if is_visible_border {
                // BC-02: border cell uses the per-cell gradient color
                // (chroma dragon gradient sweeping clockwise around the box).
                // Falls back to content_fg (head color) if palette has no
                // colors (Mono mode) or the gradient wasn't populated.
                (mc.val, border_gradient[idx].or(content_fg))
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

/// Smoothly interpolate a color from a chroma dragon palette at parametric
/// position `t ∈ [0.0, 1.0]`. `t = 0.0` returns the first palette stop;
/// `t = 1.0` returns the last; intermediate values linearly blend between
/// the two surrounding stops using `chroma::legacy::blend_toward_rgb`
/// (linear sRGB per-channel lerp).
///
/// # Why this exists
///
/// The owner reported that the border message (`--message` overlay) chroma
/// dragon sweep had visible "gaps" between palette stops — e.g. a
/// white→red sweep showed a white block then a red block with no
/// in-between, instead of the smooth white → semi-red → red gradient the
/// rain color already produced. Root cause: the previous implementation
/// rounded `t * (n-1)` to the nearest integer index and picked that
/// discrete palette stop. This helper replaces the discrete sampling with
/// linear interpolation, so adjacent border cells get smoothly-varying
/// interpolated colors matching the rain color's per-cell chroma dragon
/// sweep.
///
/// # Edge cases
///
/// - Empty palette → `None` (caller should fall back to `content_fg`).
/// - Single-stop palette → returns that stop for any `t`.
/// - `t <= 0.0` → returns the first stop (clamped).
/// - `t >= 1.0` → returns the last stop (clamped).
/// - `t` exactly at an integer stop boundary → returns that stop exactly
///   (no interpolation), so palette-identity stops are preserved.
/// - NaN / Inf `t` → returns the first stop (defensive, no panic).
///
/// # Performance
///
/// One `decode_color` + one `blend_toward_rgb` per call (≈3 ns each on a
/// warm cache). Called per visible border cell per frame; for a typical
/// 60-cell border at 60 FPS this is ≈11 µs/s — well under 0.1% CPU.
///
/// `pub(crate)` so the `cloud::tests` submodule can access it for
/// regression testing (child modules can access parent's private items,
/// but explicit `pub(crate)` makes the test surface intentional).
pub(crate) fn interpolate_palette_color(palette: &[Color], t: f32) -> Option<Color> {
    let n = palette.len();
    if n == 0 {
        return None;
    }
    if n == 1 {
        return Some(palette[0]);
    }
    // Defensive: NaN or Inf t falls back to the first stop rather than
    // producing a panic on `.floor()` or `.min()` later.
    if !t.is_finite() {
        return Some(palette[0]);
    }
    let palette_last = (n - 1) as f32;
    let scaled_t = (t.clamp(0.0, 1.0)) * palette_last;
    let pos = scaled_t.floor() as usize;
    let frac = scaled_t - pos as f32;
    let color_a = palette.get(pos).copied();
    let color_b = palette.get((pos + 1).min(n - 1)).copied();
    match (color_a, color_b) {
        // Interior stops with a non-zero fraction: interpolate linearly
        // between palette[pos] and palette[pos+1] using `frac` as the
        // blend factor (0.0 = pure palette[pos], 1.0 = pure palette[pos+1]).
        (Some(a), Some(b)) if pos + 1 < n && frac > 0.0 => {
            let (ar, ag, ab) = crate::palette::decode_color(a).unwrap_or((0, 0, 0));
            let (br, bg_c, bb) = crate::palette::decode_color(b).unwrap_or((ar, ag, ab));
            let (r, g, b) = crate::chroma_dragon_engine::legacy::blend_toward_rgb(
                ar, ag, ab, br, bg_c, bb, frac,
            );
            Some(Color::Rgb { r, g, b })
        }
        // Boundary stops (pos == palette_last) or frac == 0.0: use the
        // exact palette stop, no interpolation needed.
        (Some(a), _) => Some(a),
        _ => None,
    }
}
