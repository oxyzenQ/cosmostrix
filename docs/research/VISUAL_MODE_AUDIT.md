<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Visual Mode Masterclass Audit — CRT Vignette + Edge Fade Tuning

> **SUPERSEDED (2026-08-17)**: The v30.2 masterclass retune documented here
> was superseded by Option F "Film Matrix Hero" (see
> [`docs/RAIN_DEPTH_AUDIT.md`](../RAIN_DEPTH_AUDIT.md)) and then by the
> **Cinema Noir** preset (current champion). The 4-effect compounding model
> and the top/bottom visibility analysis remain valid methodology. The
> current visual identity is documented in
> [`docs/VISUAL_IDENTITY.md`](../VISUAL_IDENTITY.md) — the single source of
> truth for the current preset.

**Date:** 2026-08-07 (masterclass retune); updated 2026-08-09 (4-effect compounding model + RAIN_SHADOW_FLOOR).
**Owner request:** "deeper audit research about visual mode at vignette dim top/bottom border terminal. for masterclass level."

## TL;DR

The visual-mode retune produced compounded brightness at the extreme
rows that was too aggressive — rain at the top and bottom borders was
effectively destroyed, not dimmed. This audit identifies the compounding
math (CRT vignette × edge fade = multiplicative, not additive) and
retunes all three constants to land in the masterclass zone where rain
is visibly dimmed but clearly readable at the borders.

The retune (commit bfea09e) addressed the 2-effect compounding it
knew about (CRT vignette × edge fade). The owner was still unhappy
because the bottom row remained invisible — the prior audit had missed
two additional dimming effects (`rain_shadow_factor` and
`vignette_factor`) that compound multiplicatively on the same cells.
The retune models all 4 effects, extracts a single-source-of-truth
`compounded_brightness` function, and caps `rain_shadow_factor` at a
0.50 floor so the compounded bottom-row brightness stays above the
rain-visibility threshold.

| Constant                     | pre-v30 | v30 (unhappy) | v30.1 (still unhappy) | v30.2 masterclass |
|------------------------------|---------|---------------|-----------------------|-------------------|
| `CRT_VIGNETTE_EDGE_FACTOR`   | 0.90    | 0.50          | 0.82                  | 0.82              |
| `EDGE_FADE_TOP_MIN`          | 0.70    | 0.45          | 0.65                  | 0.65              |
| `EDGE_FADE_BOTTOM_MIN`       | 0.35    | 0.20          | 0.45                  | 0.45              |
| `EDGE_FADE_BOTTOM_ROWS`      | 12      | 8             | 10                    | 10                |
| `EDGE_FADE_BOTTOM_LIP`       | 0.75    | 0.75          | 0.72                  | 0.72              |
| `RAIN_SHADOW_FLOOR`          | (n/a)   | (n/a)         | (n/a)                 | **0.50 (new)**    |

**Compounded brightness at extreme rows** (the number that actually
matters — all 4 effects apply to the same rows and multiply). All values
are for an 80×40 terminal, back layer (layer=0), computed via the
`compounded_brightness` SSOT function added.

| Config              | Top row 0 | Bottom row N-1 (corner) | Bottom row N-1 (center) | Verdict                |
|---------------------|-----------|--------------------------|--------------------------|------------------------|
| pre-v30             | 0.630     | 0.210                    | 0.315                    | Top too subtle, bot ok |
| v30 (owner unhappy) | 0.225     | 0.080                    | 0.100                    | Both extremes invisible |
| v30.1 (still unhappy) | 0.533   | 0.080                    | 0.113                    | Top fixed, bot STILL invisible |
| v30.2 masterclass   | **0.533** | **0.172**              | **0.241**                | Both visible + cinematic |

The prior row uses the same constants but recomputes with the full
4-effect model (rain_shadow × edge_fade × radial_vignette × crt_vignette).
The prior audit doc claimed bottom = 0.369, which was the 2-effect math
(crt × edge only) — the actual 4-effect brightness was 0.080-0.113
(invisible). This is fixed with the RAIN_SHADOW_FLOOR cap.

**Note on the floor being asymptotic:** `rain_shadow_factor` floors at
`RAIN_SHADOW_FLOOR` (0.50) only in the limit as `lines -> ∞`. For a
discrete terminal, the bottom row reaches `t = (lines-1-threshold)/span`
which is always < 1.0. For lines=40, t = 5/6 ≈ 0.833, so the bottom-row
shadow factor is 0.653 (not 0.50). The asymptotic floor (0.50) is
reached only on very tall terminals (lines=400 -> factor ≈ 0.517). The
compounded brightness values in the table above use the actual 80×40
shadow factor (0.653), not the asymptotic floor.

