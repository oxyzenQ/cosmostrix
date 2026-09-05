// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only
// LOC_EXEMPT: Cloud struct + core impls — splitting would scatter the
// 30+ coupled struct fields and their invariants across files (state
// machine flags like drift_active/user_override_since_ambient/ambient_schedule_active
// are read and written by 6 sibling modules: post_rain.rs, runtime_controls.rs,
// ecosystem.rs, pause.rs, rain_at.rs, and the interactive event_loop_* family).

//! Core simulation engine for cosmostrix — atmospheric rendering pipeline.
//! Key systems: DrawCtx (read-only renderer snapshot for per-frame
//! callbacks), DropletSpawner (3 parallax layers, see `spawn.rs`),
//! GhostEventScheduler (ghost-kanji events, see `ghost_events.rs`),
//! LivingRain (wind-gust drift, see `living_rain.rs`).
//! On color-scheme change, new droplets inherit the new palette while
//! existing droplets keep their old colors until they age out —
//! transition smoothed via Phase 8 hue-preserving chroma shader
//! (see `chroma_dragon_engine/shaders/transition/`).

mod border;
mod border_touch;
// Newly relocated from src/ root (audit M12). Re-exported as `pub(crate)`
// so the 11 existing `crate::cinematic::Foo` and
// `crate::brightness_factors::Foo` call sites continue to resolve via the
// `pub(crate) use cloud::{...};` re-export in main.rs.
pub(crate) mod brightness_factors;
pub(crate) mod cinematic;
pub(crate) mod ecosystem;
pub(crate) mod events;
mod flux;
mod flux_field;
mod ghost_events;
mod living_rain;
mod message_draw;
mod monolith;
mod monolith_glyphs;
mod monolith_helpers;
mod palette_blend;
mod phosphor;
mod phosphor_anomaly;
mod rain;
mod rain_at;
mod rain_post;
mod render;
mod runtime_controls;
mod scene_runtime;
mod spawn;
mod spawn_logic;
mod spawn_reset;
mod state;
mod vortex;

#[cfg(test)]
#[path = "../../../../test/engine/cosmic_dragon_engine/cloud/tests/mod.rs"]
mod tests;

pub(crate) use render::{CharLoc, DrawCtx};

use std::time::{Duration, Instant};

use bitvec::prelude::BitVec;
use crossterm::style::Color;
use rand::{distr::Uniform, rngs::StdRng, SeedableRng};
use smallvec::SmallVec;

use crate::constants::*;
use crate::droplet::Droplet;
use crate::palette::{build_palette, Palette};
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorPipeline, ColorScheme, MonolithSize, ShadingMode};

use ecosystem::{
    BehaviorProfile, ColorEcosystem, EntropyDrift, ProfileParams, RendererMemory, StorytellingState,
};
use monolith::MonolithRain;
use state::{AnomalyZone, BorderPulse, ColumnStatus, MsgChr, QuantumParticle};
use vortex::VortexRain;

use flux::FluxRain;

use ghost_events::GhostEventScheduler;
use render::FlashWave;

