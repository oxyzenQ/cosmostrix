<!-- SPDX-License-Identifier: GPL-3.0-only -->

# System Feeling — Signal-Driven Palette Drift

System Feeling is cosmostrix's autonomous color drift engine. When
enabled via `--auto-color-drift`, the renderer reads two honest signals
— **process CPU%** and **local wall-clock hour** — classifies the
machine's "mood" into one of 5 emotional states, and drifts the palette
toward the color family associated with that state.

This is NOT random drift. The rain changes color because the system is
busier, or because night fell — not because a dice rolled.

---

## Design Principles

1. **Honest signals only.** Every signal read is real telemetry from
   the OS. No synthetic state, no fake randomness dressed up as
   "feeling". If a signal is unavailable, the engine says so.
2. **Open boundaries.** Every signal we read is documented here. Every
   signal we refuse to read is also documented, with the reason. The
   boundary is auditable.
3. **Owner-tunable, not config-tunable.** The taste constants (CPU
   thresholds, time windows, state→family mapping) live in exactly one
   file: `src/control_color_drift.rs`. There is no `config.toml` key
   for them. The owner edits the file and rebuilds. This keeps the
   taste surface small and reviewable.
4. **No new flags.** `--auto-color-drift` is the only flag. Its
   behavior changed from RNG-within-family (pre-v30) to
   signal-driven-family-selection (v30+). No `--system-feeling-drift`
   or similar was added.
5. **Degradation is visible.** On platforms without CPU sampling
   (Windows, some sandboxes), the engine falls back to time-only
   classification and prints the degradation in `--doctor`.

---

## Signals Read

| Signal | Source | Platform | Cost |
|--------|--------|----------|------|
| Process CPU% | `cpustat::current_cpu_ns()` | Linux (`/proc/self/stat`), macOS (Mach `task_info`) | <0.05% CPU per sample |
| Local wall-clock hour | `system_feeling::current_local_hour()` via `chrono::Local` (inlined from the deleted `atmosphere_adaptive::current_hour()` at commit 07b44b5, 2026-08-05) | All | ~0 |

**Sampling cadence:** every 3 seconds (the existing `COLOR_ECOSYSTEM_TICK_SECS`
interval). No separate timer.

**CPU smoothing:** EMA with α=0.3 (configurable in `control_color_drift.rs`).
This means ~70% weight on history, ~30% on new sample — smooths jitter
without lagging behind real load changes.

---

## Signals Refused (Open Boundary)

These signals are deliberately NOT read. The boundary is open — each
refusal is documented with the reason.

| Signal | Why refused |
|--------|-------------|
| System load average (`/proc/loadavg`, `getloadavg`) | Owner decision: CPU + time only, keep it simple. Load average is a system-wide metric that doesn't reflect cosmostrix's own load. |
| CPU temperature (`/sys/class/thermal/*`) | Owner decision: CPU + time only. Thermal is Linux-only and fragile across hardware. |
| Disk IO (`/proc/diskstats`) | Owner decision: CPU + time only. IO doesn't map cleanly to an emotional state. |
| Process RSS (`memstat::current_rss_kb`) | Owner decision: CPU + time only. Memory footprint doesn't change the rain's mood. |
| Battery level | Privacy. Not available on desktops. Would make the rain feel like it's monitoring the user. |
| GPS / location | Privacy. Never read. |
| Active window title | Privacy. Would look like spyware. |
| Network traffic | Privacy. Would look like spyware. |
| Camera / microphone | Hard no. Never read, never will be. |

If a future version wants to add a signal, it MUST:
1. Be documented in this table (read or refused).
2. Be opt-in via `--auto-color-drift` (never read when the flag is off).
3. Be visible in `--doctor` output.
4. Have a graceful degradation path when unavailable.

---

## The 5 Emotional States

The classifier maps `(cpu_percent, local_hour)` to one of 5 states.
The thresholds and time windows are in `control_color_drift.rs`.

