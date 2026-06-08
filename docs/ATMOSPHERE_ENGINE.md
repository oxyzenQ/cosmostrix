<!-- SPDX-License-Identifier: MIT -->

# Atmosphere Engine — Regime/Climate Visual Layer

The Atmosphere Engine is a visual climate layer for Cosmostrix v4.0.0+.
It models the overall visual mood of the terminal render as a slow-moving
regime that modulates rendering parameters gradually over time.

## Status: Phase 10 — Controlled Atmosphere Profile Config (v4.0.0)

v4.0.0 Phase 10 adds config-file and profile-level support for controlled atmosphere
settings. This is a "pilot safety-cover switch" — it adds the config plumbing for
atmosphere-mode and atmosphere-regime without changing default behavior, without
new public CLI flags, and without enabling visible modulation by default.

### New Config/Profile Keys

- `atmosphere-mode`: `disabled` (default) | `controlled-live`. Controls whether
  the atmosphere engine's controlled-live modulation path is activated. When disabled
  (the default), all atmosphere modulation is identity — zero visual change from v3.9.0.
- `atmosphere-regime`: `calm` (default) | `pulse` | `signal` | `compression` |
  `void` | `monolith-pressure`. Selects the visual regime for the atmosphere engine
  when mode is `controlled-live`. Storm is explicitly NOT config-safe in Phase 10
  and will be rejected with a clear error message.
- Profile keys: `profile.<name>.atmosphere-mode`, `profile.<name>.atmosphere-regime`.

### Precedence

CLI > profile > config > defaults. Since no new public CLI flags are added, the
effective precedence for atmosphere config is: profile > config > defaults (disabled/calm).

### New Functions

- `resolve_atmosphere_mode()`: converts a config string to AtmosphereApplicationMode.
  `controlled-live` → ControlledLive, anything else (including None/disabled) → Disabled.
- `resolve_atmosphere_regime()`: converts a config string to AtmosphereRegime.
  Supports all non-storm regimes. Storm is rejected at the parsing layer, never reaches
  the resolver.
- `parse_atmosphere_mode_config()` / `parse_atmosphere_regime_config()`: config-level
  validators with clean rejection warnings.
- `parse_atmosphere_mode_profile()` / `parse_atmosphere_regime_profile()`: profile-level
  validators with the same behavior.

### Diagnostics

- `-i` now reports the resolved atmosphere mode and regime from config/profile.
- Engine label updated to `phase-10-config-gated`.
- When mode is disabled (default), all atmosphere diagnostics show identity.
- When mode is controlled-live with a non-calm regime, diagnostics show the actual
  modulation and shadow metrics.

### What Phase 10 Does

- Adds `atmosphere-mode` and `atmosphere-regime` config/profile keys.
- Wires resolved config into the existing AtmosphereApplicationMode and regime
  resolution pipeline.
- Default behavior remains disabled/calm/identity — zero visual change from v3.9.0.
- ControlledLive with non-calm regime produces subtle, bounded modulation.
- Updates `--dump-config` template to document the new keys.
- Adds 25 new deterministic tests for config parsing and resolution.
- No new public CLI flags.

### What Phase 10 Does NOT Do

- Does NOT change default visual output — still identical to v3.9.0.
- Does NOT add new public CLI flags.
- Does NOT enable visible atmosphere modulation by default.
- Does NOT make storm config-safe. Storm is explicitly rejected.
- Does NOT add new dependencies or unsafe code.
- Does NOT bump version or tag.
- Does NOT alter color scheme, terminal state, or scene cycling.

### Safety Constraints

- atmosphere-mode defaults to disabled. Disabled always produces identity.
- atmosphere-regime defaults to calm. Calm always produces identity.
- Storm is not config-safe and is rejected at the parse layer.
- All values are validated before reaching the resolver.
- Invalid values produce clean stderr warnings and are ignored.
- The resolution pipeline is pure and deterministic.

## Status: Phase 9 — Internal Atmosphere Visual A/B Smoke (v4.0.0)

