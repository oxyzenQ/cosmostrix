<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Chroma Dragon Engine — LTS Lock

> **Simplified lock/unlock signature log**: see [`KEY.md`](KEY.md).
> This README holds the full audit detail (A/B benchmarks, file lists,
> stability signals).

> **3 Dragon Lock** in commit `69af079` after deeper audit for strengthening
> and stability.
>
> Signoff: **rezky_nightky** — 2026-08-19T14:40:05Z — vision & director
> project cosmostrix

---

## What This Lock Means

The Chroma Dragon Coloring Engine is locked at its current state (commit
`69af079`, audited 2026-08-19) for Long-Term Support (LTS). The code in
this directory has been audited for:

- **Peak optimization** — Phase 9-D locked (9 phases of perceptual color
  work). Every cell-color decision path reviewed for zero-cost
  abstractions, no `format!()` / `to_string()` / unnecessary `.clone()`
  in the hot path (`resolve_cell_color`).
- **Efficient resource use** — palette slot table with direct indexing
  (no hash lookup on hot path), `ShaderCtx` borrow view (no allocation
  per cell), `TRAIL_EXP_LUT` static lookup table (no per-frame compute).
- **Strong foundation** — 44-theme registry (`catalog.rs`) is the single
  source of truth for color scheme → palette mapping. OKLab gradient
  interpolation (`gradient.rs`) is the sole production path (Phase 9-A
  → 9-D, sRGB-linear fallback removed).
- **Stability** — ~1500+ tests pass, 0 clippy warnings. Lock suite
  (`chroma_dragon_engine/tests/lock.rs`, 1060 LOC) asserts the engine's
  public contract on every commit.

## Audit Findings (No Code Changes Required)

The audit confirmed the engine is already at peak. Specifically:

### 1. Palette construction (`palette/mod.rs`, 761 LOC)

- **`build_palette()`** — `#[must_use]`, called once per color switch
  (cold path). Constructs `Palette` struct with pre-decoded RGB stops
  stored as `[Color; N]` array — no per-frame decode.
- **`Palette` struct** — `bg: Color`, `colors: [Color; MAX_STOPS]`,
  `color_count: u8`. Stack-friendly, no heap allocation for ≤MAX_STOPS
  colors.
- **`apply_brightness_rgb_unclamped()`** — `#[inline]`, called per cell
  from `rain_post.rs`. No allocation.

### 2. Shader pipeline (`shaders/`, ~2K LOC across 4 files)

- **`resolve_cell_color()`** (`shaders/base/mod.rs:423`) — the per-cell
  color decision function. Profile:
  - Takes `&ShaderCtx<'_>` borrow view (zero alloc).
  - Direct array indexing via `palette_slices: [&[Color]; MAX_PALETTE_SLOTS]`
    (no hash lookup).
  - `color_map: &[u8]` (per-cell column → palette stop index) — direct
    indexing, bounds-checked defensively.
  - Returns `(Option<Color>, bool)` tuple — no allocation, compiler
    inlines on `-C opt-level=3`.
  - `#[allow(clippy::too_many_arguments)]` — explicit choice to avoid
    struct construction overhead on hot path.
- **Transition shader** (`shaders/transition/mod.rs`, 344 LOC) —
  OKLab wave transition (300 ms top-to-bottom). `TransitionLTable`
  precomputed at startup, looked up per-cell via direct index.
- **`ShaderCtx`** — borrow view constructed once per frame from
  `DrawCtx::get_attr()`. No per-cell allocation.

### 3. OKLab gradient (`gradient/mod.rs`, 389 LOC)

- **`gradient_from_stops_oklab()`** — Phase 9-A → 9-D sole production
  path. sRGB → OKLab → polar chroma lerp → sRGB. No allocation in
  inner loop; LUT-free (math is direct f32 ops, vectorized by LLVM).
- **`oklab_to_srgb()` / `srgb_to_oklab()`** — `#[inline]`, called per
  gradient stop (cold path, once per palette build).

### 4. Color cache (`color_cache.rs`, 603 LOC)

- **`ColorCache`** — pre-formatted SGR byte sequences per (palette_stop,
  bold) pair. Eliminates `format!()` calls in the hot path; the
  terminal write path uses `&[u8]` slices from this cache.
- **Lookup is direct indexing** — `cache[stop_index][bold]` returns
  `&[u8]` slice, zero allocation.

### 5. Post-FX pipeline (`post/`, ~700 LOC across 4 files)

- **`climate.rs`** (246 LOC) — `ClimateCtx` (luminance/saturation/
  instability shader). Sampled at 1 Hz, not per-frame. No hot-path
  overhead.
- **`anomaly.rs`** (182 LOC) — palette-aware anomaly halos. Phase 6
  locked. Uses `AnomalyHaloMode` enum dispatch (no dyn).
