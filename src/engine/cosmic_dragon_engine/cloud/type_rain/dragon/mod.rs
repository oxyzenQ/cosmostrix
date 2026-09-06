// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Dragon serpentine-body rain style + extracted state-machine helpers.
//!
//! Layout (mirrors the monolith family split from NIGHT-hunter-10):
//! - `dragon.rs` - state machine, chain solver, diff cleanup.
//! - `dragon_helpers.rs` - glyph pool picks, brightness zoning, noise
//!   rolls, single-cell renderer.
//!
//! `DragonState` is re-exported for test consumption only; the bin
//! target reaches the other four types directly via `crate::cloud::
//! dragon::*` (the backward-compat alias in `cloud/mod.rs`).

pub(crate) mod dragon;
pub(crate) mod dragon_helpers;

#[allow(unused_imports)]
pub(crate) use dragon::{DragonRain, DragonRandom, DragonSpawnParams, DragonState, DragonStep};
