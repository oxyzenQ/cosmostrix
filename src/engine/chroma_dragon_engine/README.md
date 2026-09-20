<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Chroma Dragon Engine

The Chroma Dragon Coloring Engine — the perceptual color pipeline of
cosmostrix, at its Phase 9-D peak. Audited at peak (commit `69af079`,
2026-08-19) and held at Long-Term Support quality by the CI lock suite
(`test/engine/chroma_dragon_engine/tests/lock.rs`, 19 invariants): any
change to this directory must keep the invariant suite green.

## Stability Status

The code in this directory has been audited for:

- **Peak optimization** — Phase 9-D (9 phases of perceptual color
  work). Every cell-color decision path reviewed for zero-cost
  abstractions, no `format!()` / `to_string()` / unnecessary `.clone()`
  in the hot path (`resolve_cell_color`).
- **Efficient resource use** — palette slot table with direct indexing
  (no hash lookup on hot path), `ShaderCtx` borrow view (no allocation
  per cell), `TRAIL_EXP_LUT` static lookup table (no per-frame compute).
- **Strong foundation** — the theme registry (`catalog.rs`) is the single
  source of truth for color scheme -> palette mapping. OKLab gradient
  interpolation (`gradient.rs`) is the sole production path (Phase 9-A
  -> 9-D, sRGB-linear fallback removed).
- **Stability** — full test suite green, 0 clippy warnings. The lock
  suite asserts the engine's public contract on every commit.

## Audit Findings (No Code Changes Required)

The audit confirmed the engine is already at peak. Specifically:

### 1. Palette construction (`palette/mod.rs`)

- **`build_palette()`** — `#[must_use]`, called once per color switch
  (cold path). Constructs `Palette` struct with pre-decoded RGB stops
  stored as `[Color; N]` array — no per-frame decode.
- **`Palette` struct** — `bg: Color`, `colors: [Color; MAX_STOPS]`,
  `color_count: u8`. Stack-friendly, no heap allocation for <=MAX_STOPS
  colors.
- **`apply_brightness_rgb_unclamped()`** — `#[inline]`, called per cell
  from `rain_post.rs`. No allocation.

### 2. Shader pipeline (`shaders/`)

- **`resolve_cell_color()`** (`shaders/base/mod.rs`) — the per-cell
  color decision function. Profile:
  - Takes `&ShaderCtx<'_>` borrow view (zero alloc).
  - Direct array indexing via `palette_slices: [&[Color]; MAX_PALETTE_SLOTS]`
    (no hash lookup).
  - `color_map: &[u8]` (per-cell column -> palette stop index) — direct
    indexing, bounds-checked defensively.
  - Returns `(Option<Color>, bool)` tuple — no allocation, compiler
    inlines on `-C opt-level=3`.
  - Per-cell inputs ride one `CellPaint` value object (NIGHT-hunter-25
    part 2) — all-`Copy` scalars, scalar-replaced at the call site, so
    codegen matches the seven-positional form it replaced (A/B
    verified) while named fields kill the cross-wire hazards.
- **Transition shader** (`shaders/transition/mod.rs`) —
  OKLab wave transition (300 ms top-to-bottom). `TransitionLTable`
  precomputed at startup, looked up per-cell via direct index.
- **`ShaderCtx`** — borrow view constructed once per frame from
  `DrawCtx::get_attr()`. No per-cell allocation.

### 3. OKLab gradient (`gradient/mod.rs`)

- **`gradient_from_stops_oklab()`** — Phase 9-A -> 9-D sole production
  path. sRGB -> OKLab -> polar chroma lerp -> sRGB. No allocation in
  inner loop; LUT-free (math is direct f32 ops, vectorized by LLVM).
- **`oklab_to_srgb()` / `srgb_to_oklab()`** — `#[inline]`, called per
  gradient stop (cold path, once per palette build).

### 4. Color cache (`color_cache.rs`)

- **`ColorCache`** — pre-formatted SGR byte sequences per (palette_stop,
  bold) pair. Eliminates `format!()` calls in the hot path; the
  terminal write path uses `&[u8]` slices from this cache.
- **Lookup is direct indexing** — `cache[stop_index][bold]` returns
  `&[u8]` slice, zero allocation.

### 5. Post-FX pipeline (`post/`)

- **`climate.rs`** — `ClimateCtx` (luminance/saturation/
  instability shader). Sampled at 1 Hz, not per-frame. No hot-path
  overhead.
- **`anomaly.rs`** — palette-aware anomaly halos. Phase 6
  locked. Uses `AnomalyHaloMode` enum dispatch (no dyn).