| State | Trigger | Target Color Family | Mood |
|-------|---------|---------------------|------|
| **Calm** | Low CPU + daytime (non-night, non-morning) | BlueWater | System breathing freely — cool blues |
| **Pulse** | Low CPU + morning (06:00–12:00) | Green | Fresh Matrix green — morning energy |
| **Signal** | High CPU (≥50%) at any hour | RedFire | Hot reds/oranges — system under load |
| **Void** | Low CPU + night (22:00–06:00) | PurpleNebula | Deep cosmic purples — silent night |
| **Compression** | Mid CPU + pre-dawn (03:00–06:00) | GrayMoon | Neutral grays — pre-dawn pressure |

### Decision tree (order matters)

1. CPU busy (≥ `CPU_BUSY_THRESHOLD`) → **Signal** (any time of day)
2. Pre-dawn (03:00–06:00) + not idle → **Compression**
3. Night (22:00–06:00) + idle → **Void**
4. Morning (06:00–12:00) + idle → **Pulse**
5. Default → **Calm**

CPU-busy wins over everything. A hot system at 3am is still Signal, not
Compression — urgency overrides mood.

### Hysteresis

State transitions are subject to a minimum dwell time
(`MIN_STATE_DWELL_SECS = 60s`). This prevents flicker when CPU% hovers
near a threshold. The state can change at most once per minute.

---

## The 7 Color Families

The 44 built-in `ColorScheme` variants are partitioned into 7 aesthetic
families. The partition is defined in `cloud/ecosystem.rs::family_for`
and `cloud/ecosystem.rs::family_members`.

| Family | Members | Used by state |
|--------|---------|---------------|
| **Green** | Green, Green2, Green3, NeonGreen, Forest, Aurora | Pulse |
| **GoldWarm** | Gold, Yellow, Orange, Sun, Venus, Jupiter, Saturn, NeonOrange, NeonYellow | (none currently — available for future states) |
| **RedFire** | Red, Fire, Mars, NeonRed | Signal |
| **BlueWater** | Blue, Ocean, Neptune, Uranus, Cyan, NeonBlue, NeonCyan | Calm |
| **PurpleNebula** | Purple, Nebula, Cosmos, Vaporwave, Neon, FancyDiamond, NeonPurple | Void |
| **GrayMoon** | Gray, Mercury, Snow, Moon, Stars, Pluto, Carbon, NeonWhite | Compression |
| **Rainbow** | Rainbow, Spectrum20 | (none currently — available for future states) |

The partition is **complete** (every `ColorScheme` variant is in exactly
one family) and **disjoint** (no variant appears in two families). The
`family_partition_covers_every_variant` and `families_are_disjoint`
tests in `system_feeling_tests.rs` guard these invariants.

---

## Owner Tuning Guide

All taste constants live in **`src/control_color_drift.rs`**. This is
the single file to edit when retuning drift behavior.

### What to edit

| Want to change... | Edit this |
|-------------------|-----------|
| CPU sensitivity | `CPU_BUSY_THRESHOLD` (default 50.0), `CPU_IDLE_THRESHOLD` (default 15.0) |
| Time windows | `NIGHT_START`/`NIGHT_END`, `PRE_DAWN_START`/`PRE_DAWN_END`, `MORNING_START`/`MORNING_END` |
| Which colors go with which mood | `family_for_state()` match arms |
| How sticky a state is | `MIN_STATE_DWELL_SECS` (default 60.0) |
| CPU smoothing aggressiveness | `CPU_EMA_ALPHA` (default 0.3) |

### Example: make Signal drift to PurpleNebula instead of RedFire

```rust
// In src/control_color_drift.rs, family_for_state():
pub const fn family_for_state(state: FeelingState) -> ColorFamily {
    match state {
        FeelingState::Calm        => ColorFamily::BlueWater,
        FeelingState::Pulse       => ColorFamily::Green,
        FeelingState::Signal      => ColorFamily::PurpleNebula,  // changed
        FeelingState::Void        => ColorFamily::PurpleNebula,
        FeelingState::Compression => ColorFamily::GrayMoon,
    }
}
```

Rebuild. Done. No config file, no CLI flag, no runtime toggle.

### Example: make CPU-busy threshold tighter

```rust
// In src/control_color_drift.rs:
pub const CPU_BUSY_THRESHOLD: f32 = 35.0;  // was 50.0
```

Rebuild. The Signal state now triggers at 35% CPU instead of 50%.

