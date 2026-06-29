// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Atmospheric Event Engine — trait-based cinematic event system.
//!
//! Future-proof: unused variants/methods are for upcoming event types.
#![allow(dead_code)]
//!
//! Manages the lifecycle of discrete cinematic visual events (lightning,
//! energy surges, ripple waves, etc.) as a Cloud submodule. Each event
//! implements the `AtmosphericEvent` trait; new event types are added
//! without modifying the renderer or event manager.
//!
//! ## Architecture
//!
//! - **AtmosphericEvent**: Trait for event types (render, update, lifecycle).
//! - **EventCtx**: Read-only rendering context, mirrors the DrawCtx pattern.
//! - **AtmosphericEventManager**: Owns active events + trigger system + Weather Director.
//! - **EventTrigger**: Registered trigger conditions, evaluated once per frame.
//!
//! ## Weather Director (v10.0.0 Phase 2D)
//!
//! An invisible electrical charge system [0.0, 1.0] accumulates from:
//! - Base idle rate (CHARGE_RATE_BASE)
//! - Anomaly zone density boost (CHARGE_RATE_ANOMALY_BOOST per zone)
//! - Enhanced rate during Storm Mode (CHARGE_RATE_STORM)
//!
//! The Weather Director uses charge level to make probabilistic strike
//! decisions, scaling ambient chance by charge saturation.
//!
//! ## Lifecycle
//!
//! ```text
//! Idle  →  Pending  →  Spawn  →  Active  →  Decay  →  Finished  →  Idle
//! ```
//!
//! Triggers fire events into the Pending queue. On the next frame,
//! Pending events spawn (precompute paths). Active events render each
//! frame. After their active duration, events enter Decay (phosphor
//! afterglow) then finish.

use std::time::Instant;

use crossterm::style::Color;
use rand::{distr::Distribution, rngs::StdRng, SeedableRng};
use smallvec::SmallVec;

use crate::constants::*;
use crate::frame::Frame;

use super::events::LightningEvent;

// ── Public types ──────────────────────────────────────────────────────────

/// Lifecycle state of an atmospheric event.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventState {
    /// No event scheduled.
    Idle,
    /// Trigger fired, awaiting next frame for spawn.
    Pending,
    /// Spawning — precomputing paths/buffers (one frame).
    Spawn,
    /// Rendering at full intensity each frame.
    Active,
    /// Fading out via phosphor integration.
    Decay,
    /// Complete; buffers will be recycled next frame.
    Finished,
}

/// Read-only rendering context passed to event `render()` methods.
/// Mirrors the `DrawCtx` pattern — avoids borrow conflicts with Cloud's
/// mutable state while providing everything events need to render.
pub struct EventCtx<'a> {
    /// Terminal dimensions.
    pub cols: u16,
    pub lines: u16,
    /// Background color for blank cells.
    pub bg: Option<Color>,
    /// Active palette colors for event rendering.
    pub palette_colors: &'a [Color],
    /// Current frame timestamp.
    pub now: Instant,
    /// Message box bounds if a message is active: (x, y, w, h).
    pub message_bounds: Option<(u16, u16, u16, u16)>,
    /// Whether a message is active (avoids recomputing bounds).
    pub has_message: bool,
}

/// Trait for atmospheric event types.
///
/// Each event type is a struct that precomputes all rendering data at spawn
/// time. The render() method iterates precomputed paths — zero per-frame
/// allocation, zero per-cell allocation.
pub trait AtmosphericEvent: Send {
    /// Returns the current lifecycle state.
    fn state(&self) -> EventState;

    /// Returns true when the event has finished and can be recycled.
    fn is_finished(&self) -> bool;

    /// Returns (active_duration_ms, decay_duration_ms).
    fn phase_durations_ms(&self) -> (u64, u64);

    /// Estimated memory footprint in bytes (for monitoring).
    fn memory_footprint(&self) -> usize;

    /// Called each frame. Updates internal phase state based on elapsed time.
    fn update(&mut self, now: Instant);

