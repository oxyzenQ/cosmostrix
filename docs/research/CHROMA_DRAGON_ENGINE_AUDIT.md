<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Chroma Dragon Engine — Deep Audit & Refactor Proposal

**Task ID**: `chroma-dragon-engine-audit-1`
**Agent**: GLM (main, master Rust + master Linux coder — Cosmic Dragon mode)
**Scope**: End-to-end audit of every color-emitting code path in cosmostrix; verify the Chroma Dragon engine is the primary coloring authority with a documented legacy fallback; surface inconsistencies; propose a masterclass refactor for owner approval.
**Mode**: Read-only. **NO SOURCE CODE MODIFIED.** This document is a research deliverable for owner approval before any code changes.
**Date**: 2026-08-09 (HEAD `4780ad4`, post visual-mode retune)
**Owner directive**: "all color → chroma dragon first → fallback legacy rgb/srgb"

---

## 0. TL;DR (10-second read)

The Chroma Dragon engine (`src/chroma_dragon_engine/`) is the project's structured color pipeline — palette construction (OKLab), per-cell base shader (`resolve_cell_color`), atmospheric post-FX (`apply_climate`), palette-aware ghost color, and palette-aware anomaly halos. The engine is real and locked (Phase 9-B, 18 invariants).

**The inconsistency the owner found is real and structural.** While `resolve_cell_color` is the convergence point for *palette-stop selection*, **eleven other code paths bypass the chroma engine entirely** and emit `Color::Rgb { r, g, b }` by direct integer RGB math. These bypasses are not "fallbacks" — they are the *primary* coloring path for those effects, with no chroma-engine alternative. There is currently:

- **No "Color Pipeline mode" enum.** Nothing distinguishes "chroma active" from "legacy RGB fallback" at runtime.
- **No fallback system.** The legacy sRGB-linear gradient path was *removed* (commits referenced in `palette.rs:250` and `gradient.rs:41`). The codebase has no fallback path — every effect either calls the chroma engine or it doesn't, with no graceful degradation.
- **No verbose/doctor disclosure.** `cosmostrix -v` prints `color_mode:` (terminal color depth) but never `chroma_dragon_engine: active|fallback`. `cosmostrix --doctor` likewise. The benchmark report discloses `crystal_dragon: false` (palette rebuild disabled) but says nothing about chroma engine status.
- **Outdated docs.** `info.rs:223` (`docs_report()` for `cosmostrix --docs`) still claims "OKLab interpolation (default) + sRGB-linear fallback + hue-preserving polar variant" — but the sRGB-linear fallback was deleted.

The refactor proposal in §6 introduces a `ColorPipeline` enum (Chroma / LegacyRgb), wires every Category-A bypass through chroma first, falls back to legacy on `ColorMode::Color256 | Color16 | Mono` (or an explicit `--no-chroma` flag), and surfaces the active pipeline in `-v`, `--doctor`, and the benchmark report.

---

## 1. What the Chroma Dragon Engine Actually Is

### 1.1 Module map

```
src/chroma_dragon_engine/                                ← the engine
├── mod.rs                                 ← declares submodules + Phase history
├── palette.rs                             ← Palette struct, build_palette, blend helpers
├── catalog.rs                             ← THEMES registry (43 themes), ThemeDef
├── gradient.rs                            ← OKLab polar interpolation (sole prod path)
├── shaders/
│   ├── base.rs                            ← ShaderCtx + resolve_cell_color() ← THE convergence point
│   ├── transition.rs                      ← TransitionLTable, apply_l_smoothing (Phase 5)
│   └── mod.rs
├── post/
│   ├── climate.rs                         ← ClimateCtx + apply_climate() (luminance/sat/instability)
│   ├── ghost.rs                           ← ghost_base_color() — palette-aware ghost derivation
│   ├── anomaly.rs                         ← anomaly_halo_target() — palette-aware anomaly target
│   └── mod.rs
├── tuning.rs                              ← COLUMN_COHERENCE_FREQ, SUBPIXEL_JITTER_AMPLITUDE, HEAD_HALO_FACTOR, etc.
├── lock_tests.rs                          ← 18 ENGINE LOCK invariants (Phase 9-B)
└── palette_floor_tests.rs                 ← palette-relative floor regression tests
```

### 1.2 The chroma API surface (functions the rest of the codebase may call)

| Function | Module | Purpose |
|---|---|---|
| `build_palette(scheme, mode, default_bg)` | `chroma::palette` | Construct a `Palette` (OKLab gradient between stops, floored, quantized to `ColorMode`) |
| `color_to_rgb(color) -> (u8,u8,u8)` | `chroma::palette` | Decode any `crossterm::style::Color` variant to RGB triple |
| `decode_color(color) -> Option<(u8,u8,u8)>` | `chroma::palette` | Same as `color_to_rgb` but `Option`-returning (`None` for `Color::Reset`) |
| `apply_brightness_rgb(r, g, b, factor) -> Color` | `chroma::palette` | Per-channel brightness scale, returns `Color::Rgb` |
| `blend_toward_bg(color, bg, factor) -> Color` | `chroma::palette` | Blend `color` toward `bg` by `factor` (OKLab-friendly linear RGB blend) |
| `blend_toward_white(color, factor) -> Color` | `chroma::palette` | Convenience: `blend_toward_bg(color, Color::Rgb{255,255,255}, factor)` |
| `format_color_hex(bg) -> String` | `chroma::palette` | `"#rrggbb"` for verbose/doctor output |
| `resolve_cell_color(shader, slot, line, col, val, loc, head_put_line, length) -> (Option<Color>, bool)` | `chroma::shaders::base` | The cell-color convergence point (palette + position + glyph + transition + head-state + halo + L-smoothing + jitter + climate) |
| `apply_climate(r, g, b, line, col, ctx) -> (u8,u8,u8)` | `chroma::post::climate` | Atmospheric post-FX on raw RGB |
| `ghost_base_color(palette_colors) -> (u8,u8,u8)` | `chroma::post::ghost` | Palette-aware ghost color (replaces hardcoded `(18,22,18)`) |
| `anomaly_halo_target(palette_colors, anomaly_kind) -> Option<Color>` | `chroma::post::anomaly` | Palette-aware anomaly halo target |
| `gradient_from_stops_oklab(stops, steps)` | `chroma::gradient` | OKLab polar interpolation between stop points |
| `srgb_to_oklab(r,g,b) -> (f32,f32,f32)` | `chroma::gradient` | sRGB → OKLab conversion |
| `oklab_to_srgb(l,a,b) -> (u8,u8,u8)` | `chroma::gradient` | OKLab → sRGB conversion |

