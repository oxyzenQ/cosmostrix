# RFC: Atmospheric Event Engine for Cosmostrix v10.0.0

**Status:** Architecture Review (Phase 2A)
**Author:** Wolfzen (Principal Engine Architect)
**Date:** 2026-06-29
**Target:** Cosmostrix v10.0.0
**Scope:** Engine Design Only — No Implementation

---

## Executive Summary

This RFC proposes a production-quality **Atmospheric Event Engine** (AEE) as a new Cloud subsystem. The engine supports discrete cinematic visual events — lightning, energy surges, EMP pulses, ripple waves, solar flares, plasma bursts, and atmospheric glitches — without redesigning the existing renderer, breaking determinism, or introducing per-cell/per-frame allocations.

The core insight: Cosmostrix already has a mature **anomaly system** (`AnomalyZone`) that renders spatial effects as a post-process overlay. The Atmospheric Event Engine generalizes this pattern into a first-class, trait-driven event framework, making anomalies the "v0 prototype" of what AEE becomes.

---

## 1. Codebase Analysis Summary

### 1.1 Render Pipeline (rain_at — src/cloud/rain.rs)

The frame pipeline executes in this strict order:

```
1. Pause check → early return if paused
2. Color transition completion detection
3. Charset transition completion detection
4. RNG periodic re-seed
5. Cinematic resume easing (smoothstep time-scale)
6. Spawn scale computation (perf pressure + atmosphere + profile + emergent + resume + scene-entry)
7. Simulation update: advance droplets / monolith streams
8. Semantic invalidation + force_draw_everything cleanup
9. Build DrawCtx (read-only snapshot)
10. Droplet draw → frame.set() per cell
11. Message overlay → frame.set() per cell
12. Phosphor decay pass → frame.set() per afterglow cell
13. Anomaly spawn + apply → frame.set() per anomaly cell
14. Ecosystem ticks (color, atmosphere, memory, storytelling)
15. Profile interpolation tick
16. Atmospheric frame effects → frame.set() per dirty cell
17. Periodic full redraw check
18. Glitch timing update
19. Flash effect expiration
```

**Critical observation:** Anomaly application (step 13) already demonstrates the pattern — spatial effects rendered as a post-process overlay that writes to Frame via `frame.set()`. The AEE follows the same contract.

### 1.2 Frame System (src/frame.rs)

- Generation-based dirty tracking: each frame bump's `gen`, cells carry generation stamps
- `frame.set(x, y, cell)` — equality check before marking dirty
- `frame.set_force(x, y, cell)` — no equality check (used in hot paths)
- `frame.clear_with_bg()` — logical clear via gen bump (no buffer zeroing)
- `frame.invalidate_semantic()` — gen bump + semantic_gen increment for terminal sync
- Dirty indices tracked in SmallVec for O(dirty) post-processing

**Critical invariant:** Frame::set() is the single write mechanism. AEE writes cells the same way droplets do — no special render path needed.

### 1.3 Cloud State Architecture

- Cloud is the monolithic state owner: droplets, phosphor, ecosystem, anomalies, profiles, RNG, timers
- DrawCtx: read-only snapshot constructed per-frame before droplet draw
- Split-borrow pattern: Cloud mutably owns droplets, passes &DrawCtx to each droplet's draw()
- Subsystems (phosphor, spawn, ecosystem, scene_runtime) are impl blocks on Cloud, not separate structs with ownership

**Implication:** AEE should follow the same pattern — owned by Cloud, with a read-only "EventCtx" snapshot for event rendering, avoiding borrow conflicts.

### 1.4 Existing Anomaly System (src/cloud/phosphor.rs, src/cloud/state.rs)

```rust
enum AnomalyKind { LuminanceSurge, GlyphCorruption, PulseWave }
struct AnomalyZone { col, line, radius, kind, start_time }
```

- 3 anomaly types, all rendered via spatial iteration in `apply_anomalies()`
- Spawned probabilistically per-frame, capped at `ANOMALY_MAX_ZONES` (3)
- Duration: `ANOMALY_DURATION_SECS` (1.5s)
- Each renders by iterating O(radius²) cells and calling `frame.set()`
- LuminanceSurge: brightness boost in circular zone
- GlyphCorruption: random character replacement
- PulseWave: expanding ring of brightness

**Key lesson:** Anomalies prove that spatial overlay effects integrate cleanly into the existing pipeline. The AEE generalizes this.

### 1.5 Ecosystem Architecture (src/cloud/ecosystem.rs)

- 7 BehaviorProfiles with interpolated ProfileParams
- AtmosphericEvolution: entropy-phase sine waves for density/luminance/anomaly modulation on minute-scale cycles
- ColorEcosystem: slow luminance/saturation/hue climate drift (3s ticks)
- RendererMemory: 32-sample anomaly/density history
- StorytellingState: rare emergent moments from convergence conditions

