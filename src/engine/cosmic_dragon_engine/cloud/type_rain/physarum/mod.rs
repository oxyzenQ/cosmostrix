// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Physarum slime-mold trail-field rain style + extracted helpers.
//!
//! Layout (mirrors the monolith family split):
//! - `physarum.rs` - stigmergic sense/decide/move/deposit +
//!   trail decay core.
//! - `physarum_helpers.rs` - trail sampling, render/level helpers,
//!   deterministic tie-break rolls, test hooks.

pub(crate) mod physarum;
pub(crate) mod physarum_helpers;

pub(crate) use physarum::{PhysarumRain, PhysarumRandom, PhysarumSpawnParams, PhysarumStep};
