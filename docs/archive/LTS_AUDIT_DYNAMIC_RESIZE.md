<!-- SPDX-License-Identifier: GPL-3.0-only -->

# LTS Audit 2026-08-20 — Dynamic Resize Screen Stability

> **Task 7/13**: Deep audit of dynamic terminal resize handling for LTS
> stability, strength, and adaptive behavior with no crash/failure.

## Audit Scope

| Component | File | Role |
|-----------|------|------|
| Event loop resize handler | `src/interactive/event_loop.rs:678-940` | Debounce, apply, post-resize state update |
| Cloud reset | `src/engine/cosmic_dragon_engine/cloud/spawn.rs:25-88` | Reallocate pools, re-seed RNG, rebuild LUTs |
| Frame resize | `src/engine/cosmic_dragon_engine/frame.rs:105-155` | Reallocate cell buffer + dirty tracking |
| Constants | `src/types/constants.rs:88-129` | Min/max terminal size, debounce interval |

## Audit Findings (No Code Changes Required)

### 1. Size Clamping (Defense-in-Depth) OK

`Cloud::reset()` clamps at TWO levels:

```rust
// spawn.rs:29-30 (Cloud::reset)
self.cols = cols.clamp(MIN_TERMINAL_COLS, MAX_TERMINAL_COLS);  // [4, 1024]
self.lines = lines.clamp(MIN_TERMINAL_LINES, MAX_TERMINAL_LINES); // [4, 500]
```

```rust
// frame.rs:128-129 (Frame::new_with_bounds)
let width = width.clamp(MIN_TERMINAL_COLS, max_cols);
let height = height.clamp(MIN_TERMINAL_LINES, max_lines);
```

A resize to 0×0 or 1×1 is clamped to 4×4 before any allocation. No panic possible.

### 2. Resize Debounce OK

```rust
// event_loop.rs:678-687
Event::Resize(nw, nh) => {
    if cfg.screen_size.is_some() { /* fixed mode: ignore */ }
    else {
        pending_resize = Some((cw, ch));
        last_resize_event = Some(Instant::now());
    }
}
```

`RESIZE_DEBOUNCE_MS = 150ms` — terminal drag-resize storms (which can fire
100+ Resize events per second) are coalesced into a single apply. The event
loop polls until 150ms of resize silence elapses, then applies once.

### 3. Zero-Size / Degenerate Protection OK

```rust
// spawn.rs:48-54
let max_line = lines.saturating_sub(2);
let max_len = max_line.max(1);  // <- prevents Uniform::new(0, 0) panic
self.rand_line = Uniform::new_inclusive(0, max_line).expect("...");
self.rand_len = Uniform::new_inclusive(1, max_len).expect("max_len >= 1 after max(1)");
self.rand_col = Uniform::new_inclusive(0, cols.saturating_sub(1)).expect("...");
```

All `Uniform::new_inclusive` calls have explicit `max(1)` or `saturating_sub`
guards. No `Uniform::new(0, 0)` panic possible even at minimum 1×1.

### 4. Fixed Mode (--screen-size) OK

When `cfg.screen_size.is_some()`, resize events are silently ignored — the
virtual terminal size stays fixed. This prevents accidental resize when
cosmostrix is running inside a tmux pane or terminal multiplexer.

### 5. Post-Resize State Update OK

```rust
// event_loop.rs:922-940
if let Some((nw, nh)) = pending_resize {
    cloud.reset(nw, nh);
    frame = Frame::new(nw, nh, cloud.palette.bg);
    if cfg.density_auto {
        cloud.set_droplet_density(effective_density(cfg.base_density, nw, true));
    }
    cloud.force_draw_everything();
    term.set_color_cache(ColorCache::new(&cloud.palette));
    last_resync_time = Instant::now();
    if cfg.screen_size.is_none() {
        hud_state.set_screen_size(nw, nh, false);
    }
}
```

All dependent state is updated:

- OK Cloud pools reallocated (droplets, monolith, col_stat, glitch_map, color_map)
- OK Frame buffer reallocated
- OK Density auto-recalculated for new width
- OK Full redraw forced (no stale cells)
- OK Color cache refreshed (H1 fix — prevents 1-frame color flicker)
- OK HUD screen size updated
- OK Resync timer reset (idle throttle)

### 6. Edge Fade LUT Rebuild OK

```rust
// spawn.rs:87-88
self.edge_fade_lut.clear();
self.edge_fade_lut.reserve(lines as usize);
```

Precomputed viewport edge fade LUT is rebuilt for the new terminal height.
No per-cell float division in the hot path.

### 7. Monolith Rain Reset OK

```rust
// spawn.rs:39
self.monolith_rain.reset(self.cols);
```

Monolith rain structure is reset with the new column count. No stale lane
state from the previous size.

### 8. Clock Jump Guard OK

```rust
// event_loop.rs:858-861
let frame_elapsed = now.saturating_duration_since(next_frame);
if frame_elapsed.as_secs_f64() > CLOCK_JUMP_GUARD_SECS {
    next_frame = now;
    break;
}
```

If the system clock jumps (NTP sync, VM suspend/resume), the event loop
detects the jump and resyncs instead of trying to catch up with a burst
of frames.

## Test Coverage

- `cargo test` (full suite): ~1500+ pass — includes resize-specific tests
  in `tests_color_stability.rs`, `tests_visual_depth.rs`, `tests_scene/`
- No flaky tests observed across 3 consecutive runs

## Conclusion

**Dynamic resize handling is LTS-stable.** No code changes required.
The resize pipeline is:

- OK Size-clamped at 2 levels (Cloud + Frame)
- OK Debounced (150ms coalescing)
- OK Zero-size protected (max(1) + saturating_sub guards)
- OK Fixed-mode aware (--screen-size ignores resize)
- OK Complete post-resize state update (all dependent subsystems)
- OK Edge fade LUT rebuilt
- OK Monolith rain reset
- OK Clock jump guarded

**Audit signoff**: Task 7 complete. No UNLOCK required for any dragon lock.
<!-- COSMOSTRIX-DISCLAIMER -->
<!--
  Documentation Disclaimer — read before relying on any data point.

  This document may contain stale data, hardcoded counts, or outdated
  file paths and symbol names. Maintainers update source code but may
  forget to sync every doc — the project ships 80+ .md files and
  perfect sync is a known maintenance burden with diminishing returns.

  Source code (`src/**/*.rs`) is the single source of truth.
  Always cross-check against the actual `.rs` files before relying on
  any specific number (test count, LOC, FPS, ms timeout), file path,
  function name, or config key.

  If you find a discrepancy, please open a PR — the doc is wrong, not
  the source.
-->
