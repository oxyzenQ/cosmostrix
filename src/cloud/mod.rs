// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Core simulation engine for Cosmostrix.
//!
//! This module implements the entire atmospheric rendering pipeline:
//! droplet spawning, advancement, palette management, phosphor persistence,
//! anomaly events, and the autonomous cinematic ecosystem.
//!
//! ## Key Systems
//!
//! - **DrawCtx**: A read-only snapshot of renderer state passed to each
//!   droplet's `draw()` method, avoiding borrow conflicts with the mutable
//!   droplet iteration loop.
//! - **Behavior Profiles**: Seven cinematic identities (Monolith, Void, Neural,
//!   Decay, Eclipse, Static, Pulse) that define fundamentally different
//!   atmospheric behaviors — not mere recolors.
//! - **Color Ecosystem**: Slow autonomous drift of luminance, saturation, and
//!   hue that makes the renderer feel organically alive over long sessions.
//! - **Atmospheric Evolution**: Entropy cycles that modulate density, luminance
//!   and anomaly pressure on minute-scale timescales.
//! - **Renderer Memory**: Long-timescale history that influences emergent
//!   behavior based on past atmospheric conditions.
//! - **Storytelling State**: Watches for convergence across other systems and
//!   occasionally produces emotionally resonant emergent moments.
//!
//! ## Palette Transition System
//!
//! When the color scheme changes, new droplets inherit the new palette while
//! existing streams retain their birth palette for their entire lifecycle.
//! Rows adopt the new palette via a top-to-bottom wave (matching the charset
//! transition visual language), creating an organic propagation cascade
//! instead of a robotic simultaneous switch.

mod atmospheric_events;
mod ecosystem;
mod events;
mod monolith;
mod monolith_glyphs;
mod phosphor;
mod rain;
mod render;
mod runtime_controls;
mod scene_runtime;
mod spawn;
mod state;

#[cfg(test)]
mod tests;

// Re-export public types needed by other modules (droplet.rs uses CharLoc + DrawCtx)
pub(super) use render::{CharLoc, DrawCtx};

use std::time::{Duration, Instant};

use bitvec::prelude::BitVec;
use crossterm::style::Color;
use rand::{distr::Uniform, rngs::StdRng, SeedableRng};
use smallvec::SmallVec;

use crate::cell::Cell;
use crate::constants::*;
use crate::droplet::Droplet;
use crate::frame::Frame;
use crate::palette::{build_palette, Palette};
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, MonolithSize, ShadingMode};

use ecosystem::{
    AtmosphericEvolution, BehaviorProfile, ColorEcosystem, ProfileParams, RendererMemory,
    StorytellingState,
};
use monolith::MonolithRain;
use state::{AnomalyZone, ColumnStatus, MsgChr};

use atmospheric_events::AtmosphericEventManager;

// Cloud is crate-facing but exposes internal state to split submodules/tests.
// Boolean fields mirror existing CLI/runtime flags and are kept explicit.
#[allow(private_interfaces, clippy::struct_excessive_bools)]
pub struct Cloud {
    pub(super) lines: u16,
    pub(super) cols: u16,

    pub(super) palette: Palette,
    pub(super) color_mode: ColorMode,
    pub(super) rain_style: RainStyle,
    monolith_size: MonolithSize,

    pub(super) full_width: bool,
    pub(super) shading_distance: bool,
    pub(super) bold_mode: BoldMode,

    pub(super) async_mode: bool,
    pub(super) raining: bool,
    pub(super) pause: bool,

    pub(super) droplet_density: f32,
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
    pub(super) charset_transition_start: Option<Instant>,
    pub(super) glitch_pool: Vec<char>,
    pub(super) glitch_pool_idx: usize,

    pub(super) glitch_map: BitVec,
    pub(super) color_map: Vec<u8>,

    /// Precomputed viewport edge fade factor per line. Indexed by `line`.
    /// Eliminates per-cell float division in Droplet::draw and Monolith draw.
    /// Resized in reset() on terminal resize.
    pub(super) edge_fade_lut: Vec<f32>,

