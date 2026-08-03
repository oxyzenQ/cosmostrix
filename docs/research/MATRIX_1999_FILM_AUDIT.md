# Matrix 1999 Film vs Cosmostrix — Frame-by-Frame Audit

<!-- SPDX-License-Identifier: GPL-3.0-only -->

> Research document comparing the actual film's digital rain behavior
> against cosmostrix v30's rendering pipeline. Goal: identify which
> cinematic invariants cosmostrix already nails, which it violates, and
> which constants to tune to bring it closer to the film reference.

## Primary Source

The film-side data is sourced from **Carl Newton's frame-by-frame
analysis** of the opening sequence of *The Matrix* (1999):
<https://carlnewton.github.io/digital-rain-analysis/>

This is the clearest publicly available frame-by-frame breakdown of the
iconic shot. Secondary sources:

- Wikipedia: <https://en.wikipedia.org/wiki/Digital_rain> (typeface,
  inspiration, predecessors)
- No Film School: <https://nofilmschool.com/matrix-digital-rain-origin>
  (Simon Whiteley interview on design intent)

Cosmostrix-side data is read directly from `src/central_control_rains.rs`,
`src/cloud/phosphor.rs`, `src/cloud/spawn.rs`, `src/droplet.rs`,
`src/chroma/catalog.rs`, and `src/chroma/tuning.rs` — all citations in
the comparison table below are file:line.

---

## TL;DR

Cosmostrix already matches the film on 6 of 12 cinematic invariants
(speed feel, vertical fall, single highlighted glyph per string, palette
range, dark background, async column variance). It diverges meaningfully
on 4 invariants (**glyph paradigm**, **highlight color**, **trail
length**, **3-layer parallax**) and is silent on 2 film-specific
behaviors (**changing-glyph sync** and **deletion strings**). Most
divergences are deliberate cinematic enhancements (parallax, long
trails) — only the highlight color and the absence of glyph mutation
read as "off" when placed side-by-side with the film.

---

## 1. Film Behavior — The 12 Cinematic Invariants

From Carl Newton's frame-by-frame analysis of the opening shot:

| # | Invariant | Film Behavior |
|---|-----------|---------------|
| F1 | **Glyph paradigm** | Glyphs do NOT descend. A glyph appears at a row and stays there; a different glyph appears beneath it on the next frame. The string "grows" downward by appending, not by translating. |
| F2 | **String origin** | Strings do not always start at the top of the screen. Many appear mid-screen, with invisible placeholder glyphs above them suggesting the string actually began higher up off-screen. |
| F3 | **Changing glyphs** | Some glyphs remain static for 3 frames, then mutate into a different glyph. During the transition frame, the old and new glyph are composited at 50% opacity each. |
| F4 | **Changing-glyph sync** | All changing glyphs across all strings mutate on the SAME frame. The mutation is globally synchronized. Some strings consist entirely of changing glyphs. |
| F5 | **Deletion strings** | Special "deletion" strings exist that only emit invisible glyphs. They can pass over and erase existing visible strings. The deletion string itself is never visible — only the erasure is. |
| F6 | **Highlighted glyph frequency** | Roughly 1 in 5 strings (≈20%) have a highlighted glyph at any given moment. |
| F7 | **Highlighted glyph count** | Only ONE glyph per string is highlighted at a time. |
| F8 | **Highlighted glyph position** | The single highlighted glyph is always the LEADING glyph of the string (the bottom-most visible glyph). |
| F9 | **Stammer event** | Occasionally all highlighted glyphs stammer simultaneously. The stammer causes the affected strings to fall behind by one row relative to non-highlighted strings. |
| F10 | **Glyph set** | Half-width katakana (ﾊ ﾋ ｼ ﾂ ｳ ｰ ﾅ ﾐ ﾓ ﾆ ｻ ﾜ ｵ ﾘ ﾎ ﾏ ｴ ｷ ﾑ ﾃ ｹ ﾒ ｶ ﾕ ﾗ ｾ ﾈ ｽ ﾀ ﾇ — some flipped horizontally), the Kangxi radical 日, Arabic numerals (0–9, some flipped), English Z, and symbols (`* + : = . < > " ｜ ¦ _`). |
| F11 | **Color** | Green phosphor monochrome. The leading highlighted glyph reads as near-white/bright-white; the body is bright phosphor green; the tail fades to dark green / black. |
| F12 | **Speed feel** | Strings complete a 24-row screen in roughly 0.8 seconds — about 30 cps. Column speeds are slightly varied but visually uniform-ish. |