    /// Called each frame during Active phase. Writes visual output to Frame.
    fn render(&self, ctx: &EventCtx, frame: &mut Frame);

    /// Called when the event enters Decay phase. Seeds phosphor arrays
    /// with afterglow energy so the existing phosphor system handles fade-out.
    fn seed_phosphor(
        &self,
        phosphor: &mut [u8],
        phosphor_base_fg: &mut [Option<Color>],
        phosphor_base_ch: &mut [char],
        cols: u16,
        lines: u16,
    );

    /// Returns the current global illumination pulse factor [0.0, 1.0].
    /// Used during strike/peak moments to subtly brighten the entire scene.
    /// Default implementation returns 0.0 (no pulse).
    fn pulse_factor(&self, _now: Instant) -> f32 {
        0.0
    }
}

// ── Trigger types ─────────────────────────────────────────────────────────

/// A registered trigger condition paired with its event factory.
struct RegisteredTrigger {
    /// The condition to evaluate.
    condition: TriggerCondition,
    /// Cooldown until this trigger can fire again.
    cooldown_until: Option<Instant>,
}

/// Types of trigger conditions for atmospheric events.
#[allow(clippy::enum_variant_names)]
#[derive(Clone)]
enum TriggerCondition {
    /// Fire once after startup, after delay_ms.
    OnStartup { delay_ms: u64, fired: bool },
    /// Fire probabilistically per second, with minimum cooldown.
    OnAmbient {
        chance_per_sec: f64,
        cooldown_secs: f64,
    },
    /// Fire on scene entry (evaluated outside — this tracks cooldown only).
    OnSceneEnter { cooldown_secs: f64 },
    /// Fire when anomaly density exceeds threshold.
    OnAnomalyDensity { threshold: f32, cooldown_secs: f64 },
}

impl TriggerCondition {
    /// Human-readable label for debugging.
    fn label(&self) -> &'static str {
        match self {
            TriggerCondition::OnStartup { .. } => "startup",
            TriggerCondition::OnAmbient { .. } => "ambient",
            TriggerCondition::OnSceneEnter { .. } => "scene_enter",
            TriggerCondition::OnAnomalyDensity { .. } => "anomaly_density",
        }
    }
}

// ── Strike Record ─────────────────────────────────────────────────────────

/// A record of a recent lightning strike used for anti-repetition logic.
#[derive(Clone, Copy, Debug)]
struct StrikeRecord {
    /// Bolt family index (0-5).
    bolt_family: u8,
    /// Starting column of the strike.
    start_col: u16,
    /// Overall directional bias: negative = left, positive = right.
    direction: i8,
    /// Length as a fraction of screen height [0.0, 1.0].
    length_pct: f32,
}

// ── Event Manager ─────────────────────────────────────────────────────────

/// Manages active atmospheric events and their trigger conditions.
///
/// Owned by Cloud. Evaluates triggers once per frame at the top of
/// `rain_at()`. Renders active events after anomalies and before
/// atmospheric frame effects.
///
/// The Weather Director adds an invisible electrical charge system
/// that accumulates over time and drives probabilistic strike decisions.
pub(super) struct AtmosphericEventManager {
    /// Active events (trait objects for polymorphism).
    events: SmallVec<[Box<dyn AtmosphericEvent>; 2]>,
    /// Registered triggers with cooldowns.
    triggers: SmallVec<[RegisteredTrigger; 8]>,
    /// Dedicated RNG for deterministic event generation.
    rng: StdRng,
    /// When the event manager was created (for startup delay calculation).
    birth_time: Instant,
    /// Last time triggers were evaluated (for delta-time calculations).
    last_trigger_eval: Instant,
    /// Total events spawned since creation (for debugging).
    total_spawned: u64,
    /// Frame counter for stale phosphor cleanup.
    event_decay_frame: u64,
    /// Events are opt-in; disabled by default (tests, bench).
    events_enabled: bool,

