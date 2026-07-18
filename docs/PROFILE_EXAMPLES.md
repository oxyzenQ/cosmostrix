<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Profile Examples — v4.7.0

Concise examples for user-defined profiles and controlled atmosphere profiles.
All profiles are opt-in only. The default remains `disabled / protected / identity`.
No profile is applied unless explicitly selected with `--scene-custom <name>` or
`profile = <name>` in the config file.

## Precedence

Values resolve through this chain (highest priority last):

```
CLI flags > CLI profile > CLI scene > low-power
  > config profile > config scene > config values
  > scene defaults (fills unset keys only) > built-in defaults
```

Shorthand: **CLI > profile > config > scene defaults > built-in**.

**Key rule**: a value explicitly set in `config.toml` ALWAYS wins over a
scene's hardcoded default. Scenes only fill keys the user did NOT set.
This prevents surprises like `speed = 30` in config being silently
overwritten by a scene's `speed = 8`.

### `--color sun` override

CLI color choice always wins. `cosmostrix --scene-custom <any> --color sun` uses
`sun` regardless of what the profile sets for color.

### Auto color drift

Auto color drift remains `false` unless explicitly enabled. No profile or
preset sets `auto_color_drift = true` implicitly.

### Terminal writer

Terminal writer remains single-owner. No profile introduces parallel
terminal writes.

### Zactrix performance

The `zactrix-20k-lab` branch is parked for v4.8. Profile examples do not
depend on or enable any Zactrix performance features.

---

## 1. Minimal Profile

A profile that only changes the scene foundation:

```text
profile.minimal.base = monolith
```

Usage: `cosmostrix --scene-custom minimal`

This applies the monolith scene defaults (cosmos color, binary charset,
subtle glitch, speed 20). All other values remain at their built-in
defaults.

## 2. Color-Only Profile

Override just the color theme:

```text
profile.warm.color = sun
```

Usage: `cosmostrix --scene-custom warm`

Only the color changes to `sun`. Scene, speed, density, and all other
values remain at their defaults (monolith scene, cosmos color from scene,
then overridden to sun by the profile).

## 3. Scene + Profile Foundation

Use a scene as the foundation, then override specific fields:

```text
profile.nightcore.base = monolith
profile.nightcore.color = purple
profile.nightcore.charset = binary
profile.nightcore.speed = 24
profile.nightcore.density = 0.70
profile.nightcore.glitch-level = subtle
profile.nightcore.monolith-size = large
```

Usage: `cosmostrix --scene-custom nightcore`

The `base = monolith` applies the monolith scene defaults first, then the
profile overrides for color, charset, speed, density, glitch level, and
monolith size are applied on top. Values not set in the profile (fps, etc.)
remain at their scene or built-in defaults.

## 4. Atmosphere Pulse Profile

Enable controlled-live pulse atmosphere:

```text
profile.my-pulse.base = monolith
profile.my-pulse.atmosphere-mode = controlled-live
profile.my-pulse.atmosphere-regime = pulse
```

Usage: `cosmostrix --scene-custom my-pulse`

This produces whisper-bounded periodic intensity waves. The visual change
is imperceptible. Shadow risk is `whisper`. Terminal writer remains
single-owner.

## 5. Atmosphere Signal Profile

Enable controlled-live signal atmosphere:

```text
profile.my-signal.base = monolith
profile.my-signal.atmosphere-mode = controlled-live
profile.my-signal.atmosphere-regime = signal
```

Usage: `cosmostrix --scene-custom my-signal`

Focused directional convergence with whisper-bounded modulation.
Imperceptible visual change.

## 6. Atmosphere Void Profile

Enable controlled-live void atmosphere:

```text
profile.my-void.base = monolith
profile.my-void.atmosphere-mode = controlled-live
profile.my-void.atmosphere-regime = void
```

Usage: `cosmostrix --scene-custom my-void`

Minimal activity, sparse streams with whisper-bounded modulation.
Imperceptible visual change.

## 7. Atmosphere Monolith-Pressure Profile

Enable controlled-live monolith-pressure atmosphere:

```text
profile.my-mono.base = monolith
profile.my-mono.atmosphere-mode = controlled-live
profile.my-mono.atmosphere-regime = monolith-pressure
```

Usage: `cosmostrix --scene-custom my-mono`

Enhanced monolith presence with whisper-bounded modulation.
Imperceptible visual change.

## 8. Profile + CLI Override

Profile sets atmosphere, CLI overrides color:

```text
profile.my-pulse.base = monolith
profile.my-pulse.color = purple
profile.my-pulse.atmosphere-mode = controlled-live
profile.my-pulse.atmosphere-regime = pulse
```

Usage: `cosmostrix --scene-custom my-pulse --color sun`

Result: atmosphere is pulse (controlled-live/whisper), but color is `sun`
because CLI `--color` always wins over the profile color.

## 9. Profile + Config Precedence

Config sets global values, profile overrides them:

```text
# Global config
color = ocean
speed = 10

# Profile overrides
profile.fast.base = monolith
profile.fast.color = green
profile.fast.speed = 50
```

Usage: `cosmostrix --scene-custom fast`

Result: color is `green` (profile beats config), speed is `50`
(profile beats config). Without `--scene-custom fast`, the config values
`ocean` and `10` would apply.

## Notes

- **Storm is unavailable** and will be rejected at every layer with a clear
  message. There is no storm profile.
- All controlled atmosphere profiles are opt-in only.
  The default remains `disabled / protected / identity`.
- **Unknown profiles fail cleanly.** CLI `--scene-custom unknown` produces a
  clear error with no partial mutation. Config `profile = unknown` emits a
  warning and continues with defaults.
- **Invalid profile values fail before runtime mutation.** Each invalid
  field is skipped independently; other valid fields in the same profile
  still apply. Profile validation modifies the in-memory `Args` struct
  only and never touches the terminal writer directly.
- The terminal writer remains single-owner. No profile introduces parallel
  terminal writes.
- `zactrix-20k-lab` branch is parked for v4.8. Profile examples do not
  depend on or enable any Zactrix performance features.
- See `docs/PROFILE_ECOSYSTEM.md` for the full profile contract,
  behavior matrix, validation details, and supported fields.
- See `docs/ATMOSPHERE_PRESETS.md` for the six controlled atmosphere
  preset definitions and constraints.

## Phase History

- Phase 1: Profile Ecosystem Audit + Contract (complete)
- Phase 2: Profile Examples + Config Dump Polish (complete)
- Phase 3: Profile Validation UX + Error Message Polish (complete)
- Phase 4: Profile RC Smoke + Closure Prep (complete)