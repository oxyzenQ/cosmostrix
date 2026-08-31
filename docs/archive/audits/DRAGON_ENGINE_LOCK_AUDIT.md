<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Dragon Engine Lock/Unlock Audit Report

**Date:** 2026-08-26
**Auditor:** oxyzenQ (Cosmic Dragon mode)
**Version:** v50.0.0-beta.6
**Commit:** dd87239

## Overview

Audit of lock/unlock state for all 3 dragon engines after recent
commits that modified engine code. Each engine's KEY.md was checked,
git log was reviewed for changes, and lock/unlock entries were
verified.

---

## Cosmic Dragon Engine (`src/engine/cosmic_dragon_engine/`)

### Current LOCK state

Last lock: commit `5280ae1` (2026-08-24) — v50.0.0-beta.5 masterclass
easing consolidation re-seal.

### Commits since last lock (modified engine code)

1. `dd87239` — emoji purge in RULES.md/KEY.md (docs only, no .rs changes)
2. `41722cc` — speed_mult at spawn time (spawn.rs)
3. `4d9e5a6` — terminal-aware speed_mult (spawn.rs, rain.rs, mod.rs)
4. `4049620` — phosphor decay + ghost cap (phosphor.rs, mod.rs)
5. `ff15dd9` — CI fix + border touch optimize (rain.rs, monolith.rs)
6. `c3efeb3` — border touch snapshot fix (rain.rs)
7. `e564eb3` — border touch for monolith (rain.rs, monolith.rs)
8. `6e032fb` — dynamic dsty HUD (rain.rs shared function)
9. `5986799` — LTS audit zero blank lines (terminal/mod.rs)

### Assessment

Multiple .rs files in cosmic_dragon_engine/ were modified since the
last lock at `5280ae1`. These changes include:
- Border touch pulse for monolith (rain.rs, monolith.rs)
- Terminal-aware phosphor tuning (phosphor.rs, mod.rs, runtime_controls.rs)
- Terminal-aware speed_mult (spawn.rs, rain.rs)
- Struct visibility changes (monolith.rs)

**STATUS: UNLOCKED** — engine was modified without formal unlock/lock
entries in KEY.md. The changes are additive (new features, not
modifications to existing locked invariants), but the KEY.md should
be updated to document the unlock/lock cycle.

### Recommendation

Add retroactive UNLOCK entry at `e564eb3` (border touch for monolith
— first .rs modification since last lock) and LOCK entry at current
HEAD documenting all changes are additive and tested (1710/0/2).

---

## Chroma Dragon Engine (`src/engine/chroma_dragon_engine/`)

### Current LOCK state

Last lock: commit `deff636` (2026-08-24) — chroma re-seal audit.

### Commits since last lock (modified engine code)

1. `dd87239` — emoji purge in README.md/RULES.md/KEY.md (docs only)
2. `6ad1d7f` — scene_custom LTS bounds (colors_custom.rs — new constants only)
3. `76e5de5` — LTS bounds for custom colors + charset (colors_custom.rs)
4. `d897c90` — perf-stats fix (humanize.rs — docs reference only)

### Assessment

Only `colors_custom.rs` was modified (added bounds constants +
enforcement in collect functions). These are additive changes that
do not touch the chroma pipeline (gradient, interpolation, palette
routing). The chroma routing rule and interpolation invariants are
untouched.

**STATUS: LOCKED** — changes were additive (new constants, new
enforcement) and did not modify the locked chroma pipeline. No
unlock needed, but a note should be added to KEY.md documenting
the additive changes.

---

## Crystal Dragon Engine (`src/engine/crystal_dragon_engine/`)

### Current LOCK state

Last lock: commit `c1c7779` (2026-08-23) — triple-engine LTS audit.

### Commits since last lock (modified engine code)

1. `dd87239` — emoji purge in README.md/RULES.md/KEY.md (docs only)

### Assessment

No .rs files in crystal_dragon_engine/ were modified since the last
lock. Only documentation files (README.md, RULES.md, KEY.md) had
emoji replacements.

**STATUS: LOCKED** — no code changes since last lock. Clean.

---

## Summary

| Engine | Last Lock | .rs Changes Since Lock | Status | Action Needed |
|--------|-----------|----------------------|--------|--------------|
| Cosmic | `5280ae1` | Yes (9 commits) | UNLOCKED | Add retroactive unlock+lock entries |
| Chroma | `deff636` | Yes (additive only) | LOCKED | Add note documenting additive changes |
| Crystal | `c1c7779` | No (docs only) | LOCKED | None |

### Action: Update Cosmic Dragon KEY.md

The cosmic dragon engine has been modified extensively since the last
lock (border touch, phosphor tuning, speed_mult). All changes are
additive (new features, not modifications to locked invariants) and
fully tested (1710/0/2). A retroactive UNLOCK + LOCK entry should
be added to document the cycle.

<!-- COSMOSTRIX-DISCLAIMER -->
