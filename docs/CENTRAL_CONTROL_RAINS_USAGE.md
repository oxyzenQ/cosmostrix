# `central_control_rains.rs` — Usage & Custom Tuning Guide

<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

> **Document ID**: RAIN-USAGE-001
> **Date**: 2026-08
> **Scope**: `src/central_control_rains.rs` — every tunable knob in the rain visual stack
> **Audience**: cosmostrix users who want to fine-tune the rain look beyond
> the built-in presets (Option F "Film Matrix Hero" and friends)
> **Goal**: make cosmostrix flexible. If a user dislikes the default
> visual tuning, they have a documented path to customize the rain
> without diving blind into source code.

---

## 1. Why this doc exists

cosmostrix ships with a single locked visual tuning — currently
**Option F "Film Matrix Hero"** (see
[`docs/RAIN_DEPTH_AUDIT.md`](./RAIN_DEPTH_AUDIT.md) for the rationale
behind that lock). Option F was calibrated against a "hero cinematic"
reference (Blade Runner 2049 rain, Ghost in the Shell opening descent,
The Matrix dojo scene). It is the optimal tuning for *that specific*
visual target.

But not every user wants the hero-cinematic look. Some want flat
uniform rain (cyberpunk wallpaper style). Some want long smearing
trails (older CRT terminal signature). Some want maximum density
(rain-wall) or minimum density (sparse drips). Some want the back
layer pushed so deep it disappears, others want the back layer
visible as a clear depth cue.

cosmostrix is built around a **single source of truth**:
`src/central_control_rains.rs`. Every constant in that file directly
controls how the rain looks — there are no hidden tuning tables, no
"magic" runtime defaults, no per-scene overrides that would surprise
you mid-tuning. This doc is the **complete map** of that file: every
parameter, what it does, its current value, its safe tuning range,
and a recipe catalog showing how to combine parameter changes into
named alternative looks.

The default build of cosmostrix locks Option F because that is the
calibrated 10/10 lock. If you want a different look, the path is:

1. Read this doc, find the section for the look you want.
2. Either use one of the **pre-built recipes** in §6 (copy-paste a
   block of `pub const` overrides), or
3. Use the **custom tuning workflow** in §7 to design your own.
4. `cargo build --release` — done.

No call-site changes needed. Every consumer in the codebase reads
from `crate::constants::*` which re-exports this module, so changing
a constant here propagates everywhere automatically.

---

## 2. The two-layer tuning surface

cosmostrix exposes tuning at **two layers**:

### 2.1 Runtime user-facing tuning (no rebuild needed)

These are knobs exposed via CLI flags or `cosmostrix.toml` config
file. They cover the **most common** visual adjustments and can be
tweaked without rebuilding the binary.

| Knob | CLI flag | Config key | Effect |
|------|----------|------------|--------|
| Color palette | `--color <name>` | `color = "green"` | Sets the rain hue family |
| Charset | `--charset <name>` | `charset = "katakana"` | Sets glyph set per droplet |
| Frame rate | `--fps <1-240>` | `fps = 60` | Caps render rate (integer) |
| Speed | `--speed <1-100>` | `speed = 20` | Global motion speed (integer; Up/Down runtime controls use the same range) |
| Density | `--density <0.01-5.0>` | `density = 1.0` | Global droplet spawn rate multiplier |
| Glitch level | `--glitch-level <none\|subtle\|default\|intense>` | `glitch-level = "subtle"` | Visual corruption intensity preset |
| Monolith size | `--monolith-size <small\|normal\|large>` | `monolith-size = "large"` | Monolith segment cell scale (enum, not pixel size) |
| Background color | `--color-bg <black\|default-background>` | `color-bg = "black"` | Terminal background mode |
| Bold mode | `--bold <0\|1\|2>` | `bold = 1` | Bold glyph style (0=off, 1=random, 2=all) |
| Shading mode | `--shadingmode <0\|1>` | `shadingmode = 1` | Shading (0=random, 1=cinematic) |
| Color mode | `--colormode <0\|16\|256\|24>` | (CLI only) | Force color depth (auto-detected by default) |
| Auto color drift | `--auto-color-drift` | `auto-color-drift = true` | Palette scheme drift (off by default; climate drift is always-on) |
| Intro type | `--intro <logo\|cosmic\|none>` | `intro = "logo"` | Cinematic intro sequence |
| Scene custom | (toml only) | `scene-custom.<name>.<field>` | Custom scene preset |
| Colors custom | (toml only) | `colors-custom.<name>.<bg\|rain>` | Custom color palette |
| Charset custom | (toml only) | `charset-custom.<name>.set` | Custom charset |
| Color tune (global) | `--color-tune "k=v,k=v"` | `color.tune.<brightness\|saturation\|head\|body\|tail>` | Global brightness/saturation/head/body/tail multiplier |

> **Removed (2026-08-05, atmosphere engine elimination):** the
> `--atmosphere-mode` / `--atmosphere-regime` CLI flags, the
> `atmosphere-mode` / `atmosphere-regime` / `adaptive-custom.*` config
> keys, and all `atmosphere-*` scene-custom presets have been removed.
> Historical reference: `docs/archive/specs/ATMOSPHERE_ENGINE.md` and
> `docs/archive/specs/CINEMATIC_BREATHING.md`. Elimination record:
> `docs/archive/audits/ATMOSPHERE_SUBSYSTEM_ARCHIVAL.md`.

**`--color-tune` is the most powerful runtime knob.** It accepts a
comma-separated list of `key=value` pairs:

```bash
# Boost saturation by 50%, dim overall brightness by 10%, pop heads by 20%
cosmostrix --color green --color-tune "saturation=1.5,brightness=0.9,head=1.2"

# Grayscale (zero saturation)
cosmostrix --color red --color-tune "saturation=0.0"

# Brightness-only boost
cosmostrix --color aurora --color-tune "brightness=1.3"
```

Accepted keys and ranges:

| Key | Range | Default | Effect |
|-----|-------|---------|--------|
| `brightness` (or `bright`) | 0.0 – 3.0 | 1.0 | Global RGB luminance multiplier |
| `saturation` (or `sat`) | 0.0 – 3.0 | 1.0 | Color vividness (0 = grayscale, 1 = neutral, >1 = oversaturated) |
| `head` | 0.0 – 3.0 | 1.0 | Per-droplet head brightness multiplier |
| `body` | 0.0 – 3.0 | 1.0 | Per-droplet body brightness multiplier |
| `tail` | 0.0 – 3.0 | 1.0 | Per-droplet tail brightness multiplier |

This runtime tuning covers most "I want it brighter / more saturated /
softer heads" requests without needing to touch source code.

### 2.2 Source-level tuning (rebuild required)

For deeper visual changes that the runtime knobs can't reach —
per-layer depth balance, trail persistence per layer, head bloom
intensity, fog depth, vignette shape, turbulence amplitude, anomaly
frequency, etc. — you edit `src/central_control_rains.rs` directly
and rebuild.

This is the main subject of this document. The rest of the doc
covers the source-level surface in full detail.

---

## 3. The parameter map — what every section controls

`central_control_rains.rs` is organized into logical sections. Each
section controls one aspect of the rain visual. Below is the full
map, with the section name, line range, what it controls, and the
key constants you'd touch to tune it.

### 3.1 Parallax depth layers (lines 170–344)

The core of the rain visual. Three layers (back/mid/front = far/mid/near)
each have per-layer multipliers that stack to produce depth. **This
is the section Option F tunes.**

