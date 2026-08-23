<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Cosmostrix Visual Identity — Single Source of Truth

**Current identity: Cinema Noir preset** (battle champion, locked 2026-08-17,
commit `0d88da5`).

This document is the canonical reference for "what does cosmostrix look like
and why". It exists because the visual-preset history is spread across
superseded audit documents (and one deleted research doc), which made the
current state ambiguous. Every value below was verified against the source
in August 2026; where this document and any other doc disagree, the source
(`src/central_control_rains/mod.rs`) wins — and this doc wins over every
other doc.

---

## 1. Disambiguation First — Two Different Things Called "Cinematic"

| Concept | What it is | Where it lives |
|---------|-----------|----------------|
| **Scene `cinematic`** | A scene named "cinematic" in the scene cycle — cosmic zen: slow vast pacing, energy-zen color, zen charset. Also the startup default scene. | `src/scene/mod.rs` |
| **Cinema Noir preset** | The visual *treatment* of the rain itself — dark entry, gentle dissolve, warm glass, hero pop. Applies to EVERY scene (monolith, matrix, cinematic, …). | `src/central_control_rains/mod.rs` |

Saying "the rain is cinematic" means the Cinema Noir treatment. Saying "run
the cinematic scene" selects the cosmic-zen scene. They are independent: the
cinematic scene is rendered *through* the Cinema Noir treatment, as is every
other scene.

## 2. The Cinema Noir Preset — Champion Values (verified in source)

The preset is a 17-parameter coherent package applied on top of the Option F
parallax foundation (speed / length / density layers are inherited from
Option F and unchanged).

### Core frame treatment

| Parameter | Value | Role |
|-----------|------:|------|
| `EDGE_FADE_TOP_MIN` | 0.45 | Dark entry — rain materializes from black (55% dim) |
| `EDGE_FADE_BOTTOM_MIN` | 0.65 | Gentle dissolve at bottom (35% dim) |
| `EDGE_FADE_BOTTOM_ROWS` | 10 | Wide dissolve zone |
| `EDGE_FADE_BOTTOM_LIP` | 0.80 | Lifted junction between body and dissolve |
| `VIGNETTE_INTENSITY` | 0.20 | 20% corner darkening — photographic lens |
| `VIGNETTE_INNER_RADIUS` | 0.70 | Vignette starts at 70% of the frame |
| `CRT_VIGNETTE_EDGE_FACTOR` | 0.85 | 15% CRT dim — warm glass, not distortion |
| `RAIN_SHADOW_PCT` | 0.15 | 15% bottom shadow zone |
| `RAIN_SHADOW_FLOOR` | 0.55 | Shadow-zone brightness floor |

### Rain field and light

| Parameter | Value | Role |
|-----------|------:|------|
| `PARALLAX_BRIGHTNESS_MULT` | [0.52, 0.80, 1.10] | Per-layer luminance (back/mid/front) |
| `PARALLAX_SATURATION_MULT` | [0.50, 0.84, 1.12] | Per-layer vividness |
| `PARALLAX_HEAD_BLOOM_MULT` | [0.48, 0.74, 1.30] | Head glow falloff |
| `PARALLAX_CONTRAST_REDUCTION` | [0.50, 0.18, 0.0] | Depth fog (back dissolves into haze) |
| `PHOSPHOR_DECAY_RATE` | 5.0 | ~400 ms cinematic afterglow |
| `PHOSPHOR_LAYER_DECAY_MULT` | [2.0, 1.2, 0.6] | Back flickers fast, front lingers |
| `PHOSPHOR_BOTTOM_DECAY_MULT` | 2.0 | Bottom dissolve persistence |
| `HEAD_BLOOM_INTENSITY` | 0.40 | Strong pop, not blown out |

### Inherited from Option F "Film Matrix Hero" (unchanged)

`PARALLAX_SPEED_MULT` [0.35, 1.0, 1.7] · `PARALLAX_LENGTH_MULT`
[0.5, 1.0, 1.4] · `PARALLAX_DENSITY_MULT` [0.45, 0.62, 0.85] ·
`PARALLAX_HEAD_SELFBLOOM_MULT` [0.38, 0.68, 1.20] ·
`MONOLITH_LAYER_BRIGHTNESS` [0.48, 0.78, 1.0] ·
`MONOLITH_BREATHING_AMPLITUDE` [0.018, 0.026, 0.034].

