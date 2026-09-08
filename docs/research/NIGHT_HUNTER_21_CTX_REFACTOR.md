<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-hunter-21 — wart #3 resolved: the event-loop context-struct refactor

Owner mandate 2026-09-08 (NIGHT-hunter-3 verdict follow-up): the
rain loop's coupled mutable state was "the largest structural debt in
the flow — the context-struct refactor is the biggest candidate for
the next stability gain." This hunt executed it.

## The wart, quantified

The v50.0.0-beta.7 LOC refactor split the loop into 17 sibling
modules, but the state stayed as ~45 locals in `run_interactive`,
threaded into siblings as positional `&mut` parameters. Evidence:

- **13 `#[allow(clippy::too_many_arguments)]` suppressions** across
  the interactive family — the machine-detected signature of the
  wart.
- **Same-typed positional pairs the compiler cannot guard:**
  - `charset_preset: &mut String, scene_name: &mut String` — adjacent
    in `apply_config_rebuild`, `poll_ambient_events`,
    `try_auto_snapback`.
  - FOUR same-typed config refs in `apply_config_rebuild`:
    `base_cfg: &mut CloudConfig, startup_cfg: &CloudConfig,
    current_cfg: &mut CloudConfig, cfg: &CloudConfig`. Swapping
    `base`/`current` at a call site compiles and silently routes
    live-reload writes to the wrong layer.
  - `update_perf_stats`'s 12 accumulator `&mut`s (the `f64` trio
    `work_sum_s`/`pressure_sum`/`utilization_sum` among them).
  - The `w: u16, h: u16` pair.
  - `apply_config_rebuild` took **23 parameters**;
    `poll_ambient_events` 21 (+ `type_complexity`);
    `update_perf_stats` 22; `sample_p5_health` 11 (Linux-gated ref
    among them); `handle_resize` 10; `run_self_healer` 11;
    `try_auto_snapback` 11; `revert_ambient_owned_scene` 10.
- A latent redundancy in the same family: the siblings received BOTH
  `cfg` (the `&CloudConfig` fn arg) and `&startup_cfg` —
  content-identical clones (`startup_cfg = cfg.clone()`; `cfg` is
  immutable) threaded as two separate same-typed parameters.

## The fix

New module `src/interactive/event_loop_ctx.rs` (~420 LOC):

- **`SceneIdentity`** — charset / scene name / generation trio.
- **`ConfigLayers`** — `startup` (pristine; also replaces the
  duplicate `cfg` param — single source now), `base` (runtime
  mutable), `current` (live-reloaded), `pending`, `last_applied_map`.
- **`AmbientState`** — handle / schedule / entry / snapback /
  ground-truth budget + path.
- **`PerfCounters`** — the 12 report accumulators.
- **`FrameObs`** — the per-frame draw observation (sim+draw +
  post-draw outputs) the frame-tail siblings share.
- **`LoopCtx`** — the whole loop state; **`LoopCtxCore`** — the
  five-value render core the constructor takes (keeps `new()` under
  the clippy threshold and names the `w`/`h` pair).

`run_interactive` builds the ctx after the intro (the intro owns the
render core pre-loop), runs the startup-ambient block on ctx fields,
and the `while ctx.cloud.raining` loop reads/writes `ctx.<field>`
exclusively.

### Signature conversions

| Function | Before | After |
|---|---|---|
| `apply_config_rebuild` | 23 params (+ type_complexity allow) | `(&mut LoopCtx) -> bool` |
| `poll_ambient_events` | 21 params (+ type_complexity allow) | `(&mut LoopCtx) -> Option<f64>` |
| `update_perf_stats` | 22 params | `(&mut LoopCtx, &FrameObs)` |
| `handle_resize` | 10 params | `(&mut LoopCtx, pending)` |
| `sample_p5_health` | 11 params (cfg-linux gated) | `(&mut LoopCtx, &FrameObs) -> bool` |
| `run_adaptive_throttle` | 7 params | `(&mut LoopCtx) -> ThrottleResult` |
| `try_auto_snapback` | 11 params | `(&mut LoopCtx) -> bool` |
| `revert_ambient_owned_scene` | 10 params (private) | `(&mut LoopCtx) -> bool` |
| `apply_ambient_fps` | 5 params (stale allow) | `(f64, &mut LoopCtx)` |
| `run_self_healer` | 11 params | 4 granular muts + `HealInputs` |
| `drain_config_events` / `update_hud_state` / `run_sim_and_draw` / `post_draw_accounting` / `run_intro_sequence` / `finalize_session` | ≤ 7, test surfaces | unchanged (granular) |

