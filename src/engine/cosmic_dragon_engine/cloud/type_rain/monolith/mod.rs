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
//! `bold_for_level` and `color_for_level` are re-exported here
//! because lorenz, vortex, dragon, physarum and flux all share the
//! monolith brightness ladder and palette logic - keeping the
//! canonical re-export at the family root avoids each consumer
//! reaching into `monolith::monolith_helpers::*` directly. Tests
//! also reference `color_for_level` via `crate::cloud::monolith::*`
//! (the backward-compat alias in `cloud/mod.rs`), so these two
//! helpers stay `pub(crate)`.
//!
//! `clear_cell` and `pick_pool_char` are NOT re-exported: after
//! NIGHT-enhanced-hunt-A and NIGHT-enhanced-hunt-D their visibility
//! is `pub(in cloud::type_rain)` (tighter than `pub(crate)`), so
//! the 5 cross-family consumers import them directly from
//! `monolith_helpers`. A `pub(crate) use` re-export would be a
//! privacy upgrade and fail to compile. Tests don't reference
//! either function directly (only in comments).
//!
//! `DrawnCellKind` is re-exported for test consumption only.

pub(crate) mod monolith;
pub(crate) mod monolith_glyphs;
pub(crate) mod monolith_helpers;

#[allow(unused_imports)]
pub(crate) use monolith::{
    BrightnessLevel, DrawnCellKind, MonolithCleanup, MonolithRain, MonolithRandom,
    MonolithSpawnParams,
};
#[allow(unused_imports)]
pub(crate) use monolith_helpers::{bold_for_level, color_for_level};
