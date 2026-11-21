<!--
  SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
  SPDX-License-Identifier: AGPL-3.0-only OR Commercial
-->

# Quantum Ripple — mouse-click particle burst (archived — feature removed)

> **ARCHIVED 2026-11-20 (night-moonlight-1 — owner removal decision).**
> This document is the historical design and implementation record for
> the Quantum Ripple mouse-click particle burst. The feature was
> removed at the owner's direction: judged dirty to maintain, a
> recurring bug source (the VTE "slow, then stuck, then vanishes"
> family of particle-clock defects), a resource burden on every
> render path, and low value relative to the cost of further
> improvement. The implementation is preserved here for future
> reference and for anyone interested in how it worked.
>
> Nothing in this document describes live behavior. It is preserved
> as a design record only.

**Date:** 2026-11-20
**Author:** oxyzenQ (night-moonlight-1 removal round)
**Version:** v100.1.0-night.1 (removal base)
**Commit:** 3c818fb^ (removal lands one commit later; this doc lands with the removal commit)
**Scope:** mouse-click particle burst (v25 masterclass feature, v30/v50 retunes)

---

## 0. Executive Summary

Quantum Ripple was the mouse-click particle burst: every click in
interactive mode spawned up to 20 outward-radiating glyphs (`*`, `+`,
`·`) from the click point, tinted by a snapshot of the active
palette's body color, drifting on a damped physics integrator and
fading out over a 4.0s lifespan. It shared a pre-allocated 96-slot
pool (`QuantumParticle`) with the border-touch splash crown spark
(removed in the same round; see
`BORDER_SPARK_SPLASH_CROWN.md`).

| Aspect | Value |
|---|---|
| Introduced | v25 (masterclass effect era) |
| Major retunes | v30 (render-time tone-down), v50 (lifespan 0.8s -> 2.5s -> 4.0s, edge bounce, three-segment brightness, comet trail) |
| Removed | night-moonlight-1 (2026-11-20) |
| Removal reason | Maintenance burden, recurring particle-clock bugs, resource cost, low visual value per cost |
| Kept sibling features | wave-equation click shockwave (`wave_field.inject`), mouse hover glow, border-touch glow pulse |

---

## 1. How it worked

### 1.1 Spawn path

`Cloud::set_mouse_click` (cloud/mod.rs) handled every mouse click:

1. The `--no-effects` gate early-returned (PERF-4).
2. The wave-equation shockwave got its impulse
   (`wave_field.inject(col, line)` — kept feature).
3. `spawn_quantum_ripple(col, line)` activated up to
   `QUANTUM_RIPPLE_PARTICLE_COUNT` (20) particles in the first free
   slots of the pre-allocated 96-slot pool. Clicks beyond pool
   capacity were silently dropped.

Each spawned particle:

- position: click point center (`col + 0.5`, `line + 0.5`);
- velocity: uniform random angle in `[0, 2π)` (the shared
  `rand_chance` sampler), speed `QUANTUM_RIPPLE_SPEED` (30 cells/s)
  scaled by a 0.9..1.1 variance so the cohort stayed visually
  coherent;
- glyph: random pick from `['*', '+', '·']`;
- color: snapshot of the palette BODY color (middle index of
  `palette.colors`) at click time, so a palette switch mid-flight
  left the old cohort fading in its original hue (crossfade); the
  fallback was the brand purple `(168, 85, 247)` for degenerate
  palettes;
- `max_trail = QUANTUM_RIPPLE_TRAIL_LEN` (6) ring-buffer slots for
  the comet trail; `lifetime = QUANTUM_RIPPLE_LIFETIME_SECS` (4.0).

### 1.2 Update + render path

`Cloud::apply_quantum_ripple` (cloud/rain_post.rs) ran once per
frame from the `rain_at` post-draw stage:

1. O(1) early-out when `quantum_active_count == 0`; the timestamp
   was still refreshed so the first frame after a click never
   integrated a huge dt.
