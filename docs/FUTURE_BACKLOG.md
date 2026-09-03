<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Future Backlog — Doc Maintenance Cleanup

> **Status**: DEFERRED (2026-09-03, S-master-HUNT-16). These items were
> found by `scripts/docs-audit.py` and manually triaged. They are
> documented here for a future dedicated doc-cleanup pass — NOT for
> immediate action. Source code is the single source of truth; these
> are doc-only issues that don't affect runtime behavior.
>
> **Owner directive**: "log broken links to FUTURE_BACKLOG.md (35+
> historical broken refs in CHANGELOG/CONTRIBUTING — deferred per
> historical record contract, but could be noted for future cleanup)."

## 1. Broken references in LIVE docs (fix in future pass)

These are live documentation files that reference paths which have
moved, been archived, or been deleted. They should be updated to
point to the current location or marked as archived.

### CONTRIBUTING.md (3 broken refs)

| Line | Broken ref | Current location |
|------|-----------|-----------------|
| (varies) | `docs/audits/COSMIC_DRAGON_AUDIT.md` | `docs/archive/audits/COSMIC_DRAGON_AUDIT.md` |
| (varies) | `src/cosmic_dragon_engine/frame.rs` | `src/engine/cosmic_dragon_engine/frame.rs` |
| (varies) | `src/cosmic_dragon_engine/runtime.rs` | `src/engine/cosmic_dragon_engine/runtime.rs` |

### docs/ (18 broken refs across 9 files)

| File | Broken ref | Status |
|------|-----------|--------|
| `docs/CENTRAL_CONTROL_RAINS_USAGE.md` | `./RAIN_DEPTH_AUDIT.md` | moved to `docs/archive/RAIN_DEPTH_AUDIT.md` |
| `docs/CENTRAL_CONTROL_RAINS_USAGE.md` | `docs/RAIN_DEPTH_AUDIT.md` | moved to `docs/archive/RAIN_DEPTH_AUDIT.md` |
| `docs/ENDURANCE.md` | `docs/audits/LTS_MATRIX_MIDSESSION_RETEST.md` | moved to `docs/archive/audits/` |
| `docs/ENDURANCE.md` | `scripts/endurance-summary.sh` | deleted (script no longer exists) |
| `docs/ENDURANCE.md` | `scripts/monitor-cosmostrix.sh` | deleted (script no longer exists) |
| `docs/LIVE_RELOAD_BEHAVIOR.md` | `docs/LTS_AUDIT_CONFIG_LIVE_RELOAD.md` | moved to archive or deleted |
| `docs/MAINTENANCE.md` | `audits/COSMIC_DRAGON_AUDIT.md` | path typo (missing `docs/` prefix) + moved to `docs/archive/audits/` |
| `docs/MAINTENANCE.md` | `docs/audits/COSMIC_DRAGON_AUDIT.md` | moved to `docs/archive/audits/` |
| `docs/PHILOSOPHY.md` | `SIMD_FEASIBILITY.md` | deleted or never created |
| `docs/PHILOSOPHY.md` | `docs/SIMD_FEASIBILITY.md` | deleted or never created |
| `docs/RULES.md` | `scripts/gen-density-presets.py` | deleted (density-map feature removed in v80.0.0-beta.2) |
| `docs/SECURITY_AUDIT.md` | `audits/SECURITY_VULNERABILITY_AUDIT.md` | path typo (missing `docs/` prefix) |
| `docs/SECURITY_AUDIT.md` | `docs/SIMD_FEASIBILITY.md` | deleted or never created |
| `docs/SECURITY_AUDIT.md` | `docs/STABILITY_AUDIT.md` | deleted or never created |
| `docs/SECURITY_AUDIT.md` | `docs/audits/SECURITY_VULNERABILITY_AUDIT.md` | moved to `docs/archive/audits/` |
| `docs/TERMINAL_LIFECYCLE_MATRIX.md` | `docs/audits/LTS_MATRIX_MIDSESSION_RETEST.md` | moved to `docs/archive/audits/` |
| `docs/VISUAL_IDENTITY.md` | `docs/RAIN_DEPTH_AUDIT.md` | moved to `docs/archive/RAIN_DEPTH_AUDIT.md` |
| `docs/VISUAL_IDENTITY.md` | `docs/research/PRESET_BATTLE_VERDICT.md` | deleted or never created |

