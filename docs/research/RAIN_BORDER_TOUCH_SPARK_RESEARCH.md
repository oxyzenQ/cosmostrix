<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Rain-Border Touch Spark — Particle Effect Research Addendum

> Source code is truth; cross-check the referenced files before relying on
> this analysis for implementation decisions. This document is an internal
> research artifact, not a contract.

**Date:** 2026-08-27
**Auditor:** oxyzenQ (Cosmic Dragon mode)
**Status:** RESEARCH ONLY — no coding yet. Owner wants cinematic options to decide.
**Parent doc:** `docs/research/RAIN_BORDER_TOUCH_GLOW_AUDIT.md` (Options A–E)
**This doc:** Adds Option F (Particle Spark) — the owner's requested "rain drop hitting a thing and showing spark" effect, smaller than the mouse-click quantum ripple.

---

## 1. Owner's Request

> "owner want to add effects like particles spark same effects usage at touch/click mouse but more little bit like rain drop hitting a thing and showing spark. give owner to an options masterclass cinematic let owner decide."

**Translation:** When a rain droplet's head touches the message-border top edge, emit a **small particle spark burst** — visually similar to the mouse-click quantum ripple (`spawn_quantum_ripple`), but scaled down (fewer particles, shorter lifetime, smaller velocity) to match the "rain drop hitting a surface" feel rather than a full click burst.

This is **Option F** — a 6th option that the parent audit did not cover (A–E are all single-cell glow/bloom/pulse effects; F is a **multi-particle** effect).

---

## 2. Precedent Audit — Mouse-Click Quantum Ripple

The mouse-click effect (`src/engine/cosmic_dragon_engine/cloud/spawn.rs:847 spawn_quantum_ripple`) is the closest precedent. It fires on mouse click and emits:

| Parameter | Value | Source |
|---|---|---|
| Particle count per click | 20 | `QUANTUM_RIPPLE_PARTICLE_COUNT` (constants.rs:248) |
| Pool size (max active particles) | 96 | `QUANTUM_RIPPLE_POOL_SIZE` (constants.rs:245) |
| Lifetime | 4.0 s | `QUANTUM_RIPPLE_LIFETIME_SECS` (constants.rs:276) |
| Speed | 30.0 cells/s | `QUANTUM_RIPPLE_SPEED` (constants.rs:295) |
| Speed variance | ±10% | `0.9 + rand * 0.2` (spawn.rs:896) |
| Bounce damping | (see constants) | `QUANTUM_RIPPLE_BOUNCE_DAMPING` |
| Trail | yes (ring buffer) | `QUANTUM_RIPPLE_TRAIL_LEN` |

**Particle struct** (`cloud/mod.rs:74 QuantumParticle`):

```rust
pub(crate) struct QuantumParticle {
    pub active: bool,
    pub x: f32, pub y: f32,        // float position (sub-cell)
    pub vx: f32, pub vy: f32,      // velocity (cells/sec)
    pub birth: Instant,
    pub ch: char,                  // glyph (from charset)
    pub r: u8, pub g: u8, pub b: u8,  // palette body color
    // + trail ring buffer (trail_x[], trail_y[], trail_count)
}

```

**Render path:** Particles are updated in `rain_post.rs` (`apply_quantum_ripple`), rendered as floating glyphs with trail streaks. They bounce off terminal edges with damping.

**Why this is too big for rain-border touch:** 20 particles × 4-second lifetime × trail = visually a major explosion. A rain drop touching the border should be a **tiny flicker** — 2–5 particles, 200–400ms lifetime, no trail (or 1-cell trail max).

---

## 3. Masterclass Cinematic Options — Option F (Particle Spark)

Three sub-variants of the particle spark, from most subtle to most dramatic. All reuse the `QuantumParticle` pool infrastructure but with spark-specific constants.

### 3.1 Option F1 — Micro-Spark (most subtle, recommended)

**Idea:** 3 particles, 250ms lifetime, no trail. A brief 3-pixel flicker that reads as "a drop of water hit something and splashed." Particles emit **upward and sideways** (not downward — the border is a ceiling, so sparks deflect up + out, like rain hitting a hard surface).

**Spark-specific constants (new, in constants.rs):**

