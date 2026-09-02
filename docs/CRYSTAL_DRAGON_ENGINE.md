<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Crystal Dragon Engine — Complete Documentation

> **Source code = truth.** This document is the human-readable companion
> to `src/engine/crystal_dragon_engine/`. When code and doc disagree, the code
> wins. Every constant, function signature, and behavior described here
> is mirrored 1:1 in the Rust source.

## 1. What the Crystal Dragon Engine Is

The Crystal Dragon is **ambient intelligence for palette drift and
time-of-day scene scheduling**. It is one of cosmostrix's three
cooperating dragon engines:

| Engine | Owns | Code Root |
|--------|------|----------|
| **Cosmic Dragon** (rendering) | "Did this cell change?" — diff-based render loop, dirty tracking, ANSI stream output | `src/engine/cosmic_dragon_engine/cloud/`, `src/engine/cosmic_dragon_engine/frame.rs`, `src/engine/cosmic_dragon_engine/terminal/`, `src/engine/cosmic_dragon_engine/runtime.rs` |
| **Chroma Dragon** (coloring) | "What color should this cell be now?" — palette construction, OKLab gradient, post-FX, anomaly halos | `src/engine/chroma_dragon_engine/` |
| **Crystal Dragon** (ambient) | "Should the palette drift, and to what?" — sensor sampling, temperature classification, weighted theme selection, time-of-day scene schedule | `src/engine/crystal_dragon_engine/` (this module) |

The Crystal Dragon never paints pixels itself. It watches the system
state (CPU% or wall-clock), decides when a palette drift is appropriate,
selects the new theme via probabilistic weighted selection, and delegates
the actual visual transition to the Chroma Dragon's 300 ms OKLab wave
shader.

## 2. Subsystem Map (Source of Truth)

| File | Role | LOC |
|------|------|-----|
| `crystal_dragon_control/mod.rs` | Config struct + defaults (polling interval, sensor mode, calc method, drift chance, EMA alpha) | 137 |
| `sensor/mod.rs` | CPU sampling (procfs) + CLOCK fallback (UTC time). Produces a 1–99 **point**. | 300 |
| `palette_groups/mod.rs` | 44 builtin themes partitioned into Cold(14) / Medium(14) / Hot(14) + Reserved(2). | 129 |
| `point_system/mod.rs` | calc-v2 (default): weighted CDF + DriftHistory recency ring (8 entries); calc-v1 (legacy): no-memory weighted selection. | 268 |
| `ambient/mod.rs` | Time-of-day schedule types, config parsing, validation, startup apply. | 524 |
| `ambient_scheduler/mod.rs` | Background **dynamic idle/wake** thread that fires phase boundaries. | 506 |
| `ambient_diag.rs` | Diagnostics counters (exit summary, `ambient_diag_summary()`). | 88 |
| `mod.rs` | Top-level module doc + re-exports. | 75 |

Plus per-subsystem `tests.rs` files (Pattern C — dedicated tests/
subdir convention).

## 3. Configuration Knobs (Source: `crystal_dragon_control/mod.rs`)

All values are `pub(crate) const`. No runtime config file exposure yet
(silent-elegant mode). Future CLI flags or config keys would override
these via the `CrystalDragonControl` struct.

| Constant | Value | Meaning |
|----------|-------|---------|
| `CRYSTAL_DRAGON_POLLING_SECS` | `60.0` | Sensor sampling interval. At 60 s, drift events fire at most once per minute — slow enough to feel organic. |
| `CRYSTAL_DRAGON_MIN_DWELL_SECS` | `60.0` | Minimum time in current theme before transition is allowed. Prevents flicker when CPU% hovers near a group boundary. |
| `CRYSTAL_DRAGON_DRIFT_CHANCE` | `0.12` (12 %) | Default probability that a poll tick actually triggers a drift event. At 60 s × 12 %, drift fires roughly once every 5 minutes. Since S-master-1 the `CrystalDragonControl.drift_chance` FIELD is the single runtime source of truth (`crystal_dragon_tick` reads the field); this const only seeds the default. |
| `CRYSTAL_DRAGON_CPU_EMA_ALPHA` | `0.25` | Default EMA smoothing for CPU%. 0.25 = 75 % weight on history, 25 % on new sample. The sensor copies `CrystalDragonControl.cpu_ema_alpha` at construction (S-master-1 wiring); this const only seeds the default. |

