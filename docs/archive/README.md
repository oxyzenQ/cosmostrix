<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Document Archive

This directory holds historical project documents that are no longer
actively maintained but are preserved as a permanent record. They have
been moved out of the live `docs/` and `docs/research/` trees so the
active documentation set stays small and current.

## What lives here

| Subdirectory | Contents | Origin |
|--------------|----------|--------|
| `audits/` | Closed one-shot audit reports (A1-A5 zombie kills, B1-B4 optimizations, Z1-Z5 LTS audits, config/deps/security/build audits, dragon engine lock audit, visual LTS, killer features, output mastery, docs audit, perf regression). All findings have been actioned or are no longer live concerns. | Moved from `docs/audits/` (25 files, v60 archive round) |
| `research/` | Historical research docs whose conclusions have been absorbed into canonical docs or whose subject is closed (DRAGON_HUNT_V2, FLAGS_AUDIT_bench, IPC, MATRIX_1999, MATRIX_BOLD, MOUSE_EFFECTS, PLATFORM_EXPANSION, SELF_HEALING, THEME_CATALOG_SPLIT). | Moved from `docs/research/` (9 files, v60 archive round) |
| `CONFIG_SYNC/` | 7 phase reports (Phase 1, 2, 3, 4, 5, 5_FINAL, 6) from the closed CONFIG_SYNC audit. | Moved from `docs/research/CONFIG_SYNC_AUDIT_PHASE*.md` |
| `cosmic_dragon/` | Two design-exploration docs from the v13.3.0 Cosmic Dragon milestone. | Moved from `docs/COSMIC_DRAGON_EXPLORATION.md` + `docs/COSMIC_DRAGON_FINDINGS.md` |
| `specs/` | Historical design specifications for eliminated subsystems (`ATMOSPHERE_ENGINE.md`, `CINEMATIC_BREATHING.md`). | Moved from `docs/` |
| `*.md` (root) | Superseded/historical root docs: `LTS_AUDIT_*` (one-time audit results), `RAIN_DEPTH_AUDIT.md` (superseded by VISUAL_IDENTITY.md), `SIMD_FEASIBILITY.md` (rejected), `DESIGN_PROPOSAL_POWER_DRAGON_DENSITY.md` (design proposal), `STABILITY_AUDIT.md` (one-time audit), `CPU_USAGE_HONESTY.md` (historical), `SYSTEM_FEELING.md` (historical), `CHANGELOG_PRE_V13.md` (pre-v13 release history). | Moved from `docs/` root across the archive rounds |

## v60 Archive Round (Z-master-1X)

The v60.0.0-beta.1 archive round moved 43 docs from the live tree:
- 25 from `docs/audits/` → `docs/archive/audits/`
- 9 from `docs/` root → `docs/archive/` root
- 9 from `docs/research/` → `docs/archive/research/`

Active doc count reduced from ~100 to ~57 files, cutting the maintenance
surface nearly in half. No src/ code references were broken — all
moved docs were verified to have zero full-path `docs/` references
from `src/`. The two docs referenced by name only (FLAGS_AUDIT_bench
and MOUSE_EFFECTS) had their src/ references updated to point to the
new archive paths.
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