    // ── Weather Director (v10.0.0 Phase 2D) ──
    /// Invisible electrical charge [0.0, 1.0].
    weather_charge: f32,
    /// Last weather evaluation timestamp.
    weather_last_tick: Instant,
    /// Storm Mode active flag.
    storm_mode_active: bool,
    /// When Storm Mode expires.
    storm_mode_end: Option<Instant>,
    /// Storm Mode cooldown — cannot re-activate before this time.
    storm_mode_cooldown: Option<Instant>,
    /// Recent strike history for anti-repeat.
    strike_history: SmallVec<[StrikeRecord; 8]>,
}

impl AtmosphericEventManager {
    /// Create a new event manager with default triggers registered.
    pub fn new(now: Instant) -> Self {
        let event_seed = RNG_INITIAL_SEED ^ EVENT_RNG_XOR;
        let rng = StdRng::seed_from_u64(event_seed);

        let mut mgr = Self {
            events: SmallVec::new(),
            triggers: SmallVec::new(),
            rng,
            birth_time: now,
            last_trigger_eval: now,
            total_spawned: 0,
            event_decay_frame: 0,
            events_enabled: false,
            weather_charge: 0.0,
            weather_last_tick: now,
            storm_mode_active: false,
            storm_mode_end: None,
            storm_mode_cooldown: None,
            strike_history: SmallVec::new(),
        };

        // Register default triggers
        mgr.register_trigger(TriggerCondition::OnStartup {
            delay_ms: LIGHTNING_STARTUP_DELAY_MS,
            fired: false,
        });
        mgr.register_trigger(TriggerCondition::OnAmbient {
            chance_per_sec: LIGHTNING_AMBIENT_CHANCE_PER_SEC,
            cooldown_secs: EVENT_AMBIENT_COOLDOWN_SECS,
        });
        mgr.register_trigger(TriggerCondition::OnAnomalyDensity {
            threshold: 0.5,
            cooldown_secs: 60.0,
        });

        mgr
    }

    /// Register a trigger condition.
    fn register_trigger(&mut self, condition: TriggerCondition) {
        if self.triggers.len() < self.triggers.capacity() {
            self.triggers.push(RegisteredTrigger {
                condition,
                cooldown_until: None,
            });
        }
    }

    /// Reset all state (terminal resize, scene change). Force-finishes
    /// active events and resets birth_time for startup trigger.
    pub fn reset(&mut self, now: Instant) {
        self.events.clear();
        self.birth_time = now;
        self.last_trigger_eval = now;

        // Reset startup trigger
        for trigger in &mut self.triggers {
            if let TriggerCondition::OnStartup { fired, .. } = &mut trigger.condition {
                *fired = false;
            }
            trigger.cooldown_until = None;
        }

        // Reset Weather Director state
        self.weather_charge = 0.0;
        self.weather_last_tick = now;
        self.storm_mode_active = false;
        self.storm_mode_end = None;
        self.storm_mode_cooldown = None;
        self.strike_history.clear();
    }

    /// Returns true if no events are active.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Returns the maximum global illumination pulse factor across all active events.
    /// Used to apply a subtle screen-wide brightness pulse during strike moments.
    pub fn global_pulse_factor(&self, now: Instant) -> f32 {
        self.events
            .iter()
            .map(|e| e.pulse_factor(now))
            .fold(0.0f32, f32::max)
    }

    /// Enable atmospheric events (called when entering interactive mode).
    pub fn enable_events(&mut self) {
        self.events_enabled = true;
    }

    /// Returns the number of active events.
    pub fn active_count(&self) -> usize {
        self.events.len()
    }

    // ── Weather Director ─────────────────────────────────────────────────

