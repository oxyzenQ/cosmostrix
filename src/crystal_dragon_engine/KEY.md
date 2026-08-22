<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Crystal Dragon Engine — LTS KEY

> Latest activity on top. This file is the simplified lock/unlock
> signature log. For full audit detail (A/B benchmarks, file lists,
> stability signals), see [README.md](README.md) and [RULES.md](RULES.md).

# LOCK

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

# UNLOCK

> Deep zombie purge of `crystal_dragon_engine/` in commit `3587ccb`.
> Removed entire `transition/` module (zombie: `CrystalDragonDrift`
> enum + `scheme()` method had false doc claiming `crystal_dragon_tick`
> returned it, but it returns `Option<ColorScheme>` directly).
> Deleted `theme_group`, `reserved_themes`, `polling_duration`,
> `min_dwell_duration`, `sensor::effective_mode` getter — all test-only
> consumers. Demoted `clock::current_local_hour` to `#[cfg(test)]`.
> 1581/1581 tests pass.
>
> Signoff: **oxyzenQ** — 2026-08-22T08:11:35Z — crystal-dragon zombie purge
