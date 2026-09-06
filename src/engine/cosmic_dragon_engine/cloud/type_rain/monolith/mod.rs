// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Monolith vertical-rain style + extracted glyph/palette helpers.
//!
//! Layout (NIGHT-hunter-10 800-LOC split):
//! - `monolith.rs` - state machine, segments, spine envelope.
//! - `monolith_glyphs.rs` - charset-aware glyph mapping per segment.
//! - `monolith_helpers.rs` - 19 free functions used by MonolithRain
//!   (activate_stream, build_segments,
//!   draw_spine, color_for_level, ...).
//!
//! `bold_for_level` and `color_for_level` are re-exported here because
//! lorenz, vortex, dragon, physarum and flux all share the monolith
//! brightness ladder and palette logic - keeping the canonical
//! re-export at the family root avoids each consumer reaching into
//! `monolith::monolith_helpers::*` directly. `clear_cell` and
//! `pick_pool_char` are also re-exported for the same reason (the
//! 4 non-monolith rain styles clear cells via the same path so the
//! phosphor metadata stays consistent across styles).

pub(crate) mod monolith;
pub(crate) mod monolith_glyphs;
pub(crate) mod monolith_helpers;

// Re-export the monolith public surface so `crate::cloud::monolith::*`
// (the backward-compat alias in `cloud/mod.rs`) keeps resolving after
// the directory move. Some of these (e.g. `DrawnCellKind`,
// `bold_for_level`, `color_for_level`) are only consumed by tests, so
// `#[allow(unused_imports)]` keeps the bin build quiet without forcing
// `#[cfg(test)]` (which would break the crate-internal call sites that
// reach the helpers through the alias).
#[allow(unused_imports)]
pub(crate) use monolith::{
    BrightnessLevel, DrawnCellKind, MonolithCleanup, MonolithRain, MonolithRandom,
    MonolithSpawnParams,
};
#[allow(unused_imports)]
pub(crate) use monolith_helpers::{bold_for_level, clear_cell, color_for_level, pick_pool_char};