### `CrystalDragonControl` struct

```rust
pub(crate) struct CrystalDragonControl {
    pub polling_secs: f32,        // 60.0
    pub min_dwell_secs: f32,      // 60.0
    pub drift_chance: f32,        // 0.12
    pub cpu_ema_alpha: f32,       // 0.25
    pub sensor_mode: CrystalDragonSensorMode,  // Cpu (with Clock fallback)
    pub calc_method: CrystalDragonCalcMethod, // CalcV2 (calc-v2, default)
}
```

### `CrystalDragonSensorMode` enum

```rust
pub(crate) enum CrystalDragonSensorMode {
    Cpu,   // Primary: read process CPU% via sysinfo/procfs
    Clock, // Fallback: derive point from UTC time-of-day
}
```

### `CrystalDragonCalcMethod` enum

```rust
pub(crate) enum CrystalDragonCalcMethod {
    Calc,    // calc-v1: legacy no-memory weighted selection (constructed only in tests)
    CalcV2,  // Pattern state machine with DriftHistory recency memory (implemented, the default since Dragon Engine v2)
}
```

## 4. The Point System (Source: `sensor/mod.rs`)

The sensor produces a **point** in the range `1..=99`. The point
classifies into one of three temperature groups:

```
 1 ──── 33 │ 34 ──── 66 │ 67 ──── 99
   Cold    │    Medium    │    Hot
```

### 4.1 CPU mode (primary)

Samples process CPU% via `crate::cpustat::current_cpu_ns()` (procfs
on Linux, sysinfo-equivalent on others), smooths with an EMA, then maps
via a **sqrt curve**:

```
point = clamp(1, 99, round(sqrt(cpu_ema_percent) * 9.9))
```

The sqrt curve spreads cosmostrix's typical low CPU usage (0.5–8%)
across the full Cold group range (points 7–28), and makes Medium
(greens/purples) reachable at ~12% CPU, Hot (yellows/reds/fire) at
~50% CPU. The old linear mapping (`cpu * 0.99`) bottlenecked
everything into points 1–8 (always Cold → blues/cyans/whites only).

- `0.5 % CPU -> point 7  -> Cold group`
- `2 % CPU  -> point 14 -> Cold group`
- `12 % CPU -> point 34 -> Medium group`
- `50 % CPU -> point 70 -> Hot group`
- `100 % CPU -> point 99 -> Hot group`

### 4.2 CLOCK fallback

When CPU sampling is unsupported (Windows, some sandboxes), the sensor
falls back to UTC time-of-day:

```
hour_frac = utc_hour + utc_minute / 60.0
point = clamp(1, 99, round(1.0 + hour_frac * 4.083))
```

- `00:00 UTC -> point 1  -> Cold group (cool night)`
- `12:00 UTC -> point ~50 -> Medium group (balanced midday)`
- `23:59 UTC -> point 99 -> Hot group (warm evening)`

The mapping is intentionally **monotonic** — time of day directly
controls color temperature, producing a natural day/night cycle without
any CPU dependency.

### 4.3 Cold-start point

`CrystalDragonSensor::new()` initializes `current_point = 17` (lower-
middle of Cold group). This avoids an immediate theme change on the
first poll tick — the engine starts calm and warms up naturally as
the system warms.

## 5. Palette Groups (Source: `palette_groups/mod.rs`)

All 44 builtin `ColorScheme` variants are partitioned into three
temperature groups of 14 themes each, plus 2 reserved themes excluded
from drift.

### 5.1 Cold group (14 themes) — points 1–33

Cool blues, cyans, neutrals, whites. Cool, calm, serene aesthetic.

| Subgroup | Themes |
|----------|--------|
| Cool blues & cyans (7) | `Blue`, `Ocean`, `Neptune`, `Uranus`, `Cyan`, `NeonBlue`, `NeonCyan` |
| Neutrals, whites, grays (7) | `Snow`, `Moon`, `Stars`, `Gray`, `Mercury`, `Carbon`, `NeonWhite` |

v80.0.0 earth-element real-color note: `Snow`'s head was retuned to
(192,222,241) — a proper Rayleigh ice-blue cast at the 655 family
luminance sum (was near-neutral gray, which dropped the body hue in
the final stop). Cold-group classification unchanged.

### 5.2 Medium group (14 themes) — points 34–66

