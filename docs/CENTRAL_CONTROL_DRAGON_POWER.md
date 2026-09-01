# `central_control_dragon_power/` — Power, Performance & Adaptive Coordinator

<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

> **Document ID**: POWER-ARCH-001
> **Date**: 2026-08
> **Scope**: `src/central_control_dragon_power/` — every tunable knob,
> every adaptive subsystem, and the `PowerManager` coordinator that
> unifies them
> **Audience**: cosmostrix contributors who need to understand, tune,
> or extend the power/perf/adaptive stack
> **Goal**: provide the complete map of the dragon power module — what
> each subsystem does, how the `PowerManager` resolves clashes, and
> where to look when fine-tuning or extending the adaptive layer

---

## 1. Why this doc exists

cosmostrix runs continuously for hours or days. The longer it runs,
the more power and performance concerns dominate: thermal throttling,
sustained CPU pressure, idle CPU step-down, memory pressure, kernel
reclaim, context-switch overhead, frame jitter drift. Each concern
has its own adaptive subsystem, and historically those subsystems
were scattered across `event_loop.rs`, `adaptive.rs`, and inline
constants — competing for the same resources without a single owner.

The dragon power module is the **single source of truth** for every
power, performance, and adaptive parameter. It mirrors the
`central_control_rains.rs` pattern (one file holding every rain
visual constant) and extends it to a **directory module** with
behavior submodules plus a coordinator (`PowerManager`).

This doc is the complete map of that directory:

- What each submodule owns
- How `PowerManager` resolves the 5 clash zones identified in the
  power audit
