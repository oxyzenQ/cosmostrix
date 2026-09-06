// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Glyph rain style (droplet family).
//!
//! The Glyph style is the sole droplet-family rain style since
//! task-19 (Flux replaced the task-18 Ripple surface style, which
//! was the second droplet-family member). Unlike the six structured
//! styles which each have a `<name>Rain` state-machine struct, Glyph
//! renders through the shared [`Droplet`] pool with column-cascade
//! motion, phosphor Pass 2 protection, and per-column timing.
//!
//! Family files:
//! - `spawn_logic.rs` — per-frame droplet spawn decision + spec
//!   construction (`spawn_droplets` + `build_droplet_spec`).
//! - `pool_lifecycle.rs` — droplet pool lifecycle methods
//!   (`recalc_droplets_per_sec`, `update_droplet_speeds`,
//!   `ensure_glyph_pool_and_warm_start`). Extracted from
//!   `cloud/spawn.rs` in NIGHT-enhanced-hunt-G.
//!
//! [`Droplet`]: crate::droplet::Droplet

pub(crate) mod pool_lifecycle;
pub(crate) mod spawn_logic;
