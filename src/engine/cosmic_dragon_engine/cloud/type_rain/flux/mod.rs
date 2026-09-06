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

pub(crate) mod flux;
pub(crate) mod flux_field;

// Re-export so `crate::cloud::flux::*` (alias) and `crate::cloud::
// flux_field::*` (the deeper alias in cloud/mod.rs) keep resolving
// after the move. `FluxField` and `FluxVel` are only consumed by
// tests directly, hence the `#[allow(unused_imports)]` to keep the
// bin build quiet without `#[cfg(test)]`-gating the alias chain.
#[allow(unused_imports)]
pub(crate) use flux::{FluxRain, FluxRandom, FluxSpawnParams, FluxStep};
#[allow(unused_imports)]
pub(crate) use flux_field::{FluxField, FluxVel, FLUX_GRID_SPACING};