```rust
/// Border-touch micro-spark: 3 particles, 250ms, no trail.
/// "Rain drop hitting a glass ceiling" — present but quiet.
pub(crate) const BORDER_SPARK_PARTICLE_COUNT: usize = 3;
pub(crate) const BORDER_SPARK_LIFETIME_SECS: f32 = 0.25;
pub(crate) const BORDER_SPARK_SPEED: f32 = 8.0;  // ~27% of quantum ripple (30.0)
/// Spark particles emit upward + sideways (ceiling deflection).
/// Angle range: [-150°, -30°] from horizontal (i.e. upward fan).
/// Downward (positive Y) is excluded — border is a ceiling.
pub(crate) const BORDER_SPARK_ANGLE_MIN_RAD: f32 = -2.618;  // -150°
pub(crate) const BORDER_SPARK_ANGLE_MAX_RAD: f32 = -0.524;  // -30°
/// No trail for micro-spark (trail_len = 0).
pub(crate) const BORDER_SPARK_TRAIL_LEN: usize = 0;
/// Max concurrent sparks (pool cap). At 60 FPS with ~80 columns,
/// expected ~2-5 touches/sec → 6-15 particles active. Cap at 48
/// (16 concurrent sparks × 3 particles) for safety.
pub(crate) const BORDER_SPARK_POOL_SIZE: usize = 48;

```

**Visual feel:** A 3-pixel upward flicker lasting ¼ second. Reads as "tic" — a single rain-drop tap. Does not compete with the message text or the chroma gradient.

**Trade-offs:**
- ✅ Most subtle — does not distract from the rain or message.
- ✅ Reuses `QuantumParticle` struct + render path (zero new infrastructure).
- ✅ Upward-only emission respects the "ceiling" metaphor.
- ✅ No trail = minimal cell writes (~3 cells × 15 frames = 45 cell-writes per spark).
- ❌ May be too subtle — owner might not notice it on a busy 200-column terminal.
- **Perf:** O(active_sparks) per frame, expected 6–15 particles — negligible (~50ns/particle).

---

### 3.2 Option F2 — Splash Crown (balanced, cinematic)

**Idea:** 6 particles, 350ms lifetime, 1-cell trail. Emits in a **semicircle upward fan** (180° arc, -180° to 0°) — mimicking the crown splash of a water drop hitting a hard surface. The 1-cell trail gives each particle a brief streak, reading as "spray."

**Spark-specific constants:**

```rust
/// Border-touch splash crown: 6 particles, 350ms, 1-cell trail.
/// "Water drop crown splash" — visible but brief.
pub(crate) const BORDER_SPARK_PARTICLE_COUNT: usize = 6;
pub(crate) const BORDER_SPARK_LIFETIME_SECS: f32 = 0.35;
pub(crate) const BORDER_SPARK_SPEED: f32 = 12.0;  // 40% of quantum ripple
/// Full upward semicircle: -180° to 0° (left to right, arc over the top).
pub(crate) const BORDER_SPARK_ANGLE_MIN_RAD: f32 = -3.14159;  // -180°
pub(crate) const BORDER_SPARK_ANGLE_MAX_RAD: f32 = 0.0;       // 0°
pub(crate) const BORDER_SPARK_TRAIL_LEN: usize = 1;
pub(crate) const BORDER_SPARK_POOL_SIZE: usize = 96;  // 16 sparks × 6 particles

```

**Visual feel:** A 6-particle semicircle splash that lasts ~⅓ second with short streaks. Reads as "plash" — a small water-crown. More visible than F1, still subtle enough not to compete with the message.

**Trade-offs:**
- ✅ Visually clear — owner will see it without tuning.
- ✅ Semicircle fan reads naturally as "impact splash."
- ✅ 1-cell trail adds motion without being noisy.
- ❌ 6 particles × 21 frames × trail = ~126 cell-writes per spark (3× F1).
- ❌ On high-density rain (many touches/sec), the border top may look "sparkly" — could be distracting.
- **Perf:** O(active_sparks) per frame, expected 12–30 particles — ~1.5µs, negligible.

---

### 3.3 Option F3 — Spark + Glow Ring (most dramatic, cinematic)

**Idea:** Combines F2 (6-particle splash crown) with a **1-frame expanding ring** at the touch point (like a mini mouse-click flash wave, but 1/4 scale). The ring is a single-cell-radius expanding circle lasting 200ms, drawn in the head_rgb color. This is the full "drop hitting water" effect: ring + splash.

**Additional ring constants:**