v4.0.0 Phase 9 adds an internal A/B smoke validation layer that compares the
baseline identity visual path against controlled whisper behavior. This is
test-only validation work — it proves that the whisper path is bounded, clean,
and safe before any public activation.

### New Types and Functions

- `AtmosphereAbSample` (`src/atmosphere_ab.rs`): captures both the baseline
  identity and a candidate whisper/shadow for a single A/B comparison. Includes
  delta fields (speed_delta_percent, density_delta_percent, brightness_delta_percent,
  trail_energy_delta_percent, glyph_pulse_delta_percent, glitch_delta), risk_label,
  and a passed boolean.
- `AtmosphereAbVerdict` (`src/atmosphere_ab.rs`): structured pass/reject outcome
  with a human-readable reason string.
- `compare_identity_vs_regime()`: builds an A/B sample for a specific regime
  under ControlledLive mode, comparing against the identity baseline.
- `compare_identity_vs_whisper()`: lower-level A/B function that takes a
  pre-built whisper and evaluates it against the identity baseline.
- `smoke_regime_under_controlled_live()`: runs A/B smoke for a single regime,
  returning both the sample and a structured verdict with specific pass/reject
  reasons.
- `smoke_all_regimes_under_controlled_live()`: batch function that runs A/B
  smoke for all seven regimes and returns a vector of results.

### What Phase 9 Does

- Adds an internal test-only A/B smoke validation layer for atmosphere whisper.
- Compares baseline identity behavior against controlled whisper behavior.
- Verifies that whisper candidates pass all safety checks (no color change, no
  terminal effect, no density collapse, no brightness spam, glitch pressure
  within whisper cap, max delta within whisper bounds).
- Proves deterministic behavior: same input always produces same A/B result.
- Adds 30 deterministic tests covering all safety checks and invariants.
- The A/B smoke module is `#[cfg(test)]` only — not compiled into production.

### What Phase 9 Does NOT Do

- Does NOT change default visual output — still identical to v3.9.0.
- Does NOT expose A/B smoke via public CLI, config, or benchmark fields.
- Does NOT enable visible atmosphere modulation in runtime.
- Does NOT add new dependencies or unsafe code.
- Does NOT alter color scheme, terminal state, or scene cycling.
- Does NOT grow src/config_apply.rs or src/bench.rs.
- Full public atmosphere controls remain future work.

### A/B Safety Checks

The A/B smoke layer verifies the following invariants for all regimes:

- No color change is allowed (color_change_allowed = false).
- No terminal effect is allowed (terminal_effect_allowed = false).
- Density does not collapse (density_scale >= 0.98).
- Brightness does not spike (brightness_scale <= 1.015).
- Glitch pressure stays at or below the whisper cap (0.05).
- Maximum absolute delta percent remains within whisper bounds (2.0%).
- Candidate risk is identity or whisper for normal regimes.
- Storm is clamped and does not exceed whisper bounds under ControlledLive.
- Calm always passes as identity.
- Default production mode remains disabled/identity.

## Status: Phase 8 — Whisper Wiring Guard / Runtime Shadow Metrics (v4.0.0)

v4.0.0 Phase 8 adds a shadow-metrics layer that measures the potential visual
impact of the atmosphere whisper system without enabling visible modulation by
default. This is measurement and guardrail work, not public visual activation.

### New Types and Functions

- `AtmosphereShadowMetrics` (`src/atmosphere_shadow.rs`): shadow metrics struct
  carrying percentage deviations from identity for each whisper parameter
  (speed_delta_percent, density_delta_percent, brightness_delta_percent,
  trail_energy_delta_percent, glyph_pulse_delta_percent), plus glitch_pressure,
  color_change_allowed, and terminal_effect_allowed. Includes `is_identity()`,
  `max_abs_delta_percent()`, and `risk_label()` methods.
- Risk labels: `identity` (no impact), `whisper` (within VisualWhisperBounds),
  `elevated` (outside whisper bounds but verifier-safe), `rejected` (color or
  terminal effect allowed).
- `shadow_metrics_from_whisper()`: converts a visual whisper into shadow metrics.
  Pure function, no side effects.
