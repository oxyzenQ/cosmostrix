// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Dragon — v15+ Incubator
//!
//! This module is the **incubator namespace** for cosmostrix's v15+ features
//! and experimental subsystems. It exists to keep the stable flat-structured
//! engine (`src/cloud/`, `src/interactive/`, `src/atmosphere_*.rs`, etc.)
//! untouched while new Dragon-era work lands in a clearly-branded home.
//!
//! ## Anatomy
//!
//! The Dragon is organized by poetic anatomical function. Each planned
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
//! | `egg/`    | Experimental dragon-egg benchmarks   | active        |
//!
//! ## Policy
//!
//! See `src/dragon/README.md` for the full incubator policy. Summary:
//!
//! 1. **All new v15+ features go here.** Patches to existing stable modules
//!    stay in their flat `src/` paths — this namespace is for additive growth.
//! 2. **Mature modules can graduate.** Once a `dragon/` module stabilizes,
//!    it may be promoted to the flat `src/` structure. The reverse never
//!    happens — stable code is never demoted back into the incubator.
//! 3. **Never break the stable engine.** Code in `dragon/` must not mutate
//!    stable module internals. It may call stable APIs (`crate::cloud::*`,
//!    `crate::atmosphere::*`, etc.) but cannot reach into private fields.
//! 4. **Each subdirectory is self-contained.** A `dragon/breath/` module
//!    does not import from `dragon/scale/` — they are siblings, not a stack.
//!    Cross-subsystem dependencies flow through the stable engine's public
//!    API, not through `dragon/` internals.
//!
//! ## History
//!
//! This namespace replaces the previous `src/dragon_engine/` directory
//! (commit `4e2ebe7`), which was a pure re-export wrapper with zero callers
//! and was deleted in commit `46ba457` as dead code. The lesson: an
//! incubator namespace must hold *real new code*, not re-exports of existing
//! code. The first inhabitant is `egg/io_uring.rs` (moved from
//! `src/dragon_egg_io_uring.rs`), which is a real `#[cfg(test)]` benchmark
//! with actual test callers.

pub mod egg;