### 1.3 What the chroma engine does NOT have

- ❌ No `ColorPipeline` / `ChromaMode` enum to signal "engine active vs fallback"
- ❌ No detection of "chroma not supported, fall back to legacy"
- ❌ No legacy sRGB-linear gradient path (removed, confirmed in `palette.rs:250-255` and `gradient.rs:41`)
- ❌ No way for the user to force chroma off (`--no-chroma` flag does not exist)
- ❌ No verbose/doctor/benchmark disclosure of pipeline status

---

## 2. Methodology

The audit was conducted by:

1. **Reading the engine definition** — `chroma/mod.rs`, `chroma_dragon_engine/shaders/base.rs` (full 784 LOC), `chroma_dragon_engine/palette/mod.rs`, `chroma_dragon_engine/post/*`, `chroma_dragon_engine/tuning.rs`, `chroma/gradient.rs`.
2. **Grepping for every direct `Color::Rgb { ... }` construction** across `src/` (250+ matches found; filtered to production non-test call sites).
3. **Grepping for every direct integer RGB manipulation pattern** (`r as i32 + ...`, `r as f32 * scale`, `>> 8`). Found ~30+ production sites.
4. **Tracing the quantum ripple and mouse-click pipelines** end-to-end (`cloud/spawn.rs::spawn_quantum_ripple` → `cloud/rain.rs::apply_quantum_ripple` → `droplet.rs` flash-wave render).
5. **Reading the verbose output** (`src/verbose.rs`, 404 LOC, full) and the doctor report (`src/doctor.rs`, 626 LOC, full).
6. **Reading the benchmark mode** (`src/bench.rs::run_benchmark`, `run_premium_benchmark`) and the bench CONFIG enrichment (`compute_config_enrichment`).
7. **Cross-checking documentation claims** — `info.rs::docs_report` claims a "sRGB-linear fallback" that does not exist in the codebase.

---

## 3. Findings — Category A: Code Paths That Bypass the Chroma Engine

Every site below emits `Color::Rgb { r, g, b }` by direct integer math, **without** calling any `chroma::*` function. They are not "fallbacks" — they are the primary coloring path for those effects.

### A1. Quantum Ripple — particle color snapshot + render blend

**Files**: `src/cosmic_dragon_engine/cloud/spawn.rs:748-797` (`spawn_quantum_ripple`), `src/cosmic_dragon_engine/cloud/rain.rs:1161-1297` (`apply_quantum_ripple`)

**Spawn path** (`spawn.rs:759-769`):
```rust
let body_idx = self.palette.colors.len() / 2;
let (body_r, body_g, body_b) = self
    .palette
    .colors
    .get(body_idx)
    .and_then(|c| crate::palette::decode_color(*c))    // ← chroma helper, OK
    .unwrap_or((                                       // ← FALLBACK is hardcoded RGB
        QUANTUM_BRAND_PURPLE_R,
        QUANTUM_BRAND_PURPLE_G,
        QUANTUM_BRAND_PURPLE_B,
    ));
```

The snapshot uses `palette::decode_color` (a chroma helper) — that part is correct. The problem is:

1. **The fallback** `(QUANTUM_BRAND_PURPLE_R, QUANTUM_BRAND_PURPLE_G, QUANTUM_BRAND_PURPLE_B)` is a hardcoded RGB triple, not derived from any chroma function. This brand purple is *the wrong default* for non-default palettes (a Red-scheme ripple should fall back to dark red, not purple).
2. **The particle stores `(r, g, b)` as raw u8 fields** (`p.r = body_r; p.g = body_g; p.b = body_b;`). The render path then does raw RGB math on those fields.

**Render path** (`rain.rs:1247-1275`):
```rust
let (pr, pg, pb) = (
    (p.r as f32 * QUANTUM_BODY_TONE_DOWN).round() as u8,
    (p.g as f32 * QUANTUM_BODY_TONE_DOWN).round() as u8,
    (p.b as f32 * QUANTUM_BODY_TONE_DOWN).round() as u8,
);
// ... raw RGB blend toward particle snapshot ...
let nr = (br as i32 + ((pr as i32 - br as i32) * wf + 128) / 256).clamp(0, 255) as u8;
let ng = (bg_ as i32 + ((pg as i32 - bg_ as i32) * wf + 128) / 256).clamp(0, 255) as u8;
let nb = (bb as i32 + ((pb as i32 - bb as i32) * wf + 128) / 256).clamp(0, 255) as u8;
let new_fg = Color::Rgb { r: nr, g: ng, b: nb };   // ← direct construction, no chroma
```

**Why this is a problem (chroma-engine perspective)**:
- The blend uses **linear sRGB interpolation** in `(r as i32 + (target - r) * wf) / 256` form. Linear sRGB interpolation produces the well-known "muddy midpoint" artifact — blending red→cyan passes through gray instead of through a perceptual mid-hue. The chroma engine's `blend_toward_bg` (which uses linear RGB too, but the palette construction uses OKLab) is at least consistent with the rest of the chroma pipeline. The ripple path is off doing its own thing.
- The blend ignores `color_mode`. In `ColorMode::Color256` or `ColorMode::Color16`, the particle writes a truecolor `Color::Rgb` that the terminal cannot display natively (it gets quantized by crossterm downstream, but the chroma engine should have first-class awareness).
- There is **no `apply_climate` call** on the particle. The rest of the frame gets atmospheric luminance/saturation drift, but the ripple particle does not — it sticks out as a "non-atmospheric" overlay.

