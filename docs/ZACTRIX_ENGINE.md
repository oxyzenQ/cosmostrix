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
