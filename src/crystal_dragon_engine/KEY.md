<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Crystal Dragon Engine — LTS KEY

> Latest activity on top. This file is the simplified lock/unlock
> signature log. For full audit detail (A/B benchmarks, file lists,
> stability signals), see [README.md](README.md) and [RULES.md](RULES.md).

## LOCK

> Engine re-locked at commit `c327803` after the crystal re-seal audit
> (2026-08-24). Confirms no crystal paths were touched in commits
> `5280ae1` (cosmic-side exp decay consolidation), `deff636`
> (cosmic-side SHA backfill + README crypto donations), or `c327803`
> (cosmic + chroma re-seal) — crystal engine is untouched by the
> v50.0.0-beta.5 masterclass easing migration and the post-amend
> re-seal cycle. No crystal ambient/scheduler/sensor/palette_groups/
> point_system/crystal_dragon_control/ambient_diag/mod.rs production
> code has been modified since the `c1c7779` baseline. The crystal
> UNLOCK at `9de2f44` (LOW-1 ambient scheduler `try_send` conflation
> fix — `DeliverOutcome` three-way contract, 4 new contract tests,
> retroactively documented) remains closed and re-sealed by the
> `c1c7779` triple-engine LTS deeper audit; no further UNLOCK has
> been opened since. Crystal ambient scheduler behavior re-verified
> (empty schedule idles 60s loops, single entry sleeps until boundary,
> live reload wakes condvar immediately, saturated-channel retry loop
> engages only when channel is full — pre-fix thread-termination
> behavior absent). A/B: no crystal code touched -> no measurable
> delta vs `c1c7779` baseline (alloc_calls 563 exact-match, 0.0
> allocs/frame; avg_fps within ±5% cross-session hardware variance;
> peak_rss 4.33-4.42 MiB). Tests: crystal suite 82/82,
> ambient_scheduler suite 17/17, full binary suite 1660/0/2, cosmic +
> chroma combined `lock_inv` filter 36/36. cargo fmt + clippy
> `--all-targets --all-features -D warnings` + gate-keepers.sh (7/7)
> all clean.
>
> Signoff: **oxyzenQ** — 2026-08-24 — crystal re-seal after cosmic-side
> v50.0.0-beta.5 amendments

> Engine re-locked at commit `c1c7779` after the triple-engine LTS deeper audit
> (2026-08-23, follows the 2026-08-17 nightly.1 audit which had not covered
> Crystal). Full white-box audit of sensor, ambient parsing, scheduler
> concurrency, point system, and control constants: zero unsafe, zero
> reachable panics on user input, SeqCst generation TOCTOU fix verified,
> bounded channel + catch_unwind verified. Audit findings LOW-1 fixed in
> `9de2f44` (try_send conflation — deliver() three-way contract, 4 unit
> tests). A/B: avg_fps 86,520/86,615 (two runs, ±0.1% run variance; vs
> locked baseline 90,819 the delta is cross-session hardware variance),
> alloc_calls 563 exact-match baseline (0.0/frame), peak_rss 4.33-4.42 MiB,
> frame_jitter=low, stability=excellent, drift=stable. Crystal suite 82/82,
> ambient_scheduler 17/17, full binary suite 1642 passed / 0 failed /
> 2 ignored.
>
> Signoff: **oxyzenQ** — 2026-08-23T09:27:41Z — triple-engine LTS deeper audit

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

> **UNLOCK crystal-dragon (retroactive)** at commit `9de2f44`, 2026-08-23T09:10:00Z
>
> **Author**: oxyzenQ (Cosmic Dragon AI Agent)
> **Reason**: Triple-engine LTS audit finding LOW-1 — the scheduler loop
> terminated its thread on ANY `try_send` error, conflating a transient full
> channel (`TrySendError::Full`) with a dead receiver (`Disconnected`). A
> saturated channel would silently disable ambient scheduling for the rest
> of the session while the rain kept running.
>
> **Files changed**:
> - `src/crystal_dragon_engine/ambient_scheduler/mod.rs` (deliver() helper, DeliverOutcome contract, both send sites)
> - `src/crystal_dragon_engine/ambient_scheduler/tests.rs` (4 new contract tests)
>
> **A/B delta** (vs locked baseline `24fa1be`):
> - alloc_calls: 563 -> 563 (Δ 0% — exact match; scheduler has zero per-frame surface)
> - stability signals: MATCH (jitter=low, stability=excellent, drift=stable)
> - avg_fps: 90,819 -> 86,520/86,615 (Δ -4.7%, cross-session hardware variance;
>   run-to-run variance in the same session is ±0.1%)
>
> **Scheduler behavior**: empty schedule / single entry / live reload all
> covered by the 17/17 ambient_scheduler suite (incl. the 4 new deliver()
> tests: delivered, receiver-gone, saturated-with-bound-elapsed, recovery).
>
> **Tests**: 1642 passed / 0 failed / 2 ignored (full binary suite);
> crystal 82/82.
>
> **Note**: RETROACTIVELY documented (same-commit entry was missed, matching
> the chroma 809a897 precedent). Future unlocks MUST include the entry in
> the same commit.
>
> Signoff: **oxyzenQ** — 2026-08-23T09:27:41Z — audit LOW-1 fix

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
