<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Atmosphere Engine

The Atmosphere Engine is cosmostrix's visual climate layer. It models the
overall mood of the terminal render as a slow-moving regime that modulates
rendering parameters gradually over time. The engine is opt-in only: the
default remains `disabled / protected / identity` (a visual no-op), and
every atmosphere feature requires the user to explicitly enable it.

This document is the canonical v20 reference. It covers the regime model,
the controlled-live application mode, the adaptive 5-phase time-driven
modulation, the `adaptive-custom` user-defined time map, the `colors-custom`
user-defined palette system, and the invariants that must hold across all
atmosphere code paths.

---

## Regime Model

The atmosphere engine classifies the visual terminal into regimes (calm,
pulse, signal, compression, void, monolith-pressure). Each regime maps to
bounded modulation parameters:

| Regime | Speed scale | Density scale | Brightness scale | Glitch pressure |
|--------|-------------|---------------|------------------|-----------------|
| `calm` | 1.0 | 1.0 | 1.0 | 0.0 |
| `pulse` | 0.5 to 1.3 | 0.4 to 1.5 | 0.4 to 1.0 | 0.0 to 0.5 |
| `signal` | 0.5 to 1.3 | 0.4 to 1.5 | 0.4 to 1.0 | 0.0 to 0.5 |
| `compression` | 0.5 to 1.3 | 0.4 to 1.5 | 0.4 to 1.0 | 0.0 to 0.5 |
| `void` | 0.5 to 1.3 | 0.4 to 1.5 | 0.4 to 1.0 | 0.0 to 0.5 |
| `monolith-pressure` | 0.5 to 1.3 | 0.4 to 1.5 | 0.4 to 1.0 | 0.0 to 0.5 |

Values outside these bounds are clamped, not rejected. Color modification
is always stripped regardless of input (color changes are opt-in only via
the adaptive engine or `--color-drift`).

`storm` does NOT exist as a regime and must never be added. The
`atmosphere-storm` preset is rejected at every layer (config, profile,
runtime) with a clear migration message.

## Controlled Atmosphere Presets

Six opt-in presets map a friendly name to a (mode, regime) pair. None is
default; selecting none produces the same behavior as `disabled`.

| Preset | Mode | Regime | Expected Shadow |
|--------|------|--------|-----------------|
| `atmosphere-calm` | disabled | calm | identity |
| `atmosphere-pulse` | controlled-live | pulse | whisper |
| `atmosphere-signal` | controlled-live | signal | whisper |
| `atmosphere-compression` | controlled-live | compression | whisper |
| `atmosphere-void` | controlled-live | void | whisper |
| `atmosphere-monolith-pressure` | controlled-live | monolith-pressure | whisper |

`atmosphere-storm` is NOT in the registry and is rejected at parse time.

### Preset Constraints

1. Presets are opt-in only. No preset is default.
2. Default remains `disabled / protected / identity`.
3. `atmosphere-storm` does not exist and must never be added.
4. No preset enables `color_change_allowed`. Color stays under explicit
   user control (`--color` or config `color =`).
5. No preset enables `terminal_effect_allowed`. Terminal behavior is
   never affected by atmosphere presets.
6. `visual_runtime` remains `protected` with every preset — presets do
   not downgrade the visual safety gate.
7. `runtime_application` remains `identity` for calm, `whisper` for
   non-calm regimes.
8. Terminal writer remains single-owner. No preset introduces parallel
   terminal writes.

---

## Application Modes

The `atmosphere-mode` config key selects how (and whether) the regime
model is wired into the runtime:

| Mode | Behavior |
|------|----------|
| `disabled` (default) | Atmosphere engine is a no-op. Effective runtime modulation is identity. |
| `controlled-live` | Regime modulation is applied to the runtime with whisper-bounded safety. Color and terminal effects remain forbidden. |

`controlled-live` is the only opt-in mode. It applies the regime's
speed/density/brightness/glitch scales to the renderer, but never
touches color or terminal behavior.

### Diagnostics Honesty Fields

The renderer reports these fields in `--doctor` and the benchmark
report so users can verify what actually happened at runtime:

- `config_gate: disabled|armed`
- `visual_runtime: protected|active`
- `runtime_application: identity|non-identity`
- `shadow_risk: identity|whisper|elevated|rejected`
- `compute_parallelism: disabled`
- `terminal_writer: single-owner`
- `actual_execution: single-threaded-renderer`

