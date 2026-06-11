<!-- SPDX-License-Identifier: MIT -->

# Controlled Atmosphere Profile Presets — v4.6.0 Phase 3

This document provides user-facing examples for the six controlled atmosphere
profile presets introduced in v4.6.0 Phase 2. Presets are **opt-in only** —
no preset is default, and the default behavior remains
`disabled / protected / identity` (identical to v4.5.0).

## Preset Registry

| Preset | Mode | Regime | Expected Shadow |
|--------|------|--------|-----------------|
| `atmosphere-calm` | disabled | calm | identity |
| `atmosphere-pulse` | controlled-live | pulse | whisper |
| `atmosphere-signal` | controlled-live | signal | whisper |
| `atmosphere-compression` | controlled-live | compression | whisper |
| `atmosphere-void` | controlled-live | void | whisper |
| `atmosphere-monolith-pressure` | controlled-live | monolith-pressure | whisper |

## Preset Constraints

These constraints are absolute and must never regress:

1. **Presets are opt-in only.** No preset is default. Users must explicitly
   select a preset through config or profile configuration.
2. **Default remains disabled/protected/identity.** Selecting no atmosphere
   preset produces the same behavior as v4.5.0 — `application_mode: disabled`,
   `visual_runtime: protected`, `runtime_application: identity`.
3. **Storm preset does not exist.** There is no `atmosphere-storm` preset,
   and adding one is forbidden. Storm remains rejected at every layer.
4. **No color change.** No preset enables `color_change_allowed`. Color
   choices remain under explicit user control (`--color`).
5. **No terminal effects.** No preset enables `terminal_effect_allowed`.
   Terminal behavior is never affected by atmosphere presets.
6. **Visual runtime remains protected.** Even under controlled-live presets,
   `visual_runtime` stays `protected`. Presets do not downgrade the visual
   safety gate.
7. **Runtime application remains identity for calm.** The `atmosphere-calm`
   preset produces `runtime_application: identity`. Non-calm presets produce
   `runtime_application: identity/whisper`.
8. **Terminal writer remains single-owner.** No preset introduces parallel
   terminal writes.
9. **Zactrix performance work remains parked for v4.8.** The `zactrix-20k-lab`
   branch is not merged during v4.6.0.

## CLI / Profile / Config Precedence

Values resolve through this chain (highest priority last):

1. Built-in clap defaults
2. Config file values
3. Config preset
4. Config scene
5. Config profile
6. CLI preset
7. CLI scene
8. CLI profile
9. Low-power values
10. Explicit CLI flags

CLI flags (step 10) always win. Profile (step 5 or 8) overrides config
(step 2). This means a profile can set `atmosphere-mode = disabled` to
override a config file that sets `atmosphere-mode = controlled-live`.

### `--color sun` Stickiness

CLI color choice is never overridden by any atmosphere preset. If a user
passes `--color sun` on the CLI alongside any atmosphere profile, `--color sun`
wins. This is consistent with the precedence chain above (step 10 above step 5
or 8).

### Auto Color Drift Remains Opt-in Only

No preset sets `auto_color_drift = true`. Auto color drift remains `false`
unless the user explicitly enables it in config or profile.

## Config File Examples

To use a preset via the config file, set the atmosphere-mode and
atmosphere-regime keys directly:

```toml
# Example: Enable pulse atmosphere via config
atmosphere-mode = controlled-live
atmosphere-regime = pulse
```

To stay at default (no visual change):

```toml
# Default behavior — no visual change from v4.5.0
# atmosphere-mode = disabled
# atmosphere-regime = calm
```

## Profile Examples

Profiles allow named configuration blocks that can be applied with
`--profile <name>`. Each preset maps to a specific profile configuration:

### atmosphere-calm (disabled, calm, identity)

```toml
[profile.atmosphere-calm]
atmosphere-mode = disabled
atmosphere-regime = calm
```

Usage: `cosmostrix --profile atmosphere-calm`

This profile produces the default v4.5.0 behavior: zero visual modulation,
identity shadow, protected visual runtime.

### atmosphere-pulse (controlled-live, pulse, whisper)

```toml
[profile.atmosphere-pulse]
atmosphere-mode = controlled-live
atmosphere-regime = pulse
```

Usage: `cosmostrix --profile atmosphere-pulse`

Periodic intensity waves with whisper-bounded modulation. Imperceptible
visual change.

### atmosphere-signal (controlled-live, signal, whisper)

```toml
[profile.atmosphere-signal]
atmosphere-mode = controlled-live
atmosphere-regime = signal
```

Usage: `cosmostrix --profile atmosphere-signal`

Focused directional convergence with whisper-bounded modulation.
Imperceptible visual change.

### atmosphere-compression (controlled-live, compression, whisper)

```toml
[profile.atmosphere-compression]
atmosphere-mode = controlled-live
atmosphere-regime = compression
```

Usage: `cosmostrix --profile atmosphere-compression`

Gradually increasing density and speed with whisper-bounded modulation.
Imperceptible visual change.

### atmosphere-void (controlled-live, void, whisper)

```toml
[profile.atmosphere-void]
atmosphere-mode = controlled-live
atmosphere-regime = void
```

Usage: `cosmostrix --profile atmosphere-void`

Minimal activity, sparse streams with whisper-bounded modulation.
Imperceptible visual change.

### atmosphere-monolith-pressure (controlled-live, monolith-pressure, whisper)

```toml
[profile.atmosphere-monolith-pressure]
atmosphere-mode = controlled-live
atmosphere-regime = monolith-pressure
```

Usage: `cosmostrix --profile atmosphere-monolith-pressure`

Enhanced monolith presence with whisper-bounded modulation.
Imperceptible visual change.

## CLI Override Examples

CLI flags always override profile and config values:

```bash
# Profile sets pulse, but CLI overrides to disabled
cosmostrix --profile atmosphere-pulse --atmosphere-mode disabled

# Profile sets pulse, but --color sun stays sticky
cosmostrix --profile atmosphere-pulse --color sun
```

## Storm Preset Does Not Exist

There is no `atmosphere-storm` preset. Storm is not config-safe and is
rejected at every parsing layer. Attempting to configure storm in any
profile or config key will produce a clean rejection warning and fall back
to safe defaults (disabled/calm/identity).

## Zactrix Performance Lab

The `zactrix-20k-lab` branch contains performance experiments for large
terminal sizes. This work is parked for v4.8.0 and must not be merged into
main during v4.6.0. Controlled atmosphere presets do not depend on, reference,
or enable any Zactrix performance features from that branch.