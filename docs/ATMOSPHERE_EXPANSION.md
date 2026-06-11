<!-- SPDX-License-Identifier: MIT -->

# Controlled Atmosphere Expansion — v4.6.0 Contract

This document defines the formal contract for controlled atmosphere expansion
in Cosmostrix v4.6.0. It establishes hard constraints, allowed state
transitions, and testable invariants that must hold before any visual
expansion is considered stable.

## Purpose

v4.6.0 prepares the atmosphere system for safe, controlled expansion by:

1. Documenting the exact contract between config, mode, regime, and runtime.
2. Defining a clear matrix of allowed and rejected states.
3. Hardening config/profile/parser tests to enforce the contract.
4. Ensuring runtime/benchmark diagnostics remain honest.
5. Parking Zactrix performance work (`zactrix-20k-lab`) for v4.8.0.

This phase does NOT implement new visual effects, does NOT change default
output, and does NOT enable live atmosphere by default.

## Hard Constraints

These constraints are absolute and must never regress:

1. **Default remains disabled.** `application_mode: disabled` is the
   production default. No code path enables it without explicit opt-in.
2. **Default runtime is identity.** `effective_runtime: identity`,
   `visual_runtime: protected`, `runtime_application: identity`.
3. **Storm is rejected.** Storm regime is not config-safe, not profile-safe,
   and not reachable through any user-facing path.
4. **Unknown values are rejected.** Any mode or regime value not in the
   allowed set is silently ignored or rejected at the parse layer.
5. **Color changes remain forbidden.** `color_change_allowed` is always
   `false` in production code paths.
6. **Terminal effects remain forbidden.** `terminal_effect_allowed` is
   always `false` in production code paths.
7. **Terminal writer remains single-owner.** No parallel terminal writes
   are introduced.
8. **No threads spawned.** No `std::thread::spawn`, no thread pools,
   no async task spawning.
9. **No parallel compute.** `compute_parallelism: disabled` is the only
   allowed state in v4.6.0.
10. **No unsafe code.** All new code is safe Rust.
11. **No new dependencies.** The dependency set is frozen.
12. **All files under 1000 LOC.** Any file approaching the limit must be
    split before adding new code.
13. **No generated logs or CSV files committed.** Artifacts are gitignored.
14. **Zactrix performance work is parked.** The `zactrix-20k-lab` branch
    is not merged into main during v4.6.0.

## Allowed Modes

| Mode | Description | Config Key | Config Value |
|------|-------------|------------|--------------|
| Disabled | No modulation. All applications produce identity. | `atmosphere-mode` | `disabled` |
| Controlled-Live | Subtle whisper-bounded modulation. Opt-in only. | `atmosphere-mode` | `controlled-live` |

Modes not in this table (e.g., `live`, `aggressive`, `storm-mode`) are
rejected at the parse layer.

## Allowed Regimes

| Regime | Description | Config Key | Config Value |
|--------|-------------|------------|--------------|
| Calm | Default resting state. Visual no-op. | `atmosphere-regime` | `calm` |
| Pulse | Periodic intensity waves. Bounded oscillation. | `atmosphere-regime` | `pulse` |
| Signal | Focused, directional stream convergence. | `atmosphere-regime` | `signal` |
| Compression | Gradually increasing density and speed. | `atmosphere-regime` | `compression` |
| Void | Minimal activity. Sparse streams, slow speed. | `atmosphere-regime` | `void` |
| Monolith-Pressure | Enhanced Monolith Rain presence. | `atmosphere-regime` | `monolith-pressure` |

Regimes not in this table (including `storm`) are rejected at the parse
layer. Storm exists as an internal enum variant for testing but is never
exposed to users through config or profile parsing.

## Rejected Values

The following values are explicitly rejected and must fall back to safe
defaults (identity/calm):

- **storm** — rejected by `parse_atmosphere_regime_config()` with a clear
  diagnostic message. Falls back to calm (None regime_str resolves to Calm).
- **Unknown mode strings** — rejected by `parse_atmosphere_mode_config()`.
  The mode field remains None, resolving to Disabled.
- **Unknown regime strings** — rejected by `parse_atmosphere_regime_config()`.
  The regime field remains None, resolving to Calm.

## State Matrix

This matrix defines the expected behavior for every combination of mode
and regime:

### Disabled Mode (production default)

| Regime | Modulation | Effective Runtime | Shadow Risk | Notes |
|--------|-----------|-------------------|-------------|-------|
| calm | identity | identity | identity | Default path |
| pulse | identity | identity | identity | Mode gates all modulation off |
| signal | identity | identity | identity | Mode gates all modulation off |
| compression | identity | identity | identity | Mode gates all modulation off |
| void | identity | identity | identity | Mode gates all modulation off |
| monolith-pressure | identity | identity | identity | Mode gates all modulation off |
| storm | N/A (rejected) | identity | identity | Storm rejected at parse layer |

### Controlled-Live Mode (opt-in)

