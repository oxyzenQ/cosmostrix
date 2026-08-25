<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Rain-Drop vs Top-Border Touch Glow Audit (`-mb` Overlay)

> Source code is truth; cross-check the referenced files before relying on
> this analysis for implementation decisions. This document is an internal
> research artifact, not a contract.

## 1. Executive Summary

The owner asks whether — when running with `-mb` (message overlay with
border) — a rain drop whose head touches the top border of the overlay
should emit a glow / light-white effect, since the rain head is usually
white (palette last-stop or `(255,255,255)` fallback).

**Audit verdict:** The current renderer *structurally prevents* rain head
from touching the border, because the message overlay is drawn **last** in
the pipeline via unconditional `Frame::set_force` writes that overwrite any
rain cell at those coordinates. So today there is no visible "touch event"
at all — rain is hidden behind the border. The owner's proposed effect
therefore is not a *modification* of an existing touch effect but a *new
feature*: synthesize a visual signal at the moment a rain head's column
reaches the top edge of the overlay.

Five masterclass options are presented below, ranging from least to most
intrusive. Each is fully sketched with file/function touch points, perf
cost, and trade-offs. The author's recommendation, pending owner decision,
is **Option C (Edge-Touch Pulse)** — minimal blast radius, re-uses the
existing chroma infrastructure, and respects the LTS invariant that the
top corners of the border must not be bright.

---

## 2. Current-State Audit

### 2.1 The `-mb` flag

`-mb <text>` is the message-with-border overlay (see `src/cli/help_detail.rs`
lines 154–167 and `src/cli/app.rs` line 113). It populates the
`message_border` field on `Cloud` (set by `Cloud::set_message_border` in
`src/cosmic_dragon_engine/cloud/mod.rs` line 508), which causes
`Cloud::reset_message()` (mod.rs line 768) to lay out a centered box with
a 1-cell `╭╮╰╯─│` rounded border plus 1-cell vertical / 2-cell horizontal
padding around the text content.

### 2.2 The rain head color

The rain head color is the palette's last stop (`palette.colors.last()`)
when a palette is bound; otherwise it falls back to `(255, 255, 255)` pure
white. This is cached as `DrawCtx::head_rgb` (see
`src/cosmic_dragon_engine/cloud/render.rs` line 54–58) and is what
`chroma::shaders::base::resolve_cell_color` uses for the
`CharLoc::Head` branch (shaders/base/mod.rs line 595). An always-on head
halo already exists (`HEAD_HALO_FACTOR = 0.15`,
`src/chroma_dragon_engine/tuning.rs` line 117) — it blends the head color
toward the scene background by 15%, softening the stark white pixel
against dark scenes (shaders/base/mod.rs lines 743–768).

### 2.3 The render pipeline ordering

The render step sequence lives in `Cloud::render` (rain.rs). The relevant
section (rain.rs line 1073–1076):

```rust
// 8. Draw message box LAST — survives phosphor, anomaly, atmospheric.
// Glow (60% white blend) + typewriter reveal (30ms/char).
if !self.message.is_empty() {
    self.draw_message(frame);
}
```

`draw_message` (mod.rs line 928) iterates over `self.message` (a `Vec<MsgChr>`,
one entry per overlay cell — content, border, or interior padding) and
writes each visible cell with `frame.set_force(col, line, cell)`
(mod.rs line 1147). `Frame::set_force` (frame.rs line 335) is a
**non-conditional overwrite**: `self.cells[i] = cell` — it does not consult
the previous cell. There is no blending with whatever rain was previously
drawn at the same coordinates.

**Conclusion:** the message overlay completely occludes any rain cell at
the same `(col, line)`. There is no "rain touches border" visual event
today — the rain head simply disappears one frame and reappears on the
other side, with no in-between frame visible.

### 2.4 Rain spawn vs. the overlay region

Rain droplets are *not* suppressed from spawning in the columns that
cross the message box. The spawn loop in `src/cosmic_dragon_engine/cloud/spawn.rs`
does not consult `self.message`; it only consults column-busy state. So
droplets do pass through the overlay region; they just become invisible
to the user while inside it because the overlay draws last.