## 3. Why Cinema Noir Won — Design Rationale

The champion was decided by an owner-run A/B terminal battle on 2026-08-17
against four challengers. The essential insight:

> Neon Sharp (runner-up) was technically excellent — maximum contrast, crisp
> neon — but it had **no narrative**. The screen was uniformly bright: "a
> green terminal with good colors". Cinema Noir tells a story: rain
> **appears from nothing** (dark top entry), **lives briefly in full neon**
> (front layer is exempt from shadow and vignette), and **fades gently**
> (soft bottom dissolve extended by the 400 ms afterglow). Emergence →
> life → dissolution. That narrative is what "cinematic" means here.

Key mechanisms:

1. **Asymmetric top/bottom** — the noir signature. Top is dramatically
   darker than bottom; the eye is drawn downward as brightness rises.
2. **Front layer exemption** — hero droplets pop even at viewport extremes,
   producing the "dark field, bright streaks" look — each bloom reads as a
   light in the dark.
3. **Warm glass, not distortion** — 15% CRT dim reads as photographed
   through vintage glass, not as an old monitor.
4. **Cinematic trail, not ghosting** — ~400 ms afterglow: long enough to
   feel like rain leaving light behind, short enough to avoid smearing.

Compounded brightness character (4-effect model:
`rain_shadow × edge_fade × radial_vignette × crt_vignette`, hardest-hit
back-layer cells on an 80×40 terminal): top ≈ 0.34, bottom-center ≈ 0.38,
bottom-corner ≈ 0.30 — dark but visible; that is the noir character.

## 4. Preset Lineage (History)

| Generation | Name | Date | Outcome |
|-----------|------|------|---------|
| 1 | v30.2 masterclass retune | 2026-08-09 | Fixed the v30 "too aggressive" regressions; top=0.533/bottom visible |
| 2 | Option F "Film Matrix Hero" | 2026-08 (RAIN-AUDIT-002) | Rated 10/10; locked as the depth-stack baseline |
| 3 | Neon Sharp (battle contender) | 2026-08-17 | Runner-up — rejected for lacking narrative |
| 4 | **Cinema Noir** | 2026-08-17 | **Champion — current identity** (`0d88da5`) |

Round 2 of the battle (new challenger presets designed for long-usage
endurance) is documented in
[`docs/research/PRESET_BATTLE_2.md`](research/PRESET_BATTLE_2.md); the
champion stays Cinema Noir until the owner declares a new winner.

## 5. Superseded Documents

These documents describe **historical** states, not the current one. They
are retained for their analysis quality:

- `docs/RAIN_DEPTH_AUDIT.md` — describes Option F, superseded by Cinema Noir
  (two back-layer values differ from current source).
- `docs/research/VISUAL_MODE_AUDIT.md` — describes the v30.2 masterclass
  retune, superseded by Option F and then Cinema Noir.
- The original `docs/research/PRESET_BATTLE_VERDICT.md` was deleted by the
  2026-08 orphan-doc cleanup; its verdict content is preserved in section 3
  of this document (restored from git history `6faf7b6`).

## 6. How to Tune

- All rain visual constants live in `src/central_control_rains/mod.rs`
  (plug-and-play control file — edit values, `cargo build --release`).
- To switch between the champion and the round-2 challenger presets:
  `./scripts/apply-visual-preset.sh <name>` (see
  [`docs/research/PRESET_BATTLE_2.md`](research/PRESET_BATTLE_2.md)).
- Tuning guide for individual parameters:
  [`docs/CENTRAL_CONTROL_RAINS_USAGE.md`](CENTRAL_CONTROL_RAINS_USAGE.md).

---

Copyright (C) 2026 rezky_nightky (oxyzenQ). All rights reserved.
cosmostrix and the cosmostrix logo are trademarks of rezky_nightky (oxyzenQ).
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