| Regime | Modulation | Effective Runtime | Shadow Risk | Notes |
|--------|-----------|-------------------|-------------|-------|
| calm | identity | identity | identity | Calm is always a no-op |
| pulse | whisper-bounded | identity/whisper | whisper | Tiny deviations, imperceptible |
| signal | whisper-bounded | identity/whisper | whisper | Tiny deviations, imperceptible |
| compression | whisper-bounded | identity/whisper | whisper | Tiny deviations, imperceptible |
| void | whisper-bounded | identity/whisper | whisper | Tiny deviations, imperceptible |
| monolith-pressure | whisper-bounded | identity/whisper | whisper | Tiny deviations, imperceptible |
| storm | N/A (rejected) | identity | identity | Storm rejected at parse layer |

## Precedence Chain

Config and profile values resolve through this precedence order:

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
override a config's `atmosphere-mode = controlled-live`.

## Diagnostic Fields

These fields must appear in `-i` and `--benchmark` output:

| Field | Expected Default | Expected Controlled-Live |
|-------|-----------------|--------------------------|
| `config_gate` | `disabled` | `armed` |
| `visual_runtime` | `protected` | `protected` |
| `runtime_application` | `identity` | `non-identity` |
| `shadow_risk` | `identity` | `whisper` |
| `compute_parallelism` | `disabled` | `disabled` |
| `terminal_writer` | `single-owner` | `single-owner` |
| `actual_execution` | `single-threaded-renderer` | `single-threaded-renderer` |

These fields must not be removed or renamed. New fields may be added
additively.

## Zactrix Performance Work (v4.8.0)

The `zactrix-20k-lab` branch contains performance experiments for large
terminal sizes. This work is explicitly parked for v4.8.0 and must NOT
be merged into main during v4.6.0. The v4.6.0 focus is exclusively on
atmosphere expansion contracts, docs, and tests.

v4.8.0 may introduce controlled parallel compute for non-terminal buffer
preparation only, gated by the runtime planner. Terminal writes remain
single-owner. Any optimization must pass the full Depth Regression Lab
before merge.

## Invariant Tests

The following test categories enforce this contract:

- Config parsing: disabled, controlled-live, invalid modes, all allowed
  regimes, storm rejection.
- Profile parsing: profile overrides config, storm rejection in profiles,
  calm identity, non-calm whisper risk.
- CLI precedence: `--color sun` remains sticky, CLI overrides profile.
- Auto color drift: remains `false` unless explicitly set to `true`.
- Terminal effect: `terminal_effect_allowed` remains `false`.
- Color change: `color_change_allowed` remains `false`.
- Parallel compute: no active parallel compute claim.
- Diagnostics: honest reporting of all fields in matrix.
- Doc guards: ATMOSPHERE_ENGINE.md and ATMOSPHERE_EXPANSION.md contain
  required contract language.

## Controlled Atmosphere Profile Presets (Phase 2)

v4.6.0 Phase 2 introduces a small registry of named atmosphere presets.
These presets are documentation and test ground truth — they define the
valid mode/regime/shadow combinations for controlled atmosphere use.

### Preset Registry

| Preset | Mode | Regime | Expected Shadow | Visual Change |
|--------|------|--------|-----------------|---------------|
| `atmosphere-calm` | disabled | calm | identity | None |
| `atmosphere-pulse` | controlled-live | pulse | whisper | Imperceptible |
| `atmosphere-signal` | controlled-live | signal | whisper | Imperceptible |
| `atmosphere-compression` | controlled-live | compression | whisper | Imperceptible |
| `atmosphere-void` | controlled-live | void | whisper | Imperceptible |
| `atmosphere-monolith-pressure` | controlled-live | monolith-pressure | whisper | Imperceptible |

### Preset Constraints

These constraints are absolute and must never regress:

1. **Presets are opt-in only.** No preset is default. Users must
   explicitly select a preset through config or profile configuration.
2. **Default remains disabled/protected/identity.** Selecting no
   atmosphere preset produces the same behavior as v4.5.0 —
   `application_mode: disabled`, `visual_runtime: protected`,
   `runtime_application: identity`.
3. **Presets only map to already-allowed regimes.** Every preset
   mode/regime combination is a subset of the allowed state matrix
   defined above. No preset introduces a new mode or regime.
4. **Storm preset does not exist.** There is no `atmosphere-storm`
   preset, and adding one is forbidden. Storm remains rejected at
   every layer (config, profile, preset).
5. **No color change allowed.** No preset enables `color_change_allowed`.
   Color choices remain under explicit user control (`--color`).
6. **No terminal effects allowed.** No preset enables
   `terminal_effect_allowed`. Terminal behavior is never affected
   by atmosphere presets.
7. **Visual runtime remains protected.** Even under controlled-live
   presets, `visual_runtime` stays `protected`. Presets do not
   downgrade the visual safety gate.
8. **Runtime application remains identity for calm.** The
   `atmosphere-calm` preset produces `runtime_application: identity`.
   Non-calm presets produce `runtime_application: identity/whisper`.
9. **Zactrix performance work remains parked for v4.8.** Presets do
   not depend on, reference, or enable any Zactrix performance
   features from the `zactrix-20k-lab` branch.
10. **Profile preset precedence remains below CLI override.** If a
    user passes `--color sun` on the CLI alongside any atmosphere
    preset, `--color sun` wins. This is consistent with the
    existing precedence chain (step 10 above).
11. **`--color sun` remains sticky with every preset.** CLI color
    choice is never overridden by any atmosphere preset.
12. **Auto color drift remains false unless explicitly enabled.**
    No preset sets `auto_color_drift = true`.