- `shadow_metrics_from_mode_and_regime()`: computes shadow metrics for a given
  mode and regime. Disabled/Calm returns identity. ControlledLive non-Calm
  routes through the whisper adapter (whisper risk). InternalVerified non-Calm
  routes through the modulation path (elevated risk).
- `shadow_metrics_from_application()`: computes shadow metrics for a given mode
  and verified application. Same routing logic as above.

### What Phase 8 Does

- Adds a shadow-metrics measurement layer for the atmosphere whisper system.
- Provides pure deterministic evaluation functions with no side effects.
- Adds risk labels (identity/whisper/elevated/rejected) for diagnostic reporting.
- Integrates shadow diagnostics into `-i` (shadow_metrics, shadow_risk).
- Integrates shadow diagnostics into `--benchmark` (atmosphere_shadow,
  atmosphere_shadow_risk).
- Default runtime shows identity shadow metrics (no visual impact).
- 18 new deterministic tests for shadow metrics.
- Shadow evaluation does not invalidate cache or alter terminal state.

### What Phase 8 Does NOT Do

- Does NOT change default visual output — still identical to v3.9.0.
- Does NOT enable visible atmosphere modulation.
- Does NOT alter benchmark field names or remove existing fields.
- Does NOT add new CLI flags, scene types, or dependencies.
- Does NOT introduce color drift, terminal effects, or random changes.
- Does NOT grow src/config_apply.rs or src/bench.rs significantly.
- Full public atmosphere controls remain future work.

### Shadow Metrics Diagnostics

- `-i` reports `shadow_metrics: identity` and `shadow_risk: identity` by default.
- `--benchmark` reports `atmosphere_shadow: identity` and
  `atmosphere_shadow_risk: identity` by default.
- Non-identity shadow would show `whisper` or `elevated` risk labels.

## Status: Phase 7 — First Real Controlled Visual Whisper (v4.0.0)

v4.0.0 Phase 7 introduces the first controlled visual modulation path that can
subtly influence visual parameters through the verified runtime seam while
preserving default v3.9.0-like behavior. The visual whisper adapter is an
internal/test-only layer that converts verified `AtmosphereRuntimeModulation`
into ultra-subtle visual-safe whisper values.

### New Types and Functions

- `AtmosphereVisualWhisper` (`src/atmosphere_visual.rs`): the visual whisper
  struct carrying ultra-tightly bounded visual modulation values (speed_scale,
  density_scale, brightness_scale, trail_energy_scale, glyph_pulse_scale,
  glitch_pressure). Default is identity.
- `VisualWhisperBounds` (`src/atmosphere_visual.rs`): ultra-tight bounds that
  are strictly tighter than ControlledLiveBounds — speed ±2%, density ±2%,
  brightness ±1.5%, trail_energy ±2%, glyph_pulse ±2%, glitch_pressure ≤ 0.05.
- `visual_whisper_from_modulation()`: converts modulation + mode into whisper.
  Disabled/Calm always returns identity.
- `visual_whisper_from_application()`: converts application + mode into whisper.
  Disabled/Calm always returns identity.
- `visual_whisper_from_regime()`: one-step pipeline from regime to whisper.
  Only useful in tests/internal code.

### What Phase 7 Does

- Introduces the visual whisper adapter as an internal/test-only module.
- Proves the atmosphere pipeline can produce visual-safe ultra-subtle values.
- Whisper bounds are tighter than ControlledLiveBounds for all parameters.
- Adds trail_energy_scale and glyph_pulse_scale as new whisper parameters.
- Default behavior remains identity (Disabled/Calm → identity whisper).
- 18 new deterministic tests for visual whisper.

### What Phase 7 Does NOT Do

- Does NOT change default visual output — still identical to v3.9.0.
- Does NOT expose visual whisper via public CLI.
- Does NOT auto-activate Pulse/Storm/Void in normal `cosmostrix`.
- Does NOT alter color scheme, terminal state, or scene cycling.
- Does NOT add new dependencies or unsafe code.
- Does NOT alter Monolith Rain behavior.
- Does NOT store non-identity whisper in CloudConfig by default.

