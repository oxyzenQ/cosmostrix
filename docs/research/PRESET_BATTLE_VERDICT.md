<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Visual Preset Battle — The 5-Way Cinematic Showdown

**Date:** 2026-08-17
**Verdict:** 🏆 **Cinema Noir (OPSI 1) — CHAMPION**
**Runner-up:** Neon Sharp (OPSI 5)

---

## The Problem

Owner confused by too many visual effects (Vignette, CRT, afterglow,
phosphor, dimmer) and no clear path to tune them cinematically. Each
parameter interacts with 3+ others through the **4-effect compounding
model**:

```
compounded_brightness = rain_shadow × edge_fade × radial_vignette × crt_vignette
```

Tuning one parameter in isolation breaks the compounded brightness at
viewport extremes. The solution: **coherent preset packages** where the
owner picks one aesthetic and all 17 parameters are pre-calibrated as
a unit.

---

## The 5 Contenders

| # | Preset | Vibe | 1-line | Status |
|---|--------|------|--------|--------|
| 1 | **Cinema Noir** | 🎬 Dark entry, gentle dissolve | Rain appears from nothing at top, fades to nothing at bottom | 🏆 **CHAMPION** |
| 2 | Classic Matrix | 🟢 Faithful 1999 film | Balanced CRT curve, moderate vignette, green-dominant | Not tested |
| 3 | Retro CRT | 📺 Heavy CRT glass | Strong tube distortion, warm amber edges, scanline feel | ❌ REJECTED |
| 4 | Dream Haze | 💭 Soft ethereal | Heavy vignette, slow phosphor, mist-like diffusion | ❌ REJECTED |
| 5 | Neon Sharp | 🔥 Max contrast, zero vignette | Crisp neon pop, no lens, no monitor frame | ❌ REJECTED (runner-up) |

---

## Battle Bracket

```
ROUND 1 — Visual Test (owner A/B on actual terminal)
==========================================

  Retro CRT (3)  ──── ❌ REJECTED
  Dream Haze (4) ──── ❌ REJECTED

  (rejected early — too much distortion / too hazy)

SEMI-FINAL
==========================================

  Neon Sharp (5) ──── vs ──── Cinema Noir (1)
       🔥                              🎬
  Max contrast                  Dark entry
  Zero vignette                Noir dissolve
  Crisp neon                   Cinematic trail
       │                              │
       └──── ❌ REJECTED ────── 🏆 CHAMPION ────┘
```

---

## Full Parameter Comparison

| Parameter | Cinema Noir 🏆 | Neon Sharp 🔥 | Retro CRT 📺 | Dream Haze 💭 |
|-----------|---------------|--------------|-------------|--------------|
| `CRT_VIGNETTE_EDGE_FACTOR` | **0.85** | 0.97 | 0.78 | 0.88 |
| `VIGNETTE_INTENSITY` | **0.20** | 0.05 | 0.30 | 0.35 |
| `VIGNETTE_INNER_RADIUS` | **0.70** | 0.85 | 0.60 | 0.55 |
| `RAIN_SHADOW_PCT` | **0.15** | 0.08 | 0.12 | 0.18 |
| `RAIN_SHADOW_FLOOR` | **0.55** | 0.78 | 0.60 | 0.62 |
| `EDGE_FADE_TOP_MIN` | **0.45** | 0.90 | 0.60 | 0.55 |
| `EDGE_FADE_BOTTOM_MIN` | **0.65** | 0.80 | 0.60 | 0.55 |
| `EDGE_FADE_BOTTOM_ROWS` | **10** | 6 | 12 | 14 |
| `EDGE_FADE_BOTTOM_LIP` | **0.80** | 0.88 | 0.78 | 0.75 |
| `PARALLAX_BRIGHTNESS_MULT` | **[0.52, 0.80, 1.10]** | [0.48, 0.80, 1.15] | [0.55, 0.78, 1.05] | [0.45, 0.72, 1.08] |
| `PARALLAX_SATURATION_MULT` | **[0.50, 0.84, 1.12]** | [0.45, 0.85, 1.15] | [0.55, 0.82, 1.05] | [0.40, 0.75, 1.05] |
| `PARALLAX_HEAD_BLOOM_MULT` | **[0.48, 0.74, 1.30]** | [0.40, 0.70, 1.40] | [0.50, 0.78, 1.20] | [0.40, 0.65, 1.15] |
| `PARALLAX_CONTRAST_REDUCTION` | **[0.50, 0.18, 0.0]** | [0.55, 0.15, 0.0] | [0.40, 0.12, 0.0] | [0.55, 0.22, 0.0] |
| `PHOSPHOR_DECAY_RATE` | **5.0** | 7.0 | 3.5 | 2.8 |
| `PHOSPHOR_LAYER_DECAY_MULT` | **[2.0, 1.2, 0.6]** | [2.5, 1.5, 0.5] | [1.5, 0.8, 0.4] | [1.2, 0.7, 0.35] |
| `PHOSPHOR_BOTTOM_DECAY_MULT` | **2.0** | 3.0 | 1.5 | 1.2 |
| `HEAD_BLOOM_INTENSITY` | **0.40** | 0.55 | 0.50 | 0.35 |

