// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Chroma Dragon — Coloring Engine
//!
//! The coloring counterpart to the Cosmic Dragon rendering engine. Where the
//! Cosmic Dragon (`src/cloud/`, `src/frame.rs`) owns the diff-based render
//! loop and droplet simulation, the Chroma Dragon owns every decision about
//! *what color a cell becomes*.
//!
//! ## Phase 1 (this commit) — Foundation
//!
//! Phase 1 is a pure relocation: `src/palette.rs` → `chroma/palette.rs` and
//! `src/central_colors.rs` → `chroma/catalog.rs`. Zero behavior change. The
//! crate root re-exports both modules (`pub use chroma::palette;` and
//! `pub use chroma::catalog;`) so every existing `use crate::palette::…` and
//! `use crate::central_colors::…` call site continues to resolve unchanged.
//!
//! ## Roadmap (later phases, not yet implemented)
//!
//! - Phase 2: Extract `get_attr()` color logic into `chroma/shaders/`.
//! - Phase 3+: OKLab interpolation, ordered dithering, temporal column hue
//!   coherence, head halo, subpixel hue jitter, luminance-remap for short
//!   droplets, integrated atmospheric shader, `hue_drift` activation,
//!   palette-aware ghost color.
//!
//! ## Module layout
//!
//! | Module       | Origin                        | Concern                          |
//! |--------------|-------------------------------|----------------------------------|
//! | `palette`    | `src/palette.rs` (moved)      | `Palette` struct, `build_palette`, gradient + blend helpers |
//! | `catalog`    | `src/central_colors.rs` (moved) | `THEMES` registry, `build_colors`, `ThemeDef`/`ThemeColors` |
//!
//! Future phases will add `shaders/`, `post/`, `gradient.rs`, `stops.rs`,
//! `ecosystem.rs` under this namespace.

pub mod catalog;
pub mod palette;
