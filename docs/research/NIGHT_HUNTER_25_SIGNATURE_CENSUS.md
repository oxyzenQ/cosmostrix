<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-hunter-25 — the remaining too_many_arguments census, part 1 (cold families)

Owner approval 2026-09-09: "the 12 remaining too_many_arguments
suppressions in out-of-scope families (bench, shaders, engine draw
paths)". Executed as two batches by risk: this commit closes the
cold-path families and the stale allows; the hot cell-draw family
(shaders `resolve_cell_color`, render `get_attr`, solar
`draw_solar_cell`, intro `render_particle_cell`, post_rain,
bench `emit_cell_lean`) is scheduled for the next hunt with its own
rigorous A/B — those are per-frame inner-loop signatures where a
parameter-bundle design deserves undivided attention.

## The census was 12 — but 4 of them were already dead

The survey found four `#[allow(clippy::too_many_arguments)]`
attributes that no longer suppressed anything (clippy `-D warnings`
passes with them deleted):

- `ghost_events.rs evaluate_triggers` — 6 args with self after the
  v30 dragon-egg hunt dropped three dead parameters (its own comment
  documents the reduction; the allow was never removed).
- `build_cloud_cfg.rs CfgInputs` — the allow sits on a STRUCT
  declaration (too_many_arguments never fires on structs); it is a
  leftover from the pre-hunter-22 function signature that the
  CfgInputs refactor replaced. The twin allow on `build_cloud_cfg`
  itself guards a function that takes ONE parameter.
- `cloud/mod.rs Cloud::new` — exactly 7 args; the lint fires above 7.

Stale allows are their own small defect class: they advertise
signature debt that no longer exists and mask future regressions
(a new 8th parameter would silently ride the old allow).

## Real refactors (cold paths)

- **startup_verbose `run_verbose_startup`** — 25 positional
  parameters, one call site, several same-typed neighbor hazards
  (two f32 densities, three u16 glitch bounds). Bundled into
  `VerboseInputs<'a>` (the CfgInputs pattern). En route: the
  `custom_palette` param was NEVER read by the body — its
  `#[allow(unused_variables)]` was the band-aid — the field is
  dropped outright and the call site stops computing it. The dump
  prints the palette NAME and BG, both still carried.
- **bench_helpers `format_backpressure_section`** — 11 positional
  parameters with cross-wirable f64/f32 pairs (avg vs
  utilization_sum, peak vs utilization_max). Bundled into
  `BackpressureStats<'a>`; both call sites (event_loop_finalize +
  the unit test) construct named fields. The old "struct would be
  overkill" comment predated the 11th parameter.

## Verification

- 2561 tests pass (no count change — pure signature refactor; the
  backpressure test now constructs named fields).
- check-all -q exit 0; gate-keepers 10/10; clippy -D warnings clean
  with the four stale allows deleted (the proof they were dead).
- main.rs hit the 800-LOC cap during the construction (804) —
  trimmed comment prose to 799 (same precedent as hunter-23).
- PTY smoke on the pro binary: `--verbose` dump renders through
  VerboseInputs; `--perf-stats` BACKPRESSURE section renders through
  BackpressureStats; both exit 0.
- A/B 10 s benches (stash methodology, 2 pairs per scene): the
  after side measured FASTER on both scenes (+1.1 % cinematic,
  +1.7 % monolith fps) — machine noise swinging the other way; the
  bench loop executes none of the changed code (verbose is opt-in
  pre-launch; backpressure prints once at exit). Visual metrics
  identical within noise. Artifacts:
  benchmark/bench-labs/night_hunter25/.

## Remaining census (6, all hot-path, next hunt)

shaders/base `resolve_cell_color` (the renderer's convergence
point), cloud/render `get_attr` (its thin wrapper — one bundle
design fixes both), solar_flare `draw_solar_cell`, intro_style
`render_particle_cell`, cloud/post_rain `post_rain_processing`,
bench_io `emit_cell_lean`.

> CLOSED by NIGHT-hunter-25 part 2 (see
> `NIGHT_HUNTER_25_PART2_SHARED_BUNDLE.md`): all six ride value
> bundles — `CellPaint` shared by the shader/render pair,
> `SolarCellPaint`, `ParticlePaint`, `PostRainInputs`, `StyleCursor`.
> src/ reached zero too_many_arguments suppressions.
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
