<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Future Backlog — Doc Maintenance Cleanup

> **Status**: EXECUTED and re-triaged (2026-09-20, NIGHT-hunt-5 — the
> post-v100.0.2 total staleness pass). This backlog is the standing
> doc-cleanup ledger. Earlier passes, in order: NIGHT-hunt-45 + docs-6
> (2026-09-14, first full triage), NIGHT-hunt-46 + docs-7 (2026-09-14,
> README flag audit + docs-audit truth-column refresh), NIGHT-cleanup-1
> (2026-09-20, dead-script sweep — 27 one-off harnesses removed),
> NIGHT-cleanup-2 (2026-09-20, engine lock-protocol retirement),
> NIGHT-hunt-5 (2026-09-20, this pass), NIGHT-boost-6 (2026-09-24,
> post-reorg flat-path citation sweep + NIGHT-perf-1 HUD.md drift).
>
> **NIGHT-boost-6 outcome** — 23 flat `scripts/*.sh|py` citations in
> the live corpus repointed at the post-reorg homes (8 docs-audit
> flags + 6 manual-sweep finds the history-context exemption had
> accidentally absorbed + wrapped re-frames); the NIGHT-lts-1 debt
> record corrected to the one live LOC_EXEMPT (depthbore.py); HUD.md
> palette-refresh section rewritten to the `palette_gen` gate reality.
> Standing note for future auditors: the docs-audit
> history-context exemption has a false-negative class — a tool
> citation sitting near history words about a DIFFERENT subject
> passes; the belt-and-suspenders move is the manual
> `scripts/X.sh|py` existence sweep over the live corpus.
>
> **NIGHT-hunt-5 outcome** — fixed in this pass:
>
> - `docs/ENDURANCE.md` — the dead-script how-to sections (how to run,
>   smoke test, process resolution, summary output, header validation,
>   no-logs guidance) for the removed `monitor-cosmostrix.sh` +
>   `endurance-summary.sh` were deleted (~127 lines); the historical
>   record (CSV format spec, acceptance criteria, past results, run
>   template) is kept and the format framing moved to past tense.
> - `test/engine/chroma_dragon_engine/tests/lock.rs` — the suite header
>   still said "Phase 9-C" while `CHROMA_DRAGON_ENGINE_VERSION` is
>   "9-D (locked)"; the header is aligned to 9-D.
> - `src/engine/chroma_dragon_engine/README.md` — the phase-history 9-C
>   row now names both removed variants (Cartesian + sRGB-linear),
>   matching commit `2e20f6cc` and the lock-suite narrative.
> - `docs/THREE_DRAGON_ENGINES.md` — the lock-suite sentence pointed at
>   a wrong glob; it now names the two real lock suites (Chroma 19
>   invariants, Cosmic renderer 17 invariants) and the Crystal
>   per-subsystem test suites.
> - `docs/VERIFY_RELEASE.md` — the key-details "verified against" line
>   was refreshed from the v100.0.1 to the v100.0.2 release signatures.
> - `.github/workflows/aur.yml` — the `repository_dispatch` trigger
>     comment described a "backward compat, keep until" state that no
>     longer matched reality; it now describes the live mechanism
>     (release.yml posts the event after a release publishes).
> - `scripts/audit/docs-audit.py` — truth notes refreshed (2952 `#[test]`
>   fns = 2418 in `test/` + 534 in `src/`), the corpus rules encoded
>   (see section 1), and sections 1-3 gained negation/history context
>   awareness so intentional history is no longer tool noise.
>
> **Standing policy** — every remaining stale-path mention in the live
> corpus is a negation-context record (see section 2); a bare path,
> count, or phase claim with no history framing is a defect. The
> "project ships 80+ .md files" line inside the injected
> `COSMOSTRIX-DISCLAIMER` blocks is a lower bound that stays true and is
> re-injected verbatim by `scripts/gates/inject-disclaimer.sh` (updating it
> would fork the disclaimer into two variants across the historical
> corpus — accepted as-is).

> **Owner directive**: "log broken links to FUTURE_BACKLOG.md (35+
> historical broken refs in CHANGELOG/CONTRIBUTING — deferred per
> historical record contract, but could be noted for future cleanup)."

## 1. Corpus rules (what `scripts/audit/docs-audit.py` audits)

The live corpus is every git-tracked `.md` file EXCEPT the historical
snapshots, which are never rewritten:

