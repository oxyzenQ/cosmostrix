<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Cosmic Dragon Engine — LTS KEY

> Latest activity on top. This file is the simplified lock/unlock
> signature log. For full audit detail (A/B benchmarks, file lists,
> stability signals), see [README.md](README.md) and [RULES.md](RULES.md).

# LOCK

> Engine re-locked at commit `0a86ff6` after deep zombie purge in
> `cloud/` (5 zombie Cloud methods removed: `rain` wrapper demoted
> to `#[cfg(test)]`, `set_glitchy` + `set_stuck_cell_sweep` deleted,
> `droplet_count` + `active_scene` demoted to `#[cfg(test)]`).
> No visual/behavior/perf change. 1581/1581 tests pass.
>
> Signoff: **oxyzenQ** — 2026-08-22T09:01:59Z — cosmic-dragon zombie audit

> **3 Dragon Lock** in commit `69af079` after deeper audit for strengthening
> and stability.
>
> Signoff: **rezky_nightky** — 2026-08-19T14:40:05Z — vision & director
> project cosmostrix

# UNLOCK

> Deep zombie audit of `cloud/` (~5K LOC) + `interactive/` (~3K LOC) in
> commit `0a86ff6`. Purged 5 zombies in `cloud/`; `interactive/`
> verified clean. Opened audit because previous zombie sweep
> (commit `3587ccb`) skipped these two large directories.
>
> Signoff: **oxyzenQ** — 2026-08-22T09:01:59Z — cosmic-dragon zombie audit