    /// Accumulate charge and evaluate weather state.
    /// Called periodically (every ~3s) from rain_at().
    pub fn weather_tick(&mut self, now: Instant, anomaly_count: usize, is_idle: bool) {
        // Only evaluate at WEATHER_TICK_SECS intervals
        let tick_elapsed = now
            .saturating_duration_since(self.weather_last_tick)
            .as_secs_f32();
        if tick_elapsed < WEATHER_TICK_SECS {
            return;
        }
        self.weather_last_tick = now;

        // Check Storm Mode expiry
        if self.storm_mode_active {
            if let Some(end) = self.storm_mode_end {
                if now >= end {
                    self.storm_mode_active = false;
                    self.storm_mode_end = None;
                    self.storm_mode_cooldown =
                        Some(now + std::time::Duration::from_secs_f32(STORM_MODE_COOLDOWN_SECS));
                }
            }
        }

        // Accumulate charge
        let rate = if self.storm_mode_active {
            CHARGE_RATE_STORM
        } else if is_idle {
            CHARGE_RATE_BASE * 0.7 // slower accumulation during idle
        } else {
            CHARGE_RATE_BASE
        };

        // Anomaly boost
        let anomaly_boost = anomaly_count as f32 * CHARGE_RATE_ANOMALY_BOOST;

        // Scale rate by tick interval
        self.weather_charge =
            (self.weather_charge + (rate + anomaly_boost) * tick_elapsed).min(1.0);

        // Slow passive decay when no storm and charge is moderate
        if !self.storm_mode_active && self.weather_charge < CHARGE_THRESHOLD_STRIKE {
            self.weather_charge = (self.weather_charge - 0.002 * tick_elapsed).max(0.0);
        }
    }

    /// Activate Storm Mode manually. Returns true if activated.
    pub fn activate_storm_mode(&mut self, now: Instant) -> bool {
        // Guard: already active
        if self.storm_mode_active {
            return false;
        }
        // Guard: on cooldown
        if let Some(cooldown) = self.storm_mode_cooldown {
            if now < cooldown {
                return false;
            }
        }

        self.storm_mode_active = true;
        self.storm_mode_end =
            Some(now + std::time::Duration::from_secs_f32(STORM_MODE_DURATION_SECS));
        self.storm_mode_cooldown = None;

        // Boost charge to at least the strike threshold
        if self.weather_charge < CHARGE_THRESHOLD_STRIKE {
            self.weather_charge = CHARGE_THRESHOLD_STRIKE;
        }

        true
    }

    /// Returns true if Storm Mode is currently active.
    pub fn is_storm_active(&self, now: Instant) -> bool {
        if !self.storm_mode_active {
            return false;
        }
        if let Some(end) = self.storm_mode_end {
            if now >= end {
                return false;
            }
        }
        true
    }

    // ── Bolt Family Selection ─────────────────────────────────────────────

    /// Select a bolt family with weighted probabilities.
    /// Returns (family_index, length_pct, brightness_mult).
    fn select_bolt_family(&mut self) -> (u8, f32, f32) {
        let uniform = rand::distr::Uniform::new(0.0f32, 1.0f32).expect("[0,1) always valid");
        let roll: f32 = uniform.sample(&mut self.rng);

        // Weighted selection with cumulative thresholds:
        // 0: Straight (30%), 1: Jagged (25%), 2: Forked (15%),
        // 3: Broken (10%), 4: Ribbon (12%), 5: Heavy (8%)
        let family = if roll < 0.30 {
            0u8
        } else if roll < 0.55 {
            1u8
        } else if roll < 0.70 {
            2u8
        } else if roll < 0.80 {
            3u8
        } else if roll < 0.92 {
            4u8
        } else {
            5u8
        };

        // During storm mode, bias toward more dramatic families
        let family = if self.storm_mode_active && family <= 2 {
            // Upgrade to heavier families
            if uniform.sample(&mut self.rng) < 0.4 {
                if family == 0 {
                    4u8
                } else {
                    family + 2
                }
            } else {
                family
            }
        } else {
            family
        };

        // Generate per-family length and brightness
        let len_min: f32;
        let len_max: f32;
        let brightness: f32;
        match family {
            0 => {
                len_min = 0.4;
                len_max = 0.9;
                brightness = 0.8;
            }
            1 => {
                len_min = 0.5;
                len_max = 1.0;
                brightness = 1.0;
            }
            2 => {
                len_min = 0.6;
                len_max = 1.0;
                brightness = 0.9;
            }
            3 => {
                len_min = 0.3;
                len_max = 0.7;
                brightness = 0.7;
            }
            4 => {
                len_min = 0.5;
                len_max = 1.0;
                brightness = 1.1;
            }
            _ => {
                len_min = 0.8;
                len_max = 1.0;
                brightness = 1.3;
            }
        }

        let length_pct = len_min + uniform.sample(&mut self.rng) * (len_max - len_min);
        (family, length_pct, brightness)
    }