    /// Free-list of dead droplet indices for O(1) spawn slot lookup.
    /// Replaces the previous linear scan in spawn_droplets that searched
    /// `droplets[]` for the next `!is_alive` slot. Seeded in reset() with
    /// 0..len (all droplets start dead). Popped on spawn, pushed on death.
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
    pub(super) spawn_remainder: f32,
    pub(super) pause_time: Option<Instant>,

    /// Resume time-scale factor: 0.0 (just resumed) → 1.0 (fully active).
    /// Scales the simulation clock for all droplets during the smoothstep
    /// resume transition, producing cinematic inertia recovery — the rain
    /// decelerates into the pause and accelerates smoothly out of it.
    pub(super) resume_blend: f32,
    /// Timestamp when the most recent unpause occurred. Used to compute
    /// the smoothstep S-curve for `resume_blend`.
    pub(super) resume_start: Option<Instant>,

    pub(super) force_draw_everything: bool,

    /// Pending semantic invalidation: set to true when the renderer's semantic
    /// identity changes (charset switch, shading mode toggle). On the next
    /// `rain_at()`, this triggers `frame.invalidate_semantic()` which bumps
    /// the frame's `semantic_gen`, forcing the Terminal to do a full redraw
    /// and properly synchronize its LastFrame cache with the new semantics.
    pub(super) semantic_invalidate: bool,

    /// Frame counter for periodic full redraw (ANSI drift correction).
    /// Every `FULL_REDRAW_INTERVAL_FRAMES`, forces a complete screen refresh
    /// to correct any accumulated terminal state desync.
    pub(super) frames_since_full_redraw: u64,

    pub(super) perf_pressure: f32,
    pub(super) max_sim_delta: Duration,

    pub(super) shading_mode: ShadingMode,

    pub(super) message: Vec<MsgChr>,
    pub(super) message_text: Option<String>,
    pub(super) message_border: bool,
    /// When the message was set — for typewriter reveal timing.
    pub(super) message_start_time: Option<Instant>,
    pub(super) color_scheme: ColorScheme,
    pub(super) default_background: bool,
    scene_name: String,

    /// Palette generation table: stores up to MAX_PALETTE_SLOTS palettes for
    /// generation-based transitions.  Each droplet carries a `palette_slot`
    /// that indexes into this table, so old streams retain their birth palette
    /// while new streams inherit the latest one.
    pub(super) palette_table: [Option<Palette>; MAX_PALETTE_SLOTS],

    /// Index of the currently active palette slot (where new streams inherit).
    pub(super) active_palette_slot: u8,

    /// Time when the current palette transition started (None if not transitioning).
    /// Used for row-based top-to-bottom wave progression.
    pub(super) transition_start: Option<Instant>,

    /// Per-column palette slot: tracks which palette each column is currently
    /// using for spawning.  During a transition, all columns adopt the new
    /// palette simultaneously since the wave is row-based (top-to-bottom),
    /// not column-based. This field is kept for spawn-time inheritance.
    pub(super) column_palette_slot: Vec<u8>,

    /// Mouse cursor column position (u16::MAX if no mouse).
    pub mouse_col: u16,

    /// Mouse cursor line position (u16::MAX if no mouse).
    pub mouse_line: u16,

    /// Whether mouse interaction is enabled.
    pub mouse_enabled: bool,

    /// Flash effect: click column.
    pub(super) flash_col: u16,

    /// Flash effect: click line.
    pub(super) flash_line: u16,

    /// Flash effect: start time (None if no active flash).
    pub(super) flash_time: Option<Instant>,

    pub(super) last_reseed_time: Instant,