```rust
/// Border-touch spark ring: mini flash wave, 200ms, 1/4 scale of mouse-click.
pub(crate) const BORDER_SPARK_RING_SPEED: f32 = 8.0;       // vs MOUSE_FLASH_SPEED 32.0
pub(crate) const BORDER_SPARK_RING_WIDTH: f32 = 2.0;       // vs MOUSE_FLASH_RING_WIDTH 8.0
pub(crate) const BORDER_SPARK_RING_INTENSITY: f32 = 0.4;   // vs MOUSE_FLASH_INTENSITY 0.85
pub(crate) const BORDER_SPARK_RING_DURATION_SECS: f32 = 0.2;  // vs 1.8

```

**Visual feel:** A small expanding ring + 6-particle upward splash. Reads as "kapow" — a mini water-impact. Most cinematic of the three, but also most attention-grabbing.

**Trade-offs:**
- ✅ Most cinematic — clearly conveys "rain hitting ceiling."
- ✅ Ring + splash combination is visually rich.
- ❌ Most intrusive — may compete with the message text on small terminals.
- ❌ Requires reusing the `FlashWave` render path (additional DrawCtx field).
- ❌ Highest perf cost: ring (O(box_w) per active ring) + 6 particles.
- **Perf:** O(active_sparks × box_w) per frame — ~3µs at expected load, still negligible.

---

## 4. Trigger Condition (shared by F1/F2/F3)

All three spark variants use the **same trigger** as Options A–E from the parent audit:

> A droplet whose `bound_col` falls inside `[start_col, start_col + box_w)` and whose `head_put_line` transitions from `< start_line` to `== start_line` between the previous frame and the current frame.

**Corner-skip guard (LTS invariant):** Spark emission is **suppressed** when the touched border cell is a corner (`╭`, `╮`, `╰`, `╯`). This preserves the "no lone bright heads at top corners" invariant from the parent audit §2.5.

**Pool cap:** Each spark consumes `BORDER_SPARK_PARTICLE_COUNT` particles from the `BORDER_SPARK_POOL_SIZE` pool. If the pool is full (rare on normal rain density), the touch is silently dropped (same pattern as `spawn_quantum_ripple`).

---

## 5. Implementation Sketch (Option F1, recommended first)

Estimated complexity: ~80 LOC across 3 files + 1 test file. **Reuses existing `QuantumParticle` struct + render path** — no new particle infrastructure.

### 5.1 New constants (types/constants.rs)

```rust
// ── Border-Touch Micro-Spark (Option F1) ────────────────────────────────
// See docs/research/RAIN_BORDER_TOUCH_SPARK_RESEARCH.md §3.1.
// "Rain drop hitting a glass ceiling" — 3 particles, 250ms, no trail.

pub(crate) const BORDER_SPARK_PARTICLE_COUNT: usize = 3;
pub(crate) const BORDER_SPARK_LIFETIME_SECS: f32 = 0.25;
pub(crate) const BORDER_SPARK_SPEED: f32 = 8.0;
pub(crate) const BORDER_SPARK_ANGLE_MIN_RAD: f32 = -2.618;  // -150°
pub(crate) const BORDER_SPARK_ANGLE_MAX_RAD: f32 = -0.524;  // -30°
pub(crate) const BORDER_SPARK_TRAIL_LEN: usize = 0;
pub(crate) const BORDER_SPARK_POOL_SIZE: usize = 48;

```

### 5.2 Spawn function (cloud/spawn.rs, alongside `spawn_quantum_ripple`)

```rust
/// Spawn a border-touch micro-spark at (col, line).
/// See docs/research/RAIN_BORDER_TOUCH_SPARK_RESEARCH.md (Option F1).
///
/// Up to `BORDER_SPARK_PARTICLE_COUNT` particles are activated per touch,
/// emitting upward (ceiling deflection). Particles use the existing
/// `QuantumParticle` pool — sparks share the pool with quantum ripples.
/// Pool cap: if the pool is full, the touch is silently dropped (same
/// pattern as spawn_quantum_ripple).
pub(crate) fn spawn_border_spark(&mut self, col: u16, line: u16) {
    let mut spawned = 0;
    for p in &mut self.quantum_particles {
        if spawned >= BORDER_SPARK_PARTICLE_COUNT {
            break;
        }
        if !p.active {
            // Random angle in [-150°, -30°] (upward fan)
            let angle = BORDER_SPARK_ANGLE_MIN_RAD
                + self.rand_chance.sample(&mut self.mt)
                    * (BORDER_SPARK_ANGLE_MAX_RAD - BORDER_SPARK_ANGLE_MIN_RAD);
            let speed = BORDER_SPARK_SPEED
                * (0.9 + self.rand_chance.sample(&mut self.mt) * 0.2);
            *p = QuantumParticle {
                active: true,
                x: col as f32,
                y: line as f32,
                vx: speed * angle.cos(),
                vy: speed * angle.sin(),  // negative = upward
                birth: Instant::now(),
                ch: self.chars[self.rand_cpidx.sample(&mut self.mt) as usize],
                r: self.head_rgb.0,
                g: self.head_rgb.1,
                b: self.head_rgb.2,
                trail_count: 0,  // no trail for micro-spark
            };
            spawned += 1;
        }
    }
    self.quantum_active_count = self.quantum_active_count.saturating_add(spawned);
}

```