**Key insight:** The ecosystem already has event scheduling infrastructure — `StorytellingState` checks convergence conditions and triggers `EmergentMoment`s. AEE expands this pattern.

### 1.6 Scene Runtime (src/cloud/scene_runtime.rs)

- Scene switching triggers: rain style transition, color change, charset change, speed/density/glitch adjustment
- `apply_scene_runtime()` is the integration point for scene-triggered events
- Glyph warm-start: sparse seed droplets for immediate visual feedback

### 1.7 Timing Infrastructure

- All timing via `Instant::now()`, passed through `rain_at(now)`
- Pause: shifts all timers by elapsed duration to prevent burst-fire on resume
- Resume: smoothstep S-curve over `RESUME_EASE_DURATION_SECS` (180ms)
- Performance pressure: adaptive spawn scale + glitch suppression
- Simulation time cap: `SIM_MAX_CAP_SECS` (1/30s) prevents catch-up jumps
- Idle detection: `IDLE_THRESHOLD_SECS` (30s) reduces FPS

---

## 2. Where Should the Atmospheric Event Engine Live?

### Recommendation: `src/cloud/atmospheric_events.rs` — a new Cloud submodule

**Rationale:**

| Option | Pros | Cons | Verdict |
|--------|------|------|---------|
| `src/atmospheric_events/` (top-level) | Clean namespace | No access to Cloud internals without pub(crate) sprawl; would require Cloud to expose too much | ❌ |
| `src/cloud/atmospheric_events.rs` (Cloud submodule) | Follows existing pattern (phosphor.rs, spawn.rs, ecosystem.rs); natural access to Cloud state; single-file simplicity | Cloud struct gains more fields | ✅ **Recommended** |
| Inside `src/atmosphere*.rs` | Near atmosphere subsystem | Atmosphere subsystem is about climate/color drift, not discrete visual events — semantic mismatch | ❌ |
| Inside `src/renderer*.rs` | Near rendering | Renderer is the hot path — coupling here increases risk of perf regression | ❌ |

The existing architecture already follows this pattern: `phosphor.rs`, `spawn.rs`, `ecosystem.rs`, `scene_runtime.rs`, `runtime_controls.rs`, `render.rs`, and `state.rs` are all `impl Cloud` submodules. Adding `atmospheric_events.rs` continues this convention.

### Module Structure

```
src/cloud/
  mod.rs                    — Cloud struct gains event-related fields
  atmospheric_events.rs     — AtmosphericEventManager + AtmosphericEvent trait + event types
  ... (existing files unchanged)
```

### Cloud struct additions (minimal)

```rust
// In Cloud struct:
pub(super) event_manager: AtmosphericEventManager,
```

All event state is encapsulated in `AtmosphericEventManager` — Cloud only needs one field.

---

## 3. Lifecycle State Machine

### 3.1 State Diagram

```
                    ┌──────────┐
           ┌───────►│   IDLE   │◄──────────┐
           │        └────┬─────┘           │
           │             │ trigger()       │ (reset)
           │             ▼                 │
           │        ┌──────────┐           │
           │        │ PENDING  │           │
           │        └────┬─────┘           │
           │             │ next frame      │
           │             ▼                 │
           │        ┌──────────┐           │
           │        │  SPAWN   │──(error)──┘
           │        └────┬─────┘
           │             │ spawn success
           │             ▼
           │        ┌──────────┐
           │        │  ACTIVE  │
           │        └────┬─────┘
           │             │ duration elapsed
           │             ▼
           │        ┌──────────┐
           │        │  DECAY   │
           │        └────┬─────┘
           │             │ decay complete
           │             ▼
           │        ┌──────────┐
           └────────│ FINISHED │
                    └──────────┘
```

### 3.2 State Definitions

| State | Description | Duration | Rendering |
|-------|-------------|----------|-----------|
| **Idle** | No event scheduled. Manager waiting for trigger. | Indefinite | None |
| **Pending** | Trigger fired but event not yet spawned. One-frame delay for deterministic scheduling. | 1 frame | None |
| **Spawn** | Event is being constructed (precompute paths, allocate buffers). | 1 frame | None |
| **Active** | Event is rendering each frame at full intensity. | Event-defined (e.g., 200ms for lightning) | Full intensity |
| **Decay** | Event has passed its peak. Rendering fades out via phosphor integration. | Event-defined (e.g., 500ms for lightning afterglow) | Fading intensity |
| **Finished** | Event complete. Buffers recycled. Transitions to Idle next frame. | 1 frame | None |

### 3.3 Why One-Frame Delays for Pending→Spawn and Finished→Idle?