---

## Diagnostics

### `--doctor`

The `SYSTEM FEELING` section prints:

```
SYSTEM FEELING
  state:               calm
  target_family:       blue-water
  local_hour:          14.52
  cpu_sampling:        supported
  cpu_ema_percent:     3.2
  auto_color_drift:    disabled (default)
```

When CPU sampling is unsupported:

```
SYSTEM FEELING
  state:               void
  target_family:       purple-nebula
  local_hour:          23.17
  cpu_sampling:        unsupported (time-only fallback)
  cpu_ema_percent:     n/a (no sample yet)
  auto_color_drift:    enabled
```

The `cpu_sampling` field makes degradation visible — never silent.

### Runtime behavior

When `--auto-color-drift` is enabled:

1. Every 3-second ecosystem tick, `SystemFeeling::tick()` samples CPU%
   and the local hour, updates the EMA, and may transition the state
   (subject to hysteresis).
2. The drift dice rolls (3% chance per tick). On success, the engine
   picks a random scheme from the current state's target family,
   skipping the current scheme to avoid no-op drift.
3. The palette transition crossfades over ~5 minutes via the existing
   `chroma/shaders/transition.rs` pipeline.

When `--auto-color-drift` is disabled (default):

1. `SystemFeeling::tick()` still runs (cheap, keeps diagnostics honest).
2. The drift dice still rolls, but the caller discards the result.
3. The palette never changes unless the user explicitly cycles it (`c`/`C`).

---

## Module Map

| Module | Responsibility |
|--------|----------------|
| `src/control_color_drift.rs` | **Owner-editable taste file.** FeelingState enum, thresholds, state→family mapping. |
| `src/system_feeling.rs` | Signal sampler + state classifier. SystemFeeling struct, classify() pure function. |
| `src/cloud/ecosystem.rs` | ColorFamily enum, family_for(), family_members(), ColorEcosystem::tick() integration. |
| `src/cpustat.rs` | Process CPU time sampling (Linux/macOS). Pre-existing, reused. |
| `src/system_feeling.rs::current_local_hour()` | Local wall-clock hour helper. Inlined from the deleted `atmosphere_adaptive::current_hour()` at commit `07b44b5` (2026-08-05, atmosphere engine elimination). |
| `src/doctor.rs` | `SYSTEM FEELING` diagnostic section. |

---

## Honorary Contract

1. **No silent fallback.** If CPU sampling is unavailable, `--doctor`
   prints "unsupported (time-only fallback)". The user always knows
   what the engine sees.
2. **No hidden signals.** Every signal read is documented in this file.
   Every signal refused is documented with the reason.
3. **Opt-in only.** `--auto-color-drift` is default-off. No signal is
   read when the flag is off (CPU sampling only happens inside
   `SystemFeeling::tick()`, which is called from `ColorEcosystem::tick()`,
   which runs regardless — but the CPU sample is cheap and the result
   is discarded when the flag is off).
4. **Benchmark mode stays clean.** `--auto-color-drift` is force-disabled
   in benchmark mode (`bench.rs`). The system feeling tracker never
   affects benchmark metrics.
5. **No new flags.** The behavior of `--auto-color-drift` changed; no
   new flag was added.
6. **Owner-tunable, not config-tunable.** Taste constants are in
   `control_color_drift.rs`, not `config.toml`. This keeps the taste
   surface small, reviewable, and version-controlled in the source tree.

---

## See Also

- [Atmosphere Engine (archived)](archive/specs/ATMOSPHERE_ENGINE.md) —
  historical v20 design spec for the time-driven 5-phase modulation
  engine. The atmosphere engine subsystem was fully eliminated at
  commit `07b44b5` (2026-08-05, Dragon Hunt v2 Phase 6 Tier E item 31).
  The 5-state taxonomy in this document (Calm / Pulse / Signal /
  Compression / Void) was originally shared with the atmosphere engine;
  it now lives only in `src/control_color_drift.rs` and is the canonical
  source. See `archive/audits/ATMOSPHERE_SUBSYSTEM_ARCHIVAL.md` for the
  full elimination record.
- [Rules](RULES.md) — project conventions and CLI flag policy
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
