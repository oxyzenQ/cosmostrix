<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Docs LTS completeness audit (NIGHT-docs-2) — all documents complete before the stable LTS release

Owner brief (2026-09-11): "all documents should be complete before
release stable LTS, source code = truth." Scope: every non-archive
document cross-checked against the source of truth — the source code,
the live `--help`, and the live discovery commands (`--list-scenes`,
`--list-colors`, `--list-charsets`). 101 non-archive `.md` files
scanned (36 `docs/`, 43 `docs/research/`, 1 `docs/audits/`, root
docs, engine/module docs).

Commit: this round (docs fixes only — zero source changes; no
benchmark needed per the docs-only rule).

## Methodology (6 checks, script-pinned)

1. **Stale file paths** — every `src/…​.rs`, `docs/…​.md`,
   `scripts/…​.sh` style reference in non-archive docs checked
   against the working tree.
2. **Config-key coverage** — every `USER_CONFIG_KEYS` entry
   (41 keys) must appear in at least one non-archive doc; removed
   keys must not be presented as current.
3. **Live counts** — scene/theme/charset/msg-fill-style counts in
   high-traffic docs vs the live `--list-*` output (source of
   truth: 31 scenes, 44 themes, 25 charsets, 11 msg-fill styles).
4. **TODO/FIXME/STUB/WIP markers** — non-archive docs must carry no
   open work markers.
5. **Version drift** — "current/latest" phrasing near version
   strings vs `Cargo.toml` (100.0.0-beta.1).
6. **Index coverage** — every `docs/*.md` file must be indexed in
   `docs/README.md`.

## Verdict by check

| check | verdict |
|---|---|
| config-key coverage | PASS — 0/41 undocumented |
| CLI flag coverage (supplementary) | PASS — all 55 `--help` flags appear in README/docs (`--uniform` appears only as removal prose in `--help` itself) |
| live counts | PASS — no drift found in high-traffic docs |
| TODO markers | PASS — all hits are meta-discussion of marker policy or historical scan records |
| version drift | PASS — all flagged hits are version-stamped feature provenance ("v80.0.0-beta.2 owner champion") or CHANGELOG history; no current-version mislabeling |
| stale paths | 124 hits → triaged into 3 classes (below); the actionable class fixed this round |
| index coverage | FIXED — 8 docs were missing from the index (see the finding below for the under-count story) |

## Findings and fixes

### Fixed: broken cross-references after archive moves (15 links, 7 files)

Live docs linked to files that had moved to `docs/archive/` without
the link being updated:

- `docs/CENTRAL_CONTROL_RAINS_USAGE.md` → `RAIN_DEPTH_AUDIT.md` (2 links)
- `docs/MAINTENANCE.md` → `audits/COSMIC_DRAGON_AUDIT.md`
- `docs/PHILOSOPHY.md` → `SIMD_FEASIBILITY.md` (2 links)
- `docs/SECURITY_AUDIT.md` → `audits/SECURITY_VULNERABILITY_AUDIT.md`, `SIMD_FEASIBILITY.md` (2), `STABILITY_AUDIT.md`
- `docs/TERMINAL_LIFECYCLE_MATRIX.md` → `audits/LTS_MATRIX_MIDSESSION_RETEST.md` (2)
- `docs/VISUAL_IDENTITY.md` → `RAIN_DEPTH_AUDIT.md`
- `docs/LIVE_RELOAD_BEHAVIOR.md` → `LTS_AUDIT_CONFIG_LIVE_RELOAD.md`

All now point at the `docs/archive/…` location (the files exist
there; the historical record is preserved, the links resolve).

### Fixed: release-process docs taught removed scripts as live gates

- `docs/RELEASE_GUARD.md` Gate 2 ran `scripts/rc-smoke.sh` and Gate 4
  (plus the "Pattern for Future Releases" step) ran
  `scripts/release-benchmark-report.sh` — both removed in a
  dead-script cleanup. Gate 2 now runs the current equivalents
  (`build.sh check-all -q`, `gate-keepers.sh`,
  `verify-release-build.sh`); Gate 4 and the pattern step now teach
  the manual 5-run `--benchmark` loop (which the doc already
  documented as the manual fallback).
- `benchmark/HIST_BENCH.md` presented `release-benchmark-report.sh`
  and `bench-compare.sh` as current how-tos; both sections rewritten
  as historical records with the current process pointed out.

### Fixed: docs index gaps

`docs/README.md` did not index eight live documents (the first read
of the audit output was line-truncated and showed only the first
two; the full extractor sweep is the authoritative list — always
drain the whole report before triaging):

- `docs/CENTRAL_CONTROL_POWER_DRAGON.md` (Power Dragon subsystem —
  throttle, idle FPS, thermal + self-heal gates) → added under
  Benchmarking & Performance.
- `docs/CLI_SUGGESTION_SYSTEM.md` (did-you-mean engine, shared by
  every value surface) → added under Architecture & Engine.
- `docs/CONFIG_LIVE_RELOAD_DISCLAIMER.md` (honest-limitations
  philosophy) → added next to LIVE_RELOAD_BEHAVIOR.
- `docs/DEPENDENCY_AUDIT.md` (dependency inventory) and
  `docs/SECURITY_AUDIT.md` (security posture) → added under Build,
  Release & CI.
- `docs/USAGE_PIPE_REDIRECT.md` (fatal-usage catalog) → added under
  Terminal Compatibility & Recovery.
- `docs/VISUAL_IDENTITY.md` (visual identity system) → added under
  Color & Theming.
- `docs/FUTURE_BACKLOG.md` (parked ideas + file-migration record)
  → added as a pointer after the Build, Release & CI table.

Final verification: every `docs/*.md` file is now indexed — the
completeness check reports zero gaps.

### Already correct (verified, no changes)

- `docs/ENDURANCE.md` — the removed monitor/summary scripts are
  covered by the doc's own dated note ("historical methodology
  record; for current measurements use `--benchmark` output and
  `benchmark/benchmark.sh sweep`"); the instruction blocks below the
  note are explicitly historical.
- `docs/FUTURE_BACKLOG.md` — its stale-looking references are the
  doc's own purpose: a "where did the files go" migration table
  ("deleted (naive matrix script removed)") plus background pointers.
- CHANGELOG.md historical entries, dated `docs/research/` records,
  and dragon `KEY.md` lock entries — point-in-time records by
  convention (the changelog describes where things WERE; the
  research docs carry dated snapshots; the locks carry dated
  signatures). Left untouched per the archive/history rule.
- `src/RULES.md` "A/B_tests.rs / my_group" hits — illustrative
  examples in the module-organization prose, not real paths.
- `docs/RENDER_ENGINE.md` `src/engine/frame.rs` — a hypothetical
  `git mv` example in anti-pattern guidance, not a reference.

## Tooling

The audit is reproducible via the session script
`docs_lts_audit.py` (stale paths, key coverage, live counts, TODO
markers, version drift, index coverage). The two supplementary
checks (CLI flag coverage, KNOWN_ISSUES currency review) ran
one-off: flag extraction from `--help` cross-checked against the
README+docs corpus; KNOWN_ISSUES.md reviewed section-by-section
(6 platform quirks, all with symptoms/workarounds/status — current).

Gates for this commit: gate-keepers all green (disclaimer injected
into this file, markdownlint, language audit), `cargo test` 2863/2863
(docs-only change, no source touched), no benchmark (docs-only).
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
