<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Color Space Master Research — Is Anything Beyond OKLab Worth It? (NIGHT-research-3, 2026-09-05)

Owner question (master research mode): "besides OKlab / chroma dragon,
what other color [science] is the most valuable for cosmostrix peak?
If already peak, just skip it and document why cosmostrix uses OKLab
as the primary instead of the alternatives."

## 0. TL;DR verdict

**The color space is at peak. Skip every alternative. OKLab stays the
primary.** The three "beyond the space" candidates an external review
usually proposes (gamut mapping, wide-gamut P3, adaptive chroma) were
assessed against the actual code and measured data:

- **Gamut mapping** — measured at effectively zero value on the shipped
  themes (section 5): worst real-theme hue deviation vs a full CSS-style
  chroma-reduction mapper is 3.69° on Rainbow and ≤0.33° on everything
  else, on a build-time-only path whose output was hand-approved with
  the clamp in effect. Swapping the clamp for chroma reduction would
  *change* every theme's appearance — a regression against the locked
  visual identity, not an improvement.
- **Wide gamut (P3)** — not actionable at all: SGR 38;2 truecolor is
  defined as sRGB, and no escape sequence exists for specifying P3 text
  cells. Authoring wide-gamut would be unrepresentable in the terminal.
- **Adaptive chroma** — the parts that matter already exist in a better
  form: the palette-relative brightness floor (Phase 7) keeps dark
  themes visible while preserving hue, and capability detection already
  switches the whole pipeline by color depth.

The genuinely valuable "beyond" work is not a different space — it is
*where* the math runs, and cosmostrix already runs it in the right
places: perceptual science at palette-build time, precomputed integer
stops on the 12.9M-cells/sec render path (section 2). That placement is
the design win, and it is already shipped.

## 1. What the pipeline actually does (source-verified)

| Concern | Implementation | Where it runs |
|---|---|---|
| Palette gradients | OKLab + polar chroma lerp + shortest-arc hue (`gradient/mod.rs::gradient_from_stops_oklab`) | build time, ~50 ns/stop |
| Intro animation blends | `oklab_blend_rgb` (same polar semantics) | intro timeline, not the rain loop |
| Round-trip fidelity | grid test ≤1 LSB (`test/.../gradient/tests.rs::round_trip_within_one_unit`); one-off exhaustive f64 replication of the same matrices: 16,777,216/16,777,216 colors with max channel error **0** | test time |
| Dark-theme visibility | palette-relative brightness floor, hue-preserving channel-uniform scale (`palette/mod.rs::apply_palette_relative_floor`) | build time |
| Banding mitigation | 4×4 Bayer ordered dither on the cinematic shading parameter (`shaders/base/helpers.rs::bayer_threshold`) | render path |
| Slow hue life | `hue_drift` as a palette-stop **index offset** (integer add, zero color math) | render path |
| 256/16-color fallback | nearest cube/gray or table entry at construction (`palette/mod.rs::rgb_to_ansi256`, `rgb_to_color16`) | build time |
| Legacy pipeline | explicit integer RGB math module (`legacy.rs`) for `Color256/16/Mono`, per the owner directive "chroma dragon first -> fallback legacy" | render path |
| User custom colors | `colors-custom` routes through the same 9-step OKLab polar engine (`colors_custom.rs`) | build time |

Note what is deliberately absent: no color-space math per cell, ever.
Stops are resolved by index; brightness effects are integer blends; hue
drift is an integer stop offset. The perceptual machinery runs once per
palette load. This is why "which color space" is a build-time-only
question for cosmostrix — the render loop could not care less.

## 2. Why OKLab is the right primary (the requested documentation)

1. **Perceptual uniformity where cosmostrix needs it.** Euclidean
   distance in OKLab tracks perceived difference, so gradient steps are
   evenly spaced by eye. The failure mode it replaced — per-channel
   sRGB/linear interpolation — produces muddy brown/gray midpoints on
   hue-distant stops (documented in `gradient/mod.rs` with the
   red→green / blue→yellow repro tests).
2. **Hue linearity in the region cosmostrix lives in.** The classic
   CIELAB defect is blue curvature: blue→cyan gradients bend through
   purple. cosmostrix's palette catalog is dominated by blue/cyan/green
   families (Blue, Ocean, Cosmos, NeonCyan, energy themes) — exactly
   the region where CIELAB is at its worst and OKLab was designed to
   be straight.
3. **Polar form gives the actual gradient semantics.** Interpolating
   chroma magnitude and rotating hue through the shortest arc (the
   OkLCh view of OKLab) avoids the "Cartesian shortcut through gray" on
   opposing-hue pairs — the same default the W3C CSS Color Level 4 spec
   chose for `oklch()` interpolation. Users get the same gradient
   behavior they see on the web.
4. **Cost profile fits the placement.** ~12 multiplies + 3 cbrt per
   stop transition, build-time only. Heavier perceptual spaces (CAM16
   family) would buy nothing visible after 8-bit sRGB quantization and
   would still have to run at the same build-time-only cadence.
5. **Industry convergence.** OKLab is the interpolation space of CSS
   Color 4 (all evergreen browsers), and every major color tooling
   stack followed. Choosing it is choosing the semantics users already
   perceive as "correct gradients".
6. **Verifiable exactness.** The transform pair round-trips the full
   sRGB cube with zero error at f64 and ≤1 LSB at f32 (the documented
   f32→u8 rounding floor). No approximation is smuggled in.

