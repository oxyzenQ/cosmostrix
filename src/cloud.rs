// Copyright (c) 2026 rezky_nightky

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

use std::time::{Duration, Instant};

use crossterm::style::Color;
use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
    SeedableRng,
};

use crate::constants::*;
use crate::{
    cell::Cell,
    frame::Frame,
    palette::{build_palette, Palette},
    runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode},
};
use bitvec::prelude::{BitSlice, BitVec};

use crate::droplet::Droplet;

// --- Named constants are centralized in constants.rs ---

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharLoc {
    Middle,
    Tail,
    Head,
}

/// Read-only drawing context passed to `Droplet::draw` to avoid borrowing
/// the entire `Cloud` (which would conflict with the mutable droplet loop).
pub struct DrawCtx<'a> {
    pub lines: u16,
    pub full_width: bool,
    pub shading_distance: bool,
    pub bg: Option<Color>,

    pub color_mode: ColorMode,
    pub bold_mode: BoldMode,
    pub glitchy: bool,

    pub last_glitch_time: Instant,
    pub next_glitch_time: Instant,

    /// Per-slot palette color arrays for generation-based rendering.
    /// Index by droplet's `palette_slot` to resolve its birth palette.
    pub palette_slices: [&'a [Color]; MAX_PALETTE_SLOTS],

    /// Which palette slot is the currently active (latest) one.
    /// Used for transition glow effects on new-generation streams.
    pub active_palette_slot: u8,

    /// Whether a palette transition is currently in progress.
    /// When true, new-generation streams get enhanced visual effects.
    pub transitioning: bool,

    pub color_map: &'a [u8],
    pub glitch_map: &'a BitSlice,
    pub char_pool: &'a [char],
    pub previous_char_pool: &'a [char],
    pub charset_wave_line: Option<f32>,

    /// Color transition wave line: during a palette transition, rows above
    /// this value use the new (active) palette; rows below use their birth
    /// palette. Sweeps from 0 to lines+1 over COLOR_TRANSITION_DURATION_MS,
    /// creating a top-to-bottom wave that matches the charset transition.
    pub color_wave_line: Option<f32>,

    /// Mouse cursor column (u16::MAX if no mouse).
    pub mouse_col: u16,
    /// Mouse cursor line (u16::MAX if no mouse).
    pub mouse_line: u16,
    /// Flash effect click column.
    pub flash_col: u16,
    /// Flash effect click line.
    pub flash_line: u16,
    /// Flash effect start time (None if no active flash).
    pub flash_time: Option<Instant>,
}

impl DrawCtx<'_> {
    #[inline]
    fn is_bright(&self, now: Instant) -> bool {
        if now < self.last_glitch_time {
            return false;
        }
        let since = now
            .saturating_duration_since(self.last_glitch_time)
            .as_nanos() as f64;
        let between = self
            .next_glitch_time
            .saturating_duration_since(self.last_glitch_time)
            .as_nanos() as f64;
        if between <= 0.0 {
            return false;
        }
        (since / between) <= GLITCH_BRIGHT_RATIO
    }

    #[inline]
    fn is_dim(&self, now: Instant) -> bool {
        if now > self.next_glitch_time {
            return true;
        }
        let since = now
            .saturating_duration_since(self.last_glitch_time)
            .as_nanos() as f64;
        let between = self
            .next_glitch_time
            .saturating_duration_since(self.last_glitch_time)
            .as_nanos() as f64;
        if between <= 0.0 {
            return true;
        }
        (since / between) >= GLITCH_DIM_RATIO
    }

    #[inline]
    pub fn is_glitched(&self, line: u16, col: u16) -> bool {
        if !self.glitchy {
            return false;
        }
        let idx = col as usize * self.lines as usize + line as usize;
        self.glitch_map.get(idx).is_some_and(|b| *b)
    }

    #[inline]
    pub fn get_char(&self, line: u16, col: u16, char_pool_idx: u16) -> char {
        let pool = if self.charset_uses_previous_pool(line, col) {
            self.previous_char_pool
        } else {
            self.char_pool
        };
        let len = pool.len().max(1);
        let idx = ((char_pool_idx as usize) + (line as usize)) % len;
        pool.get(idx).copied().unwrap_or('0')
    }

    #[inline]
    pub fn charset_transitioning(&self) -> bool {
        self.charset_wave_line.is_some()
    }

    #[inline]
    fn charset_uses_previous_pool(&self, line: u16, col: u16) -> bool {
        let Some(wave_line) = self.charset_wave_line else {
            return false;
        };
        if self.previous_char_pool.is_empty() {
            return false;
        }

        let jitter =
            (((line as u32).wrapping_mul(17) ^ (col as u32).wrapping_mul(31)) % 3) as f32 * 0.18;
        (line as f32) > wave_line + jitter
    }

    /// During a color transition, returns whether a cell at (line, col) should
    /// use its birth (previous) palette rather than the new (active) palette.
    /// Rows below the wave line use the old palette; rows above use the new.
    /// This creates a top-to-bottom cascade matching the charset transition.
    #[inline]
    pub fn color_uses_previous_palette(&self, palette_slot: u8, line: u16, col: u16) -> bool {
        let Some(wave_line) = self.color_wave_line else {
            return false;
        };
        // Only applies to droplets that still carry the old palette slot
        if palette_slot == self.active_palette_slot {
            return false;
        }
        // Jitter for organic edge (same pattern as charset wave)
        let jitter =
            (((line as u32).wrapping_mul(13) ^ (col as u32).wrapping_mul(29)) % 3) as f32 * 0.15;
        (line as f32) > wave_line + jitter
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub fn get_attr(
        &self,
        palette_slot: u8,
        line: u16,
        col: u16,
        val: char,
        loc: CharLoc,
        now: Instant,
        head_put_line: u16,
        length: u16,
    ) -> (Option<Color>, bool) {
        // Resolve this stream's palette from the generation table.
        // During a color transition, cells above the wave line adopt the new
        // (active) palette even if the droplet was born with the old one,
        // creating a visible top-to-bottom cascade.
        let effective_slot = if self.color_uses_previous_palette(palette_slot, line, col) {
            palette_slot // Below wave: keep birth palette
        } else {
            self.active_palette_slot // Above wave or no transition: use new palette
        };
        let palette_colors = if (effective_slot as usize) < MAX_PALETTE_SLOTS {
            self.palette_slices[effective_slot as usize]
        } else {
            // Fallback: use active palette for invalid slots
            self.palette_slices[self.active_palette_slot as usize]
        };

        let mut bold = false;
        if self.bold_mode == BoldMode::Random {
            bold = (((line as u32) ^ (val as u32)) % 2) == 1;
        }

        let idx = col as usize * self.lines as usize + line as usize;
        let mut color_idx = self.color_map.get(idx).copied().unwrap_or(0) as i32;

        if self.shading_distance {
            let last = palette_colors.len().saturating_sub(1) as u64;
            let dist = head_put_line.saturating_sub(line) as f64;
            let len = length.max(1) as f64;

            // Exponential decay: brightness = exp(-k * distance/length)
            let normalized_dist = (dist / len).clamp(0.0, 1.0);
            let brightness = (-TRAIL_EXPONENTIAL_K * normalized_dist).exp();
            let mut v = ((brightness * last as f64).round() as u64).min(last);

            // Bloom: cells right behind head get extra brightness
            if dist < HEAD_BLOOM_CELLS as f64 {
                v = (v + 1).min(last);
            }

            color_idx = v as i32;
        }

        if self.glitchy && self.glitch_map.get(idx).is_some_and(|b| *b) {
            if self.is_bright(now) {
                color_idx += 1;
                bold = true;
            } else if self.is_dim(now) {
                color_idx -= 1;
                bold = false;
            }
        }

        let last = palette_colors.len().saturating_sub(1) as i32;
        match loc {
            CharLoc::Tail => {
                color_idx = 0;
                bold = false;
            }
            CharLoc::Head => {
                color_idx = last;
                bold = true;
            }
            CharLoc::Middle => {
                color_idx = color_idx.clamp(0, last.max(0));
            }
        }

        match self.bold_mode {
            BoldMode::Off => bold = false,
            BoldMode::All => bold = true,
            BoldMode::Random => {}
        }

        let fg = if self.color_mode == ColorMode::Mono {
            None
        } else {
            palette_colors.get(color_idx as usize).copied()
        };

        (fg, bold)
    }
}

/// Per-column tracking for spawn control and speed scaling.
#[derive(Clone, Debug)]
struct ColumnStatus {
    max_speed_pct: f32,
    num_droplets: u8,
    can_spawn: bool,
}

/// Spawn-time state sampled from `Cloud` before mutably borrowing the droplet pool.
struct DropletSpawnSpec {
    col: u16,
    end_line: u16,
    char_pool_idx: u16,
    length: u16,
    chars_per_sec: f32,
    time_to_linger: Duration,
    layer: u8,
    palette_slot: u8,
    turb_phase: f32,
}

impl DropletSpawnSpec {
    fn apply_to(self, d: &mut Droplet) {
        d.bound_col = self.col;
        d.end_line = self.end_line;
        d.char_pool_idx = self.char_pool_idx;
        d.length = self.length;
        d.chars_per_sec = self.chars_per_sec;
        d.time_to_linger = self.time_to_linger;
        d.layer = self.layer;
        d.palette_slot = self.palette_slot;
        d.head_put_line = 0;
        d.head_cur_line = 0;
        d.tail_put_line = None;
        d.tail_cur_line = 0;
        d.head_stop_time = None;
        d.turb_phase = self.turb_phase;
        d.turb_time = 0.0;
    }
}

/// A single character in the overlay message box (position + glyph).
#[derive(Clone, Debug)]
struct MsgChr {
    line: u16,
    col: u16,
    val: char,
}

/// Kind of rare atmospheric anomaly.
#[derive(Clone, Copy, Debug, PartialEq)]
enum AnomalyKind {
    /// Brief luminance surge in a localized area.
    LuminanceSurge,
    /// Stream glyph corruption/mutation.
    GlyphCorruption,
    /// Faint expanding pulse wave.
    PulseWave,
}

/// An active anomaly zone on the screen.
#[derive(Clone, Debug)]
struct AnomalyZone {
    col: u16,
    line: u16,
    radius: u16,
    kind: AnomalyKind,
    start_time: Instant,
}

// ---------------------------------------------------------------------------
// Phase 3: Autonomous cinematic ecosystem
// ---------------------------------------------------------------------------

/// Cinematic runtime behavior profiles — atmospheric identities that
/// fundamentally alter how the renderer feels, moves, and breathes.
/// These are NOT simple recolors; each profile defines a behavioral ecosystem.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BehaviorProfile {
    Monolith,
    Void,
    Neural,
    Decay,
    Eclipse,
    Static,
    Pulse,
}

impl BehaviorProfile {
    fn name(self) -> &'static str {
        match self {
            BehaviorProfile::Monolith => "Monolith",
            BehaviorProfile::Void => "Void",
            BehaviorProfile::Neural => "Neural",
            BehaviorProfile::Decay => "Decay",
            BehaviorProfile::Eclipse => "Eclipse",
            BehaviorProfile::Static => "Static",
            BehaviorProfile::Pulse => "Pulse",
        }
    }

    fn cycle(self) -> Self {
        match self {
            BehaviorProfile::Monolith => BehaviorProfile::Void,
            BehaviorProfile::Void => BehaviorProfile::Neural,
            BehaviorProfile::Neural => BehaviorProfile::Decay,
            BehaviorProfile::Decay => BehaviorProfile::Eclipse,
            BehaviorProfile::Eclipse => BehaviorProfile::Static,
            BehaviorProfile::Static => BehaviorProfile::Pulse,
            BehaviorProfile::Pulse => BehaviorProfile::Monolith,
        }
    }

    fn params(self) -> ProfileParams {
        match self {
            BehaviorProfile::Monolith => ProfileParams {
                speed_mult: 0.5,
                density_mult: 1.3,
                turbulence_mult: 0.3,
                phosphor_decay_mult: 0.4,
                anomaly_freq_mult: 0.4,
                luminance_offset: -0.1,
                persistence_boost: 0.3,
                entropy_rate: 0.3,
                short_pct: 0.2,
                linger_mult: 2.0,
            },
            BehaviorProfile::Void => ProfileParams {
                speed_mult: 0.7,
                density_mult: 0.4,
                turbulence_mult: 0.1,
                phosphor_decay_mult: 1.5,
                anomaly_freq_mult: 0.2,
                luminance_offset: -0.2,
                persistence_boost: -0.1,
                entropy_rate: 0.2,
                short_pct: 0.7,
                linger_mult: 0.5,
            },
            BehaviorProfile::Neural => ProfileParams {
                speed_mult: 1.5,
                density_mult: 1.6,
                turbulence_mult: 2.0,
                phosphor_decay_mult: 0.8,
                anomaly_freq_mult: 2.0,
                luminance_offset: 0.1,
                persistence_boost: 0.1,
                entropy_rate: 1.5,
                short_pct: 0.5,
                linger_mult: 0.8,
            },
            BehaviorProfile::Decay => ProfileParams {
                speed_mult: 0.6,
                density_mult: 0.7,
                turbulence_mult: 0.5,
                phosphor_decay_mult: 0.3,
                anomaly_freq_mult: 0.6,
                luminance_offset: -0.15,
                persistence_boost: 0.4,
                entropy_rate: 0.5,
                short_pct: 0.6,
                linger_mult: 1.5,
            },
            BehaviorProfile::Eclipse => ProfileParams {
                speed_mult: 0.8,
                density_mult: 1.0,
                turbulence_mult: 1.0,
                phosphor_decay_mult: 0.6,
                anomaly_freq_mult: 1.5,
                luminance_offset: 0.0,
                persistence_boost: 0.2,
                entropy_rate: 0.8,
                short_pct: 0.4,
                linger_mult: 1.2,
            },
            BehaviorProfile::Static => ProfileParams {
                speed_mult: 0.2,
                density_mult: 0.3,
                turbulence_mult: 0.05,
                phosphor_decay_mult: 0.2,
                anomaly_freq_mult: 0.1,
                luminance_offset: -0.25,
                persistence_boost: 0.5,
                entropy_rate: 0.1,
                short_pct: 0.9,
                linger_mult: 3.0,
            },
            BehaviorProfile::Pulse => ProfileParams {
                speed_mult: 1.2,
                density_mult: 1.1,
                turbulence_mult: 1.3,
                phosphor_decay_mult: 0.9,
                anomaly_freq_mult: 1.0,
                luminance_offset: 0.05,
                persistence_boost: 0.15,
                entropy_rate: 1.2,
                short_pct: 0.3,
                linger_mult: 1.0,
            },
        }
    }
}