2. dt = real elapsed time since `last_quantum_update_time`, capped
   by `PARTICLE_MAX_FRAME_DT_SECS` (0.25s anti-teleport bound) and
   scaled by `resume_blend` during pause/resume ramps
   (S-master-HUNT-22 real-time physics clock).
3. Per active particle: `sim_age += dt`; expiry at `sim_age >= 
   lifetime`; trail ring-buffer push (shift-left when full); motion
   integration; exponential velocity decay
   (`QUANTUM_RIPPLE_VELOCITY_DECAY` = 0.35/s); specular edge bounce
   on all four screen edges with `QUANTUM_RIPPLE_BOUNCE_DAMPING`
   (0.78) and mirror-formula position correction plus a final clamp;
   three-segment brightness curve (HEAD full / BODY smoothstep to
   0.25 / TAIL linear to zero, split by `QUANTUM_RIPPLE_HEAD_END_FRAC`
   = 0.10 and `QUANTUM_RIPPLE_TAIL_START_FRAC` = 0.50); render-time
   color = palette sweep interpolation brightened to HSV value 200
   then tone-down by `QUANTUM_BODY_TONE_DOWN` (0.72); trail cells
   rendered with `QUANTUM_RIPPLE_TRAIL_DECAY` (0.55) per-step
   brightness; force-set writes (visible on transparent background).
4. Deactivated particles decremented `quantum_active_count`.

### 1.3 Shared pool

`QuantumParticle` (cloud/state.rs) was the pool element, shared with
the border-touch spark: `active`, `x/y/vx/vy`, `birth`, `ch`, RGB
snapshot, `trail_x/trail_y` ring buffer, `trail_count`, `max_trail`,
`lifetime`, `sim_age`. Scene switches and restarts cleared the pool
(`quantum_active_count = 0`) and shifted `last_quantum_update_time`
forward (pause bookkeeping, spawn_reset origin reset) so no stale dt
teleported particles.

---

## 2. Removed surface (night-moonlight-1)

The removal took the whole feature out with zero residual:

- `spawn_quantum_ripple` and its call in `set_mouse_click`;
- the quantum-only tuning constants
  (`QUANTUM_RIPPLE_PARTICLE_COUNT`, `QUANTUM_RIPPLE_LIFETIME_SECS`,
  `QUANTUM_RIPPLE_SPEED`);
- the three test files (`tests_quantum.rs`,
  `tests_quantum_hunt22.rs`, `tests_quantum_v50.rs`) covering spawn
  color snapshot, palette-switch crossfade, real-time expiry, edge
  bounce, and the v50 brightness/lifespan retune;
- with the border-spark removal in the same round, the shared pool
  itself: `QuantumParticle`, `quantum_particles`,
  `quantum_active_count`, `last_quantum_update_time`,
  `apply_quantum_ripple`, and the remaining
  `QUANTUM_BRAND_PURPLE_*` / `QUANTUM_RIPPLE_*` render constants.

The wave-equation click shockwave, the hover glow, and the
border-touch glow pulse were explicitly kept: they are separate
features riding the same click event.

---

## 3. Original spawn implementation (preserved)

