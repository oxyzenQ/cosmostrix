<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Atmosphere Engine

> **Archival notice (2026-08-05, Dragon Hunt v2 Phase 6 Tier E item 31).**
> The frozen/test-only half of the atmosphere subsystem (the A/B smoke
> harness in `atmosphere_ab.rs` and the regime probe in
> `atmosphere_probe.rs`) has been deleted. This document remains the
> canonical design spec for the **wired-in** atmosphere features
> (regime enum, controlled-live modulation, adaptive hour-driven path,
> custom time map, presets, shadow metrics, verifier, visual whisper).
> See `docs/archive/audits/ATMOSPHERE_SUBSYSTEM_ARCHIVAL.md` for the
> full archival record, including the design knowledge preserved from
> the deleted modules.

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
the adaptive engine or `--auto-color-drift`).

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

> **Important — `adaptive-custom` bypasses `atmosphere-mode = disabled`:**
> The `[adaptive-custom.HH-MM]` schedule (see [§ adaptive-custom](#adaptive-custom--user-defined-time-map)
> below) runs **regardless of `atmosphere-mode`**. Defining
> `adaptive-custom.*` entries is an explicit opt-in that takes precedence
> over the `disabled` mode. To disable ALL atmosphere behavior — both the
> built-in adaptive engine AND the user-defined schedule — remove all
> `adaptive-custom.*` keys from `config.toml` AND set
> `atmosphere-mode = disabled`. This is intentional by design (the user
> explicitly defined a schedule, so we honor it), but it is a common
> source of confusion if you expect `disabled` to be a global kill switch.

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
| `00:00–03:00` | Deep Void | cosmos | silent night, dense + slow + dark + glitchy |
| `03:00–06:00` | Compression | gray | pre-dawn pressure, extreme density |
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

## System Feeling — Signal-Driven Palette Drift

When `--auto-color-drift` is enabled, the color ecosystem consults a
**system feeling** classifier that reads two honest signals — process
CPU% and local wall-clock hour — and classifies the machine's mood
into one of 5 emotional states. Each state maps to a target color
family, and palette drift picks a random scheme from that family.

This is NOT random drift. The rain changes color because the system is
busier (Signal → RedFire family) or because night fell (Void →
PurpleNebula family). See [`docs/SYSTEM_FEELING.md`](SYSTEM_FEELING.md)
for the full design, refused-signal manifest, and owner tuning guide.

### The 5 states

| State | Trigger | Target family |
|-------|---------|---------------|
| Calm | Low CPU + daytime | BlueWater |
| Pulse | Low CPU + morning (06:00–12:00) | Green |
| Signal | High CPU (≥50%) at any hour | RedFire |
| Void | Low CPU + night (22:00–06:00) | PurpleNebula |
| Compression | Mid CPU + pre-dawn (03:00–06:00) | GrayMoon |

### Owner tuning

All taste constants (CPU thresholds, time windows, state→family
mapping) live in **`src/control_color_drift.rs`** — the single
owner-editable file. No `config.toml` keys. Edit the file and rebuild.

### Diagnostics

`--doctor` prints a `SYSTEM FEELING` section showing the current state,
target family, CPU% reading, and whether CPU sampling is supported.
Degradation (time-only fallback on unsupported platforms) is visible,
never silent.

Modules: `control_color_drift.rs`, `system_feeling.rs`,
`cloud/ecosystem.rs` (ColorFamily + family_for + family_members).

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
- **Optional key=value pairs** (ONLY these 5 fields are accepted;
  any other key is rejected with a parse error):
  - `speed` — float in `[1.0, 100.0]` (allows fractional values for
    smooth lerp transitions; see [§ Speed Type Asymmetry](#speed-type-asymmetry)
    below for why this differs from top-level `speed`).
  - `density` — float in `[0.0, 1.0]`.
  - `fps` — integer in `[1, 120]`.
  - `charset` — any built-in charset name or `[charset-custom.<name>]`.
  - `glitch-level` — one of `none`, `low`, `medium`, `high`, `ultra`.

  Top-level config keys like `scene`, `color`, `monolith-size`, `bold`,
  `shadingmode`, `color-bg`, `auto-color-drift`, `async-mode`,
  `atmosphere-mode`, `atmosphere-regime`, `intro` are NOT accepted
  inside `adaptive-custom` — those are configuration switches, not
  time-varying visual parameters. Use `[scene-custom.<name>]` blocks
  and switch via the `scene` field if you need coordinated scene changes.

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

## Speed Type Asymmetry (Intentional)

Top-level `speed` (CLI `--speed`, config `speed =`) is an **integer**
in `[1, 100]`. Adaptive-custom `speed` is a **float** in `[1.0, 100.0]`.

This asymmetry is intentional:

- Top-level `speed` is a **snap** — applied once at startup, no
  interpolation needed. Integer is simpler for users and matches the
  CLI flag's integer nature.
- Adaptive-custom `speed` is **lerped** across a 5-minute smoothstep
  blend window between time points. Fractional values are essential
  for smooth transitions (e.g., `speed=15.5` produces a perceptibly
  different blend than `speed=15` or `speed=16`).

If you copy a top-level `speed = 60` into an `adaptive-custom` entry,
it works (integer 60 is a valid float). The reverse is NOT true —
`speed = 15.5` at top-level is rejected at parse time with a clear
error.

## `async-mode` vs `atmosphere-mode` (Independent)

`async-mode` and `atmosphere-mode` are **independent** config keys.

- `atmosphere-mode = disabled` disables the regime-modulation engine
  (speed/density/brightness/glitch scaling). It does NOT disable async
  rendering.
- `async-mode = true` (or `false`) selects the renderer backend
  (async vs sync). It is a renderer implementation choice, not an
  atmosphere feature.

Setting `atmosphere-mode = disabled` does NOT force sync rendering.
If you want to force the sync renderer (e.g., for debugging), set
`async-mode = false` explicitly. The two keys compose orthogonally —
any combination is valid.

## Profile/Scene-Custom Strictness (Intentional Divergence)

Top-level config keys use **strict reject** (exit 2 on invalid value).
`[profile.<name>]` and `[scene-custom.<name>]` blocks use **warn-and-
continue** (`warn_invalid` emits a stderr warning, the invalid field
is dropped, the rest of the profile/scene-custom block is applied).

This divergence is intentional:

- Top-level config is the user's primary configuration surface — an
  invalid value there is almost certainly a mistake the user wants to
  fix before running.
- Profile/scene-custom blocks are **collections of overrides**. A user
  may have 10 profiles and only use 1 at a time. Rejecting the entire
  config because one profile has a typo would be hostile — the user
  can still run with the other 9 profiles.

If you want strict validation of profiles/scene-custom blocks, run
`cosmostrix --testconf` — it validates every block strictly and exits
2 on any invalid value, anywhere.

## Density-Map Memory Model (Intentional `Box::leak`)

`density-map` (in `[profile.<name>]` and `[scene-custom.<name>]`)
is parsed into a `Vec<f64>` and then leaked to `&'static [f64]` via
`Box::leak(vec.into_boxed_slice())`.

This is an intentional trade-off:

- The Cloud render engine consumes `&'static [f64]` for zero-cost
cell access in the hot 60 FPS render loop. Avoiding a lifetime
parameter on `Cloud` keeps the render loop simple and fast.
- The leak is bounded by the number of `density-map` entries in the
config (typically <10). Each leak is ~100 bytes. Total leak over a
session is <1KB — invisible.
- Live config reload re-parses and re-leaks. Over a 24-hour session
with frequent edits, this could accumulate to ~10KB. Still invisible.

If the Cloud ever moves off `&'static` (e.g., to `Arc<[f64]>`), this
leak can be removed. Until then, the trade-off is correct.

## Hard Constraints (v20)

These constraints are absolute and must never regress:

- Default regime is `calm`. Calm is a visual no-op.
- Default application mode is `disabled`. Disabled always returns identity.
- `visual_runtime` is always `protected`. The engine never downgrades
  this gate.
- `atmosphere-storm` is rejected at every layer (config, profile, runtime).
- Color modification is forbidden by default. Only the adaptive engine
  (when `color_change_allowed` is true for night phases) or explicit
  `--auto-color-drift` may shift colors.
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
| `control_color_drift.rs` | Owner-editable taste file for system feeling (FeelingState, thresholds, state→family) |
| `system_feeling.rs` | Signal-driven palette drift classifier (CPU + time → FeelingState) |

---

## See Also

- [Render Engine](RENDER_ENGINE.md) — diff-based rendering architecture
  (formal spec)
- [Performance Across Scales](PERFORMANCE_ACROSS_SCALES.md) — scaling
  audit from 6×6 to 400×200
- [Endurance](ENDURANCE.md) — endurance testing methodology
- [Rules](RULES.md) — project conventions and CLI flag policy
