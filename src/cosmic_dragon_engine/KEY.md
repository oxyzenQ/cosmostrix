<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Cosmic Dragon Engine — LTS KEY

> Latest activity on top. This file is the simplified lock/unlock
> signature log. For full audit detail (A/B benchmarks, file lists,
> stability signals), see [README.md](README.md) and [RULES.md](RULES.md).

## LOCK

> Engine re-locked at commit `24fa1be` after final dragon audit (v50.0.0-alpha.7).
> Deep audit confirmed: all color output routes through Chroma Dragon, no
> hardcoded colors in render paths. A/B benchmark: avg_fps 90,819, peak_rss
> 4.23 MiB, 0 alloc/frame, frame_jitter=low, frame_time_stability=excellent.
> 17/17 lock tests pass. No regression vs prior baseline.
>
> Signoff: **oxyzenQ** — 2026-08-22T16:30:00Z — final dragon audit v50.0.0-alpha.7

> Engine re-locked at commit `0a86ff6` after deep zombie purge in
> `cloud/` (5 zombie Cloud methods removed: `rain` wrapper demoted
> to `#[cfg(test)]`, `set_glitchy` + `set_stuck_cell_sweep` deleted,
> `droplet_count` + `active_scene` demoted to `#[cfg(test)]`).
> No visual/behavior/perf change. ~1500+ tests pass.
>
> Signoff: **oxyzenQ** — 2026-08-22T09:01:59Z — cosmic-dragon zombie audit

> **3 Dragon Lock** in commit `69af079` after deeper audit for strengthening
> and stability.
>
> Signoff: **rezky_nightky** — 2026-08-19T14:40:05Z — vision & director
> project cosmostrix

## UNLOCK

> Deep zombie audit of `cloud/` (~5K LOC) + `interactive/` (~3K LOC) in
> commit `0a86ff6`. Purged 5 zombies in `cloud/`; `interactive/`
> verified clean. Opened audit because previous zombie sweep
> (commit `3587ccb`) skipped these two large directories.
>
> Signoff: **oxyzenQ** — 2026-08-22T09:01:59Z — cosmic-dragon zombie audit
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
