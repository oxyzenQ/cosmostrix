<!-- SPDX-License-Identifier: GPL-3.0-only -->

# v5.0.0 Nightfall Plan

## Status

Release Candidate Ready — Phase 5 complete. Awaiting owner testing and approval for Phase 6.

## Theme

**Cinematic UX + Product Identity Release.** Nightfall makes Cosmostrix
feel more polished, intentional, discoverable, and product-grade without
rewriting the renderer or making unrealistic performance promises.

## Goals

- Improve the user-facing polish of preset and profile discoverability so
  that new users can quickly understand available atmospheres and how to
  activate them.
- Establish a cinematic breathing language that documents how visual
  transitions and atmospheric effects feel and behave, providing a contract
  that future development can reference.
- Polish help text and configuration UX so that error messages, `--help`
  output, and configuration diagnostics are clear, actionable, and
  consistent with product-grade expectations.
- Strengthen the product identity of Cosmostrix as a deliberate, crafted
  cinematic terminal experience rather than a raw rendering engine with
  incidental visual output.

## Non-goals

- **No renderer hot-path rewrite.** The rendering pipeline remains
  single-threaded. No changes to frame rendering logic, cell update
  strategies, or draw call ordering.
- **No benchmark output field changes.** The `--benchmark` JSON output
  retains the same field names, types, and semantics as v4.9.0. No fields
  are added, removed, or renamed.
- **No 50k FPS promise.** Cosmostrix does not guarantee a specific frame
  rate target. Benchmarks report observed performance honestly. Any
  reference to target FPS is explicitly marked as aspirational or
  experimental.
- **No Android implementation.** Cosmostrix Live (an Android live
  wallpaper sibling product) is a future project with its own repository.
  No Android code, Gradle files, or mobile-specific abstractions are added
  to this repository in v5.0.0.
- **No changes to terminal cleanup behavior.** The terminal lifecycle
  contract documented in `docs/TERMINAL_LIFECYCLE_MATRIX.md` remains
  authoritative. Normal exit cleanup remains non-destructive.
  `--reset-terminal` remains the only destructive recovery path.

## Release Safety

All release safety mechanisms established in v4.9.0 The Wolf remain in
full effect and are inherited by v5.0.0:

- The 11-gate release guard in `docs/RELEASE_GUARD.md` must pass before
  any v5.0.0 tag or release.
- Terminal lifecycle contract remains authoritative. No path in the
  14-path matrix is weakened or removed.
- Terminal writer remains single-owner. No parallel terminal writes are
  introduced.
- Benchmark honesty is preserved. No fake benchmark progress, no
  cherry-picked runs, no omitted metrics.
- Release benchmark report automation via
  `scripts/release-benchmark-report.sh` remains mandatory. A fresh
  v5.0.0 benchmark section in `benchmark/README.md` must be committed
  before the v5.0.0 release tag is created.
- The `--doctor` command continues to report terminal lifecycle contract
  fields. The diagnostic output is extended or polished but never
  weakened.

## User Experience Targets

- **Discoverability:** New users should be able to find available
  atmospheres, understand what each preset does, and activate them with a
  single CLI flag or config line without reading extensive documentation.
- **Help clarity:** `--help` output should be scannable, grouped
  logically, and avoid jargon that requires reading source code to
  understand.
- **Error quality:** Configuration errors should state what is wrong,
  why it matters, and what the user can do to fix it. Vague or misleading
  error messages are considered bugs.
- **Diagnostic trust:** `--doctor` and `--info` output should remain
  honest and actionable. No fields are fabricated or aspirational.

## Preset/Profile Direction

The profile ecosystem built in v4.7.0 and the atmosphere presets
introduced in v4.6.0 form the foundation. Nightfall focuses on making
these systems more discoverable and self-documenting:

- Preset descriptions should be human-readable and convey the visual
  intent, not just the technical parameters.
- `--list-profiles` should clearly indicate which profiles are
  atmosphere-related, which are configuration templates, and which are
  community examples.
- Configuration validation should guide users toward valid presets when
  they provide an unrecognized name.

## Cinematic Breathing Direction

Cosmostrix renders a cinematic visual experience in the terminal.
"Cinematic breathing" refers to the intentional rhythm and pacing of
visual transitions, atmospheric effects, and scene changes. Nightfall
begins documenting this as a formal concept:

- Scene transitions should feel smooth and deliberate, not jarring or
  abrupt.
- Atmospheric intensity changes should be gradual enough to perceive as
  breathing rather than flickering.
- The language used to describe these effects in documentation and help
  text should reinforce the cinematic identity.

This direction does not require renderer hot-path changes. It is
primarily a documentation, naming, and UX language effort that may
inform parameter defaults or pacing constants in future releases.

## Android / Cosmostrix Live Boundary

Cosmostrix Live is a conceptual future product: an Android live
wallpaper that delivers a native cinematic visual experience using a
purpose-built renderer, not a terminal emulator wrapper. Key boundaries:

- **Separate repository.** Cosmostrix Live will have its own codebase,
  its own release cadence, and its own versioning scheme. It is not a
  module within the Cosmostrix CLI repository.
- **Native renderer.** Cosmostrix Live would use an Android-native
  rendering surface (e.g., `WallpaperService` with `Canvas` or Vulkan),
  not a terminal emulation layer.
- **No Android code in this repo.** No Kotlin, Java, Gradle, or Android
  manifest files are added to the Cosmostrix CLI repository for v5.0.0.
- **Business model is separate.** Any discussion of one-time lifetime
  unlock, Play Store distribution, or mobile-specific licensing belongs
  to the Cosmostrix Live product, not the CLI.
- **Play Store is future.** The CLI remains the primary distribution
  channel. Mobile distribution is not part of the v5.0.0 roadmap.

## Phase Plan

| Phase | Description | Status |
|-------|-------------|--------|
| Phase 1 | Roadmap + product identity foundation | complete |
| Phase 2 | Preset/profile discoverability polish | complete |
| Phase 3 | Cinematic breathing language + docs contract (`docs/CINEMATIC_BREATHING.md`) | complete |
| Phase 4 | Help/config UX polish | complete |
| Phase 5 | Release candidate prep | complete |
| Phase 6 | Signed tag / release / AUR | pending owner approval |