    // ── Strike Decision ───────────────────────────────────────────────────

    /// Weather Director decides whether to schedule a strike.
    fn should_strike(&mut self) -> bool {
        let uniform = rand::distr::Uniform::new(0.0f32, 1.0f32).expect("[0,1) always valid");

        if self.weather_charge < CHARGE_THRESHOLD_STRIKE {
            return false;
        }

        // Probability scales with charge:
        // At threshold (0.45): ~10% chance
        // At high (0.75): ~60% chance
        // At full (1.0): ~90% chance
        let probability = if self.weather_charge >= CHARGE_THRESHOLD_HIGH {
            // High probability zone
            0.6 + (self.weather_charge - CHARGE_THRESHOLD_HIGH) / (1.0 - CHARGE_THRESHOLD_HIGH)
                * 0.3
        } else {
            // Threshold to high zone
            (self.weather_charge - CHARGE_THRESHOLD_STRIKE)
                / (CHARGE_THRESHOLD_HIGH - CHARGE_THRESHOLD_STRIKE)
                * 0.5
                + 0.1
        };

        // Storm Mode doubles the probability
        let probability = if self.storm_mode_active {
            probability * 2.0
        } else {
            probability
        };

        let roll: f32 = uniform.sample(&mut self.rng);
        roll < probability
    }

    // ── Anti-Repetition ───────────────────────────────────────────────────

    /// Record a strike in the history buffer.
    fn record_strike(&mut self, family: u8, start_col: u16, direction: i8, length_pct: f32) {
        if self.strike_history.len() >= STRIKE_HISTORY_SIZE {
            self.strike_history.remove(0);
        }
        self.strike_history.push(StrikeRecord {
            bolt_family: family,
            start_col,
            direction,
            length_pct,
        });
    }

    /// Check whether a proposed strike is too similar to recent history.
    /// Returns true if the strike would be repetitive and should be avoided.
    fn avoid_repetition(&self, family: u8, start_col: u16, direction: i8, length_pct: f32) -> bool {
        for record in &self.strike_history {
            // Same family AND close column AND same direction
            if record.bolt_family == family {
                let col_diff = start_col.abs_diff(record.start_col);
                if col_diff < STRIKE_HISTORY_COL_DISTANCE
                    && record.direction.signum() == direction.signum()
                {
                    return true;
                }
            }
            // Same family AND nearly identical length
            if record.bolt_family == family {
                let len_diff = (length_pct - record.length_pct).abs();
                if len_diff < 0.15 {
                    // Only block if length is also very similar
                    let col_diff = start_col.abs_diff(record.start_col);
                    if col_diff < STRIKE_HISTORY_COL_DISTANCE / 2 {
                        return true;
                    }
                }
            }
        }
        false
    }

    // ── Trigger Evaluation ────────────────────────────────────────────────