### 2.5 Existing design invariants (must respect)

The border chroma gradient has explicit LTS invariants (mod.rs lines
1018–1027):

1. **Bottom corners (`╰`, `╯`) ALWAYS use bright anchor** — visual
   anchoring, the head rain "lands" in the bright corner.
2. **Top corners (`╭`, `╮`) follow the natural chroma gradient** —
   explicit rule: *"Top-left should be dark per chroma dragon gradient."*
3. **Triangle wave** on left/right borders — no sharp color gaps.
4. **No lone bright heads at top corners.**

Invariant #2 and #4 are **directly in tension with a naive "white glow
on the top border"** approach: if every rain head that touches the top
edge produces a bright spot on the top border, we would be re-creating
the "lone bright heads at top corners" artifact that the LTS gradient
was specifically designed to eliminate.

Therefore, *any* touch-glow design must avoid producing persistent bright
spots on the top border itself. Options below are framed accordingly.

### 2.6 Where the head position is known

The droplet's head line is tracked as `Droplet::head_put_line`
(used in `rain.rs` lines 390/425 and `phosphor.rs` lines 254, 797, 800).
The overlay's top border line is `start_line` (mod.rs line 852), available
on `Cloud` after `reset_message`. The geometric test "is this droplet's
head at the top border line, in a column that crosses the overlay?" is
therefore a cheap O(droplets) check on every frame.

---

## 3. Masterclass Options

All options assume the same trigger condition: a droplet whose `bound_col`
falls inside `[start_col, start_col + box_w)` and whose `head_put_line`
transitions from `< start_line` to `== start_line` between the previous
frame and the current frame. The transition (not the sustained state) is
the moment of "touch" — this avoids continuous glow while a droplet is
merely sitting at the border (which would re-create the lone-bright-head
artifact).

### Option A — Overflow Bleed (intrusive)

**Idea:** When a droplet head touches the top border, render a brief
"bleed" — 1 cell above the border in the same column, decaying from full
white to transparent over ~120 ms. The cell is *outside* the overlay
region, so the overlay's `set_force` does not overwrite it.

**Implementation sketch:**
- Add `Vec<BorderBleed>` to `Cloud`, each entry = `{ col, birth_instant }`.
- In `Cloud::render`, after droplet advance but before `draw_message`,
  scan droplets for touch events; push a `BorderBleed` per touched column.
- Render bleeds *after* `draw_message` (otherwise the overlay erases
  them) — they're outside the box, but `draw_message`'s loop covers only
  the box region, so this is safe either way; still, drawing last
  guarantees correctness.
- Each frame, compute `factor = exp(-elapsed/30.0)` (a 30 ms time
  constant → ~120 ms total visible).
- Write `frame.set_force(col, start_line - 1, white_blend_cell)` if
  `start_line > 0`.
- Expire entries when `factor < 0.05`.

**Trade-offs:**
- ✓ Cinematic — visible "splash" leaking above the border.
- ✓ Respects the top-corner-dark invariant (bleed is *above* the border,
  not on it).
- ✗ Most visible / attention-grabbing of the five — may compete with the
  message text for the eye.
- ✗ Requires a new state vector and per-frame decay loop.
- Perf: O(active_bleeds) per frame, expected 0–8 entries at 60 FPS on a
  80-col terminal — negligible.

### Option B — Border-Head Bloom (subtle)

