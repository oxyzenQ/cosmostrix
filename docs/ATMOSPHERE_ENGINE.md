<!-- SPDX-License-Identifier: MIT -->

# Atmosphere Engine — Future Regime/Climate Visual Layer

The Atmosphere Engine is a **future** visual layer for Cosmostrix v4.0.0+.
It is not implemented in v4.0.0 Phase 1. This document describes the design
intent and regime model so that the foundation code can reference it without
implementing it.

## Status: Phase 2 — Internal Wiring (v4.0.0)

v4.0.0 Phase 2 wires the regime model into internal runtime state:

- `AtmosphereState` holds current regime, target regime, transition progress,
  and timing markers. Default: Calm/Calm/stable.
- `AtmosphereController` manages regime transitions with dwell-time enforcement
  and bounded ramp progress.
- `RegimeProbe` provides observable facts for deterministic regime selection.
- `select_regime_from_probe()` is a pure function that maps probe facts to a
  candidate regime without applying it to visuals.
- The Zactrix Cache `AtmosphereRegimeChange` invalidation event is wired:
  regime transitions bump the cache generation.

### What Phase 2 Does NOT Do

- No regime transitions are applied in production code paths.
- The renderer produces output identical to v3.9.0.
- No visual modulation occurs.
- No color drift unless `auto_color_drift` is explicitly enabled.
- No chaotic or random visual changes.
- No new CLI flags are added.
- `select_regime_from_probe()` is used for diagnostics only.

### Default Behavior

The default regime is `Calm`. Calm is a visual no-op: all parameter
multipliers are identity (speed 1.0, density 1.0, glitch 1.0, brightness 0.0).
The renderer behaves exactly as v3.9.0.

### Diagnostics

- `--info` (`-i`) reports an `ATMOSPHERE` section with `regime: calm`,
  `engine: phase-2-internal`, `effective: no-op`.
- `--benchmark` reports an `ATMOSPHERE` section with `regime: calm`,
  `effective: no-op`, `transition: stable`.

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

### Default Behavior

In v4.0.0 Phase 1, the default regime is `Calm`. No regime transitions
occur. The renderer produces output identical to v3.9.0.

## Transition Rules

1. Regime transitions must have a minimum dwell time (e.g., 5 seconds).
2. Transition from any regime to any other regime must be gradual (ramp
   over at least 1 second).
3. `Calm` is always a valid transition target and the default safe state.
4. Regime parameters must be clamped to their defined bounds at all times.
5. `auto_color_drift` must remain `false` by default. Atmosphere-driven
   color changes are separate from palette drift and also opt-in.

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
- **Atmosphere Engine** defines regimes but does not execute rendering.

## Hard Constraints

- Default regime is `Calm`. Calm is a visual no-op.
- No visual changes are driven by atmosphere logic in Phase 2.
- Color drift remains opt-in only (`auto_color_drift = false`).
- No chaotic or random visual changes.
- All regime parameters are bounded.
- Terminal writer remains single-owner.
- No new unsafe code.
- Scene cycling (x/X) semantics unchanged.
- Regime transitions enforce minimum dwell time (5 seconds).
- Transition ramp is bounded (minimum 1 second).
- Zactrix Cache is invalidated on `AtmosphereRegimeChange`.
