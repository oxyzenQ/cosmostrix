<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Chroma Dragon Engine — LTS KEY

> Latest activity on top. This file is the simplified lock/unlock
> signature log. For full audit detail (A/B benchmarks, file lists,
> stability signals), see [README.md](README.md) and [RULES.md](RULES.md).

## LOCK

> Engine re-locked at commit `24fa1be` after final Chroma Dragon integration
> audit (v50.0.0-alpha.7). Deep audit confirmed: ALL Color::Rgb constructors
> in render paths route through Chroma Dragon functions (blend_toward_rgb,
> scale_rgb, apply_brightness_rgb_unclamped, interpolate_palette_color). No
> hardcoded Color::White or Color::Rgb bypassing the engine. Border gradient
> fixed: triangle wave eliminates sharp white→black gap on left border.
> Chroma Dragon Routing Rule codified in RULES.md. 19/19 lock invariants
> pass. A/B: avg_fps 90,819, 0 alloc/frame, stability=excellent. No regression.
>
> Signoff: **oxyzenQ** — 2026-08-22T16:30:00Z — final dragon audit v50.0.0-alpha.7

> Engine re-locked at commit `0a86ff6` after deep zombie audit of
> `shaders/` (~3900 LOC) confirmed clean — zero zombie symbols found.
> All `pub(crate)` items verified with production callers: `ShaderCtx`,
> `CharLoc`, `TRAIL_EXP_LUT`, `column_coherence_perturbation`,
> `hue_drift_offset`, `color_uses_previous_palette`,
> `resolve_cell_color`, `TransitionLabEntry`, `TransitionLTable`,
> `apply_l_smoothing`. No code changes applied — the lock is the
> appropriate action.
>
> Signoff: **oxyzenQ** — 2026-08-22T09:01:59Z — chroma-dragon zombie audit

> **3 Dragon Lock** in commit `69af079` after deeper audit for strengthening
> and stability.
>
> Signoff: **rezky_nightky** — 2026-08-19T14:40:05Z — vision & director
> project cosmostrix

## UNLOCK

> Deep zombie audit of `shaders/` in commit `0a86ff6`. Opened audit
> because previous zombie sweep (commit `3587ccb`) skipped this
> directory. Verified zero zombies across 7 source files. Audit
> closed with no code changes.
>
> Signoff: **oxyzenQ** — 2026-08-22T09:01:59Z — chroma-dragon zombie audit

> Stale path refs + EnergyZen missing from `all_schemes()` test helper
> (INV-2 silently skipped v50 masterclass theme) in commit `809a897`.
> Real bug fix: a future regression in EnergyZen's palette construction
> would NOT have been caught by the lock suite. Also 15+ stale doc
> path refs updated.
>
> Signoff: **oxyzenQ** — 2026-08-19T16:36:02Z — chroma-dragon deeper audit
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