- **Deterministic scheduling:** Trigger conditions are evaluated once per frame at a known point in the pipeline. A one-frame Pending state means the event spawns on the *next* frame's simulation update, allowing the current frame's rendering to complete cleanly.
- **Buffer recycling:** The Finished→Idle transition collects pre-allocated buffers for reuse. Doing this synchronously (not during rendering) avoids mutable aliasing issues.

### 3.4 Event-Specific Phases (Active + Decay)

Each event type defines its own phase progression within Active/Decay:

```
Lightning example:
  ACTIVE (0-200ms):
    0-50ms:   Strike — peak brightness, bolt appears
    50-200ms: Flash — secondary glow, branch visibility
  DECAY (200-700ms):
    200-400ms: Phosphor fade — afterglow via phosphor integration
    400-700ms: Residual — dim ghost, then cleared
```

---

## 4. Ownership

### Recommendation: `AtmosphericEventManager` owned by Cloud

```rust
// Conceptual — NOT implementation code
pub(super) struct AtmosphericEventManager {
    events: SmallVec<[Box<dyn AtmosphericEvent>; 2]>,  // max 2 concurrent
    prealloc_buffers: EventBufferPool,                   // reusable path/color buffers
    triggers: EventTriggerSet,                           // registered trigger conditions
    rng: StdRng,                                         // dedicated RNG (deterministic seed)
    state: EventEngineState,                             // IDLE/ACTIVE/etc.
}
```

### Ownership Chain

```
App
 └─ Cloud
     ├─ droplets: Vec<Droplet>
     ├─ phosphor: Vec<u8>
     ├─ ecosystem: ColorEcosystem, AtmosphericEvolution, ...
     ├─ anomaly_zones: Vec<AnomalyZone>
     └─ event_manager: AtmosphericEventManager  ← NEW
          └─ events: SmallVec<[Box<dyn AtmosphericEvent>; 2]>
               ├─ LightningEvent { precomputed_path, phase, intensity, ... }
               └─ RippleWaveEvent { center, wave_radius, intensity, ... }
```

### Why Not CloudState / Renderer / Atmosphere?

| Owner | Issue |
|-------|-------|
| Cloud (via manager) | ✅ Clear ownership, single writer, follows existing patterns |
| CloudState (hypothetical) | Cloud doesn't have a "state" struct — monolithic design |
| Renderer | Renderer is the Frame + TerminalWriter — no simulation logic |
| Atmosphere | Semantic mismatch: atmosphere = slow climate, events = discrete visual |

### Why a dedicated RNG?

AEE events must be **deterministically reproducible** for testing. Using Cloud's shared `mt: StdRng` would make event generation dependent on the exact sequence of droplet spawns, glitch rolls, etc. A dedicated RNG seeded at Cloud construction ensures event behavior is isolated and testable.

---

## 5. Scheduling — Trigger System

### 5.1 Trigger Architecture

Triggers are evaluated **once per frame** at the top of `rain_at()`, before simulation update (step 6 in the pipeline). This is the same point where spawn scale is computed — decisions about what to spawn belong together.

### 5.2 Trigger Types

```rust
// Conceptual
enum EventTrigger {
    /// Fire once after startup, after startup_delay_ms.
    OnStartup { delay_ms: u64 },

    /// Fire after idle_duration_secs of no user input.
    OnIdle { min_idle_secs: f64, cooldown_secs: f64 },

    /// Fire on scene transition to this scene.
    OnSceneEnter { scene_name: &'static str, cooldown_secs: f64 },

    /// Fire when anomaly density exceeds threshold.
    OnAnomalyDensity { threshold: f32, cooldown_secs: f64 },

    /// Fire probabilistically, with per-second chance and cooldown.
    OnAmbient { chance_per_sec: f64, cooldown_secs: f64 },

    /// Fire when atmosphere entropy phase crosses a threshold.
    OnEntropyPhase { phase: f32, tolerance: f32, cooldown_secs: f64 },

    /// Explicit programmatic trigger (future: scripts, keybinds).
    OnDemand,
}
```

### 5.3 No-Polling Guarantee

Triggers are stored in a `SmallVec` inside `AtmosphericEventManager`. Each frame, the manager iterates triggers (O(triggers), typically 3-5) and checks conditions. No background threads, no timers, no polling. Just a lightweight iteration at the top of `rain_at()`.

### 5.4 Trigger Registration

```rust
// Conceptual
impl AtmosphericEventManager {
    fn register_trigger(&mut self, trigger: EventTrigger, event_factory: EventFactory);
}
```

Triggers are registered at Cloud construction time and when scenes change. The factory is a function pointer or closure that produces a `Box<dyn AtmosphericEvent>`.

### 5.5 Cooldown and Gating