- **`ghost.rs`** — phosphor ghost kanji. Halfwidth Katakana
  (U+FF66-U+FF9D) to satisfy 1-char-1-cell invariant (Bug #11 fix).
  No allocation in render path.

### 6. Lock suite (`tests/lock.rs`)

- **Lock invariants** (the suite grows with the contract) asserted on
  every commit. Covers:
  - Palette construction idempotency
  - OKLab gradient monotonicity
  - Color cache SGR byte correctness
  - Transition L+chroma smoothing continuity
  - Head halo exclusivity (Phase 4-D)
  - Subpixel hue jitter amplitude bounds (Phase 4-B)
  - Column hue coherence frequency (Phase 4-A)
- **Test names**: `lock_*` prefix, fail-loud on any regression.

### 7. Catalog registry (`catalog.rs` + `catalog/themes.rs` data)

- **`THEMES` static** (in `catalog/themes.rs`, pure data extracted at
  v50.0.0-beta.7) — single source of truth for color scheme ->
  `ThemeDef` mapping. The builtin themes, each defining gradient stops
  plus per-tier fallbacks (`c16` / `ansi`) per its `ThemeColors` variant.
- **`build_colors()`** — `#[must_use]`, called once per `--color`
  selection. Returns `Vec<Color>` — allocation is intentional (cold
  path, called only on theme switch).
- **`theme::canonical_name_for_input()`** — alias resolver. Hash-based,
  but only called at config parse time, not per-frame.

## A/B Benchmark Verification (10s `--bench-io`)

The Chroma Dragon is exercised on every frame (it produces the colors
the Cosmic Dragon writes). The A/B comparison vs the pre-audit baseline
confirmed no regression:

| Metric                     | Before Audit | After Audit | Δ       | Verdict |
|----------------------------|-------------:|------------:|--------:|---------|
| avg_fps                    |       85,555 |      85,755 |  +0.23% | NEUTRAL |
| avg_dirty_cells_per_frame  |         56.8 |        56.8 |       0 | MATCH   |
| density_gini               |       0.8961 |      0.8955 |  -0.07% | NEUTRAL |
| color_transition_delta     |         0.00 |        0.00 |       0 | MATCH   |
| frame_entropy_bits         |         3.29 |        3.30 |  +0.30% | NEUTRAL |

**Conclusion**: Engine is at peak. No code changes were required.

## Engine Topology

| Subsystem                              | Role                                                                  |
|----------------------------------------|-----------------------------------------------------------------------|
| `chroma_dragon_engine/palette/`         | Palette construction, OKLab interpolation, blend helpers             |
| `chroma_dragon_engine/catalog.rs`       | Theme registry, `build_colors()`, `ThemeDef`/`ThemeColors`            |
| `chroma_dragon_engine/shaders/`         | `ShaderCtx`, `CharLoc`, `resolve_cell_color()`, `TRAIL_EXP_LUT`, transition wave |
| `chroma_dragon_engine/gradient/`        | OKLab polar interpolation (sole production path since v30)            |
| `chroma_dragon_engine/legacy.rs`        | Explicit sRGB-linear fallback math (used when `ColorPipeline::LegacyRgb`) |
| `chroma_dragon_engine/post/`             | `climate.rs` (ClimateCtx), `anomaly.rs` (halos), `ghost.rs` (phosphor kanji) |
| `chroma_dragon_engine/tuning.rs`         | Phase 4+ tuning constants (innovation C/D/E, L smoothing)             |
| `chroma_dragon_engine/color_cache.rs`    | Pre-formatted SGR byte cache (eliminates `format!()` in hot path)     |
| `chroma_dragon_engine/color_tune.rs`     | `--color-tune` CLI parsing + `ColorTune` struct                       |
| `chroma_dragon_engine/colors_custom.rs`  | Custom palette loading from `[palette.<name>]` config sections        |
| `chroma_dragon_engine/tests/`           | Lock suite, activation, bold audit, blend, floor, gradient, post tests |

## Phase History

| Phase | Innovation | Status |
|-------|-----------|--------|
| 1     | Foundation (palette + catalog relocation) | Done |
| 2     | Shader extraction (`resolve_cell_color`) | Done |
| 3-A   | OKLab polar gradient (sole production path) | Done |
| 3-G   | Precomputed atmospheric shader | Done |
| 3-H   | Global hue drift | Done |
| 3-I   | Palette-aware ghost base color | Done |
| 4-A   | Temporal column hue coherence | Done |
| 4-B   | Subpixel hue jitter (amplitude=3) | Done |
| 4-D   | Head halo via background blend (factor=0.15) | Done |
| 5     | Perceptual L smoothing at transition wave | Done |
| 6     | Palette-aware anomaly halos | Done |
| 7-c   | Palette-relative brightness floor (replaces v17 global MIN_RGB_SUM=180) | Done |
| 7-d   | Body-tail continuity (2.0x max gap) | Done |
| 8     | L+chroma smoothing at palette transitions | Done |
| 9-A   | Hue-preserving polar gradient | Done |
| 9-B   | Lock suite (19 invariants) | Done |
| 9-C   | sRGB-linear fallback removal | Done |
| 9-D   | ColorPipeline + legacy audit (19 invariants) | Done — current peak |

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
