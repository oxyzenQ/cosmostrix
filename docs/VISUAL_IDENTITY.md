<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Cosmostrix Visual Identity — Single Source of Truth

**Current identity: Deep Focus preset** (battle round 2 champion, locked
2026-08-23 by owner verdict — the endurance refinement of the Cinema Noir
narrative, chosen for LTS long usage).

> **Visual lock (2026-08-23)**: Deep Focus declared champion by owner
> A/B terminal battle. Signature: **oxyzenQ** — preset round 2 verdict,
> long-usage endurance profile. Applied via
> `scripts/apply-visual-preset.sh deep-focus`; reverts to the previous
> champion anytime via `./scripts/apply-visual-preset.sh cinema-noir`.

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

## 2. The Deep Focus Preset — Champion Values (verified in source)

The preset is a 17-parameter coherent package applied on top of the Option F
parallax foundation (speed / length / density layers are inherited from
Option F and unchanged). Deep Focus preserves the Cinema Noir narrative —
dark entry, hero pop, gentle dissolve — while reducing the two fatigue
sources of multi-hour sessions: the luminance swing between the dark field
and bright heads, and the heavy lens darkening.

### Core frame treatment

| Parameter | Value | Role |
|-----------|------:|------|
| `EDGE_FADE_TOP_MIN` | 0.48 | Dark entry — gentler than noir (52% dim) |
| `EDGE_FADE_BOTTOM_MIN` | 0.68 | Later dissolve (32% dim) |
| `EDGE_FADE_BOTTOM_ROWS` | 12 | Wider dissolve gradient |
| `EDGE_FADE_BOTTOM_LIP` | 0.82 | Smoother junction |
| `VIGNETTE_INTENSITY` | 0.14 | Lighter photographic lens |
| `VIGNETTE_INNER_RADIUS` | 0.75 | Vignette starts later |
| `CRT_VIGNETTE_EDGE_FACTOR` | 0.87 | 13% CRT dim — warm glass |
| `RAIN_SHADOW_PCT` | 0.13 | Smaller bottom shadow zone |
| `RAIN_SHADOW_FLOOR` | 0.58 | Lifted shadow floor |

### Rain field and light

| Parameter | Value | Role |
|-----------|------:|------|
| `PARALLAX_BRIGHTNESS_MULT` | [0.56, 0.82, 1.08] | Field lifted, head peak tamed |
| `PARALLAX_SATURATION_MULT` | [0.52, 0.84, 1.10] | Slightly muted head color |
| `PARALLAX_HEAD_BLOOM_MULT` | [0.48, 0.74, 1.24] | Glare control |
| `PARALLAX_CONTRAST_REDUCTION` | [0.50, 0.18, 0.0] | Fog identity kept from noir |
| `PHOSPHOR_DECAY_RATE` | 5.5 | ~360 ms trail — less residual busyness |
| `PHOSPHOR_LAYER_DECAY_MULT` | [1.9, 1.15, 0.65] | Back clears faster, front lingers |
| `PHOSPHOR_BOTTOM_DECAY_MULT` | 1.8 | Dissolve lingers slightly |
| `HEAD_BLOOM_INTENSITY` | 0.36 | Glare control |

Measured character (80×40 back layer): bottom-center 0.419,
bottom-corner 0.362 — noir's dissolve, minus the squeeze.

### Previous champion — Cinema Noir (2026-08-17 → 2026-08-23)

Superseded by Deep Focus in battle round 2. Full parameter table retained
in [`docs/research/PRESET_BATTLE_2.md`](research/PRESET_BATTLE_2.md);
reproduced for reference: frame 0.45/0.65/10/0.80, vignette 0.20/0.70, CRT
0.85, shadow 0.15/0.55, brightness [0.52, 0.80, 1.10], saturation
[0.50, 0.84, 1.12], head bloom [0.48, 0.74, 1.30] + intensity 0.40,
contrast reduction [0.50, 0.18, 0.0], phosphor 5.0 / [2.0, 1.2, 0.6] / 2.0.

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
| 4 | Cinema Noir | 2026-08-17 | Battle 1 champion (`0d88da5`) — superseded |
| 5 | **Deep Focus** | 2026-08-23 | **Battle 2 champion — current identity** (owner verdict: long-usage endurance) |

Battle round 2 (Cinema Noir vs Deep Focus / Celluloid / Late Broadcast) is
documented in
[`docs/research/PRESET_BATTLE_2.md`](research/PRESET_BATTLE_2.md); Deep
Focus won the owner's terminal A/B verdict on 2026-08-23.

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