- The frame lifecycle (what the event loop calls, in what order)
- The thermal guard input API (feature #13) — what is ready, what
  is pending
- How to tune thresholds without surprising the adaptive layer
- The migration history (power audit consolidation -> directory module -> PowerManager)

---

## 2. Module layout

```text
src/central_control_dragon_power/
├── mod.rs              (467 LOC) — constants, PowerThresholds, re-exports
├── phase_predictor.rs  (222 LOC) — P1 PhasePredictor + local_secs_since_midnight
├── reclaim_state.rs    (188 LOC) — P2 adaptive_resync_interval + P4 ReclaimState
├── endurance_health.rs (264 LOC) — P5 EnduranceHealth score
├── self_healer.rs      (522 LOC) — PerformanceSelfHealer + SelfHealAction enum
└── power_manager.rs    (734 LOC) — PowerManager coordinator (Phase 3)
```

`mod.rs` declares each submodule `mod X;` and re-exports their public
items via `pub(crate) use X::*;`. The constants in `mod.rs` are
additionally re-exported to the rest of the crate by `constants.rs`
through `pub use central_control_dragon_power::*;`, so every call
site can write `crate::constants::SOME_CONSTANT` regardless of which
file declares it. No call-site changes are needed when a constant
moves between `constants.rs` and `central_control_dragon_power/mod.rs`.

`src/interactive/adaptive.rs` was the original home of P1/P2/P4/P5
plus the self-healer (1105 LOC originally). It is now a 93-LOC thin
shim that only re-exports the same public items from
`central_control_dragon_power`. New code should depend on
`crate::central_control_dragon_power::*` directly; the shim exists
for backward source compatibility with any in-flight branches.

---

## 3. Feature inventory (12 features)

The power audit identified 12 features that directly affect power,
performance, or adaptive behavior. Each is owned by exactly one
subsystem in this module.

| #  | Feature                    | Owner submodule / file                | Modifies                          |
|----|----------------------------|---------------------------------------|-----------------------------------|
| 1  | crystal-dragon            | `crystal_dragon_engine/` (external)   | ambient palette drift             |
| 2  | dynamic-default-fps       | `termdetect/`, `main.rs` (external) | base_target_fps                   |
| 3  | xterm.js cap + Tier 2     | `termdetect/`, `tier2.rs` (external)| base_target_fps + ANSI bytes      |
| 4  | adaptive throttling       | `power_manager.rs`                    | frame_period (idle × 0.5)         |
| 5  | phase predictor (P1)      | `phase_predictor.rs`                  | is_idle (OR'd with reactive)      |
| 6  | adaptive resync (P2)      | `reclaim_state.rs`                    | resync interval (20s/60s/120s)    |
| 7  | reclaim state (P4)        | `reclaim_state.rs`                    | madvise(MADV_DONTNEED)            |
| 8  | endurance health (P5)     | `endurance_health.rs`                 | score (RSS+jitter+ctxt)           |
| 9  | performance self-healer   | `self_healer.rs` + event_loop.rs      | scene + force_draw + madvise      |
| 10 | ambient scheduler         | `crystal_dragon_engine/ambient.rs` (external) | scene + palette                   |
| 11 | climate post-FX           | `chroma_dragon_engine/post/climate.rs` (external)   | per-cell RGB + spawn_scale        |
| 12 | perf_pressure pipeline    | `power_manager.rs` + cloud/rain.rs    | spawn + sim + glitch + vignette   |

Features 1, 2, 3, 10, and 11 are intentionally **outside** the dragon
power module — they are visual or terminal-detection concerns, not
power concerns. They consume the outputs of this module (effective
FPS, effective pressure, is_idle) but do not own power state.

---

## 4. The 5 clash zones

The power audit identified 5 high-risk areas where multiple writers
competed for the same resource without a coordinator. `PowerManager`
resolves zones 1, 3, and 4. Zones 2 and 5 are visual concerns and
remain coordinated by their existing mechanisms.

### Zone 1 — FPS / frame_period (4 writers -> 1 owner)

**Previously:** four independent writers competed for the frame
period: dynamic-default-fps (startup), xterm.js cap (Tier 1),
adaptive throttling (idle × 0.5), and the self-healer (low-power
scene fps=30). Each writer updated its own `Duration` local in
`event_loop.rs` and the resolution order was implicit.

**After migration:** `PowerManager::effective_fps(paused)` is the single
owner. The 4-writer cascade is collapsed into one method that
resolves in this order:

1. Paused -> `1000 / PAUSE_PERIOD_MS` (4 FPS at 250 ms)
2. Idle -> `base_target_fps * IDLE_FPS_FACTOR` (30 FPS at 60 base × 0.5)
3. Active -> `base_target_fps`

The upstream precedence chain (CLI > config > `dynamic_default` >
xterm.js cap) is **not** re-resolved by `PowerManager`. That chain
runs in `termdetect/mod.rs` / `main.rs` and produces a single
`base_target_fps` value, which `PowerManager::new(base_target_fps,
now)` consumes at startup. Live config reload calls
`PowerManager::set_target_fps(new_fps)`.

### Zone 2 — Scene / palette (3 writers, NOT power)

**Status:** unchanged. Two writers (crystal-dragon, self-healer
downgrade) compete for scene + palette. The
`scene_generation` counter is the reactive guard — every writer
increments it when it changes the scene, and downstream consumers
invalidate their caches when they observe a generation bump. This is
a visual concern (which scene is showing), not a power concern (how
hard the renderer is working), so it stays outside `PowerManager`.

### Zone 3 — Spawn rate / density (4 multipliers -> 1 input)

**Previously:** four layered multipliers in `cloud/rain.rs`
competed for spawn rate: `perf_pressure` clamp, entropy, profile, and
gust. None were aware of each other, and `perf_pressure` was a local
variable in `event_loop.rs` updated by inline math.

**After migration:** `PowerManager::effective_pressure()` is the single
read API for `perf_pressure`. The 4-multiplier cascade in
`cloud/rain.rs` is unchanged (it composes multiplicatively by
design), but it now reads from one source. Any future consumer of
perf_pressure — spawn rate, sim factor, glitch intensity, vignette
intensity — reads `effective_pressure()` instead of a local
variable.

**v80.0.0-beta.1 (2026-09-01):** two additions to this zone.

1. The render-path pressure FEED is gated on `power_dragon`
   (`event_loop_hud.rs::update_hud_state`): with the dragon off the
   cloud receives 0.0, so the documented promise "rain stays at
   user-configured density/speed regardless of CPU pressure" now
   holds on the density leg (v50 Option D gated only the HUD display
   while `rain_at()` kept throttling). The self-healer also releases
   a stale `aggressive_throttle` when the dragon turns off.
2. The spawn-scale curve is the owner's banded masterclass
   (`central_control_rains::density_throttle.rs::compute_spawn_scale`)
   with the configured density as the CEILING: dead zone p <= 0.05,
   low 0.84-0.70, medium 0.70-0.50, high (rare) 0.50-0.10; aggressive
   mode reads the pressure +0.20 deeper (same band edges). This
   replaced the v50 linear `1 - 0.75*p` curve
   (`PERF_PRESSURE_SPAWN_FACTOR` 0.75/0.9 + `PERF_SPAWN_SCALE_MIN`
   0.25/0.10), which cut density 0.85 -> ~0.47 at p ~0.6.

### Zone 4 — Kernel memory (madvise, already coordinated)

**Status:** already coordinated by `ReclaimState` (1-hour minimum
interval between hints). The self-healer P2 path bypasses the
cooldown when `EnduranceHealth` drops into the investigate band, but
this is intentional — P2 fires at most once per 30-second cooldown
window, well below the 1-hour reclaim interval. No clash in
practice; documented for completeness.

### Zone 5 — Per-cell color (2 writers, NOT power)

**Status:** unchanged. Climate post-FX and atmospheric post-FX both
modify per-cell RGB. They compose multiplicatively, and the
interaction is documented in `chroma_dragon_engine/post/climate.rs`. Visual
concern, not power; stays outside `PowerManager`.

---

## 5. `PowerManager` API reference

`PowerManager` is the unified coordinator. It owns three pieces of
state that were previously scattered as inline locals in
`event_loop.rs`:

- `perf_pressure: f32` — the 0.0–1.0 accumulator
- `last_input_time` + `phase_predictor` + `was_active` +
  `idle_started` — the idle-detection state
- `base_target_fps: f64` — the upstream-resolved target

Plus the thermal guard input:

- `thermal_pressure: f32` — 0.0 = cool, 1.0 = thermal emergency

The struct is intentionally NOT `Copy` — it owns a `PhasePredictor`
with EMA state that must be preserved across frames. The event loop
holds one instance by value and passes `&mut self` to the mutation
methods.

### Construction

```rust
use std::time::Instant;
use crate::central_control_dragon_power::PowerManager;

let now = Instant::now();
let mut power_manager = PowerManager::new(cfg.target_fps, now);
```

`now` seeds the idle timer so the first `begin_frame()` does not
falsely report idle.

### Frame lifecycle

The event loop calls the methods in this order every frame:

```text
┌─────────────────────────────────────────────────────────────┐
│ 1. begin_frame(now)        -> returns is_idle for this frame │
│ 2. effective_fps(paused)   -> frame_period = 1.0 / fps       │
│ 3. effective_pressure()    -> feed cloud + self-healer       │
│ 4. ── frame work happens ──                                │
│ 5. observe_frame_end(...)  -> updates perf_pressure          │
└─────────────────────────────────────────────────────────────┘
```

`is_idle()` returns the value computed by the last `begin_frame()`
call. `effective_pressure()` returns the value updated by the last
`observe_frame_end()` call (or 0.0 before the first frame).

### Method reference

| Method                              | Mutates | Returns       | Purpose                                              |
|-------------------------------------|---------|---------------|------------------------------------------------------|
| `new(base_fps, now)`                | —       | `Self`        | Construct with base FPS + idle-timer seed            |
| `with_thresholds(t)`                | —       | `Self`        | Test-only threshold override                         |
| `note_activity(now)`                | yes     | `()`          | User input arrived — reset idle timer                |
| `set_target_fps(fps)`               | yes     | `()`          | Live config reload changed the base FPS              |
| `set_thermal_pressure(p)`           | yes     | `()`          | Feature #13 input (0.0–1.0, clamped)                 |
| `begin_frame(now)`                  | yes     | `bool`        | Compute is_idle for this frame; update predictor     |
| `observe_frame_end(w, p, o)`        | yes     | `()`          | Update perf_pressure from overshoot + write latency  |
| `effective_pressure()`              | no      | `f32`         | Base pressure + thermal, clamped to [0,1]            |
| `effective_fps(paused)`             | no      | `f64`         | Pause / idle / active cascade                        |
| `is_idle()`                         | no      | `bool`        | Cached value from `begin_frame()`                    |
| `idle_started()`                    | no      | `Option<Instant>` | When the current idle window began               |
| `base_target_fps()`                 | no      | `f64`         | Read-only access for HUD + perf summary              |
| `phase_transitions_observed()`      | no      | `u64`         | For post-exit verbose summary                        |
| `phase_predictor()`                 | no      | `&PhasePredictor` | Test-only access to the predictor                |

---

## 6. Subsystem deep-dives

### 6.1 `phase_predictor.rs` — P1 Phase-Aware Adaptive Pacing

**What it does.** Learns the daily activity cycle from observed
active<->idle transitions and proactively predicts idle *before* the
reactive 30-second threshold fires. After ≥2 transitions the
predictor becomes a proactive idle signal.

**How it works.** Maintains two exponential moving averages (EMAs):
`active_start_ema` and `active_end_ema`, both in seconds since local
midnight. The learning rate is α=0.3. After enough cycles the EMAs
converge to the typical active window (e.g., 09:00 -> 17:00). On each
`begin_frame()`, `predicts_active(secs_since_midnight)` returns
`Some(true)` if the current local time falls inside the EMA window,
`Some(false)` if outside, or `None` if insufficient data.

**Midnight wrap-around.** If `active_start_ema > active_end_ema`
the active window crosses midnight (e.g., 22:00 -> 06:00 for a night
shift). The predictor handles this correctly by OR'ing the two
intervals.

**Local time source.** `local_secs_since_midnight()` uses libc
`localtime_r` on Linux (thread-safe POSIX call) and falls back to
UTC seconds on other platforms. No `chrono` dependency.

**Why it matters.** On a long-endurance run (24+ hours), the
reactive 30-second idle threshold fires after every user pause —
lunch, meetings, end-of-day. The predictor learns those boundaries
and steps the renderer down *before* the threshold fires, smoothing
the CPU step-down. The reactive threshold remains as a safety net.

**Tests.** 5 tests in `phase_predictor.rs`: empty predictor returns
`None`, two-transition predictor returns correct active/idle for
noon and evening, midnight wrap-around, transition count tracking,
EMA convergence to within 100 seconds of the true boundary.

### 6.2 `reclaim_state.rs` — P2 IPAC + P4 MPAR

Two tightly-coupled idle-mode subsystems sharing the same underlying
concern: what should cosmostrix do when nothing has changed for a
long time.

**P2 (IPAC) — `adaptive_resync_interval(idle_secs)`.** Progressively
stretches the idle redraw resync interval based on sustained idle
duration:

- < 1 hour idle -> 20 seconds (standard)
- 1–4 hours idle -> 60 seconds (3× reduction)
- > 4 hours idle -> 120 seconds (6× reduction)

This reduces forced redraw CPU spikes during long idle periods. On a
24-hour run with 13 hours of idle, this cuts ~46,800 forced redraws
down to ~390 — a 99% reduction in idle CPU work.

**P4 (MPAR) — `hint_reclaim_pages(ptr, len)` + `ReclaimState`.**
During sustained idle, the previous-generation dirty regions in the
frame buffer are no longer needed. `madvise(MADV_DONTNEED)` tells
the Linux kernel these pages can be reclaimed without swapping —
they'll be zero-filled on next access. This smooths the RSS
step-down that the kernel would otherwise perform as a sudden event
during memory pressure.

`ReclaimState` rate-limits the hints to once per hour. Without this,
every idle resync would issue a madvise syscall, which on a 12-hour
idle window is 12 syscalls — negligible cost — but on a pathological
loop could become a thundering herd. The 1-hour minimum is
defensive, not load-driven.

**Platform support.** `hint_reclaim_pages` is `cfg(target_os =
"linux")`. On other platforms it's a no-op. `ReclaimState` is
cross-platform (it just tracks timestamps).

**Tests.** 5 tests: standard interval under 1 hour, 60s after 1
hour, 120s after 4 hours, initial state should-reclaim,
min-interval respected.

### 6.3 `endurance_health.rs` — P5 Endurance Health Score

**What it does.** A single 0–100 metric tracking long-endurance
process stability based on three orthogonal signals:

- **Memory stability** (40% weight) — RSS variance over a ring
  buffer of 60 readings. Lower variance = higher score. Sampled on
  Linux via `/proc/self/status`.
- **Frame jitter** (35% weight) — EMA of frame time in ms. Lower
  jitter = higher score. Cross-platform.
- **Context switch rate** (25% weight) — EMA of voluntary switches
  per second. Lower rate = higher score. Sampled on Linux via
  `/proc/self/stat`.

Memory dominates the score because RSS variance is the earliest
indicator of a stuck or leaking long-endurance process.

**Classification bands.**

- `>= 80` -> `"healthy"` — process is stable, no action needed.
- `60–80` -> `"degraded"` — mild instability, monitor.
- `< 60` -> `"investigate"` — significant instability; the P2
  self-healer uses this band to trigger an immediate frame
  invalidate + memory reclaim hint.

**Sampling is unconditional.** The score is computed every
recompute cycle (~1 second on a typical run) regardless of whether
`--perf-stats` is enabled. This was a deliberate fix in Phase 1
(commit `e487cf8`) — the previous code gated RSS/context-switch
sampling on `--perf-stats`, which meant the self-healer P2 path
could never fire on a production run (where `--perf-stats` is off by
default). The fix moved the sampling outside the `--perf-stats`
gate; `--perf-stats` now only controls whether the score is
*printed* in the post-run summary, not whether it is *computed*.

**Tests.** 5 tests: initial score is 100, RSS variance lowers the
score, classification bands, sampling works without perf-stats,
context-switch rate influence.

### 6.4 `self_healer.rs` — Performance Self-Healer (P1 + P2)

**What it does.** Encapsulates two orthogonal auto-mitigation
policies as a single state machine so the event loop only needs to
call `observe(...)` once per frame and apply the returned
`SelfHealAction`:

- **P1 (auto scene downgrade)** — when `perf_pressure` stays at or
  above `SELF_HEAL_PRESSURE_HIGH` (0.6) for
  `SELF_HEAL_DOWNGRADE_SECS` (30s), switch to the lighter fallback
  scene ("low-power") to shed load. When pressure stays at or below
  `SELF_HEAL_PRESSURE_LOW` (0.3) for `SELF_HEAL_RESTORE_SECS` (60s),
  restore the prior scene. Hysteresis gap (0.6 -> 0.3) and a
  middle-band dead zone prevent flapping under borderline load.
- **P2 (EnduranceHealth mitigation)** — when the
  `EnduranceHealth` score drops below
  `SELF_HEAL_HEALTH_INVESTIGATE` (60.0, the "investigate" band),
  trigger an immediate frame invalidate + memory reclaim hint
  (`madvise(MADV_DONTNEED)`) to clear potential stuck state. The
  `SELF_HEAL_HEALTH_COOLDOWN_SECS` (30s) cooldown prevents a
  persistently unhealthy process from force-redrawing every
  recompute cycle.

**P2 evaluation order.** P2 (health mitigation) is checked *before*
P1 (scene actions) on every tick. Rationale: P2 is a symptom-level
response (force redraw + madvise), while P1 is a cause-level
response (shed load). If both fire on the same tick, the symptom
fix lands first so the next health recompute sees a cleaner state.

**Pure policy.** The self-healer is a *pure policy* — it does not
touch `Cloud`, `Frame`, or stdout directly. It returns a
`SelfHealAction` enum (`None`, `DowngradeScene`, `RestoreScene`,
`TriggerHealthMitigation`), and the event loop applies it. This
keeps the side-effect surface testable in isolation and lets the
event loop batch or defer actions as needed (e.g., skip a downgrade
when the user is in fixed mode or has explicitly chosen a scene).

**State machine.**

```text
                ┌─────────────────────┐
                │   Healthy (Normal)  │ <-─ default state
                │ pre_degraded = None │
                └──────────┬──────────┘
      sustained high       │       sustained low
       pressure (30s)      │       pressure (60s)
            ▼             │             ▼
┌───────────────────┐     │     ┌───────────────────┐
│  Downgraded       │◄────┴────►│  Healthy          │
│  pre_degraded=Some│           │  pre_degraded=None│
└───────────────────┘           └───────────────────┘
```

P2 (health mitigation) is orthogonal — it can fire from either the
Healthy or Downgraded state and does not change the P1 state.

**Tests.** 15 tests covering: initial state, sustained-high
accumulation, downgrade trigger after 30s, hysteresis break on
single cool frame, restore trigger after 60s, P2 health mitigation
fires below threshold, P2 cooldown respected, P2 fires from both
Healthy and Downgraded states, action enum equality, and several
edge cases around partial windows and clock jumps.

### 6.5 `power_manager.rs` — `PowerManager` coordinator

The Phase 3 coordinator. See §5 for the full API reference. The
implementation is straightforward — every method is a thin wrapper
over the previously-inline logic — but the *value* is in the
ownership model: there is now exactly one struct that downstream
code can ask "what is the effective pressure?" / "what is the
effective FPS?" / "is the process idle?" and get a consistent
answer.

**Thermal guard (feature #13) input API.** The thermal guard is
implemented as an INPUT to `effective_pressure()`, NOT as a 7th
independent signal path. Callers feed a 0.0–1.0 thermal pressure
scalar via `set_thermal_pressure(pressure)`; it is added to the
base `perf_pressure` and clamped to 1.0. This means every downstream
consumer of `effective_pressure()` — spawn cascade, self-healer, sim
factor — automatically responds to thermal throttling without
per-consumer wiring.

The actual thermal sensor sampling (Linux
`/sys/class/thermal/thermal_zone*/temp`, macOS SMC, Windows WMI) is
a future feature. The input API is ready so the sampling layer can
be added without touching `PowerManager` internals. Until the
sampler is wired, `set_thermal_pressure` is exercised only by tests;
the production call site will live in the event loop.

**Tests.** 25 tests in `power_manager.rs` covering: construction
(starts active, zero pressure, fps clamped to ≥1.0), pressure
accumulation on overshoot, pressure decay on normal frame, thermal
input added to base, thermal + base clamps to 1.0, thermal input
clamped to [0,1], effective_fps cascade (active/idle/paused),
paused overrides idle, `set_target_fps` live reload, `begin_frame`
active when input recent, idle after threshold, `idle_started`
tracked only on first transition, `note_activity` clears idle,
`PowerThresholds` overrides via `with_thresholds`, and several edge
cases around zero/negative FPS and clock jumps.

---

## 7. `PowerThresholds` struct reference

`PowerThresholds` is the grouped declaration of every tunable power
threshold. It lives in `mod.rs` and is consumed by `PowerManager` at
construction time via `PowerThresholds::defaults()`.

```rust
#[derive(Clone, Copy, Debug)]
pub(crate) struct PowerThresholds {
    pub pressure_high: f32,           // SELF_HEAL_PRESSURE_HIGH (0.6)
    pub pressure_low: f32,            // SELF_HEAL_PRESSURE_LOW (0.3)
    pub downgrade_secs: f64,          // SELF_HEAL_DOWNGRADE_SECS (30.0)
    pub restore_secs: f64,            // SELF_HEAL_RESTORE_SECS (60.0)
    pub health_investigate: f64,      // SELF_HEAL_HEALTH_INVESTIGATE (60.0)
    pub health_cooldown_secs: f64,    // SELF_HEAL_HEALTH_COOLDOWN_SECS (30.0)
    pub idle_threshold_secs: f64,     // IDLE_THRESHOLD_SECS (30.0)
    pub idle_fps_factor: f64,         // IDLE_FPS_FACTOR (0.5)
    pub pressure_increment: f32,      // PERF_PRESSURE_INCREMENT (0.25)
    pub pressure_decay: f32,          // PERF_PRESSURE_DECAY (0.02)
}
```

**Status.** Four fields are actively read by `PowerManager`:
`idle_threshold_secs`, `idle_fps_factor`, `pressure_increment`,
`pressure_decay`. Six fields (the self-healer P1 + P2 thresholds)
are read by tests and by the struct's own `defaults()` constructor;
the production self-healer currently reads the same values from the
standalone constants in `mod.rs`. Migrating `PerformanceSelfHealer`
to read from `PowerThresholds` is a follow-up step — the fields are
kept in the struct so it is the canonical declaration of every
tunable power threshold.

**Why the struct exists.** The standalone constants are the active
source of truth today; the struct is the migration target. Keeping
both in sync is enforced by the `power_thresholds_defaults_match_constants`
test, which asserts every struct field equals its corresponding
constant. When the self-healer migration lands, the constants can
be removed and the struct becomes the sole source.

**Test-only override.** `PowerManager::with_thresholds(t)` is
`#[cfg(test)]` only — production code always uses
`PowerThresholds::defaults()`.

---

## 8. Event loop wiring

The full integration lives in `src/interactive/event_loop.rs`. The
key call sites:

| Line   | Call                                                    | Purpose                                  |
|--------|---------------------------------------------------------|------------------------------------------|
| 143    | `PowerManager::new(cfg.target_fps, Instant::now())`     | Construct at startup                     |
| 402    | `power_manager.set_target_fps(safe_fps)`                | Live config reload                       |
| 504    | `power_manager.begin_frame(loop_now)`                   | Compute is_idle for this frame           |
| 507    | `power_manager.idle_started()`                          | Read idle window start (resync tier)     |
| 626–813| `register_activity(&mut power_manager, ...)`            | Reset idle timer on user input (8 sites) |
| 948    | `power_manager.effective_fps(cloud.pause)`              | Resolve frame period                     |
| 958    | `power_manager.is_idle()`                               | Adaptive resync scheduling               |
| (hud)  | `cloud.set_perf_pressure(applied)` (event_loop_hud.rs, v80.0.0-beta.1 gated on `power_dragon`) | Feed spawn cascade |
| 970    | `power_manager.effective_pressure() as f64 * SIM_PRESSURE_SCALE_FACTOR` | Sim factor      |
| 1075   | `power_manager.observe_frame_end(work_s, p, o)`         | Update perf_pressure                     |
| 1140–41| `power_manager.effective_pressure()`                    | Post-run perf summary (avg + max)        |
| 1174   | `power_manager.effective_pressure()`                    | Post-run perf summary (final line)       |
| 1397   | `power_manager.base_target_fps()`                       | HUD `tgt:` line                          |
| 1415   | `power_manager.phase_transitions_observed()`            | Post-exit verbose summary                |

`activity.rs::register_activity` was migrated in Phase 3 step 2 —
its signature changed from `&mut Instant` (the old
`last_input_time` local) to `&mut PowerManager`. It internally
calls `power_manager.note_activity(now)`. Three tests in
`interactive/tests.rs` were updated to match the new signature.

`activity.rs::is_runtime_idle` and `idle_resync_due` are now
`#[cfg(test)]` only — the production event loop delegates idle
detection to `PowerManager::begin_frame()`. Tests use them to
verify the idle-detection threshold logic in isolation.

---

## 9. Tuning guide

Every threshold in this module is a named constant in `mod.rs` with
a doc comment explaining its value and the reasoning behind it.
Tuning is intentionally low-friction:

1. Find the constant in `src/central_control_dragon_power/mod.rs`.
2. Read the doc comment — it explains the safe tuning range and
   what breaks if you push outside it.
3. Change the value.
4. `cargo build --release` — that's it.

All consumers reference `crate::constants::*` which re-exports
everything from this module. No call-site changes are needed when
tuning.

### Common tuning targets

**Reduce idle CPU further.** Lower `IDLE_FPS_FACTOR` from 0.5 to
0.25 (15 FPS at 60 base). Trade-off: phosphor decay becomes visibly
stepped at <15 FPS, which is why 0.5 was chosen as the floor.

**Make the self-healer more aggressive.** Lower
`SELF_HEAL_DOWNGRADE_SECS` from 30 to 15. Trade-off: transient
spikes (compile jobs, window drags) that would have ridden out in
30s will now trigger a scene downgrade.

**Make the self-healer less flappy.** Widen the hysteresis gap —
lower `SELF_HEAL_PRESSURE_LOW` from 0.3 to 0.2. Trade-off: the
restore window must accumulate more low-pressure frames before
restoring, so the user sees the fallback scene for longer after a
sustained overload clears.

**Raise the thermal throttle ceiling.** There is no constant for
this — `set_thermal_pressure` accepts any 0.0–1.0 value, and the
sampler (when wired) decides how to map sensor readings to that
range. The clamping is defensive: a misbehaving sampler cannot push
`effective_pressure` above 1.0.

### What NOT to tune

- `PERF_PRESSURE_INCREMENT` and `PERF_PRESSURE_DECAY` — these are
  calibrated against the frame-period overshoot math. Changing
  them without re-running the 72-hour endurance telemetry will
  silently change the self-healer's trigger behavior.
- `IDLE_THRESHOLD_SECS` — this is the reactive safety net below
  the predictor. Lowering it defeats the predictor's purpose
  (the reactive threshold fires first). Raising it past 60s means
  the user sees a non-responsive renderer for a full minute after
  they stop typing.
- `XTERMJS_HARD_CEILING_BYTES` — this is the last-resort defense
  against xterm.js V8 OOM. Lowering it triggers RIS resets too
  frequently (visible flicker); raising it removes the safety net.

---

## 10. Calibration history

- **Power audit consolidation**: extracted all power
  management constants from `constants.rs` into
  `central_control_dragon_power.rs` (flat file, 437 LOC). Established
  single source of truth. Added `PowerThresholds` struct as the
  foundation for a future `PowerManager` coordinator.
- **Phase 2 (directory module migration)**: converted the flat file to a
  directory module. Behavior code (PhasePredictor, ReclaimState,
  EnduranceHealth, PerformanceSelfHealer, SelfHealAction) moved
  from `src/interactive/adaptive.rs` (1105 LOC) into submodules.
  `interactive/adaptive.rs` became a 93-LOC thin re-export shim.
  Layout mirrors `central_control_rains.rs` extended to a directory
  module.
- **Phase 3 (PowerManager coordinator)**: added `power_manager.rs`
  submodule. `PowerManager` is the unified coordinator owning
  `perf_pressure` accumulation, `is_idle` detection, and effective
  FPS resolution. Exposes `effective_pressure()` /
  `effective_fps()` / `is_idle()` as the single read APIs for
  downstream consumers. Thermal guard (feature #13) implemented as
  INPUT to `effective_pressure()`. The event loop was migrated:
  7 inline variables removed, 12 call sites updated,
  `register_activity` signature changed. Test count: 1367 -> 1392
  (+25 PowerManager tests).

### Phase 3 commits

- `8c48070` feat(power): PowerManager coordinator struct + unified APIs (Phase 3 step 1)
- `46e3939` refactor(power): wire PowerManager into event_loop (Phase 3 step 2)
- `efc842e` fix(power): silence clippy dead_code on PowerThresholds + set_thermal_pressure
- `5ede4a4` feat(power): thermal sensor sampling on Linux feeds PowerManager (feature #13)
- `87cbb2c` refactor(power): migrate PerformanceSelfHealer to read from PowerThresholds struct
- `9990238` docs(config): clarify FPS default is dynamic (60 standard / 144 high-perf)
- `5816f0b` test(power): add audit_tests.rs — end-to-end verification of power stack contract

---

## 11. Future work

**Thermal sensor sampling (feature #13) — COMPLETED.** The
Linux sampler is implemented in `thermal_sampler.rs` and wired into
`event_loop.rs`. It reads `/sys/class/thermal/thermal_zone*/temp`,
picks the hottest zone, normalizes to 0.0–1.0 via a linear ramp
(50°C -> 0.0, 90°C -> 1.0), and feeds `PowerManager::set_thermal_pressure()`
every 600 frames (~10s at 60 FPS). Every downstream consumer of
`effective_pressure()` automatically responds.

Platform support: Linux only. macOS SMC and Windows WMI samplers are
future work — on those platforms the sampler returns `None` and the
thermal input stays at 0.0 (no behavior change).

**`PerformanceSelfHealer` threshold migration — COMPLETED.**
The self-healer now reads all 6 P1 + P2 thresholds from
`self.thresholds: PowerThresholds` instead of from the standalone
constants. The struct is the sole consumer-facing API; the standalone
constants remain as the canonical values that `defaults()` copies into
the struct. The `audit_self_healer_observe_reads_from_struct_not_constants`
test proves the migration is real (an override changes behavior; the
control case with defaults still fires).

**OKLab dithering for `colors-custom` — COMPLETED in prior sessions.**
The `colors-custom` path already routes through the OKLab polar
gradient engine (`colors_custom.rs:78` calls `colors_from_stops` which
uses `gradient_from_stops_oklab`) and the base shader applies Bayer
4×4 ordered dithering on both the `shading_distance` path
(`base.rs:486-506`) and the short-droplet luminance-remap path
(`base.rs:586-599`). Commits `2714153`, `d39c010`, `f5d037d`. The
`to_palette_routes_through_oklab_polar_engine` test verifies the
routing. No further work needed.

**Stale FPS references in config template — COMPLETED.** The
config template (`configfile.rs:603-609`), `docs/BENCHMARKING.md`,
and `docs/RELEASE_CANDIDATE.md` now document the dynamic default
(60 FPS standard, 144 FPS high-refresh) instead of the stale
"default 60.0". Users on high-refresh terminals (Alacritty, kitty,
WezTerm) no longer see misleading documentation.

---

## 12. Verification

The module is verified within the full cosmostrix test suite (1649 tests
as of 2026-08-23).
The dragon power module specifically contributes:

- `phase_predictor.rs`: 5 tests
- `reclaim_state.rs`: 5 tests
- `endurance_health.rs`: 5 tests
- `self_healer.rs`: 18 tests (15 original + 3 migration tests)
- `power_manager.rs`: 25 tests
- `thermal_sampler.rs`: 9 tests
- `audit_tests.rs`: 13 integration tests (end-to-end contract)
- `mod.rs`: 10 tests (PowerThresholds + constant sanity + thermal constants)

Total: 90 tests directly exercising the dragon power module.

`cargo fmt --check` and `cargo clippy --all-targets` are both
clean. The module is at 2820 LOC across 7 files, well under the
1500-LOC-per-file cap (largest file: `power_manager.rs` at 736
LOC).

The `audit_tests.rs` file is the "not a gimmick" verification
— 13 integration tests that exercise the public API contract end-to-end:
thermal input flows to every `effective_pressure()` read, self-healer
reads from `PowerThresholds` (not constants), frame lifecycle is stable
across 100-frame synthetic runs, and the full thermal -> self-healer
cascade triggers a downgrade at t=30s. These tests guard against a
future refactor silently breaking the cross-module wiring.

Owner-verified runtime output (commit `46e3939`): fps=144.0 from
`/proc` ancestor walk, fps_precedence=dynamic_default visible in
verbose, ambient scheduler fires (13:45 and 21:29 phases),
EnduranceHealth score=58.2/investigate (sampling works without
`--perf-stats`), backpressure low (P2 fix confirmed), Tier 2
healthy, purple verbose color confirmed on ambient + final runtime
state lines.
<!-- COSMOSTRIX-DISCLAIMER -->
<!--
  Documentation Disclaimer — read before relying on any data point.

  This document may contain stale data, hardcoded counts, or outdated
  file paths and symbol names. Maintainers update source code but may
  forget to sync every doc — the project ships 80+ .md files and
  perfect sync is a known maintenance burden with diminishing returns.

  Source code (`src/**/*.rs`) is the single source of truth.
  Always cross-check against the actual `.rs` files before relying on
  any specific number (test count, LOC, FPS, ms timeout), file path,
  function name, or config key.

  If you find a discrepancy, please open a PR — the doc is wrong, not
  the source.
-->