Greens, purples, cosmic. Balanced, natural aesthetic.

| Subgroup | Themes |
|----------|--------|
| Greens & forest (6) | `Green`, `Green2`, `Green3`, `NeonGreen`, `Forest`, `Aurora` |
| Purples & cosmic (7) | `Purple`, `Nebula`, `Cosmos`, `Vaporwave`, `Neon`, `FancyDiamond`, `NeonPurple` |
| Transitional neutral (1) | `Pluto` (v80.0.0 real-color: muted dusty-tan, still a warm-leaning neutral) |

v80.0.0 earth-element real-color note: `Forest`'s upper body now uses
real chlorophyll chartreuse steps (the neon-lime G=255 plateau is
gone — foliage never pins the green channel while red climbs), and
`Aurora`'s body now tracks the true oxygen 557.7nm emission green
(was teal-shifted) with a pale auroral-green head. Green-family
classification unchanged.

### 5.3 Hot group (14 themes) — points 67–99

Warm yellows, oranges, fiery reds. Warm, energetic aesthetic.

| Subgroup | Themes |
|----------|--------|
| Warm yellows & oranges (9) | `Gold`, `Yellow`, `Orange`, `Sun`, `Venus`, `Jupiter`, `Saturn`, `NeonOrange`, `NeonYellow` |
| Fiery reds (4) | `Red`, `Fire`, `Mars`, `NeonRed` (premium exclusive) |
| Bonus | `EnergyZen` |

### 5.4 Reserved themes (2) — excluded from drift

`Rainbow`, `Spectrum20`. These span the full color spectrum and don't
fit a single temperature. They remain available via explicit
`--color <name>` selection, but the Crystal Dragon will never drift
into them.

## 6. calc-v1 Selection Algorithm (Source: `point_system/mod.rs`)

Legacy algorithm, retained for A/B comparison and constructed only in
tests. The PRODUCTION default is calc-v2 (`calc_v2_select()`), which
adds an 8-entry `DriftHistory` recency ring: recently selected themes
get a multiplicative recency penalty so the engine cannot oscillate
A->B->A and the drift sequence feels more varied. calc-v2 otherwise
uses the same weighted-CDF machinery documented below.

When a drift event fires (12 % chance per poll tick), the engine runs
`calc_v1_select()` to pick a new theme from the current temperature
group:

```
1. Compute the current group from the current point.
2. For each theme in the group, compute a weight based on distance
   from the current point:

       natural_point = lo + (theme_index / (group_size - 1)) * range
       distance = |current_point - natural_point|
       weight = 1.0 / (1.0 + distance * 0.1)

   - distance 0  -> weight 1.00 (max)
   - distance 33 -> weight 0.23 (still selectable)

3. Normalize weights, build a cumulative distribution function (CDF).
4. Draw a uniform random value u ∈ [0, 1).
5. Binary-search the CDF: pick the first theme whose cumulative
   weight ≥ u.
6. If the selected theme == current theme, retry once. If still the
   same, accept no-op (return None) — avoids infinite loops on
   single-theme groups.
```

This produces **organic, unpredictable** transitions: any theme in the
group can be selected, but themes whose natural point is closer to the
current system intensity are favored. A high-CPU Hot-group state will
tend to drift toward fiery reds and oranges, but will occasionally
select a warm yellow — keeping the visual experience non-mechanical.

### 6.1 Uniform fallback

If all weights sum to zero (degenerate case — impossible with the
current weight formula but defensive), `uniform_select()` picks a
random theme, skipping the current one.

## 7. Transition Bridge

The Crystal Dragon does **not** implement its own visual transition.
It delegates entirely to the Chroma Dragon's proven 300 ms OKLab wave
shader via `Cloud::set_color_scheme(new_scheme)`.

`crystal_dragon_tick()` returns `Option<ColorScheme>` directly —
`Some(scheme)` when a drift event fires and a new theme is selected,
`None` otherwise. The caller (in `cloud/rain.rs`) handles the actual
apply, which:

1. Advances the palette circular buffer slot.
2. Stores the new palette.
3. Sets `transition_start = Some(Instant::now())`.
4. Triggers the 300 ms top-to-bottom OKLab wave transition in the
   Chroma Dragon shader pipeline.

This contract ensures all 6 color-change paths (Crystal Dragon drift,
ambient scheduler, `c`/`C` keys, live-config reload, ambient snapback,
and startup) use the same smooth transition.