`run_self_healer`'s four mutable targets stay granular deliberately:
they are distinct types (healer/reclaim/cloud/frame) — the compiler
already rejects transposition — and the four unit-test call sites
(`tests_v51_2_power_dragon_gate.rs`, updated to the named-field
`HealInputs`) keep constructing four small objects instead of a full
context.

### LOC + suppression outcome

- `event_loop.rs`: **924 → 795 LOC, LOC_EXEMPT removed** — under the
  800 cap with no exemption for the first time since the v50 split.
- `too_many_arguments` allows in the family: **13 → 3**, and the
  remaining three (`print_perf_report`, `set_final_state`,
  `print_final_runtime_state`) are post-exit verbose printers taking
  immutable VALUE tuples — a different wart family (not mutable-state
  coupling), left for a future hunt by scope discipline.
- The raining-loop family now carries **zero** suppressions.

## Verification

- **Full suite: 2536 passed, 0 failed** (the source-text contract
  test `rebuild_restores_ambient_deferral_flag_after_cloud_swap` was
  updated to the ctx form — it pins the swap→restore adjacency, which
  is unchanged).
- **clippy `-D warnings`: clean** across all targets.
- **LOC cap test: green** (795 < 800, no exemption).
- **E2E on the refactored loop (PTY, real binary):**
  - `hud_order_e2e.py` PASS — 25/25 labels, exact row order.
  - `hud_long_scene_e2e.py` PASS — NIGHT-hunter-20's fix survives
    the refactor.
  - Canonical live-reload smoke: `scene = "cinematic"` →
    `scene = "matrix"` edited mid-rain → `scn: matrix` + katakana
    charset on screen (watcher → drain → `apply_config_rebuild` →
    full scene-family switch through the refactored path). A
    CLI-locked scene-custom probe correctly kept the CLI scene (the
    lock contract, not a regression).
- **A/B 10 s benches (pro, dry, cinematic + monolith):** monolith
  visual metrics identical to the third decimal; cinematic inside the
  documented shared-VM noise band (fps 29.3K → 28.3K; entropy/gini
  stable). The bench harness never constructs the interactive loop
  (bench dispatch is a separate path), so this is the owner-rule
  safety check; the PTY e2e above is the behavioral verification.
  Full JSON: `benchmark/bench-labs/night_hunter21/`.

## Why this is a stability gain (not just cosmetics)

1. **The compiler now guards what it could not before.** The
   cross-wire hazards above (config layers, scene strings, perf
   floats, w/h) are all named fields — a transposed argument is now
   a compile error or a visibly-wrong field name, not a silent
   wrong-layer write.
2. **Signature churn ends.** Adding loop state = adding a field;
   sibling signatures no longer enumerate the world (the LOC_EXEMPT
   existed precisely because further splitting required threading
   more params).
3. **The duplicate `cfg`/`startup_cfg` threading is gone** — one
   pristine layer with one name.
4. **`event_loop.rs` fits the file cap without an exemption**, so
   future loop work is cap-governed like every other file.

## Files

- `src/interactive/event_loop_ctx.rs` — NEW (context + sub-structs).
- `src/interactive/event_loop.rs` — loop body on ctx; exemption gone.
- `src/interactive/event_loop_config_rebuild.rs`,
  `event_loop_ambient.rs`, `event_loop_perf_stats.rs`,
  `event_loop_p5.rs`, `event_loop_resize.rs`,
  `event_loop_adaptive.rs`, `event_loop_self_heal.rs`,
  `input.rs` — signature conversions.
- `src/interactive/mod.rs` — module registration.
- `test/interactive/tests_v51_2_power_dragon_gate.rs` — 4 call sites
  to `HealInputs`.
- `test/config/live_config/tests_alpha2_zero_key.rs` — source-text
  assertion to the ctx form.
- `benchmark/bench-labs/night_hunter21/` — A/B JSON + report.

<!-- COSMOSTRIX-DISCLAIMER -->
<!--
  Documentation Disclaimer — read before relying on any data point.

  This document may contain stale data, hardcoded counts, or outdated
  file paths and symbol names. Maintainers update source code but may
  forget to sync every doc — the project ships 80+ .md files and
  perfect sync is a known maintenance burden with diminishing returns.

  Source code (`src/**/*.rs`) is the single source of truth.
  Always cross-check against the actual `.rs` files before relying on
  any number, path, or symbol name here.
-->