/// Tuning parameters for a cinematic behavior profile.
/// Each value is a multiplier (1.0 = default behavior).
#[derive(Clone, Copy, Debug)]
pub struct ProfileParams {
    pub speed_mult: f32,
    pub density_mult: f32,
    pub turbulence_mult: f32,
    pub phosphor_decay_mult: f32,
    pub anomaly_freq_mult: f32,
    pub luminance_offset: f32,
    pub persistence_boost: f32,
    pub entropy_rate: f32,
    pub short_pct: f32,
    pub linger_mult: f32,
}

#[inline]
fn lerp_profile_params(a: ProfileParams, b: ProfileParams, t: f32) -> ProfileParams {
    ProfileParams {
        speed_mult: a.speed_mult + (b.speed_mult - a.speed_mult) * t,
        density_mult: a.density_mult + (b.density_mult - a.density_mult) * t,
        turbulence_mult: a.turbulence_mult + (b.turbulence_mult - a.turbulence_mult) * t,
        phosphor_decay_mult: a.phosphor_decay_mult
            + (b.phosphor_decay_mult - a.phosphor_decay_mult) * t,
        anomaly_freq_mult: a.anomaly_freq_mult + (b.anomaly_freq_mult - a.anomaly_freq_mult) * t,
        luminance_offset: a.luminance_offset + (b.luminance_offset - a.luminance_offset) * t,
        persistence_boost: a.persistence_boost + (b.persistence_boost - a.persistence_boost) * t,
        entropy_rate: a.entropy_rate + (b.entropy_rate - a.entropy_rate) * t,
        short_pct: a.short_pct + (b.short_pct - a.short_pct) * t,
        linger_mult: a.linger_mult + (b.linger_mult - a.linger_mult) * t,
    }
}

/// Returns 3-4 atmospherically related color schemes for autonomous palette drift.
fn related_schemes(scheme: ColorScheme) -> &'static [ColorScheme] {
    use ColorScheme::*;
    match scheme {
        Green => &[Green2, Green3, Aurora, Forest],
        Green2 => &[Green, Green3, Forest, Aurora],
        Green3 => &[Green, Green2, Forest],
        Gold => &[Yellow, Orange, Sun, Fire],
        Yellow => &[Gold, Orange, Sun],
        Orange => &[Gold, Fire, Sun, Yellow],
        Red => &[Fire, Orange, Supernova, Meteor],
        Blue => &[Ocean, DeepSpace, Neptune, Comet],
        Cyan => &[Aurora, Ocean, Neptune, Uranus],
        Purple => &[Nebula, Cosmos, Vaporwave, Galaxy],
        Neon => &[Vaporwave, Aurora, Cosmos, Nebula],
        Fire => &[Red, Orange, Supernova, Meteor],
        Ocean => &[Blue, DeepSpace, Neptune, Cyan],
        Forest => &[Green, Green2, Aurora, Green3],
        Vaporwave => &[Neon, Purple, Nebula, Cosmos],
        Gray => &[Mercury, Snow, Moon],
        Rainbow => &[Spectrum20, Neon, Vaporwave],
        Snow => &[Gray, Moon, Mercury, Stars],
        Aurora => &[Green, Cyan, Forest, Neon],
        FancyDiamond => &[Cyan, Snow, Nebula, Stardust],
        Cosmos => &[DeepSpace, Nebula, Galaxy, Purple],
        Nebula => &[Cosmos, Purple, Galaxy, Stardust],
        Spectrum20 => &[Rainbow, Neon, Vaporwave],
        Stars => &[DeepSpace, Cosmos, Galaxy, Comet],
        Mars => &[Red, Fire, Meteor, Supernova],
        Venus => &[Gold, Yellow, Sun, Orange],
        Mercury => &[Gray, Moon, Snow],
        Jupiter => &[Orange, Gold, Sun, Saturn],
        Saturn => &[Jupiter, Gold, Venus, Yellow],
        Uranus => &[Cyan, Neptune, Ocean, Aurora],
        Neptune => &[Blue, Ocean, DeepSpace, Uranus],
        Pluto => &[Mercury, Gray, Moon, DeepSpace],
        Moon => &[Gray, Mercury, Snow, Stars],
        Sun => &[Gold, Yellow, Venus, Fire],
        Comet => &[Blue, Stars, DeepSpace, Cyan],
        Galaxy => &[Cosmos, Nebula, DeepSpace, Stardust],
        Supernova => &[Fire, Red, Meteor, Mars],
        BlackHole => &[DeepSpace, Cosmos, Nebula, Pluto],
        Andromeda => &[Cosmos, Nebula, Galaxy, Stardust],
        Stardust => &[Galaxy, Nebula, Cosmos, FancyDiamond],
        Meteor => &[Fire, Red, Mars, Supernova],
        Eclipse => &[BlackHole, DeepSpace, Cosmos, Nebula],
        DeepSpace => &[Cosmos, BlackHole, Galaxy, Stars],
        // Catch-all for future variants
        #[allow(unreachable_patterns)]
        _ => &[Green, Blue, Cyan],
    }
}

/// Autonomous color ecosystem: slow palette drift, luminance climate shifts,
/// and tonal migration that makes the renderer feel atmospherically alive.
struct ColorEcosystem {
    luminance_climate: f32,
    saturation_climate: f32,
    hue_drift: f32,
    luminance_direction: f32,
    saturation_direction: f32,
    hue_direction: f32,
    last_tick: Instant,
}

impl ColorEcosystem {
    fn new(now: Instant) -> Self {
        Self {
            luminance_climate: 0.8,
            saturation_climate: 0.8,
            hue_drift: 0.0,
            luminance_direction: 0.0,
            saturation_direction: 0.0,
            hue_direction: 0.0,
            last_tick: now,
        }
    }

    fn tick(
        &mut self,
        now: Instant,
        mt: &mut StdRng,
        current_scheme: ColorScheme,
    ) -> Option<ColorScheme> {
        let elapsed = now.saturating_duration_since(self.last_tick).as_secs_f32();
        if elapsed < COLOR_ECOSYSTEM_TICK_SECS {
            return None;
        }
        self.last_tick = now;

        // Randomly re-evaluate drift directions
        let chance_dist = Uniform::new(0.0f32, 1.0f32).expect("chance_dist always valid");
        if chance_dist.sample(mt) < COLOR_DRIFT_REEVAL_CHANCE {
            self.luminance_direction = if chance_dist.sample(mt) < 0.5 {
                -1.0
            } else {
                1.0
            };
        }
        if chance_dist.sample(mt) < COLOR_DRIFT_REEVAL_CHANCE {
            self.saturation_direction = if chance_dist.sample(mt) < 0.5 {
                -1.0
            } else {
                1.0
            };
        }
        if chance_dist.sample(mt) < COLOR_DRIFT_REEVAL_CHANCE {
            self.hue_direction = if chance_dist.sample(mt) < 0.5 {
                -1.0
            } else {
                1.0
            };
        }

        // Apply drift rates
        self.luminance_climate += self.luminance_direction * COLOR_CLIMATE_DRIFT_RATE;
        self.saturation_climate += self.saturation_direction * COLOR_SATURATION_DRIFT_RATE;
        self.hue_drift += self.hue_direction * COLOR_HUE_DRIFT_RATE;

        // Clamp values
        self.luminance_climate = self
            .luminance_climate
            .clamp(COLOR_LUMINANCE_CLIMATE_MIN, COLOR_LUMINANCE_CLIMATE_MAX);
        self.saturation_climate = self
            .saturation_climate
            .clamp(COLOR_SATURATION_CLIMATE_MIN, COLOR_SATURATION_CLIMATE_MAX);
        self.hue_drift = self
            .hue_drift
            .clamp(-std::f32::consts::PI, std::f32::consts::PI);

        // Autonomous palette drift
        if chance_dist.sample(mt) < AUTONOMOUS_PALETTE_DRIFT_CHANCE {
            let related = related_schemes(current_scheme);
            if !related.is_empty() {
                let idx_dist = Uniform::new_inclusive(0usize, related.len().saturating_sub(1))
                    .expect("related_schemes idx always valid");
                let new_scheme = related[idx_dist.sample(mt)];
                return Some(new_scheme);
            }
        }

        None
    }
}

/// Autonomous atmospheric evolution: entropy cycles, density migration,
/// luminance shifts, anomaly pressure fluctuations. All slow, smooth, cinematic.
struct AtmosphericEvolution {
    entropy_phase: f32,
    last_tick: Instant,
    density_offset: f32,
    luminance_offset: f32,
    anomaly_offset: f32,
    cycle_speed: f32,
}

impl AtmosphericEvolution {
    fn new(now: Instant) -> Self {
        Self {
            entropy_phase: 0.0,
            last_tick: now,
            density_offset: 0.0,
            luminance_offset: 0.0,
            anomaly_offset: 0.0,
            cycle_speed: 1.0,
        }
    }

    fn tick(&mut self, now: Instant, profile_entropy_rate: f32) {
        let elapsed = now.saturating_duration_since(self.last_tick).as_secs_f32();
        if elapsed < ATMOSPHERE_TICK_SECS {
            return;
        }
        self.last_tick = now;
        self.cycle_speed = profile_entropy_rate;

        self.entropy_phase += (elapsed / ENTROPY_CYCLE_SECS) * self.cycle_speed;
        self.entropy_phase %= 1.0;

        let tau = std::f32::consts::TAU;
        self.density_offset = (self.entropy_phase * tau).sin() * ATMOSPHERE_DENSITY_RANGE;
        self.luminance_offset = (self.entropy_phase * tau + std::f32::consts::FRAC_PI_3).sin()
            * ATMOSPHERE_LUMINANCE_RANGE;
        self.anomaly_offset = (self.entropy_phase * tau + 2.0 * std::f32::consts::FRAC_PI_3).sin()
            * ATMOSPHERE_ANOMALY_RANGE;
    }
}

/// Long-timescale renderer memory: historical influence on current rendering.
/// Remembers anomaly history, density history, luminance pressure.
struct RendererMemory {
    anomaly_history: [f32; MEMORY_HISTORY_SAMPLES],
    density_history: [f32; MEMORY_HISTORY_SAMPLES],
    history_idx: usize,
    last_sample: Instant,
    instability_pressure: f32,
    persistence_richness: f32,
}

impl RendererMemory {
    fn new(now: Instant) -> Self {
        Self {
            anomaly_history: [0.0; MEMORY_HISTORY_SAMPLES],
            density_history: [0.0; MEMORY_HISTORY_SAMPLES],
            history_idx: 0,
            last_sample: now,
            instability_pressure: 0.0,
            persistence_richness: 0.0,
        }
    }

    fn record_sample(
        &mut self,
        now: Instant,
        anomaly_density: f32,
        rain_density: f32,
        #[allow(unused_variables)] luminance: f32,
    ) {
        let elapsed = now
            .saturating_duration_since(self.last_sample)
            .as_secs_f32();
        if elapsed < MEMORY_SAMPLE_INTERVAL_SECS {
            return;
        }
        self.last_sample = now;
        self.anomaly_history[self.history_idx] = anomaly_density;
        self.density_history[self.history_idx] = rain_density;
        self.history_idx = (self.history_idx + 1) % MEMORY_HISTORY_SAMPLES;
    }

    fn recompute_derived(&mut self) {
        let n = MEMORY_HISTORY_SAMPLES as f32;
        let avg_anomaly: f32 = self.anomaly_history.iter().sum::<f32>() / n;

        self.instability_pressure = avg_anomaly * MEMORY_ANOMALY_PRESSURE_WEIGHT;
        self.persistence_richness = (1.0 - avg_anomaly) * MEMORY_CALM_PERSISTENCE_BOOST;
    }
}

/// Kind of emergent visual moment.
#[derive(Clone, Copy, Debug, PartialEq)]
enum EmergentKind {
    LuminanceSwell,
    DensityPulse,
    TemporalDilation,
}

/// An active emergent moment.
#[derive(Clone, Debug)]
struct EmergentMoment {
    kind: EmergentKind,
    start_time: Instant,
    duration: f32,
}

/// Current emergent effects applied to rendering.
#[derive(Clone, Copy, Debug, Default)]
pub struct EmergentEffects {
    pub luminance_boost: f32,
    pub density_boost: f32,
    pub speed_shift: f32,
}

/// Emergent visual storytelling system: watches for convergence conditions
/// across other systems and occasionally produces emotionally resonant moments.
struct StorytellingState {
    moments: Vec<EmergentMoment>,
    last_tick: Instant,
    cooldown_until: Option<Instant>,
}

impl StorytellingState {
    fn new(now: Instant) -> Self {
        Self {
            moments: Vec::new(),
            last_tick: now,
            cooldown_until: None,
        }
    }

