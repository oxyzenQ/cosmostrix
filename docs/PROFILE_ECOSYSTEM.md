<!-- SPDX-License-Identifier: MIT -->

# Profile Ecosystem — v4.7.0 Audit

This document audits and defines the profile ecosystem contract for
Cosmostrix. It covers user-defined profiles, controlled atmosphere presets,
config integration, precedence rules, and the invariants that must hold
across all profile-related code paths.

## Purpose

Profiles allow users to define named configuration blocks in their config
file and apply them with `--profile <name>`. Each profile can override
visual, motion, and atmosphere settings without changing the config file's
global defaults. Profiles are lightweight — they reuse existing scenes and
presets as foundations, then override only already-supported runtime fields.

This audit was produced as the first step of v4.7.0 (Renderer Ergonomics +
Profile Ecosystem) to document the contract before adding behavior-changing
features.

## Profile Syntax

Profiles are defined in the config file using dot-separated keys:

```text
profile.<name>.<field> = <value>
```

Where `<name>` must be letters, digits, `-`, or `_`, and `<field>` is one of
the supported profile fields listed below.

### Supported Fields

| Field | Type | Description |
|-------|------|-------------|
| `base` / `scene` | Scene name | Foundation scene (e.g. `monolith`, `matrix`, `signal`) |
| `preset` | Preset name | Curated preset applied before profile overrides |
| `color` | Color name | Color theme (e.g. `cosmos`, `purple`, `sun`) |
| `charset` | Charset name | Character set (e.g. `binary`, `katakana`) |
| `fps` | Number | Target frames per second (1–240) |
| `speed` | Integer | Rain speed (1–100) |
| `density` | Float | Rain density (0.01–5.0) |
| `glitch-level` | Level name | Glitch intensity: `none`, `subtle`, `default`, `intense` |
| `monolith-size` | Size name | Monolith pillar size: `small`, `normal`, `large` |
| `color-bg` | Background | Background mode: `black`, `default-background`, `transparent` |
| `atmosphere-mode` | Mode | `disabled` (default) or `controlled-live` (opt-in) |
| `atmosphere-regime` | Regime | `calm`, `pulse`, `signal`, `compression`, `void`, `monolith-pressure` |

## Built-in / Discoverable Presets

Cosmostrix ships 8 curated presets (applied via `--preset` or `preset = <name>`
in config). These are distinct from user-defined profiles. Presets provide
complete visual configurations in a single named package. See
`cosmostrix --list-presets` for the full list.

## User Profiles

User profiles are defined in the config file. Each profile is a named block
of overrides. Profiles are applied with `--profile <name>` on the CLI or
`profile = <name>` in the config file.

Example:

```text
profile.nightcore.base = monolith
profile.nightcore.color = purple
profile.nightcore.charset = binary
profile.nightcore.speed = 24
profile.nightcore.density = 0.70
profile.nightcore.glitch-level = subtle
profile.nightcore.monolith-size = large
```

Usage: `cosmostrix --profile nightcore`

User profiles and controlled atmosphere presets are separate systems. User
profiles are defined by the user in their config file. Controlled atmosphere
presets are shipped with Cosmostrix and listed in `--list-profiles` output
for discoverability. See `docs/ATMOSPHERE_PRESETS.md` for atmosphere preset
details.

## Controlled Atmosphere Profile Presets (v4.6.0)

v4.6.0 introduced six controlled atmosphere presets. These are opt-in only.
No atmosphere preset is default. The default behavior remains
`disabled / protected / identity`.

| Preset | Mode | Regime | Expected Shadow |
|--------|------|--------|-----------------|
| `atmosphere-calm` | disabled | calm | identity |
| `atmosphere-pulse` | controlled-live | pulse | whisper |
| `atmosphere-signal` | controlled-live | signal | whisper |
| `atmosphere-compression` | controlled-live | compression | whisper |
| `atmosphere-void` | controlled-live | void | whisper |
| `atmosphere-monolith-pressure` | controlled-live | monolith-pressure | whisper |

Atmosphere presets are shown in `cosmostrix --list-profiles` output under the
"CONTROLLED ATMOSPHERE PRESETS (opt-in only)" section. Storm preset does not
exist. See `docs/ATMOSPHERE_PRESETS.md` for full documentation including
config examples and profile blocks.

## Precedence Chain

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

### CLI > profile > config > defaults

CLI flags (step 10) always win. A CLI profile (step 8) overrides a config
profile (step 5). A profile (step 5) overrides config values (step 2).
Config values (step 2) override built-in defaults (step 1).

### Profile overrides config

When a config file sets both global values and a profile, the profile's
values take precedence over the global config values. For example, if the
config file sets `color = ocean` and `profile.nightcore.color = purple`,
applying `--profile nightcore` results in `color = purple`.

### CLI overrides profile

When a CLI flag is explicitly provided, it always wins over the profile
value. The `is_explicit()` check in profile application skips any field that
was set by the user on the CLI. For example, `cosmostrix --profile nightcore
--color sun` uses `sun` for color regardless of what the profile sets.

## Color Stickiness Rules

Color choices follow the same precedence chain as all other fields. Once a
color is set at a higher precedence level, it is not overridden by lower
levels. This means `--color sun` on the CLI always wins over profile or
config color settings.

Color changes through atmosphere presets are explicitly forbidden. No
controlled atmosphere preset enables `color_change_allowed`. Color choices
remain under explicit user control only.

## Auto Color Drift

Auto color drift remains `false` unless explicitly enabled by the user in
config (`auto-color-drift = true`) or profile. No preset, scene, or
atmosphere preset sets `auto_color_drift = true` implicitly. This is an
opt-in feature with no hidden activation paths.

