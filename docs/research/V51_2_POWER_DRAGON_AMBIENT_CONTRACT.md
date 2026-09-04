<!-- SPDX-License-Identifier: GPL-3.0-only -->

# v51.2 Power-Dragon Density Gate + Ambient Overlay Lift — Internal Research Audit

> Task: owner special directive (2026-09-01), two parts. (1) Owner-approved
> continuation of the v51.1 contract audit: "run the same contract audit on
> the ambient snapback path next ... until no remaining more". (2) Owner
> bug report on power-dragon adaptive density: "when existing scene have
> a config density 0.85 when power dragon on, on runtime the density on
> HUD metrics showing range ~0.47 should not extreme throttle" — with the
> masterclass banding mandate (ceiling = the configured density; power
> off = fixed; low 0.84-0.70 / medium 0.70-0.50 / high, rare, 0.50-0.10).
> Scope: masterclass audit to peak — precision, harmony, stability/LTS;
> no over-engineering; all docs/reference updated so nothing stays stale.

## 1. Part A — power-dragon adaptive density (the premature logic)

Two defects, both in the power/perf stack:

**Defect A1 — power-dragon OFF did not actually disable the density
throttle.** The `--power-dragon false` help text and config doc promise
"rain stays at user-configured density/speed regardless of CPU
pressure", and v50.0.0-beta.6 Option D claimed to deliver that — but it
gated only the HUD display: `event_loop_hud.rs` fed
`cloud.set_perf_pressure(power_manager.effective_pressure())`
unconditionally, so `rain_at()`'s `compute_spawn_scale` kept throttling
the spawn rate with the dragon "off". The HUD `dsty:` line showed the
fixed density while the RENDER path silently throttled — a display-vs-
reality lie, and a documented-contract violation. (The idle-FPS
reduction and the self-healer's DowngradeScene/PreemptiveThrottle arms
were correctly gated; the spawn-scale leg was the miss.)

**Defect A2 — the linear curve over-throttled at moderate pressure.**
The v50 curve was `scale = (1 - 0.75*p).clamp(0.25, 1.0)` (aggressive:
factor 0.9, floor 0.10). At the owner's observed pressure (~0.6, a slow
terminal write path) a monolith 0.85 density landed at
`0.85 * (1 - 0.75*0.6)` = **0.4469 → HUD `dsty: ~0.47`** — nearly half
the configured density, from a curve with NO dead zone: even 5%
pressure already cut 3.7%. That is the "extreme throttle" the owner
rejected.

### The v51.2 masterclass curve (owner's literal numbers)

`src/central_control_rains/density_throttle.rs` (extracted to its own
submodule to keep `mod.rs` under the 800-LOC cap) — single source of
truth `compute_spawn_scale(pressure, aggressive, density)`, feeding
both `rain_at()` and the HUD `dsty:` line:

| pressure         | condition      | target density (absolute)   |
| ---------------- | -------------- | ---------------------------- |
| p <= 0.05        | none           | the configured density      |
| 0.05 < p < 0.30  | low            | 0.84 -> 0.70                |
| 0.30 <= p < 0.60 | medium         | 0.70 -> 0.50                |
| 0.60 <= p <= 1.0 | high (rare)    | 0.50 -> 0.10                |

Design properties (why this is the peak, not over-engineering):

- **Ceiling = the configured density** (CLI `-d` > config `density` >
  scene builtin, e.g. monolith 0.85) — the throttle only ever reduces
  below it. The owner's "reduce density to 0.84, 0.83" stepping is the
  low-band entry from the 0.85 ceiling.
- **Dead zone** (p <= 0.05): light occasional overshoot decays in a few
  frames (`PERF_PRESSURE_DECAY` 0.02/frame) and never throttles —
  `dsty:` shows the exact configured density at idle.
- **Monotone band edges** = the owner's numbers. The owner's observed
  regime (p ~0.6) now floors at 0.50 (medium-band bottom), not 0.47.
- **Self-harmonizing for cheap scenes**: a 0.30-density scene sits below
  every band edge — untouched until the high band crosses it (p >= 0.80).
  Expensive scenes shed progressively; cheap scenes are left alone. No
  per-scene special-casing needed.
- **Aggressive mode** (self-healer, sustained high CPU): the SAME band
  edges read the pressure +0.20 deeper — one constant
  (`DENSITY_THROTTLE_AGGRESSIVE_SHIFT`), no second curve. At p=0.5:
  normal 0.5667 vs aggressive 0.40.
- **NaN guard** (CC2-03 pattern): NaN pressure maps to the dead zone;
  `clamp(floor, density)` with `floor = min(0.10, density)` keeps the
  min <= max invariant for densities below 0.10 (no panic).