## 3. The alternatives, and why each loses (SDR 8-bit emissive terminal context)

| Space | Verdict | Concrete rejection reason |
|---|---|---|
| CIELAB | reject | blue-hue curvature (worst exactly in cosmostrix's blue/cyan heartland); D50-vs-D65 mismatch needs Bradford adaptation; more code, worse gradients |
| CIELUV | reject | older, worse hue uniformity in reds/greens; no gradient advantage over OKLab anywhere |
| JzAzBz | reject | tuned for HDR luminance ranges a terminal never emits; heavier transfer functions; independent hue-linearity evaluations put OKLab equal-or-better at SDR |
| ICtCp | reject | Dolby PQ-based HDR *video coding* transform; heavy, SDR-misaligned, gamut mapping semantics not suited to palette interpolation |
| CAM16-UCS | reject | needs viewing-condition assumptions (surround luminance) that vary wildly across terminals; ~10× the math; perceptual gain lands below the 8-bit quantization floor |
| HSL/HSV | reject | non-perceptual; hue interpolation midpoints desaturate; already historically displaced here |
| Okhsl/Okhsv | N/A | picker-oriented gamut-mapped views of OKLab; cosmostrix has no interactive color picker surface (`--color-tune sat` is an RGB-space knob and adequate) |
| Oklch | already have it | not an alternative — the polar interpolation IS the OkLch view, already implemented |
| linear sRGB | reject for interpolation | physically right for *adding* light, wrong for *interpolating* between stops (the muddy-midpoint failure); post-FX blends toward white already approximate the additive case appropriately |

## 4. Wide gamut (P3) — correcting the "medium value" prior

An external review rated Display-P3 authoring "medium" value. In the
terminal model it is **not actionable**: SGR 38;2;r;g;b is specified in
sRGB, there is no escape sequence to request P3 for text cells, and
each terminal internally maps whatever it receives onto its own display
gamut. A cosmostrix authored in P3 would be quantized back to sRGB
bytes at the exact same boundary where it quantizes today. The only
wide-gamut surfaces in terminals (kitty's graphics protocol, sixel)
carry images, not rain cells. Skip — permanently, not tentatively.

## 5. Gamut mapping — measured, not guessed

**Method.** `benchmark/research/oklab_gamut_probe.py` replicates the
production gradient math (same matrices, same polar lerp, same 9 steps)
in Python and, for every interpolated sample that exits sRGB, compares
the shipped per-channel clamp against a CSS-style chroma-reduction
gamut map (binary search on C at fixed L and hue).

**Results on real catalog stops:**

| Theme | Samples clipped | Max linear overshoot | Hue shift, clamp vs chroma-reduce | L shift |
|---|---|---|---|---|
| Blue | 3/9 | 0.0004 | 0.33° | 0.0003 |
| Ocean | 1/9 | 0.0000 | 0.10° | 0.0001 |
| Rainbow | 4/9 | 0.0393 | 3.69° | 0.0077 |
| Cosmos | 2/9 | 0.0008 | 0.01° | 0.0001 |

Worst-case synthetic opposing-hue pairs (red↔cyan, blue↔yellow — a
pattern no shipped theme uses) reach 18–26° of hue shift under the
clamp, which is the number that makes gamut mapping look valuable in
the abstract.

**Reading.** For every theme except Rainbow the clamp-vs-reduce
difference is at or below one-tenth of a degree — invisible under
8-bit quantization by construction. Rainbow's 3.69° on 4 of 9 samples
is measurable, but the shipped Rainbow look was hand-tuned *with* the
clamp in effect: the gradient everyone approved already contains the
clamped midpoints. Replacing the clamp now would silently re-shade a
locked theme to "fix" a distortion nobody reported. That is a
regression dressed as an improvement.

**The one latent case:** user-defined `colors-custom` palettes can
specify saturated opposing-hue stops and hit the 18–26° regime in
their midpoints. Even there the effect is cosmetic, build-time, and
fully user-controlled (the user picked both endpoints).

**Revisit trigger (documented, not implemented):** add build-time
chroma reduction in `oklab_to_srgb` *only if* custom-palette users
report muddy/gray midpoints on saturated opposing-hue stops. The cost
would be trivial (a 24-step binary search per clipped sample,
build-time only), which is exactly why it is safe to defer — the
decision is reversible in an afternoon and blocked on real demand, not
on engineering risk.

## 6. What about the 256-color quantization distance?

`rgb_to_ansi256` picks the nearest 6×6×6 cube corner or grayscale ramp
entry in RGB Euclidean distance. An Oklab-distance lookup would be
more perceptual in principle — but the 6-level cube itself quantizes
each axis to 5 steps; the distance metric's contribution is second
order next to the cube granularity, the whole path is the *legacy*
pipeline by owner directive (Color256 → `LegacyRgb`), and changing the
mapping would re-shade every 256-color theme. Skip, for the same
locked-aesthetic reason as section 5.

## 7. Conclusion

The chroma dragon's color stack is at peak for its medium: OKLab (with
polar OkLCh interpolation) at build time, integer stop-index math at
render time, Bayer-dithered shading, a palette-relative visibility
floor, an explicit legacy fallback, and a measured-not-guessed answer
to the gamut question. No alternative color space survives contact
with the constraints (SDR 8-bit sRGB output, emissive black background,
hand-tuned 9-stop palettes, zero color math in the hot loop). The
owner directive stands confirmed: OKLab is the primary, everything
else is either already implemented in a better form or not actionable
in a terminal.

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