    /// Evaluate all triggers and spawn new events as appropriate.
    /// Called once per frame before simulation update.
    #[allow(clippy::too_many_arguments)]
    pub fn evaluate_triggers(
        &mut self,
        now: Instant,
        perf_pressure: f32,
        cols: u16,
        lines: u16,
        anomaly_density: f32,
        palette_color: Option<Color>,
        is_paused: bool,
        in_transition: bool,
    ) {
        // Events are opt-in — disabled in tests/benchmarks by default.
        if !self.events_enabled {
            return;
        }
        // Performance gate: skip trigger evaluation under high pressure
        if perf_pressure > EVENT_PERF_GATE || is_paused {
            return;
        }

        // Transition grace: skip during scene transitions
        if in_transition {
            return;
        }

        // Don't evaluate triggers too frequently (at most once per frame)
        let elapsed_sec = now
            .saturating_duration_since(self.last_trigger_eval)
            .as_secs_f64();
        self.last_trigger_eval = now;

        // Can't spawn more events if at max concurrent
        if self.events.len() >= EVENT_MAX_CONCURRENT {
            return;
        }

        // Phase 1: Evaluate triggers (borrows self.triggers mutably)
        let mut should_fire = false;
        {
            let triggers_len = self.triggers.len();
            for i in 0..triggers_len {
                if self.events.len() >= EVENT_MAX_CONCURRENT {
                    break;
                }

                // Check cooldown
                if let Some(until) = self.triggers[i].cooldown_until {
                    if now < until {
                        continue;
                    }
                }

                should_fire = match &mut self.triggers[i].condition {
                    TriggerCondition::OnStartup { delay_ms, fired } => {
                        if *fired {
                            continue;
                        }
                        let elapsed =
                            now.saturating_duration_since(self.birth_time).as_millis() as u64;
                        if elapsed >= *delay_ms {
                            *fired = true;
                            true
                        } else {
                            continue;
                        }
                    }
                    TriggerCondition::OnAmbient {
                        chance_per_sec,
                        cooldown_secs,
                    } => {
                        // Scale ambient chance by weather charge level:
                        // base_chance * (0.3 + charge * 1.4)
                        let charge_mult = 0.3 + self.weather_charge * 1.4;
                        let probability = (*chance_per_sec as f32)
                            * charge_mult
                            * (elapsed_sec as f32).min(1.0);
                        let roll: f32 = rand::distr::Uniform::new(0.0, 1.0)
                            .expect("[0,1) always valid")
                            .sample(&mut self.rng);
                        if roll < probability {
                            self.triggers[i].cooldown_until = Some(
                                now + std::time::Duration::from_secs_f64(*cooldown_secs),
                            );
                            true
                        } else {
                            continue;
                        }
                    }
                    TriggerCondition::OnAnomalyDensity {
                        threshold,
                        cooldown_secs,
                    } => {
                        if anomaly_density >= *threshold {
                            self.triggers[i].cooldown_until = Some(
                                now + std::time::Duration::from_secs_f64(*cooldown_secs),
                            );
                            true
                        } else {
                            continue;
                        }
                    }
                    TriggerCondition::OnSceneEnter { cooldown_secs } => {
                        self.triggers[i].cooldown_until = Some(
                            now + std::time::Duration::from_secs_f64(*cooldown_secs),
                        );
                        continue;
                    }
                };

                if should_fire {
                    break; // Only fire one event per frame
                }
            }
        }

        // Phase 2: Spawn event (self.triggers borrow released)
        if should_fire && self.events.len() < EVENT_MAX_CONCURRENT {
            // Weather Director decides: use charge-based strike decision
            if !self.should_strike() {
                return;
            }

            // Select bolt family and parameters
            let (bolt_family, length_pct, brightness) = self.select_bolt_family();

            // Generate candidate position
            let uniform =
                rand::distr::Uniform::new(0.0f32, 1.0f32).expect("[0,1) always valid");
            let col_range = (cols as f32 * 0.8) as u16;
            let col_start = (cols as f32 * 0.1) as u16;
            let start_col =
                col_start + (uniform.sample(&mut self.rng) * col_range as f32) as u16;

            // Direction bias from position on screen
            let center = cols / 2;
            let direction: i8 = if start_col > center { -1 } else { 1 };

            // Anti-repetition: avoid repeating similar strikes
            if self.avoid_repetition(bolt_family, start_col, direction, length_pct) {
                // Try up to 3 alternative positions
                let mut found = false;
                let mut best_family = bolt_family;
                let mut best_col = start_col;
                let mut best_dir = direction;
                let mut best_len = length_pct;

                for _attempt in 0..3 {
                    let (alt_family, alt_len, _alt_brightness) = self.select_bolt_family();
                    let alt_col =
                        col_start + (uniform.sample(&mut self.rng) * col_range as f32) as u16;
                    let alt_dir: i8 = if alt_col > center { -1 } else { 1 };
                    if !self.avoid_repetition(alt_family, alt_col, alt_dir, alt_len) {
                        best_family = alt_family;
                        best_col = alt_col;
                        best_dir = alt_dir;
                        best_len = alt_len;
                        found = true;
                        break;
                    }
                }
                if !found {
                    return;
                }

                // Generate with the alt parameters
                let (_, _, alt_brightness) = self.resolve_family_params(best_family);
                let final_intensity = 1.0 + (self.total_spawned as f32 * 0.02).min(0.2);
                let intensity = final_intensity * alt_brightness;

                // Determine return strokes
                let return_strokes = self.determine_return_strokes();

                let event: Box<dyn AtmosphericEvent> = Box::new(LightningEvent::new(
                    cols,
                    lines,
                    &mut self.rng,
                    intensity,
                    palette_color,
                    best_family,
                    best_len,
                    return_strokes,
                ));
                self.events.push(event);
                self.total_spawned += 1;

                // Record and discharge
                self.record_strike(best_family, best_col, best_dir, best_len);
            } else {
                let final_intensity = 1.0 + (self.total_spawned as f32 * 0.02).min(0.2);
                let intensity = final_intensity * brightness;

                // Determine return strokes
                let return_strokes = self.determine_return_strokes();

                let event: Box<dyn AtmosphericEvent> = Box::new(LightningEvent::new(
                    cols,
                    lines,
                    &mut self.rng,
                    intensity,
                    palette_color,
                    bolt_family,
                    length_pct,
                    return_strokes,
                ));
                self.events.push(event);
                self.total_spawned += 1;

                // Record and discharge
                self.record_strike(bolt_family, start_col, direction, length_pct);
            }

            // Discharge: reduce charge by configured fraction
            let discharge = self.weather_charge * CHARGE_DISCHARGE_PER_STRIKE;
            self.weather_charge = (self.weather_charge - discharge).max(CHARGE_MIN_RETAINED);
        }
    }

