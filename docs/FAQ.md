<!-- SPDX-License-Identifier: GPL-3.0-only -->

# cosmostrix FAQ — Q/A

Answers to recurring configuration and rendering questions, written
once so the answer never drifts. Each entry names the source files
that implement it — source code is the single source of truth; if a
doc and the code disagree, the doc is wrong.

Index:

- [Colors & OKLab](#colors--oklab)
- [Custom blocks](#custom-blocks)

## Colors & OKLab

### Q: Does cosmostrix use OKLab when I define a custom color palette in config.toml?

A: YES. A `[colors-custom.<name>]` block routes through exactly the
same OKLab polar gradient engine as every built-in theme — there is
one palette pipeline, not two:

```text
[colors-custom.<name>] rain stops
  -> CustomPaletteDef::to_palette()            (colors_custom.rs)
  -> colors_from_stops(TrueColor, stops, 9)    (chroma palette/mod.rs)
  -> gradient_from_stops_oklab(stops, 9)       (chroma gradient/mod.rs)
  -> 9 perceptually-uniform palette samples
```

The user's raw stops (2–9 of them) are gradient control points; the
engine resamples the gradient to exactly 9 samples
(`COLORS_CUSTOM_PALETTE_STEPS` = 9) before the palette is used. This
eliminates banding on long rain trails and puts custom palettes on
identical footing with built-in themes like `synthwave` or `cosmos`
(the byte-identity is pinned by the
`to_palette_matches_builtin_gradient_path` test). Before the
masterclass round this went through, `to_palette()` returned the raw
stops verbatim — a 3-stop palette stayed 3 entries while every
built-in theme carried 9 OKLab-interpolated entries; that asymmetry
is gone.

### Q: Why is the rain-stop maximum 9?

A: Because 9 is exactly what the engine keeps. The OKLab resample
step produces 9 samples regardless of how many stops you write, so
stops beyond 9 are provably discarded input. Before NIGHT-hunt-37
(2026-09-13) the documented limit was 64 and the collector enforced
it as a silent truncation — a 67-stop block ran with 64 stops and no
error. The contract is now: min 2 stops (a gradient needs two
colors), max 9, and an over-limit block is a HARD error on every
surface (startup exit 2, `--testconf` exit 2, live-reload watcher
reject), never a silent cap. 7 stops is the sweet spot the config
template suggests.

### Q: What is OKLab and why does cosmostrix interpolate in it?

A: OKLab (Björn Ottosson, 2020) is a perceptually uniform color
space: Euclidean distance between two colors matches how different
humans perceive them. Interpolating in plain sRGB/linear-RGB rotates
hue through the desaturated center of the RGB cube on hue-crossing
gradients (red to green, blue to yellow), producing muddy brown/gray
midpoints. OKLab keeps the midpoint saturated.

cosmostrix additionally interpolates the chroma axes polar (chroma
magnitude lerped linearly, hue rotated through the shortest arc)
rather than Cartesian — on opposing-hue gradients the Cartesian
midpoint passes near (a, b) = (0, 0) = gray; polar stays saturated.
This matches the W3C CSS Color Module Level 4 default for `oklch`
interpolation. Polar is the sole production gradient path (the
legacy sRGB-linear and Cartesian variants were removed in v30).
Full rationale with examples: the module docs of
`src/engine/chroma_dragon_engine/gradient/mod.rs`.

### Q: Does OKLab apply to built-in themes too?

A: Yes. Every gradient-style built-in theme is declared as
`ThemeColors::Stops { stops, steps: 9 }` in
`src/engine/chroma_dragon_engine/catalog/themes.rs` and routes
through the same `colors_from_stops` →
`gradient_from_stops_oklab` chain (see `catalog.rs`). Fixed-list
themes (explicit color tables) skip the gradient step by design.

### Q: Where else does OKLab appear?

A: Four production surfaces, all in the Chroma Dragon engine:

1. Palette gradients (this doc's main subject).
2. The intro cinematic — color transitions during the singularity to
   burst to morph to rain sequence use `oklab_blend_rgb`
   (`gradient/mod.rs`, driven by `intro_colors.rs`).
3. Palette transition smoothing — when the active palette changes
   (live reload, ambient phase switch, crystal drift), the old and
   new colors are blended in OKLab and L is smoothed over the
   transition table (`shaders/transition/mod.rs`).
4. The 300 ms top-to-bottom palette wave — the Crystal Dragon's
   ambient/theme switches trigger the Chroma Dragon's OKLab wave
   transition (`shaders/transition/`, documented in
   `docs/CRYSTAL_DRAGON_ENGINE.md`).

### Q: What happens to my custom palette on a terminal without TrueColor?

A: The palette is still built through OKLab first; the final samples
are then quantized to the terminal's mode (`colors_from_rgb` in
`chroma palette/mod.rs`): Color256 maps each sample to the nearest
xterm-256 entry, Color16 maps to the nearest of 16, Mono collapses
to white. Custom palettes are user-defined 24-bit hex by definition,
so TrueColor is the intended mode — on weaker modes you get the
closest representable rendering of the same perceptual gradient, and
mono loses the palette entirely (by design: mono rain is white).

## Custom blocks

### Q: What makes a custom block "complete"?

A: Since NIGHT-hunt-37 the contract is strict and identical on every
surface (startup, `--testconf`, the live-reload watcher):

- `[colors-custom.<name>]` must define BOTH `bg` and `rain` (the
  deprecated `stops` alias satisfies the rain slot, but `rain` and
  `stops` together is an overload error). Values must be valid hex.
- `[charset-custom.<name>]` must define `set`, non-empty.
- `[scene-custom.<name>]` must define all seven dimensions: `rain`,
  `color` or `colors-custom`, `charset` or `charset-custom`, `fps`,
  `speed`, `density`, `glitch-level`.
- A header-only block (every field line commented out or missing) is
  a hard error — the parser records custom-block headers precisely
  so these blocks cannot hide.
- Duplicate keys, duplicate section headers, and unknown fields are
  hard errors (unknown keys carry did-you-mean hints).
- Names: 1–64 chars, letters/digits/`-`/`_`. Max 100 blocks per
  namespace.

The bounds table with rationale lives in `docs/RULES.md` ("Custom
Block LTS Bounds"). The validators: `colors_custom::strictness`,
`testconf::custom_block_headers`, `scene_custom` completeness, and
the per-namespace name-length and block-count checks.

### Q: I edited config.toml while cosmostrix was running and it kept the old colors. Why?

A: Mid-run edits are validated by the live-reload watcher with the
same strictness as startup; a rejected edit keeps the last valid
config running (the rejection is logged — run with `--verbose` to
see it). Keys that cannot change mid-run (documented in
`docs/LIVE_RELOAD_BEHAVIOR.md`) need a restart. The honest
limitations of live reload are documented in
`docs/CONFIG_LIVE_RELOAD_DISCLAIMER.md`.
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
