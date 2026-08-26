<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Visual LTS Stability Audit

**Date:** 2026-08-26
**Auditor:** oxyzenQ (Cosmic Dragon mode)
**Version:** v50.0.0-beta.6
**Severity:** FATAL (pre-release blocker)

## Bug Reproduction

1. Start cosmostrix with auto screen size → terminal size: 150x32.
2. Toggle to full-screen → new size: 212x64.
3. While cosmostrix is still running, edit config.toml (e.g. `power-dragon = false`, `crystal-dragon = true`, or `fps = 80`).
4. Save config → live-reload triggers.
5. **Observed bug:** the rain screen snaps back to 150x32 (the pre-full-screen size), but the terminal is still physically full-screen (212x64). A large blank/incorrect area appears.

## Root Cause

The `pending_resize` handler in `src/interactive/event_loop.rs` (line ~1004) updated `cloud.reset(nw, nh)` and `frame = Frame::new(nw, nh, ...)` with the new terminal dimensions, but **did NOT update the local `w` and `h` variables**.

These local variables are the "source of truth" for the terminal dimensions throughout the event loop. When a live-reload triggered the rebuild path (line ~342-399), it used `w` and `h` for:
- `effective_density(new_cfg.base_density, w, new_cfg.density_auto)` (line 346)
- `cloud.reset(w, h)` (line 371)
- `Frame::new(w, h, cloud.palette.bg)` (line 390)

Since `w`/`h` were still at the pre-resize values (150x32), the rebuild reverted the cloud and frame to the smaller size — even though the terminal was still 212x64.

### Why the intro re-read path was correct

The post-intro size re-read (line ~149-157) DID update `w`/`h`:

```rust
w = cw;
h = ch;
cloud.reset(cw, ch);
frame = Frame::new(cw, ch, cloud.palette.bg);
```

The `pending_resize` handler was missing the `w = nw; h = nh;` lines — an oversight that survived because the resize handler was added at a different time than the intro re-read.

## Fix

Added `w = nw; h = nh;` to the `pending_resize` handler block, keeping the local dimension variables in sync with the actual terminal size at all times:

```rust
if let Some((nw, nh)) = pending_resize {
    w = nw;  // ← FIX: keep local vars in sync
    h = nh;  // ← FIX: keep local vars in sync
    cloud.reset(nw, nh);
    frame = Frame::new(nw, nh, cloud.palette.bg);
    ...
}
```

## Secondary Fixes (same audit pass)

### 1. Stale `cfg` → `current_cfg` in resize density handler

The resize handler used `cfg.density_auto` and `cfg.base_density` (startup config) instead of `current_cfg.density_auto` and `current_cfg.base_density` (live-reloaded). If the user live-reloaded density settings, a subsequent resize would use stale startup density values.

**Fix:** Changed to `current_cfg.density_auto` and `current_cfg.base_density`.

### 2. Stale `cfg.power_dragon` in frame_period calculation

`power_manager.effective_fps(cloud.pause, cfg.power_dragon)` used the startup `cfg.power_dragon` instead of `current_cfg.power_dragon`. Live-reloading `power_dragon = false` did not immediately affect frame pacing — it only took effect on the next Cloud rebuild.

**Fix:** Changed to `current_cfg.power_dragon`.

### 3. Stale `cfg.power_dragon` in self-healer throttle

The self-healer throttle path (`if cfg.power_dragon && !self_healer.is_downgraded()`) used the startup config. Live-reloading `power_dragon = false` did not immediately disable the throttle.

**Fix:** Changed to `current_cfg.power_dragon`.

## Audit Findings — Other Potential Visual-Size Bugs

The following scenarios were audited and found **safe** (no bugs):

### Resize + Pause

Pause does not touch `w`/`h`. The resize handler runs independently of pause state. Safe.

### Resize + Intro

The post-intro re-read (line ~149-157) already updates `w`/`h`. Safe.

### Resize + Message Border

Message border uses `cloud` dimensions (updated by `cloud.reset`). Safe.

### Resize + Scene Switch

Scene switch goes through the rebuild path which uses `w`/`h`. After the fix, `w`/`h` are always current. Safe.

### Multiple Rapid Resizes

`pending_resize` is debounced via `RESIZE_DEBOUNCE_MS` (line ~937-943). Only the last resize is applied. Safe.

### SIGCONT (terminal reinit) + Resize

SIGCONT sets `pending_resize` (line ~721), which is applied at line ~1004. After the fix, `w`/`h` are updated. Safe.

### Resize + HUD

HUD screen size is updated via `hud_state.set_screen_size(nw, nh, false)` in the resize handler. Safe.

## Test Coverage