**Chroma-native replacement** (proposal, NOT yet implemented):
```rust
// spawn: store Color, not (r,g,b)
let body_color = self.palette.colors.get(body_idx).copied()
    .unwrap_or(crate::chroma_dragon_engine::catalog::default_body_color(self.color_scheme));
p.color = body_color;     // store Color, not (r,g,b)

// render: blend through chroma engine
let base = cell.fg.unwrap_or(p.color);
let faded = crate::chroma_dragon_engine::palette::blend_toward_bg(p.color, base, 1.0 - brightness);
let atmospheric = {
    let (r, g, b) = crate::chroma_dragon_engine::palette::color_to_rgb(faded);
    let (r, g, b) = crate::chroma_dragon_engine::post::climate::apply_climate(r, g, b, line, col, climate_ctx);
    Color::Rgb { r, g, b }
};
frame.set_force(col, line, Cell { ch: p.ch, fg: Some(atmospheric), bg: cell.bg, bold: true });
```

### A2. Mouse-Click Flash Wave — dual-ring color boost

**File**: `src/droplet.rs:950-998`

```rust
for w in ctx.flash_waves {
    // ... radius math ...
    if factor > 0.0 {
        let wf = (factor * 256.0) as i32;
        r = (r as i32 + ((255 - r as i32) * wf + 128) / 256).clamp(0, 255) as u8;
        g = (g as i32 + ((255 - g as i32) * wf + 128) / 256).clamp(0, 255) as u8;
        b = (b as i32 + ((255 - b as i32) * wf + 128) / 256).clamp(0, 255) as u8;
    }
}
```

The flash wave **always blends toward pure white (255, 255, 255)**. This is the exact behavior `chroma::palette::blend_toward_white(c, factor)` exists to provide — but the droplet render loop inlines its own raw-RGB version for performance.

**Chroma-native replacement**: refactor `blend_toward_white` to expose a `blend_toward_white_rgb(r, g, b, factor) -> (u8, u8, u8)` variant (or have the shader return `(r, g, b)` instead of `Color` to avoid the decode round-trip). The chroma engine already has the logic — it is just not the path the droplet takes.

### A3. Head Brightness Modulation

**File**: `src/droplet.rs:1002-1008`

```rust
if matches!(loc, CharLoc::Head) && head_bright < 1.0 {
    let factor = 0.7 + 0.3 * head_bright;
    let fi = (factor * 256.0) as i32;
    r = ((r as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
    g = ((g as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
    b = ((b as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
}
```

This is exactly what `chroma::palette::apply_brightness_rgb(r, g, b, factor)` does. The droplet should call the chroma helper.

### A4. Head Self-Bloom (color-channel boost)

**File**: `src/droplet.rs:1027-1043`

```rust
const HEAD_BOOST: f32 = 60.0 / 256.0;
let layer_selfbloom = PARALLAX_HEAD_SELFBLOOM_MULT[self.layer as usize];
let wf = HEAD_BOOST * layer_selfbloom;
let scale = 1.0 + wf;
r = (r as f32 * scale).round().clamp(0.0, 255.0) as u8;
g = (g as f32 * scale).round().clamp(0.0, 255.0) as u8;
b = (b as f32 * scale).round().clamp(0.0, 255.0) as u8;
```

The comment says "boost each channel toward 255 but proportionally to its current value — this preserves hue". This is a multiplicative scale, which is hue-preserving in linear RGB but **not perceptually hue-preserving** in OKLab. The chroma engine should expose a `boost_toward_white_perceptual(r, g, b, factor)` that does the scaling in OKLab L channel (preserves hue+chroma, only lifts L).

### A5. Rain Shadow (bottom-of-screen dim)

**File**: `src/droplet.rs:1059-1066`

```rust
let shadow_raw = rain_shadow_factor(line, ctx.lines);
let shadow = 1.0 - (1.0 - shadow_raw) * RAIN_SHADOW_LAYER_MULT[self.layer as usize];
if shadow < 1.0 {
    let fi = (shadow * 256.0) as i32;
    r = ((r as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
    g = ((g as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
    b = ((b as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
}
```

Same pattern as A3 — should call `apply_brightness_rgb`.

### A6. Edge Fade (top/bottom border dim)

**File**: `src/droplet.rs:1073-1078`

```rust
if edge_fade < 1.0 {
    let fi = (edge_fade * 256.0) as i32;
    r = ((r as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
    g = ((g as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
    b = ((b as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
}
```

Same pattern — should call `apply_brightness_rgb`.

### A7. Radial Vignette

**File**: `src/droplet.rs:1090-1098`

```rust
let vignette_raw = vignette_factor(self.bound_col, line, ctx.cols, ctx.lines);
let vignette = 1.0 - (1.0 - vignette_raw) * VIGNETTE_LAYER_MULT[self.layer as usize];
if vignette < 1.0 {
    let fi = (vignette * 256.0) as i32;
    r = ((r as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
    g = ((g as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
    b = ((b as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
}
```

Same pattern — should call `apply_brightness_rgb`.

### A8. CRT Vignette Cell Dim (masterclass retune — `4780ad4`)

**File**: `src/cosmic_dragon_engine/cloud/rain.rs:1310-1351` (`apply_crt_dim_cell`)

```rust
let Some((r, g, b)) = crate::palette::decode_color(fg) else { return; };
let fi = (factor * 256.0) as i32;
let nr = ((r as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
let ng = ((g as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
let nb = ((b as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
let new_fg = Color::Rgb { r: nr, g: ng, b: nb };
```

Same pattern. Uses `decode_color` (chroma helper, good) but then does raw RGB math. Should call `apply_brightness_rgb` and return the `Color` directly.

### A9. Ghost Event Render

**File**: `src/cosmic_dragon_engine/cloud/events/ghost.rs:84-135`

```rust
let (br, bg, bb) = ctx.ghost_base_color;     // ← from chroma::post::ghost, OK
let r = (br as f32 * opacity) as u8;          // ← raw RGB scaling, NOT chroma
let g = (bg as f32 * opacity) as u8;
let b = (bb as f32 * opacity) as u8;
if r == 0 && g == 0 && b == 0 { return; }
// ...
frame.set_force(self.col, self.line, Cell {
    ch: self.ch,
    fg: Some(Color::Rgb { r, g, b }),          // ← direct construction
    ..cell
});
```