- **`ghost.rs`** (278 LOC) — phosphor ghost kanji. Halfwidth Katakana
  (U+FF66-U+FF9D) to satisfy 1-char-1-cell invariant (Bug #11 fix).
  No allocation in render path.

### 6. Lock suite (`tests/lock.rs`, 1060 LOC)

- **19 invariants** asserted on every commit. Covers:
  - Palette construction idempotency
  - OKLab gradient monotonicity
  - Color cache SGR byte correctness
  - Transition L+chroma smoothing continuity
  - Head halo exclusivity (Phase 4-D)
  - Subpixel hue jitter amplitude bounds (Phase 4-B)
  - Column hue coherence frequency (Phase 4-A)
- **Test names**: `lock_*` prefix, fail-loud on any regression.

### 7. Catalog registry (`catalog.rs`, 1134 LOC)

- **`THEMES` static** — single source of truth for color scheme →
  `ThemeDef` mapping. 44 builtin themes, each with `head/body/tail`
  RGB stops.
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

**Conclusion**: Engine is at peak. No code changes applied — the lock
is the appropriate action.

## Dragon Engine Topology (Locked)

| Subsystem                              | LOC    | Role                                                                  |
|----------------------------------------|-------:|-----------------------------------------------------------------------|
| `chroma_dragon_engine/palette/`         |    761 | Palette construction, OKLab interpolation, blend helpers             |
| `chroma_dragon_engine/catalog.rs`       |  1,134 | 44-theme registry, `build_colors()`, `ThemeDef`/`ThemeColors`         |
| `chroma_dragon_engine/shaders/`         |  2,027 | `ShaderCtx`, `CharLoc`, `resolve_cell_color()`, `TRAIL_EXP_LUT`, transition wave |
| `chroma_dragon_engine/gradient/`        |    389 | OKLab polar interpolation (sole production path since v30)            |
| `chroma_dragon_engine/legacy.rs`        |    346 | Explicit sRGB-linear fallback math (used when `ColorPipeline::LegacyRgb`) |
| `chroma_dragon_engine/post/`             |    706 | `climate.rs` (ClimateCtx), `anomaly.rs` (halos), `ghost.rs` (phosphor kanji) |
| `chroma_dragon_engine/tuning.rs`         |    290 | Phase 4+ tuning constants (innovation C/D/E, L smoothing)             |
| `chroma_dragon_engine/color_cache.rs`    |    603 | Pre-formatted SGR byte cache (eliminates `format!()` in hot path)     |
| `chroma_dragon_engine/color_tune.rs`     |    346 | `--color-tune` CLI parsing + `ColorTune` struct                       |
| `chroma_dragon_engine/colors_custom.rs`  |    507 | Custom palette loading from `[palette.<name>]` config sections        |
| `chroma_dragon_engine/tests/`           |  6,487 | Lock suite, activation, bold audit, blend, floor, gradient, post tests |

**Total**: ~6,506 LOC of substantive coloring engine code + 6,487 LOC
test suite.

## Phase History (Locked at 9-D)

| Phase | Innovation | Status |
|-------|-----------|--------|
| 1     | Foundation (palette + catalog relocation) | ✅ Locked |
| 2     | Shader extraction (`resolve_cell_color`) | ✅ Locked |
| 3-A   | OKLab polar gradient (sole production path) | ✅ Locked |
| 3-G   | Precomputed atmospheric shader | ✅ Locked |
| 3-H   | Global hue drift | ✅ Locked |
| 3-I   | Palette-aware ghost base color | ✅ Locked |
| 4-A   | Temporal column hue coherence | ✅ Locked |
| 4-B   | Subpixel hue jitter (amplitude=3) | ✅ Locked |
| 4-D   | Head halo via background blend (factor=0.15) | ✅ Locked |
| 5     | Perceptual L smoothing at transition wave | ✅ Locked |
| 6     | Palette-aware anomaly halos | ✅ Locked |
| 7-c   | Palette-relative brightness floor (replaces v17 global MIN_RGB_SUM=180) | ✅ Locked |
| 7-d   | Body-tail continuity (2.0× max gap) | ✅ Locked |
| 8     | L+chroma smoothing at palette transitions | ✅ Locked |
| 9-A   | Hue-preserving polar gradient | ✅ Locked |
| 9-B   | Lock suite (18 invariants) | ✅ Locked |
| 9-C   | sRGB-linear fallback removal | ✅ Locked |
| 9-D   | ColorPipeline + legacy audit (19 invariants) | ✅ **Locked here** |

## Modification Protocol

See [`RULES.md`](RULES.md) in this directory for the UNLOCK protocol
that MUST be followed if any file in this directory is modified after
the lock.

## UNLOCK History

| Date (UTC) | Commit | Reason | Verdict |
|------------|--------|--------|---------|
| 2026-08-19T16:36:02Z | `809a897` | Stale path refs + EnergyZen missing from `all_schemes()` test helper (INV-2 silently skipped v50 masterclass theme). Real bug fix + 15+ doc updates. | ✅ PASS — 19/19 invariants, A/B NEUTRAL, visual preserved |

See [`RULES.md`](RULES.md) § UNLOCK Log for the full detailed entry.

## Documentation Lock

> **Documentation Lock** after stale docs audit completion.
>
> All documentation in `src/chroma_dragon_engine/` (this README, RULES.md,
> and inline `///` / `//` doc comments across every `.rs` file) has been
> audited for stale, misleading, or outdated content. Documentation is now
> locked — any doc changes must follow the UNLOCK protocol in
> [RULES.md](RULES.md).
>
> Signoff: **oxyzenQ** — 2026-08-20 — stale docs audit done

---

**Lock signature:**

```
3 Dragon Lock in commit 69af079 after deeper audit for strengthening
and stability. Signoff by rezky_nightky 2026-08-19T14:40:05Z vision,
& director project cosmostrix.
```
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