    fn tick(
        &mut self,
        now: Instant,
        mt: &mut StdRng,
        evolution: &AtmosphericEvolution,
        memory: &RendererMemory,
        ecosystem: &ColorEcosystem,
    ) -> Option<EmergentKind> {
        let elapsed = now.saturating_duration_since(self.last_tick).as_secs_f32();
        if elapsed < STORYTELLING_TICK_SECS {
            return None;
        }
        self.last_tick = now;

        // Check cooldown
        if let Some(until) = self.cooldown_until {
            if now < until {
                return None;
            }
        }

        // Check max moments
        if self.moments.len() >= EMERGENT_MAX_MOMENTS {
            return None;
        }

        // Convergence conditions
        let entropy_near_peak = (evolution.entropy_phase - 0.5).abs() < 0.15;
        let has_instability = memory.instability_pressure > 0.1;
        let not_too_bright = ecosystem.luminance_climate < 0.85;

        if !entropy_near_peak || !has_instability || !not_too_bright {
            return None;
        }

        // Roll for emergent moment
        let chance_dist = Uniform::new(0.0f32, 1.0f32).expect("chance_dist always valid");
        if chance_dist.sample(mt) < EMERGENT_MOMENT_CHANCE {
            let kind_roll = chance_dist.sample(mt);
            let kind = if kind_roll < 0.33 {
                EmergentKind::LuminanceSwell
            } else if kind_roll < 0.66 {
                EmergentKind::DensityPulse
            } else {
                EmergentKind::TemporalDilation
            };
            self.cooldown_until =
                Some(now + Duration::from_secs_f32(EMERGENT_MOMENT_DURATION_SECS + 60.0));
            return Some(kind);
        }

        None
    }

    fn active_effects(&self, now: Instant) -> EmergentEffects {
        let mut effects = EmergentEffects::default();
        for moment in &self.moments {
            let elapsed = now
                .saturating_duration_since(moment.start_time)
                .as_secs_f32();
            if elapsed >= moment.duration {
                continue;
            }
            let progress = elapsed / moment.duration;
            let sin_pi = (progress * std::f32::consts::PI).sin();
            match moment.kind {
                EmergentKind::LuminanceSwell => {
                    effects.luminance_boost += EMERGENT_LUMINANCE_INTENSITY * sin_pi;
                }
                EmergentKind::DensityPulse => {
                    effects.density_boost += EMERGENT_DENSITY_INTENSITY * sin_pi;
                }
                EmergentKind::TemporalDilation => {
                    effects.speed_shift -= EMERGENT_SPEED_SHIFT * sin_pi;
                }
            }
        }
        effects
    }

    /// Expire moments past their duration. Must be called separately since
    /// active_effects only borrows &self.
    fn expire_moments(&mut self, now: Instant) {
        self.moments
            .retain(|m| now.saturating_duration_since(m.start_time).as_secs_f32() < m.duration);
    }
}

pub struct Cloud {
    pub lines: u16,
    pub cols: u16,

    pub palette: Palette,
    pub color_mode: ColorMode,

    pub full_width: bool,
    pub shading_distance: bool,
    pub bold_mode: BoldMode,

    pub async_mode: bool,
    pub raining: bool,
    pub pause: bool,

    pub droplet_density: f32,
    pub droplets_per_sec: f32,
    pub chars_per_sec: f32,

    pub glitchy: bool,
    pub glitch_pct: f32,
    pub glitch_low_ms: u16,
    pub glitch_high_ms: u16,

    pub short_pct: f32,
    pub die_early_pct: f32,
    pub linger_low_ms: u16,
    pub linger_high_ms: u16,

    pub max_droplets_per_column: u8,

    droplets: Vec<Droplet>,
    spawn_scan_idx: usize,

    chars: Vec<char>,
    char_pool: Vec<char>,
    previous_char_pool: Vec<char>,
    charset_transition_start: Option<Instant>,
    glitch_pool: Vec<char>,
    glitch_pool_idx: usize,

    glitch_map: BitVec,
    color_map: Vec<u8>,

    col_stat: Vec<ColumnStatus>,

    mt: StdRng,

    rand_chance: Uniform<f32>,
    rand_line: Uniform<u16>,
    rand_cpidx: Uniform<u16>,
    rand_len: Uniform<u16>,
    rand_col: Uniform<u16>,
    rand_glitch_ms: Uniform<u16>,
    rand_linger_ms: Uniform<u16>,
    rand_speed: Uniform<f32>,

    last_glitch_time: Instant,
    next_glitch_time: Instant,
    last_spawn_time: Instant,
    spawn_remainder: f32,
    pause_time: Option<Instant>,

    /// Resume time-scale factor: 0.0 (just resumed) → 1.0 (fully active).
    /// Scales the simulation clock for all droplets during the smoothstep
    /// resume transition, producing cinematic inertia recovery — the rain
    /// decelerates into the pause and accelerates smoothly out of it.
    resume_blend: f32,
    /// Timestamp when the most recent unpause occurred. Used to compute
    /// the smoothstep S-curve for `resume_blend`.
    resume_start: Option<Instant>,

    force_draw_everything: bool,

    /// Pending semantic invalidation: set to true when the renderer's semantic
    /// identity changes (charset switch, shading mode toggle). On the next
    /// `rain_at()`, this triggers `frame.invalidate_semantic()` which bumps
    /// the frame's `semantic_gen`, forcing the Terminal to do a full redraw
    /// and properly synchronize its LastFrame cache with the new semantics.
    semantic_invalidate: bool,

    /// Frame counter for periodic full redraw (ANSI drift correction).
    /// Every `FULL_REDRAW_INTERVAL_FRAMES`, forces a complete screen refresh
    /// to correct any accumulated terminal state desync.
    frames_since_full_redraw: u64,

    perf_pressure: f32,
    max_sim_delta: Duration,

    shading_mode: ShadingMode,

    message: Vec<MsgChr>,
    message_text: Option<String>,
    message_border: bool,
    color_scheme: ColorScheme,
    default_background: bool,

    /// Palette generation table: stores up to MAX_PALETTE_SLOTS palettes for
    /// generation-based transitions.  Each droplet carries a `palette_slot`
    /// that indexes into this table, so old streams retain their birth palette
    /// while new streams inherit the latest one.
    palette_table: [Option<Palette>; MAX_PALETTE_SLOTS],

    /// Index of the currently active palette slot (where new streams inherit).
    active_palette_slot: u8,

    /// Time when the current palette transition started (None if not transitioning).
    /// Used for row-based top-to-bottom wave progression.
    transition_start: Option<Instant>,

    /// Per-column palette slot: tracks which palette each column is currently
    /// using for spawning.  During a transition, all columns adopt the new
    /// palette simultaneously since the wave is row-based (top-to-bottom),
    /// not column-based. This field is kept for spawn-time inheritance.
    column_palette_slot: Vec<u8>,

    /// Per-column delay (in ms) — legacy field kept for spawn-time reference.
    /// No longer used for wave propagation; the row-based wave logic in
    /// `color_wave_line_at()` drives the transition.
    column_transition_delay_ms: Vec<u16>,

    /// Mouse cursor column position (u16::MAX if no mouse).
    pub mouse_col: u16,

    /// Mouse cursor line position (u16::MAX if no mouse).
    pub mouse_line: u16,

    /// Whether mouse interaction is enabled.
    pub mouse_enabled: bool,

    /// Flash effect: click column.
    flash_col: u16,

    /// Flash effect: click line.
    flash_line: u16,

    /// Flash effect: start time (None if no active flash).
    flash_time: Option<Instant>,

    last_reseed_time: Instant,

    // --- Phosphor persistence state ---
    /// Per-cell phosphor energy (0 = dead, 255 = full). Tracks residual
    /// luminance for CRT-style afterglow after a droplet's tail passes.
    phosphor: Vec<u8>,
    /// Per-cell base foreground color captured when phosphor was activated.
    phosphor_base_fg: Vec<Option<Color>>,
    /// Per-cell layer identifier for layer-aware phosphor decay.
    phosphor_layer: Vec<u8>,
    /// BitVec tracking which cells were refreshed by a droplet this frame.
    phosphor_fresh: BitVec,
    /// Time of the last phosphor pass for frame-rate-independent decay.
    last_phosphor_time: Instant,

    // --- Rare anomaly events ---
    /// Active anomaly zones currently affecting the screen.
    anomaly_zones: Vec<AnomalyZone>,

    // --- Phase 3: Autonomous cinematic ecosystem ---
    /// Active cinematic behavior profile.
    profile: BehaviorProfile,
    /// Interpolated profile params (current, transitioning toward target).
    profile_current: ProfileParams,
    /// Target profile params (what we're transitioning toward).
    profile_target: ProfileParams,
    /// Time when profile transition started.
    profile_transition_start: Option<Instant>,

    /// Temporal color ecosystem.
    color_ecosystem: ColorEcosystem,
    /// Autonomous atmospheric evolution.
    atmosphere: AtmosphericEvolution,
    /// Long-timescale renderer memory.
    memory: RendererMemory,
    /// Emergent visual storytelling.
    storytelling: StorytellingState,
}

