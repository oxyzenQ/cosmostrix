<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Mouse Effects System — Deep Audit

> ## v50.0.0-alpha.5 Update Note (2026-08-17)
>
> The audit below documents the ORIGINAL mouse effects pipeline (pre-v50).
> Three owner-approved masterclass-tier upgrades have since been applied
> (see CHANGELOG.md v50.0.0-alpha.5 for the full description):
>
> 1. **Quantum ripple color cycling** (peak optimize #3): the rendered
>    particle color now sweeps palette[0] -> palette[last] over the
>    particle's 2.5s lifespan via `interpolate_palette_color`, instead
>    of being locked to the spawn-time snapshot body color. The snapshot
>    is preserved on the struct for backward-compat with the crossfade
>    regression tests, but is no longer the source of truth for the
>    RENDERED color.
>
> 2. **Chromatic shockwave** (alternative for flash wave): the flash
>    wave now blends each cell toward the active palette's HEAD color
>    (`palette[last]`) instead of pure white `(255,255,255)`. For most
>    schemes the head is near-white so the visual difference is subtle;
>    for saturated schemes (Red, Fire, Cosmos) the flash takes on a
>    distinctly-colored hue tied to the active palette.
>
> 3. **Trail particles** (alternative for quantum ripple): each particle
>    now leaves a "comet trail" of its last `QUANTUM_RIPPLE_TRAIL_LEN=6`
>    positions, rendered with the cycled color + diminishing brightness
>    via `QUANTUM_RIPPLE_TRAIL_DECAY=0.55`. Adds cinematic motion blur
>    to the click-triggered particle burst.
>
> All three effects route through the chroma dragon pipeline (primary)
> with legacy fallback (non-TrueColor terminals) — consistent with
> the LTS-wide chroma dragon sync mandate (C4-C6). The audit's
> description of the original pipeline architecture is preserved
> below for historical reference; the implementation details
> (constants, struct fields, blend paths) have changed.
>
> ---

**Task ID**: `mouse-effects-audit-1`
**Agent**: mouse-effects-audit-1 (general-purpose sub-agent)
**Scope**: Peak-optimization audit of the mouse effects pipeline (cursor glow + dual-ring click flash wave + Quantum Ripple particle burst).
**Mode**: Read-only. No source code modified.
**Date**: 2026 (post visual-mode retune)

---

## 1. Architecture Overview

The mouse effects system spans **5 source files** and **2 constants modules**. The pipeline is:

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  1. EVENT CAPTURE                                                           │
│  src/interactive/event_loop.rs:852-881  (Event::Mouse handler)              │
│  ──────────────────────────────────────────                                 │
│  crossterm::event::Event::Mouse(m)                                          │
│    ├─ cloud.set_mouse_position(m.column, m.row)        ← always             │
│    └─ if MouseEventKind::Down(_):                                            │
│         cloud.set_mouse_click(m.column, m.row)         ← on press only      │
└──────────────────────────────────────────────────────────────────────────────┘
                                     │
                                     ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│  2. STATE UPDATE                                                            │
│  src/cloud/mod.rs:441-465  (Cloud::set_mouse_click)                         │
│  ────────────────────────────────────────────                               │
│  • Scan self.flash_waves[MOUSE_FLASH_POOL_SIZE=4] for first inactive slot   │
│  • If none inactive → evict slot with smallest `birth` (oldest)             │
│  • Set slot = { active:true, col, line, birth:now }                         │
│  • Call spawn_quantum_ripple(col, line)                                     │
│                                                                              │
│  src/cloud/spawn.rs:740-789  (Cloud::spawn_quantum_ripple)                 │
│  ──────────────────────────────────────────────                             │
│  • Snapshot palette body color (mid-index)                                  │
│  • For up to QUANTUM_RIPPLE_PARTICLE_COUNT=20 particles:                    │
│    - Find first inactive slot in 64-element pool                            │
│    - Random angle ∈ [0, 2π), speed ∈ [0.8, 1.2) × QUANTUM_RIPPLE_SPEED      │
│    - Set p.{active,x,y,vx,vy,birth,ch,r,g,b}                                │
│  • quantum_active_count += spawned                                          │
└──────────────────────────────────────────────────────────────────────────────┘
                                     │
                                     ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│  3. PER-FRAME PRECOMPUTE  (runs every frame in rain_at)                    │
│  src/cloud/rain.rs:583-595                                                  │
│  ─────────────────────                                                      │
│  let mut flash_waves_buf: SmallVec<[FlashWaveCtx; 4]> = SmallVec::new();   │
│  for w in &self.flash_waves:                                                │
│    if w.active && w.birth.elapsed() < MOUSE_FLASH_DURATION_SECS:            │
│      flash_waves_buf.push(FlashWaveCtx { col, line, elapsed })              │
│  // DrawCtx.flash_waves = &flash_waves_buf  (borrows for the draw call)     │
└──────────────────────────────────────────────────────────────────────────────┘
                                     │
                                     ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│  4. PER-CELL RENDERING  (inside droplet draw loop)                          │
│  src/droplet.rs:727-816                                                     │
│  ─────────────────────                                                      │
│  for line in start_line..=head_put_line:                                    │
│    /* (a) Cursor glow  — droplet.rs:727-748 */                              │
│    if ctx.mouse_col != u16::MAX:                                            │
│      col_dist = abs(bound_col - mouse_col)                                  │
│      line_dist = abs(line - mouse_line)                                     │
│      norm_col = col_dist / MOUSE_GLOW_RADIUS_COLS  (=7.0)                   │
│      norm_line = line_dist / MOUSE_GLOW_RADIUS_LINES (=5.0)                 │
│      dist_sq = norm_col² + norm_line²                                       │
│      if dist_sq < 1.0:                                                      │
│        glow = (1 - dist_sq) × MOUSE_GLOW_INTENSITY  ← CONST = 0.0 !!        │
│        r,g,b += (255 - r,g,b) × glow × 256 / 256  ← NO-OP                   │
│                                                                              │
│    /* (b) Flash wave dual-ring  — droplet.rs:769-816 */                     │
│    for w in ctx.flash_waves:                                                │
│      col_dist = abs(bound_col - w.col)                                      │
│      line_dist = abs(line - w.line)                                         │
│      euclidean = sqrt(col_dist² + line_dist²)         ← PER CELL PER WAVE  │
│      raw_fade = (1 - elapsed / 1.8).max(0)            ← wave-invariant     │
│      fade = raw_fade × sqrt(raw_fade)                 ← wave-invariant     │
│      primary_radius = elapsed × 32.0                  ← wave-invariant     │
│      secondary_radius = elapsed × 32.0 × 0.4          ← wave-invariant     │
│      if |euclidean - primary_radius| < 8.0:                                 │
│        factor = (1 - primary_dist/8)² × 0.85 × fade                         │
│      if |euclidean - secondary_radius| < 8.0:                               │
│        factor += (1 - sec_dist/8)² × 0.85 × 0.45 × fade                     │
│      if factor > 0:                                                         │
│        r,g,b += (255 - r,g,b) × factor × 256 / 256                          │
│                                                                              │
│    frame.set_force(bound_col, line, Cell{ch,fg,bg,bold})                    │
└──────────────────────────────────────────────────────────────────────────────┘
                                     │
                                     ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│  5. POST-PROCESS  (after droplet draw, before phosphor)                    │
│  src/cloud/rain.rs:732 + 1082-1218  (Cloud::apply_quantum_ripple)          │
│  ─────────────────────────────────────────────────────────────────────      │
│  Early-out if quantum_active_count == 0                                     │
│  Else: for each active particle:                                            │
│    • age = now - p.birth ; if age >= 0.8s → deactivate                      │
│    • p.x += p.vx × dt ; p.y += p.vy × dt  (dt clamped to 1/30)              │
│    • Compute tone-down RGB: p.r × 0.72 (rounded)                            │
│    • Blend particle color onto existing cell foreground                     │
│    • frame.set_force(col, line, Cell{particle_ch, blended_rgb, bg, bold})   │
└──────────────────────────────────────────────────────────────────────────────┘
                                     │
                                     ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│  6. EXPIRY SWEEP  (end of rain_at)                                          │
│  src/cloud/rain.rs:939-945                                                  │
│  ─────────────────────                                                      │
│  for w in &mut self.flash_waves:                                            │
│    if w.active && now - w.birth >= MOUSE_FLASH_DURATION_SECS:               │
│      w.active = false                                                       │
└──────────────────────────────────────────────────────────────────────────────┘
```

### File map

| File | Role |
|---|---|
| `src/interactive/event_loop.rs:852-881` | Event capture: dispatches `Event::Mouse` → `set_mouse_position` + `set_mouse_click` |
| `src/interactive/input.rs` | Keyboard only — no mouse handling. (Verified: no `mouse`/`Mouse` references.) |
| `src/cloud/mod.rs:436-465` | `set_mouse_position` + `set_mouse_click` (pool slot selection, eviction) |
| `src/cloud/mod.rs:613-618` | Pause-time birth shift for flash waves |
| `src/cloud/spawn.rs:740-789` | `spawn_quantum_ripple` (particle pool fill) |
| `src/cloud/render.rs:27-54` | `FlashWaveCtx` (precomputed view) + `FlashWave` (runtime slot) types |
| `src/cloud/render.rs:115` | `DrawCtx::flash_waves: &[FlashWaveCtx]` field |
| `src/cloud/rain.rs:583-595` | Per-frame precompute of active flash waves into `SmallVec` |
| `src/cloud/rain.rs:732` | Call to `apply_quantum_ripple` |
| `src/cloud/rain.rs:939-945` | Per-frame expiry sweep |
| `src/cloud/rain.rs:1082-1218` | `apply_quantum_ripple` (particle update + render) |
| `src/droplet.rs:727-748` | Per-cell cursor glow (DEAD CODE — see §3.1) |
| `src/droplet.rs:769-816` | Per-cell flash wave dual-ring blend |
| `src/central_control_rains.rs:487-524` | All `MOUSE_*` constants |
| `src/quantum_constants.rs` | All `QUANTUM_*` constants |
| `src/cloud/tests/tests_quantum.rs:645-768` | fix regression tests |

---

## 2. Per-File Analysis

### 2.1 `src/interactive/event_loop.rs` — Event Capture

**Lines 852-881** (the `Event::Mouse` arm):

```rust
Event::Mouse(m) => {
    let activity_time = Instant::now();
    let _ = register_activity(/* ... */);
    cloud.set_mouse_position(m.column, m.row);
    if matches!(m.kind, MouseEventKind::Down(_)) {
        cloud.set_mouse_click(m.column, m.row);
    }
}
```

**Findings:**

- **C1 (correctness, minor):** `set_mouse_position` is called for ALL mouse events including `MouseEventKind::ScrollUp`/`ScrollDown`/`Drag(_)`/`Moved(_)`/`Up(_)`. Scroll events at position (col, row) will move the cursor glow anchor to (col, row) even though the user did not move the pointer. Invisible today because `MOUSE_GLOW_INTENSITY = 0.0`, but becomes a visible glitch if the glow is ever re-enabled. Suggested guard: only call `set_mouse_position` for `Moved | Drag | Down` kinds.

- **C2 (correctness, minor):** No `FocusLost` handling — mouse position is never reset to `u16::MAX` when the terminal loses focus. The glow anchor persists at the last in-window position. Same caveat as C1 (invisible while intensity is 0).

- **C3 (correctness, minor):** `MouseEventKind::Down(_)` triggers `set_mouse_click` for all three buttons (Left, Right, Middle). Right-click typically emits a `Down(Right)` event in crossterm → spawns a flash wave + quantum ripple. Whether this is desired depends on owner intent (the comments only mention "click" generically). Not a bug, but worth flagging — right-click is conventionally a context-menu action, not a "ripple" trigger.

- **P1 (perf, negligible):** `Instant::now()` is called once per mouse event for `activity_time`. This is fine — mouse events are infrequent (~10-100 Hz during fast motion).

### 2.2 `src/cloud/mod.rs` — Click State Update

**Lines 441-465** (`set_mouse_click`):

```rust
pub fn set_mouse_click(&mut self, col: u16, line: u16) {
    let now = Instant::now();                              // syscall #1
    let mut slot = None;
    let mut oldest = (0usize, Instant::now());             // syscall #2  ← redundant
    for (i, w) in self.flash_waves.iter_mut().enumerate() {
        if !w.active { slot = Some(i); break; }
        if i == 0 || w.birth < oldest.1 {
            oldest = (i, w.birth);
        }
    }
    let s = slot.unwrap_or(oldest.0);
    self.flash_waves[s] = FlashWave { active: true, col, line, birth: now };
    self.spawn_quantum_ripple(col, line);                  // calls Instant::now() again (#3)
}
```

**Findings:**

- **P2 (perf, minor):** `Instant::now()` called twice in this function (lines 445, 447) and a third time in `spawn_quantum_ripple` (spawn.rs:743). All three could share a single `now` passed down. On Linux this is a vDSO call (~20-40 ns), so the saving is ~60-80 ns per click. Negligible for human-speed clicking (~5-10 cps) but harmless to fix.

- **C4 (correctness, subtle):** The `oldest` initialization `(0usize, Instant::now())` relies on the invariant "any past `w.birth` is strictly less than `Instant::now()`". This is true because `birth` was set in the past. However, the code's correctness depends on this implicit invariant. A cleaner pattern uses `Option<(usize, Instant)>`:
  ```rust
  let mut oldest: Option<usize> = None;
  for (i, w) in self.flash_waves.iter().enumerate() {
      if !w.active { slot = Some(i); break; }
      if oldest.is_none() || w.birth < self.flash_waves[oldest.unwrap()].birth {
          oldest = Some(i);
      }
  }
  ```
  Not a bug, but the current code is fragile to refactoring.

- **C5 (correctness, edge case):** If two clicks happen so fast that both record the same `birth` (sub-microsecond), the eviction policy picks index 0 (the `i == 0` short-circuit in the `||`). On supported platforms `Instant` has nanosecond resolution so this is unlikely, but on platforms with coarser clocks (older Windows builds: ~15 ms granularity) two rapid clicks could share a birth time, and the eviction would always pick index 0 — i.e., the same slot keeps getting evicted on each overflow click. The wave at slot 0 would never complete its 1.8s animation under sustained click storms. Realistically only an issue on Windows pre-Win10.

- **V30 fix verification:** The pool design correctly handles double-click (verified by `flash_wave_pool_double_click_keeps_both_waves` test at tests_quantum.rs:684) and overflow (verified by `flash_wave_pool_overflow_evicts_oldest` at tests_quantum.rs:712). The fix is solid for the documented "2-3 rapid clicks" use case. Edge cases beyond 4 concurrent clicks are handled by eviction, which is acceptable per the design comment.

### 2.3 `src/cloud/spawn.rs` — Quantum Ripple Spawner

**Lines 740-789** (`spawn_quantum_ripple`):

**Findings:**

- **P3 (perf, minor):** Each click triggers `self.rand_chance.sample(&mut self.mt)` 3 times per particle (angle, speed, char_idx). For 20 particles, that's 60 RNG draws. The RNG is `StdRng` (ChaCha12BlockRand), which is fast but not free (~5-10 ns/draw). Total ~300-600 ns per click. Negligible.

- **D1 (doc bug):** `quantum_constants.rs:22-24` says "32 covers the peak case of 2-3 rapid clicks (each spawns up to 25) with overlap." But:
  - `QUANTUM_RIPPLE_POOL_SIZE = 64` (not 32)
  - `QUANTUM_RIPPLE_PARTICLE_COUNT = 20` (not 25)
  - 64 / 20 = 3.2 clicks worth of pool capacity, not "2-3"

  The comment is stale. Should be updated to: "64 covers the peak case of 3 rapid clicks (each spawns 20) with overlap."

- **C6 (correctness, silent drop):** When the particle pool is full (3 active clicks + partial 4th), `spawn_quantum_ripple` silently drops the new particles — `spawned` stays below `QUANTUM_RIPPLE_PARTICLE_COUNT`. The doc comment acknowledges this: "clicks beyond the pool capacity are silently dropped (the flash wave still spawns)." This is by design, but the user gets no feedback that their click was partially dropped. The flash wave still appears, which is the more visually prominent effect, so this is acceptable.

### 2.4 `src/cloud/render.rs` — DrawCtx & FlashWaveCtx Types

**Lines 27-54** + **Lines 109-115**:

**Findings:**

- **P4 (perf, optimization opportunity):** `FlashWaveCtx` has 3 fields: `col`, `line`, `elapsed`. The per-cell renderer (droplet.rs:785-807) then recomputes from `elapsed`:
  - `raw_fade = (1 - elapsed / 1.8).max(0)`
  - `fade = raw_fade × sqrt(raw_fade)`
  - `primary_radius = elapsed × 32.0`
  - `secondary_radius = elapsed × 32.0 × 0.4`

  All four are pure functions of `elapsed` (a wave property, not a cell property). They are recomputed for **every cell × every wave**. With 200 active cells × 4 waves × 60 fps = 48,000 redundant computations/sec. Each involves 1 sqrt + ~5 mul/div. Estimated savings from precomputation: ~150-300K cycles/sec → ~50-100 µs/sec → negligible in absolute terms but easy to eliminate.

  **Recommended change:** extend `FlashWaveCtx` with `primary_radius`, `secondary_radius`, `fade` (or `fade × MOUSE_FLASH_INTENSITY`). Compute once in rain.rs:583-595.

### 2.5 `src/cloud/rain.rs` — Per-Frame Precompute + Expiry Sweep

**Lines 583-595** (precompute) + **939-945** (expiry):

```rust
let mut flash_waves_buf: SmallVec<[FlashWaveCtx; 4]> = SmallVec::new();
for w in &self.flash_waves {
    if w.active {
        let e = w.birth.elapsed().as_secs_f32();
        if e < MOUSE_FLASH_DURATION_SECS {
            flash_waves_buf.push(FlashWaveCtx { col, line, elapsed: e });
        }
    }
}
```

**Findings:**

- **P5 (perf, negligible):** `SmallVec::new()` is called every frame. Since the inline capacity is 4 and we never push more than 4, this is purely a stack allocation (no heap). Cost: ~0. The pattern is fine.

- **P6 (perf, negligible):** The precompute loop iterates all 4 pool slots every frame, even when none are active. The check `if w.active` short-circuits the elapsed computation. Cost: 4 branch checks + 4 array reads = ~8 ns/frame. Negligible. Could be eliminated with an `active_count` tracker (like `quantum_active_count`), but the ROI is poor.

- **P7 (perf, minor):** `w.birth.elapsed()` calls `Instant::now()` internally. So the precompute loop calls `Instant::now()` 4 times per frame (once per pool slot, when active). Could be hoisted to a single `let now_secs = now.elapsed().as_secs_f32()` — but wait, `birth.elapsed()` is `Instant::now() - birth`, and the `now` parameter is already passed to `rain_at`. Better: `now.saturating_duration_since(w.birth).as_secs_f32()`. Saves 3-4 `Instant::now()` syscalls per frame when waves are active.

### 2.6 `src/droplet.rs` — Per-Cell Rendering (HOT PATH)

**Lines 727-748** (cursor glow) + **769-816** (flash wave):

This is the most impactful file. See §3 (Performance Findings) for the full breakdown.

**Findings:**

- **P8 (perf, HIGH IMPACT):** The cursor glow block (lines 727-748) is **dead code** because `MOUSE_GLOW_INTENSITY = 0.0`. The math runs per-cell but produces zero color change. See §3.1 for details.

- **P9 (perf, MEDIUM IMPACT):** The flash wave block (lines 769-816) computes `sqrt(col_dist² + line_dist²)` per cell per wave. For cells far from the wave's current radius, this is wasted — a squared-distance early-out would skip them. See §3.2.

- **P10 (perf, MEDIUM IMPACT):** Wave-invariant quantities (`primary_radius`, `secondary_radius`, `fade`) are recomputed per cell. See §3.3.

- **P11 (perf, LOW IMPACT):** The `col_dist` / `line_dist` computation uses `if/else` for abs-diff:
  ```rust
  let col_dist = if self.bound_col > w.col {
      (self.bound_col - w.col) as f32
  } else {
      (w.col - self.bound_col) as f32
  };
  ```
  `u16::abs_diff` would be cleaner and possibly 1 instruction shorter. Same applies to `mouse_col`/`mouse_line` in the cursor glow block.

- **C7 (correctness, visual quality):** The flash wave color contribution is only applied to cells inside the per-droplet draw loop (`for line in start_line..=head_put_line`). Empty cells (no active droplet trail) get NO wave contribution. This means the wave's expanding ring is **invisible** in regions of the screen with no rain. Visually, the wave appears "broken" or "gappy" in sparse areas — only the parts of the ring that intersect active rain trails light up. This may be intentional (the wave "tints" existing rain rather than painting empty space), but it's a visual quality limitation worth flagging. The same applies to the cursor glow.

### 2.7 `src/cloud/mod.rs:613-618` — Pause Birth Shift

```rust
// fix: shift ALL active flash wave births (was single slot).
for w in &mut self.flash_waves {
    if w.active {
        w.birth += elapsed;
    }
}
```

**Findings:**

- **C8 (correctness, BUG):** Quantum particle births are **NOT** shifted on pause/resume. Only `last_quantum_update_time += elapsed` is updated (line 585). This means after a long pause, `now - p.birth` includes the pause duration, so all active particles instantly expire on the first frame after unpause (their age exceeds `QUANTUM_RIPPLE_LIFETIME_SECS = 0.8s`).

  Reproduction: spawn a click, immediately press `p` to pause, wait >0.8s, press `p` to unpause. The quantum ripple particles vanish instantly on unpause (instead of continuing their motion). The flash wave itself survives correctly (its `birth` was shifted).

  **Severity:** Minor visual bug — particles are short-lived (0.8s) so most users wouldn't notice. But it's an inconsistency between the flash wave (survives pause) and quantum ripple (doesn't survive pause).

  **Fix:** In `toggle_pause` BRANCH 2, add:
  ```rust
  for p in &mut self.quantum_particles {
      if p.active {
          p.birth += elapsed;
      }
  }
  ```

### 2.8 `src/cloud/spawn.rs:25-153` — `reset()` Does Not Clear Mouse State

**Findings:**

- **C9 (correctness, minor):** `reset()` (called on terminal resize, space-bar replay, etc.) does NOT clear:
  - `self.flash_waves` (active click waves continue to expand from old positions)
  - `self.quantum_particles` (active particles continue to move)
  - `self.quantum_active_count`
  - `self.mouse_col` / `self.mouse_line` (cursor position may now be off-screen)

  The flash waves and particles self-expire within 1.8s / 0.8s, so the visual impact is bounded. The mouse position is invisible (MOUSE_GLOW_INTENSITY=0). But for cleanliness, `reset()` could clear these:
  ```rust
  for w in &mut self.flash_waves { w.active = false; }
  for p in &mut self.quantum_particles { p.active = false; }
  self.quantum_active_count = 0;
  self.mouse_col = u16::MAX;
  self.mouse_line = u16::MAX;
  ```

  **Counter-argument:** the existing behavior preserves "click-then-resize" visual continuity (the wave keeps expanding from the click point in the new coordinate space, which is correct if the click was within the new bounds). So this is a judgment call, not a clear bug.

---

## 3. Performance Findings (Ranked by Impact)

### 3.1 [HIGH] Cursor glow block is dead code (`MOUSE_GLOW_INTENSITY = 0.0`)

**Location:** `src/droplet.rs:727-748` + `src/central_control_rains.rs:496`

```rust
pub(crate) const MOUSE_GLOW_INTENSITY: f32 = 0.0;  // ← zero!
```

The cursor glow block:
```rust
if ctx.mouse_col != u16::MAX {
    let col_dist = ...; let line_dist = ...;
    let norm_col = col_dist / MOUSE_GLOW_RADIUS_COLS;
    let norm_line = line_dist / MOUSE_GLOW_RADIUS_LINES;
    let dist_sq = norm_col * norm_col + norm_line * norm_line;
    if dist_sq < 1.0 {
        let glow = (1.0 - dist_sq) * MOUSE_GLOW_INTENSITY;  // = 0.0
        let wf = (glow * 256.0) as i32;                     // = 0
        r = (r as i32 + ((255 - r as i32) * wf + 128) / 256).clamp(0, 255) as u8;  // = r
        g = ...;  // = g
        b = ...;  // = b
    }
}
```

**Impact:** Every cell of every active droplet evaluates the outer `if` (1 comparison) + the inner distance math (2 abs-diff, 2 f32 div, 2 f32 mul, 1 f32 add, 1 f32 cmp). For cells inside the elliptical glow region (15 cols × 11 lines around the cursor = ~130 cells), the inner block also runs (4 more mul, 3 clamp) — all producing zero visual change.

**Estimated cost:** At 200×60 terminal with 30% active droplets (~2000 cells/frame), ~2000 outer-checks + ~130 inner-blocks = ~6000-8000 ops/frame. At 60 fps = 360-480K ops/sec. Roughly 100-200 µs/sec of wasted work.

**Fix options (in order of preference):**

1. **Delete the block** if cursor glow is no longer a desired feature. The docstring at `central_control_rains.rs:495-496` says "Intensity of the mouse hover glow (0.0 = disabled in default mode)" — suggesting it's a disabled-by-default feature. If never to be re-enabled, delete.

2. **Const-gate the block:**
   ```rust
   const GLOW_ENABLED: bool = MOUSE_GLOW_INTENSITY > 0.0;
   if GLOW_ENABLED && ctx.mouse_col != u16::MAX { ... }
   ```
   LLVM should compile out the entire block when `GLOW_ENABLED = false`. This preserves the code for future re-enablement.

3. **Re-enable the glow** by raising `MOUSE_GLOW_INTENSITY` to e.g. `0.15`. The block then does actual visual work. Owner decision.

### 3.2 [MEDIUM] Per-cell `sqrt` in flash wave loop

**Location:** `src/droplet.rs:781`

```rust
let euclidean = (col_dist * col_dist + line_dist * line_dist).sqrt();
```

This is computed **per cell × per wave**. For 200 active cells × 4 active waves × 60 fps = 48,000 sqrts/sec. Each sqrt is ~10-20 cycles on modern x86 (`sqrtss` instruction). Total: ~500K-1M cycles/sec → ~200-500 µs/sec.

**Optimization:** Skip the sqrt for cells clearly outside the wave's reach. The wave's max influence radius is `primary_radius + MOUSE_FLASH_RING_WIDTH` (primary) or `secondary_radius + MOUSE_FLASH_RING_WIDTH` (secondary). Use the larger one as a bounding radius.

```rust
// Per-wave precomputed (add to FlashWaveCtx):
//   max_reach_sq = (max(primary_radius, secondary_radius) + RING_WIDTH).powi(2)

// Per-cell early-out:
let dist_sq = col_dist * col_dist + line_dist * line_dist;
if dist_sq > w.max_reach_sq { continue; }  // skip sqrt + ring math
let euclidean = dist_sq.sqrt();
```

This skips the sqrt for cells far from the wave. For a typical wave at radius 30, the bounding circle has area ~3000 cells². On a 200×60 = 12000-cell screen, ~75% of cells are outside → sqrt skipped for 75% of cells.

**Estimated savings:** ~75% of 48K sqrts = 36K sqrts/sec saved → ~150-300 µs/sec.

### 3.3 [MEDIUM] Wave-invariant quantities recomputed per cell

**Location:** `src/droplet.rs:785-807`

```rust
let raw_fade = (1.0 - elapsed / MOUSE_FLASH_DURATION_SECS).max(0.0);  // wave-only
let fade = raw_fade * raw_fade.sqrt();                                 // wave-only (sqrt!)
let primary_radius = elapsed * MOUSE_FLASH_SPEED;                      // wave-only
let secondary_radius = elapsed * MOUSE_FLASH_SPEED * MOUSE_FLASH_SECONDARY_SPEED_FRAC;  // wave-only
```

These four quantities depend only on `elapsed` (a wave property). They are recomputed for every cell of every droplet that the wave touches.

**Per-cell cost:** 1 div + 1 sub + 1 max + 1 sqrt + 2 mul = ~6 ops + 1 sqrt.

**Fix:** Extend `FlashWaveCtx` with precomputed fields:

```rust
pub(crate) struct FlashWaveCtx {
    pub col: u16,
    pub line: u16,
    pub primary_radius: f32,      // ← new
    pub secondary_radius: f32,    // ← new
    pub fade: f32,                // ← new (already includes the raw_fade^1.5)
    pub max_reach_sq: f32,        // ← new (for §3.2 early-out)
}
```

Compute once per wave in `rain.rs:583-595`:
```rust
flash_waves_buf.push(FlashWaveCtx {
    col: w.col,
    line: w.line,
    primary_radius: e * MOUSE_FLASH_SPEED,
    secondary_radius: e * MOUSE_FLASH_SPEED * MOUSE_FLASH_SECONDARY_SPEED_FRAC,
    fade: {
        let raw = (1.0 - e / MOUSE_FLASH_DURATION_SECS).max(0.0);
        raw * raw.sqrt()
    },
    max_reach_sq: {
        let max_r = (e * MOUSE_FLASH_SPEED).max(e * MOUSE_FLASH_SPEED * MOUSE_FLASH_SECONDARY_SPEED_FRAC) + MOUSE_FLASH_RING_WIDTH;
        max_r * max_r
    },
});
```

**Estimated savings:** ~6 ops × 200 cells × 4 waves × 60 fps = ~290K ops/sec + ~48K sqrts/sec (the `raw_fade.sqrt()` is now per-wave not per-cell). Combined with §3.2's early-out, total savings ~300-500 µs/sec.

### 3.4 [LOW] Per-frame `Instant::now()` in precompute loop

**Location:** `src/cloud/rain.rs:586`

```rust
let e = w.birth.elapsed().as_secs_f32();
```

`Instant::elapsed()` internally calls `Instant::now()`. With 4 active waves, this is 4 syscalls/frame. Should use the `now` parameter already passed to `rain_at`:

```rust
let e = now.saturating_duration_since(w.birth).as_secs_f32();
```

**Estimated savings:** ~3 syscalls × 20 ns = ~60 ns/frame = ~4 µs/sec. Negligible but free.

### 3.5 [LOW] `vignette_factor` and `rain_shadow_factor` per-cell sqrt/mul

**Location:** `src/droplet.rs:907` (vignette) + `:876` (shadow)

Not strictly mouse-effect code, but in the same per-cell hot path. `vignette_factor` does 1 sqrt + ~6 ops per cell. `rain_shadow_factor` does ~5 ops per cell but only depends on `line` — could be a 1D LUT (like `edge_fade_lut`). Out of scope for this audit but flagged for future optimization.

### 3.6 [LOW] Triple `Instant::now()` per click

**Location:** `src/cloud/mod.rs:445, 447` + `src/cloud/spawn.rs:743`

Three `Instant::now()` calls per click. Could be one. Saves ~40-80 ns/click. Negligible.

---

## 4. Correctness Findings (Ranked by Severity)

### 4.1 [MEDIUM] Quantum particles don't survive pause (BUG)

**Location:** `src/cloud/mod.rs:toggle_pause` BRANCH 2 (lines 562-625)

Flash wave births are shifted by `elapsed` (line 614-618), but quantum particle births are NOT shifted. After a pause > `QUANTUM_RIPPLE_LIFETIME_SECS` (0.8s), all active particles instantly expire on unpause.

**Reproduction:**
1. Click → spawns 20 particles (lifespan 0.8s)
2. Immediately press `p` to pause
3. Wait 1 second
4. Press `p` to unpause
5. **Observed:** particles vanish instantly on unpause
6. **Expected:** particles continue their outward motion for their remaining lifespan

**Fix:** Add to `toggle_pause` BRANCH 2, near line 618:
```rust
for p in &mut self.quantum_particles {
    if p.active {
        p.birth += elapsed;
    }
}
```

**Severity:** MEDIUM — visible inconsistency between flash wave (survives pause) and quantum ripple (doesn't). Both should behave the same way.

### 4.2 [LOW] Mouse position updated on scroll events

**Location:** `src/interactive/event_loop.rs:877`

`cloud.set_mouse_position(m.column, m.row)` is called for ALL `Event::Mouse` events, including `ScrollUp`/`ScrollDown`. Scroll events carry the position where the scroll happened, not the current cursor position. This means a scroll wheel flick at position (col=50, row=10) would move the "cursor glow anchor" to (50, 10) even if the user's mouse is actually at (col=80, row=20).

**Currently invisible** because `MOUSE_GLOW_INTENSITY = 0.0` (see §3.1). If the glow is ever re-enabled, this becomes a visible glitch.

**Fix:**
```rust
if matches!(m.kind, MouseEventKind::Moved | MouseEventKind::Drag(_) | MouseEventKind::Down(_)) {
    cloud.set_mouse_position(m.column, m.row);
}
if matches!(m.kind, MouseEventKind::Down(_)) {
    cloud.set_mouse_click(m.column, m.row);
}
```

### 4.3 [LOW] Mouse position not cleared on focus loss

**Location:** `src/interactive/event_loop.rs:882` (FocusGained) — no FocusLost handler

The `_ => {}` catch-all swallows `Event::FocusLost`. The mouse position persists at its last in-window value, even after the user alt-tabs away. Same caveat as §4.2 — invisible while glow is disabled.

**Fix:** Add a `FocusLost` arm that resets mouse position:
```rust
Event::FocusLost => {
    cloud.set_mouse_position(u16::MAX, u16::MAX);
}
```
(Requires a `clear_mouse_position` method on Cloud, or making `set_mouse_position(u16::MAX, u16::MAX)` the canonical "clear".)

### 4.4 [LOW] Right-click and middle-click spawn flash waves

**Location:** `src/interactive/event_loop.rs:878`

`matches!(m.kind, MouseEventKind::Down(_))` matches all three buttons. Right-click in most terminal apps triggers context menu or paste — spawning a ripple on right-click may be unexpected. Owner decision whether to restrict to `Down(MouseButton::Left)` only.

### 4.5 [LOW] `reset()` does not clear mouse state

**Location:** `src/cloud/spawn.rs:25-153`

After `reset()`, active flash waves and quantum particles continue to expand from their old positions (which may now be off-screen if the terminal was resized smaller). Self-expire within 1.8s / 0.8s. Not a bug per se, but a cleanliness issue. See §2.8.

### 4.6 [INFO] `oldest` initialization is fragile but correct

**Location:** `src/cloud/mod.rs:447`

The `oldest = (0usize, Instant::now())` initialization works because any past `birth` is < `Instant::now()`. But it relies on an implicit invariant. See §2.2 C4 for the cleaner pattern.

### 4.7 [INFO] Stale doc comment in `quantum_constants.rs`

**Location:** `src/quantum_constants.rs:22-28`

Says "32 covers the peak case of 2-3 rapid clicks (each spawns up to 25)" but the actual values are 64 and 20. See §2.3 D1.

---

## 5. Optimization Recommendations (Ranked by ROI)

| # | Optimization | Impact | Effort | ROI |
|---|---|---|---|---|
| 1 | Const-gate or delete the cursor glow block (§3.1) | HIGH (~200 µs/sec + cleaner code) | LOW (5 min) | **VERY HIGH** |
| 2 | Precompute wave-invariant quantities in `FlashWaveCtx` (§3.3) | MEDIUM (~300 µs/sec) | LOW (15 min) | **HIGH** |
| 3 | Add squared-distance early-out before per-cell sqrt (§3.2) | MEDIUM (~200 µs/sec) | LOW (10 min) | **HIGH** |
| 4 | Fix quantum particle pause-survival bug (§4.1) | CORRECTNESS | LOW (5 min) | **HIGH** |
| 5 | Use `now.saturating_duration_since` instead of `elapsed()` in precompute (§3.4) | LOW (~4 µs/sec) | TRIVIAL (1 line) | MEDIUM |
| 6 | Restrict `set_mouse_position` to `Moved/Drag/Down` kinds (§4.2) | LOW (correctness) | TRIVIAL (3 lines) | MEDIUM |
| 7 | Add `FocusLost` handler to clear mouse position (§4.3) | LOW (correctness) | TRIVIAL (3 lines) | MEDIUM |
| 8 | Update stale doc comment in `quantum_constants.rs` (§4.7) | NONE (doc only) | TRIVIAL (1 line) | LOW |
| 9 | Replace `if/else` abs-diff with `u16::abs_diff` (§2.6 P11) | NEGLIGIBLE | TRIVIAL (4 lines) | LOW |
| 10 | Clear mouse state in `reset()` (§4.5) | LOW (cleanliness) | LOW (5 lines) | LOW |
| 11 | Consolidate triple `Instant::now()` in click path (§3.6) | NEGLIGIBLE | LOW (3 lines) | LOW |
| 12 | Precompute `vignette_factor` and `rain_shadow_factor` LUTs (§3.5) | MEDIUM (~100 µs/sec) | MEDIUM (30 min) | MEDIUM (out of scope) |

---

## 6. Quick Wins (< 30 min, High Impact)

### Quick Win #1: Const-gate the cursor glow block (5 min, HIGH impact)

**File:** `src/droplet.rs:727`

**Change:**
```rust
// BEFORE:
if ctx.mouse_col != u16::MAX {
    // ... 22 lines of dead math ...
}

// AFTER:
const GLOW_ENABLED: bool = MOUSE_GLOW_INTENSITY > 0.0;
if GLOW_ENABLED && ctx.mouse_col != u16::MAX {
    // ... same math, now only runs if glow is enabled ...
}
```

LLVM will constant-fold `GLOW_ENABLED = false` and eliminate the entire block at compile time. Zero runtime cost when glow is disabled, full behavior preserved for future re-enablement.

### Quick Win #2: Precompute wave quantities in `FlashWaveCtx` (15 min, MEDIUM impact)

**File:** `src/cloud/render.rs:33-41` + `src/cloud/rain.rs:583-595` + `src/droplet.rs:769-816`

**Change:** Add `primary_radius`, `secondary_radius`, `fade`, `max_reach_sq` to `FlashWaveCtx`. Compute once in rain.rs precompute loop. Use directly in droplet.rs hot loop.

This eliminates 1 sqrt + ~5 mul/div per cell per wave. Combined with Quick Win #3, eliminates ~75% of the per-cell flash wave math.

### Quick Win #3: Squared-distance early-out (10 min, MEDIUM impact)

**File:** `src/droplet.rs:769-816`

**Change:** Before computing `euclidean = sqrt(...)`, check `dist_sq > w.max_reach_sq` and `continue` if true. This skips the sqrt and the ring math for cells outside the wave's bounding circle.

Depends on Quick Win #2 for the `max_reach_sq` field (or compute it inline).

### Quick Win #4: Fix quantum particle pause-survival bug (5 min, CORRECTNESS)

**File:** `src/cloud/mod.rs:613-618`

**Change:** Add after the flash wave birth shift:
```rust
for p in &mut self.quantum_particles {
    if p.active {
        p.birth += elapsed;
    }
}
```

This makes quantum particles survive pauses consistently with flash waves.

### Quick Win #5: Restrict mouse position updates to actual motion events (3 min, LOW correctness)

**File:** `src/interactive/event_loop.rs:877`

**Change:**
```rust
// BEFORE:
cloud.set_mouse_position(m.column, m.row);

// AFTER:
if matches!(m.kind, MouseEventKind::Moved | MouseEventKind::Drag(_) | MouseEventKind::Down(_)) {
    cloud.set_mouse_position(m.column, m.row);
}
```

---

## 7. Fix Verification

The fix replaced a single `flash_time: Option<Instant>` slot with a bounded pool of `MOUSE_FLASH_POOL_SIZE = 4` slots. Verified:

- ✅ **Single click activates one slot** (`flash_wave_pool_single_click_activates_one_slot`, tests_quantum.rs:663)
- ✅ **Double-click keeps both waves** (`flash_wave_pool_double_click_keeps_both_waves`, tests_quantum.rs:684) — the original bug scenario
- ✅ **Pool overflow evicts oldest** (`flash_wave_pool_overflow_evicts_oldest`, tests_quantum.rs:712)
- ✅ **Pool size constant is in [2, 8]** (`flash_wave_pool_size_constant_is_reasonable`, tests_quantum.rs:756)
- ✅ **Pause shifts all active wave births** (cloud/mod.rs:613-618) — waves survive pauses correctly
- ✅ **Expiry sweep runs per-frame** (cloud/rain.rs:939-945) — expired waves are deactivated
- ✅ **Precompute skips expired waves** (cloud/rain.rs:587) — renderer never sees stale waves on the frame before sweep

**Edge cases NOT covered by tests:**

- ⚠️ **Same-instant clicks** (sub-microsecond): two clicks with identical `birth` values. The eviction policy picks index 0 (the `i == 0` short-circuit). On platforms with coarse `Instant` resolution (older Windows), this could cause the same slot to be repeatedly evicted under sustained click storms. Not a regression vs. the previous design (which always overwrote the single slot), but worth noting.

- ⚠️ **Click during pause deceleration**: `set_mouse_click` is callable during the pause deceleration ramp (`pause_start.is_some()` but `pause == false`). The new wave's `birth` is set to `Instant::now()`. If the user then fully pauses (deceleration completes), the wave's `birth` is correctly shifted on unpause. No bug — just an unusual interaction worth being aware of.

- ⚠️ **Click at terminal edge**: `col == 0` or `col == cols-1`. The wave expands from the edge, but only the inward half is visible (the outward half is off-screen). Not a bug — expected behavior.

**Verdict:** The fix is **solid** for the documented use case (2-3 rapid clicks within 1.8s). The pool size of 4 is adequate for normal clicking. Click storms (>5 cps) will cause evictions, but that's by design (the oldest wave gets evicted, which is the least-bad choice).

---

## 8. Summary

The mouse effects system is well-designed and the fix is correctly implemented. The main optimization opportunities are:

1. **Eliminate dead code** (cursor glow block with `MOUSE_GLOW_INTENSITY = 0.0`) — high ROI, 5-minute fix.
2. **Precompute wave-invariant quantities** in `FlashWaveCtx` — medium ROI, 15-minute fix.
3. **Squared-distance early-out** before per-cell sqrt — medium ROI, 10-minute fix.

The main correctness bug is:

1. **Quantum particles don't survive pause** — `p.birth` not shifted in `toggle_pause`, unlike `flash_waves[i].birth`. 5-minute fix.

All quick wins combined: ~40 minutes of work for ~500-800 µs/sec of CPU savings + 1 correctness fix + 2 minor robustness improvements.