    /// Fire a scene-entry event (called from scene_runtime).
    pub fn fire_scene_entry(
        &mut self,
        now: Instant,
        cols: u16,
        lines: u16,
        palette_color: Option<Color>,
    ) {
        if self.events.len() >= EVENT_MAX_CONCURRENT {
            return;
        }
        // Check if scene_enter trigger is off cooldown
        let can_fire = self.triggers.iter().all(|t| {
            if let TriggerCondition::OnSceneEnter { .. } = &t.condition {
                t.cooldown_until.map_or(true, |until| now >= until)
            } else {
                true
            }
        });
        if !can_fire {
            return;
        }

        // Set cooldown on scene_enter triggers
        for trigger in &mut self.triggers {
            if let TriggerCondition::OnSceneEnter { cooldown_secs } = &trigger.condition {
                trigger.cooldown_until =
                    Some(now + std::time::Duration::from_secs_f64(*cooldown_secs));
            }
        }

        let event: Box<dyn AtmosphericEvent> = Box::new(LightningEvent::new(
            cols,
            lines,
            &mut self.rng,
            1.0,
            palette_color,
            1,   // Jagged by default for scene entry
            0.7, // 70% screen height
            0,   // No return strokes for scene entry
        ));
        self.events.push(event);
        self.total_spawned += 1;
    }