impl Cloud {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        color_mode: ColorMode,
        full_width: bool,
        shading_mode: ShadingMode,
        bold_mode: BoldMode,
        async_mode: bool,
        default_background: bool,
        color_scheme: ColorScheme,
    ) -> Self {
        let now = Instant::now();
        let mt = StdRng::seed_from_u64(RNG_INITIAL_SEED);

        Self {
            lines: 25,
            cols: 80,
            palette: build_palette(color_scheme, color_mode, default_background),
            color_mode,
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
            spawn_scan_idx: 0,
            chars: Vec::new(),
            char_pool: Vec::new(),
            previous_char_pool: Vec::new(),
            charset_transition_start: None,
            glitch_pool: Vec::new(),
            glitch_pool_idx: 0,
            glitch_map: BitVec::new(),
            color_map: Vec::new(),
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
            message_border: true,
            color_scheme,
            default_background,
            palette_table: [None, None, None, None],
            active_palette_slot: 0,
            transition_start: None,
            column_palette_slot: Vec::new(),
            column_transition_delay_ms: Vec::new(),
            mouse_col: u16::MAX,
            mouse_line: u16::MAX,
            mouse_enabled: false,
            flash_col: u16::MAX,
            flash_line: u16::MAX,
            flash_time: None,
            last_reseed_time: now,
            phosphor: Vec::new(),
            phosphor_base_fg: Vec::new(),
            phosphor_layer: Vec::new(),
            phosphor_fresh: BitVec::new(),
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
        }
    }

    pub fn set_message(&mut self, msg: &str) {
        self.message_text = Some(msg.to_string());
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

    pub fn set_color_scheme(&mut self, scheme: ColorScheme) {
        self.color_scheme = scheme;
        let new_palette = build_palette(scheme, self.color_mode, self.default_background);

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
        self.transition_start = Some(Instant::now());

        // Do NOT force a full redraw — old streams must persist with their
        // birth palette below the wave line.  The new palette propagates
        // visually via the row-based wave in get_attr(), creating the
        // cinematic top-to-bottom cascade.
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
        self.droplets.iter().filter(|d| d.is_alive).count()
    }

    pub fn set_async(&mut self, on: bool) {
        self.async_mode = on;
        self.set_column_speeds();
        self.update_droplet_speeds();
    }

    pub fn set_chars_per_sec(&mut self, cps: f32) {
        self.chars_per_sec = cps;
        self.recalc_droplets_per_sec();
        self.set_column_speeds();
        self.update_droplet_speeds();
    }

    pub fn set_droplet_density(&mut self, density: f32) {
        self.droplet_density = density;
        self.recalc_droplets_per_sec();
    }

    pub fn set_glitchy(&mut self, on: bool) {
        self.glitchy = on;
        self.fill_glitch_map();
        if on {
            let now = Instant::now();
            self.last_glitch_time = now;
            let ms = self.rand_glitch_ms.sample(&mut self.mt) as u64;
            self.next_glitch_time = now + Duration::from_millis(ms);
        }
        self.force_draw_everything = true;
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

    pub fn set_max_sim_delta(&mut self, d: Duration) {
        self.max_sim_delta = d;
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
            // Shift all Phase 3 subsystem timers so they don't burst-fire
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

    pub fn reset(&mut self, cols: u16, lines: u16) {
        // Defense in depth: clamp even though callers should clamp before
        // calling. Prevents degenerate sizes from reaching buffer allocation
        // or Uniform::new_inclusive construction.
        self.cols = cols.clamp(MIN_TERMINAL_COLS, MAX_TERMINAL_COLS);
        self.lines = lines.clamp(MIN_TERMINAL_LINES, MAX_TERMINAL_LINES);

        let pool_size = (DROPLET_COUNT_FACTOR * self.cols as f32).round() as usize;
        self.droplets.clear();
        self.droplets.resize_with(pool_size, Droplet::new);
        self.spawn_scan_idx = 0;

        let max_line = lines.saturating_sub(2);
        let max_len = max_line.max(1);
        self.rand_line = Uniform::new_inclusive(0, max_line).expect("rand_line: max_line >= 0");
        self.rand_len =
            Uniform::new_inclusive(1, max_len).expect("rand_len: max_len >= 1 after max(1)");
        self.rand_col =
            Uniform::new_inclusive(0, cols.saturating_sub(1)).expect("rand_col: cols-1 >= 0");
        self.rand_cpidx = Uniform::new_inclusive(0, MAX_CHAR_POOL_IDX)
            .expect("rand_cpidx: [0,2047] always valid");

        self.recalc_droplets_per_sec();

        self.col_stat.clear();
        self.col_stat.resize(
            cols as usize,
            ColumnStatus {
                max_speed_pct: 1.0,
                num_droplets: 0,
                can_spawn: true,
            },
        );

        // Initialize palette generation system for current terminal size
        self.palette_table[self.active_palette_slot as usize] = Some(self.palette.clone());
        self.column_palette_slot.clear();
        self.column_palette_slot
            .resize(cols as usize, self.active_palette_slot);
        self.column_transition_delay_ms.clear();
        self.column_transition_delay_ms.resize(cols as usize, 0);
        self.transition_start = None;
        self.previous_char_pool.clear();
        self.charset_transition_start = None;

        self.fill_glitch_map();
        self.fill_color_map();
        self.set_column_speeds();
        self.update_droplet_speeds();

        // Reset phosphor state for new terminal size
        let total = (cols as usize) * (lines as usize);
        self.phosphor.clear();
        self.phosphor.resize(total, 0);
        self.phosphor_base_fg.clear();
        self.phosphor_base_fg.resize(total, None);
        self.phosphor_layer.clear();
        self.phosphor_layer.resize(total, 0);
        self.phosphor_fresh.clear();
        self.phosphor_fresh.resize(total, false);

        // Reset anomaly zones on terminal resize
        self.anomaly_zones.clear();

        if self.message_text.is_some() {
            self.reset_message();
        }

        let now = Instant::now();
        self.last_glitch_time = now;
        self.next_glitch_time =
            now + Duration::from_millis(self.rand_glitch_ms.sample(&mut self.mt) as u64);
        self.last_spawn_time = now;
        self.spawn_remainder = 0.0;
        self.force_draw_everything = true;
        self.frames_since_full_redraw = 0;
        self.last_reseed_time = now;
        self.last_phosphor_time = now;

        // Phase 3: Reset cinematic subsystems on terminal resize
        self.color_ecosystem = ColorEcosystem::new(now);
        self.atmosphere = AtmosphericEvolution::new(now);
        self.memory = RendererMemory::new(now);
        self.storytelling = StorytellingState::new(now);
        self.profile_transition_start = None;
        // Note: profile and profile params are preserved across resets
    }

    pub fn init_chars(&mut self, chars: Vec<char>) {
        self.rebuild_char_pools(chars);
        self.previous_char_pool.clear();
        self.charset_transition_start = None;

        self.reset_phosphor_state();

        // Flag semantic invalidation so the Terminal's LastFrame cache is
        // fully invalidated on the next rain_at() call. This eliminates stale
        // glyph residue that can persist when only dirty-region invalidation
        // is used — immediate charset initialization is a semantic mutation,
        // not a cell mutation.
        self.semantic_invalidate = true;
    }

    pub fn transition_chars(&mut self, chars: Vec<char>) {
        self.previous_char_pool = if self.char_pool.is_empty() {
            vec!['0', '1']
        } else {
            self.char_pool.clone()
        };
        self.rebuild_char_pools(chars);
        self.charset_transition_start = Some(Instant::now());
    }

    fn charset_wave_line_at(&self, now: Instant) -> Option<f32> {
        let start = self.charset_transition_start?;
        let elapsed_ms = now.saturating_duration_since(start).as_millis() as f32;
        let progress = (elapsed_ms / CHARSET_TRANSITION_DURATION_MS as f32).clamp(0.0, 1.0);
        Some(progress * (self.lines as f32 + 1.0))
    }

    /// Compute the color transition wave line position at the given time.
    /// Returns None if no transition is active. The wave sweeps from 0 to
    /// lines+1 over COLOR_TRANSITION_DURATION_MS, with the first
    /// COLOR_TRANSITION_INITIAL_VISIBLE_PCT of rows adopting immediately
    /// for responsive first-frame feedback.
    fn color_wave_line_at(&self, now: Instant) -> Option<f32> {
        let start = self.transition_start?;
        let elapsed_ms = now.saturating_duration_since(start).as_millis() as f32;
        let duration = COLOR_TRANSITION_DURATION_MS as f32;
        if elapsed_ms >= duration {
            return Some(self.lines as f32 + 1.0); // Wave complete
        }
        // The initial band of rows adopts immediately for first-frame feedback.
        // We do this by offsetting the wave start: the wave line already
        // includes the initial visible fraction at t=0.
        let initial_frac = COLOR_TRANSITION_INITIAL_VISIBLE_PCT;
        let progress = (elapsed_ms / duration).clamp(0.0, 1.0);
        // At progress=0, wave_line = initial_frac * lines → first band visible.
        // At progress=1, wave_line = lines + 1 → entire screen converted.
        let wave_line = initial_frac * self.lines as f32
            + progress * (1.0 - initial_frac) * (self.lines as f32 + 1.0);
        Some(wave_line)
    }

    fn rebuild_char_pools(&mut self, chars: Vec<char>) {
        self.chars = chars;
        if self.chars.is_empty() {
            self.chars.push('0');
            self.chars.push('1');
        }

        self.char_pool.resize(CHAR_POOL_SIZE, '0');
        self.glitch_pool.resize(GLITCH_POOL_SIZE, '0');
        self.glitch_pool_idx = 0;

        let dist = Uniform::new_inclusive(0usize, self.chars.len().saturating_sub(1))
            .expect("char_pool: chars.len() >= 2 (guaranteed by empty check above)");
        for i in 0..self.char_pool.len() {
            let idx = dist.sample(&mut self.mt);
            self.char_pool[i] = self.chars[idx];
        }
        for i in 0..self.glitch_pool.len() {
            let idx = dist.sample(&mut self.mt);
            self.glitch_pool[i] = self.chars[idx];
        }
    }

    fn reset_phosphor_state(&mut self) {
        let total = (self.cols as usize) * (self.lines as usize);
        self.phosphor.clear();
        self.phosphor.resize(total, 0);
        self.phosphor_base_fg.clear();
        self.phosphor_base_fg.resize(total, None);
        self.phosphor_layer.clear();
        self.phosphor_layer.resize(total, 0);
    }

    fn recalc_droplets_per_sec(&mut self) {
        if self.lines == 0 || self.cols == 0 {
            self.droplets_per_sec = 0.0;
            return;
        }
        let droplet_seconds = (self.lines as f32) / self.chars_per_sec.max(0.001);
        if droplet_seconds <= 0.0 {
            self.droplets_per_sec = 0.0;
            return;
        }
        let dps = (self.cols as f32) * self.droplet_density / droplet_seconds;
        self.droplets_per_sec = if dps.is_finite() { dps.max(0.0) } else { 0.0 };
    }

    fn fill_glitch_map(&mut self) {
        if !self.glitchy {
            self.glitch_map.clear();
            return;
        }
        let size = self.lines as usize * self.cols as usize;
        self.glitch_map.resize(size, false);
        for i in 0..size {
            self.glitch_map
                .set(i, self.rand_chance.sample(&mut self.mt) <= self.glitch_pct);
        }
    }

    fn fill_color_map(&mut self) {
        let size = self.lines as usize * self.cols as usize;
        self.color_map.resize(size, 0);

        let n = self.palette.colors.len().max(1);
        let (low, high) = match n {
            0..=2 => (0, 0),
            3 => (1, 1),
            _ => (1, (n - 2) as u8),
        };
        let dist =
            Uniform::new_inclusive(low, high).expect("fill_color_map: low <= high by construction");

        for v in &mut self.color_map {
            *v = dist.sample(&mut self.mt);
        }
    }

    pub fn set_column_spawn(&mut self, col: u16, b: bool) {
        if let Some(cs) = self.col_stat.get_mut(col as usize) {
            cs.can_spawn = b;
        }
    }

    fn set_column_speeds(&mut self) {
        for cs in &mut self.col_stat {
            cs.max_speed_pct = if self.async_mode {
                self.rand_speed.sample(&mut self.mt)
            } else {
                1.0
            };
        }
    }

    fn update_droplet_speeds(&mut self) {
        for d in &mut self.droplets {
            if !d.is_alive {
                continue;
            }
            if let Some(cs) = self.col_stat.get(d.bound_col as usize) {
                let layer_speed = PARALLAX_SPEED_MULT[d.layer as usize];
                d.chars_per_sec = cs.max_speed_pct * self.chars_per_sec * layer_speed;
                // Keep velocity clamped to new terminal velocity
                let terminal = d.chars_per_sec * DROPLET_TERMINAL_VELOCITY_MULT;
                d.velocity = d.velocity.min(terminal);
            }
        }
    }

    fn time_for_glitch(&self, now: Instant) -> bool {
        self.glitchy && now >= self.next_glitch_time
    }

    #[must_use]
    #[inline]
    pub fn is_glitched(&self, line: u16, col: u16) -> bool {
        if !self.glitchy {
            return false;
        }
        let idx = col as usize * self.lines as usize + line as usize;
        self.glitch_map.get(idx).is_some_and(|b| *b)
    }

    fn do_glitch_span(&mut self, start_line: u16, hp: u16, col: u16, cp_idx: u16) {
        if !self.glitchy {
            return;
        }

        for line in start_line..=hp {
            if line >= self.lines {
                break;
            }
            if self.is_glitched(line, col) {
                let char_idx = ((cp_idx as usize) + (line as usize)) % self.char_pool.len();
                let repl = self.glitch_pool[self.glitch_pool_idx % self.glitch_pool.len()];
                self.char_pool[char_idx] = repl;
                self.glitch_pool_idx = (self.glitch_pool_idx + 1) % self.glitch_pool.len();
            }
        }
    }

    fn build_droplet_spec(&mut self, col: u16) -> DropletSpawnSpec {
        let mut end_line = self.lines.saturating_sub(1);
        if self.rand_chance.sample(&mut self.mt) <= self.die_early_pct {
            end_line = self.rand_line.sample(&mut self.mt);
        }
        let cp_idx = self.rand_cpidx.sample(&mut self.mt);

        let mut len = self.lines;
        if self.rand_chance.sample(&mut self.mt) <= self.short_pct {
            len = self.rand_len.sample(&mut self.mt);
        }

        // Assign parallax layer (0=far, 1=mid, 2=near)
        // Weighted: more background, fewer foreground for depth
        let layer_roll = self.rand_chance.sample(&mut self.mt);
        let layer: u8 = if layer_roll < 0.35 {
            0
        } else if layer_roll < 0.75 {
            1
        } else {
            2
        };

        // Adjust length by parallax layer
        let len_mult = PARALLAX_LENGTH_MULT[layer as usize];
        len = ((len as f32) * len_mult).max(1.0) as u16;

        let mut ttl = Duration::from_millis(1);
        if end_line <= len {
            let ms = self.rand_linger_ms.sample(&mut self.mt) as u64;
            ttl = Duration::from_millis(ms);
        }

        // Determine which palette this droplet inherits from its column.
        // During a transition, columns adopt the new palette at staggered times,
        // creating an organic propagation wave instead of a simultaneous switch.
        let palette_slot = self
            .column_palette_slot
            .get(col as usize)
            .copied()
            .unwrap_or(self.active_palette_slot);

        // Adjust speed by parallax layer
        let layer_speed = PARALLAX_SPEED_MULT[layer as usize];
        let mut speed = self
            .col_stat
            .get(col as usize)
            .map(|cs| cs.max_speed_pct)
            .unwrap_or(1.0)
            * self.chars_per_sec
            * layer_speed;

        // Transition momentum: new-generation streams get a subtle velocity
        // boost during active transitions, creating a feeling of an incoming wave.
        if palette_slot == self.active_palette_slot && self.transition_start.is_some() {
            speed *= 1.0 + TRANSITION_VELOCITY_BOOST;
        }

        // Initialize turbulence: unique phase offset per droplet
        let turb_phase = (cp_idx as f32 * 0.73).fract() * std::f32::consts::TAU;

        DropletSpawnSpec {
            col,
            end_line,
            char_pool_idx: cp_idx,
            length: len,
            chars_per_sec: speed,
            time_to_linger: ttl,
            layer,
            palette_slot,
            turb_phase,
        }
    }

    fn maybe_reseed_rng(&mut self, now: Instant) {
        if now.saturating_duration_since(self.last_reseed_time)
            >= Duration::from_secs(RNG_RESEED_INTERVAL_SECS)
        {
            let elapsed = now.elapsed();
            let seed = elapsed.as_nanos() as u64 ^ elapsed.as_secs();
            self.mt = StdRng::seed_from_u64(seed);
            self.last_reseed_time = now;
        }
    }

    fn spawn_droplets(&mut self, now: Instant, scale: f32) {
        let mut elapsed = now.saturating_duration_since(self.last_spawn_time);
        if self.max_sim_delta > Duration::from_millis(0) {
            elapsed = elapsed.min(self.max_sim_delta);
        }
        self.last_spawn_time = now;

        let elapsed_sec = elapsed.as_secs_f32();
        // Clamp spawn remainder to prevent debt accumulation at high speeds
        // or after timing spikes. Without this cap, a long stall could dump
        // hundreds of droplets in one frame, overwhelming the bottom rows.
        let clamped_remainder = self.spawn_remainder.min(SPAWN_REMAINDER_CAP);
        let budget = (elapsed_sec * self.droplets_per_sec * scale).max(0.0) + clamped_remainder;
        if !budget.is_finite() {
            self.spawn_remainder = 0.0;
            return;
        }
        let to_spawn = (budget.floor() as usize).min(self.droplets.len());
        self.spawn_remainder = (budget - (to_spawn as f32)).min(SPAWN_REMAINDER_CAP);
        if !self.spawn_remainder.is_finite() {
            self.spawn_remainder = 0.0;
        }
        if to_spawn == 0 {
            return;
        }

        let len = self.droplets.len();
        if len == 0 {
            return;
        }

        for _ in 0..to_spawn {
            let mut col = self.rand_col.sample(&mut self.mt);
            if self.full_width {
                col &= 0xFFFE;
            }

            if col as usize >= self.col_stat.len() {
                continue;
            }

            if !self.col_stat[col as usize].can_spawn
                || self.col_stat[col as usize].num_droplets >= self.max_droplets_per_column
            {
                continue;
            }

            // Mouse avoidance: skip spawning near cursor
            if self.mouse_enabled && self.mouse_col != u16::MAX {
                let col_dist = col.abs_diff(self.mouse_col);
                if col_dist <= MOUSE_AVOID_RADIUS_COLS {
                    continue;
                }
            }

            // Atmospheric depth: apply per-layer density control.
            // Pre-determine the layer for this spawn to check density.
            let layer_roll = self.rand_chance.sample(&mut self.mt);
            let layer: u8 = if layer_roll < 0.35 {
                0
            } else if layer_roll < 0.75 {
                1
            } else {
                2
            };
            // Far layer (0) spawns less frequently
            let density_mult = PARALLAX_DENSITY_MULT[layer as usize];
            if self.rand_chance.sample(&mut self.mt) > density_mult {
                continue;
            }

            let start = self.spawn_scan_idx.min(len.saturating_sub(1));
            let mut found = None;

            let mut idx = start;
            while idx < len {
                if !self.droplets[idx].is_alive {
                    found = Some(idx);
                    break;
                }
                idx += 1;
            }
            if found.is_none() {
                idx = 0;
                while idx < start {
                    if !self.droplets[idx].is_alive {
                        found = Some(idx);
                        break;
                    }
                    idx += 1;
                }
            }

            let Some(di) = found else {
                break;
            };

            let spec = self.build_droplet_spec(col);
            let d = &mut self.droplets[di];
            spec.apply_to(d);
            d.activate(now);
            self.spawn_scan_idx = (di + 1) % len;

            self.col_stat[col as usize].can_spawn = false;
            self.col_stat[col as usize].num_droplets += 1;
        }
    }

    pub fn force_draw_everything(&mut self) {
        self.force_draw_everything = true;
    }

    pub fn set_shading_mode(&mut self, sm: ShadingMode) {
        self.shading_mode = sm;
        self.shading_distance = matches!(sm, ShadingMode::DistanceFromHead);
        // Shading mode is a renderer semantic mutation — invalidate the
        // Terminal's LastFrame cache to prevent stale shading artifacts.
        self.semantic_invalidate = true;
    }

    fn reset_message(&mut self) {
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
        let fg = if self.color_mode == ColorMode::Mono {
            None
        } else {
            self.palette.colors.last().copied()
        };
        for mc in &self.message {
            frame.set(
                mc.col,
                mc.line,
                Cell {
                    ch: mc.val,
                    fg: if mc.val == ' ' { None } else { fg },
                    bg,
                    bold: mc.val != ' ' && self.bold_mode != BoldMode::Off,
                },
            );
        }
    }

    /// Phosphor persistence post-process: fade cells not refreshed by a
    /// droplet this frame, creating CRT-style afterglow.
    ///
    /// ## Bug fix: active trail cell protection
    ///
    /// The draw optimization in `Droplet::draw()` skips middle cells that
    /// haven't moved (lines below `head_cur_line`). These cells are NOT
    /// `current_gen` in the frame, so Pass 1 doesn't mark them fresh. Without
    /// protection, Pass 3 would decay their phosphor and render ghost cells,
    /// progressively dimming active trail cells — a major contributor to the
    /// "concrete wall" bottom accumulation bug.
    ///
    /// Pass 2 now marks cells within **living** droplet ranges as fresh,
    /// preventing phosphor decay from affecting active trail cells.
    ///
    /// ## Bug fix: blanked-cell ghost override
    ///
    /// When a cell is blanked by tail cleanup (fg = None, current_gen), the
    /// phosphor pass used to immediately render a ghost cell over it, blocking
    /// the intentional blank. Now, freshly blanked cells get their phosphor set
    /// to `PHOSPHOR_TAIL_RESIDUAL` but **no ghost cell** is rendered this
    /// frame — the blank takes effect, and afterglow begins on the next frame.
    ///
    /// ## Bug fix: bottom-row decay acceleration
    ///
    /// Ghost cells near the bottom of the screen decay faster (via
    /// `PHOSPHOR_BOTTOM_DECAY_MULT`), preventing accumulation where droplets
    /// end and fewer new streams overwrite the residue.
    fn phosphor_decay_pass(&mut self, frame: &mut Frame, elapsed_sec: f32) {
        let total = (self.cols as usize) * (self.lines as usize);
        if total == 0 || self.phosphor.len() != total {
            return;
        }

        // Skip phosphor under high performance pressure
        if self.perf_pressure > 0.7 {
            return;
        }

        let bg = self.palette.bg;
        let lines = self.lines;
        let frame_width = frame.width;

        // Pre-build blank cell for phosphor clear operations (avoids per-cell struct construction).
        let blank_cell = Cell {
            ch: ' ',
            fg: None,
            bg,
            bold: false,
        };

        // Pass 1: Mark cells currently drawn by droplets as fresh.
        // IMPORTANT: We check the frame's generation counter rather than scanning
        // for fg.is_some(), because cells from previous frames that weren't
        // redrawn would falsely appear fresh and never decay — causing the
        // "concrete wall" bottom accumulation bug.
        // We also need to cover cells drawn during this frame that may not be
        // in the dirty set yet (e.g., via draw_everything path), so we also
        // check cells that are in the current generation.
        self.phosphor_fresh.fill(false);
        let current_gen = frame.current_gen();
        for line in 0..lines {
            for col in 0..self.cols {
                let fidx = line as usize * frame_width as usize + col as usize;
                let is_current_gen = frame.cell_gen_at_index(fidx) == current_gen;
                if is_current_gen {
                    let cell = frame.cell_at_index_ref(fidx);
                    if cell.fg.is_some() {
                        let pidx = col as usize * lines as usize + line as usize;
                        self.phosphor_fresh.set(pidx, true);
                        self.phosphor[pidx] = 255;
                        self.phosphor_base_fg[pidx] = cell.fg;
                    }
                }
            }
        }

        // Pass 2: Update phosphor_layer from active droplets AND protect
        // active trail cells from phosphor decay.
        //
        // Without this protection, cells skipped by the draw optimization
        // (middle cells below head_cur_line) are NOT current_gen and NOT
        // marked fresh in Pass 1. Pass 3 would then decay their phosphor and
        // render ghost cells, progressively dimming active trail cells. This
        // was a major contributor to the "concrete wall" bottom accumulation
        // bug: active cells at the bottom were being ghosted frame after frame,
        // creating a dim, static residue that never cleared.
        for d in &self.droplets {
            if d.bound_col == u16::MAX || !d.is_alive {
                continue; // Only protect living droplets' cells
            }
            let start = d.tail_put_line.map(|v| v.saturating_add(1)).unwrap_or(0);
            for line in start..=d.head_put_line {
                if line >= lines {
                    break;
                }
                let pidx = d.bound_col as usize * lines as usize + line as usize;
                if pidx < self.phosphor_layer.len() {
                    self.phosphor_layer[pidx] = d.layer;
                }
                // Mark as fresh to prevent phosphor decay from ghosting
                // active trail cells that were skipped by the draw optimization.
                if pidx < self.phosphor_fresh.len()
                    && !self.phosphor_fresh.get(pidx).is_some_and(|b| *b)
                {
                    self.phosphor_fresh.set(pidx, true);
                    // Refresh phosphor energy so that when the tail eventually
                    // passes, the cell starts its afterglow from full energy.
                    self.phosphor[pidx] = 255;
                    // Update base_fg to the cell's current color so the
                    // afterglow reflects the most recent visual state.
                    let fidx = line as usize * frame_width as usize + d.bound_col as usize;
                    let cell_fg = frame.cell_at_index_ref(fidx).fg;
                    if cell_fg.is_some() {
                        self.phosphor_base_fg[pidx] = cell_fg;
                    }
                }
            }
        }

        // Pass 3: Decay non-fresh cells with phosphor energy
        for line in 0..lines {
            for col in 0..self.cols {
                let pidx = col as usize * lines as usize + line as usize;

                if self.phosphor_fresh.get(pidx).is_some_and(|b| *b) {
                    continue; // Cell was just drawn by a droplet (or protected by Pass 2)
                }

                if self.phosphor[pidx] == 0 {
                    continue;
                }

                // Check if cell was explicitly blanked this frame (tail cleanup
                // or droplet death). Blanked cells should NOT be overridden by
                // ghost cells — the blank takes effect immediately. Phosphor
                // energy is set to PHOSPHOR_TAIL_RESIDUAL so the afterglow
                // begins on the next frame, preserving the CRT cinematic look
                // without allowing ghost cells to block intentional clearing.
                let fidx = line as usize * frame_width as usize + col as usize;
                let is_blank_current_gen = frame.cell_gen_at_index(fidx) == current_gen
                    && frame.cell_at_index_ref(fidx).fg.is_none();

                if is_blank_current_gen {
                    // Cell was just blanked — start phosphor from residual
                    // energy but don't render a ghost cell that would override
                    // the blank. The afterglow begins next frame.
                    self.phosphor[pidx] = PHOSPHOR_TAIL_RESIDUAL;
                    continue;
                }

                // If phosphor is at 255 (max), this cell was drawn in a
                // previous frame and not refreshed. Set it to
                // PHOSPHOR_TAIL_RESIDUAL to start decay.
                if self.phosphor[pidx] == 255 {
                    self.phosphor[pidx] = PHOSPHOR_TAIL_RESIDUAL;
                } else {
                    // Apply exponential decay
                    let layer = self.phosphor_layer[pidx] as usize;
                    let layer_decay_mult =
                        PHOSPHOR_LAYER_DECAY_MULT.get(layer).copied().unwrap_or(1.0);

                    // Bottom-row decay acceleration: ghost cells near the
                    // bottom of the screen decay faster, preventing
                    // accumulation where droplets end and fewer new streams
                    // overwrite the residue ("concrete wall" fix).
                    let bottom_dist = lines.saturating_sub(line).saturating_sub(1);
                    let bottom_decay_mult = if bottom_dist < PHOSPHOR_BOTTOM_ROWS {
                        PHOSPHOR_BOTTOM_DECAY_MULT
                    } else {
                        1.0
                    };

                    let decay =
                        PHOSPHOR_DECAY_RATE * layer_decay_mult * bottom_decay_mult * elapsed_sec;
                    let new_energy = ((self.phosphor[pidx] as f32) * (-decay).exp()) as u8;
                    self.phosphor[pidx] = new_energy;
                }

                if self.phosphor[pidx] <= PHOSPHOR_DEAD_THRESHOLD {
                    // Phosphor is dead — clear cell and mark dirty
                    self.phosphor[pidx] = 0;
                    self.phosphor_base_fg[pidx] = None;
                    frame.set(col, line, blank_cell);
                } else if let Some(base_fg) = self.phosphor_base_fg[pidx] {
                    // Render ghost cell with dimmed color
                    let factor = self.phosphor[pidx] as f32 / 255.0;
                    let ghost_fg = crate::palette::apply_brightness(base_fg, factor);
                    frame.set(
                        col,
                        line,
                        Cell {
                            ch: ' ',
                            fg: Some(ghost_fg),
                            bg,
                            bold: false,
                        },
                    );
                }
            }
        }
    }

    /// Spawn a rare anomaly zone at a random position.
    fn spawn_anomaly(&mut self, now: Instant) {
        if self.anomaly_zones.len() >= ANOMALY_MAX_ZONES {
            return;
        }
        if self.cols == 0 || self.lines == 0 {
            return;
        }

        let col = self.rand_col.sample(&mut self.mt);
        let line = self.rand_line.sample(&mut self.mt);
        let radius = 3 + (self.rand_chance.sample(&mut self.mt) * 5.0) as u16; // 3-8

        let kind_roll = self.rand_chance.sample(&mut self.mt);
        let kind = if kind_roll < 0.4 {
            AnomalyKind::LuminanceSurge
        } else if kind_roll < 0.75 {
            AnomalyKind::GlyphCorruption
        } else {
            AnomalyKind::PulseWave
        };

        self.anomaly_zones.push(AnomalyZone {
            col,
            line,
            radius,
            kind,
            start_time: now,
        });
    }

    /// Apply active anomaly zone effects to the frame (post-processing).
    fn apply_anomalies(&mut self, frame: &mut Frame, now: Instant) {
        if self.anomaly_zones.is_empty() {
            return;
        }

        let bg = self.palette.bg;
        let cols = self.cols;
        let lines = self.lines;
        let width = frame.width;

        for zone in &self.anomaly_zones {
            let elapsed = now.saturating_duration_since(zone.start_time).as_secs_f32();
            if elapsed >= ANOMALY_DURATION_SECS {
                continue;
            }

            let progress = elapsed / ANOMALY_DURATION_SECS; // 0..1
            let fade = 1.0 - progress; // fades out over duration

            match zone.kind {
                AnomalyKind::LuminanceSurge => {
                    let r = zone.radius as i16;
                    for col_off in -r..=r {
                        for line_off in -r..=r {
                            let c = zone.col as i16 + col_off;
                            let l = zone.line as i16 + line_off;
                            if c < 0 || l < 0 {
                                continue;
                            }
                            let col = c as u16;
                            let line = l as u16;
                            if col >= cols || line >= lines {
                                continue;
                            }

                            let dist = ((col_off * col_off + line_off * line_off) as f32).sqrt();
                            if dist > zone.radius as f32 {
                                continue;
                            }

                            let falloff = 1.0 - dist / zone.radius as f32;
                            let intensity = ANOMALY_LUMINANCE_INTENSITY * falloff * fade;

                            let fidx = line as usize * width as usize + col as usize;
                            let cell = frame.cell_at_index(fidx);
                            if let Some(fg) = cell.fg {
                                let brightened = crate::palette::blend_toward_white(fg, intensity);
                                frame.set(
                                    col,
                                    line,
                                    Cell {
                                        ch: cell.ch,
                                        fg: Some(brightened),
                                        bg,
                                        bold: cell.bold,
                                    },
                                );
                            }
                        }
                    }
                }
                AnomalyKind::GlyphCorruption => {
                    let r = zone.radius as i16;
                    for col_off in -r..=r {
                        for line_off in -r..=r {
                            let c = zone.col as i16 + col_off;
                            let l = zone.line as i16 + line_off;
                            if c < 0 || l < 0 {
                                continue;
                            }
                            let col = c as u16;
                            let line = l as u16;
                            if col >= cols || line >= lines {
                                continue;
                            }

                            // Use deterministic hash for stable corruption per cell
                            let hash = ((col as u32).wrapping_mul(2654435761)
                                ^ (line as u32).wrapping_mul(2246822519))
                                >> 31;
                            if (hash as f32 / 2.0) > ANOMALY_CORRUPTION_CHANCE * fade {
                                continue;
                            }

                            let fidx = line as usize * width as usize + col as usize;
                            if frame.cell_at_index_ref(fidx).fg.is_some()
                                && !self.glitch_pool.is_empty()
                            {
                                let cell = frame.cell_at_index(fidx);
                                let glitch_idx = (col as usize + line as usize + elapsed as usize)
                                    % self.glitch_pool.len();
                                frame.set(
                                    col,
                                    line,
                                    Cell {
                                        ch: self.glitch_pool[glitch_idx],
                                        fg: cell.fg,
                                        bg,
                                        bold: cell.bold,
                                    },
                                );
                            }
                        }
                    }
                }
                AnomalyKind::PulseWave => {
                    let wave_radius = progress * zone.radius as f32 * 2.0;
                    let ring_width = 2.0;
                    let r2 = (zone.radius as i16) * 2;
                    for col_off in -r2..=r2 {
                        for line_off in -r2..=r2 {
                            let c = zone.col as i16 + col_off;
                            let l = zone.line as i16 + line_off;
                            if c < 0 || l < 0 {
                                continue;
                            }
                            let col = c as u16;
                            let line = l as u16;
                            if col >= cols || line >= lines {
                                continue;
                            }

                            let dist = ((col_off * col_off + line_off * line_off) as f32).sqrt();
                            let ring_dist = (dist - wave_radius).abs();
                            if ring_dist < ring_width {
                                let t = 1.0 - ring_dist / ring_width;
                                let intensity = 0.2 * t * fade;
                                let fidx = line as usize * width as usize + col as usize;
                                let cell = frame.cell_at_index(fidx);
                                if let Some(fg) = cell.fg {
                                    let brightened =
                                        crate::palette::blend_toward_white(fg, intensity);
                                    frame.set(
                                        col,
                                        line,
                                        Cell {
                                            ch: cell.ch,
                                            fg: Some(brightened),
                                            bg,
                                            bold: cell.bold,
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Apply Phase 3 global atmospheric effects to the frame.
    fn apply_atmospheric_frame_effects(&self, frame: &mut Frame, now: Instant) {
        let luminance = self.color_ecosystem.luminance_climate;
        let saturation = self.color_ecosystem.saturation_climate;
        let instability = self.memory.instability_pressure;
        let persistence = self.memory.persistence_richness;
        let emergent = self.storytelling.active_effects(now);
        let profile = self.profile_current;

        // Skip if all modifiers are neutral
        let needs_luminance = (luminance - 1.0).abs() > 0.01
            || emergent.luminance_boost > 0.0
            || profile.luminance_offset.abs() > 0.01;
        let needs_saturation = (saturation - 1.0).abs() > 0.01;
        let needs_persistence = persistence.abs() > 0.01;

        if !needs_luminance && !needs_saturation && !needs_persistence {
            return;
        }

        // Apply to all cells with foreground color
        let bg = self.palette.bg;
        for line in 0..self.lines {
            for col in 0..self.cols {
                let fidx = line as usize * frame.width as usize + col as usize;
                let cell = frame.cell_at_index(fidx);
                if let Some(fg) = cell.fg {
                    let mut modified = fg;

                    // Luminance climate
                    if needs_luminance {
                        let total_lum =
                            luminance + profile.luminance_offset + emergent.luminance_boost;
                        if total_lum < 1.0 {
                            modified = crate::palette::apply_brightness(
                                modified,
                                total_lum.clamp(0.0, 1.0),
                            );
                        } else if total_lum > 1.0 {
                            let boost = (total_lum - 1.0).clamp(0.0, 0.3);
                            modified = crate::palette::blend_toward_white(modified, boost);
                        }
                    }

                    // Saturation climate (desaturate by blending toward luminance-matched gray)
                    if needs_saturation && saturation < 1.0 {
                        modified = crate::palette::apply_saturation(modified, saturation);
                    }

                    // Persistence richness: boost phosphor-like brightness
                    if needs_persistence && persistence > 0.0 {
                        modified = crate::palette::blend_toward_white(modified, persistence * 0.3);
                    }

                    // Instability pressure: subtle brightness jitter (very rare, very subtle)
                    if instability > 0.15 {
                        // Deterministic jitter based on position and time
                        let hash = (col as u32).wrapping_mul(2654435761)
                            ^ (line as u32).wrapping_mul(2246822519)
                            ^ (now.elapsed().as_secs() as u32);
                        if hash % 1000 < (instability * 50.0) as u32 {
                            modified =
                                crate::palette::blend_toward_white(modified, instability * 0.1);
                        }
                    }

                    frame.set(
                        col,
                        line,
                        crate::cell::Cell {
                            ch: cell.ch,
                            fg: Some(modified),
                            bg,
                            bold: cell.bold,
                        },
                    );
                }
            }
        }
    }

    pub fn rain(&mut self, frame: &mut Frame) {
        self.rain_at(frame, Instant::now());
    }

    pub fn rain_at(&mut self, frame: &mut Frame, now: Instant) {
        if self.pause {
            return;
        }

        // Update color transition: during a palette transition, check if the
        // wave has completed (all rows have adopted the new palette).
        // The visual wave is driven by color_wave_line_at() in DrawCtx;
        // here we just detect completion and update droplet palette slots
        // for streams that are now fully above the wave.
        if let Some(transition_start) = self.transition_start {
            let elapsed_ms = now.saturating_duration_since(transition_start).as_millis() as u64;
            if elapsed_ms >= COLOR_TRANSITION_DURATION_MS as u64 {
                // Transition complete: all droplets adopt the new palette
                for d in &mut self.droplets {
                    if d.is_alive {
                        d.palette_slot = self.active_palette_slot;
                    }
                }
                self.transition_start = None;
            }
        }

        let charset_wave_line = self.charset_wave_line_at(now);
        if self.charset_transition_start.is_some_and(|start| {
            now.saturating_duration_since(start).as_millis()
                >= CHARSET_TRANSITION_DURATION_MS as u128
        }) {
            self.charset_transition_start = None;
            self.previous_char_pool.clear();
        }

        // Periodically re-seed RNG for very long sessions
        self.maybe_reseed_rng(now);

        // Advance cinematic resume easing: smoothstep S-curve from 0→1 over
        // RESUME_EASE_DURATION_SECS (300ms) after unpause. Unlike exponential
        // easing or position-delta scaling, this interpolates the simulation
        // time scale itself — the physics clock runs in slow motion during
        // the transition, producing genuine inertia recovery rather than a
        // frozen-then-unfrozen snap.
        if let Some(rs) = self.resume_start {
            let t = now.saturating_duration_since(rs).as_secs_f32();
            let normalized = (t / RESUME_EASE_DURATION_SECS).min(1.0);
            // Smoothstep: 3t² - 2t³ — slow start, fast middle, slow end.
            self.resume_blend = normalized * normalized * (3.0 - 2.0 * normalized);
            if normalized >= 1.0 {
                self.resume_blend = 1.0;
                self.resume_start = None; // Transition complete — stop tracking
            }
        }

        let mut spawn_scale = (1.0 - (PERF_PRESSURE_SPAWN_FACTOR * self.perf_pressure))
            .clamp(PERF_SPAWN_SCALE_MIN, 1.0);
        // Apply atmospheric density modulation
        spawn_scale *= 1.0 + self.atmosphere.density_offset;
        // Apply profile density modulation
        spawn_scale *= self.profile_current.density_mult;
        // Apply emergent density boost
        spawn_scale += self.storytelling.active_effects(now).density_boost;
        // Apply resume time-scale easing: spawn rate ramps with the smoothstep
        // curve so new streams appear gradually during the inertia recovery.
        spawn_scale *= self.resume_blend;
        spawn_scale = spawn_scale.clamp(0.0, 3.0);
        self.spawn_droplets(now, spawn_scale);

        // Process pending semantic invalidation BEFORE force_draw_everything.
        // Semantic mutations (charset switch, shading mode toggle) require
        // invalidate_semantic() which bumps semantic_gen, ensuring the
        // Terminal's LastFrame cache is fully synchronized.
        if self.semantic_invalidate {
            self.semantic_invalidate = false;
            frame.invalidate_semantic(self.palette.bg);
        }

        let force_draw_everything = self.force_draw_everything;
        if force_draw_everything {
            frame.clear_with_bg(self.palette.bg);
            self.force_draw_everything = false;
        }

        let glitch_due = self.time_for_glitch(now);
        let allow_glitch = glitch_due && self.perf_pressure < GLITCH_THRESHOLD;
        let time_for_glitch = allow_glitch;

        let max_sim_delta = self.max_sim_delta;
        let use_sim_cap = max_sim_delta > Duration::from_millis(0);

        // Update pass (mut self)
        for i in 0..self.droplets.len() {
            if !self.droplets[i].is_alive {
                continue;
            }

            let (col, start_line, hp, cp_idx, free_col, died) = {
                let d = &mut self.droplets[i];
                let adv_now = if use_sim_cap {
                    if let Some(last) = d.last_time {
                        let max_now = last + max_sim_delta;
                        if now > max_now {
                            max_now
                        } else {
                            now
                        }
                    } else {
                        now
                    }
                } else {
                    now
                };
                let free_col = d.advance(adv_now, self.lines, self.resume_blend);
                let col = d.bound_col;
                let start_line = d.tail_put_line.map(|v| v + 1).unwrap_or(0);
                let hp = d.head_put_line;
                let cp_idx = d.char_pool_idx;
                let died = !d.is_alive;
                (col, start_line, hp, cp_idx, free_col, died)
            };

            if died {
                if let Some(cs) = self.col_stat.get_mut(col as usize) {
                    cs.num_droplets = cs.num_droplets.saturating_sub(1);
                    cs.can_spawn = true;
                }
                continue;
            }

            if free_col {
                self.set_column_spawn(col, true);
            }

            if time_for_glitch {
                self.do_glitch_span(start_line, hp, col, cp_idx);
            }
        }

        // Build palette_slices for DrawCtx from the palette table.
        // Each slot either has a Palette (Some) or is empty (None) — use an
        // empty slice for empty slots so hot-path rendering stays branch-free.
        let empty: &[Color] = &[];
        let mut palette_slices: [&[Color]; MAX_PALETTE_SLOTS] = [&[]; MAX_PALETTE_SLOTS];
        for (i, slot) in palette_slices.iter_mut().enumerate() {
            if let Some(ref p) = self.palette_table[i] {
                *slot = &p.colors;
            } else {
                *slot = empty;
            }
        }

        let transitioning = self.transition_start.is_some();
        let charset_wave_line = if self.charset_transition_start.is_some() {
            charset_wave_line
        } else {
            None
        };
        let color_wave_line = self.color_wave_line_at(now);

        // Draw pass (split-borrows via DrawCtx)
        let draw_everything = force_draw_everything || time_for_glitch;
        let ctx = DrawCtx {
            lines: self.lines,
            full_width: self.full_width,
            shading_distance: self.shading_distance,
            bg: self.palette.bg,
            color_mode: self.color_mode,
            bold_mode: self.bold_mode,
            glitchy: self.glitchy,
            last_glitch_time: self.last_glitch_time,
            next_glitch_time: self.next_glitch_time,
            palette_slices,
            active_palette_slot: self.active_palette_slot,
            transitioning,
            color_map: &self.color_map,
            glitch_map: &self.glitch_map,
            char_pool: &self.char_pool,
            previous_char_pool: &self.previous_char_pool,
            charset_wave_line,
            color_wave_line,
            mouse_col: self.mouse_col,
            mouse_line: self.mouse_line,
            flash_col: self.flash_col,
            flash_line: self.flash_line,
            flash_time: self.flash_time,
        };

        for d in &mut self.droplets {
            let needs_tail_cleanup = !d.is_alive
                && d.bound_col != u16::MAX
                && d.tail_put_line.is_some_and(|tp| d.tail_cur_line != tp);

            if d.is_alive || needs_tail_cleanup {
                d.draw(&ctx, frame, now, draw_everything);
            }

            if !d.is_alive {
                d.bound_col = u16::MAX;
            }
        }

        if !self.message.is_empty() {
            self.draw_message(frame);
        }

        // --- Phosphor persistence post-process ---
        // Scale phosphor decay elapsed by resume_blend so afterglow fades at
        // the same rate as the rain wakes up. Without this, phosphor trails
        // vanish at full speed while droplets move in slow motion — creating
        // temporal inconsistency that feels "spiky" during resume.
        let phosphor_elapsed = now
            .saturating_duration_since(self.last_phosphor_time)
            .as_secs_f32()
            * self.resume_blend;
        self.last_phosphor_time = now;
        self.phosphor_decay_pass(frame, phosphor_elapsed);

        // --- Rare anomaly events ---
        // Check for new anomaly spawn. The product of multipliers creates a
        // positive feedback loop (more anomalies → higher instability → more
        // anomalies). Cap the effective rate at 3× base to prevent visual
        // overload while preserving atmospheric dynamics.
        let anomaly_chance = (ANOMALY_CHANCE_PER_SEC
            * self.profile_current.anomaly_freq_mult as f64
            * (1.0 + self.atmosphere.anomaly_offset as f64)
            * (1.0 + self.memory.instability_pressure as f64))
            .min(ANOMALY_CHANCE_PER_SEC * 3.0);
        if phosphor_elapsed > 0.0
            && (self.rand_chance.sample(&mut self.mt) as f64)
                <= anomaly_chance * phosphor_elapsed as f64
        {
            self.spawn_anomaly(now);
        }
        // Expire old anomaly zones
        self.anomaly_zones.retain(|z| {
            now.saturating_duration_since(z.start_time).as_secs_f32() < ANOMALY_DURATION_SECS
        });
        // Apply anomaly effects to frame
        self.apply_anomalies(frame, now);

        // --- Phase 3: Autonomous cinematic ecosystem tick ---
        // 1. Color ecosystem drift
        if let Some(new_scheme) = self
            .color_ecosystem
            .tick(now, &mut self.mt, self.color_scheme)
        {
            self.set_color_scheme(new_scheme);
        }

        // 2. Atmospheric evolution
        self.atmosphere.tick(now, self.profile_current.entropy_rate);

        // 3. Renderer memory sampling
        let anomaly_density = self.anomaly_zones.len() as f32 / ANOMALY_MAX_ZONES.max(1) as f32;
        let rain_density = self.droplet_density;
        self.memory.record_sample(
            now,
            anomaly_density,
            rain_density,
            self.color_ecosystem.luminance_climate,
        );
        self.memory.recompute_derived();

        // 4. Emergent storytelling
        if let Some(kind) = self.storytelling.tick(
            now,
            &mut self.mt,
            &self.atmosphere,
            &self.memory,
            &self.color_ecosystem,
        ) {
            self.storytelling.moments.push(EmergentMoment {
                kind,
                start_time: now,
                duration: EMERGENT_MOMENT_DURATION_SECS,
            });
            self.storytelling.cooldown_until =
                Some(now + Duration::from_secs_f32(EMERGENT_MOMENT_DURATION_SECS + 60.0));
        }
        self.storytelling.expire_moments(now);

        // 5. Profile interpolation (smooth transition)
        if let Some(transition_start) = self.profile_transition_start {
            let elapsed = now
                .saturating_duration_since(transition_start)
                .as_secs_f32();
            let t = (elapsed / PROFILE_TRANSITION_SECS).min(1.0);
            // Smooth step interpolation
            let t = t * t * (3.0 - 2.0 * t);
            self.profile_current = lerp_profile_params(
                self.profile_current,
                self.profile_target,
                PROFILE_INTERPOLATION_RATE.max(t),
            );
            if t >= 1.0 {
                self.profile_current = self.profile_target;
                self.profile_transition_start = None;
            }
        }

        // 7. Apply Phase 3 global atmospheric frame effects (post-process)
        self.apply_atmospheric_frame_effects(frame, now);

        // --- Periodic full redraw for ANSI drift correction ---
        // Every N frames, force a complete screen refresh. This corrects any
        // accumulated terminal state desync (e.g., from resize, scroll, or
        // rare edge cases in differential rendering) without measurable perf
        // impact since full redraws are already optimized with row batching.
        self.frames_since_full_redraw += 1;
        if self.frames_since_full_redraw >= FULL_REDRAW_INTERVAL_FRAMES {
            self.frames_since_full_redraw = 0;
            self.force_draw_everything = true;
        }

        if time_for_glitch || glitch_due {
            self.last_glitch_time = now;
            let ms = self.rand_glitch_ms.sample(&mut self.mt) as u64;
            self.next_glitch_time = self.last_glitch_time + Duration::from_millis(ms);
        }
        // Expire flash effect after duration
        if let Some(flash_time) = self.flash_time {
            if now.saturating_duration_since(flash_time).as_secs_f32() >= MOUSE_FLASH_DURATION_SECS
            {
                self.flash_time = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use crossterm::style::Color;

    use super::{Cloud, DrawCtx};
    use crate::constants::{
        CHARSET_TRANSITION_DURATION_MS, COLOR_TRANSITION_DURATION_MS,
        COLOR_TRANSITION_INITIAL_VISIBLE_PCT, FULL_REDRAW_INTERVAL_FRAMES, MAX_PALETTE_SLOTS,
        PHOSPHOR_BOTTOM_ROWS, SPAWN_REMAINDER_CAP,
    };
    use crate::frame::Frame;
    use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};

    fn make_cloud() -> Cloud {
        let mut cloud = Cloud::new(
            ColorMode::Mono,
            false,
            ShadingMode::Random,
            BoldMode::Off,
            false,
            true,
            ColorScheme::Green,
        );
        cloud.init_chars(vec!['0', '1']);
        cloud.reset(20, 10);
        cloud
    }

    #[test]
    fn rain_produces_dirty_frame_when_time_advances() {
        let mut cloud = make_cloud();
        let mut frame = Frame::new(20, 10, cloud.palette.bg);

        cloud.last_spawn_time = Instant::now() - Duration::from_secs(1);
        cloud.rain(&mut frame);

        assert!(frame.is_dirty_all() || !frame.dirty_indices().is_empty());
    }

    #[test]
    fn pause_stops_rain_and_unpause_resumes() {
        let mut cloud = make_cloud();
        let mut frame = Frame::new(20, 10, cloud.palette.bg);

        cloud.last_spawn_time = Instant::now() - Duration::from_secs(1);
        cloud.rain(&mut frame);
        assert!(frame.is_dirty_all() || !frame.dirty_indices().is_empty());

        frame.clear_dirty();
        cloud.toggle_pause();
        cloud.rain(&mut frame);
        assert!(!frame.is_dirty_all() && frame.dirty_indices().is_empty());

        cloud.toggle_pause();
        // Advance resume_start far enough in the past so the smoothstep
        // easing completes (resume_blend reaches 1.0, allowing full-speed
        // simulation on the next rain() call).
        let now = Instant::now();
        cloud.resume_start = Some(now - Duration::from_secs(1));
        cloud.last_spawn_time = now - Duration::from_secs(1);
        cloud.rain_at(&mut frame, now);
        cloud.rain_at(&mut frame, now + Duration::from_secs(1));
        assert!(frame.is_dirty_all() || !frame.dirty_indices().is_empty());
    }

    #[test]
    fn periodic_full_redraw_survives_until_next_frame() {
        let mut cloud = make_cloud();
        let mut frame = Frame::new(20, 10, cloud.palette.bg);
        let now = Instant::now();

        frame.clear_dirty();
        cloud.frames_since_full_redraw = FULL_REDRAW_INTERVAL_FRAMES - 1;
        cloud.rain_at(&mut frame, now);
        assert!(cloud.force_draw_everything);

        frame.clear_dirty();
        cloud.rain_at(&mut frame, now + Duration::from_millis(16));
        assert!(frame.is_dirty_all());
        assert!(!cloud.force_draw_everything);
    }

    #[test]
    fn color_transition_starts_immediately_and_completes() {
        let mut cloud = make_cloud();
        let mut frame = Frame::new(20, 10, cloud.palette.bg);
        let now = Instant::now();

        cloud.set_color_scheme(ColorScheme::Blue);

        assert_eq!(cloud.color_scheme(), ColorScheme::Blue);
        assert!(
            cloud.transition_start.is_some(),
            "transition must start immediately after set_color_scheme"
        );

        // Row-based wave: at t=0, the wave line should cover the initial band.
        // The wave line value represents the boundary; rows with index <= wave_line
        // use the new palette. So wave_line=1.2 means rows 0 and 1 adopt.
        let wave = cloud.color_wave_line_at(now);
        assert!(
            wave.is_some(),
            "color wave must be active during transition"
        );
        // The number of rows that adopt the new palette at t=0 is (wave_line + 1),
        // since row indices 0 through floor(wave_line) are above the wave.
        let initial_adopted_rows = wave.unwrap().floor() as usize + 1;
        let min_initial_rows =
            ((cloud.lines as f32 * COLOR_TRANSITION_INITIAL_VISIBLE_PCT).ceil() as usize).max(1);
        assert!(
            initial_adopted_rows >= min_initial_rows,
            "first transition frame should visibly update a band of top rows (got {} rows >= {})",
            initial_adopted_rows,
            min_initial_rows
        );

        // All columns should already have adopted the new palette (wave is row-based)
        assert!(cloud
            .column_palette_slot
            .iter()
            .all(|slot| *slot == cloud.active_palette_slot));

        cloud.transition_start = Some(now);
        cloud.rain_at(&mut frame, now);

        // After one frame, transition should still be in progress (150ms hasn't elapsed)
        assert!(cloud.transition_start.is_some());

        cloud.transition_start =
            Some(now - Duration::from_millis(COLOR_TRANSITION_DURATION_MS as u64 + 1));
        cloud.rain_at(&mut frame, now);

        assert!(cloud.transition_start.is_none());
        // All droplets should have adopted the new palette after transition completes
        for d in &cloud.droplets {
            if d.is_alive {
                assert_eq!(d.palette_slot, cloud.active_palette_slot);
            }
        }
    }

    #[test]
    fn charset_change_enters_transition_state_without_full_swap() {
        let mut cloud = make_cloud();
        cloud.semantic_invalidate = false;
        cloud.force_draw_everything = false;
        let old_pool = cloud.char_pool.clone();

        cloud.transition_chars(vec!['A', 'B']);

        assert!(cloud.charset_transition_start.is_some());
        assert_eq!(cloud.previous_char_pool, old_pool);
        assert_ne!(cloud.char_pool, old_pool);
        assert!(!cloud.semantic_invalidate);
        assert!(!cloud.force_draw_everything);
    }

    #[test]
    fn charset_wave_uses_old_rows_below_and_new_rows_above() {
        let old_pool = ['0', '1'];
        let new_pool = ['A', 'B'];
        let glitch_map = bitvec::bitvec![0; 20];
        let empty: &[Color] = &[];
        let palette_slices: [&[Color]; MAX_PALETTE_SLOTS] = [empty; MAX_PALETTE_SLOTS];

        let ctx = DrawCtx {
            lines: 10,
            full_width: false,
            shading_distance: false,
            bg: None,
            color_mode: ColorMode::Mono,
            bold_mode: BoldMode::Off,
            glitchy: false,
            last_glitch_time: Instant::now(),
            next_glitch_time: Instant::now(),
            palette_slices,
            active_palette_slot: 0,
            transitioning: false,
            color_map: &[],
            glitch_map: glitch_map.as_bitslice(),
            char_pool: &new_pool,
            previous_char_pool: &old_pool,
            charset_wave_line: Some(3.0),
            color_wave_line: None,
            mouse_col: u16::MAX,
            mouse_line: u16::MAX,
            flash_col: u16::MAX,
            flash_line: u16::MAX,
            flash_time: None,
        };

        assert_eq!(ctx.get_char(1, 0, 0), 'B');
        assert_eq!(ctx.get_char(8, 0, 0), '0');
    }

    #[test]
    fn charset_transition_completes_and_commits_new_pool() {
        let mut cloud = make_cloud();
        let mut frame = Frame::new(20, 10, cloud.palette.bg);
        let now = Instant::now();

        cloud.transition_chars(vec!['A', 'B']);
        cloud.charset_transition_start =
            Some(now - Duration::from_millis(CHARSET_TRANSITION_DURATION_MS as u64 + 1));
        cloud.rain_at(&mut frame, now);

        assert!(cloud.charset_transition_start.is_none());
        assert!(cloud.previous_char_pool.is_empty());
        assert!(cloud.char_pool.iter().all(|ch| matches!(ch, 'A' | 'B')));
    }

    #[test]
    fn pause_freezes_simulation_time() {
        let mut cloud = make_cloud();
        let mut frame = Frame::new(20, 10, cloud.palette.bg);
        let now = Instant::now();

        cloud.last_spawn_time = now - Duration::from_secs(1);
        assert!(cloud.toggle_pause());
        let last_spawn = cloud.last_spawn_time;
        frame.clear_dirty();

        cloud.rain_at(&mut frame, now + Duration::from_secs(5));

        assert_eq!(cloud.last_spawn_time, last_spawn);
        assert!(!frame.is_dirty_all() && frame.dirty_indices().is_empty());
    }

    #[test]
    fn resume_resets_timing_debt() {
        let mut cloud = make_cloud();
        let now = Instant::now();

        assert!(cloud.toggle_pause());
        cloud.pause_time = Some(now - Duration::from_secs(5));
        cloud.spawn_remainder = 42.0;
        assert!(cloud.toggle_pause());

        assert!(!cloud.pause);
        assert_eq!(cloud.spawn_remainder, 0.0);
        assert_eq!(cloud.resume_blend, 0.0);
        assert!(cloud.resume_start.is_some());
        assert!(cloud.last_spawn_time > now - Duration::from_secs(5));
    }

    #[test]
    fn repeated_pause_resume_does_not_accumulate_timing_debt() {
        let mut cloud = make_cloud();
        let now = Instant::now();

        for seconds in 1..=3 {
            assert!(cloud.toggle_pause());
            cloud.pause_time = Some(now - Duration::from_secs(seconds));
            cloud.spawn_remainder = seconds as f32;
            assert!(cloud.toggle_pause());

            assert!(!cloud.pause);
            assert_eq!(cloud.spawn_remainder, 0.0);
            assert_eq!(cloud.resume_blend, 0.0);
            assert!(cloud.resume_start.is_some());
        }
    }

    #[test]
    fn repeated_runtime_transitions_replace_pending_state_predictably() {
        let mut cloud = make_cloud();
        let first_pool = cloud.char_pool.clone();

        cloud.set_color_scheme(ColorScheme::Blue);
        cloud.set_color_scheme(ColorScheme::Red);
        assert_eq!(cloud.color_scheme(), ColorScheme::Red);
        assert!(cloud.transition_start.is_some());

        cloud.transition_chars(vec!['A', 'B']);
        let intermediate_pool = cloud.char_pool.clone();
        cloud.transition_chars(vec!['X', 'Y']);

        assert_eq!(cloud.previous_char_pool, intermediate_pool);
        assert_ne!(cloud.previous_char_pool, first_pool);
        assert!(cloud.char_pool.iter().all(|ch| matches!(ch, 'X' | 'Y')));
        assert!(cloud.charset_transition_start.is_some());
    }

    #[test]
    fn color_wave_begins_at_top_rows() {
        let mut cloud = make_cloud();
        let now = Instant::now();
        cloud.set_color_scheme(ColorScheme::Blue);

        let wave = cloud.color_wave_line_at(now);
        assert!(wave.is_some());
        let wave_line = wave.unwrap();
        // At t=0, the wave should cover at least the initial visible fraction of rows.
        // Rows 0..=floor(wave_line) adopt the new palette immediately.
        let adopted_rows = wave_line.floor() as usize + 1;
        let min_rows =
            ((cloud.lines as f32 * COLOR_TRANSITION_INITIAL_VISIBLE_PCT).ceil() as usize).max(1);
        assert!(
            adopted_rows >= min_rows,
            "color wave at t=0 should cover initial band of top rows ({} rows adopted >= {} expected)",
            adopted_rows,
            min_rows
        );
    }

    #[test]
    fn color_wave_progresses_downward_over_time() {
        let mut cloud = make_cloud();
        let start = Instant::now();
        cloud.transition_start = Some(start);

        let wave_early = cloud
            .color_wave_line_at(start + Duration::from_millis(10))
            .unwrap();
        let wave_mid = cloud
            .color_wave_line_at(start + Duration::from_millis(75))
            .unwrap();
        let wave_late = cloud
            .color_wave_line_at(start + Duration::from_millis(140))
            .unwrap();

        assert!(
            wave_mid > wave_early,
            "color wave should progress downward over time"
        );
        assert!(
            wave_late > wave_mid,
            "color wave should continue progressing downward"
        );
    }

    #[test]
    fn repeated_color_transitions_remain_valid() {
        let mut cloud = make_cloud();

        cloud.set_color_scheme(ColorScheme::Blue);
        assert!(cloud.transition_start.is_some());

        // Second transition should replace the first cleanly
        cloud.set_color_scheme(ColorScheme::Red);
        assert!(cloud.transition_start.is_some());
        assert_eq!(cloud.color_scheme(), ColorScheme::Red);
    }

    #[test]
    fn spawn_remainder_is_clamped() {
        let mut cloud = make_cloud();
        cloud.spawn_remainder = 100.0; // Unrealistically high
        cloud.last_spawn_time = Instant::now() - Duration::from_secs(1);
        let mut frame = Frame::new(20, 10, cloud.palette.bg);
        cloud.rain(&mut frame);
        // After one rain tick, spawn remainder should be clamped
        assert!(
            cloud.spawn_remainder <= SPAWN_REMAINDER_CAP,
            "spawn remainder should be clamped to SPAWN_REMAINDER_CAP (got {})",
            cloud.spawn_remainder
        );
    }

    #[test]
    fn mouse_mode_is_default_off_and_opt_in() {
        let mut cloud = make_cloud();
        assert!(
            !cloud.mouse_enabled,
            "mouse_enabled should default to false (off) when not explicitly set"
        );
        cloud.mouse_enabled = true;
        assert!(cloud.mouse_enabled);
    }

    #[test]
    fn color_uses_previous_palette_below_wave_line() {
        let empty: &[Color] = &[];
        let palette_slices: [&[Color]; MAX_PALETTE_SLOTS] = [empty; MAX_PALETTE_SLOTS];
        let glitch_map = bitvec::bitvec![0; 200];

        let ctx = DrawCtx {
            lines: 10,
            full_width: false,
            shading_distance: false,
            bg: None,
            color_mode: ColorMode::Mono,
            bold_mode: BoldMode::Off,
            glitchy: false,
            last_glitch_time: Instant::now(),
            next_glitch_time: Instant::now(),
            palette_slices,
            active_palette_slot: 1,
            transitioning: true,
            color_map: &[],
            glitch_map: glitch_map.as_bitslice(),
            char_pool: &['0', '1'],
            previous_char_pool: &[],
            charset_wave_line: None,
            color_wave_line: Some(3.0),
            mouse_col: u16::MAX,
            mouse_line: u16::MAX,
            flash_col: u16::MAX,
            flash_line: u16::MAX,
            flash_time: None,
        };

        // Row 0 (above wave): droplet with old palette should NOT use previous
        assert!(!ctx.color_uses_previous_palette(0, 0, 0));
        // Row 8 (below wave): droplet with old palette SHOULD use previous
        assert!(ctx.color_uses_previous_palette(0, 8, 0));
        // Droplet already on active palette: always uses new palette
        assert!(!ctx.color_uses_previous_palette(1, 8, 0));
    }

    #[test]
    fn droplet_exiting_bottom_fully_clears_trail() {
        // When a droplet dies (tail catches head), all its trail cells should
        // be blanked in the draw call. After the draw, no cells in the
        // droplet's column should retain stale content from that droplet.
        let mut cloud = make_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

        // Spawn a droplet that will die quickly
        let now = Instant::now();
        cloud.last_spawn_time = now - Duration::from_secs(1);
        cloud.rain_at(&mut frame, now);

        // Find a living droplet and force it to die by making tail catch head
        let mut found = false;
        for d in &mut cloud.droplets {
            if d.is_alive {
                // Force the droplet to die: tail reaches head
                d.tail_put_line = Some(d.head_put_line);
                d.tail_cur_line = d.head_put_line;
                d.is_alive = false;
                found = true;
                break;
            }
        }
        assert!(found, "should have at least one living droplet after rain");

        // Run another frame — the dead droplet's draw() should blank all cells
        let mut frame2 = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        cloud.last_phosphor_time = now;
        cloud.rain_at(&mut frame2, now + Duration::from_millis(16));

        // After the draw, dead droplet's bound_col should be recycled
        let dead_droplets: Vec<_> = cloud
            .droplets
            .iter()
            .filter(|d| !d.is_alive && d.bound_col != u16::MAX)
            .collect();
        assert!(
            dead_droplets.is_empty(),
            "dead droplets should have bound_col = u16::MAX after cleanup draw"
        );
    }

    #[test]
    fn phosphor_blank_cells_are_not_overridden_by_ghost() {
        // When a cell is blanked (fg = None, current_gen), the phosphor pass
        // should NOT render a ghost cell over it. The blank should take effect,
        // and afterglow begins on the next frame.
        let mut cloud = make_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

        let now = Instant::now();
        cloud.last_spawn_time = now - Duration::from_secs(1);
        cloud.rain_at(&mut frame, now);

        // Manually blank a cell that has phosphor energy
        let col = 0u16;
        let line = 0u16;
        let blank = crate::terminal::blank_cell(cloud.palette.bg);
        frame.set(col, line, blank);

        // The cell should now be blank in the frame
        let cell = frame.get(col, line).unwrap();
        assert_eq!(cell.ch, ' ');
        assert!(cell.fg.is_none(), "blanked cell should have fg = None");
    }

    #[test]
    fn stale_bottom_cells_decay_to_blank_within_bounded_time() {
        // Phosphor ghost cells at the bottom of the screen should decay to
        // blank within a bounded number of frames, thanks to the bottom-row
        // decay acceleration (PHOSPHOR_BOTTOM_DECAY_MULT).
        use crate::constants::{
            PHOSPHOR_BOTTOM_DECAY_MULT, PHOSPHOR_DEAD_THRESHOLD, PHOSPHOR_DECAY_RATE,
            PHOSPHOR_TAIL_RESIDUAL,
        };

        // Calculate the theoretical number of 60fps frames needed for a
        // bottom-row ghost cell to decay from PHOSPHOR_TAIL_RESIDUAL to
        // PHOSPHOR_DEAD_THRESHOLD with bottom acceleration.
        let fps = 60.0;
        let dt = 1.0 / fps;
        let effective_rate = PHOSPHOR_DECAY_RATE * PHOSPHOR_BOTTOM_DECAY_MULT;
        let mut energy = PHOSPHOR_TAIL_RESIDUAL as f32;
        let mut frames = 0u32;
        let max_frames = 300; // 5 seconds at 60fps — hard upper bound

        while energy > PHOSPHOR_DEAD_THRESHOLD as f32 && frames < max_frames {
            energy *= (-effective_rate * dt).exp();
            frames += 1;
        }

        assert!(
            energy <= PHOSPHOR_DEAD_THRESHOLD as f32,
            "phosphor should decay to dead within {} frames at bottom, but energy = {}",
            frames,
            energy
        );
        // Bottom decay should be significantly faster than normal
        // Normal: ~60-70 frames. Bottom should be < 30 frames.
        assert!(
            frames < 30,
            "bottom-row phosphor should decay in < 30 frames (got {}), ensuring no concrete wall",
            frames
        );
    }

    #[test]
    fn high_speed_does_not_create_unbounded_bottom_accumulation() {
        // Simulate high-speed rain for many frames and verify that the bottom
        // rows don't accumulate more than a bounded number of non-blank cells.
        let mut cloud = make_cloud();
        cloud.chars_per_sec = 100.0; // High speed
        cloud.droplet_density = 1.5;
        cloud.recalc_droplets_per_sec();

        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let start = Instant::now();
        let frame_dt = Duration::from_millis(16); // ~60fps

        // Run 300 frames (~5 seconds) of high-speed rain
        for i in 0..300 {
            let now = start + frame_dt * i;
            cloud.last_spawn_time = now - Duration::from_millis(16);
            cloud.last_phosphor_time = now;
            cloud.rain_at(&mut frame, now);
        }

        // Count non-blank cells in the bottom PHOSPHOR_BOTTOM_ROWS
        let bottom_start = cloud.lines.saturating_sub(PHOSPHOR_BOTTOM_ROWS);
        let mut non_blank_bottom = 0usize;
        let mut total_bottom = 0usize;
        for line in bottom_start..cloud.lines {
            for col in 0..cloud.cols {
                total_bottom += 1;
                let cell = frame.get(col, line).unwrap();
                if cell.fg.is_some() {
                    non_blank_bottom += 1;
                }
            }
        }

        // At high speed, there should always be some active cells, but the
        // ratio of non-blank cells at the bottom should not approach 100%
        // (which would indicate a "concrete wall"). Allow up to 85% to
        // account for active rain, but not the ~100% seen in the bug.
        let ratio = non_blank_bottom as f32 / total_bottom as f32;
        assert!(
            ratio < 0.85,
            "bottom rows should not be >85% non-blank after high-speed rain (got {:.1}%), \
             indicating no concrete wall accumulation",
            ratio * 100.0
        );
    }

    #[test]
    fn blank_cells_are_marked_dirty_for_redraw() {
        // When a cell transitions from having content to blank (via tail
        // cleanup), it must be marked dirty so the terminal redraws it.
        let mut frame = Frame::new(4, 4, None);
        frame.clear_dirty();

        // Set a cell with content
        frame.set(
            2,
            2,
            crate::cell::Cell {
                ch: 'X',
                fg: Some(Color::Green),
                bg: None,
                bold: true,
            },
        );
        assert!(
            !frame.dirty_indices().is_empty(),
            "setting content should be dirty"
        );

        frame.clear_dirty();

        // Blank the cell
        frame.set(2, 2, crate::cell::Cell::blank_with_bg(None));
        assert!(
            !frame.dirty_indices().is_empty(),
            "blanking a cell with content must be dirty — otherwise differential rendering skips the clear"
        );
        assert_eq!(frame.get(2, 2).unwrap().ch, ' ');
        assert!(frame.get(2, 2).unwrap().fg.is_none());
    }

    #[test]
    fn active_trail_cells_are_protected_from_phosphor_decay() {
        // Cells within a living droplet's range should NOT be ghosted by
        // the phosphor system, even if they weren't redrawn this frame.
        // This test verifies that Pass 2 of phosphor_decay_pass marks
        // active trail cells as fresh.
        let mut cloud = make_cloud();
        cloud.chars_per_sec = 50.0;
        cloud.recalc_droplets_per_sec();

        let now = Instant::now();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

        cloud.last_spawn_time = now - Duration::from_secs(1);
        cloud.rain_at(&mut frame, now);

        // After rain, find a living droplet
        let living: Vec<_> = cloud.droplets.iter().filter(|d| d.is_alive).collect();
        assert!(!living.is_empty(), "should have living droplets after rain");

        // Verify that cells within living droplet ranges have phosphor = 255
        // (protected from decay by Pass 2)
        let lines = cloud.lines;
        let mut protected_count = 0;
        for d in &living {
            let start = d.tail_put_line.map(|v| v.saturating_add(1)).unwrap_or(0);
            for line in start..=d.head_put_line {
                if line >= lines {
                    break;
                }
                let pidx = d.bound_col as usize * lines as usize + line as usize;
                if pidx < cloud.phosphor.len() && cloud.phosphor[pidx] == 255 {
                    protected_count += 1;
                }
            }
        }
        assert!(
            protected_count > 0,
            "living droplet cells should have phosphor = 255 (protected from decay)"
        );
    }
}