Every trigger has a cooldown period. After firing, the trigger is suppressed for `cooldown_secs`. Additionally:

- **Maximum concurrent events:** 2 (hard cap)
- **Performance gate:** No new events if `perf_pressure > 0.5`
- **Pause gate:** No new events while paused
- **Transition gate:** No new events during scene transitions (400ms grace)

### 5.6 Concrete Trigger Examples

| Event | Trigger | Rationale |
|-------|---------|-----------|
| Lightning (ambient) | `OnAmbient { chance_per_sec: 0.008, cooldown: 30.0 }` | ~1 bolt every 2 minutes on average, minimum 30s gap |
| Lightning (startup burst) | `OnStartup { delay_ms: 500 }` + spawn 1-3 bolts | Dramatic entry |
| Lightning (storm surge) | `OnAnomalyDensity { threshold: 0.5, cooldown: 60.0 }` | Reactive to high anomaly activity |
| Energy Surge | `OnEntropyPhase { phase: 0.25, tolerance: 0.05, cooldown: 120.0 }` | At quarter-cycle of entropy wave |
| EMP Pulse | `OnSceneEnter { scene: "signal", cooldown: 30.0 }` | Cinematic scene entry |
| Ripple Wave | `OnIdle { min_idle_secs: 120.0, cooldown: 180.0 }` | Rare ambient after prolonged inactivity |

---

## 6. Rendering Integration

### 6.1 Optimal Render Order

```
Step 10: Droplet draw → frame.set()
Step 11: Message overlay → frame.set()
Step 12: Phosphor decay pass → frame.set()
Step 13: Anomaly zones → frame.set()
── NEW ──
Step 13.5: Atmospheric Event render → frame.set()   ← AEE INSERTION POINT
── END NEW ──
Step 14: Ecosystem ticks
Step 15: Profile interpolation
Step 16: Atmospheric frame effects → frame.set()
```

### 6.2 Why This Position?

| Before phosphor? | ❌ Events should overlay on phosphor afterglow, not be erased by it |
| After phosphor, before anomalies? | ✅ Events render on top of phosphor, but anomalies are faster/shorter-lived and should overdraw events |
| After anomalies? | ✅ Events render first, then anomaly zones (small radius, short duration) can overdraw |
| After atmospheric effects? | ❌ Events should be affected by luminance/saturation climate modulation |
| Before message overlay? | ✅ Message always on top — user messages take priority |

Actually, re-evaluating: **Events should render AFTER anomalies**, not before. Anomalies are brief (1.5s), small-radius (3-8 cells) effects. Events like lightning are large, full-screen effects. Anomalies overdrawing a lightning bolt would look wrong. The correct order is:

```
Revised order:
  Step 12: Phosphor decay
  Step 13: Anomaly zones → small, brief, overdraw phosphor
  Step 13.5: Atmospheric Events → large, cinematic, overdraw anomalies  ← AEE
  Step 14: Ecosystem ticks
  Step 15: Profile interpolation
  Step 16: Atmospheric frame effects → climate modulation on everything below
  Step 11: Message overlay (already rendered earlier, stays on top)
```

Wait — step 11 (message) already ran before phosphor. So the effective order after phosphor is: Anomalies → Events → Atmospheric effects. Messages are already in the frame and won't be overridden unless someone calls `frame.set()` on the same cells. Events should **avoid message cells** by checking the message bounding box.

### 6.3 EventCtx — Read-Only Snapshot for Event Rendering

Following the DrawCtx pattern:

```rust
// Conceptual
pub struct EventCtx<'a> {
    pub lines: u16,
    pub cols: u16,
    pub bg: Option<Color>,
    pub color_mode: ColorMode,
    pub palette_colors: &'a [Color],
    pub phosphor: &'a [u8],            // Read phosphor state for blending
    pub message_bounds: Option<Rect>,  // Avoid overwriting message
    pub now: Instant,
}
```

This is constructed once per frame, before event rendering. Each active event receives `&EventCtx` and `&mut Frame`, identical to how droplets receive `&DrawCtx` and `&mut Frame`.

### 6.4 Frame Integration

Events write to Frame via the same `frame.set()` / `frame.set_force()` API used by droplets and phosphor. No special rendering path. The Frame's generation-based dirty tracking automatically handles delta detection.

### 6.5 Phosphor Integration for Afterglow

After an event's Active phase ends, its visual residue enters the Decay phase. Rather than implementing a separate decay system, event cells write to **both** Frame and Phosphor:

```
Event's final Active frame:
  For each event cell:
    frame.set(col, line, event_cell);
    phosphor[col, line] = decay_energy;     // seed phosphor for afterglow
    phosphor_base_fg[col, line] = event_fg; // capture color for ghost
```

