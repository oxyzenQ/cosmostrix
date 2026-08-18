# The Three Dragon Engines of Cosmostrix v50

> v50.0.0-alpha.6 — 2026-08-19

Cosmostrix runs three independent dragon engines, each owning a distinct
rendering concern. They never share mutable state; they communicate only
through the immutable `Cloud` snapshot each frame.

```
┌──────────────────────────────────────────────────────────┐
│                    Cloud (frame state)                     │
│                                                           │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐      │
│  │  COSMIC 🔮  │  │  CHROMA 🎨  │  │ CRYSTAL ❄️  │      │
│  │  Dragon     │  │  Dragon     │  │  Dragon     │      │
│  │             │  │             │  │             │      │
│  │ simulation  │  │ color       │  │ ambient     │      │
│  │ physics     │  │ palette     │  │ drift       │      │
│  │ behavior    │  │ OKLab       │  │ intelligence│      │
│  └─────────────┘  └─────────────┘  └─────────────┘      │
└──────────────────────────────────────────────────────────┘
```

## 1. Cosmic Dragon — `src/cosmic_dragon/`
The simulation core. Owns droplet lifecycle, spawn physics, atmospheric
evolution, cinematic behavior profiles, and the self-healer. Reads
palette colors produced by Chroma Dragon; never writes palette state.

## 2. Chroma Dragon — `src/chroma/`
The coloring engine. Owns palette construction (OKLab gradients since
v30), per-cell shader pipeline, climate post-FX (luminance/saturation/
hue drift), L-smoothing, and the 300 ms top-to-bottom wave transition.
Every color-change path (keypress, Crystal Dragon, scene runtime, live
reload) delegates to `set_color_scheme()` → `apply_new_palette()` which
advances the circular buffer and activates the wave.

## 3. Crystal Dragon — `src/crystal_dragon_engine/`
The ambient intelligence engine. Maps system state → color temperature:

```
CPU% ──→ point (1-99) ──→ group ──→ weighted theme selection
  │                          │
  │   1-33 = Cold (14)       │   calc-v1: probabilistic CDF
  │   34-66 = Medium (14)    │   60s polling, 12% drift chance
  │   67-99 = Hot (14)       │   60s dwell hysteresis
  │                          │
  └── CPU unsupported? ──→ CLOCK fallback (UTC hour → point)
```

44 builtin themes: 14 Cold + 14 Medium + 14 Hot + 2 Reserved.
Low CPU → Snow/Moon/Ocean (Cold). High CPU → Sun/Fire/Red (Hot).
Transitions delegate to Chroma Dragon for smooth 300 ms OKLab waves.

### File architecture
| File | Role |
|------|------|
| `crystal_dragon_control.rs` | Config: polling 60s, calc-v1, CPU/CLOCK mode |
| `sensor.rs` | CPU sampling (sysinfo/procfs) + CLOCK fallback |
| `palette_groups.rs` | 44 themes → Cold/Medium/Hot partition |
| `point_system.rs` | calc-v1: probabilistic weighted CDF selection |
| `transition.rs` | Hook → Chroma Dragon `set_color_scheme()` |

---

*Rezky / oxyzenQ — 2026*