`ghost_base_color` is from `chroma::post::ghost` — palette-aware, good. But the opacity fade is raw RGB scaling. Should be `apply_brightness_rgb(br, bg, bb, opacity)`.

### A10. Monolith Render

**File**: `src/cosmic_dragon_engine/cloud/monolith.rs:894`

```rust
Some(Color::Rgb { r, g, b })
```

Direct construction at the end of the monolith color pipeline. Need to audit the full pipeline (the file is large) — but the construction itself bypasses any chroma wrapper. Likely candidates for chroma migration: the brightness/contrast math feeding into `(r, g, b)`.

### A11. Phosphor anomaly halos — partial chroma use

**File**: `src/cosmic_dragon_engine/cloud/phosphor.rs:296, 318, 354, 489-497, 592-599`

The phosphor file is **half-migrated**:
- Lines 296, 318, 354 call `palette::apply_brightness_rgb` ✓ (chroma helper)
- Lines 489-497 call `palette::blend_toward_bg` ✓ (chroma helper, palette-aware anomaly halo target)
- Lines 592-599 also use `blend_toward_bg` with anomaly halo target ✓

This is the model the rest of the codebase should follow. Phosphor is the **only** production module that consistently routes through the chroma engine for color manipulation. The other Category-A sites should be brought up to phosphor's standard.

---

## 4. Findings — Category B: No Chroma Engine Status Disclosure

### B1. `cosmostrix -v` / `--verbose`

**File**: `src/verbose.rs:104-341`

The verbose output prints `color_mode: TrueColor` (or `Color256`/`Color16`/`Mono`) but **never** discloses whether the Chroma Dragon engine is active. The user has no way to verify "is chroma running, or did something fall back?".

Relevant existing fields:
```
  ── Scene & Color ──
color_scheme: Green (CLI default)
color_mode: TrueColor
color_tune: sat=1.00 bright=1.00 head=1.00 body=1.00 tail=1.00
color_bg: default-background (terminal native bg, no override)
```

Missing field (proposed):
```
color_pipeline: chroma_dragon (oklab gradient, perceptual blend, climate post-fx)
```
…or on fallback:
```
color_pipeline: legacy_rgb (color256 mode — chroma disabled, raw sRGB fallback)
```

### B2. `cosmostrix --doctor`

**File**: `src/doctor.rs:23-323`

The doctor report has:
- `RENDERER.color_depth` — terminal color depth (truecolor / 256-color / 16-color)
- `TERMINAL.color_auto_detected` — auto-detected mode
- `TERMINAL.color_forced` — `--colormode` override (only shown when set)
- `COMPATIBILITY.color_capability` — same as `color_depth` essentially

**No** `RENDERER.color_pipeline` field. The doctor report should expose the active pipeline so a user running `cosmostrix --doctor` can immediately see whether they're getting the chroma engine or the legacy fallback.

### B3. `cosmostrix --benchmark`

**Files**: `src/bench.rs:180-244` (`run_benchmark`), `src/bench.rs:60-180` (`compute_config_enrichment`), `src/bench_report.rs` (BenchReportData)

**Current behavior**:
1. The benchmark loop sets `cloud.crystal_dragon = false` (line 201) to keep p99/max metrics deterministic (palette rebuilds inject timing spikes).
2. **Climate drift still runs** — the comment at line 196-200 says: "Climate drift (luminance/saturation/hue modulation) still runs because it is deterministic (fixed RNG seed) and has no rebuild cost."
3. The Chroma Dragon engine itself is **NOT disabled**. Every cell still goes through `resolve_cell_color` → `apply_climate` → etc.

**The benchmark report discloses** (in the CONFIG block):
- `color_mode_label` (e.g. "truecolor")
- `custom_palette_name`
- `color_bg_label`
- `color_tune_summary`
- `crystal_dragon: false` (with the `bench_override:` notice in verbose)

**The benchmark report does NOT disclose**:
- Whether the chroma engine is active
- Whether any chroma bypasses (Category A) are running

The owner's question — *"when benchmarking mode 'cosmostrix --benchmark' is the chroma dragon enable/disable?"* — has the answer: **chroma is ENABLED** in benchmark mode (only palette *drift* is disabled, not the chroma engine itself). But the user cannot see this from the report.

**Proposal**: add `color_pipeline: chroma_dragon` (or `legacy_rgb`) to the benchmark CONFIG block, and add a `bench_chroma_status:` line that explicitly states "chroma engine: enabled (climate drift active, palette drift disabled for determinism)".

---

## 5. Findings — Category C: Outdated Documentation

### C1. `info.rs::docs_report` claims a non-existent sRGB-linear fallback

**File**: `src/info.rs:222-224`

```rust
  gradient   OKLab interpolation (default) + sRGB-linear fallback +
             hue-preserving polar variant (Phase 9-A).
```

**Reality** (from `src/chroma_dragon_engine/palette.rs:250-255`):
> Historically this file held `srgb_to_linear`, `linear_to_srgb`, and `lerp_u8_gamma` — the gamma-correct sRGB interpolator used by ... [the] sole production path; **the legacy sRGB-linear path and the Cartesian [variant] have been removed**.

**Reality** (from `src/chroma_dragon_engine/gradient.rs:10-41`):
> The previous `lerp_u8_gamma` (sRGB → linear → sRGB) interpolated each [channel independently] ... **[the] variant and the legacy sRGB-linear variant have been removed.** The [sole production path is OKLab polar].

The `docs_report` output is shown by `cosmostrix --docs` and is also embedded in the binary for `strings(1)` discovery. The claim "sRGB-linear fallback" is false — there is no fallback. This must be corrected.

**Proposed correction**:
```
  gradient   OKLab polar interpolation (sole production path).
             Hue-preserving, perceptually uniform. No sRGB-linear fallback —
             the legacy path was removed (see palette.rs:250).
```

### C2. `chroma_dragon_engine/mod.rs` Phase history mentions "sRGB-linear" being removed but the docs_report still claims it exists