| Constant | Type | Current | Effect |
|----------|------|--------:|--------|
| `PARALLAX_LAYERS` | usize | 3 | Layer count. Do not change without major refactor — every consumer assumes 3 layers. |
| `PARALLAX_SPEED_MULT` | [f32; 3] | [0.35, 1.0, 1.7] | Per-layer motion speed. Back recedes (0.35×), front whooshes (1.7×). |
| `PARALLAX_BRIGHTNESS_MULT` | [f32; 3] | [0.48, 0.80, 1.10] | Per-droplet luminance. The hero pop lever for front. |
| `PARALLAX_SATURATION_MULT` | [f32; 3] | [0.50, 0.84, 1.12] | Color vividness. The neon signature lever for front. |
| `PARALLAX_HEAD_BLOOM_MULT` | [f32; 3] | [0.48, 0.74, 1.30] | Head glow falloff. The eye-lock trigger. |
| `PARALLAX_HEAD_SELFBLOOM_MULT` | [f32; 3] | [0.38, 0.68, 1.20] | Head self-illumination halo width. |
| `PARALLAX_LENGTH_MULT` | [f32; 3] | [0.5, 1.0, 1.4] | Droplet streak length per layer. |
| `PARALLAX_DENSITY_MULT` | [f32; 3] | [0.45, 0.62, 0.85] | Per-layer spawn density. |
| `PARALLAX_GLYPH_DIM` | [f32; 3] | [1.0, 1.0, 1.0] | Per-layer glyph brightness (rarely tuned — keep at 1.0). |
| `PARALLAX_CONTRAST_REDUCTION` | [f32; 3] | [0.55, 0.18, 0.0] | Depth-of-field fog blend. Back haze depth. |

**Tuning recipe pattern**: most "looks" you'd want to design are a
combination of edits in this section. See §6 for named recipes.

### 3.2 Phosphor persistence (lines 252–286)

Controls CRT afterglow decay — how long trails linger after a droplet
passes. This is the "movie rain streak" signature.

| Constant | Type | Current | Effect |
|----------|------|--------:|--------|
| `PHOSPHOR_DECAY_RATE` | f32 | 5.0 | Global trail decay rate. Higher = shorter trails. |
| `PHOSPHOR_TAIL_RESIDUAL` | u8 | 160 | Minimum tail brightness (0–255). Higher = trails never fully fade. |
| `PHOSPHOR_DEAD_THRESHOLD` | u8 | 6 | Below this brightness, trail cell is killed. |
| `PHOSPHOR_GLYPH_THRESHOLD` | u8 | 96 | Below this, trail cell shows no glyph (just dim glow). |
| `PHOSPHOR_LAYER_DECAY_MULT` | [f32; 3] | [2.0, 1.2, 0.6] | Per-layer decay multiplier. Back fades fastest, front slowest. |
| `PHOSPHOR_BOTTOM_ROWS` | u16 | 12 | How many bottom rows get extra decay (rain-pool effect). |
| `PHOSPHOR_BOTTOM_DECAY_MULT` | f32 | 3.0 | Bottom-row decay multiplier (3× faster fade at the bottom). |