## 8. Ambient Scheduler (Source: `ambient/mod.rs` + `ambient_scheduler/mod.rs`)

The ambient scheduler is the second subsystem of the Crystal Dragon.
It drives **time-of-day scene switching** via `ambient.<HH-MM> = <scene>`
keys in `config.toml`.

### 8.1 Config format

```toml
# Top-level — NEVER inside any [section] block
ambient.15-00 = signal            # 3 PM -> signal scene
ambient.22-30 = monolith          # 10:30 PM -> monolith scene
ambient.07-00 = afternoon         # 7 AM -> custom scene (see below)
```

The value is a **single scene name** — either a builtin (`cinematic`,
`signal`, `monolith`, etc.) or a custom scene defined via
`[scene-custom.<name>]`. All visual parameters (color, charset, speed,
density, fps, glitch-level, rain_style) live inside the scene itself,
eliminating the precedence confusion of the legacy multi-field format.

### 8.2 Multi-field format migration (REJECTED)

The previous archived `adaptive-custom` subsystem accepted entries
like `ambient.15-00 = neon-purple, signal, speed=50, density=0.65`.
The Crystal Dragon rejects this with a migration error. To preserve
the entry, define a custom scene:

```toml
[scene-custom.afternoon]
color = "neon-purple"      # built-in name OR colors-custom = "<palette>"
charset = "retro"          # built-in preset OR charset-custom = "<set>"
fps = 60
speed = "50"
density = "0.65"
glitch-level = "subtle"    # all six dimensions required (v80.0.0-beta.2)

# Top-level — outside any [section] block:
ambient.15-00 = afternoon
```

This separates concerns cleanly: **the schedule says WHEN, the scene
says WHAT**. There is no override-precedence bug surface because the
scene IS the source of truth — no field can be "lost" between the scene
switch and the override layer.

### 8.3 Dynamic idle/wake scheduler (NOT a fixed poller)

The scheduler thread (`ambient_scheduler/mod.rs`) does NOT poll on a
fixed 30-second interval like the archived `adaptive-custom` engine
did. Instead:

1. Computes `time_to_next_phase` (seconds until the next entry's
   `HH:MM` boundary, with midnight wrap-around).
2. Sleeps for that duration (capped at 1 hour for reload
   responsiveness).
3. On wake, fires the new phase via an mpsc channel.
4. Returns to step 1.

Between phase boundaries, the thread is parked in
`Condvar::wait_timeout` — **zero CPU usage, zero wakeups**. The OS
only schedules the thread when:

- The timeout expires (a phase boundary was reached), OR
- The condvar is notified (live-reload path pushed a new schedule).

### 8.4 Instant switch (no smoothstep blend)

The user explicitly asked for **snappy phase boundaries** — not the
imperceptible 5-minute cross-fade the old engine used. When the thread
fires a phase, the entry is sent to the event loop, which calls
`Cloud::apply_ambient_entry` to apply the scene immediately. The only
visual smoothing comes from the existing `transition_chars` (glyph
warm-start) and `transition_rain_style` (pool reset) — those exist for
correctness, not for cinematic blending.

### 8.5 Live reload

When the user saves `config.toml`, the live-reload watcher re-parses
the file. If any `ambient.*` keys are present, `reload_schedule()` is
called with the new `AmbientSchedule`. This function:

1. Swaps the schedule atomically (Mutex).
2. Notifies the condvar — the thread wakes immediately.
3. Recomputes `time_to_next_phase` and adjusts its sleep.

If the new schedule's currently-active phase differs from the
previously-applied one, the thread fires it on the next loop iteration
(no need to wait for a boundary).

### 8.6 Edge cases (handled)

| Case | Behavior |
|------|----------|
| Empty schedule | Thread detects `entries.is_empty()`, sleeps 60 s, then loops. On reload with entries, condvar wakes it immediately. |
| Single entry | Thread sleeps until boundary, fires, then sleeps 24 h (capped to 1 h, so it polls hourly — but the phase is already applied, so it no-ops). |
| DST spring-forward | `current_minute_of_day()` returns wall-clock local time. Entries in the skipped hour (02:00–02:59) are never fired. Acceptable — user won't notice. |
| DST fall-back | Entries in the repeated hour (01:00–01:59) fire twice. Acceptable — `apply_ambient_entry` is idempotent. |
| Midnight wrap | Handled in `AmbientSchedule::seconds_to_next_phase`. |

