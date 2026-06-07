<!-- SPDX-License-Identifier: MIT -->

# Atmosphere Engine — Regime/Climate Visual Layer

The Atmosphere Engine is a visual climate layer for Cosmostrix v4.0.0+.
It models the overall visual mood of the terminal render as a slow-moving
regime that modulates rendering parameters gradually over time.

## Status: Phase 3 — Verifier + Controlled Application (v4.0.0)

v4.0.0 Phase 3 adds a verifier layer and controlled internal application path:

- `AtmosphereVerifier` (`src/atmosphere_verifier.rs`): a pure deterministic
  verifier that ensures all atmosphere modulation parameters are bounded
  before reaching the renderer.
- `AtmosphereBounds`: defines safe ranges for speed, density, brightness, and
  glitch pressure modulation.
- `AtmosphereApplication`: the verified output struct carrying bounded
  modulation parameters (speed_scale, density_scale, brightness_scale,
  glitch_pressure, color_change).
- `build_application()`: converts current AtmosphereState into a verified
  AtmosphereApplication. For Calm, this is always identity (no-op).
- Verification operates independently of the cache: verifier rejection does
  not invalidate cache. Accepted regime transitions still use
  `AtmosphereRegimeChange` invalidation.

### What Phase 3 Does

- Adds a verifier safety gate between regime state and renderer.
- Proves that atmosphere parameters can be applied safely.
- Computes verified applications for all regimes (test-only for non-Calm).
- Reports verifier and application status in diagnostics.

### What Phase 3 Does NOT Do

- No regime transitions are applied in production code paths.
- The renderer produces output identical to v3.9.0.
- No visual modulation occurs in default runtime.
- Storm/Pulse/Void/Signal/Compression/MonolithPressure are computed and
  verified in tests but not unleashed.
- No color drift unless `auto_color_drift` is explicitly enabled.
- No chaotic or random visual changes.
- No new CLI flags are added.
- No terminal reset/cleanup behavior changes.

### Default Behavior

The default regime is `Calm`. Calm is a visual no-op: all parameter
multipliers are identity (speed 1.0, density 1.0, glitch 1.0, brightness 0.0).
The renderer behaves exactly as v3.9.0.

### Diagnostics

- `--info` (`-i`) reports an `ATMOSPHERE` section with `regime: calm`,
  `engine: phase-3-verified-internal`, `effective: no-op`,
  `verifier: pass`, `application: identity`.
- `--benchmark` reports an `ATMOSPHERE` section with `regime: calm`,
  `effective: no-op`, `transition: stable`, `verifier: pass`,
  `application: identity`.

## Phase 2 — Internal Wiring (v4.0.0)

v4.0.0 Phase 2 wired the regime model into internal runtime state:

- `AtmosphereState` holds current regime, target regime, transition progress,
  and timing markers. Default: Calm/Calm/stable.
- `AtmosphereController` manages regime transitions with dwell-time enforcement
  and bounded ramp progress.
- `RegimeProbe` provides observable facts for deterministic regime selection.
- `select_regime_from_probe()` is a pure function that maps probe facts to a
  candidate regime without applying it to visuals.
- The Zactrix Cache `AtmosphereRegimeChange` invalidation event is wired:
  regime transitions bump the cache generation.

## Concept

The Atmosphere Engine models the overall visual climate of the terminal
render. Instead of every frame being an independent rendering decision, the
Atmosphere Engine defines a slow-moving regime that modulates rendering
parameters gradually over time.

### Not Random Chaos

Atmosphere changes must be **gradual and bounded**. They must not introduce
random visual noise or chaotic flickering. Regime transitions are smooth,
with explicit ramp-up and ramp-down periods. Color drift remains opt-in
(`auto_color_drift = false` by default).

## Regime Model

The Atmosphere Engine defines a set of visual regimes:

| Regime | Description |
|--------|-------------|
| `Calm` | Default resting state. Stable, minimal modulation. Closest to v3.9.0 behavior. |
| `Compression` | Gradually increasing density and speed. Like market compression before a breakout. |
| `Pulse` | Periodic intensity waves. Regular, bounded oscillation in brightness/speed. |
| `Storm` | High activity, but bounded. Faster streams, more glitches, but not uncontrolled. |
| `Void` | Minimal activity. Sparse streams, slow speed. Visual breathing room. |
| `Signal` | Focused, directional. Streams converge toward a region or message. |
| `MonolithPressure` | Enhanced Monolith Rain presence. Deeper spines, stronger breathing. |

### Regime Properties

Each regime has bounded parameters:

- **Speed multiplier**: bounded range (e.g., 0.5x to 2.0x)
- **Density multiplier**: bounded range (e.g., 0.5x to 1.5x)
- **Glitch probability multiplier**: bounded range (e.g., 0.0x to 2.0x)
- **Brightness bias**: bounded range (e.g., -0.1 to +0.1)
- **Transition duration**: minimum time between regime changes (e.g., 5s)

### Verifier Bounds (Phase 3)

The verifier enforces conservative bounds on all applications:

- **Speed scale**: 0.5 to 2.0
- **Density scale**: 0.5 to 1.5
- **Brightness scale**: 0.9 to 1.1
- **Glitch pressure**: 0.0 to 0.5
- **Color change**: always forbidden (false)

Values outside these bounds are clamped, not rejected. Color modification
is always stripped regardless of input.

## Transition Rules

1. Regime transitions must have a minimum dwell time (e.g., 5 seconds).
2. Transition from any regime to any other regime must be gradual (ramp
   over at least 1 second).
3. `Calm` is always a valid transition target and the default safe state.
4. Regime parameters must be clamped to their defined bounds at all times.
5. `auto_color_drift` must remain `false` by default. Atmosphere-driven
   color changes are separate from palette drift and also opt-in.
6. All applications must pass verification before reaching the renderer.

## Crypto Market Analogy

The Atmosphere Engine plays the role of a **market regime model**. Just as
quantitative trading systems classify market conditions into regimes (trending,
mean-reverting, volatile, quiet) and adjust strategy parameters accordingly,
the Atmosphere Engine classifies the visual terminal into regimes and adjusts
rendering parameters.

A regime model does not execute trades (that is the Engine's job). It provides
the climate context within which execution decisions are made. Similarly, the
Atmosphere Engine does not render frames — it provides the regime context
within which the renderer makes visual decisions.

## Integration with Zactrix Architecture

- **Zactrix Core** verifies atmosphere invariants (bounded parameters).
- **Zactrix Engine** considers regime when planning execution strategy.
- **Zactrix Cache** invalidates on `AtmosphereRegimeChange` events.
- **Atmosphere Verifier** ensures all modulation parameters are bounded.
- **Atmosphere Engine** defines regimes but does not execute rendering.

## Hard Constraints

- Default regime is `Calm`. Calm is a visual no-op.
- No visual changes are driven by atmosphere logic in Phase 3.
- Color drift remains opt-in only (`auto_color_drift = false`).
- No chaotic or random visual changes.
- All regime parameters are bounded.
- All applications must pass verification.
- Color modification is always forbidden by default.
- Terminal writer remains single-owner.
- No new unsafe code.
- Scene cycling (x/X) semantics unchanged.
- Regime transitions enforce minimum dwell time (5 seconds).
- Transition ramp is bounded (minimum 1 second).
- Zactrix Cache is invalidated on `AtmosphereRegimeChange`.
- Verification does not invalidate cache (separation of concerns).
