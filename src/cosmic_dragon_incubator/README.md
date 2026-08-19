<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Cosmic Dragon Incubator — Policy

This directory is the **incubator namespace** for cosmostrix v15+ features
and experimental subsystems. It exists to keep the stable flat-structured
engine untouched while new Cosmic Dragon-era work lands in a clearly-branded home.

## Naming clarification

The directory name `cosmic_dragon_incubator/` is intentionally distinct
from the substantive dragon engines (`chroma_dragon_engine/`,
`crystal_dragon_engine/`). Those contain thousands of LOC of production
code; this directory contains experimental / concluded work only (~200 LOC).

The actual Cosmic Dragon rendering engine code lives in:
- `src/cloud/` (rain simulation, monolith, render pipeline, ecosystem)
- `src/frame.rs` (frame buffer + dirty tracking)
- `src/terminal/` (ANSI stream output, draw, last_frame)
- `src/runtime.rs` (color mode detection, color pipeline enum)

## 0. Engine Topology — Flat Forever (Non-Negotiable)

The Cosmic Dragon Diff-Based Rendering Engine is **three modules at the
crate root**, and that is the final answer:

| File                  |   LOC | Role                                                                  |
|-----------------------|------:|-----------------------------------------------------------------------|
| `src/frame.rs`        |  368 | Differential frame buffer with double-buffered generation-based dirty tracking |
| `src/terminal/mod.rs`| ~1,332 | Raw-mode guard, alternate screen, RLE-batched ANSI diff pipeline, 256 KiB single-syscall flush |
| `src/terminal/terminal_tty.rs` | 201 | /dev/tty fallback helpers (extracted from terminal.rs in v30 to keep it under the 1500-LOC guard) |
| `src/runtime.rs`      |   ~280 | Runtime type vocabulary: `ColorScheme`, `ColorMode`, `BoldMode`, `ColorPipeline` |

These are not a subsystem waiting for a folder — they are the
**substrate** every rendering path stands on. Foundations do not get
relocated; they get maintained in place. See `docs/RENDER_ENGINE.md` §0
for the full formal statement.

### The lesson is already paid for

The earliest `src/cosmic_dragon_engine/` was created (commit `4e2ebe7`)
as a pure re-export wrapper. It had zero callers. It was deleted as dead
code in commit `46ba457`. The invoice for that mistake is framed on the
wall of this README so the same mistake is never made at a larger scale.

> An incubator namespace must hold *real new code*, not re-exports of
> existing code.

### Hard policy

1. **`frame`, `terminal`, and `runtime` stay at the crate root.**
   Forever. No `engine/` folder. No `render/` folder. No `core/`
   folder. They are crate-level primitives, like `crossterm::event` or
   `std::io`.
2. **Patches land in place.** New rendering optimizations extend the
   existing files (under the 1,500-LOC cap, splitting if needed) —
   they do not branch into a new namespace.
3. **Additive growth goes to `cosmic_dragon_incubator/`.** This namespace is for
   new v15+ features. The flat engine is not v15+ — it is the
   foundation. Foundations are not relocated; they are maintained.
4. **A reorganization commit will be rejected at review.** If you find
   yourself reaching for `git mv src/frame.rs src/engine/frame.rs`,
   stop. Read commit `46ba457`. Read this section. Read
   `docs/RENDER_ENGINE.md` §0. Open a doc issue instead.

### For competitors reading this repo

The brand "Cosmic Dragon Diff-Based Rendering Engine" lives in the
docs, the benchmark output, the changelog, and the code's behavior —
not in the directory tree. The folder is not the moat. The moat is the
hard-won diff logic that you will have to reimplement from
scratch if you cannot read it. Keeping the layout flat makes the engine
easier to lift, study, and cite — and harder to mistake for a
reorg-friendly codebase that can be shuffled into a folder on a whim.

The flat layout is a deliberate architectural decision, not an
oversight waiting to be "fixed." Treat it that way.

## The Rule

1. **All new v15+ features go here.** Patches to existing stable modules
   stay in their flat `src/` paths — this namespace is for additive growth,
   not reorganization.

2. **Mature modules can graduate.** Once a `cosmic_dragon_incubator/` module stabilizes and
   is no longer experimental, it may be promoted to the flat `src/`
   structure. The reverse never happens — stable code is never demoted
   back into the incubator.