## 9. Ambient Diagnostics (Source: `ambient_diag.rs`)

The engine tracks a set of atomic counters for diagnostics. The
summary is surfaced on exit via `ambient_diag_summary()`:

```
ambient_diag: startup=N rx=N reapply=N snapback=N cfg_rebuilds=N
              sked_reloads=N sked_empties=N consistency_fixes=N
              snapback_killed=N snapback_guard_sked_len=N
              snapback_guard_last_applied=N last_scene_change=<source>
```

| Counter | Increments when… |
|---------|-----------------|
| `startup` | Ambient scheduler spawned at process start. |
| `rx` | Event loop received a phase event from the scheduler thread. |
| `reapply` | A phase was re-applied (e.g., after live-reload of an already-active phase). |
| `snapback` | Auto-snapback restored the ambient phase after user override expired. |
| `cfg_rebuilds` | Config was rebuilt (post-rebuild consistency fix path). |
| `sked_reloads` | Schedule was reloaded from live-config watcher. |
| `sked_empties` | Schedule was reloaded as empty (all `ambient.*` keys removed). |
| `consistency_fixes` | A consistency fix was applied (snapback guard detected drift). |
| `snapback_killed` | Permanent snapback kill flag was set (user explicitly disabled). |
| `snapback_guard_sked_len` | Last captured schedule length at snapback guard call site. |
| `snapback_guard_last_applied` | Last captured "is last_applied Some?" state (1 = Some, 0 = None). |
| `last_scene_change` | Source string of the last scene change event (e.g., `"startup"`, `"ambient"`, `"user-c"`, `"crystal-dragon"`). |

## 10. Owner Decisions (Locked-in)

These choices were made by the owner during the Crystal Dragon design
phase and are now invariants of the engine:

| Decision | Choice | Rationale |
|----------|--------|-----------|
| HUD indicator | **Silent-Elegant (Option A)** — no HUD indicator, no verbose drift-event logging | The engine should be felt, not seen. |
| Calc method | **calc-v2** (pattern state machine with DriftHistory recency memory, default since Dragon Engine v2) | calc-v1 (legacy no-memory weighted selection) retained for A/B and constructed only in tests. |
| Polling interval | **60 s** | Slow enough to feel organic, fast enough to react to real load within a minute. |
| Sensor mode | **CPU primary, CLOCK fallback** | CPU is the meaningful signal; CLOCK is the graceful degradation when CPU sampling is unsupported. |
| Phase switching | **Instant** (no smoothstep blend) | Owner explicitly asked for snappy boundaries, not 5-minute cross-fades. |
| Schedule format | **Single scene name** (no multi-field) | Eliminates override-precedence bug surface. Scene IS the source of truth. |

## 11. Interaction With Other Engines

### 11.1 Crystal -> Chroma (transition delegation)

When Crystal selects a new theme, `crystal_dragon_tick()` returns
`Some(new_scheme)`. The caller invokes `Cloud::set_color_scheme(new_scheme)`,
which triggers the Chroma Dragon's 300 ms OKLab wave transition. Crystal
never paints pixels.

### 11.2 Crystal <- User override (`c`/`C`/`x` keys)

When the user presses `c` (cycle color), `C` (reverse cycle), or `x`
(cycle scene), the event loop:

1. Sets `cloud.user_override_since_ambient = true`.
2. Clears `cloud.ambient_palette_locked`.
3. Applies the user's selection immediately.

The Crystal Dragon's next poll tick sees the user override flag and
**does not drift** — the user is in control. The override persists until
the user goes idle for the `IDLE_AUTO_SNAPBACK_THRESHOLD_SECS` duration,
at which point the ambient scheduler (if active) snaps back to the
scheduled phase.

**Z-master-1X**: when the ambient schedule is empty, `user_override_since_ambient`
stays `true` forever (no ambient fire to clear it), but the drift gate in
`cloud/post_rain.rs` skips the override check when `ambient_schedule_active == false`.
So manual user overrides do NOT block crystal dragon drift when ambient is off —
the engine continues to drift on its 60s poll cadence regardless of the
override flag. See `docs/AMBIENT_SCHEDULER.md` "Self-reset when ambient is OFF".