    // --- Phosphor persistence state ---
    /// Per-cell phosphor energy (0 = dead, 255 = full). Tracks residual
    /// luminance for CRT-style afterglow after a droplet's tail passes.
    pub(super) phosphor: Vec<u8>,
    /// Per-cell base foreground color captured when phosphor was activated.
    pub(super) phosphor_base_fg: Vec<Option<Color>>,
    /// Per-cell base character captured when phosphor was activated.
    /// Used to render ghost cells with the original character at dimmed
    /// brightness instead of a blank space — this makes trail afterglow
    /// look like fading text rather than dim colored patches, which is
    /// critical for perceived smoothness and cinematic quality.
    pub(super) phosphor_base_ch: Vec<char>,
    /// Per-cell layer identifier for layer-aware phosphor decay.
    pub(super) phosphor_layer: Vec<u8>,
    /// BitVec tracking which cells were refreshed by a droplet this frame.
    pub(super) phosphor_fresh: BitVec,
    /// BitVec tracking which cells are currently in `phosphor_active`.
    /// Provides O(1) membership check during dedup, replacing the
    /// previous O(N) `phosphor_active.contains()` linear scan that ran
    /// per fresh cell per frame (5,000-100,000 wasted ops/frame).
    pub(super) phosphor_in_active: BitVec,
    /// Time of the last phosphor pass for frame-rate-independent decay.
    pub(super) last_phosphor_time: Instant,
    /// Active phosphor indices — cells with non-zero energy, tracked for O(active) decay.
    /// Typical frame has <100 active cells, eliminating 95%+ of Pass 3 iterations.
    pub(super) phosphor_active: SmallVec<[usize; 256]>,
    /// Snapshot of `tracked_fresh` from the previous phosphor_decay_pass.
    /// Used to incrementally clear `phosphor_fresh` bits at the start of
    /// the next pass — replaces the previous O(W×H) `phosphor_fresh.fill(false)`
    /// that scanned the entire grid every frame even when only ~200 cells
    /// were dirty.
    pub(super) phosphor_last_fresh: SmallVec<[usize; 256]>,

    // --- Rare anomaly events ---
    /// Active anomaly zones currently affecting the screen.
    pub(super) anomaly_zones: Vec<AnomalyZone>,

    // --- Autonomous cinematic ecosystem ---
    /// Active cinematic behavior profile.
    pub(super) profile: BehaviorProfile,
    /// Interpolated profile params (current, transitioning toward target).
    pub(super) profile_current: ProfileParams,
    /// Target profile params (what we're transitioning toward).
    pub(super) profile_target: ProfileParams,
    /// Time when profile transition started.
    pub(super) profile_transition_start: Option<Instant>,

    /// Temporal color ecosystem.
    pub(super) color_ecosystem: ColorEcosystem,
    /// Autonomous atmospheric evolution.
    pub(super) atmosphere: AtmosphericEvolution,
    /// Long-timescale renderer memory.
    pub(super) memory: RendererMemory,
    /// Emergent visual storytelling.
    pub(super) storytelling: StorytellingState,

    // --- Scene-entry ramp ---
    /// Timestamp when the glyph scene-entry ramp started. During the ramp
    /// period (GLYPH_ENTRY_RAMP_DURATION_MS), spawn rate is gradually
    /// increased from GLYPH_ENTRY_RAMP_MIN_SCALE to 1.0 via smoothstep,
    /// creating a cinematic top-entry cascade. None when no ramp is active.
    pub(super) glyph_entry_time: Option<Instant>,

    // --- Color drift gate ---
    /// When false (default), autonomous palette drift from ColorEcosystem is
    /// suppressed so that explicit CLI/config/profile color remains sticky.
    /// When true, the ecosystem may spontaneously change color schemes via
    /// `related_schemes()` drift, providing atmospheric color evolution.
    pub(super) auto_color_drift: bool,

    /// Runtime idle state (passed from event loop).
    pub(super) is_idle: bool,

    // --- Atmospheric Event Engine ---
    /// Event manager for cinematic atmospheric events (ghosts, etc.).
    pub(super) event_manager: AtmosphericEventManager,
}