`visual_runtime` is always `protected`. The engine never downgrades
this gate — even under controlled-live presets, visual changes are
whisper-bounded and never reach the "active" state that would imply
uncontrolled visual mutation.

---

## Adaptive Atmosphere Engine (Default Time-Driven Modulation)

When `atmosphere-mode = controlled-live` is set without a custom time
map, the engine uses the built-in 5-phase adaptive schedule. The 24-hour
day is divided into five emotional phases that transition smoothly via
smoothstep interpolation so the rain breathes rather than jumps.

| Time window | Phase | Color palette | Mood |
|-------------|-------|---------------|------|
| `00:00–03:00` | Deep Void | deepspace | silent night, dense + slow + dark + glitchy |
| `03:00–06:00` | Compression | blackhole | pre-dawn pressure, extreme density |
| `06:00–12:00` | Pulse | aurora | morning energy, sparse + fast + bright |
| `12:00–18:00` | Calm | cosmos | stable afternoon, balanced |
| `18:00–24:00` | Signal | neon | dusk to night, rising glitch |

The engine checks the local wall-clock every 30 seconds and applies
palette transitions via a smooth 5-minute blend window so the atmosphere
evolves imperceptibly across a long-running session.

Each phase returns bounded `AdaptiveParams`:

- Speed multiplier: 0.5 to 1.3
- Density multiplier: 0.4 to 1.5
- Brightness multiplier: 0.4 to 1.0
- Glitch pressure: 0.0 to 0.5
- `color_change_allowed`: only true for night phases
- `terminal_effect_allowed`: only true for night phases
- `target_color`: suggested palette name (None = keep current)

Modules: `atmosphere_adaptive.rs`, `atmosphere_apply.rs`.

---

## `adaptive-custom` — User-Defined Time Map

Users can override the default 5-phase schedule with their own 24-hour
time-to-parameter mapping via `[adaptive-custom.HH-MM]` config entries.

### Format

```toml
# Format: adaptive-custom.HH-MM = <color>, <scene>, [key=value, ...]
adaptive-custom.00-00 = green3, matrix, speed=60
adaptive-custom.02-10 = cosmos, monolith, density=1.2
adaptive-custom.06-00 = aurora, signal, speed=10, density=0.5
adaptive-custom.22-00 = sunset, monolith, speed=10
```

### Fields

- **HH-MM**: time in 24h format (00-00 to 23-59).
- **First value**: color scheme name (52 built-in themes, or any
  `colors-custom` palette name).
- **Second value**: scene name (11 built-in scenes: matrix, monolith,
  signal, classic, cinematic, calm, storm, cosmos, neon, hacker,
  low-power).
- **Optional key=value pairs**: `speed`, `density`, `fps`, `charset`,
  `glitch-level`.

### Behavior

- **Sticky parameters**: keys not specified in a time point keep the
  previous value (the engine does not reset unspecified fields to
  defaults when transitioning between time points).
- **Smooth transitions**: a 5-minute smoothstep blend window runs before
  each scheduled time point so the atmosphere evolves imperceptibly.
- **Live config reload**: editing the config file triggers an immediate
  re-parse of the `adaptive-custom` map. The new schedule takes effect
  on the next 30-second tick.
- **Custom color palettes**: time points may reference any
  `[colors-custom.<name>]` block defined in the same config file. See
  the next section.
- **Fallback**: if no `[adaptive-custom.*]` entries are defined, the
  default 5-phase adaptive schedule runs.

Modules: `atmosphere_custom.rs`, `atmosphere_apply.rs`.

---

## `colors-custom` — User-Defined Color Palettes

Users can define their own color palettes in config and reference them
by name, either via the `--colors-custom <name>` CLI flag or from
`adaptive-custom` time points.

### Format

```toml
[colors-custom.sunset]
bg = "#0a0a12"
rain = "#1a0033", "#4d0080", "#9933ff", "#cc66ff", "#ffffff"
```

### Fields

- **`bg`** (optional): solid background color as `#RRGGBB`. When
  omitted, the terminal emulator's background is used (same as
  `color-bg = default-background`).
