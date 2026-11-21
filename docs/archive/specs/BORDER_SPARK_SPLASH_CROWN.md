<!--
  SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
  SPDX-License-Identifier: AGPL-3.0-only OR Commercial
-->

# Border-Touch Splash Crown Spark — F2 (archived — feature removed)

> **ARCHIVED 2026-11-20 (night-moonlight-1 — owner removal decision).**
> This document is the historical design and implementation record for
> the F2 Splash Crown spark: the six-particle upward splash emitted
> when a rain droplet's head touched the message-overlay border. The
> feature was removed at the owner's direction together with the
> Quantum Ripple burst it shared a pool with: judged dirty to
> maintain, known-problematic despite repeated fixes, a resource
> burden, and low value relative to the cost of further improvement.
>
> Nothing in this document describes live behavior. It is preserved
> as a design record only.

**Date:** 2026-11-20
**Author:** oxyzenQ (night-moonlight-1 removal round)
**Version:** v100.1.0-night.1 (removal base)
**Scope:** border-touch splash crown spark (F2, shared QuantumParticle pool)

---

## 0. Executive Summary

The splash crown spark was part of the RAIN_BORDER_TOUCH family: when
a droplet head crossed into the message overlay's top border
(`detect_border_touch`), the border-touch GLOW pulse (kept feature)
lit the touched border cell, and on non-corner touches the spark
spawned six particles in an upward semicircle fan, mimicking a water
drop crown-splashing against a glass ceiling. Particles lived 350ms,
moved at 12 cells/s, carried the droplet's head RGB (palette
last-stop, usually white), and rendered as middle-dot glyphs with a
single-cell trail.

| Aspect | Value |
|---|---|
| Introduced | F2 Splash Crown (see the archived research doc listed below) |
| Removed | night-moonlight-1 (2026-11-20) |
| Removal reason | Shared maintenance burden with the Quantum Ripple pool; repeated fixes never made it dependable; low value per cost |
| Kept sibling features | border-touch glow pulse (`BorderPulse`), corner-skip LTS invariant, `detect_border_touch` detection itself |

The original design research (including the §3.2 splash-crown
variant) is preserved at
`docs/archive/research/RAIN_BORDER_TOUCH_SPARK_RESEARCH.md` (moved
from the live research tree by this round).

---

## 1. How it worked

1. `Cloud::detect_border_touch` (cloud/border_touch.rs) ran in the
   droplet advance loop with pre/post-advance head lines. On the
   transition `prev_hp < top && hp >= top` inside the overlay span,
   it dedup-refreshed the `BorderPulse` glow and — for non-corner
   cells only — called `spawn_border_spark`.
2. `Cloud::spawn_border_spark` (cloud/spawn.rs) activated
   `BORDER_SPARK_PARTICLE_COUNT` (6) particles from the shared
   pre-allocated pool (`quantum_particles`, 96 slots, shared with the
   removed Quantum Ripple burst). Angles came from an upward
   semicircle fan `[-180°, 0°]` (negative Y = up in terminal
   coordinates), speed 12 cells/s with 0.9..1.1 variance, glyph `·`,
   `max_trail = 1`, lifetime 0.35s. Pool-full touches were silently
   dropped.
3. The shared render path (`apply_quantum_ripple` in
   cloud/rain_post.rs) integrated motion on the real-time clock
   (`PARTICLE_MAX_FRAME_DT_SECS` anti-teleport cap), applied velocity
   decay and edge bounce, and rendered with the same three-segment
   brightness curve and trail machinery the quantum burst used.

## 2. Tuning constants (final values, preserved)

| Constant | Value | Meaning |
|---|---|---|
| `BORDER_SPARK_PARTICLE_COUNT` | 6 | particles per touch |
| `BORDER_SPARK_LIFETIME_SECS` | 0.35 | spark lifespan |
| `BORDER_SPARK_SPEED` | 12.0 | cells/second (40% of the old burst speed) |
| `BORDER_SPARK_ANGLE_MIN_RAD` | -PI | fan start (left) |
| `BORDER_SPARK_ANGLE_MAX_RAD` | 0.0 | fan end (right) |
| `BORDER_SPARK_TRAIL_LEN` | 1 | single-cell streak |

## 3. Removed surface (night-moonlight-1)

- `spawn_border_spark` and its call site in `detect_border_touch`
  (the corner-skip guard stays — it protects the glow pulse);
- the `BORDER_SPARK_*` constants;
- the shared pool machinery both features rode on:
  `QuantumParticle`, `quantum_particles`, `quantum_active_count`,
  `last_quantum_update_time`, `apply_quantum_ripple`, the remaining
  `QUANTUM_RIPPLE_*` / `QUANTUM_BRAND_PURPLE_*` /
  `QUANTUM_BODY_TONE_DOWN` render constants, and the quantum test
  files (removed one commit earlier);
- the timestamp bookkeeping for the pool clock (restart origin reset,
  pause shift, scene-switch clear).

The border-touch glow pulse (`BorderPulse`, the halo above the
touched border cell) is an independent feature with its own state and
remains live.

## 4. Original spawn implementation (preserved)

```rust
pub(crate) fn spawn_border_spark(&mut self, col: u16, line: u16, head_rgb: (u8, u8, u8)) {
    // PERF-4: --no-effects gate. No-op when effects are disabled.
    if !self.effects_enabled {
        return;
    }
    let cx = col as f32 + 0.5;
    let cy = line as f32 + 0.5;
    let now = Instant::now();
    let mut spawned = 0usize;
    for p in &mut self.quantum_particles {
        if spawned >= BORDER_SPARK_PARTICLE_COUNT {
            break;
        }
        if p.active {
            continue;
        }
        // Upward semicircle fan: [-180°, 0°] (left through up to right).
        // In terminal coords, negative Y = upward. The border is a
        // ceiling, so sparks deflect up + sideways (crown splash).
        let angle = BORDER_SPARK_ANGLE_MIN_RAD
            + self.rand_chance.sample(&mut self.mt)
                * (BORDER_SPARK_ANGLE_MAX_RAD - BORDER_SPARK_ANGLE_MIN_RAD);
        let speed = BORDER_SPARK_SPEED * (0.9 + self.rand_chance.sample(&mut self.mt) * 0.2);
        p.active = true;
        p.x = cx;
        p.y = cy;
        p.vx = speed * angle.cos();
        p.vy = speed * angle.sin();
        p.birth = now;
        p.ch = '·';
        p.r = head_rgb.0;
        p.g = head_rgb.1;
        p.b = head_rgb.2;
        p.trail_count = 0;
        p.max_trail = BORDER_SPARK_TRAIL_LEN as u8;
        p.lifetime = BORDER_SPARK_LIFETIME_SECS;
        p.sim_age = 0.0;
        spawned += 1;
    }
    self.quantum_active_count = self.quantum_active_count.saturating_add(spawned);
}
```

---

<!--
  Documentation disclaimer (placement law: bottom of every living
  .md file).

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