The chroma engine's own `mod.rs` Phase history is accurate (Phase 9-A says "sole production path"). The mismatch is only in `info.rs::docs_report` (which is user-facing via `--docs`).

---

## 6. Refactor Proposal (for owner approval — NO CODE YET)

### 6.1 Design principle

**Owner's rule**: "all color → chroma dragon first → fallback legacy rgb/srgb"

**Operational definition**:
- *Primary path*: every color-emitting code path calls a `chroma::*` function to produce its final `Color`.
- *Fallback path*: when the chroma engine is not supported (or explicitly disabled), the same code path calls a `chroma::legacy::*` function that does raw sRGB-linear math — the *exact* math the bypasses currently inline.

The fallback is **never silent**. The user is told via `-v`, `--doctor`, and the benchmark report which path is active.

### 6.2 New `ColorPipeline` enum

**Location**: `src/chroma_dragon_engine/mod.rs` (new pub enum) or `src/cosmic_dragon_engine/runtime.rs` (alongside `ColorMode`).

```rust
/// Which color pipeline is active.
///
/// The Chroma Dragon engine is the primary coloring authority. When the
/// terminal cannot support truecolor (or the user explicitly disables
/// chroma via --no-chroma), the pipeline falls back to legacy sRGB-linear
/// math — the same math the chroma engine uses internally, but without
/// OKLab palette construction, perceptual blending, or atmospheric post-FX.
///
/// Disclosed in `cosmostrix -v`, `cosmostrix --doctor`, and the benchmark
/// CONFIG block so the user can verify which path is active.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorPipeline {
    /// Chroma Dragon engine: OKLab gradient, perceptual blend, climate post-FX.
    /// Active when ColorMode == TrueColor AND no --no-chroma flag.
    ChromaDragon,

    /// Legacy sRGB-linear pipeline: raw RGB math, no OKLab, no climate post-FX.
    /// Active when ColorMode in {Color256, Color16, Mono} OR --no-chroma flag.
    ///
    /// This is NOT a separate code path — it is the SAME code path with the
    /// chroma helpers swapped for their raw-RGB equivalents. The visual
    /// result is degraded (no perceptual blending, no atmospheric drift)
    /// but functionally correct.
    LegacyRgb,
}

impl ColorPipeline {
    pub fn detect(color_mode: ColorMode, no_chroma_flag: bool) -> Self {
        if no_chroma_flag || !matches!(color_mode, ColorMode::TrueColor) {
            Self::LegacyRgb
        } else {
            Self::ChromaDragon
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::ChromaDragon => "chroma_dragon",
            Self::LegacyRgb => "legacy_rgb",
        }
    }

    pub fn is_chroma(self) -> bool { matches!(self, Self::ChromaDragon) }
}
```

### 6.3 New `chroma::legacy` module (the explicit fallback)

**Location**: `src/chroma_dragon_engine/legacy.rs` (new file, ~80 LOC).

Houses the raw-RGB math that Category-A bypasses currently inline. Every function is the *exact* code that today lives inside `droplet.rs` / `rain.rs` / `spawn.rs` / `ghost.rs` — extracted as `pub(crate)` free functions so the bypass sites can call them when `ColorPipeline::LegacyRgb` is active.

```rust
// src/chroma_dragon_engine/legacy.rs
//! Legacy sRGB-linear color math — the explicit fallback when the Chroma
//! Dragon engine is not active. Each function is the verbatim code that
//! used to be inlined in droplet.rs / rain.rs / spawn.rs / ghost.rs.
//! Kept as a separate module so the chroma engine and the legacy path
//! are auditable side-by-side.

/// Linear-sRGB brightness scale. (r,g,b) * factor, clamped.
/// Used by: edge_fade, vignette, rain_shadow, head_brightness, crt_dim.
#[inline]
pub(crate) fn scale_rgb(r: u8, g: u8, b: u8, factor: f32) -> (u8, u8, u8) {
    let fi = (factor * 256.0) as i32;
    (
        ((r as i32 * fi + 128) >> 8).clamp(0, 255) as u8,
        ((g as i32 * fi + 128) >> 8).clamp(0, 255) as u8,
        ((b as i32 * fi + 128) >> 8).clamp(0, 255) as u8,
    )
}

/// Linear-sRGB blend toward target. (r,g,b) + (target - r,g,b) * factor.
/// Used by: flash_wave (toward white), quantum_ripple (toward snapshot).
#[inline]
pub(crate) fn blend_toward_rgb(
    r: u8, g: u8, b: u8,
    tr: u8, tg: u8, tb: u8,
    factor: f32,
) -> (u8, u8, u8) {
    let wf = (factor * 256.0) as i32;
    (
        (r as i32 + ((tr as i32 - r as i32) * wf + 128) / 256).clamp(0, 255) as u8,
        (g as i32 + ((tg as i32 - g as i32) * wf + 128) / 256).clamp(0, 255) as u8,
        (b as i32 + ((tb as i32 - b as i32) * wf + 128) / 256).clamp(0, 255) as u8,
    )
}
```

### 6.4 Migration pattern for every Category-A bypass

For each bypass site, the migration is:

```rust
// BEFORE (raw RGB inline)
let fi = (factor * 256.0) as i32;
r = ((r as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
g = ((g as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
b = ((b as i32 * fi + 128) >> 8).clamp(0, 255) as u8;

// AFTER (chroma first, legacy fallback)
let (nr, ng, nb) = if pipeline.is_chroma() {
    // Chroma path: perceptual OKLab brightness scale (preserves hue+chroma)
    crate::chroma_dragon_engine::palette::scale_rgb_perceptual(r, g, b, factor)
} else {
    // Legacy fallback: linear sRGB scale (matches old behavior exactly)
    crate::chroma_dragon_engine::legacy::scale_rgb(r, g, b, factor)
};
r = nr; g = ng; b = nb;
```

**Hot-path concern**: the `if pipeline.is_chroma()` branch is on a `Copy` enum field, branch-predicted to always-take the chroma path in production. Zero cost on the hot path. Alternatively, hoist the branch out of the per-cell loop and pass a `&dyn ColorOps` trait object — but the inline branch is simpler and equally fast.

### 6.5 Status disclosure wiring