```rust
pub(crate) fn spawn_quantum_ripple(&mut self, col: u16, line: u16) {
    // PERF-4: --no-effects gate. No-op when effects are disabled.
    if !self.effects_enabled {
        return;
    }
    let cx = col as f32 + 0.5;
    let cy = line as f32 + 0.5;
    let now = Instant::now();
    let chars = ['*', '+', '·'];
    let body_idx = self.palette.colors.len() / 2;
    let (body_r, body_g, body_b) = self
        .palette
        .colors
        .get(body_idx)
        .and_then(|c| crate::palette::decode_color(*c))
        .unwrap_or((
            QUANTUM_BRAND_PURPLE_R,
            QUANTUM_BRAND_PURPLE_G,
            QUANTUM_BRAND_PURPLE_B,
        ));
    let mut spawned = 0usize;
    for p in &mut self.quantum_particles {
        if spawned >= QUANTUM_RIPPLE_PARTICLE_COUNT {
            break;
        }
        if p.active {
            continue;
        }
        let angle: f32 = self.rand_chance.sample(&mut self.mt) * std::f32::consts::TAU;
        let speed = QUANTUM_RIPPLE_SPEED * (0.9 + self.rand_chance.sample(&mut self.mt) * 0.2);
        let char_idx =
            (self.rand_chance.sample(&mut self.mt).min(0.999) * chars.len() as f32) as usize;
        p.active = true;
        p.x = cx;
        p.y = cy;
        p.vx = angle.cos() * speed;
        p.vy = angle.sin() * speed;
        p.birth = now;
        p.ch = chars[char_idx.min(chars.len() - 1)];
        p.r = body_r;
        p.g = body_g;
        p.b = body_b;
        p.trail_count = 0;
        p.max_trail = QUANTUM_RIPPLE_TRAIL_LEN as u8;
        p.lifetime = QUANTUM_RIPPLE_LIFETIME_SECS;
        p.sim_age = 0.0;
        spawned += 1;
    }
    self.quantum_active_count = self.quantum_active_count.saturating_add(spawned);
}
```

The full update + render body (dt integration, trail push, bounce
math, brightness curve, chroma/legacy color routing, trail render)
is preserved in the git history of
`src/engine/cosmic_dragon_engine/cloud/rain_post.rs` at the removal
base commit.

---

## 4. Tuning constants (final values, preserved)

| Constant | Value | Meaning |
|---|---|---|
| `QUANTUM_RIPPLE_POOL_SIZE` | 96 | pre-allocated pool slots |
| `QUANTUM_RIPPLE_PARTICLE_COUNT` | 20 | particles per click |
| `QUANTUM_RIPPLE_LIFETIME_SECS` | 4.0 | particle lifespan |
| `QUANTUM_RIPPLE_SPEED` | 30.0 | cells/second base speed |
| `QUANTUM_RIPPLE_TRAIL_LEN` | 6 | comet trail ring-buffer depth |
| `QUANTUM_RIPPLE_TRAIL_DECAY` | 0.55 | per-step trail brightness decay |
| `QUANTUM_RIPPLE_VELOCITY_DECAY` | 0.35 | exponential velocity decay per second |
| `QUANTUM_RIPPLE_BOUNCE_DAMPING` | 0.78 | per-bounce velocity retention |
| `QUANTUM_RIPPLE_HEAD_END_FRAC` | 0.10 | HEAD segment end (life fraction) |
| `QUANTUM_RIPPLE_TAIL_START_FRAC` | 0.50 | TAIL segment start |
| `QUANTUM_BODY_TONE_DOWN` | 0.72 | render-time brightness tone-down |
| `QUANTUM_BRAND_PURPLE_*` | (168, 85, 247) | fallback color for degenerate palettes |

`PARTICLE_MAX_FRAME_DT_SECS` (0.25) was shared with the engrave and
scorch particle systems and REMAINS in the live tree after the
removal.

---

## 5. Re-implementation notes

Anyone re-implementing this effect should start from:

1. a pre-allocated pool plus an incremental active-count for O(1)
   early-out;
2. real-time physics with a generous anti-teleport dt cap
   (S-master-HUNT-22) — the old `min(dt, 1/30, sim_cap)` chain
   caused the VTE slow-motion bug family;
3. snapshot colors at spawn time for palette-switch crossfades;
4. specular edge bounce with mirror-formula position correction;
5. force-set rendering so particles stay visible on transparent
   backgrounds.

The known bug history (stale dt teleport on scene switch, VTE
slow-motion, velocity decay freeze) is documented in
`docs/archive/research/MOUSE_EFFECTS_AUDIT.md` and
`KNOWN_ISSUES.md` history.

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