- **`rain`** (required): gradient stops for the rain trail, listed in
  tail-to-head order. Minimum 2 colors. Each color is `#RRGGBB` hex.

### Usage

```bash
# Load a custom palette by name
cosmostrix --colors-custom sunset

# Reference the same palette from an adaptive-custom time point
# (in config.toml)
adaptive-custom.22-00 = sunset, monolith, speed=10
```

### Validation

- `rain` must contain at least 2 hex colors (a gradient needs endpoints).
- Hex colors must be exactly 6 hex digits, with or without a leading `#`.
- Unknown palette names referenced from `adaptive-custom` produce a
  config-validation error (exit 2). No silent fallback.
- Custom palettes are listed alongside built-in themes in
  `--list-colors` output.

Modules: `colors_custom.rs`, `palette.rs`.

---

## Live Config Reload

The `notify` crate watches `config.toml` for changes (Linux inotify,
macOS FSEvents). On any write, the engine:

1. Re-reads and strictly validates the entire config file.
2. Re-parses `[adaptive-custom.*]` entries into a fresh `CustomTimeMap`.
3. Re-parses `[colors-custom.*]` entries into fresh `CustomPaletteDef`s.
4. Rebuilds the Cloud with the new configuration.
5. Logs errors to stderr AFTER terminal restore (never during rain).

Strict validation: malformed lines, unknown keys, invalid values, and
unparsable hex colors all trigger exit code 2. No silent fallback.

Modules: `live_config.rs`, `testconf.rs` (shared validation).

---

## Config Validation

`--testconf` validates all keys and values strictly:

- Startup: rejects invalid config (exit 2, same as `--testconf`).
- Live reload: rejects invalid config (exit 2, error printed after exit).
- Malformed lines (no `=` or empty key/value) → error.
- Unknown keys → error.
- Invalid values (out of range, unknown enum) → error.
- Unparsable hex colors in `colors-custom` → error.
- Unknown palette/scene names in `adaptive-custom` → error.
- No silent fallback. No warnings. Errors only.

---

## Hard Constraints (v20)

These constraints are absolute and must never regress:

- Default regime is `calm`. Calm is a visual no-op.
- Default application mode is `disabled`. Disabled always returns identity.
- `visual_runtime` is always `protected`. The engine never downgrades
  this gate.
- `atmosphere-storm` is rejected at every layer (config, profile, runtime).
- Color modification is forbidden by default. Only the adaptive engine
  (when `color_change_allowed` is true for night phases) or explicit
  `--color-drift` may shift colors.
- Terminal behavior is never affected by atmosphere logic.
- Terminal writer remains single-owner.
- No new unsafe code in the atmosphere path.
- Scene cycling (`x`/`X`) semantics unchanged.
- Regime transitions enforce minimum dwell time (5 seconds).
- Transition ramp is bounded (minimum 1 second, smoothstep blend).
- Verification does not invalidate cache (separation of concerns).
- The application adapter does not invalidate cache or alter terminal
  state.
- Effective runtime derivation preserves identity when modulation is
  `disabled`.

---

## Module Map

| Module | Responsibility |
|--------|----------------|
| `atmosphere_adaptive.rs` | Default 5-phase time-driven modulation |
| `atmosphere_custom.rs` | `adaptive-custom` user-defined time map parsing |
| `atmosphere_apply.rs` | Apply modulation to runtime (whisper-bounded) |
| `atmosphere_presets.rs` | Controlled atmosphere preset registry |
| `atmosphere_verifier.rs` | Verify modulation parameters are bounded |
| `atmosphere_runtime.rs` | Runtime modulation state |
| `atmosphere_probe.rs` | Atmosphere probe diagnostics |
| `colors_custom.rs` | `colors-custom` palette parsing and validation |
| `palette.rs` | Palette construction from gradient stops |
| `live_config.rs` | Live config reload via `notify` crate |
| `testconf.rs` | Shared strict validation for `--testconf` and live reload |

---

## See Also

- [Render Engine](RENDER_ENGINE.md) — diff-based rendering architecture
  (formal spec)
- [Performance Across Scales](PERFORMANCE_ACROSS_SCALES.md) — scaling
  audit from 6×6 to 400×200
- [Endurance](ENDURANCE.md) — endurance testing methodology
- [Rules](RULES.md) — project conventions and CLI flag policy