#### 6.5.1 Verbose (`-v`)

Add to `verbose.rs::print_verbose` after the `color_mode:` line:

```rust
let pipeline = ColorPipeline::detect(color_mode, args.no_chroma);
output::eprintln_verbose(
    "color_pipeline:",
    &format!(" {} ({})", pipeline.label(), pipeline.description()),
);
if pipeline.is_chroma() {
    output::eprintln_verbose(
        "  chroma_features:",
        " oklab_gradient, perceptual_blend, climate_post_fx, head_halo, l_smoothing, subpixel_jitter",
    );
} else {
    output::eprintln_verbose(
        "  chroma_features:",
        " disabled — legacy sRGB-linear fallback in effect",
    );
    output::eprintln_verbose(
        "  chroma_disable_reason:",
        &format!(" {}", pipeline.disable_reason(color_mode, args.no_chroma)),
    );
}
```

#### 6.5.2 Doctor (`--doctor`)

Add a new field to the `RENDERER` section:

```rust
let pipeline = ColorPipeline::detect(effective, args.no_chroma);
s.field("color_pipeline", pipeline.label());
s.field("color_pipeline_detail", pipeline.description());
if !pipeline.is_chroma() {
    s.field("chroma_disable_reason", &pipeline.disable_reason(effective, args.no_chroma));
}
```

#### 6.5.3 Benchmark (`--benchmark`)

Add to `compute_config_enrichment` and the `BenchReportData` struct:

```rust
let color_pipeline = ColorPipeline::detect(cfg.color_mode, cfg.no_chroma);
let color_pipeline_label = color_pipeline.label();
// ...
// In BenchReportData:
//   color_pipeline: &'static str,
//   color_pipeline_active: bool,    // chroma active even in benchmark mode?
//   chroma_drift_in_benchmark: &'static str,  // "palette_drift_off, climate_drift_active"
```

The benchmark report's CONFIG block then prints:
```
  color_pipeline: chroma_dragon
  chroma_in_benchmark: enabled (palette_drift off for determinism, climate_drift active)
```

#### 6.5.4 New `--no-chroma` CLI flag

**Files**: `src/cli_parse.rs`, `src/cli.rs`, `src/config.rs`

Add a `--no-chroma` flag that forces `ColorPipeline::LegacyRgb` regardless of `ColorMode`. Useful for:
- Debugging (is the chroma engine causing a visual artifact?)
- Terminals that report `COLORTERM=truecolor` but render OKLab-blended colors incorrectly (rare, but reported on some Windows Terminal versions)
- A/B comparison screenshots

### 6.6 Phased rollout (each phase is a microcommit, independently revertable)

| Phase | Scope | LOC delta | Risk |
|---|---|---|---|
| **P1** | Add `ColorPipeline` enum + `detect()` + `label()` + `description()` + unit tests. No callers yet. | +60 | Zero (additive only) |
| **P2** | Add `chroma::legacy` module with `scale_rgb` + `blend_toward_rgb` (verbatim from current bypass sites). No callers yet. | +80 | Zero (additive only) |
| **P3** | Add `--no-chroma` CLI flag, wire to `ColorPipeline::detect`. Verbose prints `color_pipeline:` line. | +120 | Low (new field, no behavior change) |
| **P4** | Add `color_pipeline` field to doctor report RENDERER section. | +30 | Low |
| **P5** | Add `color_pipeline` + `chroma_in_benchmark` to bench CONFIG block + report. | +50 | Low |
| **P6** | Migrate A8 (CRT vignette dim, `apply_crt_dim_cell`) — single function, easy first migration. | +5 / -8 | Low |
| **P7** | Migrate A3 (head brightness) + A5 (rain shadow) + A6 (edge fade) + A7 (radial vignette) — all the same `apply_brightness_rgb` pattern in droplet.rs. | +20 / -40 | Medium (touches droplet hot path; bench must verify no FPS regression) |
| **P8** | Migrate A2 (flash wave) — `blend_toward_white` path. | +10 / -15 | Medium |
| **P9** | Migrate A1 (quantum ripple spawn + render) — biggest change, stores `Color` not `(r,g,b)` on particle. | +40 / -30 | High (changes particle struct layout, regression-tests in `tests_quantum.rs` need update) |
| **P10** | Migrate A9 (ghost event render) — `apply_brightness_rgb` for opacity fade. | +5 / -8 | Low |
| **P11** | Migrate A4 (head self-bloom) — needs new `boost_toward_white_perceptual` chroma helper (OKLab L lift). | +30 / -10 | Medium (new chroma helper, needs lock_tests invariant) |
| **P12** | Migrate A10 (monolith render) — full audit of `cloud/monolith.rs` color pipeline. | TBD | High (monolith is complex) |
| **P13** | Fix C1 (info.rs docs_report outdated sRGB-linear fallback claim). | +3 / -2 | Zero (docs only) |
| **P14** | Add `INV-19: ColorPipeline disclosure` to `chroma_dragon_engine/tests/lock.rs` — assert that verbose/doctor/bench all disclose the pipeline. | +60 | Zero (test only) |

**Total estimated delta**: +550 / -120 LOC across 14 microcommits.

### 6.7 Test strategy

- **P1-P5** (disclosure): unit tests assert `ColorPipeline::detect` returns the right variant for each `(ColorMode, no_chroma)` combination.
- **P6-P12** (migration): the existing regression tests in `tests_quantum.rs`, `tests_anomaly.rs`, `tests_visual_depth.rs`, `tests_monolith/depth.rs` must pass unchanged. The chroma path's output must be perceptually equivalent (within ±2 per channel) to the legacy path's output for the same inputs — verified by a new `tests_chroma_legacy_parity.rs`.
- **P14** (lock test): the existing 18 invariants in `chroma_dragon_engine/tests/lock.rs` continue to pass. Add INV-19 asserting that `verbose::print_verbose`, `doctor::print_doctor_report`, and `bench::compute_config_enrichment` all call `ColorPipeline::detect` and emit the `color_pipeline:` field.

### 6.8 Benchmark impact assessment

The chroma-native path for the Category-A bypasses is **not slower** than the legacy path:

