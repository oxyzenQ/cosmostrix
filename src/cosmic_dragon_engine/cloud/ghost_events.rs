// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cinematic Event Engine — ghost-kanji event system.
//!
//! Manages lifecycle of discrete cinematic visual events. Each event
//! implements `GhostEvent`; new types are added without modifying
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
pub(crate) struct EventCtx {
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
    /// (chroma audit, A9): cached ColorPipeline so the ghost event
    /// render can route its opacity fade through chroma::palette (chroma
    /// path) or chroma::legacy (legacy fallback). Copy enum, predicted-
    /// true in production.
    pub color_pipeline: crate::runtime::ColorPipeline,
    /// v30 Hinnant: frame-start Instant captured once in `rain_at()` and
    /// shared with all event `is_finished()` / `render()` calls. Removes
    /// 3 hidden `Instant::now()` syscalls per active event per frame (one
    /// in `is_finished` called from `render_phase`, one in `is_finished`
    /// called from `update`, one in `render`).
    pub now: Instant,
}

/// Trait for atmospheric event types.
///
/// Each event precomputes data at spawn; `render()` iterates stored data
/// with zero per-frame allocation. Lifecycle is binary: an event is
/// either alive (rendered each frame) or finished (recycled).
///
/// v30: renamed from `AtmosphericEvent` to `CinematicEvent` to avoid
/// collision with the `GhostEvent` struct (which implements this trait)
/// and to disambiguate from the deleted atmosphere engine subsystem.
/// The scheduler (`GhostEventScheduler`) is currently scoped to ghost
/// events only; the trait name is broader to allow future non-ghost
/// cinematic events without rename churn.
pub(crate) trait CinematicEvent: Send {
    /// Returns true when the event has finished and can be recycled.
    /// v30 Hinnant: takes `ctx` so implementations use `ctx.now` instead
    /// of `self.spawn_time.elapsed()` (which issues an `Instant::now()`
    /// syscall per call).
    fn is_finished(&self, ctx: &EventCtx) -> bool;

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
pub(crate) struct GhostEventScheduler {
    /// Active events (trait objects for polymorphism).
    events: SmallVec<[Box<dyn CinematicEvent>; 2]>,
    /// Dedicated RNG for deterministic event generation.
    rng: StdRng,
    /// Events are opt-in; disabled by default (tests, bench).
    events_enabled: bool,
}

impl GhostEventScheduler {
    /// Create a new event manager.
    ///
    /// The `now` parameter is unused by the event manager itself but is kept
    /// to match the uniform `Subsys::new(now)` constructor pattern used by
    /// every cloud subsystem (ColorEcosystem, EntropyDrift,
    /// RendererMemory, StorytellingState, GustState). See `cloud/mod.rs:397-405`
    /// for the constructor batch — every subsystem takes `now` so the batch
    /// reads symmetrically.
    pub(crate) fn new(_now: Instant) -> Self {
        let event_seed = RNG_INITIAL_SEED ^ EVENT_RNG_XOR;
        let rng = StdRng::seed_from_u64(event_seed);

        Self {
            events: SmallVec::new(),
            rng,
            events_enabled: false,
        }
    }

    /// Reset all state (terminal resize, scene change).
    ///
    /// Drops all active events — events are stateless between frames (no
    /// finalizer callback). The `now` parameter follows the same uniform-
    /// constructor convention documented on `new()`.
    pub(crate) fn reset(&mut self, _now: Instant) {
        self.events.clear();
    }

    /// Returns true if no events are active.
    pub(crate) fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Enable atmospheric events (called when entering interactive mode).
    pub(crate) fn enable_events(&mut self) {
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
    pub(crate) fn evaluate_triggers(
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
    pub(crate) fn render_pre_rain(&self, ctx: &EventCtx, frame: &mut Frame) {
        self.render_phase(ctx, frame, true);
    }

    /// Render post-rain events.
    pub(crate) fn render(&self, ctx: &EventCtx, frame: &mut Frame) {
        self.render_phase(ctx, frame, false);
    }

    fn render_phase(&self, ctx: &EventCtx, frame: &mut Frame, pre_rain: bool) {
        for event in &self.events {
            if !event.is_finished(ctx) && event.is_pre_rain() == pre_rain {
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
    ///
    /// v30 Hinnant: `ctx` is required to call `is_finished(ctx)` without
    /// issuing an `Instant::now()` syscall per event per frame.
    pub(crate) fn update(&mut self, ctx: &EventCtx) {
        let mut i = 0;
        while i < self.events.len() {
            if self.events[i].is_finished(ctx) {
                self.events.swap_remove(i);
                // Don't increment i — swap_remove moved last element to i
            } else {
                i += 1;
            }
        }
    }

    // ── Private Helpers ────────────────────────────────────────────────────

    /// Try to spawn a ghost kanji character.
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
        let event: Box<dyn CinematicEvent> = Box::new(GhostEvent::new(col, line, now));
        self.events.push(event);
    }
}
