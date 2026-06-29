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
//! - **AtmosphericEventManager**: Owns active events + trigger system.
//! - **EventTrigger**: Registered trigger conditions, evaluated once per frame.
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

// ── Event Manager ─────────────────────────────────────────────────────────

/// Manages active atmospheric events and their trigger conditions.
///
/// Owned by Cloud. Evaluates triggers once per frame at the top of
/// `rain_at()`. Renders active events after anomalies and before
/// atmospheric frame effects.
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
    }

    /// Returns true if no events are active.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Enable atmospheric events (called when entering interactive mode).
    pub fn enable_events(&mut self) {
        self.events_enabled = true;
    }

    /// Returns the number of active events.
    pub fn active_count(&self) -> usize {
        self.events.len()
    }

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
        let _elapsed = now
            .saturating_duration_since(self.last_trigger_eval)
            .as_secs_f64();
        self.last_trigger_eval = now;

        // Can't spawn more events if at max concurrent
        if self.events.len() >= EVENT_MAX_CONCURRENT {
            return;
        }

        // Evaluate each trigger
        for trigger in &mut self.triggers {
            if self.events.len() >= EVENT_MAX_CONCURRENT {
                break;
            }

            // Check cooldown
            if let Some(until) = trigger.cooldown_until {
                if now < until {
                    continue;
                }
            }

            let should_fire = match &mut trigger.condition {
                TriggerCondition::OnStartup { delay_ms, fired } => {
                    if *fired {
                        continue;
                    }
                    let elapsed = now.saturating_duration_since(self.birth_time).as_millis() as u64;
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
                    let roll: f32 = rand::distr::Uniform::new(0.0, 1.0)
                        .expect("[0,1) always valid")
                        .sample(&mut self.rng);
                    if roll < *chance_per_sec as f32 {
                        trigger.cooldown_until =
                            Some(now + std::time::Duration::from_secs_f64(*cooldown_secs));
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
                        trigger.cooldown_until =
                            Some(now + std::time::Duration::from_secs_f64(*cooldown_secs));
                        true
                    } else {
                        continue;
                    }
                }
                TriggerCondition::OnSceneEnter { cooldown_secs } => {
                    // Scene entry triggers are fired externally (via
                    // `fire_scene_entry`), not evaluated here.
                    trigger.cooldown_until =
                        Some(now + std::time::Duration::from_secs_f64(*cooldown_secs));
                    // Don't fire — this is set externally
                    continue;
                }
            };

            if should_fire {
                let intensity = 1.0 + (self.total_spawned as f32 * 0.03).min(0.3);
                let event: Box<dyn AtmosphericEvent> = Box::new(LightningEvent::new(
                    cols,
                    lines,
                    &mut self.rng,
                    intensity,
                    palette_color,
                ));
                self.events.push(event);
                self.total_spawned += 1;
            }
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
}