- `apply_brightness_rgb` is 3 multiplies + 3 clamps — identical to the inlined `>> 8` math.
- `blend_toward_bg` is 6 multiplies + 3 clamps — identical to the inlined `(target - r) * wf / 256` math.
- The `if pipeline.is_chroma()` branch is predicted-true in production and costs 0 cycles on the hot path.

The only **new** cost is the perceptual (OKLab) variants proposed in A4 (head self-bloom) and A1 (quantum ripple blend). These do an sRGB→OKLab→sRGB round-trip per call. At ~12.9M Middle cells/sec, this is ~38M cycles/sec extra (sRGB→OKLab is ~3 cycles, OKLab→sRGB is ~3 cycles, plus the L lift = ~1 cycle). On a 3 GHz core that's ~1.3% CPU. Acceptable, but the legacy fallback must remain for users on slow CPUs or non-truecolor terminals.

The benchmark report's `render_ns_per_cell` and `total_ns_per_cell` metrics will surface any regression. P7 (droplet migration) is the highest-risk phase — it must be validated with `cosmostrix --benchmark --bench-frames 10000` before/after comparison.

---

## 7. Open Questions for the Owner

1. **`ColorPipeline::detect` criterion**: should chroma be active only on `ColorMode::TrueColor`, or should it also be active on `ColorMode::Color256` (with palette quantization)? Current proposal: TrueColor-only. Owner to confirm.

2. **`--no-chroma` flag**: is this desired, or should chroma be mandatory when supported? Current proposal: yes, provide the flag for debugging and edge-case terminals. Owner to confirm.

3. **Perceptual blend in quantum ripple (A1)**: the current linear-RGB blend produces a "muddy midpoint" when blending between two complementary palette colors. The OKLab blend would produce a perceptually smooth midpoint. This is a visible behavior change. Owner to confirm: is the visual change desired, or should the chroma path preserve the current "muddy" look for parity?

4. **Head self-bloom OKLab L lift (A11)**: the current multiplicative scale is hue-preserving in linear RGB but not perceptually. The OKLab L lift is perceptually hue-preserving. Visible behavior change. Owner to confirm desired.

