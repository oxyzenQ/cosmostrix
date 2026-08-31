// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Cosmic Dragon — v15+ Incubator
//!
//! This module is the **incubator namespace** for cosmostrix's v15+ features
//! and experimental subsystems. It exists to keep the substantive rendering
//! engine (`src/engine/cosmic_dragon_engine/`) and other stable modules untouched
//! while new Cosmic Dragon-era experimental work lands in a clearly-branded
//! home.
//!
//! ## Naming clarification
//!
//! The directory name `cosmic_dragon_incubator/` is intentionally distinct
//! from the substantive dragon engines (`cosmic_dragon_engine/`,
//! `chroma_dragon_engine/`, `crystal_dragon_engine/`). Those contain
//! thousands of LOC of production code; this directory contains experimental
//! / concluded work only (~200 LOC).
//!
//! The actual Cosmic Dragon rendering engine code lives at
//! `src/engine/cosmic_dragon_engine/` with 4 subsystems: `cloud/`, `frame.rs`,
//! `terminal/`, `runtime.rs`. These are re-exported at the crate root via
//! `pub(crate) use cosmic_dragon_engine::{cloud, frame, runtime, terminal};`
//! in main.rs so all `crate::cloud::Foo` / `crate::frame::Foo` /
//! `crate::runtime::Foo` / `crate::terminal::Foo` call sites continue to
//! resolve unchanged.
//!
//! ## Anatomy
//!
//! The Cosmic Dragon is organized by poetic anatomical function. Each planned
//! subdirectory maps to a subsystem concern. Modules are created on-demand
//! — empty subdirectories are NOT pre-created to avoid dead-code warnings.
//!
//! | Subdir    | Concern                              | Status        |
//! |-----------|--------------------------------------|---------------|
//! | `breath/` | Atmosphere engine extensions         | planned       |
//! | `heart/`  | Cloud simulation extensions          | planned       |
//! | `eye/`    | Interactive mode extensions          | planned       |
//! | `voice/`  | CLI / output extensions              | planned       |
//! | `scale/`  | Rendering primitive extensions       | planned       |
//! | `memory/` | Diagnostics / benchmark extensions   | planned       |
//! | `egg/`    | Experimental cosmic-dragon-egg benchmarks   | concluded (io_uring rejected; see `egg/io_uring_rejected.rs`) |
//!
//! ## Policy
//!
//! See `src/cosmic_dragon_incubator/README.md` for the full incubator policy. Summary:
//!
//! 1. **All new v15+ features go here.** Patches to existing stable modules
//!    stay in their flat `src/` paths — this namespace is for additive growth.
//! 2. **Mature modules can graduate.** Once a `cosmic_dragon_incubator/` module stabilizes,
//!    it may be promoted to the flat `src/` structure. The reverse never
//!    happens — stable code is never demoted back into the incubator.
//! 3. **Never break the stable engine.** Code in `cosmic_dragon_incubator/` must not mutate
//!    stable module internals. It may call stable APIs (`crate::cloud::*`,
//!    `crate::chroma_dragon_engine::post::climate::*` for the post-FX shader, etc.) but
//!    cannot reach into private fields. (v30 2026-08-05: the historical
//!    `crate::atmosphere::*` reference was the eliminated atmosphere engine
//!    subsystem; the live reference is now `crate::chroma_dragon_engine::post::climate`.)
//! 4. **Each subdirectory is self-contained.** A `cosmic_dragon_incubator/breath/` module
//!    does not import from `cosmic_dragon_incubator/scale/` — they are siblings, not a stack.
//!    Cross-subsystem dependencies flow through the stable engine's public
//!    API, not through `cosmic_dragon_incubator/` internals.
//!
//! ## History
//!
//! This namespace has cycled through three names as its role clarified:
//!
//! 1. **`src/engine/cosmic_dragon_engine/`** (early): a pure re-export wrapper with
//!    zero callers — deleted in commit `46ba457` as dead code.
//!
//! 2. **`src/cosmic_dragon/`** (commit `4e2ebe7`'s descendant): the
//!    incubator pattern was established with real code (`egg/io_uring_rejected.rs`).
//!    The shortened name dropped the misleading `_engine` suffix.
//!
//! 3. **`src/engine/cosmic_dragon_engine/`** (2026-08-19): renamed back to match the
//!    `crystal_dragon_engine/` and `chroma_dragon_engine/` naming convention
//!    for dragon-engine consistency. Owner mandate.
//!
//! 4. **`src/cosmic_dragon_incubator/`** (2026-08-19, same day): renamed
//!    again because the `_engine` suffix was still misleading. The
//!    substantive dragon engines (`chroma_dragon_engine/`,
//!    `crystal_dragon_engine/`) contain thousands of LOC of production
//!    code; this directory is an incubator that holds ~200 LOC of
//!    experimental / concluded work. The `_incubator` suffix makes the
//!    role unambiguous — it is NOT a peer engine.
//!
//! The lesson from the original deletion still applies: an incubator
//! namespace must hold *real new code*, not re-exports of existing code.
//! The first inhabitant is `egg/io_uring_rejected.rs` (moved from
//! `src/cosmic_dragon_egg_io_uring.rs`), which is a real `#[cfg(test)]`
//! benchmark with actual test callers.

pub(crate) mod egg;

// Tests now live in cosmic_dragon_incubator/tests/ subdir (Pattern C — dedicated tests/).
// Was previously a #[path] declaration (Pattern B).
#[cfg(test)]
mod tests;
