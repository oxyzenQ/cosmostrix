<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Docs Deep Audit — Broken References, Stale Data, Duplicates

**Date**: 2026-08-23 · **Scope**: all 71 git-tracked `.md` files excluding
`docs/archive/**` and auto-generated `benchmark/bench-labs/sweep_*` (per
owner directive) · **Method**: **pattern-engine sweeps, not manual reading**
— every finding below was produced by a repeatable script
(`docs_audit.py`, methodology in §1) that extracts and verifies references
mechanically. Source code is truth; docs are verified against it.

**TL;DR**: 70 broken file references found -> **every living doc fixed
(23 corrections applied)**; remaining references are intentional historical
records now labeled with snapshot banners. **Zero file-level duplicates
exist** (verified two ways). Stale counts (tests/themes/invariants) corrected
in 6 living docs; historical records intentionally left frozen, with
rationale in §4.

---

## 1. Methodology — rg-philosophy, not eyeballs

The owner's observation is correct: agents that *read* docs one by one miss
systemic rot, because rot is a *pattern* (an old path shape, a wrong count,
a moved file) best found with sweeps. The audit engine
(`/home/z/my-project/scripts/docs_audit.py`, mirrored methodology below)
runs five passes over the tracked non-archive `.md` corpus:

1. **Reference extraction + dual resolution** — every markdown link and
   backtick path (`docs/…`, `src/…`, `scripts/…`, `benchmark/…`) is
   resolved file-relative, then repo-root; unresolved = broken.
2. **Stale path shapes** — regex sweep for the pre-refactor flat layout
   (`src/cloud/`, `src/config.rs`, `src/adaptive.rs`, …) and known moved
   modules.
3. **Stale count claims** — sweeps for `43 themes`, `18 invariants`,
   `~1500 tests`, `Phase 9-B`, etc., each paired with the verified current
   truth (44 themes / 19 lock invariants / 1649 tests, all re-verified by
   run on 2026-08-23).
4. **Duplicate detection** — H1-title similarity grouping + normalized
   content-prefix signature (first 400 alphanumeric chars).
5. Manual triage of every hit into: **living-doc defect** (fix),
   **historical record** (leave, banner if it pollutes search), or
   **intentional mention** (leave).

## 2. Findings and fixes

### 2.1 Broken references — 70 found, living docs all fixed

**Fixed (living docs that misrepresented the tree):**

| File | Defect | Fix |
|------|--------|-----|
| `benchmark/HIST_BENCH.md` | link to moved `docs/ATMOSPHERE_ENGINE.md` | -> `docs/archive/specs/ATMOSPHERE_ENGINE.md` |
| `docs/AMBIENT_SCHEDULER.md` | `src/config/live_config.rs` (module became a dir) | -> `live_config/mod.rs` |
| `docs/HUD.md` | `src/interactive/hud.rs` | -> `hud/mod.rs` |
| `docs/README.md`, `docs/RULES.md` | `src/engine/chroma_dragon_engine/post/climate.rs` | -> `climate/mod.rs` |
| `docs/RENDER_ENGINE.md` | referenced deleted `scripts/bench-compare.sh` | -> `benchmark/benchmark.sh` |
| `docs/PERFORMANCE_ACROSS_SCALES.md` | pointed at long-gone `benchmark/scaling_results.{json,md}` | -> `benchmark/bench-labs/` per-sweep outputs |
| `docs/ENDURANCE.md` | built its workflow on two scripts removed in `936c7ba` | historical note added; points to `--benchmark` + `benchmark.sh sweep` |
| `docs/research/VISUAL_MODE_AUDIT.md` | misspelled script name `visual_mode_audit.py` | -> `visual-mode-audit.py` (the real file) |

**Intentional, now labeled (historical records):** 61 of the 70 references
live in dated research snapshots (`docs/research/FLAGS_AUDIT…`,
`DRAGON_HUNT_V2…`, `CHROMA_DRAGON_ENGINE_AUDIT…`, `MATRIX_1999_FILM…`,
`MATRIX_BOLD…`, `MOUSE_EFFECTS…`, `SELF_HEALING…`,
`benchmark/research/COMPETITOR_COMPARISON.md`) whose `src/config.rs`-era
paths are accurate *for their audit date*. Rewriting them would falsify the
record; instead each now carries a **snapshot banner** stating paths/counts
reflect the codebase at audit time. Two mentions remain by design:
`docs/VISUAL_IDENTITY.md` deliberately names the deleted
`PRESET_BATTLE_VERDICT.md` while explaining where its content lives, and
`src/RULES.md`'s `my_group/submodule_a` is a template example, not a path.

### 2.2 Stale counts — corrected where "current" was claimed