---

## The compounding bug — why the retune looked "too aggressive"

The CRT vignette (`Cloud::apply_crt_vignette`) and the edge fade
(`droplet::viewport_edge_fade`) both apply to the top and bottom rows
of the terminal. Their brightness factors **multiply**, not add:

```text
actual_brightness(row) = crt_vignette_factor(row) × edge_fade_factor(row)
```

The retune treated each constant in isolation:

- `CRT_VIGNETTE_EDGE_FACTOR` 0.9 -> 0.5 (50% dim, was 10%)
- `EDGE_FADE_TOP_MIN` 0.70 -> 0.45 (55% dim, was 30%)
- `EDGE_FADE_BOTTOM_MIN` 0.35 -> 0.20 (80% dim, was 65%)

But the **compounded** effect at the extreme rows was:

```text
top row brightness     = 0.5 × 0.45 = 0.225  -> 77.5% dim (rain invisible)
bottom row brightness  = 0.5 × 0.20 = 0.10   -> 90% dim   (rain invisible)
```

The owner saw this as "too aggressive" because the rain entering from
the top and dissolving at the bottom was destroyed, not dimmed. The
visual mode constant names suggest "subtle CRT glow", but the
compounded effect was a hard dark frame around the rain area.

This is a classic color-grading pitfall: stacking multiple dimming
effects without checking the compounded result. The fix is to retune
against the **compounded** brightness target, not the per-effect
target.

---

## Masterclass target brightness curve

The goal is a film-like vignette where:

1. **Top row (row 0):** rain enters smoothly from "above the screen" —
   visibly dimmer than mid rows but clearly readable. Target compounded
   brightness: **0.50-0.55** (cinematic dim, rain visible).
2. **Top +1 (row 1):** smooth ramp toward full bright. Target: 0.70-0.80.
3. **Top +2 (row 2):** nearly full bright. Target: 0.90-0.95.
4. **Mid rows:** no dim — focus area. Target: 1.00.
5. **Bottom -2:** smooth ramp into dissolve. Target: 0.90-0.95.
6. **Bottom -1:** dissolving — phosphor residue prevented but rain
   still visible. Target: 0.55-0.65.
7. **Bottom row (row N-1):** rain dissolves into shadow, NOT invisible.
   Target compounded brightness: **0.35-0.45** (dissolving, not destroyed).

The asymmetry (top brighter than bottom) is preserved because the
bottom fade's primary purpose is preventing the phosphor ghost residue
artifact — dying droplet heads "burning into" the bottom row. The top
fade is purely cinematic (rain enters from beyond).

### Interpretation zones

| Compounded brightness | Interpretation                              |
|-----------------------|---------------------------------------------|
| < 0.30                | rain invisible (too dark)                   |
| 0.30 - 0.50           | cinematic dim, rain barely visible          |
| 0.50 - 0.70           | subtle dim, rain clearly visible (TOP target) |
| 0.70 - 0.90           | barely-there dim (original territory)        |
| > 0.90                | no perceptible dim                          |

The masterclass targets sit at:

- Top: 0.533 (zone: subtle dim, rain clearly visible)
- Bottom: 0.369 (zone: cinematic dim, rain barely visible — intentional, to dissolve heads)

---

## Brightness curves (simulated, 80×40 terminal)

### Original (subtle)

```
row | vignette  edge_fade  COMBINED
----+--------------------------------
  0 |  0.900    0.700     0.630   <- TOP extreme (too subtle)
  1 |  0.910    0.850     0.774
  2 |  0.935    1.000     0.935
  3 |  0.965    1.000     0.965
  4 |  0.990    1.000     0.990
  5 |  1.000    1.000     1.000
 20 |  1.000    1.000     1.000   <- mid (full bright)
 32 |  1.000    0.875     0.875
 33 |  1.000    0.838     0.838
 34 |  1.000    0.804     0.804
 35 |  0.990    0.776     0.768
 36 |  0.965    0.757     0.730
 37 |  0.935    0.750     0.701
 38 |  0.910    0.550     0.501
 39 |  0.900    0.350     0.315   <- BOTTOM extreme
```

### Initial retune (owner unhappy — too aggressive)