The existing phosphor system then naturally handles the exponential decay. This is zero-cost: no new decay logic, just piggybacking on a proven system.

---

## 7. Lightning Event Design

### 7.1 Bolt Path Precomputation

At spawn time (Spawn state), a lightning bolt's path is fully computed and stored. No per-frame simulation.

**Algorithm:**
1. Choose start column (weighted toward center)
2. Choose end column (random, within `max_horizontal_span` of start)
3. Generate zigzag path:
   - Start at (start_col, 0) — top of screen
   - At each step, descend 1-3 rows, shift ±0-2 columns
   - Store each (col, line) in a pre-allocated `SmallVec<[(u16, u16); 200]>`
   - Stop when line >= terminal height
4. Optionally generate 1-3 branches:
   - Branch from a random point along the main bolt
   - Shorter path (30-50% of remaining length)
   - Wanders ±3 columns from origin

**Path characteristics:**
- Total path length: roughly equal to terminal height (25-60 segments)
- Branch count: 0-3 (probability-weighted: 40% no branch, 35% one, 20% two, 5% three)
- Zigzag frequency: direction changes every 2-5 rows
- Horizontal wander: max 40% of screen width from start

### 7.2 Visual Rendering (Active Phase: 0-200ms)

**Phase 1: Strike (0-50ms)**
- Main bolt rendered at full brightness (palette index max)
- Thick rendering: 2-3 adjacent columns illuminated (bell-shaped brightness)
- Branches at 70% brightness
- Bolt character: `│║┃┇┋` weighted by angle (vertical = `│`, diagonal = `/` or `\`)
- Optional: bolt core at index max+1 (white blend) for 1-2 cells near center

**Phase 2: Flash (50-200ms)**
- Background flash: cells within `flash_radius` (8-15 cols) of bolt get brightness boost
- Flash intensity follows gaussian falloff from bolt path
- Bolt remains visible at 80-100% brightness
- Branches fade to 50%

### 7.3 Decay Phase (200-700ms)

- Bolt cells transition to phosphor energy (seed at `PHOSPHOR_TAIL_RESIDUAL` = 160)
- Flash cells also seed phosphor
- No per-frame event rendering needed — phosphor handles the exponential decay
- At 700ms, any remaining phosphor energy is force-cleared (prevent indefinite afterglow)

### 7.4 Precomputed Data Structures

```rust
// Conceptual
struct LightningEvent {
    phase: LightningPhase,         // Strike, Flash, Decay, Finished
    phase_start: Instant,          // When current phase began
    spawn_time: Instant,           // When event was created

    // Precomputed at spawn — zero per-frame computation
    main_bolt: SmallVec<[(u16, u16); 200]>,    // (col, line) path
    branches: SmallVec<[SmallVec<[(u16, u16); 64]>; 3]>, // up to 3 branches
    flash_cells: SmallVec<[(u16, u16, f32); 1024]>, // (col, line, falloff_factor)
    bolt_chars: SmallVec<[char; 200]>,          // Preselected bolt characters

    // No per-frame allocation — all buffers precomputed at spawn
}
```

### 7.5 Memory Budget (Single Lightning Bolt)

| Buffer | Size | Bytes |
|--------|------|-------|
| main_bolt | 200 × (u16, u16) | 800 |
| branches | 3 × 64 × (u16, u16) | 768 |
| flash_cells | 1024 × (u16, u16, f32) | 8,192 |
| bolt_chars | 200 × char | 800 |
| **Total** | | **~10.5 KB** |

Pre-allocated once, reused across all lightning events via the `EventBufferPool`.

### 7.6 Rarity Calibration

- **Ambient rate:** ~0.008 chance/second → ~1 bolt every 125 seconds (2 minutes) average
- **Cooldown:** 30 seconds minimum between bolts
- **Startup burst:** 1-3 bolts within first 2 seconds
- **Anomaly surge:** When anomaly density > 0.5, ambient rate doubles
- **Max concurrent bolts:** 5 (but AEE global max is 2 concurrent events; startup burst spawns sequentially with 80ms stagger)

---

## 8. Future Expandability

### 8.1 The AtmosphericEvent Trait

The key to expandability is a trait that abstracts event behavior:

```rust
// Conceptual — design contract, not implementation
trait AtmosphericEvent: Send {
    /// Called once when event enters Spawn state. Precompute all paths/buffers.
    fn spawn(&mut self, cols: u16, lines: u16, rng: &mut StdRng);

    /// Called each frame during Active/Decay. Write visual output to Frame.
    fn render(&self, ctx: &EventCtx, frame: &mut Frame);

    /// Called each frame. Returns whether this event has finished.
    fn update(&mut self, now: Instant, elapsed: Duration) -> bool;

    /// Returns current state for lifecycle management.
    fn state(&self) -> EventState;

    /// Returns the total memory footprint (for debugging/monitoring).
    fn memory_footprint(&self) -> usize;

    /// Phase durations: tuple of (active_ms, decay_ms).
    fn phase_durations(&self) -> (u64, u64);
}
```

### 8.2 Implementing Future Events

Each new event type is a struct implementing `AtmosphericEvent`:

| Event | Implemented As | Complexity |
|-------|---------------|------------|
| **Lightning** | Precomputed zigzag path + branches + flash radius | Medium |
| **Energy Surge** | Vertical columns of boosted brightness, sweeping left→right | Simple |
| **EMP Pulse** | Expanding circle from center, phosphate-style desaturation | Simple |
| **Ripple Wave** | Multiple concentric expanding rings, like PondRipple ×5 | Medium |
| **Solar Flare** | Top-edge brightness burst that cascades downward with exponential fade | Simple |
| **Plasma Burst** | Localized sphere of color-shifted cells with smooth radial gradient | Simple |
| **Atmospheric Glitch** | Randomized block regions with character corruption + color inversion | Simple |
| **Future Cinematic Overlay** | Anything implementing the trait — full creative freedom | Variable |

### 8.3 No Renderer Changes Required

The trait-based design means:
- New event types are **new files** (`src/cloud/events/lightning.rs`, `src/cloud/events/pulse.rs`, etc.)
- Cloud's `rain_at()` only calls `event_manager.render(ctx, frame)` — one line
- The renderer pipeline (steps 1-18) remains unchanged
- No new branches in the hot path: `if events.is_empty() { return; }` is a single branch

### 8.4 Event Composition

Multiple events can be composed:
- Two events rendering simultaneously → both write to Frame independently
- Frame handles layering naturally via `set()` (last writer wins)
- Order within AEE: events rendered in insertion order (oldest first, newest last = on top)

### 8.5 Scripted Events (Future)

The `OnDemand` trigger enables programmatic event scheduling:
- Keybinding: press 'L' to trigger lightning
- Scene script: "after 3 seconds, trigger EMP pulse"
- Atmosphere state reaction: "if luminance_climate drops below 0.8 for 10+ seconds, trigger solar flare"
- External IPC: a Unix socket or named pipe for remote event injection (v11+)

---

## 9. Risk Analysis

### 9.1 Coupling Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Event rendering interferes with droplet rendering | Medium | Events render AFTER droplets step, so Frame.set() simply overwrites. Phosphor protects active trails. |
| Event borrows Cloud state mutably during render | High | EventCtx is immutable snapshot, identical to DrawCtx pattern. No borrow conflicts. |
| Event relies on Cloud internals that change | Medium | EventCtx provides a stable API surface. Cloud internal changes only affect EventCtx construction (one location). |
| Multiple events overdraw each other unpredictably | Low | Deterministic iteration order. Events are deliberately sparse — lightning is a thin line, ripple is a ring — so overlap is rare. |

### 9.2 Maintenance Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Trait method signatures need to change | Low | Trait is minimal (6 methods). Adding optional methods with defaults is backward-compatible. |
| Event types duplicate rendering logic | Medium | Shared rendering utilities extracted into `event_render_helpers.rs` (gaussian falloff, radial gradient, screen-space line drawing). These are pure functions, no state. |
| Phosphor integration bugs (afterglow persists forever) | Medium | Event sets explicit `max_decay_frames` after which phosphor is force-cleared. Lightning: 700ms total decay max. |
| RNG determinism drift between platforms | Low | Dedicated RNG seeded from Cloud's initial seed + event type discriminator. Fully deterministic. |

### 9.3 Performance Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Event rendering causes frame time spike | High | Precomputed paths at spawn (Spawn state, not during render). Render iterates SmallVec, O(path_length). Lightning: 200-400 cell writes per frame — comparable to a single droplet's draw(). |
| Multiple events compound cost | Medium | Hard cap of 2 concurrent events. Combined worst case: 800 cell writes. Still less than the droplet draw pass (typically 2000-5000 cell writes). |
| Per-frame allocation | High (zero tolerance) | All buffers pre-allocated at spawn. Render phase is read-only + Frame::set() calls. No Vec::push, no Box::new, no format!. |
| Phosphor seeding causes O(n) decay | Medium | Phosphor already handles O(active_phosphor) decay. Event cells join the existing phosphor_active set — no algorithmic change. |

### 9.4 Ownership Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Event lifetimes outlive Cloud | Low | Events owned by EventManager, which is owned by Cloud. Drop order is deterministic: Cloud::drop → EventManager::drop → events dropped. |
| Event references stale data after resize | Medium | Terminal resize triggers Cloud::reset(), which calls EventManager::reset(). All active events are force-finished; buffers are reallocated at new size. |
| Trait objects (Box<dyn AtmosphericEvent>) add virtual dispatch | Low | Dispatch only at event activation/deactivation (1-2 calls/frame), not in hot render loop. Render iterates known event struct, not trait objects. |

### 9.5 Rendering Order Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Event cells get wiped by later phosphor pass | High | Events render AFTER phosphor decay pass. Corrected: events in step 13.5, phosphor in step 12. |
| Event overwrites message box | Medium | EventCtx includes `message_bounds: Option<Rect>`. Events check bounds before rendering each cell (single branch: `if msg_bounds.contains(col, line) { continue; }`). |
| Atmospheric effects dim event cells | Low | This is desirable — events should respect luminance/saturation climate. A lightning bolt in a dim atmosphere should be dimmer. |
| Full redraw erases event residue | Low | Periodic full redraw (step 17) calls `force_draw_everything`, which triggers `clear_with_bg()`. Event cells are re-rendered in the same frame by the event render step. |

---

## 10. Final Architecture Recommendation

### 10.1 Single Recommended Architecture

**AtmosphericEventManager as a Cloud submodule (`src/cloud/atmospheric_events.rs`)**, with:

1. **Trait-based event system** (`AtmosphericEvent` trait) — 6 methods, zero-cost abstraction
2. **Precomputed, allocation-free rendering** — all paths/buffers computed at spawn time
3. **Phosphor integration for afterglow** — leverages existing decay infrastructure
4. **Trigger-driven scheduling** — no polling, no background threads, lightweight per-frame check
5. **Dedicated RNG** — deterministic, testable, isolated from Cloud's simulation RNG
6. **Post-anomaly render insertion** — step 13.5 in the pipeline, after phosphor + anomalies
7. **2-event hard cap** — prevents visual overload and frame time spikes
8. **Performance-gated** — no events when `perf_pressure > 0.5` or paused
9. **Resize-safe** — force-finish + reallocate on terminal resize

### 10.2 Why Not Alternative Architectures

| Alternative | Rejected Because |
|-------------|-----------------|
| **Inline in rain_at()** — add lightning rendering directly in the main loop | Couples event logic to render loop. Every new event type requires editing `rain_at()`. Violates open/closed principle. |
| **Separate render thread** — events computed on a background thread, written to a shared buffer | Breaks single-thread invariant. Introduces synchronization complexity. Overkill for terminal-sized rendering. |
| **GPU/compute shader** — offload event rendering to GPU | Cosmostrix is a terminal renderer. No GPU access. |
| **Event system in atmosphere crate** — put events in src/atmosphere*.rs | Atmosphere subsystem is about continuous climate/color modulation, not discrete visual events. Semantic mismatch. |
| **ECS (Entity-Component-System)** — use an ECS for event management | Massive overkill. Terminal rain ~500 droplets × 1 frame. ECS overhead exceeds rendering cost. |
| **Scripting engine** — embed Lua/Rhai for event scripting | v11+ feature. v10 should focus on engine quality, not scripting flexibility. |

### 10.3 Integration Summary

```
rain_at() modification (ONE new block, ~5 lines):

  // [existing] Step 13: Anomaly zones
  self.apply_anomalies(frame, now);

  // [NEW] Step 13.5: Atmospheric Event Engine
  if !self.event_manager.is_empty() {
      let event_ctx = EventCtx::new(self, now);
      self.event_manager.render(&event_ctx, frame);
      self.event_manager.update(now);
  }

  // [existing] Step 14: Ecosystem ticks
  ...
```

### 10.4 File Impact

| File | Change | Lines |
|------|--------|-------|
| `src/cloud/mod.rs` | Add `mod atmospheric_events;` + 1 field to Cloud struct | +3 |
| `src/cloud/atmospheric_events.rs` | New file: EventManager, AtmosphericEvent trait, EventCtx, trigger system | ~200 |
| `src/cloud/events/mod.rs` | New directory, module declarations | ~5 |
| `src/cloud/events/lightning.rs` | LightningEvent implementation | ~150 |
| `src/cloud/events/helpers.rs` | Shared rendering utilities (gaussian, radial, line drawing) | ~80 |
| `src/cloud/rain.rs` | 1 block insertion in `rain_at()` | ~8 |
| `src/cloud/spawn.rs` | Call `event_manager.reset()` on terminal resize | +1 |

**Total:** ~450 new lines, ~10 modified lines. Minimal blast radius.

### 10.5 Implementation Sequence (for Phase 2B onward)

1. **Phase 2B:** Implement `AtmosphericEvent` trait + `EventCtx` + `AtmosphericEventManager` skeleton (no event types yet). Verify it compiles and integrates into `rain_at()` as a no-op.
2. **Phase 2C:** Implement `LightningEvent` as the first concrete event type. Test with ambient trigger at high frequency for visual validation.
3. **Phase 2D:** Implement phosphor integration for lightning afterglow.
4. **Phase 2E:** Add startup burst + anomaly surge triggers.
5. **Phase 3+:** Implement remaining event types (Energy Surge, EMP Pulse, Ripple Wave, etc.) following the established pattern.

---

## Appendix A: Performance Budget Analysis

### Frame Cost Breakdown (current, at 60 FPS, 80×25 terminal)

| Step | Est. Cell Writes | Est. Time |
|------|-----------------|-----------|
| Droplet advance | 0 (sim only) | ~50μs |
| Droplet draw | 2,500-5,000 | ~200μs |
| Phosphor decay | 50-200 | ~30μs |
| Anomaly zones | 0-300 | ~20μs |
| Ecosystem ticks | 0 | ~10μs |
| Atmospheric effects | 50-200 | ~15μs |
| **Total (current)** | **~2,600-5,700** | **~325μs** |

### AEE Addition

| Step | Est. Cell Writes | Est. Time |
|------|-----------------|-----------|
| AEE (lightning, 1 bolt) | 150-400 | ~15μs |
| AEE (lightning, 2 bolts) | 300-800 | ~30μs |
| AEE (ripple wave) | 200-500 | ~20μs |
| **Worst case (2 events)** | **~800** | **~50μs** |

**Total frame with AEE:** ~375μs — well within the 16.6ms frame budget (60 FPS). AEE adds ~15% worst-case overhead.

### Memory Budget

| Component | Size |
|-----------|------|
| EventManager (struct) | ~200 bytes |
| EventBufferPool (prealloc) | ~32 KB |
| LightningEvent | ~11 KB |
| RippleWaveEvent | ~8 KB |
| EnergySurgeEvent | ~4 KB |
| **Total steady-state** | **~55 KB** |

Allocations happen once at Cloud construction. No per-frame or per-event allocation.

---

## Appendix B: Testability

### Deterministic RNG

```rust
// Event RNG seeded deterministically from Cloud's seed
let event_seed = CLOUD_RNG_SEED ^ EVENT_RNG_XOR;
let event_rng = StdRng::seed_from_u64(event_seed);
```

### Snapshot Testing

```rust
#[test]
fn lightning_event_is_deterministic() {
    let mut rng = StdRng::seed_from_u64(42);
    let bolt1 = LightningEvent::generate_path(80, 25, &mut rng);
    let mut rng = StdRng::seed_from_u64(42);
    let bolt2 = LightningEvent::generate_path(80, 25, &mut rng);
    assert_eq!(bolt1, bolt2); // Identical paths
}

#[test]
fn lightning_event_within_bounds() {
    let mut rng = StdRng::seed_from_u64(12345);
    let bolt = LightningEvent::generate_path(80, 25, &mut rng);
    for &(col, line) in &bolt.main_path {
        assert!(col < 80, "col {col} out of bounds");
        assert!(line < 25, "line {line} out of bounds");
    }
}
```

### Frame Integration Testing

```rust
#[test]
fn event_respects_message_bounds() {
    let mut frame = Frame::new(80, 25, None);
    let mut event = LightningEvent::new_test_bolt();
    let ctx = EventCtx {
        message_bounds: Some(Rect { x: 30, y: 10, w: 20, h: 5 }),
        ..EventCtx::default()
    };
    event.render(&ctx, &mut frame);
    // Verify no event cells inside message box
    for col in 30..50 {
        for line in 10..15 {
            let cell = frame.get(col, line).unwrap();
            assert_eq!(cell.ch, ' ', "event overwrote message at ({col}, {line})");
        }
    }
}
```

---

## Conclusion

The Atmospheric Event Engine is the correct architectural foundation for cinematic rendering events in Cosmostrix. It:

- **Does not modify the renderer** — events render as an overlay pass using the existing Frame API
- **Does not allocate per-cell or per-frame** — all paths precomputed at spawn, buffers reused
- **Preserves determinism** — dedicated RNG, deterministic path generation, snapshot-testable
- **Preserves frame stability** — hard caps, performance gating, O(active_cells) complexity
- **Integrates naturally** — follows the existing DrawCtx/phosphor/anomaly patterns
- **Enables future expansion** — trait-based design, new event types are isolated implementations
- **Respects the existing pipeline** — renders at the optimal insertion point, uses phosphor for decay

Lightning is the perfect first consumer: visually dramatic, architecturally simple (precomputed path + flash radius), and an ideal validation of the trait-based design before expanding to more complex event types.

**Recommendation: APPROVE for Phase 2B implementation.**