/// Message-reveal lead: when a cinematic intro plays, the message
/// overlay waits this long before revealing, so the text lands shortly
/// after the intro finishes (logo ~4.5 s → ~1.5 s of rain-alone
/// breathing room; cosmic ~5 s → ~1 s). v52: armed ONLY by the intro
/// runner (`hold_message_behind_intro`), never by `set_message`.
pub(crate) const MESSAGE_INTRO_LEAD: Duration = Duration::from_secs(6);

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
    /// Task-18 third rain style: polar-orbit motes (vortex scene).
    pub(crate) vortex_rain: VortexRain,
    /// Task-19 fourth rain style: fluid particles in a PIC/FLIP
    /// incompressible liquid (flux scene — replaces the task-18
    /// ripple surface style the owner rejected as not unique).
    pub(crate) flux_rain: FluxRain,

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
    /// PERF-4: when false, ALL particle subsystems are no-ops. --no-effects.
    pub(crate) effects_enabled: bool,
    /// M1: hysteresis state for phosphor decay skip (prevents strobing).
    pub(crate) phosphor_skipped: bool,
    /// PERF-3: hysteresis state for phosphor pressure boost (prevents
    /// oscillation on VTE fullscreen). Trigger at >0.30, release at <0.15.
    /// Same hysteresis pattern as `phosphor_skipped`.
    pub(crate) phosphor_pressure_boost_active: bool,
    /// v50.0.0-beta.6: terminal-aware phosphor decay multiplier.
    /// 1.0 = high-perf (Alacritty), 1.3 = standard (VTE), 1.6 = xterm.js.
    /// Applied as `PHOSPHOR_DECAY_RATE * phosphor_decay_mult * elapsed_sec`.
    pub(crate) phosphor_decay_mult: f32,
    /// v50.0.0-beta.6: ghost brightness cap (fraction of 255). When >0.0,
    /// phosphor cells with energy below `cap * 255` are killed immediately.
    /// Prevents dim ghosts persisting on VTE terminals. 0.0 = no cap.
    pub(crate) ghost_brightness_cap: f32,
    /// v50.0.0-beta.6: terminal-aware droplet speed multiplier.
    /// Applied to chars_per_sec so droplets fall faster on slower-rendering
    /// terminals (VTE/xterm.js) to match Alacritty's visual speed.
    pub(crate) speed_mult: f32,
    pub(crate) max_sim_delta: Duration,

    pub(crate) shading_mode: ShadingMode,

    pub(crate) message: Vec<MsgChr>,
    pub(crate) message_text: Option<String>,
    pub(crate) message_border: bool,
    pub(crate) message_start_time: Option<Instant>,
    /// v80.0.0-beta.1 msg-fill-style: active reveal style for the message overlay.
    /// Set from CloudConfig (`-mfs`/`--msg-fill-style` CLI or
    /// `msg-fill-style` config key). Default `Typewriter` = bit-identical
    /// to the pre-v80.0.0-beta.1 renderer (LTS guarantee).
    pub(crate) msg_fill_style: crate::msg_fill_style::MsgFillStyle,
    /// v80.0.0-beta.1 msg-fill-style (words): 1-based word ordinal per message cell,
    /// rebuilt in `reset_message` (Z-5 hoisted — no per-frame rebuild).
    /// Content cells carry the ordinal of the word they belong to;
    /// non-content cells (border + spaces) carry the running ordinal
    /// (never read for them). 0 = padding space before the first word.
    pub(crate) message_word_ordinals: Vec<u32>,
    /// BN-01/02 (Dragon Hunt v3): hoisted clockwise border-cell index list.
    /// Rebuilt only in `reset_message` (rare — once per `--message` invocation
    /// or border toggle). `draw_message` borrows this instead of calling
    /// `build_border_order` per frame (was O((W+H)×N) per frame; now O(1) borrow).
    pub(crate) border_order: Vec<usize>,

    /// v80.0.0-beta.1 msg-fill-style (engrave): bounded 48-slot spark sidecar
    /// (dedicated pool — see `msg_fill_style/engrave.rs`).
    pub(crate) engrave: crate::msg_fill_style::engrave::EngraveState,

    /// v80.0.0-beta.1 msg-fill-style (scorch): bounded 16-slot smoke sidecar
    /// (dedicated pool — see `msg_fill_style/scorch.rs`).
    pub(crate) scorch: crate::msg_fill_style::scorch::ScorchState,

    /// RAIN_BORDER_TOUCH_GLOW: cached top-border geometry, refreshed in
    /// `reset_message`. Used by the droplet advance loop (rain.rs) for
    /// cheap O(1) touch detection: `prev_head_put_line < top` and
    /// `head_put_line >= top` and `bound_col in [left, right)` → push
    /// a `BorderPulse`.
    /// `top_line` is `u16::MAX` sentinel when no message overlay is active
    /// (so the check `head_put_line >= top_line` is false for all real
    /// droplet head positions, avoiding spurious touches).
    pub(crate) message_top_line: u16,
    pub(crate) message_left_col: u16,
    pub(crate) message_right_col: u16,

    /// RAIN_BORDER_TOUCH_GLOW: active touch pulses. Drained of expired
    /// entries every frame in `draw_message`. Expected max size ~8 (one
    /// per active column crossing the top edge at any time).
    pub(crate) border_pulses: Vec<BorderPulse>,
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
    pub(crate) border_cross_candidates: Vec<(usize, u16, u16)>, // B-1: hoisted scratch (was per-frame Vec alloc in rain.rs monolith path)
    pub(crate) border_gradient_scratch: Vec<Option<Color>>, // Z-5: hoisted scratch (was per-frame Vec alloc in draw_message)
    pub(crate) bottom_corner_scratch: std::collections::HashSet<usize>, // Z-5: hoisted scratch (was per-frame HashSet alloc in draw_message)

    pub(crate) anomaly_zones: Vec<AnomalyZone>,

    // Profile identity — currently always Monolith. Retained for future
    // profile selector (Void, Neural, etc.) which will read this field.
    #[allow(dead_code)]
    pub(crate) profile: BehaviorProfile,
    pub(crate) bench_mode: bool, // Z-6: true in benchmark mode — skips message cosmetics (draw_message + border-cross detection)
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
    /// Dragon Engine v2: calc-v2 recency history. Tracks recently
    /// selected themes so calc-v2 can apply a recency penalty,
    /// preventing A→B→A oscillation. Bounded ring buffer (8 entries).
    pub(crate) crystal_dragon_drift_history:
        crate::crystal_dragon_engine::point_system::DriftHistory,
    /// v30 Bug #4: true when --colors-custom active.
    pub(crate) custom_palette_active: bool,
    /// v80.0.0-beta.2 HUD honesty (owner bug 2026-09-02): the NAME of the
    /// active custom palette, tracked by the Cloud itself so every
    /// activation path (startup create_cloud, live-reload rebuild, ambient
    /// fire, scene-runtime custom-scene switch) can surface it on the HUD
    /// `clr:` line. Previously the HUD read only
    /// `CloudConfig.custom_palette_name` (startup/live-rebuild), so a
    /// palette activated Cloud-side (e.g. an ambient entry firing a
    /// scene-custom block with `colors-custom`) rendered its colors but
    /// the HUD kept showing the base-scene scheme name ("clr: Purple").
    /// Cleared together with `custom_palette_active` by `set_color_scheme`.
    pub(crate) custom_palette_name: Option<String>,
    /// v30 Bug #5: color_tune on Cloud for set_color_scheme re-apply.
    pub(crate) color_tune: crate::color_tune::ColorTune,
    /// true when ambient asserted palette. Cleared by `c`/`C`/`x`.
    pub(crate) ambient_palette_locked: bool,
    /// v50.0.0-beta.7 state machine: drift_active=true while drift waiting for snapback; drift_start=when it began.
    pub(crate) drift_active: bool,
    pub(crate) drift_start: Option<std::time::Instant>,
    /// true when user overrode scene/color/charset (`x`/`c`/`s`/`C`/`S`) or
    /// Crystal Dragon drifted. Cleared by ambient fire. Prevents day-boundary dedup skip.
    pub(crate) user_override_since_ambient: bool,
    /// Z-master-1X: true when ambient schedule has entries. When false,
    /// drift gate skips `user_override_since_ambient` (see post_rain.rs).
    pub(crate) ambient_schedule_active: bool,

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
            vortex_rain: VortexRain::new(),
            flux_rain: FluxRain::new(),
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
            effects_enabled: true,
            phosphor_skipped: false,
            phosphor_pressure_boost_active: false,
            phosphor_decay_mult: 1.0,
            ghost_brightness_cap: 0.0,
            speed_mult: 1.0,
            max_sim_delta: Duration::from_millis(0),
            shading_mode,
            message: Vec::new(),
            message_text: None,
            message_border: false,
            message_start_time: None,
            // v80.0.0-beta.2 msg-fill-style: Engrave default (owner champion
            // winner). The pre-beta.2 default was Typewriter for LTS
            // bit-identical parity. Cloud::new sets the field here, but the
            // event loop always calls set_msg_fill_style() with the resolved
            // CLI/config value before the first frame, so this default only
            // matters for tests that construct a Cloud without going through
            // the CLI path.
            msg_fill_style: crate::msg_fill_style::MsgFillStyle::Engrave,
            message_word_ordinals: Vec::new(),
            border_order: Vec::new(),
            message_top_line: u16::MAX,
            message_left_col: 0,
            message_right_col: 0,
            border_pulses: Vec::new(),
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
                    max_trail: QUANTUM_RIPPLE_TRAIL_LEN as u8,
                    lifetime: QUANTUM_RIPPLE_LIFETIME_SECS,
                    sim_age: 0.0,
                };
                QUANTUM_RIPPLE_POOL_SIZE
            ],
            quantum_active_count: 0,
            engrave: crate::msg_fill_style::engrave::EngraveState::new(now),
            scorch: crate::msg_fill_style::scorch::ScorchState::new(now),
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
            border_cross_candidates: Vec::with_capacity(128),
            border_gradient_scratch: Vec::with_capacity(64),
            bottom_corner_scratch: std::collections::HashSet::with_capacity(2),
            last_phosphor_time: now,
            last_quantum_update_time: now,
            anomaly_zones: Vec::new(),
            profile: BehaviorProfile::Monolith,
            bench_mode: false,
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
            crystal_dragon_drift_history:
                crate::crystal_dragon_engine::point_system::DriftHistory::new(),
            // v30 strengthen: overridden in app.rs create_cloud.
            custom_palette_active: false,
            custom_palette_name: None,
            color_tune: crate::color_tune::ColorTune::IDENTITY,
            // ambient-harmony flags start false (set by ambient fire,
            // cleared by user override x/c/s).
            ambient_palette_locked: false,
            drift_active: false,
            drift_start: None,
            user_override_since_ambient: false,
            // Z-master-1X: set from schedule in `create_cloud`.
            ambient_schedule_active: false,
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
        // v52: the reveal timeline starts NOW — set_message is
        // context-free (--intro none runs, live-reload rebuilds, the
        // intro's temporary clouds). The lead is armed by the intro
        // runner when a cinematic actually plays; pre-v52 armed it
        // unconditionally (`now + 6 s` → dead air everywhere else).
        self.message_start_time = Some(Instant::now());
        self.reset_message();
        self.force_draw_everything = true;
    }

    /// Arm the message-reveal lead so the overlay stays hidden behind
    /// the cinematic intro. Called by the intro runner right before the
    /// animation starts; no-op without a message.
    pub fn hold_message_behind_intro(&mut self) {
        if self.message_text.is_some() {
            self.message_start_time = Some(Instant::now() + MESSAGE_INTRO_LEAD);
        }
    }

    /// Pull a still-future message reveal start to NOW (q skip / signal /
    /// terminal below the intro floor — nothing hides the message anymore,
    /// so waiting out the lead is dead air). Past starts untouched.
    pub fn cut_message_intro_lead(&mut self) {
        let now = Instant::now();
        if let Some(start) = self.message_start_time {
            if start > now {
                self.message_start_time = Some(now);
            }
        }
    }

    pub fn restart_message_typewriter(&mut self) {
        if self.message_text.is_some() {
            // v52: r restart replays immediately (no intro lead at
            // runtime). v80.0.0-beta.1 engrave/scorch: re-arm spark/smoke detector.
            self.message_start_time = Some(Instant::now());
            self.engrave.reset();
            self.scorch.reset();
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

    /// v80.0.0-beta.1 msg-fill-style: set the reveal style. On a real change with
    /// an active message, the reveal restarts immediately (no intro
    /// lead). v80.0.0-beta.1 engrave/scorch: re-arm spark/smoke detector.
    pub fn set_msg_fill_style(&mut self, style: crate::msg_fill_style::MsgFillStyle) {
        if self.msg_fill_style != style {
            self.msg_fill_style = style;
            if self.message_text.is_some() {
                self.message_start_time = Some(Instant::now());
                self.engrave.reset();
                self.scorch.reset();
                self.force_draw_everything = true;
            }
        }
    }

    pub fn enable_events(&mut self) {
        self.event_manager.enable_events();
    }

    /// PERF-4: when false, ALL particle subsystems become no-ops (quantum
    /// ripple, border spark, click flash waves, anomaly zones). --no-effects.
    pub fn set_effects_enabled(&mut self, enabled: bool) {
        self.effects_enabled = enabled;
    }

    pub fn set_mouse_position(&mut self, col: u16, line: u16) {
        // v80.0.0-alpha.1 (--no-effects audit, PERF-4 final closure): when
        // effects are disabled, store the u16::MAX "no cursor" sentinel so
        // the droplet hover-glow branch (`ctx.mouse_col != u16::MAX`) never
        // fires — the per-cell elliptical-brightening math near the cursor
        // is a cosmetic overlay, exactly the class --no-effects exists to
        // suppress. Position tracking itself stays live (mouse events are
        // still consumed — blocks drag-select); only the glow read goes
        // dark. Benchmark mode is unaffected (mouse_col is already MAX
        // there — no mouse events). This closes the last cosmetic leak
        // found in the --no-effects audit: hover glow + CRT vignette were
        // the only two cosmetic paths still running under the flag.
        if !self.effects_enabled {
            self.mouse_col = u16::MAX;
            self.mouse_line = u16::MAX;
            return;
        }
        self.mouse_col = col;
        self.mouse_line = line;
    }

    pub fn set_mouse_click(&mut self, col: u16, line: u16) {
        // PERF-4 strengthen: --no-effects gate. When effects are disabled,
        // skip flash-wave activation entirely. Without this gate the
        // dual-ring click flash continued to spawn under --no-effects —
        // a partial-disable leak (only the quantum-ripple particles were
        // suppressed, the expanding ring overlay was not). Early-return
        // here is the correct fix: no new FlashWave slot is activated,
        // and any previously-active waves fade out naturally on their
        // next update tick. spawn_quantum_ripple (called below when
        // effects are on) is also independently gated for safety.
        if !self.effects_enabled {
            return;
        }
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

    /// Derive the effective `GlitchLevel` from the live Cloud state.
    ///
    /// v80.0.0-beta.2 (S-master-LOGIC-1): the Cloud stores the resolved
    /// numeric glitch fields (glitchy/glitch_pct), not the enum — the
    /// enum on CloudConfig can be stale after an ambient apply (the
    /// ambient path writes the cloud fields directly). This derivation
    /// (shared by the HUD `glth:` line and the post-exit final-runtime
    /// verbose) always reports the EFFECTIVE level. Thresholds match the
    /// preset definitions in `apply_glitch_level_runtime`.
    #[must_use]
    pub fn glitch_level(&self) -> crate::config::GlitchLevel {
        use crate::config::GlitchLevel;
        if !self.glitchy {
            GlitchLevel::None
        } else if self.glitch_pct < 0.05 {
            GlitchLevel::Subtle
        } else if self.glitch_pct < 0.15 {
            GlitchLevel::Default
        } else {
            GlitchLevel::Intense
        }
    }

    #[must_use]
    pub fn rain_style(&self) -> RainStyle {
        self.rain_style
    }

    /// v50.0.0-beta.7: accessor for monolith_size (needed by HUD mnst metric).
    #[must_use]
    pub fn monolith_size(&self) -> MonolithSize {
        self.monolith_size
    }

    /// Total droplet count (active + inactive slots). Test-only diagnostic —
    /// production reads `active_droplet_count()` instead.
    #[cfg(test)]
    #[must_use]
    pub fn droplet_count(&self) -> usize {
        self.droplets.len()
    }

    #[must_use]
    pub fn active_droplet_count(&self) -> usize {
        match self.rain_style {
            RainStyle::Monolith => self.monolith_rain.active_count(),
            RainStyle::Vortex => self.vortex_rain.active_count(),
            RainStyle::Flux => self.flux_rain.active_count(),
            // Droplet family: the Glyph cascade pool.
            RainStyle::Glyph => self.droplets.iter().filter(|d| d.is_alive).count(),
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

    /// Phase D Bug #9: carry color_ecosystem + entropy_drift across live-reload
    /// (prevents brightness discontinuity when config is edited mid-session).
    /// v30: also carries `start_anchor` so time-varying phases stay continuous.
    ///
    /// Z-master-1X round 4: do NOT carry `drift_active` / `drift_start` across
    /// live reload. Those are drift-CYCLE state (not engine state), and carrying
    /// them creates a deadlock when the reload re-applies an ambient entry: the
    /// re-apply sets `user_override_since_ambient = false`, which disables the
    /// snapback mechanism that would normally clear `drift_active`. With
    /// `drift_active = true` inherited + snapback disabled, the drift gate
    /// `!drift_active` blocks all future drifts forever — the owner symptom
    /// "ambient dominant, drift rare after live reload, restart fixes it."
    /// The sensor state (`crystal_dragon_sensor`) IS still carried — it
    /// represents the engine's understanding of the system (CPU point, EMA,
    /// theme entered-at), not the per-cycle drift bookkeeping. A live reload
    /// is a config change; the drift cycle should reset cleanly so the next
    /// poll can fire a fresh drift. `crystal_dragon_last_poll` is carried
    /// ONLY when the old engine was running (v80.0.0-alpha.1 S-master-HUNT-6,
    /// see the inline comment at the assignment) so an off->on enable arms
    /// a fresh cadence clock instead of inheriting a never-ticking stale
    /// one.
    ///
    /// v80.0.0-alpha.1: `crystal_dragon_control` is NO LONGER inherited.
    /// The fresh Cloud's control is config-derived (create_cloud applies
    /// `crystal_dragon_secs` from the rebuilt CloudConfig); carrying the
    /// old cloud's control would pin `polling_secs` to the pre-edit value —
    /// exactly the bug class the tunable exists to avoid (a live-reload edit
    /// to `crystal-dragon-secs` would have silently kept the old cadence).
    /// All other control fields (drift_chance, ema_alpha, dwell floor,
    /// sensor/calc mode) are constants — the sensor carries its own EMA
    /// alpha snapshot, so nothing user-visible is lost.
    pub fn inherit_ecosystem_state(&mut self, other: &Cloud) {
        self.color_ecosystem = other.color_ecosystem;
        self.entropy_drift = other.entropy_drift;
        self.start_anchor = other.start_anchor;
        // Crystal Dragon sensor state survives live reload.
        self.crystal_dragon_sensor = other.crystal_dragon_sensor;
        // v80.0.0-alpha.1 (S-master-HUNT-6): the cadence clock survives a
        // reload only when the engine was ALREADY running. If the previous
        // cloud had crystal-dragon OFF, the clock is reset to None so the
        // freshly-enabled engine ARMS (first poll + first decision one
        // full polling_secs after the enabling edit — see the arming-tick
        // contract in `crystal_dragon_tick`). Carrying a stale/None clock
        // from an off engine was harmless pre-HUNT-6 (the decision ran
        // every frame anyway); with the poll gate it is the difference
        // between "elegant first drift at +10m" and a mid-cycle burst.
        // The off->on transition is detected via the OLD cloud's flag:
        // the fresh Cloud's flag comes from the rebuilt config, so a
        // same-state reload (on->on) keeps the boundary phase, and
        // on->off carries a clock that is simply never read (the tick
        // does not run while the engine is off).
        self.crystal_dragon_last_poll = if other.crystal_dragon {
            other.crystal_dragon_last_poll
        } else {
            None
        };
        // Dragon Engine v2: carry drift history across live-reload.
        self.crystal_dragon_drift_history = other.crystal_dragon_drift_history;
        // drift_active + drift_start are NOT inherited — see doc comment above.
        // They default to false / None on the fresh Cloud (set in Cloud::new).
        // crystal_dragon_control is NOT inherited — see doc comment above
        // (v80.0.0-alpha.1: the fresh Cloud's control already carries the
        // live-reloaded polling_secs).
    }
    /// Active scene name. Test-only accessor — production reads the
    /// `scene_name` field directly or via `hud_colors()`.
    #[cfg(test)]
    #[must_use]
    pub fn active_scene(&self) -> &str {
        &self.scene_name
    }
    #[must_use]
    pub fn hud_colors(&self) -> &[crossterm::style::Color] {
        &self.palette.colors
    }
    /// Returns `true` when the cloud is in any pause-related state:
    /// fully paused (`self.pause`) OR decelerating toward pause
    /// (`self.pause_start.is_some()`).
    ///
    /// Callers that need to gate user input (keyboard shortcuts, mouse
    /// click effects) during pause MUST check this instead of only
    /// `self.pause`, otherwise interactions during the deceleration
    /// window accumulate stale state that causes "stuck particles" on
    /// resume (owner-reported bug: rapid p-taps left effects hanging).
    #[must_use]
    pub fn is_paused_or_decelerating(&self) -> bool {
        self.pause || self.pause_start.is_some()
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
}

// v50.0.0-beta.7 LOC refactor: reset_message method extracted to
// reset_message.rs as a separate impl Cloud block.
mod pause;
mod post_rain;
mod reset_message;

// v50.0.0-beta.7 LTS: interpolate_palette_color extracted to palette_blend.rs
// to keep this file under the 800-LOC cap. Re-exported here so all
// existing crate::cloud::interpolate_palette_color(...) call sites
// (rain_post, hud, chroma shaders, tests) continue to resolve unchanged.
pub(crate) use palette_blend::interpolate_palette_color;
