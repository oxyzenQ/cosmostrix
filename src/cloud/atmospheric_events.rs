// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Atmospheric Event Engine — cinematic event system for ghosts.
//!
//! Manages lifecycle of discrete cinematic visual events. Each event
//! implements `AtmosphericEvent`; new types are added without modifying
//! the renderer.
//!
//! ## Lifecycle
//!
//! ```text
//! Idle → Pending → Spawn → Active → Decay → Finished → Idle
//! ```
use super::events::GhostEvent;
use crate::constants::*;
use crate::frame::Frame;
use crossterm::style::Color;
use rand::{distr::Distribution, rngs::StdRng, SeedableRng};
use smallvec::SmallVec;
use std::time::Instant;

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
#[allow(dead_code)]
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
/// Each event precomputes data at spawn; render() iterates stored data
/// with zero per-frame allocation.
pub trait AtmosphericEvent: Send {
    /// Returns the current lifecycle state.
    fn state(&self) -> EventState;
    /// Returns true when the event has finished and can be recycled.
    fn is_finished(&self) -> bool;
    /// Returns (active_duration_ms, decay_duration_ms).
    #[allow(dead_code)]
    fn phase_durations_ms(&self) -> (u64, u64);
    /// Estimated memory footprint in bytes (for monitoring).
    #[allow(dead_code)]
    fn memory_footprint(&self) -> usize;
    /// Called each frame. Updates internal phase state based on elapsed time.
    fn update(&mut self, now: Instant);

    /// Called each frame during Active phase. Writes visual output to Frame.
    fn render(&self, ctx: &EventCtx, frame: &mut Frame);

    /// Called when the event enters Decay phase. Seeds phosphor arrays
    /// with afterglow energy.
    fn seed_phosphor(
        &self,
        phosphor: &mut [u8],
        phosphor_base_fg: &mut [Option<Color>],
        phosphor_base_ch: &mut [char],
        cols: u16,
        lines: u16,
    );

    /// Returns true if this event should render before rain (behind droplets).
    /// Ghost events render pre-rain so rain partially overwrites them.
    fn is_pre_rain(&self) -> bool {
        false
    }
}

// ── Event Manager ─────────────────────────────────────────────────────────
/// Manages active atmospheric events. Owned by Cloud.
pub(super) struct AtmosphericEventManager {
    /// Active events (trait objects for polymorphism).
    events: SmallVec<[Box<dyn AtmosphericEvent>; 2]>,
    /// Dedicated RNG for deterministic event generation.
    rng: StdRng,
    /// Total events spawned since creation (for debugging).
    total_spawned: u64,
    /// Frame counter for stale phosphor cleanup.
    event_decay_frame: u64,
    /// Events are opt-in; disabled by default (tests, bench).
    events_enabled: bool,
}

impl AtmosphericEventManager {
    /// Create a new event manager.
    pub fn new(_now: Instant) -> Self {
        let event_seed = RNG_INITIAL_SEED ^ EVENT_RNG_XOR;
        let rng = StdRng::seed_from_u64(event_seed);

        Self {
            events: SmallVec::new(),
            rng,
            total_spawned: 0,
            event_decay_frame: 0,
            events_enabled: false,
        }
    }

    /// Reset all state (terminal resize, scene change). Force-finishes
    /// active events.
    pub fn reset(&mut self, _now: Instant) {
        self.events.clear();
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
    #[allow(dead_code)]
    pub fn active_count(&self) -> usize {
        self.events.len()
    }

    // ── Trigger Evaluation ────────────────────────────────────────────────
    /// Evaluate triggers and spawn new ghost events as appropriate.
    /// Called once per frame before simulation update.
    #[allow(clippy::too_many_arguments)]
    pub fn evaluate_triggers(
        &mut self,
        _now: Instant,
        perf_pressure: f32,
        cols: u16,
        lines: u16,
        _anomaly_density: f32,
        _palette_color: Option<Color>,
        is_paused: bool,
        in_transition: bool,
    ) {
        // Events are opt-in — disabled in tests/benchmarks by default.
        if !self.events_enabled {
            return;
        }
        if perf_pressure > EVENT_PERF_GATE || is_paused {
            return;
        }
        if in_transition {
            return;
        }

        self.try_spawn_ghost(cols, lines, is_paused);
    }

    /// Render pre-rain events (ghosts, behind droplets).
    pub fn render_pre_rain(&self, ctx: &EventCtx, frame: &mut Frame) {
        self.render_phase(ctx, frame, true);
    }

    /// Render post-rain events.
    pub fn render(&self, ctx: &EventCtx, frame: &mut Frame) {
        self.render_phase(ctx, frame, false);
    }

    fn render_phase(&self, ctx: &EventCtx, frame: &mut Frame, pre_rain: bool) {
        for event in &self.events {
            if !event.is_finished()
                && event.state() == EventState::Active
                && event.is_pre_rain() == pre_rain
            {
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

    /// Clear phosphor residue from expired events to prevent indefinite afterglow.
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

        // Clear event-phosphor cells when no events are active/decaying
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

    /// Try to spawn a phosphor ghost kanji character.
    fn try_spawn_ghost(&mut self, cols: u16, lines: u16, is_paused: bool) {
        if !self.events_enabled || is_paused {
            return;
        }
        // Max 1 ghost active
        if self.events.iter().filter(|e| e.is_pre_rain()).count() >= GHOST_MAX_ACTIVE {
            return;
        }
        let uniform = rand::distr::Uniform::new(0.0f64, 1.0f64).expect("[0,1) valid");
        if uniform.sample(&mut self.rng) >= GHOST_SPAWN_CHANCE_PER_TICK {
            return;
        }
        let col = if cols > 5 {
            1 + (uniform.sample(&mut self.rng) * (cols - 5) as f64) as u16
        } else {
            1
        };
        let line = if lines > 3 {
            1 + (uniform.sample(&mut self.rng) * (lines - 3) as f64) as u16
        } else {
            1
        };
        let now = Instant::now();
        let event: Box<dyn AtmosphericEvent> = Box::new(GhostEvent::new(col, line, now));
        self.events.push(event);
        self.total_spawned += 1;
    }
}