- Removed as superseded: `PERF_PRESSURE_SPAWN_FACTOR` (0.75),
  `PERF_PRESSURE_SPAWN_FACTOR_AGGRESSIVE` (0.9),
  `PERF_SPAWN_SCALE_MIN` (0.25), `PERF_SPAWN_SCALE_MIN_AGGRESSIVE`
  (0.10) — the v51.2 band set replaces all four.

### The gate (defect A1 fix)

`event_loop_hud.rs::update_hud_state` now gates the render-path FEED:

```text
applied_pressure = power_dragon ? effective_pressure() : 0.0
hud.set_effective_pressure(applied)   // prs: shows the applied value
cloud.set_perf_pressure(applied)      // every consumer sees zero
```

With the dragon off, every cloud consumer (spawn scale, phosphor ramp,
glitch gate, atmospheric event gate, CRT vignette) returns to
zero-pressure behavior; `prs:`/`dsty:` never disagree. The
`PowerManager` still accumulates real pressure internally (the
self-healer and the post-exit report keep their signal). Additionally
the self-healer releases a stale `aggressive_throttle` when the dragon
turns off
(`run_self_healer`) — the config promise "disables
aggressive_throttle" now holds even for a flag that engaged while the
dragon was still on.

## 2. Part B — ambient snapback path contract audit (owner-approved continuation)

Same methodology as v51.1: walk every runtime writer of the ambient
state against the CLI-locked fallback contract. Findings:

**Finding B1 (the remaining defect) — ambient comment-out left the
engine stuck on the ambient scene.** `ambient.*` keys are a
config-family overlay on the scene family. When the user commented out
ALL ambient keys while an ambient phase was applied, the rebuild's
`SceneBaseAction::SyncRuntime` arm synced the ambient-owned scene into
`base_cfg` and the AB-05 "restore" branch re-applied the contaminated
`new_cfg.scene_name` — the engine stayed on the ambient scene forever.
The same "last value sticks on comment-out" defect family v51.1 fixed
for the plain `scene` key.

**Finding B2 — the ground-truth nuke faked user ownership.** Both
AB-08 nuke sites (`event_loop_ambient.rs`) set
`cloud.user_override_since_ambient = true` when clearing ambient state
— a lie about who owned the visual state, which poisoned any later
rebuild's ownership decision (the flag is the input to the overlay
decision). The rx-drain + empty schedule + killed snapback + cleared
tracker already make re-application impossible; the fake was redundant
AND harmful.

**Verified-correct (no change needed):**

- startup priority (CLI flags defer ambient via snapback delay;
  lockless startup applies instantly — `CliExplicit::any()` from v51.1)
- rx-event apply vs CLI lock (config overlay outranks the lock at
  runtime)
- auto-snapback re-application (current phase, drift-aware reference)
- comment-in recovery (AB-09 identity reset refires the phase)
- scene key present during ambient removal (the plain key still wins
  via ApplyConfig)

### The v51.2 ambient contract

```text
ambient keys present  → ambient overlay outranks the CLI lock (runtime)
ambient keys removed  → overlay lifts:
    ambient-owned scene → revert to the locked startup scene family
    user shortkey scene → survives (shortkeys are runtime top priority)
```

Implementation (both paths cooperate — whichever sees the emptied file
first):

- `event_loop_scene_sync::resolve_scene_base_with_ambient` — pure
  decision function composing the scene-key delta with the overlay
  rule (`ambient_removed_between_maps` detects removal from the raw
  config-map pair, which survives nuke-cleared runtime state).
- `event_loop_ambient::revert_ambient_owned_scene` — the nuke-path
  revert (pre-clear ownership capture, apply the locked startup scene's
  runtime profile, rebuild the render triple). In practice this path
  wins the race: the AB-08 file re-read polls every frame while ambient
  is actively applied, beating the watcher's latency — both PTY runs
  exercised it.
- `poll_ambient_events` gained a `startup_cfg` parameter (pristine
  snapshot, same one the rebuild uses).
- The nukes no longer fake `user_override_since_ambient = true`.

## 3. Changes (8 source files + 2 test files, net +30 tests)