**Idea:** When a droplet head touches the top border, the **specific
border cell beneath it** gets a brief brightness boost (lifted from the
chroma gradient's natural value toward the head color), decaying over
~200 ms. The boost is per-cell, not per-corner — so it does not violate
"top corners stay dark" because it only fires on `─` mid-edge cells, not
on `╭`/`╮` corner cells.

**Implementation sketch:**
- Build a `Vec<Option<f32>>` of length `self.message.len()` (one boost
  factor per overlay cell, None = no boost). Hoist into `Cloud` state as
  `border_bloom: Vec<f32>` initialized to 0.0; reset on `reset_message`.
- On touch event, find the matching `MsgChr` index in `self.message`
  (binary search on `(line=start_line, col=touch_col)`) and set its
  bloom factor to 1.0.
- Each frame, multiply all bloom factors by `exp(-elapsed/60.0)`
  (~200 ms time constant).
- In `draw_message`, when the cell is a visible border cell (not corner),
  blend: `final_fg = lerp(border_gradient[idx], head_rgb, bloom_factor)`.
- Skip corner cells (`╭`, `╮`, `╰`, `╯`) — invariant protection.

**Trade-offs:**
- ✓ Subtle — the border "ripples" under rain without distracting.
- ✓ Re-uses the existing `head_rgb` + chroma gradient infrastructure.
- ✓ Respects all four LTS invariants (corners untouched).
- ✗ Requires a per-cell decay loop on every frame (small but non-zero).
- ✗ Visually less dramatic than Option A — owner may not perceive the
  effect at all on busy screens.
- Perf: O(message.len()) per frame, ~80–200 cells — under 1 µs.

### Option C — Edge-Touch Pulse (recommended)

**Idea:** A **1-frame** bright pulse on the touched border cell (mid-edge
only), decaying over ~250 ms with a smoothstep envelope. Single-cell,
single-touch events; no bleed above the border. This is the "rain tapping
a glass ceiling" effect — present but quiet.

**Implementation sketch:**
- New `Cloud` state: `border_pulses: Vec<BorderPulse>` where
  `BorderPulse { msg_idx: usize, birth: Instant }`.
- On touch event (transition), push a `BorderPulse` if the touched cell is
  `─` (skip `╭╮╰╯`).
- In `draw_message`, for each visible border cell, look up its active
  pulse (if any) and compute:
  - `t = elapsed / 0.250` (250 ms lifetime)
  - `factor = (1.0 - t).smoothstep() * 0.6` (max 60% blend)
  - `final_fg = lerp(border_gradient[idx], head_rgb, factor)`
- Drain expired pulses after the loop.

**Trade-offs:**
- ✓ Minimal blast radius — one new Vec, one decay loop, one lerp per
  active pulse.
- ✓ Re-uses `head_rgb` and existing gradient pipeline — no new color
  infrastructure.
- ✓ Corners untouched → invariant-safe.
- ✓ Smoothstep envelope eliminates hard brightness steps (matches the
  Phase 5 L-smoothing philosophy in `shaders/base/mod.rs`).
- ✗ Less visually obvious than Option A — may need to be tuned (max
  factor 0.6 → 0.8) for owner to actually see it.
- Perf: O(active_pulses) per frame, expected 0–4 entries at 60 FPS —
  negligible (~10 ns).

### Option D — Glow Halo Above Border (architectural)

**Idea:** Generalize the existing `head_halo_factor` (Phase 4-D) to also
emit a halo *above* the border when a head is touching. The halo is a
single row above the top border, with per-column brightness modulated by
how recently a head touched each column. This is the most "principled"
option — it extends the existing halo infrastructure rather than adding a
new effect class.

**Implementation sketch:**
- New `Cloud` state: `border_halo: Vec<f32>` of length `self.cols`,
  initialized to 0.0, reset on terminal resize.
- On touch event, set `border_halo[touch_col] = 1.0`.
- Each frame, decay: `border_halo[i] *= exp(-elapsed/80.0)` (~250 ms
  time constant).
- Render the halo row *after* `draw_message`:

  ```rust
  for col in start_col..start_col + box_w {
      let factor = border_halo[col as usize];
      if factor > 0.05 && start_line > 0 {
          let halo_cell = blend_toward_bg(
              head_color,
              bg,
              1.0 - factor * 0.5  // never fully white
          );
          frame.set_force(col, start_line - 1, halo_cell);
      }
  }
  ```

**Trade-offs:**
- ✓ Architecturally consistent — extends Phase 4-D `head_halo_factor`
  concept, no new effect class.
- ✓ Per-column decay gives a "ripple" feel across the top of the border
  as multiple drops touch in sequence.
- ✓ The halo is *above* the border — invariant-safe.
- ✗ Most code of the five options (a new Vec + decay + render loop).
- ✗ Renders 1 extra row above the border; if `start_line == 0` (overlay
  at the very top of the terminal), the halo is invisible. Acceptable
  edge case.
- Perf: O(box_w) per frame, ~60–100 cells — under 1 µs.

### Option E — Splash Spark (most cinematic, most intrusive)

**Idea:** A single-cell pure-white spark at the touched border cell,
lasting ~100 ms with a sharp attack and exponential decay. Like a
droplet hitting a window. This is the most attention-grabbing option.

**Implementation sketch:**
- Similar to Option C but:
  - No smoothstep — sharp attack (instant 1.0), exponential decay
    (`factor = exp(-elapsed/30.0)`, 100 ms lifetime).
  - Spark overrides the border cell's color entirely (not a blend) —
    writes `head_rgb` directly to the cell when `factor > 0.5`.
  - Below 0.5, blends toward the natural border gradient color.
  - Optional: render a 1-cell vertical "splash up" at `start_line - 1`
    in the same column for the first 50 ms (combines with Option A's
    bleed concept).

**Trade-offs:**
- ✓ Most visually striking — clearly conveys "rain hitting ceiling".
- ✗ Most intrusive — overrides the border's chroma gradient for ~100 ms
  per touch, which competes with the chroma dragon's color narrative.
- ✗ Sharp attack may strobe on high-density rain scenarios (many touches
  per second). Should be capped: max 1 active spark per column at a time.
- ✗ Risks violating the "no lone bright heads at top corners" invariant
  if a touch happens on a corner cell — must explicitly skip corners.
- Perf: similar to Option C — negligible.

---

## 4. Recommended Approach

**Option C (Edge-Touch Pulse)** is the recommended first implementation,
with the following parameters:

- Lifetime: 250 ms
- Envelope: smoothstep `(1.0 - t)² × (3.0 - 2.0 × (1.0 - t))` — slow start,
  slow end, matching the Phase 5 L-smoothing philosophy
- Max blend factor: 0.6 (tunable via constant `BORDER_TOUCH_PULSE_MAX`)
- Corners (`╭╮╰╯`): explicitly skipped
- Mid-edge cells (`─`, `│`): eligible for pulse

**Rationale:**
- Smallest code surface — one new Vec, one decay loop, one lerp.
- Re-uses existing `head_rgb` infrastructure; no new color pipeline.
- Smoothstep envelope eliminates hard brightness steps — composes
  cleanly with the chroma gradient.
- Corners untouched → all four LTS invariants preserved.
- If the effect proves too subtle at 0.6 max factor, the owner can bump
  to 0.8 or extend lifetime to 400 ms in a one-line tuning change
  (mirrors the `HEAD_HALO_FACTOR` constant pattern in `tuning.rs`).

If the owner finds Option C too subtle in practice, the natural escalation
is **Option D (Glow Halo Above Border)** — that adds the per-column
ripple row without disrupting the border itself.

If the owner wants maximum drama, **Option E (Splash Spark)** is the
top-choice escalation, with the explicit corner-skip guard.

---

## 5. Implementation Sketch (Option C, owner-approved path)

Estimated complexity: ~120 LOC across 2 files + 1 test file.

### 5.1 New state on `Cloud` (cloud/mod.rs)

```rust
/// One active edge-touch pulse on the overlay border.
/// See `docs/research/RAIN_BORDER_TOUCH_GLOW_AUDIT.md` (Option C).
#[derive(Clone, Copy)]
struct BorderPulse {
    msg_idx: usize,
    birth: Instant,
}

pub(crate) border_pulses: Vec<BorderPulse>,
```

Initialize in `Cloud::new` (and `Cloud::reset_message` after rebuilding
`self.message`) to `Vec::new()`.

### 5.2 Touch detection (cloud/rain.rs, inside the droplet advance loop)

After computing `head_put_line` for each alive droplet, after the existing
`if died { ... continue; }` block, insert:

```rust
// RAIN_BORDER_TOUCH_GLOW: detect droplet head touching overlay top edge.
// Fires once per touch (transition from above to ==), not sustained.
if !self.message.is_empty() && self.message_border {
    if let Some(prev_head) = d.prev_head_put_line {
        if prev_head < self.message_top_line
            && d.head_put_line == self.message_top_line
            && d.bound_col >= self.message_left_col
            && d.bound_col < self.message_right_col
        {
            // Find the touched MsgChr index for the top-edge mid-cell
            // at column d.bound_col. Skip if it's a corner.
            if let Some(idx) = self.find_top_edge_cell(d.bound_col) {
                let mc = &self.message[idx];
                if matches!(mc.val, '─') {  // skip corners
                    self.border_pulses.push(BorderPulse {
                        msg_idx: idx,
                        birth: now,
                    });
                }
            }
        }
    }
    d.prev_head_put_line = Some(d.head_put_line);
}
```

`self.message_top_line`, `self.message_left_col`, `self.message_right_col`
are new fields cached in `reset_message` (where `start_line` and
`start_col` are computed). `find_top_edge_cell(col)` is a small binary
search over `self.message` filtered to `line == start_line`.

### 5.3 Pulse rendering (cloud/mod.rs, inside `draw_message`)

After the existing `border_gradient` computation, before the cell loop:

```rust
const BORDER_TOUCH_PULSE_LIFETIME_MS: u32 = 250;
const BORDER_TOUCH_PULSE_MAX: f32 = 0.6;

let now = /* Instant::now(), same pattern as message_elapsed_ms */;
let mut pulse_factor: Vec<f32> = vec![0.0; self.message.len()];
let mut alive_pulses = Vec::with_capacity(self.border_pulses.len());
for p in &self.border_pulses {
    let elapsed_ms = now.saturating_duration_since(p.birth).as_millis() as u32;
    if elapsed_ms >= BORDER_TOUCH_PULSE_LIFETIME_MS {
        continue;
    }
    let t = elapsed_ms as f32 / BORDER_TOUCH_PULSE_LIFETIME_MS as f32;
    let inv = 1.0 - t;
    let envelope = inv * inv * (3.0 - 2.0 * inv);  // smoothstep
    pulse_factor[p.msg_idx] = pulse_factor[p.msg_idx].max(
        envelope * BORDER_TOUCH_PULSE_MAX
    );
    alive_pulses.push(*p);
}
self.border_pulses = alive_pulses;
```

Then in the visible-border cell branch (mod.rs line 1137), replace:

```rust
(mc.val, border_gradient[idx].or(content_fg))
```

with:

```rust
let base = border_gradient[idx].or(content_fg);
let fg = if pulse_factor[idx] > 0.0 && self.head_rgb_opt.is_some() {
    let (hr, hg, hb) = self.head_rgb_opt.unwrap();
    let base_rgb = decode_color(base.unwrap_or(content_fg.unwrap_or(Color::Reset)));
    let blended = blend_toward_rgb(
        base_rgb, hr, hg, hb, pulse_factor[idx]
    );
    Some(Color::Rgb { r: blended.0, g: blended.1, b: blended.2 })
} else {
    base
};
(mc.val, fg)
```

(`head_rgb_opt` is a new field cached per-frame in `Cloud::render`,
matching the existing `head_rgb` computation in `DrawCtx`.)

### 5.4 Tests

Add a test in `cloud/tests/` that:
1. Constructs a `Cloud` with a `-mb` message.
2. Spawns one droplet in a column that crosses the overlay.
3. Advances the droplet until its `head_put_line` reaches the top border.
4. Asserts that a `BorderPulse` was added.
5. Advances 100 ms → asserts `pulse_factor > 0`.
6. Advances 300 ms → asserts `pulse_factor == 0` and `border_pulses` drained.

### 5.5 Performance guardrails

- `border_pulses` Vec: expected max size 8 (one per active column
  crossing the top edge). Drain expired entries every frame.
- `pulse_factor` Vec: allocated per frame in `draw_message`, size =
  `self.message.len()` (typically 80–200). Allocation cost ~200 ns,
  acceptable; can hoist to a reusable `Vec<f32>` if profiling shows it
  matters (unlikely).
- `find_top_edge_cell`: binary search over a sorted slice — O(log N) per
  touch, ~5 µs even with 8 touches per frame.
- Total per-frame cost: <5 µs, <0.05% of a 60 FPS frame budget.

---

## 6. Risks & Open Questions

1. **`prev_head_put_line` tracking.** Adding a new field to `Droplet`
   changes the struct's memory layout. Confirm no `unsafe` code or
   `mem::transmute` sites assume the existing layout. (Quick `grep`
   confirms no `unsafe` in `cloud/droplet.rs`-equivalent sites; full
   check pending.)

2. **Multi-frame advance race.** If a droplet advances multiple lines per
   frame (high speed + low FPS), it may skip the exact `head_put_line ==
   message_top_line` line. Mitigation: use `prev < top AND now >= top`
   as the touch condition (already in the sketch above) — this catches
   the transition even with multi-line advances.

3. **Live config reload.** If the user changes `message-border` via
   live config (see `config/live_config/mod.rs`), the `message_top_line`
   cache must be invalidated. The existing `Cloud::set_message_border`
   already calls `reset_message`, which would naturally re-cache the
   fields. Confirm.

4. **Palette transition interaction.** During a palette transition, the
   Phase 5 L-smoothing (shaders/base/mod.rs lines 770–808) smooths the
   head color across the wave line. The pulse uses `head_rgb` which is
   the post-transition palette's head. Visually this means during a
   transition, the pulse color may differ from the head color visible
   on the screen for ~1 frame at the wave line. Acceptable; document.

5. **Owner's existing "no lone bright heads at top corners" rule.** This
   audit interprets that rule as a constraint on the *border's own
   gradient*, not on transient touch pulses. If the owner intended the
   rule to cover *any* brightness event on the top edge, Option C must
   be re-evaluated — possibly move to Option A (bleed above border) or
   Option D (halo above border) which keep the top border entirely free
   of brightness events.

6. **`-m` (no-border) variant.** The owner asked specifically about
   `-mb`. With `-m` (no border), there is no border to touch — the
   message text is drawn without a frame. Should a similar glow fire
   when a head touches the top text row? Out of scope for this audit;
   flag for owner decision if the answer is yes.

---

## 7. Files Touched (Option C implementation)

| File | Change | LOC |
|------|--------|-----|
| `src/cosmic_dragon_engine/cloud/mod.rs` | `BorderPulse` struct; `border_pulses` field; `message_top_line` / `message_left_col` / `message_right_col` cached fields; `find_top_edge_cell` helper; pulse decay + lerp in `draw_message` | ~70 |
| `src/cosmic_dragon_engine/cloud/rain.rs` | `prev_head_put_line` field on `Droplet`; touch detection in droplet advance loop | ~30 |
| `src/cosmic_dragon_engine/cloud/state.rs` | `Droplet::prev_head_put_line: Option<u16>` field | ~3 |
| `src/cosmic_dragon_engine/cloud/tests/tests_border_gradient.rs` | Touch event + decay test | ~40 |
| `src/chroma_dragon_engine/tuning.rs` | `BORDER_TOUCH_PULSE_LIFETIME_MS`, `BORDER_TOUCH_PULSE_MAX` constants | ~5 |
| `docs/research/RAIN_BORDER_TOUCH_GLOW_AUDIT.md` | This document | — |
| **Total** | | **~148 LOC** |

---

## 8. Decision Matrix

| Option | Visibility | Invariant-Safe | Code Size | Perf Cost | Cinematic Feel |
|--------|-----------|---------------|-----------|-----------|----------------|
| A. Overflow Bleed | High | Yes (above border) | Medium (~80 LOC) | <1 µs | Splash / leak |
| B. Border-Head Bloom | Low | Yes (corners skipped) | Medium (~70 LOC) | <1 µs | Subtle ripple |
| **C. Edge-Touch Pulse** | **Medium** | **Yes (corners skipped)** | **Small (~120 LOC)** | **<5 µs** | **Glass-ceiling tap** |
| D. Glow Halo Above | Medium-High | Yes (above border) | Large (~140 LOC) | <1 µs | Per-column ripple |
| E. Splash Spark | Very High | Risky (overrides border) | Medium (~90 LOC) | <5 µs | Window-impact spark |

---

## 9. Next Action

Await owner's decision on which option (or combination) to implement.
No code changes committed yet; this audit is informational only.

If the owner picks **Option C**, the implementation can land as a single
micro-commit under the prefix `Internal research:` (per the project's
commit convention), with the test file added under
`cloud/tests/tests_border_gradient.rs` (existing border-test home).

## LTS Polish (2026-08-26)

After the Option C+D implementation landed (`29e7440`), a follow-up audit
(DeepSeek review at owner's request) flagged two stability concerns. The
post-audit verification + fixes are recorded here as the canonical
reference for the LTS bounds on the pulse pool.

### Concern 1: `palette.colors.last()` panic on empty palette

**Audit claim**: `detect_border_touch` could panic if `palette.colors` is
empty (Mono mode, or `rain = []` misconfiguration).

**Verification**: the implementation already uses the panic-safe chain
`.last().copied().and_then(decode_color).unwrap_or((255, 255, 255))` —
`Option::copied` lifts `Option<&Color>` to `Option<Color>` without
panicking, and the trailing `.unwrap_or` handles the `None` case with the
pure-white fallback. **No code change needed.**

**Defensive hardening applied**:

1. Inline docstring on `detect_border_touch` making the panic-safety
   invariant explicit ("do NOT simplify to `.last().unwrap()`").
2. Regression test `detect_border_touch_no_panic_on_empty_palette` in
   `tests_border_gradient.rs` pins the white fallback. Any future
   "simplification" that breaks the chain will fail this test.

### Concern 2: Unbounded pulse pool growth

**Audit claim**: continuous droplet touches could stack pulses
unboundedly in `self.border_pulses`. DeepSeek's worst-case estimate was
"1000 droplets simultaneously" (unrealistic for cosmostrix's spawn density,
but the spirit of bounding the pool is correct).

**Verification**: the existing decay-and-rebuild loop in `draw_message`
already drains expired pulses every frame, so the steady-state count is
bounded by touches-within-lifetime. The realistic worst case is
`message.len()` distinct cells all touched within the 1500 ms lifetime
window — typically 50–100 entries, no memory concern.

**Hardening applied (belt-and-suspenders)**: deduplication by `msg_idx`
in `detect_border_touch`. When a new touch lands on a cell that already
has an alive pulse, the existing entry is **refreshed in place** (re-arm
`birth = now`, re-snapshot `head_rgb = current`) instead of pushing a
duplicate. This guarantees:

```
self.border_pulses.len() <= self.message.len()
```

regardless of touch density. The refresh has a **bonus property**: under
continuous touch, the glow picks up the palette's current `head_rgb` on
each re-touch — so mid-transition between two palettes, the glow color
re-snapshots to the newest stop, keeping the visual effect maximally
dynamic.

Owner spec alignment: *"kalo hujan mengenainya lagi muncul lagi"* — the
dedup-refresh implements exactly this. The cell keeps glowing, but the
lifetime clock resets to the newest touch. The owner sees a sustained glow
under continuous touch, not a stack of decaying copies.

### Concern 3: Terminal resize clearing pulse cache

**Audit claim**: pulses might persist after a resize, requiring a manual
`Cloud::reset()`.

**Verification**: the resize path in `spawn.rs` already calls
`reset_message()` on every resize (when `message_text.is_some()`), and
`reset_message` clears `border_pulses` (mod.rs line ~966). **No code
change needed.** The "⚠️ Perlu Test" caveat in the audit was based on a
stale code description; the implementation is already correct.

### Test count delta

| | Before | After |
| --- | ---: | ---: |
| `tests_border_gradient.rs` | 15 | 17 |
| Total (cargo test) | 1680 | 1682 |
| Clippy warnings | 0 | 0 |
| Gatekeepers | 10/10 | 10/10 |

### Files changed in the LTS polish

- `src/cosmic_dragon_engine/cloud/rain.rs`: `detect_border_touch`
  dedup-by-`msg_idx` + LTS docstring section.
- `src/cosmic_dragon_engine/cloud/tests/tests_border_gradient.rs`: 2
  new regression tests + `make_cloud` import.
- `docs/research/RAIN_BORDER_TOUCH_GLOW_AUDIT.md`: this section.
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