| File | Was | Now (verified 2026-08-23) |
|------|-----|--------------------------|
| `docs/COSMIC_DRAGON_ARCHITECTURE.md` | 1500+ tests | 1649 tests |
| `docs/MAINTENANCE.md` | 1400 tests | 1649 tests |
| `docs/CENTRAL_CONTROL_DRAGON_POWER.md` | 1417 tests | 1649 tests (full suite) |
| `src/engine/chroma_dragon_engine/README.md` | 18 invariants; 1500+ tests | 19 invariants; 1649 tests |
| `src/engine/cosmic_dragon_engine/README.md` | 1500+ tests | 1649 tests |
| `src/engine/crystal_dragon_engine/README.md` | 1500+ tests | 1649 tests |

**Left frozen by design**: `CHANGELOG.md` (per-version release records),
`KEY.md` lock signatures (test counts are part of the historical signoff
evidence), superseded audits (`RAIN_DEPTH_AUDIT`, `VISUAL_MODE_AUDIT`,
`SECURITY_AUDIT` — all bannered), and research snapshots. Historical counts
are evidence, not errors.

### 2.3 Duplicates — none found (verified, not assumed)

Both detectors came back empty: no H1-title similarity groups and **zero
identical-content-prefix pairs** across all 71 non-archive files. The
perceived duplication (owner's complaint) is actually **cross-referencing
overlap** — e.g. `SECURITY_AUDIT.md` vs `audits/SECURITY_VULNERABILITY_AUDIT.md`,
`RAIN_DEPTH_AUDIT.md` vs `VISUAL_IDENTITY.md` — which is now governed by
supersede banners pointing at one canonical doc per topic. The cleanup that
deleted `PRESET_BATTLE_VERDICT.md` (commit `49efe84`) already removed the
true duplicates; what remains is a single source of truth per subject plus
explicitly-bannered history.

## 3. Post-fix verification

Re-running the engine after the fixes: living-doc broken references =
**0**. Every remaining hit is inside a bannered historical file, an
intentional mention, or a template example. `gate-keepers.sh` (markdownlint,
disclaimer, SPDX, codespell) passes on all 24 modified files.

## 4. Round 2 (2026-08-23, same day) — README + KNOWN_ISSUES + root *.md

Owner follow-up audit of the root-level docs, with every claim verified
against source:

- **README.md — CLEAN.** All quantitative claims verified exact:
  18 built-in scenes (18 `name:` entries in `src/scene/mod.rs`),
  44 color themes (44 `scheme:` entries in `catalog.rs`),
  25 character sets (25 primary names in the `src/scene/charset.rs`
  resolver, aliases excluded). Platform table current (includes the
  windows-arm64 prebuilt added for the LTS). No broken references.
- **KNOWN_ISSUES.md — CLEAN.** All three documented issues verified
  still-accurate against source: the `i` HUD binding is still in
  `src/interactive/event_loop.rs` as documented; `--reset-terminal`
  exists with the 5-layer recovery sequence; the TTY cleanup
  description matches the terminal restore code. All cited file
  paths exist.
- **CHANGELOG.md — CLEAN.** The "43 builtin" mention sits in the
  v11.1.0 entry, written when the theme count WAS 43 — era-accurate
  historical record, frozen by policy.
- **CONTRIBUTING.md / TRADEMARK.md / NOTICE — CLEAN** (no engine
  findings in any pass).
- Engine hits in `src/engine/chroma_dragon_engine/RULES.md` ("43 themes",
  "18 invariants") were manually triaged: all occurrences are quotes
  INSIDE the historical UNLOCK log entry `809a897`, where they
  describe the 43->44 / 18->19 fix itself — historical record, not
  living claims.

Result: zero living-doc defects in the root docs. The audit engine
remains reusable (`python3 scripts/docs-audit.py`); its remaining
hits are all inside bannered historical files or historical log
entries, each manually verified as intentional.

## 5. Policy going forward (the part that keeps it clean)

1. **Audits are dated snapshots**: research/audit docs get the snapshot
   banner at creation; their paths are never "fixed" — they are history.
2. **Living docs cite truths, not numbers, where possible**: prefer "the
   lock suite" over "19 invariants" when the count is contractual;
   when a count is stated, it is verified at write time.
3. **One canonical doc per topic**; superseded docs keep a banner and
   their analysis value, never silent deletion (the PRESET_BATTLE_VERDICT
   lesson).
4. The audit engine is reusable: `python3 docs_audit.py` — run it in CI
   or before releases; new rot shows up as a diff in the broken-reference
   list.

---

Copyright (C) 2026 rezky_nightky (oxyzenQ). All rights reserved.
cosmostrix and the cosmostrix logo are trademarks of rezky_nightky (oxyzenQ).
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