| File | Change |
| ---- | ------ |
| `src/central_control_rains/density_throttle.rs` | NEW — band constants + `compute_spawn_scale` (banded, ceiling, NaN-safe) |
| `src/central_control_rains/mod.rs` | re-export; old floor constants removed; tests module wired |
| `src/central_control_rains/tests.rs` | NEW — 14 curve tests (bands, ceiling, cheap scenes, aggressive, NaN) |
| `src/central_control_power_dragon/mod.rs` | `PERF_PRESSURE_SPAWN_FACTOR`(+AGGR) removed; pipeline doc updated |
| `src/engine/cosmic_dragon_engine/cloud/rain_at.rs` | passes `droplet_density` as the throttle ceiling |
| `src/interactive/hud/metrics.rs` | `dsty:` uses the banded curve with the density ceiling |
| `src/interactive/event_loop_hud.rs` | power-dragon pressure gate (feed 0.0 when off) |
| `src/interactive/event_loop_self_heal.rs` | stale aggressive release on power-dragon off |
| `src/interactive/event_loop_scene_sync.rs` | `ambient_removed_between_maps` + `resolve_scene_base_with_ambient` + 8 ambient tests |
| `src/interactive/event_loop_config_rebuild.rs` | overlay rule wired into the rebuild |
| `src/interactive/event_loop_ambient.rs` | nuke honesty + `revert_ambient_owned_scene` + `startup_cfg` param |
| `src/interactive/event_loop.rs` | passes `startup_cfg` to the ambient poll |
| `src/interactive/hud/tests_dragon_indicators.rs` | dsty tests rewritten for the bands (+4 new) |
| `src/interactive/hud/tests_chroma_metrics.rs` | dsty expectation 0.91 -> 0.80 (low band) |
| `src/interactive/tests_v51_2_power_dragon_gate.rs` | NEW — 4 gate tests (feed 0.0, prs consistency, stale aggressive) |
| `src/interactive/hud/mod.rs` | test-only `test_metric_line` accessor |

Full binary suite: **2045 passed, 0 failed** (2015 at v51.1 + 30 net).

## 4. Verification

- **Unit**: 14 curve tests + 8 ambient decision tests + 4 gate tests +
  11 rewritten dsty tests, all in the full 2045-suite.
- **Live PTY** (real binary, `COSMOSTRIX_LIVE_RELOAD_DEBUG=1`, graceful
  `q` exit; `--scene crystal-dragon` + `ambient.<now> = monolith` +
  `ambient-snapback-secs = 2`):
  - phase 1 (t=2s): `ambient: auto-snapback after 2.0s — applying
    phase (scene=monolith)` — the overlay outranks the CLI lock;
  - phase 2 (t=5s): `ambient: schedule emptied — reverting
    ambient-owned scene 'monolith' to the locked startup scene
    'crystal-dragon'` — **the owner's contract, live**;
  - phase 3 (t=10s): scheduler refires + re-applies monolith (final
    verbose summary `scene: monolith (was crystal-dragon)`) —
    comment-in recovers.
  - Diagnostic note: the per-frame AB-08 file re-read (nuke path) wins
    the comment-out race against the watcher, so the PTY exercised the
    nuke revert; the rebuild's RestoreLocked arm is covered by unit
    tests. The nuke's `user_override` honesty fix was what let phase 3
    re-apply cleanly via the rx branch.
- **10s monolith 80x24 A/B** (A/A2 pre-change vs B/B2/B3/B4 post-change,
  same machine): visual parity — entropy 3.2938-3.2954 vs
  3.2936-3.2949, gini 0.8957-0.8960 vs 0.8961-0.8962, dirty cells
  56.74-56.78 both, active streams 23 identical, allocs/deallocs
  bit-stable 563/553 (one B2 run showed 564 — one-off noise, B3/B4 back
  to 563), stability excellent, fps within the session noise band
  (A-spread itself was 2.5%). The banded curve is behaviorally
  identical at bench pressure (p=0 dead zone). Raw JSON + PTY trace:
  `benchmark/bench-labs/v51_2_pdragon_ambient/`.

## 5. LTS notes

- The band constants live in ONE file (`density_throttle.rs`) with the
  full rationale; `docs/CENTRAL_CONTROL_RAINS_USAGE.md` §3.11 + the
  constants dump were synced (the removed `PERF_SPAWN_SCALE_MIN` no
  longer appears as current anywhere).
- The power-dragon gate centralizes the pressure-feed decision in
  `update_hud_state` (one site, both HUD + cloud), keeping the
  self-healer's own gating unchanged — no second policy layer.
- The ambient overlay rule is a pure function
  (`resolve_scene_base_with_ambient`) — the same testability property
  as v51.1's `resolve_scene_base_action`.
- Not done (over-engineering, skipped deliberately): per-pressure
  telemetry for the band transitions; a CLI flag to reshape the bands;
  plumbing `startup_cfg` deeper into the scheduler thread. The
  residual documented gap: if the watcher dies AND no rebuild ever
  arrives AND the user never edits config again, a reverted-by-nuke
  scene stays at the locked scene (which is the contract target
  anyway) — no action needed.
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
