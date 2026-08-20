<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Cosmic Dragon Incubator — Policy

This directory is the **incubator namespace** for cosmostrix v15+ features
and experimental subsystems. It exists to keep the substantive rendering
engine (`cosmic_dragon_engine/`) and other stable modules untouched while
new Cosmic Dragon-era experimental work lands in a clearly-branded home.

## Naming clarification

The directory name `cosmic_dragon_incubator/` is intentionally distinct
from the substantive dragon engines (`cosmic_dragon_engine/`,
`chroma_dragon_engine/`, `crystal_dragon_engine/`). Those contain
thousands of LOC of production code; this directory contains experimental
/ concluded work only (~200 LOC).

The actual Cosmic Dragon rendering engine code lives at
`src/cosmic_dragon_engine/` with 4 subsystems:

- `src/cosmic_dragon_engine/cloud/` (rain simulation, monolith, render
  pipeline, ecosystem, phosphor, ghost events)
- `src/cosmic_dragon_engine/frame.rs` (frame buffer + dirty tracking)
- `src/cosmic_dragon_engine/terminal/` (ANSI stream output, draw, last_frame,
  `/dev/tty` fallback)
- `src/cosmic_dragon_engine/runtime.rs` (color mode detection, color
  pipeline enum)

These 4 subsystems are re-exported at the crate root via
`pub(crate) use cosmic_dragon_engine::{cloud, frame, runtime, terminal};`
in main.rs, so all `crate::cloud::Foo`, `crate::frame::Foo`,
`crate::runtime::Foo`, and `crate::terminal::Foo` call sites continue
to resolve unchanged. The substantive code lives in
`cosmic_dragon_engine/`; this incubator directory is for v15+
experimental work only.

## 0. Engine Topology — Substantive Engine Now Consolidated

The Cosmic Dragon Diff-Based Rendering Engine is consolidated under
`src/cosmic_dragon_engine/` with 4 subsystems (cloud, frame, terminal,
runtime). The previous "Flat Forever" policy (modules at crate root)
was overturned by owner mandate on 2026-08-19 to make the engine's
substantive status explicit and align directory naming with the other
dragon engines.

| Subsystem                                | LOC    | Role                                                                  |
|------------------------------------------|-------:|-----------------------------------------------------------------------|
| `cosmic_dragon_engine/cloud/`            | ~6,000 | Rain simulation, monolith, render pipeline, ecosystem, phosphor, ghost events |
| `cosmic_dragon_engine/frame.rs`          |   404  | Differential frame buffer with double-buffered generation-based dirty tracking |
| `cosmic_dragon_engine/terminal/`         | ~1,500 | Raw-mode guard, alternate screen, RLE-batched ANSI diff pipeline, 256 KiB single-syscall flush, `/dev/tty` fallback |
| `cosmic_dragon_engine/runtime.rs`        |   312  | Runtime type vocabulary: `ColorScheme`, `ColorMode`, `BoldMode`, `ColorPipeline` |

The re-export at crate root keeps all 224 existing call sites
(`crate::cloud::*` × 54, `crate::frame::*` × 44, `crate::terminal::*` × 17,
`crate::runtime::*` × 109) working without code edits. See
`docs/RENDER_ENGINE.md` for the full architectural statement.

### The lesson is already paid for

The earliest `src/cosmic_dragon_engine/` (commit `4e2ebe7`) was a pure
re-export wrapper with zero callers — deleted as dead code in commit
`46ba457`. That early failure was because the directory held only
re-exports, not real code. The current `src/cosmic_dragon_engine/`
holds the actual rendering engine code (~8K LOC across 4 subsystems) —
NOT re-exports.

> An incubator namespace must hold *real new code*, not re-exports of
> existing code. The substantive engine, by contrast, may relocate
> real code into a branded directory — provided the move is transparent
> to call sites via re-export.

### Hard policy

1. **`frame`, `terminal`, `runtime`, and `cloud` stay under
   `cosmic_dragon_engine/`.** Forever. They are the substantive rendering
   engine, branded under the same naming convention as the other dragon
   engines (`chroma_dragon_engine/`, `crystal_dragon_engine/`).
2. **Patches land in place.** New rendering optimizations extend the
   existing files (under the 1,500-LOC cap, splitting if needed) —
   they do not branch into a new namespace.
3. **Additive experimental growth goes to `cosmic_dragon_incubator/`.**
   This namespace is for new v15+ features. The substantive engine is
   not v15+ — it is the foundation.
4. **A reorganization commit will be rejected at review.** If you find
   yourself reaching for `git mv src/cosmic_dragon_engine/cloud
   src/engine/cloud`, stop. Read commit `46ba457`. Read this section.
   Read `docs/RENDER_ENGINE.md`. Open a doc issue instead.

