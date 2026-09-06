// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Flux PIC/FLIP hybrid incompressible-fluid rain style + Eulerian
//! velocity grid solver.
//!
//! Layout:
//! - `flux.rs` - particle half of the hybrid: glyph splatting,
//!   gravity, G2P readback, FLIP/PIC blend.
//! - `flux_field.rs` - Eulerian grid half: P2G, pressure projection
//!   (Jacobi), divergence, no-through-flow walls,
//!   open top/bottom boundaries.
//!
//! `FluxField` and `FluxVel` are re-exported for test consumption
//! only; the bin target reaches the rain/step/params types directly
//! via `crate::cloud::flux::*` (the backward-compat alias in
//! `cloud/mod.rs`).

pub(crate) mod flux;
pub(crate) mod flux_field;

#[allow(unused_imports)]
pub(crate) use flux::{FluxRain, FluxRandom, FluxSpawnParams, FluxStep};
#[allow(unused_imports)]
pub(crate) use flux_field::{FluxField, FluxVel, FLUX_GRID_SPACING};
