<!-- SPDX-License-Identifier: MIT -->

# Zactrix Engine — Adaptive Execution Planner

Zactrix Engine is an internal adaptive execution planner for Cosmostrix v4.0.0.
It is not a public API. It is not a parallelization framework. It is a
deterministic planner that observes terminal dimensions, frame-time pressure,
and workload characteristics, then produces a bounded execution plan.

## Philosophy

Zactrix Engine follows the same discipline as Zactrix Core: small, bounded,
deterministic, and verifiable. It does not blindly spawn threads or enable
multithreading. It plans execution strategy based on measurable conditions.

### Terminal Writer Invariant

The terminal output writer **remains single-owner at all times**. Zactrix Engine
will never parallelize terminal writes. The `terminal_writer_single_owner` field
in every `EnginePlan` is always `true`. This is a non-negotiable architectural
invariant.

### Adaptive, Not Always-On

Zactrix Engine is adaptive execution planning, not always-on multithreading.
For normal terminal sizes (80x24 through 200x60), the planner selects
`SingleCore` mode. Parallel compute paths are only considered for very large
screens (e.g., 300+ columns or 100+ lines) or explicit benchmark workloads
where the measurable benefit justifies the complexity.

### No Speculative Parallelism

Compute and planning may become adaptive only after thresholds are met and
tests confirm correctness. The planner never assumes parallelism is beneficial.
Every mode transition must be justified by a concrete reason string.

## Engine Modes

| Mode | Description |
|------|-------------|
| `SingleCore` | Default for all normal terminal sizes. Single-threaded execution, no worker budget. |
| `Assist` | Large screen or moderate workload. Small bounded worker budget for non-terminal tasks. |
| `ParallelCompute` | Very large screen or benchmark mode. Moderate bounded worker budget for compute-heavy paths. |
| `SafeFallback` | Invalid, zero, or extreme dimensions. Always safe, minimal computation. |

## Planning Logic

The planner (`plan_execution`) takes an `EngineProbe` and produces an
`EnginePlan`. The probe contains observable facts:

- `cols`, `rows`, `cell_count` — terminal geometry
- `target_fps` — requested frame rate
- `benchmark_mode` — whether this is a benchmark run
- `active_streams` — current droplet count (if available)
- `dirty_cell_ratio` — fraction of cells needing redraw
- `frame_time_pressure` — p99 frame time in milliseconds (if available)

The planner applies deterministic thresholds:

1. **Zero or invalid dimensions** (0 cols, 0 rows) → `SafeFallback`
2. **Extremely high frame-time pressure** (> 50ms p99) → `SafeFallback`
3. **Normal screens** (cols < 200, rows < 60, non-benchmark) → `SingleCore`
4. **Large screens** (cols >= 200 or rows >= 60) → `Assist`
5. **Very large screens** (cols >= 300 and rows >= 80) or benchmark mode → `ParallelCompute`

Worker budgets are always bounded by `available_parallelism()` from
`std::thread` and a hard cap (currently 4). The budget is never zero for
active modes, and never exceeds the smaller of available parallelism and
the hard cap.

## Crypto Market Analogy

In the Zactrix architecture, the Engine plays the role of a **smart execution
router**. Just as a trading engine routes orders to the optimal execution
venue based on market conditions, latency, and liquidity, Zactrix Engine
routes rendering work to the appropriate execution strategy based on terminal
size, frame pressure, and workload characteristics.

- **Zactrix Core** = risk management and verifier (checks invariants)
- **Zactrix Engine** = smart execution router (plans strategy)
- **Zactrix Cache** = orderbook/liquidity memory (bounded reusable state)
- **Atmosphere Engine** = market regime model (climate/regime layer)
- **Renderer** = actual ordered execution to terminal (fills and prints)

## v4.0.0 Phase 1 Scope

Phase 1 implements the planner and its types. It does **not** implement actual
parallel rendering. The planner produces diagnostic output used in benchmark
reports. Actual parallel execution paths may be added in future phases only
when tests confirm correctness and benchmarks confirm benefit.

