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
//! Active → (recycle when is_finished())
//! ```
//!
//! Events are spawned `Active`, render every frame until `is_finished()`
//! returns true, then are recycled. The earlier `Decay` phase was
//! aspirational — no event implementation ever produced it, and the
//! `seed_phosphor`/`clean_stale_phosphor` machinery that depended on it
//! was unreachable. Both were removed in the v30 dragon-egg hunt.
use super::events::GhostEvent;
use crate::constants::*;
use crate::frame::Frame;
use rand::{distr::Distribution, rngs::StdRng, SeedableRng};
use smallvec::SmallVec;
use std::time::Instant;

// ── Public types ──────────────────────────────────────────────────────────

/// Read-only rendering context passed to event `render()` methods.
pub struct EventCtx {
    /// Terminal dimensions.
    pub cols: u16,
    pub lines: u16,
    /// Phase 3-I (Chroma Dragon Innovation I): palette-aware ghost base
    /// color. Derived from the current palette's darkest stop via
    /// `chroma::post::ghost::ghost_base_color()`. Replaces the hardcoded
    /// `(18, 22, 18)` in `cloud::events::ghost` — ghosts now match the
    /// scene's color scheme (green palette → dark green ghosts, red
    /// palette → dark red ghosts, etc.).
    pub ghost_base_color: (u8, u8, u8),
}

/// Trait for atmospheric event types.
///
/// Each event precomputes data at spawn; `render()` iterates stored data
/// with zero per-frame allocation. Lifecycle is binary: an event is
/// either alive (rendered each frame) or finished (recycled).
pub trait AtmosphericEvent: Send {
    /// Returns true when the event has finished and can be recycled.
    fn is_finished(&self) -> bool;

    /// Called each frame while alive. Writes visual output to Frame.
    fn render(&self, ctx: &EventCtx, frame: &mut Frame);

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

    // ── Trigger Evaluation ────────────────────────────────────────────────
    /// Evaluate triggers and spawn new ghost events as appropriate.
    /// Called once per frame before simulation update.
    ///
    /// v30 dragon-egg hunt: dropped three legacy parameters (`now`,
    /// `anomaly_density`, `palette_color`) that were computed by the
    /// caller every frame just to be passed in here and then ignored.
    /// The remaining parameters are all read by the trigger logic.
    #[allow(clippy::too_many_arguments)]
    pub fn evaluate_triggers(
        &mut self,
        perf_pressure: f32,
        cols: u16,
        lines: u16,
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

        self.try_spawn_ghost(cols, lines);
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
            if !event.is_finished() && event.is_pre_rain() == pre_rain {
                event.render(ctx, frame);
            }
        }
    }

    /// Recycle finished events. Called once per frame after rendering.
    ///
    /// v30 dragon-egg hunt: removed the `now` parameter (was only passed
    /// to `event.update(now)`, which was a no-op for every implementation)
    /// and the phosphor-seeding path that fired on Active→Decay
    /// transitions (no event ever entered Decay).
    pub fn update(&mut self) {
        let mut i = 0;
        while i < self.events.len() {
            if self.events[i].is_finished() {
                self.events.swap_remove(i);
                // Don't increment i — swap_remove moved last element to i
            } else {
                i += 1;
            }
        }
    }

    // ── Private Helpers ────────────────────────────────────────────────────

    /// Try to spawn a phosphor ghost kanji character.
    fn try_spawn_ghost(&mut self, cols: u16, lines: u16) {
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