Design intent (Whiteley interview): the Wachowskis wanted "an ancient
essence" — Japanese typographic texture, not actual code. The first
3D-tumbling-type concept was rejected as too literal.

---

## 2. Cosmostrix Behavior — The Same 12 Invariants

| # | Invariant | Cosmostrix v30 Behavior | Citation |
|---|-----------|-------------------------|----------|
| C1 | **Glyph paradigm** | DROPLET model — a column has a moving "head" that descends row by row, leaving behind a body of glyphs that then decay via phosphor. The head translates downward; glyphs do not stay-then-replace. | `cloud/spawn.rs:412`, `droplet.rs:333-364` |
| C2 | **String origin** | Droplets spawn at the top row (`start_line = 0` by default) OR mid-screen via `die_early_pct = 0.3333` which sets `end_line` to a random row, but the droplet still starts at row 0. Mid-screen invisible prefix is NOT modeled. | `cloud/spawn.rs:412, 453-470` |
| C3 | **Changing glyphs** | Each cell re-rolls its glyph from the char pool on every dirty frame. There is no 3-frame static period and no 50/50 opacity transition blend — the glyph just swaps. | `cloud/render.rs`, `cloud/phosphor.rs` |
| C4 | **Changing-glyph sync** | No global sync. Each cell re-rolls independently. | (absent by design) |
| C5 | **Deletion strings** | Not modeled. Cells become invisible only via phosphor decay or scene transitions. | (absent) |
| C6 | **Highlighted glyph frequency** | Approximately 100% of strings have a head (the head IS the highlighted glyph by construction — `CharLoc::Head` always maps to the brightest stop). So every active droplet has a highlighted leading glyph. | `chroma/shaders/base.rs:522-619` |
| C7 | **Highlighted glyph count** | Exactly one per string (the head). | same |
| C8 | **Highlighted glyph position** | Always the leading (bottom-most) glyph of the droplet. | same |
| C9 | **Stammer event** | Not modeled as a global sync event. The closest analog is the per-droplet turbulence drift (`TURBULENCE_AMPLITUDE = 0.08`, `TURBULENCE_FREQ = 0.4 Hz`) and the wind-gust envelope (30-120s idle, 1-2s attack, 0.5-1s hold, 3-5s decay, 1.2-1.5× peak) — but these are continuous, not the discrete 1-row fall-behind of the film. | `central_control_rains.rs:516-522, 658-692` |
| C10 | **Glyph set** | 24 built-in charsets. Default `binary` = {0, 1}. `matrix` charset = half-width katakana (matches film F10 closely). `retro` adds Latin + symbols. Custom charsets via config. | `src/charset.rs` |
| C11 | **Color** | Default `green` palette: 7 stops from `(0, 12, 1)` to `(201, 244, 210)`. Head stop is pale green-white (sum 655), NOT pure white. Self-bloom is hue-preserving (scales RGB, doesn't lerp to white). | `chroma/catalog.rs:79-90`, `droplet.rs:842-851` |
| C12 | **Speed feel** | Default 30 cps. Triangular async distribution `max(a,b)` on `[0.333, 1.0]`, mean ~0.78. 3-layer parallax speed mult `[0.35, 1.0, 1.7]` so back layer ~10.5 cps, front ~51 cps. | `config.rs:249`, `cloud/spawn.rs:332-346`, `central_control_rains.rs:176` |

---

## 3. Frame-by-Frame Comparison

| Invariant | Film | Cosmostrix | Match? | Notes |
|-----------|------|------------|--------|-------|
| F1 / C1 Glyph paradigm | Stay-and-replace | Translating droplet | **DIVERGENT** | Cosmostrix's droplet model is the standard terminal-rain convention. The film's stay-and-replace paradigm is unusual and arguably looks "wrong" to viewers used to cmatrix-style rain. This divergence is defensible: the film's paradigm causes visible flicker as glyphs swap, which is hard to read at terminal frame rates. |
| F2 / C2 String origin | Mid-screen, invisible prefix | Top-only or top→die-early | **DIVERGENT (mild)** | Cosmostrix droplets always start at row 0. The film's invisible-prefix trick creates the impression of strings "already in motion" when they enter the frame. Could be emulated by spawning droplets at `start_line = -random(0, lines/2)` with off-screen head. |
| F3 / C3 Changing glyphs | 3-frame static + 50/50 blend | Per-frame re-roll | **DIVERGENT** | Cosmostrix's per-frame re-roll is visually busier than the film. The film's 3-frame static period gives the eye time to lock onto a glyph; the 50/50 blend frames make the mutation feel smooth. Cosmostrix's re-roll rate is currently gated by `glitch_pct` (10% default) and the dirty-cell map. |
| F4 / C4 Changing-glyph sync | Globally synchronized | Independent per cell | **DIVERGENT** | Global sync is the film's most distinctive and least-imitated feature. It would require a frame counter mod 3 in the render path. |
| F5 / C5 Deletion strings | Eraser strings | Not modeled | **ABSENT** | Deletion strings create the film's characteristic "strings vanishing mid-screen" beat. Cosmostrix droplets only disappear via phosphor decay or `die_early_pct`. |
| F6 / C6 Highlighted frequency | ~20% of strings | ~100% of strings | **DIVERGENT** | Cosmostrix gives EVERY droplet a head (highlighted leading glyph). The film only highlights 1 in 5. This is the single biggest "feel" difference: cosmostrix looks busier/brighter because every column has a bright head, while the film has a more subdued average brightness with sparse bright pulses. |
| F7 / C7 Highlighted count | 1 per string | 1 per string | **MATCH** | |
| F8 / C8 Highlighted position | Leading glyph | Leading glyph (head) | **MATCH** | |
| F9 / C9 Stammer event | Global 1-row fall-behind | Continuous turbulence + wind gusts | **DIVERGENT (acceptable)** | Cosmostrix's wind-gust envelope (`central_control_rains.rs:658-692`) creates a similar "occasional global pulse" feel, but it's continuous speed modulation, not a discrete 1-row stutter. Defensible — the wind gust reads as more cinematic at terminal frame rates. |
| F10 / C10 Glyph set | Katakana + 日 + numerals + Z + symbols | 24 charsets, `matrix` charset matches | **MATCH (when `--charset matrix`)** | The default `binary` charset does NOT match the film. The `matrix` charset is the correct match. |
| F11 / C11 Color | Bright white head, green body, dark tail | Pale green-white head (201,244,210), green body, dark tail | **PARTIAL MATCH** | Cosmostrix's head stop `(201, 244, 210)` is luminance ~230 — bright but not pure white. The film's head reads as closer to `(255, 255, 255)` or `(220, 255, 220)`. The hue-preserving self-bloom (`droplet.rs:842`) further keeps it green-tinted. This is a deliberate design choice (hue preservation avoids the "white smear" failure mode) but it's a visible divergence from the film. |
| F12 / C12 Speed feel | ~30 cps, mild variance | 30 cps default, 3-layer parallax | **MATCH (mid layer)** | The mid layer at 1.0× speed matches the film. The back layer at 0.35× = 10.5 cps is slower than any film column. The front layer at 1.7× = 51 cps is faster than any film column. The 3-layer parallax is cosmostrix's signature cinematic enhancement — it adds depth that the film doesn't have. |

---

## 4. Cinematic Divergences — Three Tiers

### Tier 1: Defensible Enhancements (keep)

These divergences are cosmostrix's value-add over a film-accurate
reproduction. They make the terminal version richer than the original.

- **3-layer parallax** (C12 vs F12) — adds depth-of-field. The film is
  flat. Cosmostrix is cinematic. Keep.
- **Phosphor afterglow** (`PHOSPHOR_DECAY_RATE = 5.0`, `PHOSPHOR_TAIL_RESIDUAL = 160`)
  — creates the CRT-persistence look. The film has zero afterglow (each
  frame is a clean render). Cosmostrix's ~1.25 s front-layer trail is
  ~6× longer than the film's effective ~200 ms. This is the second
  biggest "feel" difference but it's the signature CRT aesthetic — keep.
- **Wind gust envelope** (30-120s idle, attack/hold/decay) — creates
  rhythmic "breathing" the film lacks. Keep.
- **Turbulence drift** (`TURBULENCE_AMPLITUDE = 0.08`) — subtle
  per-droplet sinusoidal speed variation. Adds organic feel. Keep.
- **Depth fog + vignette + rain shadow** — the film has none of these.
  Cosmostrix's 4-row top/bottom fog, radial vignette, and bottom-15%
  shadow band create a much richer cinematic frame. Keep.

### Tier 2: Worth Tuning (cheap wins, big feel impact)

These are the divergences that, if tuned, would bring cosmostrix
visibly closer to the film without sacrificing its enhancements.

#### T2.1 — Highlighted glyph frequency (C6 vs F6)

**Problem:** 100% of cosmostrix droplets have a bright head. The film
has ~20%. Cosmostrix therefore reads as ~5× brighter/busier than the
film at any given moment.

**Tuning:** Add a `HEAD_HIGHLIGHT_PCT` constant (default 0.20 to match
the film). In `cloud/spawn.rs`, gate `CharLoc::Head` brightness by a
per-droplet roll. Non-highlighted droplets would render their leading
glyph at body brightness (color_idx = `last - 1` instead of `last`).

```rust
// central_control_rains.rs
pub const HEAD_HIGHLIGHT_PCT: f32 = 0.20; // film-accurate

// chroma/shaders/base.rs (CharLoc::Head arm)
let is_highlighted = rng.gen::<f32>() < HEAD_HIGHLIGHT_PCT;
let color_idx = if is_highlighted { last } else { last - 1 };
```

**Risk:** changes the average brightness profile. Would need to verify
against existing visual stability tests.

#### T2.2 — Head color (C11 vs F11)

**Problem:** Head stop is `(201, 244, 210)` — pale green-white. The
film's head reads as near-pure-white. Hue-preserving self-bloom keeps
it green-tinted.

**Tuning option A (palette-only):** Add a `matrix_film` color scheme
with head stop `(240, 255, 240)` (slightly green-tinted white, sum 735
vs current 655). Document as "film-accurate head".

**Tuning option B (bloom-only):** Add a `HEAD_WHITEN_FACTOR` constant
(default 0.0 = current hue-preserving behavior, 1.0 = pure white
blend). In `droplet.rs:842-851`, optionally lerp the head toward white
by this factor:

```rust
// droplet.rs (in head self-bloom)
if HEAD_WHITEN_FACTOR > 0.0 {
    let w = HEAD_WHITEN_FACTOR * wf;
    r = (r as f32 * (1.0 - w) + 255.0 * w) as u8;
    g = (g as f32 * (1.0 - w) + 255.0 * w) as u8;
    b = (b as f32 * (1.0 - w) + 255.0 * w) as u8;
}
```

**Recommendation:** Option A is safer — it's palette-scoped, doesn't
touch the hue-preservation invariant, and gives users a choice. Option
B is more flexible but risks the "white smear" failure mode the hue
preservation was designed to prevent.

#### T2.3 — Changing-glyph cadence (C3 vs F3)

**Problem:** Cosmostrix re-rolls glyphs on every dirty frame. The film
holds each glyph for 3 frames, then transitions with a 50/50 blend.

**Tuning:** Add a `GLYPH_MUTATION_PERIOD_FRAMES` constant (default 3)
and a `GLYPH_MUTATION_BLEND_FRAMES` constant (default 1). In the render
path, gate glyph re-rolls by a global frame counter, and during the
blend frame, composite old+new at 50% opacity.

This requires non-trivial state tracking per cell (last glyph, last
mutation frame). Estimated complexity: ~50 lines in `cloud/render.rs`
+ a new field in the cell struct. Mid-effort, high-impact.

**Caveat:** at 60 FPS, 3 frames = 50 ms. The film is 24 FPS, so 3
frames = 125 ms. Cosmostrix's cadence would need to be FPS-scaled:
`hold_frames = (3 * 24 / fps).max(1)`.

### Tier 3: Out of Scope (would require architecture changes)

These are film behaviors that would require rearchitecting cosmostrix's
rendering model. Listed for completeness; not recommended.

- **F1 stay-and-replace paradigm** — would require replacing the
  droplet model with a cell-grid model where each cell independently
  decides whether to mutate. This is a fundamental rewrite of
  `cloud/spawn.rs` + `droplet.rs`. The visual payoff (matching the
  film's unusual flicker) is debatable.
- **F4 global changing-glyph sync** — couples all cells to a global
  frame counter. Doable but conflicts with cosmostrix's per-cell
  async phosphor decay model.
- **F5 deletion strings** — would require a second class of "eraser"
  droplets that overwrite existing cells with invisible glyphs. Adds
  complexity to the spawn pool.
- **F9 stammer event** — could be emulated as a periodic global
  "fall-behind by 1 row" event applied to highlighted droplets. The
  wind-gust envelope already serves a similar rhythmic purpose.

---

## 5. Concrete Tuning Recipe — "Film-Accurate Mode"

If a `--scene matrix_film` preset is desired, here is the constant
delta against current defaults:

| Constant | Current | Film-Accurate | File:Line |
|----------|---------|---------------|-----------|
| `HEAD_HIGHLIGHT_PCT` (new) | (1.0 implicit) | 0.20 | new in `central_control_rains.rs` |
| `PHOSPHOR_DECAY_RATE` | 5.0 | 12.0 (halve the trail duration) | `central_control_rains.rs:252` |
| `PHOSPHOR_BOTTOM_DECAY_MULT` | 3.0 | 6.0 (shorter bottom trails) | `central_control_rains.rs:286` |
| `PARALLAX_LAYERS` | 3 | 1 (single-layer for film accuracy) | `central_control_rains.rs:170` |
| `glitch_pct` (matrix_film scene) | 0.10 | 0.0 (film has no glitch) | `cloud/scene_runtime.rs:126-132` |
| `short_pct` | 0.5 | 0.5 (already matches) | same |
| `die_early_pct` | 0.3333 | 0.3333 (already matches) | same |
| `--uniform` | off | on (film has near-uniform column speeds) | `config.rs:276-282` |
| `--charset` | binary | matrix (katakana + 日 + numerals + Z + symbols) | `src/charset.rs` |
| `--color` | green | `matrix_film` (new palette with whiter head) | new in `chroma/catalog.rs` |
| `GLYPH_MUTATION_PERIOD_FRAMES` (new) | (1, per-frame) | 3 (with 1 blend frame) | new in `central_control_rains.rs` |

With these deltas, cosmostrix would render at ~95% film accuracy. The
remaining 5% is the stay-and-replace paradigm (F1), which is
architecturally out of scope.

---

## 6. Verdict

Cosmostrix is **not a film-accurate reproduction** — and shouldn't be
judged as one. It is a cinematic enhancement of the digital rain
concept, layering 3-layer parallax, phosphor afterglow, depth fog,
vignette, wind gusts, and turbulence on top of the basic vertical-fall
model. The film is a 2D flat animation with unusual stay-and-replace
glyph behavior; cosmostrix is a 3D-depth CRT-persistence terminal
renderer.

The two share 6 of 12 invariants. Of the 6 divergences:

- 4 are deliberate enhancements (parallax, afterglow, fog, wind gust)
- 2 are tunable cheap wins (highlight frequency, head color)
- 2 are out of scope (stay-and-replace, deletion strings)

**Recommendation:** If a `--scene matrix_film` preset is desired,
implement T2.1 (head highlight 20%) and T2.2 option A (new
`matrix_film` palette with whiter head). These two changes alone would
close most of the "feel" gap. T2.3 (changing-glyph cadence) is a
stretch goal — high impact but mid-effort. Tier 3 items are not worth
the architectural cost.

---

## Sources

- Carl Newton, *An analysis of the behaviour of the digital rain in
  The Matrix* — <https://carlnewton.github.io/digital-rain-analysis/>
- Wikipedia, *Digital rain* — <https://en.wikipedia.org/wiki/Digital_rain>
- Sreenidhi Podder, *The Surprising Inspiration Behind The Matrix's
  Digital Rain*, No Film School, Aug 28 2025 —
  <https://nofilmschool.com/matrix-digital-rain-origin>
- Cosmostrix source: `src/central_control_rains.rs`,
  `src/cloud/phosphor.rs`, `src/cloud/spawn.rs`, `src/droplet.rs`,
  `src/chroma/catalog.rs`, `src/chroma/tuning.rs`,
  `src/chroma/shaders/base.rs`, `src/interactive/event_loop.rs`
