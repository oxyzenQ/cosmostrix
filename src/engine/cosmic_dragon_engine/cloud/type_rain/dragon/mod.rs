// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Dragon serpentine-body rain style + extracted state-machine helpers.
//!
//! Layout (mirrors the monolith family split from NIGHT-hunter-10):
//! - `dragon.rs` - state machine, chain solver, diff cleanup.
//! - `dragon_helpers.rs` - glyph pool picks, brightness zoning, noise
//!   rolls, single-cell renderer.

pub(crate) mod dragon;
pub(crate) mod dragon_helpers;

// Re-export the public surface so `crate::cloud::dragon::*` (the
// backward-compat alias in `cloud/mod.rs`) and `super::dragon::*`
// inside `cloud::*` keep resolving after the directory move. Some of
// these are only consumed by tests under `test/engine/.../cloud/tests/`,
// so `#[allow(unused_imports)]` keeps the bin build quiet without
// gating the re-export behind `#[cfg(test)]` (which would break the
// crate-internal `use` paths the bin target reaches via the alias).
#[allow(unused_imports)]
pub(crate) use dragon::{DragonRain, DragonRandom, DragonSpawnParams, DragonState, DragonStep};