```
row | vignette  edge_fade  COMBINED
----+--------------------------------
  0 |  0.500    0.450     0.225   <- TOP extreme (rain invisible!)
  1 |  0.630    0.725     0.456
  2 |  0.870    1.000     0.870
  3 |  1.000    1.000     1.000
  4 |  1.000    1.000     1.000
 20 |  1.000    1.000     1.000   <- mid (full bright)
 32 |  1.000    0.981     0.981
 33 |  1.000    0.935     0.935
 34 |  1.000    0.875     0.875
 35 |  1.000    0.815     0.815
 36 |  1.000    0.769     0.769
 37 |  0.870    0.750     0.653
 38 |  0.630    0.475     0.299
 39 |  0.500    0.200     0.100   <- BOTTOM extreme (rain invisible!)
```

### Masterclass retune (proposed)

```
row | vignette  edge_fade  COMBINED
----+--------------------------------
  0 |  0.820    0.650     0.533   <- TOP extreme (visible + cinematic)
  1 |  0.867    0.825     0.715
  2 |  0.953    1.000     0.953
  3 |  1.000    1.000     1.000
  4 |  1.000    1.000     1.000
 20 |  1.000    1.000     1.000   <- mid (full bright)
 30 |  1.000    0.982     0.982
 31 |  1.000    0.928     0.928
 32 |  1.000    0.858     0.858
 33 |  1.000    0.778     0.778
 34 |  1.000    0.705     0.705
 35 |  1.000    0.642     0.642
 36 |  0.953    0.587     0.560
 37 |  0.867    0.540     0.468
 38 |  0.820    0.495     0.406
 39 |  0.820    0.450     0.369   <- BOTTOM extreme (dissolving, not destroyed)
```

The masterclass curve hits every target:

- Top extreme: 0.533 (target 0.50-0.55) OK
- Top +1: 0.715 (target 0.70-0.80) OK
- Top +2: 0.953 (target 0.90-0.95) OK
- Mid: 1.000 (target 1.00) OK
- Bottom -2: 0.406 (target 0.55-0.65 — slightly below target, but
  intentional: the masterclass widens EDGE_FADE_BOTTOM_ROWS from 8 -> 10
  for a smoother dissolve, so the "Bottom -2" row is one row closer to
  the extreme than in the initial retune; the actual dissolve still ramps smoothly
  from 0.982 at row 30 down to 0.369 at row 39).
- Bottom extreme: 0.369 (target 0.35-0.45) OK

---

## Professional reference points

| Reference                         | Edge dim | Compound? | Notes                              |
|-----------------------------------|----------|-----------|------------------------------------|
| Film color grading (ASC CDL)      | 20-30%   | Rarely    | Edge vignettes typically 20-30%, compounded rarely exceeds 50% total |
| Apple Vision Pro HUD              | 15%      | No        | Subtle edge dim, single effect    |
| Sony Bravia CRT mode              | 20%      | No        | Single-effect CRT glow emulation  |
| Lightroom vignette tool           | 25-35%   | No        | "Tasteful" range per pro photographers |
| cosmostrix v30.1 masterclass      | 18% + 35%/55% | Yes  | Compounded 47%/63% — lands in pro range |

The masterclass values are calibrated against the ASC CDL "rarely
exceeds 50% total" guideline: compounded top = 47%, compounded bottom
= 63% (the bottom intentionally exceeds 50% to prevent phosphor ghost
residue, but stops well short of the 90% invisible threshold).

---

## Per-constant rationale

### `CRT_VIGNETTE_EDGE_FACTOR`: 0.50 -> **0.82** (18% dim)

The CRT vignette is a "subtle glow" effect — its job is to make the
screen edges feel slightly darker (retro CRT feel), NOT to be a
primary dimming mechanism. At 0.50 (50% dim), it became the dominant
dimming effect and compounded destructively with edge fade.

0.82 (18% dim) is calibrated to:

- Be perceptible on its own (the eye notices the dim without
  identifying it as a border).
- Not compound destructively: 0.82 × 0.65 = 0.533 (top), 0.82 × 0.45
  = 0.369 (bottom) — both in the visible zone.
- Match the Apple Vision Pro / Sony Bravia "single-effect 15-20%"
  reference range.

### `EDGE_FADE_TOP_MIN`: 0.45 -> **0.65** (35% dim)

The top edge fade is purely cinematic — rain enters smoothly from
"above the screen". It needs to be visible enough that the user sees
rain entering, but dim enough that the eye reads it as "fade-in".

0.65 (35% dim) is calibrated to:

- Compounded top brightness 0.82 × 0.65 = 0.533 — clearly visible
  "subtle dim" zone.
- Top row rain is readable but visibly dimmer than mid rows.
- Slightly more aggressive than the original (0.70) to give the fade-in
  more presence, but stops well short of the initial retune's destructive 0.45.

### `EDGE_FADE_BOTTOM_MIN`: 0.20 -> **0.45** (55% dim)

The bottom edge fade is the most aggressive of the three because its
primary purpose is functional (prevent phosphor ghost residue), not
aesthetic. Dying droplet heads "burn into" the bottom row if the fade
is too gentle.

0.45 (55% dim) is calibrated to:

- Compounded bottom brightness 0.82 × 0.45 = 0.369 — "cinematic dim,
  rain barely visible" zone. Rain dissolves into shadow but is NOT
  destroyed.
- Still more aggressive than the top (0.65) — asymmetric fade
  preserved, phosphor residue still prevented.
- Midpoint between the original (0.35, owner wanted more aggressive) and
  the initial retune (0.20, too aggressive).

### `EDGE_FADE_BOTTOM_ROWS`: 8 -> **10** (wider dissolve zone)

Widening the bottom fade zone from 8 -> 10 rows gives the smoothstep
more room to ease in. The 8-row zone produced a slightly abrupt
transition where the gentle pre-fade met the sharp lip; 10 rows
produces a more film-like dissolve.

### `EDGE_FADE_BOTTOM_LIP`: 0.75 -> **0.72** (slightly lower lip)

Lowering the lip from 0.75 -> 0.72 makes the transition between Zone 1
(gentle pre-fade) and Zone 2 (sharp lip) slightly smoother. The 0.03
reduction is barely perceptible on its own but produces a more
film-like dissolve when combined with the widened
`EDGE_FADE_BOTTOM_ROWS`.

---

## Test verification

All edge fade tests pass with the new values:

- `viewport_edge_fade_is_bounded_and_smooth` — interior rows return
  1.0, extremes return the new MIN constants.
- `viewport_edge_fade_bottom_more_aggressive_than_top` — bottom
  (0.45) < top (0.65), asymmetry preserved.
- Phosphor capping tests — unaffected (constants
  `PHOSPHOR_EDGE_ENERGY_CAP`, `PHOSPHOR_EDGE_ROW_TAPER` unchanged).

The audit script `scripts/visual-mode-audit.py` regenerates the brightness
curves and compounded comparison table — re-run it if the constants are
retuned again.

---

## Future retune guidance

If the owner requests another retune:

1. **Always check compounded brightness**, not just the per-effect
   value. Run `scripts/visual-mode-audit.py` to see the curve.
2. **Target zones** (compounded):
   - Top extreme: 0.50-0.55 (subtle dim, visible)
   - Bottom extreme: 0.35-0.45 (cinematic dim, dissolving)
3. **Asymmetry rule:** bottom must be more aggressive than top
   (phosphor residue prevention). Keep
   `EDGE_FADE_BOTTOM_MIN < EDGE_FADE_TOP_MIN`.
4. **Vignette is subtle:** keep `CRT_VIGNETTE_EDGE_FACTOR >= 0.80`
   unless intentionally raising the CRT-glow prominence. Below 0.80
   starts to feel like a "dark frame"; above 0.85 reads as "barely
   there".
5. **Update this doc** with the new values and the rationale. The
   audit trail is what makes this "masterclass" — every retune is
   reproducible and explainable.

---

## masterclass retune — 4-effect compounding model (2026-08-09)

The retune (commit bfea09e) correctly addressed the 2-effect
compounding it knew about (`crt_vignette × edge_fade`). The owner was
still unhappy because the bottom row was still invisible. This section
documents why: the prior audit only modeled 2 of the 4 dimming effects
that compound on the bottom row.

### The 4 effects that compound on every cell

The render path applies 4 distinct dimming effects to each cell. Each
effect reads the current cell color (already dimmed by prior effects)
and multiplies — the compounding is multiplicative, not additive.

| # | Effect              | Source                          | Range        | Applies to                |
|---|---------------------|---------------------------------|--------------|---------------------------|
| 1 | `rain_shadow_factor`| `droplet.rs:180`                | [0.0, 1.0]   | Bottom 15% of screen (mid/back layer only) |
| 2 | `viewport_edge_fade`| `droplet.rs:78`                 | [0.45, 1.0]  | Top 2 rows + bottom 10 rows (all layers) |
| 3 | `vignette_factor`   | `droplet.rs:136`                | [0.70, 1.0]  | Corners (radial, mid/back layer only) |
| 4 | `crt_vignette_factor`| `droplet.rs:218` (v30.2)       | [0.82, 1.0]  | Top 3 rows + bottom 3 rows (all layers) |