    /// Render all active events. Called after anomalies, before atmospheric effects.
    pub fn render(&self, ctx: &EventCtx, frame: &mut Frame) {
        for event in &self.events {
            if !event.is_finished() && event.state() == EventState::Active {
                event.render(ctx, frame);
            }
        }
    }

    /// Update event states and handle Decay→Finished transitions.
    /// Seeds phosphor when an event enters Decay phase.
    pub fn update(
        &mut self,
        now: Instant,
        phosphor: &mut [u8],
        phosphor_base_fg: &mut [Option<Color>],
        phosphor_base_ch: &mut [char],
        cols: u16,
        lines: u16,
    ) {
        let mut i = 0;
        let mut newly_decayed: SmallVec<[usize; 2]> = SmallVec::new();

        while i < self.events.len() {
            let was_active = self.events[i].state() == EventState::Active;
            self.events[i].update(now);

            if was_active && self.events[i].state() == EventState::Decay {
                newly_decayed.push(i);
            }

            if self.events[i].is_finished() {
                self.events.swap_remove(i);
                // Don't increment i — swap_remove moved last element to i
            } else {
                i += 1;
            }
        }

        // Seed phosphor for events that just entered decay
        for &idx in &newly_decayed {
            if idx < self.events.len() {
                self.events[idx].seed_phosphor(
                    phosphor,
                    phosphor_base_fg,
                    phosphor_base_ch,
                    cols,
                    lines,
                );
            }
        }
    }

    /// Clean up phosphor residue from events that have exceeded max decay frames.
    /// Should be called periodically to prevent indefinite afterglow.
    pub fn clean_stale_phosphor(
        &mut self,
        phosphor: &mut [u8],
        phosphor_base_fg: &mut [Option<Color>],
        phosphor_base_ch: &mut [char],
        phosphor_active: &mut SmallVec<[usize; 256]>,
        total_cells: usize,
    ) {
        self.event_decay_frame += 1;
        if self.event_decay_frame < EVENT_MAX_PHOSPHOR_DECAY_FRAMES {
            return;
        }
        self.event_decay_frame = 0;

        // Check if any events are still in decay — if so, don't clear yet
        let any_decaying = self.events.iter().any(|e| e.state() == EventState::Decay);
        if any_decaying {
            return;
        }

        // Force-clear event phosphor cells by removing them from active set
        // when no events are active or decaying. This prevents orphaned
        // phosphor from accumulating over long sessions.
        // We only clear cells seeded with EVENT_PHOSPHOR_SEED_ENERGY.
        let mut i = 0;
        while i < phosphor_active.len() {
            let pidx = phosphor_active[i];
            if pidx < total_cells
                && phosphor[pidx] > 0
                && phosphor[pidx] <= EVENT_PHOSPHOR_SEED_ENERGY
            {
                phosphor[pidx] = 0;
                phosphor_base_fg[pidx] = None;
                phosphor_base_ch[pidx] = '\0';
                phosphor_active.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }

    // ── Private Helpers ────────────────────────────────────────────────────

    /// Resolve brightness for a given bolt family (used during alternative
    /// position selection in anti-repetition path).
    fn resolve_family_params(&self, family: u8) -> (u8, f32, f32) {
        match family {
            0 => (0, 0.7, 0.8),
            1 => (1, 0.75, 1.0),
            2 => (2, 0.8, 0.9),
            3 => (3, 0.5, 0.7),
            4 => (4, 0.75, 1.1),
            _ => (5, 0.9, 1.3),
        }
    }

    /// Determine number of return strokes for a bolt.
    fn determine_return_strokes(&mut self) -> u8 {
        let uniform = rand::distr::Uniform::new(0.0f32, 1.0f32).expect("[0,1) always valid");
        let roll: f32 = uniform.sample(&mut self.rng);
        if roll < RETURN_STROKE_CHANCE {
            // 1 or 2 return strokes
            if uniform.sample(&mut self.rng) < 0.5 {
                1
            } else {
                (RETURN_STROKE_MAX as u8).min(2)
            }
        } else {
            0
        }
    }
}
