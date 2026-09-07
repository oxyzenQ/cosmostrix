// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

#![allow(clippy::module_inception)]

//! Rain-style family modules for the cosmic dragon engine cloud.
//!
//! Each rain style lives in its own subdirectory so the main module
//! (`<name>.rs`) and its extracted helpers (`<name>_helpers.rs`,
//! `<name>_glyphs.rs`, `<name>_field.rs`) are grouped together
//! instead of being flat siblings of unrelated cloud infrastructure
//! (spawn, render, message_draw, ...). The directory-per-style
//! convention also matches the existing `cloud/events/` layout.
//!
//! Cross-style references are expected and intentional: lorenz,
//! vortex, dragon, physarum and flux all reuse the monolith
//! brightness ladder (`BrightnessLevel` + the `bold_for_level` /
//! `color_for_level` palette helpers), so the helpers stay
//! `pub(crate)` to keep that reuse ergonomic without leaking past
//! the engine boundary.
//!
//! Members (NIGHT-enhanced-1 directory-per-style layout):
//! - `glyph` - droplet-family rain (column cascade, shared Droplet pool).
//! - `lorenz` - chaotic-attractor rain (Lorenz 1963 strange attractor).
//! - `dragon` - Chinese-dragon serpentine body state machine.
//! - `monolith` - vertical monolith rain with brightness spine +
//!   glyph helpers reused by 4 of the 6 styles.
//! - `physarum` - Physarum polycephalum slime-mold foraging trails.
//! - `vortex` - vortex swirl rain.
//! - `flux` - PIC/FLIP hybrid incompressible fluid solver rain.
//! - `black_hole` - event-horizon ball for the sorgonemous_intrascals
//!   scene (NIGHT-special-1; staged rollout — stage 1 ships the ball,
//!   the RK4 ring and glyph infall follow in stages 2 and 3).
//!
//! `module_inception` (`pub(crate) mod lorenz;` inside `type_rain/
//! lorenz/mod.rs`) is allowed at the file level because the directory
//! convention is the explicit NIGHT-enhanced-1 layout choice — the
//! inner `lorenz.rs` is the canonical module file for the rain style
//! and shares its name with the family directory for navigational
//! symmetry with `dragon/dragon.rs`, `monolith/monolith.rs`, etc.

pub(crate) mod black_hole;
pub(crate) mod dragon;
pub(crate) mod flux;
pub(crate) mod glyph;
pub(crate) mod lorenz;
pub(crate) mod monolith;
pub(crate) mod physarum;
pub(crate) mod vortex;