- `docs/archive/**` — archived everything (audits, research, specs).
- `docs/research/**` — dated investigation logs.
- `docs/audits/**` — dated audit reports.
- `CHANGELOG-V50-ERA.md`, `CHANGELOG-V80-ERA.md`,
  `CHANGELOG-V100-ERA.md` — the era changelog split; as historical as
  `docs/archive/**`.
- `benchmark/bench-labs/**` A/B artifacts, except the hand-maintained
  index `benchmark/bench-labs/BENCH_LABS.md`.
- `benchmark/bench-labs/sweep_*.md` — machine-generated (pre-existing
  rule).
- `CHANGELOG.md` below the `## Unreleased` section — released history;
  only the `## Unreleased` section is live.

Within the live corpus, a reference to a path that no longer exists is
a defect UNLESS its context marks it as intentional history — explicit
removal/retirement/move/example framing on the line, in the preceding
3 lines, or under the nearest markdown heading (the same awareness
`scripts/audit/stale-hunt.py` applies to Rust comments).

## 2. Intentional-history register (live corpus, kept by design)

| File | Reference | Framing |
|------|-----------|---------|
| `docs/ENDURANCE.md` | removed `monitor-cosmostrix.sh` / `endurance-summary.sh` | removal note (commit `936c7ba`) + "historical sampling script" narration |
| `docs/RELEASE_GUARD.md` | removed `rc-smoke.sh` / `release-benchmark-report.sh` | "previously ran ... removed" history notes |
| `docs/RULES.md` | removed `gen-density-presets.py` | "was deleted" (density-map retirement) |
| `docs/VISUAL_IDENTITY.md` | removed `PRESET_BATTLE_VERDICT.md` | "was deleted by the 2026-08 orphan-doc cleanup" |
| `docs/THREE_DRAGON_ENGINES.md` | retired `KEY.md` / `RULES.md` / `dragon-history.sh` | "former ... was retired 2026-09-20" narration |
| `benchmark/HIST_BENCH.md` | removed `bench-compare.sh` / `release-benchmark-report.sh` | "historical ... removed" measurement records |
| `docs/LIVE_RELOAD_BEHAVIOR.md` | removed `live_pty_trace.log` (v51.2 ambient lab evidence) | "the trace log itself was later removed in a benchmark-artifact cleanup" framing (registered NIGHT-improve-2) |
| `benchmark/bench-labs/v51_2_pdragon_ambient/README.md` | removed `live_pty_trace.log` | "(removed in a later benchmark-artifact cleanup)" bullet framing (registered NIGHT-improve-2) |
| `benchmark/research/COMPETITOR_COMPARISON.md` | removed `naive_matrix.py` | "was written as a baseline" |
| `src/cosmic_dragon_incubator/README.md` | pre-restructure flat paths (`src/cloud/`, `src/frame.rs`, `src/runtime.rs`, the `cosmic_dragon_egg_io_uring` experiment) | "at that time" history section |
| `src/RULES.md` | `src/A/B_tests.rs` → `test/A/B_tests.rs`, `src/my_group/submodule_a.rs` | illustrative mirror-mapping / re-export-pattern examples |
| `CHANGELOG.md` (Unreleased) | removed scripts + paths inside cleanup entries | the entries document the removals themselves |

## 3. Stale count claims (historical — leave as-is)

The era files and dated records quote test counts and theme counts that
were accurate at their time; they stay verbatim. Current numbers for
anyone triaging (recounted 2026-09-26, NIGHT-improve-2): 2998 `#[test]`
fns across the tracked tree (2436 in `test/` + 562 in `src/`), 44
builtin themes (`THEME_COUNT`, `src/theme/mod.rs`), 515 tracked `.rs`
files, 43 scripts in `scripts/`. The chroma engine sits at Phase 9-D
(the locked final form). Historical entries are never retroactively
updated.

## 4. Standing verification (run after any doc pass)

1. `python3 scripts/audit/docs-audit.py` — broken refs, stale paths, stale
   counts, duplicates over the live corpus. Expected: sections 1-3
   report nothing beyond context-exempt intentional history.
2. `python3 scripts/audit/stale-hunt.py` — comment-structure stale-reference
   scan over `src/**` + `test/**`. Expected: 0 stale flags, paths,
   modules. The duplicate-comment groups are the heuristic
   mirror-test pattern (parallel test families repeat their contract
   narration by design).
3. `bash scripts/gates/inject-disclaimer.sh --check` — every live `.md`
   file carries the disclaimer marker.
4. Commit: one micro-commit per task, per the micro-commit-push owner
   rule.

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
