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
//! This module owns the Glyph-specific spawn decision and spec
//! construction (`spawn_droplets` + `build_droplet_spec`). The
//! remaining Glyph-specific lifecycle methods (`recalc_droplets_per_sec`,
//! `update_droplet_speeds`, `ensure_glyph_pool_and_warm_start`) stay
//! in `cloud/spawn.rs` because they are declared in the same `impl
//! Cloud` block as general Cloud lifecycle methods (reset, init_chars,
//! transition_chars, glitch management) and extracting them would
//! split that block unnecessarily.
//!
//! [`Droplet`]: crate::droplet::Droplet

pub(crate) mod spawn_logic;
