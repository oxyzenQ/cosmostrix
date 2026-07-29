<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Cosmic Dragon Incubator — Policy

This directory is the **incubator namespace** for cosmostrix v15+ features
and experimental subsystems. It exists to keep the stable flat-structured
engine untouched while new Cosmic Dragon-era work lands in a clearly-branded home.

## 0. Engine Topology — Flat Forever (Non-Negotiable)

The Cosmic Dragon Diff-Based Rendering Engine is **three files at the
crate root**, and that is the final answer:

| File              |  LOC | Role                                                                  |
|-------------------|----:|-----------------------------------------------------------------------|
| `src/frame.rs`    | 388 | Differential frame buffer with double-buffered generation-based dirty tracking |
| `src/terminal.rs` | 974 | Raw-mode guard, alternate screen, RLE-batched ANSI diff pipeline, 64 KiB single-syscall flush |
| `src/runtime.rs`  |  91 | Runtime type vocabulary: `ColorScheme`, `ColorMode`, `BoldMode`      |

1,453 LOC total. Imported by **54 files** across `src/`. These are not a
subsystem waiting for a folder — they are the **substrate** every
rendering path stands on. Foundations do not get relocated; they get
maintained in place. See `docs/RENDER_ENGINE.md` §0 for the full
formal statement.

### The lesson is already paid for

`src/cosmic_dragon_engine/` was created (commit `4e2ebe7`) as a pure
re-export wrapper. It had zero callers. It was deleted as dead code in
commit `46ba457`. The invoice for that mistake is framed on the wall
of this README so the same mistake is never made at a larger scale.

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
3. **Additive growth goes to `cosmic_dragon/`.** This namespace is for
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
1,453 LOC of hard-won diff logic that you will have to reimplement from
scratch if you cannot read it. Keeping the layout flat makes the engine
easier to lift, study, and cite — and harder to mistake for a
reorg-friendly codebase that can be shuffled into a folder on a whim.

The flat layout is a deliberate architectural decision, not an
oversight waiting to be "fixed." Treat it that way.

## The Rule

1. **All new v15+ features go here.** Patches to existing stable modules
   stay in their flat `src/` paths — this namespace is for additive growth,
   not reorganization.

2. **Mature modules can graduate.** Once a `cosmic_dragon/` module stabilizes and
   is no longer experimental, it may be promoted to the flat `src/`
   structure. The reverse never happens — stable code is never demoted
   back into the incubator.

3. **Never break the stable engine.** Code in `cosmic_dragon/` must not mutate
   stable module internals. It may call stable APIs (`crate::cloud::*`,
   `crate::atmosphere_*`, `crate::interactive::*`, etc.) but cannot reach
   into private fields or break abstractions.

4. **Each subdirectory is self-contained.** A `cosmic_dragon/breath/` module
   does not import from `cosmic_dragon/scale/` — they are siblings, not a stack.
   Cross-subsystem dependencies flow through the stable engine's public
   API, not through `cosmic_dragon/` internals.

5. **Experimental code is `#[cfg(test)]`-gated when possible.** Production
   builds must not carry dead experimental code. If a module is ready for
   production, it is no longer experimental and should graduate (see rule 2).

## Anatomy

The Cosmic Dragon is organized by poetic anatomical function:

| Subdir    | Concern                              | Status        |
|-----------|--------------------------------------|---------------|
| `breath/` | Atmosphere engine extensions         | planned       |
| `heart/`  | Cloud simulation extensions          | planned       |
| `eye/`    | Interactive mode extensions          | planned       |
| `voice/`  | CLI / output extensions              | planned       |
| `scale/`  | Rendering primitive extensions       | planned       |
| `memory/` | Diagnostics / benchmark extensions   | planned       |
| `egg/`    | Experimental cosmic-dragon-egg benchmarks   | active        |

Subdirectories are created on-demand when the first module for that
anatomy lands. Empty subdirectories are NOT pre-created to avoid
dead-code warnings and `mod.rs` boilerplate.

## History

This namespace replaces the previous `src/cosmic_dragon_engine/` directory
(commit `4e2ebe7`), which was a pure re-export wrapper with zero callers
and was deleted in commit `46ba457` as dead code.

**The lesson:** an incubator namespace must hold *real new code*, not
re-exports of existing code. The first inhabitant of `cosmic_dragon/` is
`egg/io_uring.rs` (moved from `src/cosmic_dragon_egg_io_uring.rs`), which is a
real `#[cfg(test)]` benchmark with actual test callers.

## Migration Path

When a `cosmic_dragon/` module is ready to graduate:

1. Move the file from `src/cosmic_dragon/<anatomy>/<name>.rs` to `src/<name>.rs`.
2. Update `src/cosmic_dragon/<anatomy>/mod.rs` to remove the now-empty module
   declaration. If the anatomy directory becomes empty, delete its
   `mod.rs` and the directory itself.
3. Update `src/cosmic_dragon/mod.rs` if the anatomy subdir is removed.
4. Update any `crate::cosmic_dragon::<anatomy>::<name>` references to
   `crate::<name>`.
5. Run `cargo test --all` and `./scripts/build.sh check-all`.

The graduation is a one-way operation. Once a module lives in the flat
`src/` structure, it is part of the stable engine and follows the stable
engine's rules (1500-LOC cap, no breaking changes without a major version
bump, etc.).