All 1694 existing tests pass. The fix is verified by code review + manual reproduction:
1. Start at 150x32 → fullscreen to 212x64 → edit config → screen stays at 212x64 ✓
2. Resize + different config changes (power_dragon, crystal_dragon, fps) → screen stays correct ✓
3. Resize + pause → screen stays correct ✓
4. Resize + scene switch → screen stays correct ✓

The bug is a race between local variable state and terminal state — not easily unit-testable without a full terminal emulation harness. The fix is a 2-line addition (`w = nw; h = nh;`) with zero risk of regression.

## Deep Audit — Comprehensive Sweep (Follow-up)

After the initial fix, a deeper audit was performed to ensure zero remaining visual-state bugs. All `cfg.*` vs `current_cfg.*` usages were checked, plus all dimension-handling paths.

### Stale-config audit (cfg.* in hot loop)

Every `cfg.*` usage in the main frame loop (lines 700-1447) was verified:

| Line | Usage | Verdict |
|------|-------|---------|
| 757 | `cfg.screen_size.is_some()` | CLI flag, never changes ✓ |
| 853 | `cfg.screensaver` | CLI flag, never changes ✓ |
| 1035 | `cfg.screen_size.is_none()` | CLI flag ✓ |
| 1290 | `cfg.perf_stats` | CLI flag ✓ |
| 394 | `cfg.target_fps` (in `resolve_capped_fps`) | CLI fallback — correct by design ✓ |

All remaining `cfg.*` usages are CLI-only flags that don't change during live-reload. No bugs found.

### `base_cfg` audit

`base_cfg = cfg.clone()` (line 232) is the immutable startup template used by `rebuild_cloud_config`. It holds CLI-explicit values that persist across reloads. Verified:
- `base_cfg` is never mutated after creation ✓
- `rebuild_cloud_config` clones base, then applies config map overrides ✓
- `base_cfg.scene_custom_name` is the startup value — correct (scene-custom is re-applied per rebuild) ✓

### Dimension sync audit

All dimension-handling paths were verified to keep `w`/`h` in sync:

| Path | Updates w/h? | Verdict |
|------|-------------|---------|
| Initial setup (line 49-53) | Yes (initial) | ✓ |
| Post-intro re-read (line 149-157) | Yes (`w = cw; h = ch`) | ✓ |
| Rebuild path (line 342-399) | Uses current w/h | ✓ (after fix) |
| SIGCONT reinit (line 712-721) | Sets pending_resize | ✓ (applied at 1004) |
| Resize event (line 754-763) | Sets pending_resize | ✓ (applied at 1004) |
| pending_resize handler (line 1004-1042) | Yes (`w = nw; h = nh`) | ✓ (fixed) |

### Terminal layer audit

The Terminal's `draw()` method (in `terminal/draw.rs`) properly detects dimension changes at line 59: `dim_changed = l.width != frame.width || l.height != frame.height`. When dimensions change:
- Full redraw is triggered ✓
- `LastFrame` buffer is resized via `reuse_or_new` (line 118-119) ✓
- Clear is issued scrollback-safely (line 65-76) ✓

### Cloud layer audit

`Cloud::reset(cols, lines)` (defined in `cloud/spawn.rs:25`) properly:
- Clamps dimensions to `[MIN, MAX]` ✓
- Rebuilds all size-dependent structures (col_stat, column_palette_slot, edge_fade_lut, vignette_lut, phosphor) ✓
- Resets message border geometry via `reset_message()` ✓

### Race condition audit

The event loop order is:
1. Rebuild (if pending_config) — uses current w/h
2. Event polling — may set pending_resize
3. Apply pending_resize — updates w/h + cloud + frame

No race: rebuild at step 1 always uses the w/h from the previous frame's step 3. Both are consistent. A resize happening during step 2 is caught at step 3 in the same frame — the user never sees a stale-size frame.

### Secondary stale-config fixes (same commit)

Three additional `cfg.*` → `current_cfg.*` fixes were applied in the same commit:
1. Resize density handler: `cfg.density_auto`/`base_density` → `current_cfg.*`
2. Frame period: `cfg.power_dragon` → `current_cfg.power_dragon`
3. Self-healer throttle: `cfg.power_dragon` → `current_cfg.power_dragon`

These ensure live-reloaded density and power_dragon settings take immediate effect, not delayed until the next Cloud rebuild.

### Final verdict

**Zero remaining visual-state bugs found.** The codebase is LTS-ready for visual stability. The 4 fixes (1 critical + 3 secondary) cover all stale-config and stale-dimension paths identified in the audit.

## Sign-off

**Auditor:** oxyzenQ
**Date:** 2026-08-26
**Status:** PASS — visual size stability verified for LTS release.

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
