// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Cosmic Dragon — v15+ Incubator
//!
//! This module is the **incubator namespace** for cosmostrix's v15+ features
//! and experimental subsystems. It exists to keep the stable flat-structured
//! engine (`src/cloud/`, `src/interactive/`, `src/chroma/`, etc.)
//! untouched while new Cosmic Dragon-era work lands in a clearly-branded home.
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
//! See `src/cosmic_dragon/README.md` for the full incubator policy. Summary:
//!
//! 1. **All new v15+ features go here.** Patches to existing stable modules
//!    stay in their flat `src/` paths — this namespace is for additive growth.
//! 2. **Mature modules can graduate.** Once a `cosmic_dragon/` module stabilizes,
//!    it may be promoted to the flat `src/` structure. The reverse never
//!    happens — stable code is never demoted back into the incubator.
//! 3. **Never break the stable engine.** Code in `cosmic_dragon/` must not mutate
//!    stable module internals. It may call stable APIs (`crate::cloud::*`,
//!    `crate::chroma::post::climate::*` for the post-FX shader, etc.) but
//!    cannot reach into private fields. (v30 2026-08-05: the historical
//!    `crate::atmosphere::*` reference was the eliminated atmosphere engine
//!    subsystem; the live reference is now `crate::chroma::post::climate`.)
//! 4. **Each subdirectory is self-contained.** A `cosmic_dragon/breath/` module
//!    does not import from `cosmic_dragon/scale/` — they are siblings, not a stack.
//!    Cross-subsystem dependencies flow through the stable engine's public
//!    API, not through `cosmic_dragon/` internals.
//!
//! ## History
//!
//! This namespace replaces the previous `src/cosmic_dragon_engine/` directory
//! (commit `4e2ebe7`), which was a pure re-export wrapper with zero callers
//! and was deleted in commit `46ba457` as dead code. The lesson: an
//! incubator namespace must hold *real new code*, not re-exports of existing
//! code. The first inhabitant is `egg/io_uring.rs` (moved from
//! `src/cosmic_dragon_egg_io_uring.rs`), which is a real `#[cfg(test)]` benchmark
//! with actual test callers.

pub(crate) mod egg;

#[cfg(test)]
#[path = "lock_tests.rs"]
mod lock_tests;
