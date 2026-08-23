<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Preset Battle Round 2 — The Long-Usage Championship

**Date**: 2026-08-23
**Reigning champion (entering)**: Cinema Noir (battle 1 winner, 2026-08-17)
**Question**: The owner is not fully convinced by the current champion's tune
and wants challenger presets designed for **LTS long usage** — one of them
may become the new shipped default.

## VERDICT (owner, 2026-08-23)

> **CHAMPION: Deep Focus** — declared by owner terminal A/B verdict.
> Applied as the shipped default and locked as the visual identity
> (see `docs/VISUAL_IDENTITY.md`). Cinema Noir is retained as a switchable
> preset (`./scripts/apply-visual-preset.sh cinema-noir`), along with the
> two other challengers.
>
> Signoff: **oxyzenQ** — 2026-08-23 — preset battle round 2 verdict,
> long-usage endurance profile.

**How to test each challenger on a real terminal**:

```bash
./scripts/apply-visual-preset.sh <cinema-noir|deep-focus|celluloid|late-broadcast>
cargo build --release
./target/release/cosmostrix
```

Switch back any time: `./scripts/apply-visual-preset.sh cinema-noir`.

---

## 1. Why Round 2 — What Round 1 Did Not Optimize For

Cinema Noir won battle 1 on **narrative** (emergence → life → dissolution)
against uniformly-bright challengers. It was tuned for the first-impression
wow of a cinematic frame. It was never specifically tuned for **watching for
hours**: sustained wide luminance swing (very dark frame vs bright hero
heads), 400 ms afterglow busyness, and heavy corner darkening can fatigue
the eye over a long session, and on some panels the noir field reads as
murky rather than dramatic.

Round 2 keeps the noir narrative DNA — dark entry, hero pop, gentle
dissolve — and offers three coherent refinements, each pushing a different
axis the owner can feel within the first minute:

| Challenger | Axis | One-line identity |
|-----------|------|-------------------|
| **Deep Focus** | Endurance / eye comfort | The same film, watched in a well-calibrated room |
| **Celluloid** | Film-stock authenticity | The Matrix as printed on 35 mm stock |
| **Late Broadcast** | Luminous comfort | A lit monitor at 2 AM — never swallowed by dark |

All three pass the full test suite (1651 tests), including the universal
visibility floor and the cinematic dissolve window regression guards.

## 2. The Challengers — Full Parameter Tables

### 2.1 Deep Focus — endurance refinement (recommended for long usage)

**Narrative**: preserve Cinema Noir's story exactly, but reduce the two
fatigue sources over multi-hour sessions — the luminance swing between the
dark field and bright heads, and the heavy lens darkening. Slightly lift
the field, slightly tame the heads, lighten the vignette. The frame stops
"pressing" on the eye at hour three.

| Parameter | Cinema Noir | Deep Focus | Delta |
|-----------|------------:|-----------:|-------|
| `EDGE_FADE_TOP_MIN` | 0.45 | **0.48** | gentler dark entry |
| `EDGE_FADE_BOTTOM_MIN` | 0.65 | **0.68** | later dissolve |
| `EDGE_FADE_BOTTOM_ROWS` | 10 | **12** | wider dissolve gradient |
| `EDGE_FADE_BOTTOM_LIP` | 0.80 | **0.82** | smoother junction |
| `VIGNETTE_INTENSITY` | 0.20 | **0.14** | lighter lens |
| `VIGNETTE_INNER_RADIUS` | 0.70 | **0.75** | vignette starts later |
| `CRT_VIGNETTE_EDGE_FACTOR` | 0.85 | **0.87** | a touch less glass |
| `RAIN_SHADOW_PCT` | 0.15 | **0.13** | smaller shadow zone |
| `RAIN_SHADOW_FLOOR` | 0.55 | **0.58** | lifted floor |
| `PARALLAX_BRIGHTNESS_MULT` | [0.52, 0.80, 1.10] | **[0.56, 0.82, 1.08]** | field up, head peak tamed |
| `PARALLAX_SATURATION_MULT` | [0.50, 0.84, 1.12] | **[0.52, 0.84, 1.10]** | slightly muted head color |
| `PARALLAX_HEAD_BLOOM_MULT` | [0.48, 0.74, 1.30] | **[0.48, 0.74, 1.24]** | glare control |
| `PARALLAX_CONTRAST_REDUCTION` | [0.50, 0.18, 0.0] | [0.50, 0.18, 0.0] | fog identity kept |
| `PHOSPHOR_DECAY_RATE` | 5.0 | **5.5** | ~360 ms trail (less residual busyness) |
| `PHOSPHOR_LAYER_DECAY_MULT` | [2.0, 1.2, 0.6] | **[1.9, 1.15, 0.65]** | back clears faster, front lingers more |
| `PHOSPHOR_BOTTOM_DECAY_MULT` | 2.0 | **1.8** | dissolve lingers slightly |
| `HEAD_BLOOM_INTENSITY` | 0.40 | **0.36** | glare control |

