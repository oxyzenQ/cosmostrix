<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Zactrix Core Architecture Lab

Zactrix Core is an internal Cosmostrix architecture concept for v3.8.0. It is
inspired by the shape of eBPF systems, but it is not Linux eBPF. It does not
require root, kernel hooks, BPF bytecode, or any BPF dependency. In this codebase
it means plain stable Rust helpers and documentation that make renderer state
small, bounded, observable, and verifiable before future visual work lands.

v3.8.0 is preparation and cleanup. It is not the v4.0.0 Full Atmosphere Engine,
and it does not retune the existing Matrix, Signal, or Monolith Rain visuals.
The foundation is intended to support v3.9.0 Ultimate Subtle Monolith Rain and,
later, v4.0.0 atmosphere work without turning the renderer into a pile of
implicit side effects.

## Model

Zactrix Core uses five ideas:

- **Probe**: observe frame, stream, terminal, benchmark, and runtime state at
  defined points rather than scattering ad hoc reads across unrelated code.
- **Map**: store compact state and caches with clear bounds. No unbounded
  history is allowed in hot paths.
- **Filter**: turn observations into meaningful transitions only. Stable states
  should stay stable.
- **Verifier**: enforce invariants such as safe dirty-cell thresholds, finite
  stability classes, bounded dimensions, and explicit opt-in behavior.
- **Bounded event history**: maintain bounded history with enough signal to reason about stability,
  residue, and transitions without letting memory or visual state grow forever.

The current Rust seam is intentionally small: `src/zactrix_core.rs` holds
deterministic benchmark helpers for frame jitter, frame-time stability, and
dirty redraw threshold decisions. Those helpers are used by `src/bench.rs`, so
the architecture layer is real code, not a marketing-only skeleton.

## Safety Rules

- Zactrix Core is internal and not a public API.
- Zactrix Core itself must remain stable Rust and must not introduce unsafe
  code.
- Project policy is no new unsafe in renderer/core paths unless it is
  explicitly audited with local safety invariants. Existing platform recovery
  FFI must stay isolated and documented.
- It must not change terminal cleanup/reset semantics.
- It must not enable autonomous palette drift by default.
- Fixed CLI, config, or profile colors must remain sticky unless
  `auto-color-drift` is explicitly enabled.
- It must prefer bounded scalar logic over speculative SIMD or heavy
  dependencies.
- It must never become an excuse for unused abstractions.

## Current Invariants

- `auto-color-drift = false` remains the default.
- Explicit color choices remain stable by default.
- Normal terminal exit is non-destructive.
- `--reset-terminal` remains explicit destructive recovery.
- Runtime `x` and `X` cycle scenes forward.
- Monolith Rain avoids full-height spine residue and unbounded bottom buildup.
- Benchmark stability output keeps the existing field names and meaning.
- All Rust files remain under the 1000 LOC release gate.

## Future Use

For v3.9.0, this foundation can host subtle Monolith Rain decisions such as
bounded depth signals, stream-pressure probes, and verifier-backed transitions.
For v4.0.0, it can grow into the Full Atmosphere Engine only where existing code
actually consumes the helpers. New modules should replace duplicated logic,
document invariants, or expose deterministic tests. They should not exist merely
because the architecture has a cool name.
