<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Cosmic Dragon Engine — LTS KEY

> Latest activity on top. This file is the simplified lock/unlock
> signature log. For full audit detail (A/B benchmarks, file lists,
> stability signals), see [README.md](README.md) and [RULES.md](RULES.md).

## LOCK

> Engine re-locked at commit `2e6959f` after **BD-02 Corner Gradient LTS Stabilization**.
>
>
> ## Change Summary
>
> - **BD-01**: Precise centering formula fix (`(cols-box_w)/2` instead of `cols/2-box_w/2`)
> - **BD-02**: Corner-aware gradient system — bottom corners (╰╯) use bright anchor (t=0.8),
>   top corners (╭╮) follow natural triangle-wave gradient (dark at t=0, t=1.0)
> - **LTS Hardening**: Named constants, pre-allocated HashSet, defensive bounds checks,
>   numerical clamping, comprehensive design invariant documentation
>
>
> ## Metrics
>
> - **Files changed**: 1 (`cloud/mod.rs`)
> - **LOC impact**: +71 insertions, -17 deletions (net +54 LOC)
> - **Gatekeeper**: ✅ `cargo fmt --check` PASS, ✅ `cargo clippy -D warnings` PASS
> - **Visual rating**: 10/10 (owner-verified production ready)
>
>
> ## Design Invariants (LTS Guaranteed)
>
> 1. Bottom corners (╰╯) ALWAYS use bright anchor → visual anchoring
> 2. Top corners (╭╮) follow natural gradient → chroma dragon flow
> 3. Triangle wave ensures no sharp color gaps on left/right borders
> 4. All t-values clamped to [0.0, 1.0] → safe interpolation
>
>
> Signoff: **oxyzenQ** — 2026-08-23T00:45:00Z — BD-02 corner gradient LTS stabilization

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

> **UNLOCK cosmic-dragon-border** at commit `8de6bb0`, 2026-08-23T00:15:00Z
>
> **Author**: oxyzenQ (Cosmic Dragon AI Agent)
> **Reason**: Owner-reported visual issue: message overlay border corners (╰╯) had
> misaligned white heads that didn't perfectly enter round corners. Bottom-left corner
> appeared too far forward, bottom-right too far back. Required BD-01 (centering fix)
> and BD-02 (corner gradient system) to resolve.
>
> **Files changed**:
> - `src/cosmic_dragon_engine/cloud/mod.rs` (BD-01 centering + BD-02 gradient)
>
> **A/B delta** (vs locked baseline `24fa1be`):
> - Gatekeeper: ✅ cargo fmt --check PASS, ✅ cargo clippy -D warnings PASS
> - Visual audit: 10/10 owner-verified production ready
>
> **LOC impact**: +71 insertions, -17 deletions (net +54 LOC)
>
> **Tests**: All existing tests pass (no test regressions)

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