**Measured** (80×40 back layer): bottom-center 0.419, bottom-corner 0.362
(Cinema Noir: 0.380 / 0.305). Top-entry ≈ 0.39 (model estimate; noir ≈ 0.34).

**Watch for**: after 30+ minutes, does the eye feel less "squeezed" than
noir? Is the front-layer head still a satisfying pop?

### 2.2 Celluloid — 35 mm film-stock authenticity

**Narrative**: what if the Matrix rain was photographed on real film stock?
Deeper atmospheric recession (back layer sinks further into fog), a muted
film palette instead of digital neon, halation glow instead of hard pop,
and physical persistence — the ~480 ms trail of light sitting on celluloid.
Darker entry, longer roll-off, stronger lens character. The most *filmic*
of the three; also the darkest.

| Parameter | Cinema Noir | Celluloid | Delta |
|-----------|------------:|----------:|-------|
| `EDGE_FADE_TOP_MIN` | 0.45 | **0.42** | harder film-contrast entry |
| `EDGE_FADE_BOTTOM_MIN` | 0.65 | **0.62** | earlier roll-off |
| `EDGE_FADE_BOTTOM_ROWS` | 10 | **14** | long halation tail |
| `EDGE_FADE_BOTTOM_LIP` | 0.80 | **0.78** | — |
| `VIGNETTE_INTENSITY` | 0.20 | **0.24** | stronger photographic lens |
| `VIGNETTE_INNER_RADIUS` | 0.70 | **0.66** | lens starts earlier |
| `CRT_VIGNETTE_EDGE_FACTOR` | 0.85 | **0.83** | warmer glass |
| `RAIN_SHADOW_PCT` | 0.15 | **0.17** | wider shadow |
| `RAIN_SHADOW_FLOOR` | 0.55 | **0.52** | deeper floor |
| `PARALLAX_BRIGHTNESS_MULT` | [0.52, 0.80, 1.10] | **[0.46, 0.78, 1.06]** | darker stock, restrained highlights |
| `PARALLAX_SATURATION_MULT` | [0.50, 0.84, 1.12] | **[0.44, 0.80, 1.06]** | muted film palette |
| `PARALLAX_HEAD_BLOOM_MULT` | [0.48, 0.74, 1.30] | **[0.44, 0.70, 1.22]** | halation, not neon |
| `PARALLAX_CONTRAST_REDUCTION` | [0.50, 0.18, 0.0] | **[0.58, 0.22, 0.04]** | deep recession; front gets a whisper of haze |
| `PHOSPHOR_DECAY_RATE` | 5.0 | **4.2** | ~480 ms film persistence |
| `PHOSPHOR_LAYER_DECAY_MULT` | [2.0, 1.2, 0.6] | **[2.2, 1.3, 0.55]** | front lingers longest |
| `PHOSPHOR_BOTTOM_DECAY_MULT` | 2.0 | **2.4** | dissolve lingers |
| `HEAD_BLOOM_INTENSITY` | 0.40 | **0.34** | glow, not pop |

**Measured**: bottom-center 0.333, bottom-corner 0.254. Top-entry ≈ 0.31
(model estimate) — the darkest entry of the four.

**Watch for**: does the frame feel *photographed*? Do heads read as
"glowing light on film" rather than "neon pixels"? If the field feels too
murky over time, this one is too dark for your panel.

### 2.3 Late Broadcast — luminous comfort

**Narrative**: a newsroom monitor at 2 AM — a screen that is clearly lit in
a dark room. Built for bright rooms, glossy panels, and viewers who want
cinematic framing (top/bottom fades still present) without the rain ever
being swallowed by darkness. Snappier ~330 ms trails, minimal lens, highest
readability of the four.