### Benchmark Diagnostic Labels (Phase 1)

In v4.0.0 Phase 1, the benchmark reports Zactrix Engine fields with
`planned_` prefixes to clearly communicate that the engine **plans only** and
does **not** execute parallel compute:

| Field | Example Value | Meaning |
|-------|--------------|---------|
| `planned_mode` | `parallel-compute` | Mode the planner recommends. Not the mode currently executing. |
| `planned_worker_budget` | `2` | Future execution budget if parallel paths were wired. Not current thread count. |
| `plan_reason` | `benchmark mode` | Why the planner chose this mode. |
| `actual_execution` | `single-threaded-renderer` | What is actually running right now: always single-threaded in Phase 1. |
| `terminal_writer` | `single-owner` | Terminal writes remain single-owner. Always true. |

**No worker threads are spawned by Zactrix Engine in Phase 1.** The
`planned_worker_budget` is a future execution budget, not a current thread
count. The renderer remains single-threaded/single-owner for terminal output.

## v4.5.0 Phase 1: Architecture Split / Boundary Definition

Starting with v4.5.0, the Zactrix Engine has been reorganized into a modular
directory structure under `src/zactrix_engine/`:

```
src/zactrix_engine/
  mod.rs        — facade with re-exports (preserves backward-compatible import paths)
  core.rs       — deterministic helpers (frame jitter, monolith depth effects)
  cache.rs      — bounded generation-aware cache policy
  scheduler.rs  — adaptive execution planner (EngineMode, EngineProbe, EnginePlan)
  system.rs     — Zactrix System diagnostic model (RuntimeMode, CpuBudget, etc.)
  render.rs     — render planning boundary types (TerminalWriterPolicy, RenderPlan)
  metrics.rs    — diagnostic labels and metric constants
```

### What Changed in v4.5.0

- Internal code was moved from flat files (`zactrix_engine.rs`, `zactrix_core.rs`,
  `zactrix_cache.rs`) into the `src/zactrix_engine/` directory with submodules.
- A facade `mod.rs` re-exports all types, so `crate::zactrix_engine::*` imports
  continue to work without modification.
- Backward-compatible wrapper modules in `main.rs` preserve `crate::zactrix_cache::*`
  and `crate::zactrix_core::*` import paths.
- New foundation modules were added: `system.rs`, `render.rs`, `metrics.rs`.
- The Zactrix System diagnostic model defines conservative defaults:
  - `runtime_mode: normal`
  - `cpu_budget: balanced`
  - `compute_parallelism: disabled`
  - `idle_policy: adaptive-sleep`

### What Did NOT Change

- No real parallel rendering was implemented.
- No worker threads are spawned.
- No visual output changed.
- No terminal behavior changed.
- No benchmark field names were renamed.
- The terminal writer remains single-owner.
- `actual_execution` remains `single-threaded-renderer`.
- `terminal_writer` remains `single-owner`.
- v4.0.1 visual behavior is fully preserved.

## v4.5.0 Phase 2: Docs Test Pressure Relief + Zactrix System Diagnostics

### What Changed

- `src/docs_tests.rs` (993 LOC, dangerously close to the 1000 LOC cap) was
  split into a module directory `src/docs_tests/` with submodules:
  `mod.rs`, `assets.rs`, `endurance.rs`, `metadata.rs`, `readme.rs`,
  `release.rs`, `safety.rs`, `zactrix.rs`. All existing guard behavior
  is preserved; test names are unchanged.
- A new **ZACTRIX SYSTEM** diagnostic section was added to both `-i` (info)
  and `--benchmark` output. This section reports policy/diagnostic values:

  ```
  ZACTRIX SYSTEM
    runtime_mode: normal
    cpu_budget: balanced
    render_plan: single-owner
    compute_parallelism: disabled
    idle_policy: adaptive-sleep
  ```

### ZACTRIX SYSTEM Is Policy/Diagnostic Only