---

## Compounded Brightness at Viewport Extremes

The **actual number** the eye sees — all 4 effects multiplied together
at the hardest-hit cells (80×40 terminal, back layer).

| Preset | Top row | Bottom-center | Bottom-corner | Character |
|--------|---------|---------------|---------------|-----------|
| **Cinema Noir 🏆** | **0.342** | **0.380** | **0.302** | Dark but visible — noir |
| Neon Sharp 🔥 | ~0.87 | ~0.680 | ~0.648 | Bright everywhere — crisp |
| Retro CRT 📺 | ~0.28 | ~0.42 | ~0.34 | Heavy CRT distortion |
| Dream Haze 💭 | ~0.20 | ~0.35 | ~0.27 | Ethereal mist |

**Why 0.342 top wins over 0.87 top:** The dramatic dark entry at top
is the soul of film noir — rain materializing from black creates
tension and depth. Bright top (Neon Sharp) feels flat, like a
spreadsheet with green text. Noir **conceals** and **reveals** — that
asymmetry is cinematic.

---

## Cinema Noir — Design Rationale

### Asymmetric Top/Bottom: The Noir Signature

```
TOP:    ████████░░░░░░░░  55% dim — "from darkness" entry
BOTTOM: ░░░░░░░░████████  35% dim — gentle dissolve
FRONT:  ████████████████  EXEMPT from shadow & vignette
```

The top edge is dramatically darker than the bottom. This is
**intentional** and the core of the noir aesthetic:

1. **Rain enters from deep shadow** — like a film noir scene where the
   subject materializes from black. The viewer's eye is drawn downward
   as brightness increases row-by-row from the top.

2. **Rain dissolves gently at bottom** — the softer bottom fade (65% min
   vs 45% top) lets rain trail off naturally rather than cutting hard.
   The phosphor afterglow (5.0 decay = ~400ms trail) extends this
   dissolve cinematically.

3. **Front layer stays bright** — layer 2 is exempt from both
   `rain_shadow_factor` and `vignette_factor`, so hero neon droplets
   pop even at viewport extremes. This creates the signature **dark
   field, bright streaks** look.

### CRT Vignette: Warm Glass, Not Distortion

At 0.85 edge factor (15% dim), CRT vignette adds warmth without the
heavy barrel distortion of Retro CRT (0.78). The effect reads as
"photographed through vintage glass" rather than "old monitor".

### Phosphor: Cinematic Trail, Not Ghosting

Decay rate 5.0 produces ~400ms afterglow — long enough to feel like a
cinematic trail (rain leaves light as it falls), short enough that
droplets don't smear. Combined with layer decay [2.0, 1.2, 0.6], the
back layer flickers fast (atmospheric), while front trails linger
(hero glow).

