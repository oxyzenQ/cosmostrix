<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Dragon Incubator — Policy

This directory is the **incubator namespace** for cosmostrix v15+ features
and experimental subsystems. It exists to keep the stable flat-structured
engine untouched while new Dragon-era work lands in a clearly-branded home.

## The Rule

1. **All new v15+ features go here.** Patches to existing stable modules
   stay in their flat `src/` paths — this namespace is for additive growth,
   not reorganization.

2. **Mature modules can graduate.** Once a `dragon/` module stabilizes and
   is no longer experimental, it may be promoted to the flat `src/`
   structure. The reverse never happens — stable code is never demoted
   back into the incubator.

3. **Never break the stable engine.** Code in `dragon/` must not mutate
   stable module internals. It may call stable APIs (`crate::cloud::*`,
   `crate::atmosphere_*`, `crate::interactive::*`, etc.) but cannot reach
   into private fields or break abstractions.

4. **Each subdirectory is self-contained.** A `dragon/breath/` module
   does not import from `dragon/scale/` — they are siblings, not a stack.
   Cross-subsystem dependencies flow through the stable engine's public
   API, not through `dragon/` internals.

5. **Experimental code is `#[cfg(test)]`-gated when possible.** Production
   builds must not carry dead experimental code. If a module is ready for
   production, it is no longer experimental and should graduate (see rule 2).

## Anatomy

The Dragon is organized by poetic anatomical function:

| Subdir    | Concern                              | Status        |
|-----------|--------------------------------------|---------------|
| `breath/` | Atmosphere engine extensions         | planned       |
| `heart/`  | Cloud simulation extensions          | planned       |
| `eye/`    | Interactive mode extensions          | planned       |
| `voice/`  | CLI / output extensions              | planned       |
| `scale/`  | Rendering primitive extensions       | planned       |
| `memory/` | Diagnostics / benchmark extensions   | planned       |
| `egg/`    | Experimental dragon-egg benchmarks   | active        |

Subdirectories are created on-demand when the first module for that
anatomy lands. Empty subdirectories are NOT pre-created to avoid
dead-code warnings and `mod.rs` boilerplate.

## History

This namespace replaces the previous `src/dragon_engine/` directory
(commit `4e2ebe7`), which was a pure re-export wrapper with zero callers
and was deleted in commit `46ba457` as dead code.

**The lesson:** an incubator namespace must hold *real new code*, not
re-exports of existing code. The first inhabitant of `dragon/` is
`egg/io_uring.rs` (moved from `src/dragon_egg_io_uring.rs`), which is a
real `#[cfg(test)]` benchmark with actual test callers.

## Migration Path

When a `dragon/` module is ready to graduate:

1. Move the file from `src/dragon/<anatomy>/<name>.rs` to `src/<name>.rs`.
2. Update `src/dragon/<anatomy>/mod.rs` to remove the now-empty module
   declaration. If the anatomy directory becomes empty, delete its
   `mod.rs` and the directory itself.
3. Update `src/dragon/mod.rs` if the anatomy subdir is removed.
4. Update any `crate::dragon::<anatomy>::<name>` references to
   `crate::<name>`.
5. Run `cargo test --all` and `./scripts/build.sh check-all`.

The graduation is a one-way operation. Once a module lives in the flat
`src/` structure, it is part of the stable engine and follows the stable
engine's rules (1200-LOC cap, no breaking changes without a major version
bump, etc.).