### 11.3 Crystal <-> Ambient scheduler (snapback coordination)

If the ambient scheduler has an active phase when the user goes idle:

1. The event loop's idle detector fires `should_auto_snapback()`.
2. The scheduler re-applies the current phase via `apply_ambient_entry`.
3. `ambient_diag_snapback()` increments.
4. The Crystal Dragon's drift continues normally on subsequent ticks.

If the user has explicitly disabled ambient (empty schedule or
`snapback_killed` flag), no snapback occurs — the user's last manual
selection persists, and the Crystal Dragon's drift cycle self-resets
every `CRYSTAL_DRAGON_POLLING_SECS` (60s) so drift continues to fire on
its poll cadence (Z-master-1X round 2, commit `40bad33`).

## 12. Test Coverage

Each subsystem has a dedicated `tests.rs` file (Pattern C — co-located
test subdir convention). Tests cover:

| Subsystem | Test File | Coverage |
|-----------|-----------|----------|
| `crystal_dragon_control` | `crystal_dragon_control/tests.rs` | Default values match owner-chosen constants |
| `sensor` | `sensor/tests.rs` | Point-to-group mapping, group-point-range bounds, CLOCK fallback math |
| `palette_groups` | `palette_groups/tests.rs` | All 44 themes classified, no orphan themes, group counts (14/14/14/2) |
| `point_system` | `point_system/tests.rs` | calc-v2 DriftHistory recency + distribution properties, calc-v1 legacy parity, no-op skip behavior, uniform fallback |
| `ambient` | `ambient/tests.rs` | Config parsing, validation, scene-custom interaction, startup apply |
| `ambient_scheduler` | `ambient_scheduler/tests.rs` | Dynamic idle/wake timing, reload behavior, edge cases |

Run all Crystal Dragon tests:

```bash
cargo test crystal_dragon
```

## 13. File Layout (Quick Reference)

```
src/engine/crystal_dragon_engine/
├── mod.rs                          # Top-level doc + re-exports (75 LOC)
├── ambient_diag.rs                 # Diagnostics counters (88 LOC)
├── ambient/
│   ├── mod.rs                      # Schedule types + config parsing (524 LOC)
│   └── tests.rs                    # ambient tests (346 LOC)
├── ambient_scheduler/
│   ├── mod.rs                      # Dynamic idle/wake thread (506 LOC)
│   └── tests.rs                    # Scheduler tests (446 LOC)
├── crystal_dragon_control/
│   ├── mod.rs                      # Config struct + constants (137 LOC)
│   └── tests.rs                    # Control tests (32 LOC)
├── palette_groups/
│   ├── mod.rs                      # 44 themes -> 3 groups (129 LOC)
│   └── tests.rs                    # Group tests (71 LOC)
├── point_system/
│   ├── mod.rs                      # calc-v2 (default) + calc-v1 selection, DriftHistory (268 LOC)
│   └── tests.rs                    # Point system tests (242 LOC)
└── sensor/
    ├── mod.rs                      # CPU + CLOCK sensor (300 LOC)
    └── tests.rs                    # Sensor tests (164 LOC)

Total: 2,019 LOC (production) + 1,801 LOC (tests) = 3,820 LOC
```

## 14. See Also

- [`docs/AMBIENT_SCHEDULER.md`](AMBIENT_SCHEDULER.md) — Focused doc on the ambient scheduler subsystem.
- [`docs/THREE_DRAGON_ENGINES.md`](THREE_DRAGON_ENGINES.md) — High-level overview of all three dragon engines and how they cooperate.
- [`docs/CENTRAL_CONTROL_DRAGON_POWER.md`](CENTRAL_CONTROL_DRAGON_POWER.md) — Power management / thermal / self-healing subsystem (separate from Crystal Dragon).
- [`src/engine/chroma_dragon_engine/mod.rs`](../src/engine/chroma_dragon_engine/mod.rs) — The coloring engine that Crystal delegates transitions to.
- [`src/cosmic_dragon_incubator/mod.rs`](../src/cosmic_dragon_incubator/mod.rs) — The rendering engine (incubator namespace; actual rendering code lives in `src/engine/cosmic_dragon_engine/cloud/`, `src/engine/cosmic_dragon_engine/frame.rs`, `src/engine/cosmic_dragon_engine/terminal/`, `src/engine/cosmic_dragon_engine/runtime.rs`).
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
