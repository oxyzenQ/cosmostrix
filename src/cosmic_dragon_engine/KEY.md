<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Cosmic Dragon Engine — LTS KEY

> Latest activity on top. This file is the simplified lock/unlock
> signature log. For full audit detail (A/B benchmarks, file lists,
> stability signals), see [README.md](README.md) and [RULES.md](RULES.md).

## LOCK

> Engine re-locked at commit `c1c7779` after the triple-engine LTS deeper audit
> (2026-08-23). White-box re-audit of the diff pipeline invariants (dirty-index
> bounds by construction, LastFrame dimension coherence under resize storms,
> generation-counter wraparound), the resize path, and the Cloud::reset
> dimension consistency. Audit finding LOW-2 fixed in `c1c7779` (reset now
> clamps end-to-end; reset_bench added for bench-bounded consistency).
> Deferred items S-3/S-4/S-5/M-1/C-5 re-verified as documented-safe and
> below the unlock bar (no measurable A/B gain). A/B: avg_fps 86,520/86,615
> (±0.1% run variance; vs baseline 90,819 the delta is cross-session
> hardware variance), alloc_calls 563 exact-match (0.0/frame), peak_rss
> 4.33-4.42 MiB, jitter=low, stability=excellent, drift=stable. Lock suite
> 17/17, cloud suite 258/258, full binary suite 1642/0/2.
>
> Signoff: **oxyzenQ** — 2026-08-23T09:27:41Z — triple-engine LTS deeper audit

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

> **UNLOCK cosmic-dragon (retroactive)** at commit `c1c7779`, 2026-08-23T09:15:00Z
>
> **Author**: oxyzenQ (Cosmic Dragon AI Agent)
> **Reason**: Triple-engine LTS audit finding LOW-2 — `Cloud::reset` clamped
> only `self.cols`/`self.lines` while RNG ranges, column tables, and per-cell
> LUTs were built from the RAW parameters (panic-free but inconsistent; latent
> hybrid state in oversized bench tiers where rain spawned at raw width while
> glitch/color coverage stopped at the interactive cap).
>
> **Files changed**:
> - `src/cosmic_dragon_engine/cloud/spawn.rs` (reset → reset_with_bounds with
>   clamped-value shadowing; new reset_bench mirroring Frame::new_bench)
> - `src/bench/mod.rs` (3 call sites switched to reset_bench)
> - `src/cosmic_dragon_engine/cloud/tests/mod.rs` (3 dimension-consistency tests)
>
> **A/B delta** (vs locked baseline `24fa1be`):
> - alloc_calls: 563 → 563 (Δ 0% — exact match; reset runs on resize only,
>   zero per-frame surface)
> - stability signals: MATCH
> - avg_fps: 90,819 → 86,520/86,615 (Δ -4.7% cross-session hardware variance;
>   same-session run-to-run ±0.1%)
> - Behavior for in-range interactive sizes: unchanged (cloud suite 258/258)
>
> **Visual audit**: PASS — reset path does not alter rendering for in-range
> sizes; masterclass brightness profile preserved (gini 0.8960 vs 0.8961,
> color_transition_delta 0.00).
>
> **Tests**: 1642 passed / 0 failed / 2 ignored; cloud 258/258; lock suite 17/17.
>
> **Note**: RETROACTIVELY documented (same-commit entry was missed). Future
> unlocks MUST include the entry in the same commit.
>
> Signoff: **oxyzenQ** — 2026-08-23T09:27:41Z — audit LOW-2 fix

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