impl Cloud {
    // Constructor mirrors the public runtime config knobs; grouping them now
    // would churn stable call sites without reducing renderer complexity.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        color_mode: ColorMode,
        full_width: bool,
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
            rain_style,
            monolith_size: MonolithSize::Normal,
            full_width,
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
            chars: Vec::new(),
            char_pool: Vec::new(),
            previous_char_pool: Vec::new(),
            charset_transition_start: None,
            glitch_pool: Vec::new(),
            glitch_pool_idx: 0,
            glitch_map: BitVec::new(),
            color_map: Vec::new(),
            edge_fade_lut: Vec::new(),
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
            spawn_remainder: 0.0,
            pause_time: None,
            resume_blend: 1.0,
            resume_start: None,
            force_draw_everything: false,
            semantic_invalidate: false,
            frames_since_full_redraw: 0,
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
            flash_col: u16::MAX,
            flash_line: u16::MAX,
            flash_time: None,
            last_reseed_time: now,
            phosphor: Vec::new(),
            phosphor_base_fg: Vec::new(),
            phosphor_base_ch: Vec::new(),
            phosphor_layer: Vec::new(),
            phosphor_fresh: BitVec::new(),
            phosphor_in_active: BitVec::new(),
            phosphor_active: SmallVec::new(),
            phosphor_last_fresh: SmallVec::new(),
            last_phosphor_time: now,
            anomaly_zones: Vec::new(),
            profile: BehaviorProfile::Monolith,
            profile_current: BehaviorProfile::Monolith.params(),
            profile_target: BehaviorProfile::Monolith.params(),
            profile_transition_start: None,
            color_ecosystem: ColorEcosystem::new(now),
            atmosphere: AtmosphericEvolution::new(now),
            memory: RendererMemory::new(now),
            storytelling: StorytellingState::new(now),
            glyph_entry_time: None,
            auto_color_drift: AUTO_COLOR_DRIFT_DEFAULT,
            is_idle: false,
            event_manager: AtmosphericEventManager::new(now),
        }
    }

    pub fn set_message(&mut self, msg: &str) {
        self.message_text = Some(msg.to_string());
        self.message_start_time = Some(Instant::now());
        self.reset_message();
        self.force_draw_everything = true;
    }

    pub fn set_message_border(&mut self, on: bool) {
        self.message_border = on;
        if self.message_text.is_some() {
            self.reset_message();
            self.force_draw_everything = true;
        }
    }

    /// Enable atmospheric events (called when entering interactive mode).
    pub fn enable_events(&mut self) {
        self.event_manager.enable_events();
    }

    /// Set mouse cursor position for interaction effects.
    pub fn set_mouse_position(&mut self, col: u16, line: u16) {
        self.mouse_col = col;
        self.mouse_line = line;
    }

    /// Trigger a click flash effect at the given position.
    pub fn set_mouse_click(&mut self, col: u16, line: u16) {
        self.flash_col = col;
        self.flash_line = line;
        self.flash_time = Some(Instant::now());
    }

    #[must_use]
    pub fn color_scheme(&self) -> ColorScheme {
        self.color_scheme
    }

    #[must_use]
    pub fn rain_style(&self) -> RainStyle {
        self.rain_style
    }

    /// Get current behavior profile.
    pub fn profile(&self) -> BehaviorProfile {
        self.profile
    }

    /// Cycle to the next behavior profile with smooth transition.
    pub fn cycle_profile(&mut self) {
        let next = self.profile.cycle();
        self.profile = next;
        self.profile_target = next.params();
        self.profile_transition_start = Some(Instant::now());
    }

    /// Get the name of the current behavior profile.
    pub fn profile_name(&self) -> &'static str {
        self.profile.name()
    }

    /// Return the total number of droplet slots (alive + dead).
    #[must_use]
    pub fn droplet_count(&self) -> usize {
        self.droplets.len()
    }

    /// Return the number of currently active (alive) droplets.
    /// More accurate for performance metrics than `droplet_count()`
    /// which includes recycled slots waiting to be reused.
    #[must_use]
    pub fn active_droplet_count(&self) -> usize {
        if matches!(self.rain_style, RainStyle::Monolith) {
            self.monolith_rain.active_count()
        } else {
            self.droplets.iter().filter(|d| d.is_alive).count()
        }
    }

    /// Get the name of the active scene.
    #[must_use]
    pub fn active_scene(&self) -> &str {
        &self.scene_name
    }

    pub fn toggle_pause(&mut self) -> bool {
        self.pause = !self.pause;
        if self.pause {
            self.pause_time = Some(Instant::now());
            true
        } else if let Some(pt) = self.pause_time.take() {
            let now = Instant::now();
            let elapsed = now.saturating_duration_since(pt);
            // Drop all spawn debt on resume. The next frame starts from this
            // instant and the smoothstep resume ramp reintroduces motion.
            self.last_spawn_time = now;
            self.spawn_remainder = 0.0;
            for d in &mut self.droplets {
                if d.is_alive {
                    d.increment_time(elapsed);
                    d.last_time = Some(now);
                    d.advance_remainder = 0.0;
                }
            }
            // Shift all atmospheric subsystem timers so they don't burst-fire
            // on the first tick after unpause (each sees a large elapsed).
            self.last_phosphor_time += elapsed;
            self.last_glitch_time += elapsed;
            self.next_glitch_time += elapsed;
            self.last_reseed_time += elapsed;
            self.color_ecosystem.last_tick += elapsed;
            self.atmosphere.last_tick += elapsed;
            self.memory.last_sample += elapsed;
            self.storytelling.last_tick += elapsed;
            if let Some(ref mut cd) = self.storytelling.cooldown_until {
                *cd += elapsed;
            }
            // Shift palette transition and profile interpolation timers
            // so they don't jump on resume. Without this, a transition in
            // progress during pause would see a large elapsed and instantly
            // complete, causing a visible visual discontinuity.
            if let Some(ref mut ts) = self.transition_start {
                *ts += elapsed;
            }
            if let Some(ref mut pt) = self.profile_transition_start {
                *pt += elapsed;
            }
            if let Some(ref mut ct) = self.charset_transition_start {
                *ct += elapsed;
            }
            // Initialize cinematic resume easing: simulation time scale ramps
            // from 0→1 over RESUME_EASE_DURATION_SECS using smoothstep S-curve.
            self.resume_blend = 0.0;
            self.resume_start = Some(now);
            true
        } else {
            true
        }
    }

    /// Returns whether a full redraw is pending. Used by tests to verify
    /// that ignored keys don't trigger destructive redraws.
    #[cfg(test)]
    pub fn is_force_draw_everything(&self) -> bool {
        self.force_draw_everything
    }

    /// Returns whether a semantic invalidation is pending. Used by tests to
    /// verify that ignored keys don't trigger frame invalidation.
    #[cfg(test)]
    pub fn is_semantic_invalidate(&self) -> bool {
        self.semantic_invalidate
    }

    /// Clear all pending redraw flags for test setup. After init_chars() and
    /// reset(), both semantic_invalidate and force_draw_everything are set.
    /// Tests that verify "key X does not trigger redraw" need these cleared
    /// first, or the assertion fails due to initialization residue rather
    /// than the tested key's behavior.
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
                    ch = if (is_top || is_bottom) && (is_left || is_right) {
                        '+'
                    } else if is_top || is_bottom {
                        '-'
                    } else if is_left || is_right {
                        '|'
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

        // Adaptive color: use palette last color (follows 'c' key cycling).
        // No white blend — message matches current rain color scheme.
        let fg = if self.color_mode == ColorMode::Mono {
            None
        } else {
            self.palette.colors.last().copied()
        };

        // Typewriter: reveal characters progressively.
        // Each char takes ~30ms to appear. Total reveal = chars * 30ms.
        // After full reveal, all chars stay visible.
        let reveal_count = if let Some(start) = self.message_start_time {
            let elapsed_ms = start.elapsed().as_millis() as usize;
            // 30ms per char, minimum 1 char on first frame
            let count = (elapsed_ms / 30).max(1);
            // Count only non-space content chars (border chars always visible)
            let mut content_total = 0usize;
            for mc in &self.message {
                if mc.val != ' ' && mc.val != '+' && mc.val != '-' && mc.val != '|' {
                    content_total += 1;
                }
            }
            count.min(content_total)
        } else {
            usize::MAX // no timer = show all immediately
        };

        let mut content_idx = 0usize;
        for mc in &self.message {
            let is_content = mc.val != ' ' && mc.val != '+' && mc.val != '-' && mc.val != '|';

            let (ch, cell_fg) = if is_content {
                if content_idx < reveal_count {
                    content_idx += 1;
                    (mc.val, fg)
                } else {
                    // Not yet revealed — show as space (invisible)
                    (' ', None)
                }
            } else {
                // Border chars: always visible with palette color
                (mc.val, fg)
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