### Visual Whisper Safety Guarantees

- Speed deviation from identity: ≤ ±2% (VisualWhisperBounds::SPEED_MAX_DELTA = 0.02).
- Density deviation: ≤ ±2% (DENSITY_MAX_DELTA = 0.02).
- Brightness deviation: ≤ ±1.5% (BRIGHTNESS_MAX_DELTA = 0.015).
- Trail energy deviation: ≤ ±2% (TRAIL_ENERGY_MAX_DELTA = 0.02).
- Glyph pulse deviation: ≤ ±2% (GLYPH_PULSE_MAX_DELTA = 0.02).
- Glitch pressure: ≤ 0.05 (GLITCH_PRESSURE_MAX = 0.05).
- Color change: always false.
- Terminal effects: always false.
- Calm regime: always identity (no modulation).
- Disabled mode: always identity (no modulation).
- Whisper is always within VisualWhisperBounds (verified by tests).
- Whisper is tighter than ControlledLive for every parameter.

### Default Behavior

The default application mode is `Disabled`. The visual whisper adapter returns
identity for Disabled mode and for Calm regime. Non-identity whisper is only
reachable through test or internal code paths. The production runtime path
computes identity. The renderer behaves exactly as v3.9.0.

## Status: Phase 6 — Controlled Live Modulation (v4.0.0)

v4.0.0 Phase 6 adds an internal-only ControlledLive modulation path while
preserving the default v3.9.0 visual identity. The system can apply very
subtle verified modulation through an extra clamping layer without exposing
any public CLI or changing default behavior.

### New Types and Functions

- `ControlledLive` variant in `AtmosphereApplicationMode` (atmosphere_apply.rs):
  internal-only mode that applies modulation with extra clamping. NOT exposed
  via public CLI. Only reachable through internal/test code paths.
- `ControlledLiveBounds` (atmosphere_apply.rs): tighter bounds than conservative —
  speed ±4%, density ±4%, brightness ±3%, glitch_pressure ≤ 0.2.
- `apply_controlled_live_modulation()`: deterministic function that clamps a
  verified application to ControlledLiveBounds. Calm applications always
  return identity.
- `controlled_live_modulation_from_regime()`: pipeline function that combines
  regime→params→application→verify→CL-clamp into a single step.
- `params_for_regime()` (atmosphere.rs, Phase 6): maps each regime to specific
  bounded rendering parameters. Calm returns identity. Non-Calm regimes
  return subtle, conservative modulation values.

### What Phase 6 Does

- Adds a ControlledLive application mode as an internal-only option.
- Defines ControlledLiveBounds — much tighter than conservative bounds.
- Wires ControlledLive through `apply_application()` with extra clamping.
- All non-Calm regimes have defined parameter mappings via `params_for_regime()`.
- Calm regime always returns identity regardless of mode.
- Default production mode remains Disabled (identity, no visual change).
- 17 new deterministic tests for ControlledLive path.

### What Phase 6 Does NOT Do

- Does NOT change default visual output — still identical to v3.9.0.
- Does NOT expose ControlledLive via public CLI.
- Does NOT add new dependencies or unsafe code.
- Does NOT alter color scheme, terminal state, or scene cycling.
- Does NOT make benchmark visual behavior different.
- Does NOT grow atmosphere_apply.rs beyond its LOC budget.

### ControlledLive Safety Guarantees

- Speed deviation from identity: ≤ ±4% (ControlledLiveBounds::SPEED_MAX_DELTA = 0.04).
- Density deviation: ≤ ±4% (DENSITY_MAX_DELTA = 0.04).
- Brightness deviation: ≤ ±3% (BRIGHTNESS_MAX_DELTA = 0.03).
- Glitch pressure: ≤ 0.2 (GLITCH_PRESSURE_MAX = 0.2).
- Color change: always false.
- Terminal effects: always false.
- Calm regime: always identity (no modulation).
- ControlledLive is always more restrictive than InternalVerified.

