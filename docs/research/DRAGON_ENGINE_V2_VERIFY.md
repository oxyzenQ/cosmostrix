<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Dragon Engine v2 Depth-Verify — d55442d (2026-09-01)

Owner suspicion: "cosmostrix 2 dragon engine upgrade v2 was implemented
on commit d55442d4f7a93ff8ae710915d5e2acd184742b63, but owner suspects
that not real working so want to depth verify."

## 1. Verdict

**The v2 implementation is REAL, WIRED, and WORKING — but was UNPROVEN
and carried one real integration bug.** The commit's claims were true at
the code level; what was missing was proof (zero tests for the v2 logic)
plus one state-leak bug and stale doc references that made the feature
LOOK dead. This audit supplies the proof, fixes the bug, and syncs the
docs.

Per-feature verification matrix (all verified in source at HEAD, not
just in the d55442d diff):

| Engine | v2 feature | Code | Production wiring | Tests before | Tests after |
|--------|-----------|------|-------------------|--------------|-------------|
| Crystal | calc-v2 recency ring buffer (8 entries) | real | default; dispatched in `crystal_dragon_tick`; survives live-reload via `inherit_ecosystem_state` | 0 | 7 |
| Cosmic | Predictive self-healer (EMA alpha 0.3, trend > 0.05/tick) | real | `PreemptiveThrottle` handled in `event_loop_self_heal.rs` (sets `aggressive_throttle`, gated by `power_dragon`) | 0 | 6 |
| Cosmic | Ghost AI (pressure-scaled spawn) | real | `try_spawn_ghost` linear ramp in `evaluate_triggers` | 0 | 6 |
| Cosmic | Adaptive phosphor (0.8 idle -> 1.0 at 0.30 pressure) | real | composed with the 1.2x/1.5x pressure/throttle boosts in `phosphor_decay_pass` | 0 | 6 |
| Chroma | All 6 Phase 3/4 features (Bayer 4x4 dithering, column coherence LUT, head halo 0.15, subpixel jitter 3, hue_drift 0.015 rad/tick, palette-aware ghost color) | real | `rain_at.rs` DrawCtx construction passes `Some(...)` for every field in production; `ghost_base_color(&palette.colors)` called from `post_rain.rs` + `rain_at.rs` | existing activation tests | unchanged (already covered) |

## 2. Bugs found and fixed

### 2.1 self-healer reset() leaked v2 predictive state (REAL BUG)

`PerformanceSelfHealer::reset()` clears P1 bookkeeping
(`high_pressure_since`, `low_pressure_since`, `pre_degraded_scene`,
`is_downgraded`) but did NOT clear the v2 fields `pressure_ema`,
`pressure_ema_prev`, `preemptive_throttle_active`. The reset fires on
every user scene switch and every live-config rebuild
(`event_loop_self_heal.rs`, comment: "Reset on scene change BEFORE
observe() so we don't fire on the same frame the user switched").

Consequence: after a scene switch the healer kept the previous scene's
pressure EMA — a phantom trend could pre-throttle the fresh scene, and
a stale `preemptive_throttle_active` flag suppressed legitimate
re-fires. Fixed by zeroing all three fields in `reset()`, regression
test `self_healer_reset_clears_v2_predictive_state`.

### 2.2 Stale comments contradicting the v2 default

`point_system/mod.rs` documented calc-v1 as "the default, no memory"
while the actual default is `CrystalDragonCalcMethod::CalcV2`
(`crystal_dragon_control/mod.rs`). Fixed. Same class of staleness in
`docs/THREE_DRAGON_ENGINES.md` ("Config: polling 60s, calc-v1") —
fixed, plus a Cosmic v2 feature paragraph added.

## 3. The 25 new regression tests (the missing proof)

- `point_system/tests.rs` (7): DriftHistory recency factors (0.3/0.6/
  0.8/1.0), ring overwrite after 8 entries, calc-v2 group membership,
  never-reselect-current, and a 20k-sample statistical proof that the
  recency penalty suppresses a recently-selected theme's share below
  70% of its calc-v1 baseline.
- `self_healer/tests.rs` (6): steep sustained spike fires
  PreemptiveThrottle within ~3 observations; single-frame spike from
  idle is filtered by the warning-zone gate; gradual ramps (0.05/tick)
  never fire — the documented noise-filter contract; steady mid-band
  pressure converges without firing; recovery clears the flag; reset()
  clears all v2 state.
- `cloud/tests/tests_ghost_ai.rs` (6): hard gate blocks above
  EVENT_PERF_GATE; exact-gate pressure yields zero spawn (ramp
  endpoint); calm spawns; mid-pressure still spawns (ramp midpoint
  alive); paused and in-transition never spawn.
- `cloud/tests/tests_phosphor_adaptive.rs` (6): idle retains more
  energy than loaded; ramp monotonic idle > mid > loaded; skip gate
  above PHOSPHOR_SKIP_HIGH leaves energy untouched; dead cells stay
  dead; idle decay matches the exact documented formula (200 -> 66 at
  50 ms, layer-0 + bottom multiplier); the pass never mutates the
  caller-owned `last_phosphor_time`.

## 4. Verified design contracts worth recording

1. **The predictor's trigger window is instant spikes only.**
   `run_self_healer` is called every frame with no cadence gate; the
   EMA (alpha 0.3, per frame) needs pressure deltas > 0.167/tick
   (>= 10 pressure/sec) to cross the 0.05 trend threshold. A load ramp
   spread over 10+ frames mathematically cannot fire it — by design
   ("0.05 filters out noise, catches real spikes"); gradual load is the
   reactive P1 path's job (30s sustained window). The useful property:
   an instant spike (terminal resize, compile job landing) gets
   `aggressive_throttle` within 2-3 frames instead of waiting 30s.
2. **Ghost AI spawn math**: chance = GHOST_SPAWN_CHANCE_PER_TICK *
   (1 - pressure/EVENT_PERF_GATE), clamped; hard gate above the gate;
   max 1 active ghost (GHOST_MAX_ACTIVE).
3. **Adaptive phosphor envelope**: factor 0.8 at 0% pressure ramping to
   1.0 at 0.30; above 0.30 the two-tier boost (1.2x, then 1.5x with
   aggressive_throttle) takes over; above PHOSPHOR_SKIP_HIGH (0.70,
   hysteresis release 0.50) the pass skips entirely. Breathing lives
   below 0.30.

## 5. v80.0.0 masterclass scene tune

Owner directive: `crystal-dragon` speed was too slow. Raised 10 -> 30
(owner-specified value) in `src/scene/mod.rs`. The scene's comment and
docs were updated to match ("living-crystal energy, not crawl"); the
Monolith segmented structure preserves the meditative texture at the
new pace. All other 17 scenes audited against their documented design
intent (calm=meditative 6.0, orange-cat=memorial 7.0,
north-stars=sparse stargazing 5.0, storm=28.0, monolith=30.0 premium
pacing, low-power=5.0 battery saver) — every value matches its intent,
no further changes (bicycle rule: only the owner-flagged tune).

## 6. Remaining v2 potential (not implemented — documented decision)

- **Ambient scene transitions** (crystal, MEDIUM value / HIGH risk):
  smooth fade of charset+speed+density across scene switches. Not
  shipped in d55442d; per the "don't over-engineer / skip if peak"
  mandate and its HIGH complexity (touches scene_runtime, create_cloud,
  multiple subsystems), left as documented potential.
- Chroma is at peak: all 6 features verified active in production.

Source of truth: `src/**/*.rs` + the 25 tests above.

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