| Parameter | Cinema Noir | Late Broadcast | Delta |
|-----------|------------:|---------------:|-------|
| `EDGE_FADE_TOP_MIN` | 0.45 | **0.55** | visible entry (still darker than body) |
| `EDGE_FADE_BOTTOM_MIN` | 0.65 | **0.72** | late soft dissolve |
| `EDGE_FADE_BOTTOM_ROWS` | 10 | **8** | tighter dissolve |
| `EDGE_FADE_BOTTOM_LIP` | 0.80 | **0.84** | — |
| `VIGNETTE_INTENSITY` | 0.20 | **0.10** | minimal lens |
| `VIGNETTE_INNER_RADIUS` | 0.70 | **0.80** | lens starts late |
| `CRT_VIGNETTE_EDGE_FACTOR` | 0.85 | **0.91** | barely-there glass |
| `RAIN_SHADOW_PCT` | 0.15 | **0.10** | narrow shadow |
| `RAIN_SHADOW_FLOOR` | 0.55 | **0.65** | high floor |
| `PARALLAX_BRIGHTNESS_MULT` | [0.52, 0.80, 1.10] | **[0.60, 0.84, 1.12]** | brighter field |
| `PARALLAX_SATURATION_MULT` | [0.50, 0.84, 1.12] | **[0.56, 0.86, 1.14]** | vivid |
| `PARALLAX_HEAD_BLOOM_MULT` | [0.48, 0.74, 1.30] | **[0.52, 0.76, 1.28]** | even pop on a bright field |
| `PARALLAX_CONTRAST_REDUCTION` | [0.50, 0.18, 0.0] | **[0.42, 0.14, 0.0]** | less fog — clarity |
| `PHOSPHOR_DECAY_RATE` | 5.0 | **6.0** | ~330 ms snappy trail |
| `PHOSPHOR_LAYER_DECAY_MULT` | [2.0, 1.2, 0.6] | **[1.8, 1.1, 0.55]** | monitor-like |
| `PHOSPHOR_BOTTOM_DECAY_MULT` | 2.0 | **1.7** | crisper bottom |
| `HEAD_BLOOM_INTENSITY` | 0.40 | **0.42** | strong pop on the bright field |

**Measured**: bottom-center 0.526, bottom-corner 0.476. Top-entry ≈ 0.48
(model estimate) — the brightest of the four, still framed.

**Watch for**: is every layer readable even at the corners in your room
lighting? Does it still feel cinematic (framed), or does it start feeling
like "a nice green terminal" (the reason Neon Sharp lost round 1)?

## 3. Measured Character Comparison

Compounded brightness, 80×40 terminal, back layer (all 4 effects):

| Preset | Bottom-center | Bottom-corner | Top-entry (est.) | Character |
|--------|--------------:|--------------:|-----------------:|-----------|
| Cinema Noir (champion) | 0.380 | 0.305 | ~0.34 | Dark but visible — noir |
| Deep Focus | 0.419 | 0.362 | ~0.39 | Noir, minus the squeeze |
| Celluloid | 0.333 | 0.254 | ~0.31 | Darkest — photographed film |
| Late Broadcast | 0.526 | 0.476 | ~0.48 | Brightest — lit monitor |

Universal guards (all four pass): every bottom-row column >= 0.10
visibility floor; bottom values inside the cinematic dissolve window
[0.30, 0.55] center / [0.22, 0.52] corner — enforced by the
`compounded_brightness_bottom_row_above_visibility_threshold` regression
test, which now guards ANY shipped preset instead of pinning one
champion's exact calibration.

## 4. Battle Protocol (Owner)

1. Run each preset for a real session — not 10 seconds. Recommended order:
   `deep-focus` → `cinema-noir` (baseline refresh) → `celluloid` →
   `late-broadcast`. Long-usage feel shows up after ~30 minutes.
2. Judge on: eye comfort over time, head pop satisfaction, entry/dissolve
   narrative, readability in your actual room lighting.
3. Declare the winner; the champion preset is then set as the shipped
   default (a one-line script call + this document and VISUAL_IDENTITY.md
   get updated with the verdict).
4. If none wins, the champion stays Cinema Noir and these become
   documented presets users can switch to at will.

## 5. Engineering Notes

- The switcher patches only the 17 preset-identity parameters in
  `src/central_control_rains/mod.rs`; the Option F inherited values
  (speed / length / density / selfbloom / monolith) are identical in every
  preset.
- Test impact: exactly one test pinned the champion's exact calibration
  (`compounded_brightness_bottom_row_above_visibility_threshold`). Its
  exact pins were relaxed into the universal visibility floor + the
  cinematic dissolve window (both still hard asserts) so any preset —
  present or future — is guarded. Verified: all four presets pass the
  full 1651-test suite.
- No production code was changed by the battle infrastructure; only the
  test calibration contract (documented as a cosmic-dragon KEY.md unlock
  entry) and docs.

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