### 5.3 Trigger wiring (cloud/rain.rs, in `detect_border_touch`)

The existing `detect_border_touch` (rain.rs:1226) already fires on the transition. Add the spark spawn call:

```rust
pub(crate) fn detect_border_touch(&mut self, col: u16, prev_hp: u16, hp: u16, now: Instant) {
    // ... existing guard checks ...

    // Existing: push a BorderPulse (Option C from parent audit)
    // ... (existing pulse code) ...

    // NEW: spawn micro-spark (Option F1)
    // Skip corners (LTS invariant: no lone bright heads at top corners)
    if let Some(idx) = msg_idx {
        let mc = &self.message[idx];
        if !matches!(mc.val, '╭' | '╮' | '╰' | '╯') {
            self.spawn_border_spark(col, top);
        }
    }
}

```

### 5.4 Render path (cloud/rain_post.rs `apply_quantum_ripple`)

**No changes needed** — sparks use the same `QuantumParticle` struct, so the existing `apply_quantum_ripple` render path handles them automatically. The only difference is the spark particles have `trail_count = 0` (no trail), which the render path already supports.

### 5.5 Benchmark mode (Z-6 / PERF-1 gate)

Sparks are **cosmetics** — they must be gated behind `!self.bench_mode` (same as the border-cross detection that triggers them). Since `detect_border_touch` is already gated behind `!bench_mode` (Z-6), the spark spawn is automatically skipped in benchmark mode. No additional gate needed.

---

## 6. Perf Budget (all three variants)

| Variant | Particles/touch | Lifetime | Cell-writes/touch | Perf cost/frame | Notes |
|---|---|---|---|---|---|
| F1 Micro-Spark | 3 | 250ms | ~45 | ~50ns × 6–15 active | Negligible |
| F2 Splash Crown | 6 | 350ms | ~126 | ~50ns × 12–30 active | Negligible |
| F3 Spark + Ring | 6 + ring | 350ms + 200ms | ~126 + ring | ~3µs total | Negligible |

All three are well under 1% of the frame budget at 60 FPS (16.67ms). The existing `QuantumParticle` pool is pre-allocated — zero per-frame heap allocations.

---

## 7. Recommendation

**Start with Option F1 (Micro-Spark).** Rationale:

1. **Smallest code surface** — ~80 LOC, reuses `QuantumParticle` pool, no new render path.
2. **Most subtle** — does not compete with the message text or chroma gradient.
3. **LTS-safe** — corner-skip guard preserves all four border invariants from the parent audit.
4. **Tunable** — if F1 is too subtle, bump `BORDER_SPARK_PARTICLE_COUNT` from 3→6 (becomes F2) or add the ring (becomes F3) in one-line constant changes.
5. **Benchmark-safe** — automatically skipped in bench mode (trigger is already gated).

**Escalation path:**
- F1 too subtle → F2 (Splash Crown): change 3 constants (count 3→6, lifetime 0.25→0.35, trail 0→1).
- F2 not cinematic enough → F3 (Spark + Ring): add ring constants + reuse `FlashWave` render path.

---

## 8. Open Questions for Owner

1. **F1 vs F2 vs F3?** Owner's call on subtlety level. F1 is "tic", F2 is "plash", F3 is "kapow."
2. **Spark color?** Default = `head_rgb` (palette last stop, usually white). Alternative: palette body color (mid-stop) for a less stark look.
3. **Spark glyph?** Default = random from charset (same as quantum ripple). Alternative: fixed `·` (middle dot) for a more "water droplet" feel.
4. **Cap concurrent sparks?** Default = `BORDER_SPARK_POOL_SIZE` (48 particles = 16 concurrent sparks at F1's 3 particles each). Owner may want lower cap on high-density rain scenarios.

---

## 9. Audit Signoff

**Task:** Rain-border touch spark particle effect research (Option F).
**Status:** RESEARCH ONLY — no coding. Owner to decide F1/F2/F3 + answer open questions in §8.
**Artifacts:** This report only.

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