### For competitors reading this repo

The brand "Cosmic Dragon Diff-Based Rendering Engine" lives in the
docs, the benchmark output, the changelog, and the code's behavior —
and now also in the directory name. The folder IS a substantive
declaration, not the moat. The moat is the hard-won diff logic that
you will have to reimplement from scratch if you cannot read it.

## The Rule

1. **All new v15+ features go here (incubator).** Patches to existing
   stable modules stay in their substantive homes (`cosmic_dragon_engine/`,
   `chroma_dragon_engine/`, etc.) — this namespace is for additive
   experimental growth, not reorganization.

2. **Mature modules can graduate.** Once a `cosmic_dragon_incubator/`
   module stabilizes and is no longer experimental, it may be promoted
   to a substantive location (e.g., `cosmic_dragon_engine/<subsystem>/`
   if it's rendering-related, or another dragon engine if it's coloring
   or ambient). The reverse never happens — stable code is never
   demoted back into the incubator.

3. **Never break the stable engine.** Code in `cosmic_dragon_incubator/`
   must not mutate stable module internals. It may call stable APIs
   (`crate::cloud::*`, `crate::chroma_dragon_engine::post::climate` for
   the post-FX shader, `crate::interactive::*`, etc.) but cannot reach
   into private fields or break abstractions.

4. **Each subdirectory is self-contained.** A
   `cosmic_dragon_incubator/breath/` module does not import from
   `cosmic_dragon_incubator/scale/` — they are siblings, not a stack.
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
   zero callers — deleted in commit `46ba457` as dead code. The
   substantive rendering engine code at that time lived flat at the
   crate root (`src/cloud/`, `src/frame.rs`, `src/terminal/`,
   `src/runtime.rs`).

2. **`src/cosmic_dragon/`** (commit `4e2ebe7`'s descendant): the
   incubator pattern was established with real code (`egg/io_uring_rejected.rs`).
   The shortened name dropped the misleading `_engine` suffix.

3. **`src/cosmic_dragon_engine/`** (2026-08-19, first attempt): renamed back
   to match the `crystal_dragon_engine/` and `chroma_dragon_engine/` naming
   convention for dragon-engine consistency. But this name was still
   misleading because the substantive rendering engine was still flat at
   the crate root, while the `cosmic_dragon_engine/` directory held only
   ~200 LOC of experimental work.

4. **`src/cosmic_dragon_incubator/`** (2026-08-19): renamed because the
   `_engine` suffix was misleading. The substantive dragon engines
   (`chroma_dragon_engine/`, `crystal_dragon_engine/`) contain thousands
   of LOC of production code; this directory is an incubator that holds
   ~200 LOC of experimental / concluded work. The `_incubator` suffix
   makes the role unambiguous.

5. **`src/cosmic_dragon_engine/`** (2026-08-19, second attempt): recreated
   — this time as the substantive rendering engine home. The 4 modules
   (`cloud/`, `frame.rs`, `terminal/`, `runtime.rs`) were relocated
   from the crate root into `cosmic_dragon_engine/` per owner mandate.
   The re-export pattern (`pub(crate) use cosmic_dragon_engine::{cloud,
   frame, runtime, terminal};` in main.rs) keeps all 224 call sites
   working without code edits. The incubator stayed at
   `cosmic_dragon_incubator/` (step 4 above).

**The lesson:** an incubator namespace must hold *real new code*, not
re-exports of existing code. The substantive engine, by contrast, may
relocate real code into a branded directory — provided the move is
transparent to call sites via re-export. The first inhabitant of the
incubator is `egg/io_uring_rejected.rs` (moved from
`src/cosmic_dragon_egg_io_uring.rs`), which is a real `#[cfg(test)]`
benchmark with actual test callers.

## Migration Path

When a `cosmic_dragon_incubator/` module is ready to graduate:

1. Move the file from
   `src/cosmic_dragon_incubator/<anatomy>/<name>.rs` to a substantive
   location (e.g., `src/cosmic_dragon_engine/<subsystem>/<name>.rs` if
   rendering-related, or another dragon engine if coloring/ambient).
2. Update `src/cosmic_dragon_incubator/<anatomy>/mod.rs` to remove the
   now-empty module declaration. If the anatomy directory becomes empty,
   delete its `mod.rs` and the directory itself.
3. Update `src/cosmic_dragon_incubator/mod.rs` if the anatomy subdir is
   removed.
4. Update any `crate::cosmic_dragon_incubator::<anatomy>::<name>`
   references to the new path.
5. Run `cargo test --all` and `./scripts/build.sh check-all`.

The graduation is a one-way operation. Once a module lives in a
substantive engine directory, it is part of the stable engine and
follows the stable engine's rules (1500-LOC cap, no breaking changes
without a major version bump, etc.).
