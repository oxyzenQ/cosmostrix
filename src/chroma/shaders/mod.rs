// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Chroma Dragon — Shaders
//!
//! Pure color-decision functions extracted from the Cosmic Dragon's
//! `cloud::render::DrawCtx::get_attr()` in Phase 2 of the Chroma Dragon
//! migration.
//!
//! ## Design
//!
//! Each shader is a free function `fn(shader: &ShaderCtx, …) -> (Color, bool)`
//! that takes a read-only borrow view of the per-frame inputs plus per-cell
//! coordinates. No allocation, no virtual dispatch, no trait objects — the
//! hot path is 100–300 calls/frame and must remain monomorphic and inlinable.
//!
//! ## Module layout
//!
//! | Module | Concern                                                          |
//! |--------|------------------------------------------------------------------|
//! | `base` | `ShaderCtx`, `CharLoc`, `resolve_cell_color()`, `color_uses_previous_palette()`, `TRAIL_EXP_LUT` |
//!
//! Future phases will add `oklab`, `dither`, `halo`, `atmosphere` under this
//! namespace as the innovations (A–I) land one micro-commit at a time.

pub mod base;