### Diagnostics

- `--benchmark` reports `atmosphere_application_mode: disabled` by default.
  When ControlledLive is used internally, it reports `controlled-live`.
- `effective_runtime` reflects the actual modulation applied.

## Status: Phase 5 — Runtime Atmosphere Seam (v4.0.0)

v4.0.0 Phase 5 wires the verified atmosphere application seam into runtime
parameter derivation without changing default visuals. The renderer can now
receive verified atmosphere modulation safely, while default production behavior
remains identity.

- `AtmosphereEffectiveRuntime` (`src/atmosphere_apply.rs`): derives effective
  runtime values (speed, density, brightness_scale, glitch_pressure) from base
  config values + AtmosphereRuntimeModulation. Disabled modulation returns
  exact base values.
- `derive_effective_runtime()`: pure deterministic function that computes the
  final renderer parameters. Speed is clamped to RUNTIME_SPEED_MIN..RUNTIME_SPEED_MAX
  (1.0..100.0). Density is clamped to DENSITY_CLAMP_MIN..DENSITY_CLAMP_MAX (0.01..5.0).
  Color and terminal effects are always false.
- `CloudConfig` now stores `atmosphere_modulation` and `atmosphere_mode` fields,
  both defaulting to identity/Disabled. `create_cloud()` computes effective values
  via `derive_effective_runtime()` before setting speed and density on the Cloud.

### What Phase 5 Does

- Wires the atmosphere pipeline into runtime parameter derivation.
- Proves the renderer can receive verified atmosphere modulation safely.
- Default production behavior remains identity (no visual change from v3.9.0).
- Effective runtime derivation exists but is disabled by default.
- Reports `effective_runtime: identity` in `--info` and `--benchmark` diagnostics.

### What Phase 5 Does NOT Do

- Does not change default visual output — still identical to v3.9.0.
- Does not auto-select non-Calm regime during normal runtime.
- Does not apply Storm/Pulse/Void to real runtime by default.
- Does not alter color scheme, terminal state, or scene cycling.
- Does not make benchmark visual behavior different.
- Non-Calm values are only validated/tested, not enabled by default.
- Full visible atmosphere remains a future phase.

### Default Behavior

The default application mode is `Disabled`. `derive_effective_runtime()` with
identity modulation returns exact base speed and base density values. The
renderer behaves exactly as v3.9.0. `CloudConfig::create_cloud()` now routes
speed and density through `derive_effective_runtime()`, but since the modulation
is always identity by default, the effective values are unchanged.

### Diagnostics

- `--info` (`-i`) reports an `ATMOSPHERE` section with `regime: calm`,
  `engine: phase-5-runtime-seam`, `effective: identity`,
  `verifier: pass`, `application: identity`, `application_mode: disabled`,
  `effective_runtime: identity`.
- `--benchmark` reports an `ATMOSPHERE` section with `regime: calm`,
  `effective: no-op`, `transition: stable`, `verifier: pass`,
  `application: identity`, `atmosphere_application: identity`,
  `atmosphere_application_mode: disabled`, `atmosphere_visual_effect: disabled`,
  `effective_runtime: identity`.

## Phase 3 — Verifier + Controlled Application (v4.0.0)

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
- Default application mode is `Disabled`. Disabled always returns identity.
- No visual changes are driven by atmosphere logic in production code paths.
- Color drift remains opt-in only (`auto_color_drift = false`).
- No chaotic or random visual changes.
- All regime parameters are bounded.
- All applications must pass verification.
- Color modification is always forbidden by default.
- Terminal behavior is never affected by atmosphere.
- Terminal writer remains single-owner.
- No new unsafe code.
- Scene cycling (x/X) semantics unchanged.
- Regime transitions enforce minimum dwell time (5 seconds).
- Transition ramp is bounded (minimum 1 second).
- Zactrix Cache is invalidated on `AtmosphereRegimeChange`.
- Verification does not invalidate cache (separation of concerns).
- Application adapter does not invalidate cache or alter terminal state.
- Effective runtime derivation preserves identity when modulation is Disabled.
