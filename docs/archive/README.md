# Document Archive

This directory holds historical project documents that are no longer
actively maintained but are preserved as a permanent record. They have
been moved out of the live `docs/` and `docs/research/` trees so the
active documentation set stays small and current.

## What lives here

| Subdirectory | Contents | Origin |
|--------------|----------|--------|
| `CONFIG_SYNC/` | 7 phase reports (Phase 1, 2, 3, 4, 5, 5_FINAL, 6) from the closed CONFIG_SYNC audit. Closed phase reports with zero live consumers in `src/`. | Moved from `docs/research/CONFIG_SYNC_AUDIT_PHASE*.md` |
| `cosmic_dragon/` | Two design-exploration docs from the v13.3.0 Cosmic Dragon milestone. Conclusions have been folded into `docs/PHILOSOPHY.md` and `docs/PERFORMANCE_ACROSS_SCALES.md`. | Moved from `docs/COSMIC_DRAGON_EXPLORATION.md` + `docs/COSMIC_DRAGON_FINDINGS.md` |
| `audits/` | Closed one-shot audit reports (`UNSAFE_SOUNDNESS_AUDIT.md`, `FLAGS_AUDIT_dead_weight.md`, `ATMOSPHERE_SUBSYSTEM_ARCHIVAL.md`) whose findings have either been actioned or are no longer live concerns. | Moved from `docs/research/` (first two) / newly created 2026-08-05 (third) |

## Why these were archived

Each archived file was audited in `docs/research/DRAGON_HUNT_V2_AUDIT.md`
(Tier B, items 18-21) and met one or more of these criteria:

- The phase or audit it documented has been formally closed.
- Its conclusions have been absorbed into a newer, canonical doc.
- No live `src/` or `scripts/` code references it.
- It references version markers that have since been superseded
  (e.g. `v13.3.0` measurements in a `v30` codebase).

## How to read these files

Archive files are kept verbatim — no content edits were made on move.
Cross-reference links inside an archive file may point to other archive
files (still valid) or to live docs (also still valid). If a link target
has itself been archived, look in the corresponding subdirectory here.

## Test-lock tripwires

The live doc tree is enforced by `src/docs_tests/`. None of the archived
files are referenced by any test tripwire — that was verified before the
move. The only doc-related tripwires still in force are:

- `src/docs_tests/metadata.rs` — `CHANGELOG.md` content anchors
  (`v4.0.0`, `v3.9.0`, `"568 deterministic tests"`).
- `src/docs_tests/assets.rs` — current-major demo GIF/PNG presence.
- `src/validation.rs:74` — error message names `FLAGS_AUDIT_bench-frames_chars_bold.md`
  (note: that file is **not** archived; only its sibling
  `FLAGS_AUDIT_dead_weight.md` is).