At this stage, ZACTRIX SYSTEM is purely diagnostic. It reports conservative
defaults from `ZactrixSystemConfig` and `RenderPlan`. No real parallel
compute is active. The terminal writer remains single-owner. `actual_execution`
remains `single-threaded-renderer`. `terminal_writer` remains `single-owner`.
The existing `ZACTRIX ENGINE` benchmark section is unchanged.

## v4.5.0 Phase 3: Depth Regression Lab

Added 15 categories of deterministic regression tests that lock down the
v4.0.1/v4.5 Monolith Rain visual identity. The Depth Regression Lab is a
protective test suite that future v4.8.0 optimization work must pass before
merge. No visual behavior changed. See `docs/VISUAL_STABILITY.md` for details.

## v4.5.0 Phase 4: Monolith Test Pressure Relief

`src/cloud/tests/tests_monolith.rs` (999 LOC, dangerously close to the
1000 LOC cap) was split into a focused module directory:

```
src/cloud/tests/tests_monolith/
  mod.rs         — facade with shared helpers
  core.rs        — initialization, state, size, deterministic phase
  depth.rs       — depth lab, sparse density, brightness hierarchy
  residue.rs     — bottom residue, top clear, stale cleanup
  transitions.rs — resize reset, stream move clearing, semantic invalidation
  charset.rs     — charset transition, glyph style, presets
```

This is pressure relief only. No test behavior, test names, or visual
output changed. The Depth Regression Lab remains the gate for future v4.8
optimization. All 8 coverage categories are preserved with guard tests
verifying no category was accidentally removed.

## v4.5.0 Phase 5: Scene Test Pressure Relief

`src/cloud/tests/tests_scene.rs` (959 LOC, dangerously close to the
1000 LOC cap) was split into a focused module directory:

```
src/cloud/tests/tests_scene/
  mod.rs          — facade with shared helpers and LOC/coverage guards
  cycle.rs        — forward/backward scene cycling, roundtrips
  transitions.rs  — monolith↔glyph switches, dirty-frame behavior, semantic invalidation
  fresh_entry.rs  — upper-quarter seeding, top-biased visibility, short trails
  sparse_entry.rs — alive-count bounded, ramp start/clear, repeated stays sparse
  residue.rs      — monolith residue guards, depth lab scene switch residue
  controls.rs     — speed/density/glitch/color after scene switch, unknown scene guard
```

This is pressure relief only. No test behavior, test names, visual
output, or runtime behavior changed. Scene/depth regression coverage
remains required before v4.8 optimization. All 10 scene coverage
categories are preserved with guard tests verifying no category was
accidentally removed.

### Future Milestones

- **v4.8.0** may introduce controlled parallel compute for non-terminal buffer
  preparation, gated by the runtime planner.
- **v5.0.0** requires a proven, stable runtime planner before any default
  parallel execution is enabled.

### CPU Target Research

- Calm/idle target: < 1-3% realistic CPU usage.
- Benchmark/stress can use dynamic high CPU.
- Paused should remain near 0%.

### Roadmap

- **v4.8.0** = Zactrix Render / Efficiency Finishing. May introduce controlled
  parallel compute for non-terminal buffer preparation.
- **v5.0.0** = Zactrix Engine stable default + precision/efficiency release.
  Only when the runtime planner is real and stable.

### Allowed in Future

- Parallel compute for non-terminal buffer preparation.
- Dirty-cell planning and render batch preparation.
- Lane/stream simulation chunks in bounded worker budgets.

### Forbidden

- Multiple threads writing ANSI to the terminal.
- Multiple threads mutating terminal state directly.
- Any terminal writer ownership ambiguity.
- Real parallel rendering without single-owner guarantee.

## Hard Constraints

- Terminal writer remains single-owner.
- No new unsafe code.
- No unbounded thread pools.
- No always-on multithreading.
- Worker budget is always bounded.
- Visual identity must remain identical to v3.9.0.
- Scene cycling semantics (x/X) unchanged.
- Color stability behavior unchanged.
- `auto_color_drift` remains default `false`.