## Profile Validation

Profiles are validated at application time, not at parse time. The
`collect_profiles` function gathers all profile key-value pairs from the
config file into an in-memory map without interpreting values. Validation
happens when `apply_profile_layer` is called with a specific profile name.

### Unknown Profiles

When an unknown profile is requested via CLI (`--profile unknown`), the
application returns a clean error before any runtime mutation occurs:

```
error: invalid profile: unknown
expected one of: <list>
```

No partial mutation — the `Args` struct remains unmodified. The error is
returned as a `Result::Err` and the application exits cleanly.

When an unknown profile is referenced in config (`profile = unknown`), the
config layer emits a warning and continues with defaults:

```
config: ignoring unknown profile 'unknown' (available: <list>; see --list-profiles)
```

This prevents a typo in the config file from breaking the entire
application.

### Invalid Profile Values

Invalid field values in a profile produce clear, actionable warnings. Each
invalid value is skipped independently — other valid fields in the same
profile still apply. Example warnings:

```
profile: invalid color='not-a-color' in profile 'myprof' (expected: see --list-colors)
profile: invalid atmosphere-regime='storm' in profile 'myprof' — storm is unavailable
profile: invalid glitch-level='extreme' in profile 'myprof' (expected: none, subtle, default, intense)
```

Profile validation does not mutate terminal state directly. Profile
application modifies the `Args` struct (in-memory configuration) only. The
terminal writer remains single-owner and is never touched by profile code.

### Unknown Profile Fields

Keys in the config file that do not match `profile.<name>.<field>` for a
known field are silently ignored. This allows forward-compatible config
files that work across multiple versions of Cosmostrix without producing
spurious warnings for newly added fields.

### Storm Unavailable

Storm is unavailable and will be rejected at every parsing layer (config,
profile, preset). There is no `atmosphere-storm` preset. Attempting to set
`atmosphere-regime = storm` in any profile or config key produces a clean
rejection warning and falls back to safe defaults (disabled/calm/identity).
Storm will not be made available without a formal contract change.

## Terminal Writer Single-Owner

The terminal writer remains single-owner. No profile, preset, or atmosphere
configuration introduces parallel terminal writes. The `terminal_writer:
single-owner` invariant is reported in both `-i` and `--benchmark` output
and must never regress.

## Profile Changes Must Not Mutate Terminal State Directly

Profile application modifies the `Args` struct (in-memory configuration) only.
Profiles never interact with the terminal directly — no cursor manipulation,
no alternate screen changes, no raw mode transitions. Terminal state is
managed exclusively by the RAII terminal guard in the main application loop.

## Profile Behavior Matrix

| # | Scenario | Expected Behavior |
|---|----------|-------------------|
| 1 | No profile | Built-in defaults apply (monolith scene, cosmos color, etc.) |
| 2 | Known user profile | Profile fields are applied; CLI can still override |
| 3 | Unknown profile (CLI) | Clean error listing available profiles; no partial mutation |
| 4 | CLI overrides profile | CLI flag wins for that specific field; other profile fields still apply |
| 5 | Profile overrides config | Profile fields override same-named config values |
| 6 | Config applies when no profile/CLI override | Config file values are the base layer above defaults |
| 7 | Profile color remains sticky unless explicit override | Profile color holds unless CLI `--color` or higher-precedence layer sets it |
| 8 | Profile atmosphere preset remains opt-in only | Atmosphere mode must be explicitly set; default is disabled |
| 9 | `--color sun` overrides profile atmosphere preset color assumptions | CLI color always wins over profile/preset color |
| 10 | Auto color drift remains false unless explicitly enabled | No implicit auto_color_drift activation |
| 11 | Storm unavailable | Storm regime rejected at parse layer with clean warning |
| 12 | Terminal effects unavailable | `terminal_effect_allowed` always false |
| 13 | Terminal writer single-owner | No parallel terminal writes from any profile |
| 14 | Compute parallelism disabled | `compute_parallelism: disabled` in all diagnostics |

## Unknown Profile Behavior

When an unknown profile is requested via CLI (`--profile unknown`), the
application returns a clean error before any runtime mutation occurs (see
[Profile Validation](#profile-validation) for details). The `Args` struct
remains unmodified before the error is returned.

When an unknown profile is referenced in config (`profile = unknown`), the
config layer emits a warning with available profiles and a pointer to
`--list-profiles`, then continues with defaults. This prevents a typo in
the config file from breaking the entire application.

## Zactrix Render Efficiency Parked for v4.8

The `zactrix-20k-lab` branch contains performance experiments for large
terminal sizes. This work is parked for v4.8.0 and must not be merged into
main during v4.7.0. Profile ecosystem work in v4.7.0 does not depend on,
reference, or enable any Zactrix performance features from that branch.

## Discoverability

- `cosmostrix --list-profiles` — lists user profiles and controlled atmosphere presets
- `cosmostrix --dump-config` — prints example config with profile syntax
- `cosmostrix --help-detail` — full CLI reference including `--profile`
- `docs/ATMOSPHERE_PRESETS.md` — controlled atmosphere preset documentation
- `docs/ATMOSPHERE_EXPANSION.md` — atmosphere contract and state matrix
- `docs/PROFILE_ECOSYSTEM.md` — this document

## Phase History

- Phase 1 (complete): Profile Ecosystem Audit + Contract
- Phase 2 (complete): Profile Examples + Config Dump Polish
- Phase 3 (complete): Profile Validation UX + Error Message Polish
- Phase 4 (complete): Profile RC Smoke + Closure Prep

v4.7 remains profile/ergonomics focused. v4.8 remains Zactrix render
efficiency / zactrix-20k-lab review.