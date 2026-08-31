// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Chroma Dragon — Coloring Engine
//!
//! The coloring counterpart to the Cosmic Dragon rendering engine. Where the
//! Cosmic Dragon (`src/engine/cosmic_dragon_engine/cloud/`, `src/engine/cosmic_dragon_engine/frame.rs`)
//! owns the diff-based render loop and droplet simulation, the Chroma Dragon
//! owns every decision about *what color a cell becomes*.
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
//! | `gradient`   | new in Phase 3-A                         | OKLab polar interpolation (sole production path since v30) |
//! | `legacy`     | current                             | Explicit sRGB-linear fallback math (`scale_rgb`, `blend_toward_rgb`, `boost_rgb`) used when `ColorPipeline::LegacyRgb` is active |
//! | `intro_colors` | new in Intro Integration audit    | Cinematic brand color constants (cosmic burst, logo, singularity) — single source of truth for intro colors |
//!
//! Modules `palette`, `catalog`, `shaders`, `gradient`, `legacy`, `post`,
//! `tuning`, `intro_colors` cover all chroma concerns; no further sub-modules are planned.

pub mod catalog;
pub(crate) mod gradient;
pub(crate) mod legacy;
pub mod palette;
pub(crate) mod post;
pub(crate) mod shaders;
pub(crate) mod tuning;

// Newly relocated from src/ root (audit M2). Re-exported as `pub(crate)`
// so the existing `crate::color_cache::Foo` / `crate::color_tune::Foo` /
// `crate::colors_custom::Foo` call sites continue to resolve via the
// `pub(crate) use chroma::{...}` re-export in main.rs.
pub(crate) mod color_cache;
pub(crate) mod color_tune;
pub(crate) mod colors_custom;
pub(crate) mod intro_colors;

// Tests now live in chroma/tests/ subdir (Pattern C — dedicated tests/).
// Was previously two separate #[path] declarations (Pattern B).
#[cfg(test)]
mod tests;
