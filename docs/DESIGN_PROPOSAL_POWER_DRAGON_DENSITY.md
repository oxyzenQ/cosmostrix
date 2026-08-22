<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Design Proposal — Power Dragon Density Override Control

> **Owner decision needed**: Should the Central Control Dragon Power
> subsystem be allowed to modify user-visible settings (density, speed)
> under thermal/perf pressure, or should it be disabled/configurable?

## Current Behavior (as of v50)

The Central Control Dragon Power subsystem does NOT directly modify
user density/speed. It uses two indirect mechanisms:

### 1. Aggressive Throttle (AB-11)

When sustained CPU pressure exceeds the high threshold (0.7) for
`downgrade_secs` (30s), the self-healer sets `cloud.aggressive_throttle = true`.

**What changes:**
- Spawn rate curve steepens (0.9 vs 0.75 factor)
- Spawn floor drops (lower minimum spawn rate)
- Glitch effects disabled (`glitch_due && !aggressive_throttle`)

**What does NOT change:**
- User's density value (`cloud.droplet_density()`) — unchanged
- User's speed value — unchanged
- User's color/charset/scene — unchanged
- HUD `dsty:` line — shows the original density value

**User perception:** Rain may appear sparser (fewer droplets spawning)
even though the density setting hasn't changed. Glitches stop. This
can look "broken" to a user who set `--density 0.8` and expects
consistent density.

### 2. Idle FPS Throttling

When the user is idle for 30+ seconds, `PowerManager::effective_fps()`
returns `target_fps * idle_fps_factor` (0.5 by default = 30 FPS).

**What changes:**
- Frame pacing slows from 60 FPS to 30 FPS
- Rain motion appears slower (fewer simulation steps per second)

**What does NOT change:**
- User's density value — unchanged
- User's speed value — unchanged
- Spawn rate per frame — unchanged (just fewer frames)

**User perception:** Rain appears to slow down. Not "broken" per se,
but may be surprising.

## Owner's Concern

> "User set density -d xx, at runtime dragon control power reduce the
> density because reason x, so user see the density is changes also think
> like broken/unhappy."

The concern is that indirect modifications (throttle, idle FPS) make
the rain look different from what the user configured, creating a
"broken" perception.

## Proposed Options

### Option A: Keep Current (Default ON, Configurable Off)

Add a config key to disable power dragon visual modifications:

```toml
# config.toml
power-dragon = true    # true (default): enable adaptive throttle + idle FPS
                       # false: disable ALL power dragon visual modifications
                       #        (aggressive_throttle never fires, idle FPS stays at target)
```

**Pros:**
- Default behavior unchanged (power dragon protects the system)
- Users who want consistent visuals can disable it
- Simple, one config key

**Cons:**
- Users who disable it may experience higher CPU usage
- The `false` setting is a footgun (no protection under heavy load)

### Option B: Granular Control (Per-Feature)

Add separate config keys for each power dragon feature:

```toml
# config.toml
power-dragon-throttle = true    # aggressive_throttle under sustained pressure
power-dragon-idle-fps = true    # idle FPS throttling after 30s inactivity
power-dragon-thermal = true     # thermal pressure detection (sysfs)
```

**Pros:**
- Maximum flexibility
- User can keep thermal protection but disable visual throttle

**Cons:**
- More complex config surface
- More keys to document + maintain

### Option C: Silent + Transparent (Current + HUD Indicator)

Keep the current behavior but add a HUD indicator when power dragon
is actively modifying visuals:

- When `aggressive_throttle` is active: HUD shows `prs: high` (already
  exists via `effective_pressure`)
- When idle FPS is active: HUD shows `tgt: 30 idle` (already exists
  via `FrameMode::Idle`)

**Pros:**
- No new config keys
- User can see WHY the rain looks different (HUD explains it)
- Already partially implemented (HUD pressure + frame mode lines)

**Cons:**
- User must have HUD on (`i` key) to see the explanation
- Doesn't let the user opt out

### Option D: Masterclass — Option A + HUD Enhancement

Combine Option A (config toggle) with Option C (HUD indicator):

```toml
# config.toml
power-dragon = true    # default: adaptive protection ON
```

When power dragon is ON and actively throttling:
- HUD `prs:` line shows the current pressure level (already exists)
- HUD `tgt:` line shows `idle` or `paused` suffix (already exists)
- Verbose output includes `[self-heal] sustained high CPU pressure — throttling spawn rate` (already exists)

When power dragon is OFF:
- No throttle, no idle FPS reduction
- HUD `prs:` shows `0.00` (no pressure tracking)
- HUD `tgt:` shows the raw target FPS without idle suffix

**This is the recommended option.** It gives users control while
keeping the default safe, and the HUD provides transparency when
the dragon is active.

## Recommendation

**Option D (Masterclass)** is recommended:
1. Add `power-dragon = true` to config.toml (default ON)
2. When `false`: disable `aggressive_throttle` + `idle_fps_factor`
3. HUD already shows pressure + frame mode (no additional work needed)
4. Verbose already logs self-heal actions (no additional work needed)
5. `--testconf` validates the boolean value

Implementation: ~20 lines of code (read config key, gate the
`set_aggressive_throttle()` call + `effective_fps()` idle branch).

## Owner Decision Required

Pick one:
- [ ] Option A: Simple on/off toggle
- [ ] Option B: Granular per-feature control
- [ ] Option C: HUD transparency only (no config key)
- [x] Option D: Toggle + HUD transparency (recommended)
- [ ] Other: ____________________
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