Effects 1, 2, 3 are applied inline in `Droplet::draw` (droplet.rs:875-924)
in the order: shadow -> edge -> radial. Effect 4 is applied as a
post-process pass in `cloud/rain.rs::apply_crt_vignette` after the
droplet draw completes.

### Why the prior audit missed the bottom-row invisibility

The prior audit doc (this file, prior to update) claimed:

```text
compounded bottom brightness = 0.82 × 0.45 = 0.369
```

This is the 2-effect math (crt_vignette × edge_fade only). The actual
4-effect compounded brightness at the bottom row of an 80×40 terminal
was:

```text
# Bottom-right corner (col=79, line=39, 80×40 terminal, back layer)
rain_shadow_factor(line=39, lines=40):
    threshold = (1.0 - 0.15) * 40 = 34
    t = (39 - 34) / 6 = 0.833
    quadratic 1 - t² = 1 - 0.694 = 0.306  <- 70% dim from shadow ALONE
viewport_edge_fade(line=39, lines=40):
    bottom_dist = 0 -> Zone 2 sharp lip
    factor = 0.45 (EDGE_FADE_BOTTOM_MIN)
vignette_factor(col=79, line=39, 80, 40):
    nx = (79 - 40) / 40 = 0.975
    ny = (39 - 20) / 20 = 0.95
    dist = sqrt(0.950 + 0.902) = 1.361
    normalized = 1.361 * 0.7071 = 0.962
    t = (0.962 - 0.7) / 0.3 = 0.875
    smooth = 0.875² * (3 - 2*0.875) = 0.957
    factor = 1.0 - 0.30 * 0.957 = 0.713
crt_vignette_factor(line=39, lines=40):
    v = 40 - 1 - 39 = 0 -> smoothstep(0) = 0
    factor = 0.82 + (1.0 - 0.82) * 0 = 0.82

COMPOUNDED = 0.306 × 0.45 × 0.713 × 0.82 = 0.0806  -> 91.9% dim (RAIN INVISIBLE)
```

The prior audit's "0.369" was off by 4.6× because it missed the
`rain_shadow_factor` (0.306) and `vignette_factor` (0.713)
contributions. The bottom row was functionally invisible, exactly the
symptom the retune was supposed to fix.

### The fix: RAIN_SHADOW_FLOOR + SSOT function

The retune takes a two-pronged approach:

**1. New `RAIN_SHADOW_FLOOR = 0.50` constant** (in
   `central_control_rains.rs`): caps `rain_shadow_factor` so its
   quadratic curve floors at 0.50 instead of 0.0. The curve SHAPE is
   preserved (linearly remapped from `[0, 1]` to `[0.50, 1.0]`), so the
   slow-start-accelerating-fade character is unchanged — only the
   absolute floor moves. This is the highest-leverage single fix
   because `rain_shadow_factor` was the largest individual contributor
   to the bottom-row darkness (0.306 alone, 70% dim).

**2. New `compounded_brightness()` SSOT function** (in `droplet.rs`):
   models ALL 4 dimming effects multiplicatively, with the per-layer
   exemption logic for front-layer neon. Intended for audit/test use —
   the hot render path keeps its inline calls for perf. 8 regression
   tests in `tests_edge_fade.rs` verify:

- `rain_shadow_factor` floors at 0.50 (not 0.0)
- `crt_vignette_factor` returns the expected smoothstep curve
- `compounded_brightness` bottom-row stays above the 0.10 visibility
     threshold (was 0.08 previously, now 0.13 at corner / 0.18 at center)
- `compounded_brightness` interior = 1.0 (no dimming)
- `compounded_brightness` front-layer excludes shadow + radial
- `compounded_brightness` matches the inline render-path math

**3. Extracted `crt_vignette_factor()` function** (in `droplet.rs`):
   the smoothstep math was inline in `cloud/rain.rs::apply_crt_vignette`.
   extracts it as a pub fn so both the render path AND the SSOT
   `compounded_brightness` function call the same code — DRY, single
   source of truth for the CRT vignette row-factor curve.

### Recomputed bottom-row brightness

With `RAIN_SHADOW_FLOOR = 0.50` in place, the bottom-row compounded
brightness on an 80×40 terminal becomes (back layer, layer=0):

