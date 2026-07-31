// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Chroma Dragon — Coloring Engine
//!
//! The coloring counterpart to the Cosmic Dragon rendering engine. Where the
//! Cosmic Dragon (`src/cloud/`, `src/frame.rs`) owns the diff-based render
//! loop and droplet simulation, the Chroma Dragon owns every decision about
//! *what color a cell becomes*.
//!
//! ## Phase 1 — Foundation
//!
//! Phase 1 was a pure relocation: `src/palette.rs` → `chroma/palette.rs` and
//! `src/central_colors.rs` → `chroma/catalog.rs`. Zero behavior change. The
//! crate root re-exports both modules (`pub use chroma::palette;` and
//! `pub use chroma::catalog;`) so every existing `use crate::palette::…` and
//! `use crate::central_colors::…` call site continues to resolve unchanged.
//!
//! ## Phase 2 — Shader extraction
//!
//! Phase 2 extracts the cell-color decision logic out of
//! `cloud::render::DrawCtx::get_attr()` into a pure function
//! `chroma::shaders::base::resolve_cell_color()`. The `CharLoc` enum and
//! `TRAIL_EXP_LUT` static move with it. `DrawCtx::get_attr()` becomes a
//! thin wrapper that constructs a `ShaderCtx` borrow view and delegates.
//! Zero behavior change — the body is identical, only the receiver type
//! changed.
//!
//! ## Roadmap (later phases, not yet implemented)
//!
//! - Phase 3+: ordered dithering, temporal column hue coherence, head halo,
//!   subpixel hue jitter, luminance-remap for short droplets, integrated
//!   atmospheric shader, `hue_drift` activation, palette-aware ghost color.
//!
//! ## Module layout
//!
//! | Module       | Origin                                   | Concern                          |
//! |--------------|------------------------------------------|----------------------------------|
//! | `palette`    | `src/palette.rs` (moved in Phase 1)      | `Palette` struct, `build_palette`, gradient + blend helpers |
//! | `catalog`    | `src/central_colors.rs` (moved in Phase 1) | `THEMES` registry, `build_colors`, `ThemeDef`/`ThemeColors` |
//! | `shaders`    | new in Phase 2                           | `ShaderCtx`, `CharLoc`, `resolve_cell_color()`, `TRAIL_EXP_LUT` |
//! | `gradient`   | new in Phase 3-A                         | OKLab interpolation, `gradient_from_stops_oklab()` |
//!
//! Future phases will add `post/`, `stops.rs`, `ecosystem.rs` under this
//! namespace.

pub mod catalog;
pub mod gradient;
pub mod palette;
pub mod post;
pub mod shaders;
pub mod tuning;
