<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Zactrix Cache — Bounded Generation-Aware Cache Policy

Zactrix Cache is an internal cache policy module for Cosmostrix v4.0.0.
It provides deterministic, bounded, generation-aware caching discipline.
It is **not** a public API. It does not cache terminal output strings.
It is a policy framework that future rendering paths may use to avoid
redundant computation while guaranteeing freshness.

## Philosophy

Like Zactrix Core and Zactrix Engine, Zactrix Cache follows the discipline
of being small, bounded, deterministic, and verifiable. It exists to ensure
that any future caching behavior in the renderer follows explicit rules
rather than ad hoc growth.

### Bounded

Cache entries have a hard maximum count. The cache never grows unbounded.
When the entry count reaches `max_entries`, the oldest or least-recently-used
entries are evicted before new ones are admitted. This prevents memory usage
from growing with terminal uptime.

### Generation-Aware

Every cache entry is tagged with a generation identifier. When the renderer
state changes in ways that would invalidate cached results (resize, color
change, scene switch, etc.), the generation counter is incremented. Any
cached data from a previous generation is immediately considered stale
and is not reused.

### Deterministic

Cache invalidation is triggered by explicit events, not by heuristics or
time-based expiration. The same sequence of events always produces the same
cache state. This makes the cache behavior testable and predictable.

## Invalidation Events

The cache is invalidated (generation bumped) on the following events:

| Event | Trigger |
|-------|---------|
| `Resize` | Terminal window dimensions change |
| `ColorChange` | Color scheme or color mode changes |
| `CharsetChange` | Character set or preset changes |
| `SceneSwitch` | Active scene changes (x/X cycle) |
| `ProfileApply` | User profile is applied |
| `TerminalModeChange` | Terminal mode changes (e.g., mouse enable) |
| `AtmosphereRegimeChange` | Atmosphere regime transitions (future) |

Each event has a clear, deterministic trigger. There is no probabilistic
or heuristic invalidation.

## Cache Policy

The `CachePolicy` struct defines the cache configuration:

- `max_entries`: Hard upper bound on cache size (e.g., 256 or 512).
- `generation`: Current generation identifier. Starts at 0, incremented on
  each invalidation event.
- `entry_count`: Current number of entries in the cache.

The `is_generation_current(generation)` method returns whether a cached
entry with the given generation is still valid for the current policy.

The `should_admit(entry_count, max_entries)` method returns whether a new
entry can be added without exceeding the bounds.

## Integration Strategy

In v4.0.0 Phase 2, Zactrix Cache is wired into the Atmosphere Engine:
when `AtmosphereController::transition_to()` accepts a regime change,
it invalidates the cache with `InvalidationEvent::AtmosphereRegimeChange`,
bumping the generation and resetting entry count. This seam is tested
through unit tests in `atmosphere.rs`.

In Phase 1, Zactrix Cache was a policy module tested through unit tests.
In Phase 2, the `AtmosphereRegimeChange` event is used in real code paths
(controller transitions), but the cache is not yet wired into the hot
rendering path for frame-level caching.

## What Zactrix Cache Does NOT Do

- It does not cache terminal output strings.
- It does not cache frame buffers.
- It does not perform background eviction or periodic cleanup.
- It does not use weak references or lazy invalidation.
- It does not grow unbounded.
- It does not introduce any new unsafe code.

## Crypto Market Analogy

Zactrix Cache plays the role of an **orderbook/liquidity memory** in the
Zactrix architecture. Just as a trading system maintains a bounded, fresh
view of market liquidity that is invalidated by market events (trades,
cancellations, regime changes), Zactrix Cache maintains a bounded, fresh
view of reusable rendering state that is invalidated by terminal events
(resize, color change, scene switch).

Stale orderbook data leads to bad fills. Stale cache data leads to
incorrect rendering. Both must be generation-aware and bounded.

## Hard Constraints

- Max entries is a hard cap, never exceeded.
- Generation invalidation is explicit and deterministic.
- No unbounded growth.
- No caching of terminal output strings.
- No new unsafe code.
- Visual identity must remain identical to v3.9.0.
