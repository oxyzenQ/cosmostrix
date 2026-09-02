<!-- SPDX-License-Identifier: GPL-3.0-only -->

# The Three Dragon Engines of cosmostrix v50

> v50.0.0-alpha.6 — 2026-08-19

cosmostrix runs three independent dragon engines, each owning a distinct
rendering concern. They never share mutable state; they communicate only
through the immutable `Cloud` snapshot each frame.

```
Cloud (frame state)

  COSMIC Dragon      CHROMA Dragon      CRYSTAL Dragon
  - simulation       - color            - palette
  - physics          - palette          - drift +
  - behavior         - OKLab            - ambient
```

## 1. Cosmic Dragon — `src/engine/cosmic_dragon_engine/`

The simulation core. Owns droplet lifecycle, spawn physics, atmospheric
evolution, cinematic behavior profiles, and the self-healer. Reads
palette colors produced by Chroma Dragon; never writes palette state.

Dragon Engine v2 additions: the self-healer is now predictive (EMA
trend with alpha 0.3 fires PreemptiveThrottle when pressure rises
>0.05/tick inside the warning zone — before the 30s reactive downgrade),
ghost events are pressure-scaled (ghosts as a living system health
indicator: frequent at calm, none near the perf gate), and phosphor
decay is adaptive (trails ~20% longer at idle, shorter under load —
"the rain breathes with your CPU").

## 2. Chroma Dragon — `src/engine/chroma_dragon_engine/`

The coloring engine. Owns palette construction (OKLab gradients since
v30), per-cell shader pipeline, climate post-FX (luminance/saturation/
hue drift), L-smoothing, and the 300 ms top-to-bottom wave transition.
Every color-change path (keypress, Crystal Dragon, scene runtime, live
reload) delegates to `set_color_scheme()` -> `apply_new_palette()` which
advances the circular buffer and activates the wave.

## 3. Crystal Dragon — `src/engine/crystal_dragon_engine/`

The ambient intelligence engine. Two subsystems working in harmony:

### 3a. Palette drift (CPU/CLOCK -> theme)

```
CPU% ──> point (1-99) ──> group ──> weighted theme selection
  │                          │
  │   1-33 = Cold (14)       │   calc-v2 (DEFAULT): CDF + recency
  │   34-66 = Medium (14)    │   crystal-dragon-secs polling (60s default),
  │   67-99 = Hot (14)       │   12% drift chance, 60s dwell hysteresis
  │                          │   calc-v1: legacy, no memory
  └── CPU unsupported? ──> CLOCK fallback (UTC hour -> point)
```

44 builtin themes: 14 Cold + 14 Medium + 14 Hot + 2 Reserved.
Low CPU -> Snow/Moon/Ocean (Cold). High CPU -> Sun/Fire/Red (Hot).
Transitions delegate to Chroma Dragon for smooth 300 ms OKLab waves.

### 3b. Ambient scheduler (time-of-day -> scene)

Time-of-day scene switches via `ambient.HH-MM = <scene>` in config.toml.
Fires at scheduled times, applies scene+palette. Crystal Dragon wins
(drift overrides the palette), but ambient snapback reverts after
`ambient-snapback-secs` of idle — the two systems cooperate, and since
v80.0.0-alpha.1 both timing knobs are tunable (keep snapback <
`crystal-dragon-secs` for a clean take-turns rhythm).

### File architecture

| File | Role |
|------|------|
| `crystal_dragon_control/mod.rs` | Config: polling (60s default — `crystal-dragon-secs` tunable, v80.0.0-alpha.1), calc-v2 (default) / calc-v1 (legacy), CPU/CLOCK mode |
| `sensor/mod.rs` | CPU sampling (sysinfo/procfs) + CLOCK fallback |
| `palette_groups/mod.rs` | 44 themes -> Cold/Medium/Hot partition |
| `point_system/mod.rs` | calc-v2 (default): weighted CDF + DriftHistory recency ring buffer (8 entries, prevents A->B->A oscillation); calc-v1 (legacy): no-memory CDF |
| `ambient/mod.rs` | Schedule types, parsing, validation, startup apply |
| `ambient_scheduler/mod.rs` | Background thread: fire entries on schedule |
| `ambient_diag.rs` | Diagnostics counters (exit summary) |

---

*Rezky / oxyzenQ — 2026*
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