### Head Bloom: Strong Pop, Not Overblown

Intensity 0.40 with front bloom 1.30 — every front-layer head reads as
a bright neon point, but not blown out. The noir darkness surrounding
each bloom point makes it read as **a light in the dark**, which is the
entire point.

---

## Why Neon Sharp Lost

Neon Sharp is technically excellent — maximum contrast, zero vignette,
crisp neon. But it lacks **narrative**. The screen is uniformly bright;
rain doesn't tell a story of emergence and dissolution. It reads as
"green terminal with good colors" rather than "cinematic experience".

| Aspect | Neon Sharp | Cinema Noir |
|--------|-----------|-------------|
| Top row | 0.90 → bright entry | 0.45 → dark entry |
| Bottom row | 0.80 → fills to edge | 0.65 → gentle dissolve |
| Vignette | 5% → no lens | 20% → photographic lens |
| Phosphor | 7.0 → snappy (285ms) | 5.0 → trail (400ms) |
| CRT curve | 3% → invisible | 15% → warm glass |
| **Story** | **None — uniform field** | **Emergence → dissolution** |

The noir asymmetry (top aggressive, bottom gentle) creates a **visual
narrative**: rain appears from nothing, lives briefly in full neon, then
fades. That narrative is what makes it cinematic.

---

## Why Retro CRT & Dream Haze Were Rejected Early

### Retro CRT (OPSI 3) — Too Distorted

CRT_VIGNETTE_EDGE_FACTOR=0.78 (22% dim) + VIGNETTE_INTENSITY=0.30
creates heavy barrel distortion. Reads as "broken monitor" rather than
"vintage CRT". The phosphor decay 3.5 is also too slow — trails smear
into noise rather than clean cinematic trails.

### Dream Haze (OPSI 4) — Too Soft

VIGNETTE_INTENSITY=0.35 + VIGNETTE_INNER_RADIUS=0.55 creates an
extremely soft, misty field. Combined with PHOSPHOR_DECAY_RATE=2.8
(very slow) and RAIN_SHADOW_PCT=0.18, everything feels like it's
behind frosted glass. Loses the sharp neon character that makes the
rain read as "digital rain" rather than "fog".

---

## Test Calibration

Each preset swap required updating 4 test assertions because the
compounded brightness targets change:

| Test | Neon Sharp | Cinema Noir |
|------|-----------|-------------|
| `viewport_edge_fade_top_more_aggressive_than_bottom` | `bottom < top` | `top < bottom` (flipped) |
| `rain_shadow_factor_floors` threshold | 36 (PCT=0.08) | 34 (PCT=0.15) |
| Bottom-center brightness | ~0.680 | ~0.380 |
| Bottom-corner brightness | ~0.648 | ~0.302 |
| Brightness boost delta | 11.0..=19.0 (front=1.15) | 8.0..=14.0 (front=1.10) |
| Top-center floor | >= 0.30 | >= 0.25 |
| Top-corner floor | >= 0.25 | >= 0.15 |

All 1532 tests pass on main with Cinema Noir values.

---

## Git History

```
c1738c3 feat(visual): apply Neon Sharp preset (previous main)
0d88da5 feat(visual): apply Cinema Noir preset — dark entry, gentle dissolve
0245712 Merge branch 'preset/cinema-noir' — Cinema Noir wins champion
```

---

## Champion Lock

**Cinema Noir (OPSI 1) is the LTS v50 visual preset.** All future
tuning must preserve the noir asymmetry (top < bottom in edge fade)
and the compounded brightness targets (top ~0.342, bottom-center
~0.380, bottom-corner ~0.302). Any regression in these values is a
test failure.

The preset system is designed so that if the owner wants to revisit
this decision, they can checkout any branch and A/B test again without
touching main. But for now — **noir reigns**. 🎬