**Tuning recipes**:
- **Longer cinematic trails**: lower `PHOSPHOR_LAYER_DECAY_MULT[2]` from 0.6 → 0.4 (front trails last ~1s instead of ~670ms). Warning: may smear on hero-bright fronts.
- **Harder CRT look (shorter, sharper)**: raise `PHOSPHOR_LAYER_DECAY_MULT[2]` from 0.6 → 0.8. Heads stay very crisp.
- **Brighter trail residual (glowing puddles)**: raise `PHOSPHOR_TAIL_RESIDUAL` from 160 → 200.
- **Darker bottom (rain doesn't pool)**: raise `PHOSPHOR_BOTTOM_DECAY_MULT` from 3.0 → 5.0.

### 3.3 Head bloom (lines 422–428 + 216–232)

Controls the Gaussian glow around droplet heads — the "phosphor
excitation" effect that makes heads read as small light sources
rather than bright pixels.

| Constant | Type | Current | Effect |
|----------|------|--------:|--------|
| `HEAD_BLOOM_SIGMA` | f32 | 1.2 | Gaussian sigma for head glow spread. Higher = wider, softer halo. |
| `HEAD_BLOOM_INTENSITY` | f32 | 0.40 | Peak head glow brightness (0–1). |
| `HEAD_BLOOM_CELLS` | u16 | 2 | How many cells around the head get bloom (radius). |
| `PARALLAX_HEAD_BLOOM_MULT` | [f32; 3] | [0.48, 0.74, 1.30] | Per-layer head bloom multiplier (Option F hero lever). |
| `PARALLAX_HEAD_SELFBLOOM_MULT` | [f32; 3] | [0.38, 0.68, 1.20] | Per-layer self-bloom halo multiplier. |

**Tuning recipes**:
- **Bigger, softer halos**: raise `HEAD_BLOOM_SIGMA` from 1.2 → 1.8 and `HEAD_BLOOM_CELLS` from 2 → 3.
- **Smaller, harder pinprick heads**: lower `HEAD_BLOOM_SIGMA` from 1.2 → 0.8 and `HEAD_BLOOM_CELLS` from 2 → 1.
- **Stronger hero pop**: raise `PARALLAX_HEAD_BLOOM_MULT[2]` from 1.30 → 1.45 (pushes head pop ratio above 9× back).

### 3.4 Atmospheric depth & fog (lines 433–470)

Controls top/bottom row dimming, vignette, and the depth fog that
makes the back layer read as "rain in haze".

| Constant | Type | Current | Effect |
|----------|------|--------:|--------|
| `FOG_ROWS` | u16 | 4 | How many top rows get fog blend. |
| `FOG_MIN_FACTOR` | f32 | 0.65 | Minimum fog brightness at the very top row (0.65 = 65% of full). |
| `CRT_VIGNETTE_HEIGHT` | u16 | 5 | Top/bottom vignette band height. |
| `CRT_VIGNETTE_EDGE_FACTOR` | f32 | 0.9 | Edge darkness factor (0.9 = 10% darker at edges). |
| `CRT_VIGNETTE_PERF_THRESHOLD` | f32 | 0.5 | Performance gate — below this FPS, vignette is skipped. |
| `VIGNETTE_INTENSITY` | f32 | 0.30 | Radial vignette strength (0–1). |
| `VIGNETTE_INNER_RADIUS` | f32 | 0.7 | Inner radius where vignette starts (0–1 of screen). |
| `VIGNETTE_LAYER_MULT` | [f32; 3] | [1.0, 1.0, 0.0] | Per-layer vignette (front exempt). |
| `RAIN_SHADOW_PCT` | f32 | 0.15 | Bottom 15% of screen gets quadratic fade. |
| `RAIN_SHADOW_LAYER_MULT` | [f32; 3] | [1.0, 1.0, 0.0] | Per-layer rain shadow (front exempt). |

**Tuning recipes**:
- **No vignette (flat full-screen)**: set `VIGNETTE_INTENSITY = 0.0` and `CRT_VIGNETTE_EDGE_FACTOR = 1.0`.
- **Stronger CRT vibe**: raise `VIGNETTE_INTENSITY` from 0.30 → 0.50 and `CRT_VIGNETTE_HEIGHT` from 5 → 8.
- **Front layer also vignetted (uniform depth)**: change `VIGNETTE_LAYER_MULT` from `[1.0, 1.0, 0.0]` → `[1.0, 1.0, 1.0]`.

### 3.5 Velocity & motion (lines 405–422)

Controls droplet motion physics.

| Constant | Type | Current | Effect |
|----------|------|--------:|--------|
| `DROPLET_GRAVITY` | f32 | 2.0 | Per-frame acceleration. |
| `DROPLET_TERMINAL_VELOCITY_MULT` | f32 | 1.8 | Cap on max velocity (× base). |
| `STARTUP_VELOCITY_FRACTION` | f32 | 0.03 | Initial velocity as fraction of terminal (cinematic ramp-up). |
| `STARTUP_EASE_TAU` | f32 | 0.30 | Ease time constant for startup ramp (lower = faster ramp). |
| `TURBULENCE_AMPLITUDE` | f32 | 0.08 | Horizontal drift oscillation amplitude. |
| `TURBULENCE_FREQ` | f32 | 0.4 | Drift oscillation frequency (Hz). |

**Tuning recipes**:
- **Faster rain (heavy downpour)**: raise `DROPLET_GRAVITY` from 2.0 → 3.0 and `DROPLET_TERMINAL_VELOCITY_MULT` from 1.8 → 2.5.
- **Slower, dreamier rain**: lower `DROPLET_GRAVITY` from 2.0 → 1.2 and `STARTUP_EASE_TAU` from 0.30 → 0.60.
- **More wind drift**: raise `TURBULENCE_AMPLITUDE` from 0.08 → 0.20.
- **No drift (straight vertical)**: set `TURBULENCE_AMPLITUDE = 0.0`.

### 3.6 Cinematic smoothness & shimmer (lines 519–547)

Controls subtle per-frame variations that give the rain "life" rather
than feeling mechanical.

| Constant | Type | Current | Effect |
|----------|------|--------:|--------|
| `FRACTIONAL_HEAD_BRIGHTNESS_AMP` | f32 | 0.15 | Per-frame head brightness flicker amplitude. |
| `FRACTIONAL_BLOOM_AMP` | f32 | 0.10 | Per-frame bloom flicker amplitude. |
| `HEAD_SHIMMER_PERIOD_SECS` | f32 | 0.10 | Head shimmer cycle period. |
| `SPAWN_PHASE_JITTER` | bool | true | Randomize spawn phase per droplet (avoids sync bands). |
| `TRAIL_CYCLE_PROBABILITY` | f32 | 0.02 | Chance per tick of mid-trail character re-randomization. |

**Tuning recipes**:
- **Calmer (less flicker)**: lower `FRACTIONAL_HEAD_BRIGHTNESS_AMP` from 0.15 → 0.05.
- **More glitchy / digital**: raise `TRAIL_CYCLE_PROBABILITY` from 0.02 → 0.10.
- **Synchronous rain (visible bands)**: set `SPAWN_PHASE_JITTER = false`.

### 3.7 Anomalies & emergent moments (lines 552–722)

Controls rare visual events — luminance anomalies, color drift,
emergent moments, gusts, atmosphere ticks.

| Constant | Type | Current | Effect |
|----------|------|--------:|--------|
| `ANOMALY_CHANCE_PER_SEC` | f64 | 0.017 | Per-second chance of a luminance anomaly (~1/min). |
| `ANOMALY_DURATION_SECS` | f32 | 1.5 | How long an anomaly lasts. |
| `ANOMALY_MAX_ZONES` | usize | 3 | Max simultaneous anomaly zones. |
| `ANOMALY_LUMINANCE_INTENSITY` | f32 | 0.3 | Anomaly brightness boost. |
| `ANOMALY_CORRUPTION_CHANCE` | f32 | 0.4 | Chance anomaly also corrupts glyphs. |
| `COLOR_ECOSYSTEM_TICK_SECS` | f32 | 3.0 | Color climate re-evaluation interval. |
| `COLOR_CLIMATE_DRIFT_RATE` | f32 | 0.008 | Luminance climate drift speed. |
| `COLOR_SATURATION_DRIFT_RATE` | f32 | 0.005 | Saturation climate drift speed. |
| `COLOR_HUE_DRIFT_RATE` | f32 | 0.015 | Hue climate drift speed. |
| `AUTONOMOUS_PALETTE_DRIFT_CHANCE` | f32 | 0.03 | Chance per ecosystem tick of autonomous palette change. |
| `AUTO_COLOR_DRIFT_DEFAULT` | bool | false | Whether auto-color-drift is on by default (CLI flag overrides). |
| `EMERGENT_MOMENT_CHANCE` | f32 | 0.08 | Chance per storytelling tick of an emergent moment. |
| `EMERGENT_MOMENT_DURATION_SECS` | f32 | 8.0 | How long an emergent moment lasts. |
| `EMERGENT_MAX_MOMENTS` | usize | 1 | Max simultaneous emergent moments. |
| `EMERGENT_LUMINANCE_INTENSITY` | f32 | 0.12 | Emergent brightness boost. |
| `EMERGENT_DENSITY_INTENSITY` | f32 | 0.25 | Emergent density boost. |
| `EMERGENT_SPEED_SHIFT` | f32 | 0.15 | Emergent speed shift (±). |
| `GUST_IDLE_MIN_SECS` | f64 | 30.0 | Min idle time between gusts. |
| `GUST_IDLE_MAX_SECS` | f64 | 120.0 | Max idle time between gusts. |
| `GUST_PEAK_MIN` | f32 | 1.2 | Min gust speed multiplier. |
| `GUST_PEAK_MAX` | f32 | 1.5 | Max gust speed multiplier. |

**Tuning recipes**:
- **Disable anomalies entirely**: set `ANOMALY_CHANCE_PER_SEC = 0.0`.
- **Disable emergent moments**: set `EMERGENT_MOMENT_CHANCE = 0.0`.
- **Disable gusts**: set `GUST_IDLE_MIN_SECS = 1e18` (effectively never).
- **More chaotic (frequent anomalies)**: raise `ANOMALY_CHANCE_PER_SEC` from 0.017 → 0.10 (about 6/min).
- **Calmer (no drift)**: set `COLOR_CLIMATE_DRIFT_RATE = 0.0`, `COLOR_SATURATION_DRIFT_RATE = 0.0`, `COLOR_HUE_DRIFT_RATE = 0.0`.

### 3.8 Spawn pacing & warm start (lines 340–362)

Controls spawn rate mechanics — how droplets enter the field.

| Constant | Type | Current | Effect |
|----------|------|--------:|--------|
| `TRAIL_EXPONENTIAL_K` | f64 | 1.2 | Exponential decay shape for trail brightness. |
| `SPAWN_REMAINDER_CAP` | f32 | 4.0 | Max accumulated spawn remainder (prevents burst catch-up). |
| `ADVANCE_REMAINDER_CAP` | f32 | 3.0 | Max accumulated advance remainder. |
| `WARM_START_MAX_HEAD` | u16 | 8 | Max droplets with heads during warm start. |
| `WARM_START_SEED_FRACTION` | f32 | 0.12 | Fraction of columns seeded with droplets at startup. |
| `WARM_START_SEED_MIN` | usize | 3 | Min seeded columns (small terminals). |
| `WARM_START_SEED_MAX` | usize | 12 | Max seeded columns (large terminals). |
| `WARM_START_SPAWN_DEBT` | f32 | 0.5 | Initial spawn debt (delays first new spawns slightly). |

**Tuning recipes**:
- **Faster field fill at startup**: raise `WARM_START_SEED_FRACTION` from 0.12 → 0.25.
- **Slower, more cinematic fill**: lower `WARM_START_SEED_FRACTION` from 0.12 → 0.05.

### 3.9 Glitch system (lines 819–828)

Controls visual corruption — the "Matrix glitch" effect.

| Constant | Type | Current | Effect |
|----------|------|--------:|--------|
| `GLITCH_THRESHOLD` | f32 | 0.35 | Density threshold above which glitch can trigger. |
| `GLITCH_BRIGHT_RATIO` | f64 | 0.25 | Fraction of glitch cells that are bright. |
| `GLITCH_DIM_RATIO` | f64 | 0.75 | Fraction of glitch cells that are dim. |

**Tuning recipes**:
- **More glitchy**: raise `GLITCH_THRESHOLD` from 0.35 → 0.20 (triggers more often) and adjust `GLITCH_BRIGHT_RATIO` from 0.25 → 0.40.
- **Disable glitch**: set `GLITCH_THRESHOLD = 1.0` (never triggers).

### 3.10 Front layer tail (lines 479–485)

Controls the long-stream tail allocation for the front layer.

| Constant | Type | Current | Effect |
|----------|------|--------:|--------|
| `FRONT_LAYER_TAIL_PCT` | f32 | 0.45 | Fraction of front droplets that get long tails. |
| `FRONT_LAYER_TAIL_MAX_CELLS` | u8 | 12 | Max cells in a long tail. |
| `FRONT_LAYER_MAX_TAIL_STOPS` | u8 | 3 | Max brightness stops in tail gradient. |

**Tuning recipes**:
- **More long streaks**: raise `FRONT_LAYER_TAIL_PCT` from 0.45 → 0.70.
- **Shorter max streaks**: lower `FRONT_LAYER_TAIL_MAX_CELLS` from 12 → 6.

### 3.11 Performance gating (lines 765–858)

Constants that gate visual fidelity based on performance. **Touch
these only if you understand the perf trade-offs.**

| Constant | Type | Current | Effect |
|----------|------|--------:|--------|
| `EVENT_PERF_GATE` | f32 | 0.5 | FPS below which events (anomalies, gusts) are suppressed. |
| `PERF_SPAWN_SCALE_MIN` | f32 | 0.25 | Min spawn scale under SIM pressure. |
| `GLITCH_THRESHOLD` | f32 | 0.35 | (See §3.9.) |
| `SIM_PRESSURE_SCALE_FACTOR` | f64 | 0.7 | How aggressively sim pressure scales spawn. |
| `SIM_MIN_FRACTION` | f64 | 0.5 | Min simulation fraction under pressure. |
| `SIM_MAX_CAP_SECS` | f64 | 1.0/30.0 | Max sim time catch-up per frame. |
| `SIM_BASE_MULTIPLIER` | f64 | 3.0 | Base sim steps per render frame. |
| `DENSITY_STEP` | f32 | 0.25 | Density quantization step. |
| `WATCHDOG_INTERVAL_SECS` | u64 | 1 | Watchdog check interval. |
| `FRAME_SPIN_BUDGET` | Duration | 500µs | Time budget for spin-wait before yielding. |
| `FRAME_SPIN_LIMIT` | Duration | 1000µs | Hard cap on spin-wait. |
| `SIM_FACTOR_MIN` | f64 | 0.3 | Min sim factor (below this, visuals degrade). |

### 3.12 Mouse interactions (lines 490–514)

Controls the mouse glow / flash effect when the user moves the cursor
over the rain.

| Constant | Type | Current | Effect |
|----------|------|--------:|--------|
| `MOUSE_GLOW_RADIUS_COLS` | f32 | 7.0 | Glow radius in columns. |
| `MOUSE_GLOW_RADIUS_LINES` | f32 | 5.0 | Glow radius in lines. |
| `MOUSE_GLOW_INTENSITY` | f32 | 0.0 | Static glow intensity (0 = off by default). |
| `MOUSE_FLASH_SPEED` | f32 | 32.0 | Flash expansion speed. |
| `MOUSE_FLASH_RING_WIDTH` | f32 | 8.0 | Flash ring width. |
| `MOUSE_FLASH_INTENSITY` | f32 | 0.85 | Flash peak intensity. |
| `MOUSE_FLASH_DURATION_SECS` | f32 | 1.8 | Flash total duration. |
| `MOUSE_FLASH_SECONDARY_FRAC` | f32 | 0.45 | Fraction of secondary flash. |
| `MOUSE_FLASH_SECONDARY_SPEED_FRAC` | f32 | 0.4 | Secondary flash speed fraction. |

**Tuning recipes**:
- **Always-on mouse glow**: set `MOUSE_GLOW_INTENSITY = 0.5`.
- **Bigger flash**: raise `MOUSE_FLASH_RING_WIDTH` from 8.0 → 14.0.
- **Disable flash entirely**: set `MOUSE_FLASH_INTENSITY = 0.0`.

### 3.13 Monolith scene (lines 886–898)

Controls the "monolith" mode — code-rain wall with per-layer glyph
streams.

| Constant | Type | Current | Effect |
|----------|------|--------:|--------|
| `MONOLITH_LAYER_BRIGHTNESS` | [f32; 3] | [0.48, 0.78, 1.0] | Per-layer monolith brightness. |
| `MONOLITH_BREATHING_AMPLITUDE` | [f32; 3] | [0.018, 0.026, 0.034] | Per-layer breathing ±%. |

### 3.14 RNG & character pools (lines 794–810)

| Constant | Type | Current | Effect |
|----------|------|--------:|--------|
| `CHAR_POOL_SIZE` | usize | 2048 | Glyph pool size for non-monolith modes. |
| `GLITCH_POOL_SIZE` | usize | 1024 | Glyph pool size for glitch cells. |
| `MAX_CHAR_POOL_IDX` | u16 | 2047 | Max glyph pool index. |
| `RNG_RESEED_INTERVAL_SECS` | u64 | 600 | RNG reseed interval. |
| `RNG_INITIAL_SEED` | u64 | 0x0123_4567 | Initial RNG seed. |
| `EVENT_RNG_XOR` | u64 | 0xCAFE_BABE_1337_0420 | XOR fold for event RNG. |

**Tuning recipe**: change `RNG_INITIAL_SEED` for a different
"deterministic rain pattern" — useful for reproducible benchmarking
or for users who want the same rain every launch.

### 3.15 Edge fade & redraw (lines 739–814)

| Constant | Type | Current | Effect |
|----------|------|--------:|--------|
| `EDGE_FADE_ROWS` | u16 | 3 | Top edge fade rows. |
| `EDGE_FADE_BOTTOM_ROWS` | u16 | 12 | Bottom edge fade rows. |
| `EDGE_FADE_BOTTOM_LIP` | f32 | 0.75 | Bottom lip fade factor. |
| `EDGE_FADE_TOP_MIN` | f32 | 0.70 | Top min brightness. |
| `EDGE_FADE_BOTTOM_MIN` | f32 | 0.35 | Bottom min brightness. |
| `EDGE_FADE_BOLD_THRESHOLD` | f32 | 0.5 | Brightness below which bold is suppressed at edges. |
| `PHOSPHOR_EDGE_ENERGY_CAP` | u8 | 64 | Edge phosphor energy cap. |
| `PHOSPHOR_EDGE_ROW_TAPER` | u8 | 8 | Edge row taper count. |
| `HEAD_LINGER_BRIGHTNESS_MS` | u64 | 300 | How long a killed head stays bright (ms). |
| `FULL_REDRAW_INTERVAL_FRAMES` | u64 | 18000 | Force full redraw every N frames (~5min at 60fps). |

---

## 4. The full constant catalog (compact reference)

Below is the complete list of all 110+ constants in
`central_control_rains.rs`, grouped by section, with current values.
Use this as a quick lookup when designing a custom tuning.

```
=== Parallax depth layers ===
PARALLAX_LAYERS                    = 3
PARALLAX_SPEED_MULT                = [0.35, 1.0, 1.7]
PARALLAX_BRIGHTNESS_MULT           = [0.48, 0.80, 1.10]
PARALLAX_SATURATION_MULT           = [0.50, 0.84, 1.12]
PARALLAX_HEAD_BLOOM_MULT           = [0.48, 0.74, 1.30]
PARALLAX_HEAD_SELFBLOOM_MULT       = [0.38, 0.68, 1.20]
PARALLAX_LENGTH_MULT               = [0.5, 1.0, 1.4]
PARALLAX_DENSITY_MULT              = [0.45, 0.62, 0.85]
PARALLAX_GLYPH_DIM                 = [1.0, 1.0, 1.0]
PARALLAX_CONTRAST_REDUCTION        = [0.55, 0.18, 0.0]

=== Phosphor persistence ===
PHOSPHOR_DECAY_RATE                = 5.0
PHOSPHOR_TAIL_RESIDUAL             = 160
PHOSPHOR_DEAD_THRESHOLD            = 6
PHOSPHOR_GLYPH_THRESHOLD           = 96
PHOSPHOR_LAYER_DECAY_MULT          = [2.0, 1.2, 0.6]
PHOSPHOR_BOTTOM_ROWS               = 12
PHOSPHOR_BOTTOM_DECAY_MULT         = 3.0

=== Head bloom ===
HEAD_BLOOM_SIGMA                   = 1.2
HEAD_BLOOM_INTENSITY               = 0.40
HEAD_BLOOM_CELLS                   = 2

=== Atmospheric depth & fog ===
FOG_ROWS                           = 4
FOG_MIN_FACTOR                     = 0.65
CRT_VIGNETTE_HEIGHT                = 5
CRT_VIGNETTE_EDGE_FACTOR           = 0.9
CRT_VIGNETTE_PERF_THRESHOLD        = 0.5
VIGNETTE_INTENSITY                 = 0.30
VIGNETTE_INNER_RADIUS              = 0.7
VIGNETTE_LAYER_MULT                = [1.0, 1.0, 0.0]
RAIN_SHADOW_PCT                    = 0.15
RAIN_SHADOW_LAYER_MULT             = [1.0, 1.0, 0.0]

=== Velocity & motion ===
DROPLET_GRAVITY                    = 2.0
DROPLET_TERMINAL_VELOCITY_MULT     = 1.8
STARTUP_VELOCITY_FRACTION          = 0.03
STARTUP_EASE_TAU                   = 0.30
TURBULENCE_AMPLITUDE               = 0.08
TURBULENCE_FREQ                    = 0.4

=== Cinematic smoothness ===
FRACTIONAL_HEAD_BRIGHTNESS_AMP     = 0.15
FRACTIONAL_BLOOM_AMP               = 0.10
HEAD_SHIMMER_PERIOD_SECS           = 0.10
SPAWN_PHASE_JITTER                 = true
TRAIL_CYCLE_PROBABILITY            = 0.02

=== Anomalies ===
ANOMALY_CHANCE_PER_SEC             = 0.017
ANOMALY_DURATION_SECS              = 1.5
ANOMALY_MAX_ZONES                  = 3
ANOMALY_LUMINANCE_INTENSITY        = 0.3
ANOMALY_CORRUPTION_CHANCE          = 0.4

=== Color ecosystem ===
COLOR_ECOSYSTEM_TICK_SECS          = 3.0
COLOR_CLIMATE_DRIFT_RATE           = 0.008
COLOR_SATURATION_DRIFT_RATE        = 0.005
COLOR_HUE_DRIFT_RATE               = 0.015
COLOR_DRIFT_REEVAL_CHANCE          = 0.15
COLOR_LUMINANCE_CLIMATE_MIN        = 0.75
COLOR_LUMINANCE_CLIMATE_MAX        = 1.0
COLOR_SATURATION_CLIMATE_MIN       = 0.7
COLOR_SATURATION_CLIMATE_MAX       = 1.0
AUTONOMOUS_PALETTE_DRIFT_CHANCE    = 0.03
AUTO_COLOR_DRIFT_DEFAULT           = false

=== Profile transitions ===
PROFILE_TRANSITION_SECS            = 30.0
PROFILE_INTERPOLATION_RATE         = 0.02

=== Atmosphere ===
ATMOSPHERE_TICK_SECS               = 5.0
ENTROPY_CYCLE_SECS                 = 300.0
ATMOSPHERE_DENSITY_RANGE           = 0.4
ATMOSPHERE_LUMINANCE_RANGE         = 0.2
ATMOSPHERE_ANOMALY_RANGE           = 0.5

=== Density noise ===
DENSITY_NOISE_PERIOD_SECS          = 10.0
DENSITY_NOISE_MIN                  = 0.6
DENSITY_NOISE_MAX                  = 1.4
DENSITY_NOISE_HASH_K               = 2_654_435_761
DENSITY_NOISE_HASH_SEED_K          = 1_103_515_245

=== Gusts ===
GUST_IDLE_MIN_SECS                 = 30.0
GUST_IDLE_MAX_SECS                 = 120.0
GUST_ATTACK_MIN_SECS               = 1.0
GUST_ATTACK_MAX_SECS               = 2.0
GUST_HOLD_MIN_SECS                 = 0.5
GUST_HOLD_MAX_SECS                 = 1.0
GUST_DECAY_MIN_SECS                = 3.0
GUST_DECAY_MAX_SECS                = 5.0
GUST_PEAK_MIN                      = 1.2
GUST_PEAK_MAX                      = 1.5

=== Memory & storytelling ===
MEMORY_HISTORY_SAMPLES             = 32
MEMORY_SAMPLE_INTERVAL_SECS        = 30.0
MEMORY_ANOMALY_PRESSURE_WEIGHT     = 0.3
MEMORY_CALM_PERSISTENCE_BOOST      = 0.15
STORYTELLING_TICK_SECS             = 10.0

=== Emergent moments ===
EMERGENT_MOMENT_CHANCE             = 0.08
EMERGENT_MOMENT_DURATION_SECS      = 8.0
EMERGENT_MAX_MOMENTS               = 1
EMERGENT_LUMINANCE_INTENSITY       = 0.12
EMERGENT_DENSITY_INTENSITY         = 0.25
EMERGENT_SPEED_SHIFT               = 0.15

=== Easing ===
RESUME_EASE_DURATION_SECS          = 0.45
PAUSE_EASE_DURATION_SECS           = 0.30

=== Edge fade ===
EDGE_FADE_ROWS                     = 3
EDGE_FADE_BOTTOM_ROWS              = 12
EDGE_FADE_BOTTOM_LIP               = 0.75
EDGE_FADE_TOP_MIN                  = 0.70
EDGE_FADE_BOTTOM_MIN               = 0.35
EDGE_FADE_BOLD_THRESHOLD           = 0.5
PHOSPHOR_EDGE_ENERGY_CAP           = 64
PHOSPHOR_EDGE_ROW_TAPER            = 8

=== Mouse ===
MOUSE_GLOW_RADIUS_COLS             = 7.0
MOUSE_GLOW_RADIUS_LINES            = 5.0
MOUSE_GLOW_INTENSITY               = 0.0
MOUSE_FLASH_SPEED                  = 32.0
MOUSE_FLASH_RING_WIDTH             = 8.0
MOUSE_FLASH_INTENSITY              = 0.85
MOUSE_FLASH_DURATION_SECS          = 1.8
MOUSE_FLASH_SECONDARY_FRAC         = 0.45
MOUSE_FLASH_SECONDARY_SPEED_FRAC   = 0.4

=== Front layer tail ===
FRONT_LAYER_TAIL_PCT               = 0.45
FRONT_LAYER_TAIL_MAX_CELLS         = 12
FRONT_LAYER_MAX_TAIL_STOPS         = 3

=== Spawn pacing ===
TRAIL_EXPONENTIAL_K                = 1.2
SPAWN_REMAINDER_CAP                = 4.0
ADVANCE_REMAINDER_CAP              = 3.0

=== Warm start ===
WARM_START_MAX_HEAD                = 8
WARM_START_SEED_FRACTION           = 0.12
WARM_START_SEED_MIN                = 3
WARM_START_SEED_MAX                = 12
WARM_START_SPAWN_DEBT              = 0.5

=== Color transitions ===
MAX_PALETTE_SLOTS                  = 4
COLOR_TRANSITION_DURATION_MS       = 300
COLOR_TRANSITION_INITIAL_VISIBLE_PCT = 0.12
CHARSET_TRANSITION_DURATION_MS     = 500
TRANSITION_VELOCITY_BOOST          = 0.05
TRANSITION_ENERGY_DURATION_SECS    = 1.5
TRANSITION_ENERGY_SATURATION_BOOST = 0.15
TRANSITION_HEAD_GLOW_BOOST         = 0.2

=== Glyph entry ramp ===
GLYPH_ENTRY_RAMP_DURATION_MS       = 700
GLYPH_ENTRY_RAMP_MIN_SCALE         = 0.15

=== Event gating ===
EVENT_RNG_XOR                      = 0xCAFE_BABE_1337_0420
EVENT_PERF_GATE                    = 0.5

=== Ghosts ===
GHOST_SPAWN_CHANCE_PER_TICK        = 0.003
GHOST_MAX_ACTIVE                   = 1

=== Droplet count & length ===
DROPLET_COUNT_FACTOR               = 1.5
MIN_DROPLET_LENGTH                 = 4
MAX_DROPLET_LENGTH_CAP             = 200

=== Pools & RNG ===
CHAR_POOL_SIZE                     = 2048
GLITCH_POOL_SIZE                   = 1024
MAX_CHAR_POOL_IDX                  = 2047
RNG_RESEED_INTERVAL_SECS           = 600
RNG_INITIAL_SEED                   = 0x0123_4567

=== Head linger & redraw ===
HEAD_LINGER_BRIGHTNESS_MS          = 300
FULL_REDRAW_INTERVAL_FRAMES        = 18000

=== Performance ===
PERF_SPAWN_SCALE_MIN               = 0.25
SIM_PRESSURE_SCALE_FACTOR          = 0.7
SIM_MIN_FRACTION                   = 0.5
SIM_MAX_CAP_SECS                   = 0.0333
SIM_BASE_MULTIPLIER                = 3.0
DENSITY_STEP                       = 0.25
WATCHDOG_INTERVAL_SECS             = 1
FRAME_SPIN_BUDGET                  = 500µs
FRAME_SPIN_LIMIT                   = 1000µs
SIM_FACTOR_MIN                     = 0.3

=== Glitch ===
GLITCH_THRESHOLD                   = 0.35
GLITCH_BRIGHT_RATIO                = 0.25
GLITCH_DIM_RATIO                   = 0.75

=== Monolith ===
MONOLITH_LAYER_BRIGHTNESS          = [0.48, 0.78, 1.0]
MONOLITH_BREATHING_AMPLITUDE       = [0.018, 0.026, 0.034]
```

---

## 5. Tuning safety rules

Before you start editing constants, read these rules. They exist
because every one of them was learned the hard way.

### Rule 1 — Don't change `PARALLAX_LAYERS`

`PARALLAX_LAYERS = 3` is assumed everywhere. The spawn distribution
is hardcoded `[0.35, 0.30, 0.35]` in `src/cloud/spawn.rs`. Many
consumers index `[0]`, `[1]`, `[2]` directly. Changing this to 2 or
4 will require a major refactor across multiple files. Do not touch.

### Rule 2 — Per-layer arrays must sum to a sane range

For `PARALLAX_BRIGHTNESS_MULT`, `PARALLAX_SATURATION_MULT`,
`PARALLAX_HEAD_BLOOM_MULT`, `PARALLAX_HEAD_SELFBLOOM_MULT`:

- `front > mid > back` (depth gradient preserved)
- `front ≤ 1.50` (above this, clips into neon noise on bright themes)
- `back ≥ 0.30` (below this, back layer disappears)

For `PARALLAX_CONTRAST_REDUCTION`:

- `back > mid > front` (haze depth gradient)
- `back ≤ 0.65` (above this, back layer milks out)
- `front = 0.0` (front is the sharp focal plane — never fog it)

For `PHOSPHOR_LAYER_DECAY_MULT`:

- `back > mid > front` (front trails persist longest)
- `front ≥ 0.30` (below this, trails smear on hero-bright fronts)
- `back ≤ 3.0` (above this, back layer flickers too fast)

### Rule 3 — Field energy front share must stay in 60–75%

Compute `front_share = front_FE / (back_FE + mid_FE + front_FE)`
where `FE = brightness × saturation × (1 - contrast_red) × density × layer_dist`.

If `front_share < 60%`, the eye loses the focal plane (rain reads
flat). If `front_share > 75%`, depth gradient collapses (rain reads
as a single bright front wall). Option F sits at 75.4% — at the
upper edge, do not push further.

### Rule 4 — Don't disable `SPAWN_PHASE_JITTER` unless you want visible bands

When `SPAWN_PHASE_JITTER = false`, droplets spawn synchronously per
column, producing visible horizontal "bands" of rain. This is
sometimes desirable for a "synced digital rain" look but breaks the
cinematic feel. Default is `true`.

### Rule 5 — Performance gating constants need benchmarking

Constants in §3.11 (`SIM_*`, `FRAME_SPIN_*`, `PERF_SPAWN_SCALE_MIN`)
are calibrated for a balance of visual fidelity and frame-rate
stability. Changing them without benchmarking on your target
hardware can cause frame drops, simulation stutters, or watchdog
trips. If you must tune them, use `--bench-frames 10000 --bench-io`
before and after to measure impact.

### Rule 6 — Test on multiple terminal sizes

A tuning that looks great on 80×24 may look empty on 200×60 or
crushed on 40×12. After any visual tuning change, test on at least:

- 80×24 (standard terminal)
- 120×40 (large terminal)
- 200×60 (huge terminal)
- 40×12 (small terminal / tmux pane)

The `PARALLAX_DENSITY_MULT` array is the most size-sensitive —
small terminals need higher density to feel "full", large terminals
need lower density to avoid feeling "busy".

### Rule 7 — Don't change `RNG_INITIAL_SEED` if you want random rain

`RNG_INITIAL_SEED = 0x0123_4567` is fixed by default so that
benchmark runs are reproducible. If you want a different rain
pattern on every launch, you'd need to modify the seeding logic
in `src/main.rs` to seed from system time — not just change this
constant.

---

## 6. Recipe catalog — pre-built alternative looks

These recipes are copy-paste blocks. Each one is a complete
alternative to Option F that produces a specific named look.
To use one, edit `src/central_control_rains.rs` and replace the
listed constants with the values shown, then `cargo build --release`.

### 6.1 Recipe — Option A "Baseline" (softer hero, longer trails)

The Option F predecessor. Softer hero pop, longer smearing trails,
back layer slightly more present. Use this if Option F feels too
"aggressive" on your terminal.

```rust
pub const PARALLAX_BRIGHTNESS_MULT: [f32; PARALLAX_LAYERS] = [0.48, 0.80, 1.05];
pub const PARALLAX_SATURATION_MULT: [f32; PARALLAX_LAYERS] = [0.50, 0.84, 1.05];
pub const PARALLAX_HEAD_BLOOM_MULT: [f32; PARALLAX_LAYERS] = [0.48, 0.74, 1.15];
pub const PARALLAX_HEAD_SELFBLOOM_MULT: [f32; PARALLAX_LAYERS] = [0.38, 0.68, 1.15];
pub const PARALLAX_CONTRAST_REDUCTION: [f32; PARALLAX_LAYERS] = [0.45, 0.18, 0.0];
pub const PHOSPHOR_LAYER_DECAY_MULT: [f32; PARALLAX_LAYERS] = [2.0, 1.2, 0.4];
```

Field energy ratio: 1 : 4.9 : 15.8. Front share: 72.7%. Visual
rating: 9/10 (good but not the hero lock).

### 6.2 Recipe — "Flat Cyberpunk Wall" (uniform depth, no hero)

All three layers at similar intensity — reads as a "wall of rain"
rather than a depth field. Good for cyberpunk wallpaper aesthetics
where you don't want a focal plane.

```rust
pub const PARALLAX_BRIGHTNESS_MULT: [f32; PARALLAX_LAYERS] = [0.70, 0.85, 1.00];
pub const PARALLAX_SATURATION_MULT: [f32; PARALLAX_LAYERS] = [0.75, 0.88, 1.00];
pub const PARALLAX_HEAD_BLOOM_MULT: [f32; PARALLAX_LAYERS] = [0.65, 0.80, 1.00];
pub const PARALLAX_HEAD_SELFBLOOM_MULT: [f32; PARALLAX_LAYERS] = [0.55, 0.72, 1.00];
pub const PARALLAX_CONTRAST_REDUCTION: [f32; PARALLAX_LAYERS] = [0.20, 0.10, 0.0];
pub const PHOSPHOR_LAYER_DECAY_MULT: [f32; PARALLAX_LAYERS] = [1.5, 1.3, 1.1];
pub const VIGNETTE_LAYER_MULT: [f32; PARALLAX_LAYERS] = [1.0, 1.0, 1.0];
pub const RAIN_SHADOW_LAYER_MULT: [f32; PARALLAX_LAYERS] = [1.0, 1.0, 1.0];
```

Field energy ratio: ~1 : 1.5 : 2.2. Front share: ~50%. No hero —
the eye roams freely. Trade-off: loses the cinematic depth feel.

### 6.3 Recipe — "Sparse Drip Mode" (minimal, meditative)

Very few droplets, long trails, slow motion. Reads as "rain on a
window" rather than "rain in a city". Good for meditative / lo-fi
focus backgrounds.

```rust
pub const PARALLAX_DENSITY_MULT: [f32; PARALLAX_LAYERS] = [0.20, 0.30, 0.45];
pub const PARALLAX_LENGTH_MULT: [f32; PARALLAX_LAYERS] = [0.7, 1.2, 1.8];
pub const PHOSPHOR_LAYER_DECAY_MULT: [f32; PARALLAX_LAYERS] = [1.5, 0.9, 0.35];
pub const DROPLET_GRAVITY: f32 = 1.2;
pub const DROPLET_TERMINAL_VELOCITY_MULT: f32 = 1.2;
pub const TURBULENCE_AMPLITUDE: f32 = 0.15;
```

Trade-off: feels empty on small terminals. Best on 120×40+.

### 6.4 Recipe — "Heavy Downpour" (dense, fast, short trails)

Maximum density, fast motion, short crisp trails. Reads as a
"monsoon" or "heavy storm". Good for high-energy scenes.

```rust
pub const PARALLAX_DENSITY_MULT: [f32; PARALLAX_LAYERS] = [0.80, 1.10, 1.40];
pub const PARALLAX_LENGTH_MULT: [f32; PARALLAX_LAYERS] = [0.4, 0.7, 1.0];
pub const PHOSPHOR_LAYER_DECAY_MULT: [f32; PARALLAX_LAYERS] = [3.0, 2.0, 1.2];
pub const DROPLET_GRAVITY: f32 = 3.0;
pub const DROPLET_TERMINAL_VELOCITY_MULT: f32 = 2.5;
pub const TURBULENCE_AMPLITUDE: f32 = 0.20;
pub const TURBULENCE_FREQ: f32 = 0.6;
```

Trade-off: may drop FPS on large terminals. Watch the watchdog.

### 6.5 Recipe — "Retro CRT" (long trails, hard heads, scanline vibe)

Strong CRT signature — long phosphor trails, sharp hard heads,
stronger vignette. Use this if you want the rain to feel like it's
running on a 1990s broadcast monitor.

```rust
pub const HEAD_BLOOM_SIGMA: f32 = 0.8;
pub const HEAD_BLOOM_CELLS: u16 = 1;
pub const PARALLAX_HEAD_BLOOM_MULT: [f32; PARALLAX_LAYERS] = [0.40, 0.70, 1.45];
pub const PHOSPHOR_LAYER_DECAY_MULT: [f32; PARALLAX_LAYERS] = [1.8, 1.0, 0.35];
pub const PHOSPHOR_TAIL_RESIDUAL: u8 = 200;
pub const VIGNETTE_INTENSITY: f32 = 0.50;
pub const CRT_VIGNETTE_HEIGHT: u16 = 8;
pub const CRT_VIGNETTE_EDGE_FACTOR: f32 = 0.75;
pub const FOG_ROWS: u16 = 6;
pub const FOG_MIN_FACTOR: f32 = 0.55;
```

Trade-off: heads may clip on bright themes (white-bg). Best on
black-bg.

### 6.6 Recipe — "Glitch Storm" (heavy corruption, frequent anomalies)

Maximum glitch and anomaly activity — reads as a "matrix
destabilization" sequence. Good for cyberpunk horror or
"system failure" scenes.

```rust
pub const GLITCH_THRESHOLD: f32 = 0.15;
pub const GLITCH_BRIGHT_RATIO: f64 = 0.40;
pub const GLITCH_DIM_RATIO: f64 = 0.60;
pub const ANOMALY_CHANCE_PER_SEC: f64 = 0.15;
pub const ANOMALY_DURATION_SECS: f32 = 2.5;
pub const ANOMALY_MAX_ZONES: usize = 5;
pub const ANOMALY_LUMINANCE_INTENSITY: f32 = 0.5;
pub const ANOMALY_CORRUPTION_CHANCE: f32 = 0.7;
pub const TRAIL_CYCLE_PROBABILITY: f32 = 0.08;
pub const EMERGENT_MOMENT_CHANCE: f32 = 0.20;
pub const EMERGENT_LUMINANCE_INTENSITY: f32 = 0.25;
```

Trade-off: visual chaos may be distracting. Use sparingly.

### 6.7 Recipe — "Calm Ambient" (no anomalies, no drift, no gusts)

Strip all dynamic events. Pure steady rain with no surprises. Good
for focus / coding backgrounds where you don't want any visual
interruptions.

```rust
pub const ANOMALY_CHANCE_PER_SEC: f64 = 0.0;
pub const EMERGENT_MOMENT_CHANCE: f32 = 0.0;
pub const GUST_IDLE_MIN_SECS: f64 = 1e18;
pub const GUST_IDLE_MAX_SECS: f64 = 1e18;
pub const COLOR_CLIMATE_DRIFT_RATE: f32 = 0.0;
pub const COLOR_SATURATION_DRIFT_RATE: f32 = 0.0;
pub const COLOR_HUE_DRIFT_RATE: f32 = 0.0;
pub const AUTONOMOUS_PALETTE_DRIFT_CHANCE: f32 = 0.0;
pub const GHOST_SPAWN_CHANCE_PER_TICK: f64 = 0.0;
```

Trade-off: feels less "alive" — the rain loses its subtle
variation. Combine with §6.5 (retro CRT) for a "tube monitor in a
quiet room" feel.

### 6.8 Recipe — "Hyper Hero" (push Option F further — at your own risk)

Option F sits at the upper edge of the masterclass envelope. If you
want to push past it for a hyper-cinematic "all-front" look, this
recipe pushes every front lever harder. **Untested — likely too
aggressive on most terminals.** A/B test before committing.

```rust
pub const PARALLAX_BRIGHTNESS_MULT: [f32; PARALLAX_LAYERS] = [0.45, 0.78, 1.20];
pub const PARALLAX_SATURATION_MULT: [f32; PARALLAX_LAYERS] = [0.45, 0.82, 1.20];
pub const PARALLAX_HEAD_BLOOM_MULT: [f32; PARALLAX_LAYERS] = [0.40, 0.70, 1.50];
pub const PARALLAX_HEAD_SELFBLOOM_MULT: [f32; PARALLAX_LAYERS] = [0.35, 0.65, 1.35];
pub const PARALLAX_CONTRAST_REDUCTION: [f32; PARALLAX_LAYERS] = [0.60, 0.20, 0.0];
pub const PHOSPHOR_LAYER_DECAY_MULT: [f32; PARALLAX_LAYERS] = [2.2, 1.3, 0.55];
```

Field energy ratio: ~1 : 6.5 : 26. Front share: ~78% (above the
75% masterclass bound). The eye locks hard onto front but may lose
depth perception. Watch for clipping on bright themes.

---

## 7. Custom tuning workflow — designing your own look

If none of the recipes in §6 fit, design your own. The workflow
below is the same one used to calibrate Option F.

### Step 1 — Define your visual target in one sentence

Examples:
- "I want sparse neon drips on a black background, no depth, calm."
- "I want a dense cyberpunk wall with frequent glitches."
- "I want a CRT-like rain with long trails and hard heads."

Without a one-sentence target, you'll tune blindly. Write it down
first.

### Step 2 — Identify which sections affect your target

Map your target to sections in §3:

| Target phrase | Sections to touch |
|----------------|-------------------|
| "sparse" / "dense" | §3.1 (PARALLAX_DENSITY_MULT), §3.8 (WARM_START_SEED_FRACTION) |
| "bright" / "dim" | §3.1 (PARALLAX_BRIGHTNESS_MULT), §3.3 (HEAD_BLOOM_INTENSITY) |
| "long trails" / "short trails" | §3.2 (PHOSPHOR_LAYER_DECAY_MULT, PHOSPHOR_TAIL_RESIDUAL) |
| "depth" / "flat" | §3.1 (PARALLAX_CONTRAST_REDUCTION, VIGNETTE_LAYER_MULT) |
| "neon" / "desaturated" | §3.1 (PARALLAX_SATURATION_MULT) |
| "hero pop" / "soft heads" | §3.3 (PARALLAX_HEAD_BLOOM_MULT, HEAD_BLOOM_SIGMA) |
| "fast" / "slow" | §3.5 (DROPLET_GRAVITY, DROPLET_TERMINAL_VELOCITY_MULT) |
| "windy" / "straight" | §3.5 (TURBULENCE_AMPLITUDE) |
| "glitchy" / "clean" | §3.9 (GLITCH_THRESHOLD, TRAIL_CYCLE_PROBABILITY) |
| "events" / "calm" | §3.7 (ANOMALY_*, EMERGENT_*, GUST_*) |
| "CRT vibe" | §3.2 (PHOSPHOR_*), §3.4 (VIGNETTE_*, CRT_VIGNETTE_*) |

### Step 3 — Make one change at a time

Don't edit six constants at once. Change one, rebuild, look, decide.
The visual impact of each constant is non-obvious and interactions
are strong. The Option F calibration took ~20 iterations of
single-change-test cycles.

### Step 4 — Use `--color-tune` for fast iteration

Before editing source, try the runtime `--color-tune` knob. If your
desired look is just "brighter / more saturated / softer heads",
`--color-tune` may give you 80% of the way there without a rebuild:

```bash
# Try this first — no rebuild needed
cosmostrix --color green --color-tune "saturation=1.4,brightness=1.1,head=1.2,tail=0.8"
```

If `--color-tune` gets you close but not quite, then edit source
for the remaining 20%.

### Step 5 — Test on at least 3 terminal sizes

Per Rule 6 in §5. A tuning that looks perfect on 80×24 may look
broken on 200×60. Test on small / medium / large.

### Step 6 — Compute your field energy ratio

After settling on values, compute the field energy ratio to verify
you're still in the masterclass envelope (or deliberately outside
it, if that's your target):

```python
BRIGHT  = [0.48, 0.80, 1.10]  # your values
SAT     = [0.50, 0.84, 1.12]
CONTRAST= [0.55, 0.18, 0.00]
DENSITY = [0.45, 0.62, 0.85]
layer_dist = [0.35, 0.30, 0.35]

vis = [BRIGHT[i]*SAT[i]*(1-CONTRAST[i]) for i in range(3)]
spawn_rate = [layer_dist[i]*DENSITY[i] for i in range(3)]
field_energy = [vis[i]*spawn_rate[i] for i in range(3)]
total = sum(field_energy)
front_share = field_energy[2] / total

print(f"Visibility:        {[round(v,3) for v in vis]}")
print(f"Field energy:      {[round(f,3) for f in field_energy]}")
print(f"Layer share:       {[round(f/total*100,1) for f in field_energy]}%")
print(f"Front share:       {front_share*100:.1f}%  (masterclass: 60-75%)")
```

If `front_share` is in 60–75%, you're in the cinematic envelope.
Below 60% = flat. Above 75% = hero-pushed (Option F territory).
Above 80% = neon noise (likely too aggressive).

### Step 7 — Document your tuning

If your custom tuning works well, save it as a recipe in your fork
or share it with the project. Add an entry to §6 of this doc
(fork) with:

- Recipe name
- One-sentence visual target
- The `pub const` block
- Field energy ratio + front share
- Trade-offs / when to use

---

## 8. Common tuning pitfalls

### 8.1 "I changed `PARALLAX_BRIGHTNESS_MULT[2]` to 1.5 but the front looks washed out, not brighter"

You pushed past the clipping threshold. Above ~1.30 on a
hero-bright front, the RGB channels saturate and the droplet reads
as "white" rather than "bright colored". The fix is not to push
brightness higher — it's to push saturation instead. Try
`PARALLAX_SATURATION_MULT[2] = 1.20` instead of pushing brightness
past 1.30.

### 8.2 "I lowered `PARALLAX_CONTRAST_REDUCTION[0]` to 0.20 and now the back layer is too visible"

You reduced the back haze. The back layer is supposed to read as
"rain in fog" — visible enough to provide depth, but soft enough
to not draw the eye. At 0.20 contrast reduction, back droplets
read as visible streaks and compete with mid for attention. Raise
it back to 0.45–0.55.

### 8.3 "I set `PHOSPHOR_LAYER_DECAY_MULT[2]` to 0.20 for longer trails and now the rain smears"

You went below the smear threshold. Front trails at 0.20 decay
persist for ~1.5 seconds, which on a hero-bright front means each
droplet leaves a long bright streak that competes with the head pop.
The Option F value of 0.60 (670ms persistence) is the calibrated
sweet spot — longer trails need a corresponding reduction in head
brightness to avoid smearing.

### 8.4 "I disabled vignette and now the rain feels flat"

Vignette is a depth cue, not just an aesthetic. Without it, the
eye reads the screen as a flat 2D plane and the depth gradient
loses perceptual support. If you want less vignette, lower
`VIGNETTE_INTENSITY` to 0.15 rather than disabling it entirely.

### 8.5 "I raised `DROPLET_GRAVITY` to 5.0 and now I get watchdog trips"

High gravity means high velocity means more sim steps per frame
means the watchdog may trip on slower hardware. If you need fast
rain, also raise `SIM_BASE_MULTIPLIER` and check
`WATCHDOG_INTERVAL_SECS` trip frequency with `--bench-frames`.

### 8.6 "My custom tuning looks great on green but terrible on red"

Different hues have different perceptual brightness thresholds.
Green is the most perceptually efficient (peak human eye
sensitivity is ~555nm, near green). Red and blue need higher
brightness multipliers to read at the same perceptual level. If
your tuning is hue-specific, either:

- Document it as "tuned for green" and let users adjust per-hue via
  `--color-tune`.
- Or compensate by raising `PARALLAX_BRIGHTNESS_MULT[2]` by ~10%
  for red and ~20% for blue.

---

## 9. Quick reference — what to touch for common requests

| User says... | Touch this |
|--------------|-----------|
| "Make it brighter" | `--color-tune brightness=1.2` (runtime) or `PARALLAX_BRIGHTNESS_MULT[2] += 0.05` |
| "Make it more neon" | `--color-tune saturation=1.4` (runtime) or `PARALLAX_SATURATION_MULT[2] += 0.07` |
| "Heads should pop more" | `--color-tune head=1.3` (runtime) or `PARALLAX_HEAD_BLOOM_MULT[2] += 0.15` |
| "Trails are too long / smearing" | `PHOSPHOR_LAYER_DECAY_MULT[2] += 0.20` |
| "Trails are too short / chopped" | `PHOSPHOR_LAYER_DECAY_MULT[2] -= 0.10` |
| "Back layer is too present" | `PARALLAX_CONTRAST_REDUCTION[0] += 0.10` |
| "Back layer is invisible" | `PARALLAX_CONTRAST_REDUCTION[0] -= 0.10` or `PARALLAX_BRIGHTNESS_MULT[0] += 0.05` |
| "Rain is too fast" | `DROPLET_GRAVITY -= 0.5` |
| "Rain is too slow" | `DROPLET_GRAVITY += 0.5` |
| "Too few droplets" | `--density 1.3` (runtime) or `PARALLAX_DENSITY_MULT[2] += 0.10` |
| "Too many droplets / busy" | `--density 0.7` (runtime) or `PARALLAX_DENSITY_MULT[2] -= 0.10` |
| "Too many glitches" | `--glitch-level none` (runtime) or `GLITCH_THRESHOLD = 1.0` |
| "Not enough glitches" | `--glitch-level intense` (runtime) or `GLITCH_THRESHOLD -= 0.10` |
| "Disable anomalies" | `ANOMALY_CHANCE_PER_SEC = 0.0` |
| "Disable gusts" | `GUST_IDLE_MIN_SECS = 1e18` |
| "Disable color drift" | `COLOR_*_DRIFT_RATE = 0.0` |
| "Stronger CRT vibe" | `VIGNETTE_INTENSITY += 0.20`, `CRT_VIGNETTE_HEIGHT += 3` |
| "Flat, no vignette" | `VIGNETTE_INTENSITY = 0.0` |
| "Windier / more drift" | `TURBULENCE_AMPLITUDE += 0.10` |
| "Straight vertical, no drift" | `TURBULENCE_AMPLITUDE = 0.0` |

---

## 10. References

- [`docs/RAIN_DEPTH_AUDIT.md`](./RAIN_DEPTH_AUDIT.md) — the Option F
  "Film Matrix Hero" calibration audit with the 4-mechanism 10/10
  analysis.
- [`src/central_control_rains.rs`](../src/central_control_rains.rs) —
  the source file this doc documents. Every constant listed in §3
  and §4 lives here.
- [`src/configfile.rs`](../src/configfile.rs) — the runtime config
  parser. Documents the toml keys listed in §2.1.
- [`src/color_tune.rs`](../src/color_tune.rs) — the `--color-tune`
  runtime knob implementation.
- [`docs/RELEASE_CANDIDATE.md`](./RELEASE_CANDIDATE.md) — has
  `--color-tune` validation examples and edge cases.
- [`scripts/build.sh`](../scripts/build.sh) — use `--check-all` to
  verify your tuning change compiles and passes tests before
  committing.
