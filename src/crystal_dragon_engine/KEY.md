<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Crystal Dragon Engine — LTS KEY

> Latest activity on top. This file is the simplified lock/unlock
> signature log. For full audit detail (A/B benchmarks, file lists,
> stability signals), see [README.md](README.md) and [RULES.md](RULES.md).

## LOCK

> Engine re-locked at commit `24fa1be` after final dragon audit (v50.0.0-alpha.7).
> Deep audit confirmed: all zombie symbols purged (transition/ module deleted,
> theme_group/reserved_themes/polling_duration/min_dwell_duration/effective_mode
> removed, current_local_hour demoted to #[cfg(test)]). CLI intent guards on
> live-reload for crystal-dragon, power-dragon, async-mode, monolith-size.
> 68/68 tests pass. A/B: avg_fps 90,819, 0 alloc/frame, stability=excellent.
> No regression vs prior baseline.
>
> Signoff: **oxyzenQ** — 2026-08-22T16:30:00Z — final dragon audit v50.0.0-alpha.7

> Engine re-locked at commit `0a86ff6` after deep zombie purge and
> follow-up verification. `crystal_dragon_engine/` was swept in
> commit `3587ccb` (8 zombies purged: `transition/` module deleted,
> `theme_group` + `reserved_themes` + `polling_duration` +
> `min_dwell_duration` + `effective_mode` getter deleted,
> `current_local_hour` demoted to `#[cfg(test)]` in `clock/`).
> Verified clean again at `0a86ff6` with no regressions.
>
> Signoff: **oxyzenQ** — 2026-08-22T09:01:59Z — crystal-dragon zombie audit

> **3 Dragon Lock** in commit `69af079` after deeper audit for strengthening
> and stability.
>
> Signoff: **rezky_nightky** — 2026-08-19T14:40:05Z — vision & director
> project cosmostrix

## UNLOCK

> Deep zombie purge of `crystal_dragon_engine/` in commit `3587ccb`.
> Removed entire `transition/` module (zombie: `CrystalDragonDrift`
> enum + `scheme()` method had false doc claiming `crystal_dragon_tick`
> returned it, but it returns `Option<ColorScheme>` directly).
> Deleted `theme_group`, `reserved_themes`, `polling_duration`,
> `min_dwell_duration`, `sensor::effective_mode` getter — all test-only
> consumers. Demoted `clock::current_local_hour` to `#[cfg(test)]`.
> ~1500+ tests pass.
>
> Signoff: **oxyzenQ** — 2026-08-22T08:11:35Z — crystal-dragon zombie purge
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