### benchmark/ (3 broken refs)

| File | Broken ref | Status |
|------|-----------|--------|
| `benchmark/bench-labs/BENCH_LABS.md` | `docs/research/IPC_RESEARCH.md` | deleted or never created |
| `benchmark/bench-labs/PGO_AB_20260823.md` | `docs/research/IPC_RESEARCH.md` | deleted or never created |
| `benchmark/research/COMPETITOR_COMPARISON.md` | `benchmark/naive_matrix.py` | deleted (naive matrix script removed) |

### src/ (4 broken refs)

| File | Broken ref | Status |
|------|-----------|--------|
| `src/RULES.md` | `src/my_group/submodule_a.rs` | example path (may be intentional) |
| `src/cosmic_dragon_incubator/README.md` | `src/cosmic_dragon_egg_io_uring.rs` | moved or deleted |
| `src/cosmic_dragon_incubator/README.md` | `src/frame.rs` | moved to `src/engine/cosmic_dragon_engine/frame.rs` |
| `src/cosmic_dragon_incubator/README.md` | `src/runtime.rs` | moved to `src/engine/cosmic_dragon_engine/runtime.rs` |

## 2. Broken references in HISTORICAL records (leave as-is)

These are in CHANGELOG.md entries and research snapshots with explicit
"Historical research snapshot" headers. They cite paths that existed
at the time of writing — rewriting them would falsify history. The
`docs/archive/**` files are by-design historical records.

### CHANGELOG.md (14 broken refs — all historical)

All cite paths that existed at the time of the changelog entry:
`docs/audits/DEPS_AUDIT.md`, `scripts/gen-density-presets.py`,
`src/cosmic_dragon_engine/KEY.md`, `src/cosmic_dragon_engine/cloud/*.rs`,
`src/msg_fill_style/pulse.rs`, etc. These paths are correct for the
commit they describe — they have since moved or been deleted, but the
CHANGELOG is a historical record and should NOT be rewritten.

### docs/research/ (8 broken refs — all historical)

Files with "Historical research snapshot" headers:
- `docs/research/CHROMA_DRAGON_ENGINE_AUDIT.md` (6 refs to old flat
  `src/*.rs` paths — modules moved to `src/engine/chroma_dragon_engine/`)
- `docs/research/S_MASTER_V2_AUDIT.md` (1 ref to `src/validation.rs`)
- `docs/research/VISUAL_MODE_AUDIT.md` (3 refs to old paths)

### Stale count claims (historical — leave as-is)

- `~1500+ tests` in `CHANGELOG.md` (3 occurrences) + `KEY.md` (2
  occurrences) — accurate at time of writing (the `~` prefix means
  "approximately"). Current count is 2190 but historical entries
  should not be retroactively updated.
- `43 themes` in `CHANGELOG.md` — accurate at time of writing
  (EnergyZen was added later, bumping to 44).
- `Phase 9-B` in `CHANGELOG.md` — accurate at time of writing
  (engine has since progressed to Phase 9-D).

## 3. Fix strategy (when this backlog is picked up)

1. **Live docs (section 1)**: update each broken ref to point to the
   current location. For deleted files (scripts/naive_matrix.py, etc.),
   remove the reference or replace with a note ("deleted in vXX").
2. **Historical records (section 2)**: leave as-is. These are accurate
   for the time they were written.
3. **Run `scripts/docs-audit.py` after the cleanup pass** to verify
   the live-doc section 1 refs are resolved. The historical section 2
   refs will remain (the script flags them but they are intentional).
4. **Commit**: one micro-commit per file or per category (whichever
   is smaller), per the micro-commit-push-per-task owner rule.

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