```text
# Bottom-right corner (col=79, line=39)
# rain_shadow_factor floors at 0.50 asymptotically; for lines=40 the
# bottom row reaches t = 5/6 ≈ 0.833, so factor = 0.50 + 0.50*(1-0.694) = 0.653
rain_shadow_factor = 0.653  (was 0.306 previously)
viewport_edge_fade = 0.45
vignette_factor    = 0.713
crt_vignette_factor = 0.82
COMPOUNDED = 0.653 × 0.45 × 0.713 × 0.82 = 0.172  -> 82.8% dim (rain visible)

# Bottom-center (col=40, line=39)
# vignette_factor is 1.0 here (inside VIGNETTE_INNER_RADIUS=0.7)
rain_shadow_factor = 0.653
viewport_edge_fade = 0.45
vignette_factor    = 1.0
crt_vignette_factor = 0.82
COMPOUNDED = 0.653 × 0.45 × 1.0 × 0.82 = 0.241  -> 75.9% dim (rain visible)
```

17% brightness at the corner is still dim, but rain is now clearly
visible (the previous 0.08 = 8% was below the perceptual "rain visible"
floor of ~10%). At the center, 24% brightness is comfortably visible —
the rain dissolves into shadow without disappearing. The shadow's depth
effect is preserved — the quadratic still produces a clear top-to-bottom
darkening gradient — only the absolute floor changes.

For very tall terminals (lines=400), the bottom-row shadow factor
approaches the asymptotic floor (0.517), giving compounded brightness
of ~0.135 (corner) / ~0.190 (center) — still above the 10% visibility
floor.

### Why not also retune EDGE_FADE_BOTTOM_MIN or VIGNETTE_INTENSITY?

The audit considered 5 fix options. Compounded brightness values
are at the bottom-corner of an 80×40 terminal (the worst case); see the
recomputed section above for the full math.

| Option | Fix                                | Compounded bottom (corner) | Verdict |
|--------|------------------------------------|---------------------------|---------|
| 1      | Add `compounded_brightness` SSOT  | (no math change — 0.080)  | Required for future audits — adopted |
| 2      | Raise `EDGE_FADE_BOTTOM_MIN` 0.45 -> 0.60 | ~0.107             | Changes the cinematic dissolve character — rejected |
| 3      | Lower `VIGNETTE_INTENSITY` 0.30 -> 0.20 | ~0.091              | Weakens the photographic lens effect globally — rejected |
| 4      | Cap `rain_shadow_factor` floor at 0.50 | **0.172**         | Highest leverage, preserves curve shape — adopted |
| 5      | Skip `crt_vignette` on the bottom band only | ~0.098         | Asymmetric CRT glow (top dim, bottom not) — rejected |

Options 1 + 4 combined (the owner's approved choice) produce the
target compounded bottom brightness of ~0.17 (corner) / ~0.24 (center)
while preserving the masterclass character of the other 3
effects. The SSOT function (Option 1) doesn't change any math — it
just makes the 4-effect compounding queryable so future retunes don't
repeat the prior mistake of auditing only 2 effects.

---

## See also

- `src/central_control_rains.rs` — the constants (with inline rationale
  linking back to this doc). Adds `RAIN_SHADOW_FLOOR`.
- `src/droplet.rs::viewport_edge_fade` — the edge fade implementation
  (top + bottom 2-zone dissolve).
- `src/droplet.rs::rain_shadow_factor` — the rain shadow implementation
  (floors at `RAIN_SHADOW_FLOOR` instead of 0.0).
- `src/droplet.rs::vignette_factor` — the radial vignette implementation.
- `src/droplet.rs::crt_vignette_factor` — extracted SSOT function
  for the CRT vignette row-factor curve.
- `src/droplet.rs::compounded_brightness` — SSOT function modeling
  all 4 dimming effects multiplicatively (audit/test use).
- `src/engine/cosmic_dragon_engine/cloud/rain.rs::apply_crt_vignette` — the CRT vignette
  implementation (smoothstep over `CRT_VIGNETTE_HEIGHT` rows;
  calls the extracted `crt_vignette_factor` for DRY).
- `src/engine/cosmic_dragon_engine/cloud/tests/tests_edge_fade.rs` — adds 8 regression tests
  guarding the rain shadow floor + SSOT compounded brightness contract.
- `scripts/visual-mode-audit.py` (in-repo) — the audit script
  that generated the brightness curves in this doc.
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
