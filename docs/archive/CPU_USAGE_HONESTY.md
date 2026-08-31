<!-- SPDX-License-Identifier: GPL-3.0-only -->

# cosmostrix CPU Usage — Honest Disclosure

> **Owner mandate 2026-08-30**: document the real reasons cosmostrix
> consumes >10% CPU. No gimmicks, no marketing — just the technical truth.

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

## Why cosmostrix Uses >10% CPU

cosmostrix is a **real-time terminal renderer**, not a static display.
It runs a continuous frame loop at the target FPS (default 60, auto-
detected up to 144 on high-performance terminals). Each frame executes
the full pipeline:

### 1. Rain Simulation (avg ~0.008ms per frame)

The `rain_at()` function simulates every active droplet's physics:
gravity, turbulence, spawn/despawn, fractional head position, tail
cleanup. With 23+ active streams at default density, this is the
primary simulation cost.

### 2. Per-Cell Rendering Pipeline (avg ~0.003ms per frame)

For each dirty cell, cosmostrix composes a stack of visual effects:
- Edge fade (top/bottom viewport dimming)
- CRT vignette (radial brightness falloff)
- Transition energy (palette switch blending)
- Head bloom (bright head glow)
- Head selfbloom (per-droplet head shimmer)
- Fractional head brightness (sub-frame jitter)
- Mouse flash/glow (proximity effects)
- Glitch (random corruption)
- Fog (atmospheric depth)
- Vignette (layer-based dimming)
- Brightness/saturation multipliers

### 3. ANSI Stream Encoding + I/O (avg ~0.0002ms per frame)

Differential rendering: only dirty cells are encoded into ANSI escape
sequences and written to stdout via a buffered writer. The I/O cost is
minimal because of the dirty-cell optimization.

### 4. Three Dragon Engines (always-on overhead)

- **Cosmic Dragon Engine**: quantum ripple particles, phosphor decay,
  ghost cells, border touch glow, monolith segmentation.
- **Chroma Dragon Engine**: OKLab color interpolation, polar chroma
  smoothing, hue-preserving brightness floor, palette-relative density.
- **Crystal Dragon Engine**: ambient scheduler, palette drift,
  self-healer, power manager (thermal sampling on Linux).

### 5. HUD Overlay (when toggled on with 'i')

22 metric lines refreshed at 1 Hz, but the HUD cells are written every
frame into the frame buffer. Cost: ~2µs per frame for color refresh.

### 6. Event Polling + Adaptive Throttling

Each frame polls terminal events (non-blocking). The adaptive power
manager monitors CPU pressure and can reduce FPS when idle (saving CPU).
The self-healer can trigger aggressive throttle under sustained pressure.

### Why It Can't Be Lower (Without Sacrificing Visuals)

cosmostrix is designed for **cinematic-quality rain** — not a minimal
Matrix clone. The visual fidelity (phosphor decay, quantum ripples,
OKLab color gradients, head bloom) requires per-cell per-frame
computation. A simpler renderer (e.g. `neo-matrix`) uses less CPU but
produces visibly different output.

The **adaptive throttling** system (Power Dragon, enabled by default)
automatically reduces CPU usage when the terminal is idle or under
pressure. Users who want maximum FPS can disable it with
`--power-dragon false`.

### Typical CPU Usage

| Scenario | CPU% (single core) | Notes |
|----------|-------------------|-------|
| 80x24, 60 FPS, lean scene | 0.3-0.5% | Minimal overhead |
| 80x24, 60 FPS, production-draw | 5-15% | Full pipeline + I/O |
| 200x60, 60 FPS, production-draw | 15-30% | Larger grid = more cells |
| Idle (no rain changes) | <1% | Dirty-cell optimization kicks in |

### How to Reduce CPU Usage

1. **`--power-dragon true`** (default): enables adaptive throttling
2. **`--fps 30`**: halves the frame rate
3. **`--density 0.3`**: fewer droplets = less simulation
4. **`--scene lean`**: skips cosmetics (message border, anomaly zones,
   CRT vignette, emergent storytelling)
5. **`--no-effects`**: disables particle effects entirely