3. **Never break the stable engine.** Code in `cosmic_dragon_incubator/` must not mutate
   stable module internals. It may call stable APIs (`crate::cloud::*`,
   `crate::chroma_dragon_engine::post::climate` for the post-FX shader,
   `crate::interactive::*`, etc.) but cannot reach into private fields or
   break abstractions. (v30 2026-08-05: the historical `crate::atmosphere_*`
   reference was the eliminated atmosphere engine subsystem; the live
   reference is now `crate::chroma_dragon_engine::post::climate`.)

4. **Each subdirectory is self-contained.** A `cosmic_dragon_incubator/breath/` module
   does not import from `cosmic_dragon_incubator/scale/` — they are siblings, not a stack.
   Cross-subsystem dependencies flow through the stable engine's public
   API, not through `cosmic_dragon_incubator/` internals.

5. **Experimental code is `#[cfg(test)]`-gated when possible.** Production
   builds must not carry dead experimental code. If a module is ready for
   production, it is no longer experimental and should graduate (see rule 2).

## Anatomy

The Cosmic Dragon incubator is organized by poetic anatomical function:

| Subdir    | Concern                              | Status        |
|-----------|--------------------------------------|---------------|
| `breath/` | Atmosphere engine extensions         | planned       |
| `heart/`  | Cloud simulation extensions          | planned       |
| `eye/`    | Interactive mode extensions          | planned       |
| `voice/`  | CLI / output extensions              | planned       |
| `scale/`  | Rendering primitive extensions       | planned       |
| `memory/` | Diagnostics / benchmark extensions   | planned       |
| `egg/`    | Experimental cosmic-dragon-egg benchmarks   | concluded (io_uring rejected; see `egg/io_uring_rejected.rs` doc)        |

Subdirectories are created on-demand when the first module for that
anatomy lands. Empty subdirectories are NOT pre-created to avoid
dead-code warnings and `mod.rs` boilerplate.

## History

This namespace has cycled through names as its role clarified:

1. **`src/cosmic_dragon_engine/`** (early): a pure re-export wrapper with
   zero callers — deleted in commit `46ba457` as dead code.

2. **`src/cosmic_dragon/`** (commit `4e2ebe7`'s descendant): the
   incubator pattern was established with real code (`egg/io_uring_rejected.rs`).
   The shortened name dropped the misleading `_engine` suffix.

3. **`src/cosmic_dragon_engine/`** (2026-08-19): renamed back to match the
   `crystal_dragon_engine/` and `chroma_dragon_engine/` naming convention
   for dragon-engine consistency.

4. **`src/cosmic_dragon_incubator/`** (2026-08-19, same day): renamed
   again because the `_engine` suffix was still misleading. The
   substantive dragon engines (`chroma_dragon_engine/`,
   `crystal_dragon_engine/`) contain thousands of LOC of production
   code; this directory is an incubator that holds ~200 LOC of
   experimental / concluded work. The `_incubator` suffix makes the
   role unambiguous — it is NOT a peer engine.

**The lesson:** an incubator namespace must hold *real new code*, not
re-exports of existing code. The first inhabitant of this directory is
`egg/io_uring_rejected.rs` (moved from `src/cosmic_dragon_egg_io_uring.rs`), which is a
real `#[cfg(test)]` benchmark with actual test callers.

## Migration Path

When a `cosmic_dragon_incubator/` module is ready to graduate:

1. Move the file from `src/cosmic_dragon_incubator/<anatomy>/<name>.rs` to `src/<name>.rs`.
2. Update `src/cosmic_dragon_incubator/<anatomy>/mod.rs` to remove the now-empty module
   declaration. If the anatomy directory becomes empty, delete its
   `mod.rs` and the directory itself.
3. Update `src/cosmic_dragon_incubator/mod.rs` if the anatomy subdir is removed.
4. Update any `crate::cosmic_dragon_incubator::<anatomy>::<name>` references to
   `crate::<name>`.
5. Run `cargo test --all` and `./scripts/build.sh check-all`.

The graduation is a one-way operation. Once a module lives in the flat
`src/` structure, it is part of the stable engine and follows the stable
engine's rules (1500-LOC cap, no breaking changes without a major version
bump, etc.).