5. **Phasing**: P1-P5 (disclosure) can land first as a single PR (low risk, high value — user can immediately see what's happening). P6-P14 (migration) can land as a second PR per phase. Owner to confirm phasing strategy.

---

## 8. Summary

The Chroma Dragon engine is real, locked, and well-architected. The inconsistency the owner found is not in the engine itself — it is in the **eleven code paths that bypass the engine** and do raw RGB math, plus the **missing runtime disclosure** of which pipeline is active.

The refactor proposal introduces a `ColorPipeline` enum, an explicit `chroma::legacy` module housing the raw-RGB math (so the fallback is auditable, not inlined), and wires every bypass through `chroma::*` first with `legacy::*` as the explicit fallback. The user is told via `-v`, `--doctor`, and `--benchmark` which path is active.

The refactor is **14 microcommits** totaling +550/-120 LOC, with each phase independently revertable. No commit breaks the existing 18 chroma engine lock invariants. The benchmark `render_ns_per_cell` metric is the regression guard.

**Owner approval required before any code is written.** This document is the proposal.

---

## 9. Second-Pass Audit — A14–A23 (10 additional bypass sites)

**Date**: 2026-08-09 (post A1–A13 migration, HEAD `bd9ba00`+)
**Trigger**: After the original §3 audit (A1–A11) was migrated and committed (P6–P14 phased rollout), a thorough file-by-file + ripgrep re-audit found **10 additional bypass sites** that the first pass had missed. This section documents them and tracks their migration status.

### 9.1 Why the first pass missed them

The first audit (§3) focused on **direct `Color::Rgb { r, g, b }` construction by raw integer math** as its detection signal. The 10 sites below were missed because they fall into two patterns the original grep did not catch:

1. **Sites calling chroma helpers without the `is_chroma()` branch** (A14–A17, A18–A22). These sites already invoke `palette::apply_brightness_rgb`, `palette::blend_toward_bg`, `palette::blend_toward_white`, or the `_rgb` tuple variants — so they pass the "no raw RGB math" grep. But they bypass the routing pattern: they call the chroma helper unconditionally, without the `if ctx.color_pipeline.is_chroma() { chroma } else { legacy }` branch that A1–A13 established as the standard.

2. **A site that uses the chroma helper as a one-shot fade** (A23). Single line, easy to overlook in a long file.

The structural invariant the owner's rule implies is: *every color-emitting call site branches on `is_chroma()`*. The first pass implemented this for the 11 sites it found; the second pass extends it to the 10 sites below.

### 9.2 Findings

#### HIGH priority — 4 sites in `droplet.rs::Droplet::draw` (per-cell hot path)

| ID | File:Line | Effect | Chroma helper | Special handling |
|---|---|---|---|---|
| A14 | `droplet.rs:786-790` | Transition energy glow (new-palette shimmer) | `blend_toward_white_rgb` / `blend_toward_white` | No — `factor = t * 0.15` always in `[0, 0.15]` |
| A15 | `droplet.rs:810-813` | Head bloom (Gaussian glow behind head) | `blend_toward_white_rgb` / `blend_toward_white` | No — factor similar |
| A16 | `droplet.rs:827-832` | Parallax layer brightness × glyph dim | `apply_brightness_rgb` / `scale_rgb` | **YES** — `PARALLAX_BRIGHTNESS_MULT[2] = 1.10` (>1.0); needs the unclamped variant pattern introduced by A11 |
| A17 | `droplet.rs:923-927` | Cursor glow (mouse halo) | `blend_toward_white_rgb` / `blend_toward_white` | **DEAD CODE** — `MOUSE_GLOW_INTENSITY = 0.0` in production, LLVM folds the block away. Migration optional (audit consistency only) |

#### MEDIUM priority — 5 sites in `cloud/phosphor.rs` (the audit doc's "A11 partial migration" claim)

The original §3 A11 entry said phosphor.rs was the **model** — "the only production module that consistently routes through the chroma engine for color manipulation". The second pass re-examined this claim and found it was structurally a **bypass** by the routing-pattern standard: phosphor.rs calls chroma helpers unconditionally, without the `is_chroma()` branch. Functionally this is correct in legacy mode (chroma helpers produce bit-identical output to legacy helpers per the parity contracts in `chroma_dragon_engine/legacy.rs`), but it bypasses the routing pattern every other site follows.

| ID | File:Line | Effect | Current | Migration |
|---|---|---|---|---|
| A18 | `phosphor.rs:295-296` | Sub-threshold ghost brightness | `palette::apply_brightness_rgb` direct | Wrap in `is_chroma()` branch; legacy calls `scale_rgb` |
| A19 | `phosphor.rs:317-318` | Main ghost brightness (visible trail) | same | same |
| A20 | `phosphor.rs:353-354` | Orphan trail fallback (rare path) | same | same |
| A21 | `phosphor.rs:489-493` | Anomaly `LuminanceSurge` halo | `palette::blend_toward_bg` / `blend_toward_white` direct | Wrap; legacy decodes `Color` and calls `blend_toward_rgb` |
| A22 | `phosphor.rs:593-598` | Anomaly `PulseWave` halo | same | same |

A18–A20 share the same shape (3 sites of identical duplication) — a shared helper could be extracted. A21–A22 also share shape (2 sites). The shared-helper extraction is deferred to a separate refactor; the migration commits keep the change minimal (wrap each site in-place) to preserve the bit-exact parity property.

#### LOW priority — 1 site in `cloud/mod.rs`

| ID | File:Line | Effect | Notes |
|---|---|---|---|
| A23 | `cloud/mod.rs:900` | `draw_message` overlay fade-in brightness | Only runs during cinematic message reveal (rare, transient). ~20–50 cells/frame for ~100 ms per message. Low impact. |

### 9.3 Migration status

| ID | Priority | Status | Commit |
|---|---|---|---|
| A14 | HIGH | ✅ Done | `3a8fc96` — "migrate A14 transition energy to chroma engine" |
| A15 | HIGH | ✅ Done | `6bbbc7e` — "migrate A15 head bloom white-blend to chroma engine" |
| A16 | HIGH | ✅ Done | `1274e23` — "migrate A16 parallax brightness+glyph dim to chroma engine" |
| A17 | HIGH (dead code) | ✅ Done | `31c7a41` — "migrate A17 cursor glow white-blend to chroma engine" |
| A18 | MEDIUM | ✅ Done | `8309f85` — "unify phosphor.rs A18-A22 with is_chroma() branch" |
| A19 | MEDIUM | ✅ Done | `8309f85` (same commit) |
| A20 | MEDIUM | ✅ Done | `8309f85` (same commit) |
| A21 | MEDIUM | ✅ Done | `8309f85` (same commit) |
| A22 | MEDIUM | ✅ Done | `8309f85` (same commit) |
| A23 | LOW | ✅ Done | `9c02916` — "migrate A23 draw_message fade-in to chroma engine" (last bypass site; cloud/mod.rs kept at 1000/1000 LOC cap via nearby iterator compaction) |

### 9.4 Post-migration verification

After A14–A23 (HEAD `5992007`):

- `cargo check --bins`: clean
- `cargo test --bins`: **1453 passed, 0 failed** (35.64 s)
- `cargo clippy --bins --no-deps`: **0 warnings** (the pre-existing `palette.rs:473` `doc_lazy_continuation` warning was fixed in `5992007`)
- `scripts/check-rs-loc.sh`: all files at or below 1500 lines (`phosphor.rs` 828/1500 after shared-helper extraction, `droplet.rs` 1500/1500 unchanged, `cloud/mod.rs` 1000/1000 — exactly at the per-file cap)
- `tests_scene::all_rust_files_under_loc_cap`: PASSED (cloud/mod.rs 1000/1000)

### 9.5 Categories correctly NOT migrated (re-confirmed)

The second-pass audit re-confirmed that the following categories correctly do NOT route through the chroma engine and should remain as-is:

1. **Overlay UI** — `interactive/hud.rs::brighten_color` (HSV-value scaling for HUD readability), `interactive/intro.rs::lerp_rgb` (linear-sRGB blend for intro particle fade). Different math semantics; migrating would break regression tests and produce ±1/channel visible artifacts.

2. **CLI chrome** — `output.rs` Tailwind brand colors. Static, not animated.

3. **Storage/parsing** — `catalog.rs` RGB tuples, `colors_custom.rs` hex parsing. Input data, not color operations.

4. **Documented legacy fallbacks** — `cloud/events/ghost.rs:116-118` (A9 legacy fallback). Intentionally preserves pre-migration `(c as f32 * opacity) as u8` truncation behavior; the chroma path at line 112 uses `apply_brightness_rgb` (integer `>> 8` + 128 rounding). The two paths can differ by ±1 per channel (e.g. `255 * 0.5 = 127.5` → 127 legacy, 128 chroma), which is imperceptible on a dim ghost overlay and acceptable per the owner rule.

5. **Scalar decay** — `cloud/phosphor.rs:277` phosphor energy scalar decay (`u8` persistence counter, not a color channel). The energy value is later consumed by `apply_brightness_rgb` (already chroma-routed at A18–A20).

### 9.6 Net migration status after this audit cycle

- **Original §3 audit (A1–A11)**: 11 sites → all migrated in P6–P14 rollout.
- **Migration expansion (A12–A13)**: 2 sites discovered during P9/P10 implementation (DoF contrast reduction, depth fog). Migrated as `8ca0fca` and `e15dfc9`.
- **Second-pass audit (A14–A23)**: 10 sites discovered post-A1–A13 migration. All 10 migrated in this cycle (A14–A17 in `3a8fc96`/`6bbbc7e`/`1274e23`/`31c7a41`, A18–A22 in `8309f85` + shared helpers extracted in `aaf4003`, A23 in `9c02916`).
- **Total identified bypass sites**: 23.
- **Total migrated**: **23 / 23 — 100% complete**.
- **Remaining**: 0.

The Chroma Dragon engine is now the **routing authority** for every active color-operation site in the rain renderer, with `chroma::legacy` as the explicit auditable fallback. No bypass sites remain.
