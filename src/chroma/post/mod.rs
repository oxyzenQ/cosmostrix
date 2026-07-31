// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Chroma Dragon — Post-processing Shaders
//!
//! Pure functions that operate on the resolved cell color AFTER the base
//! shader (`chroma::shaders::base::resolve_cell_color`) has picked a
//! palette stop, but BEFORE the color is written to the frame.
//!
//! ## Why integrate post-processing into the shader pipeline?
//!
//! Pre-Phase-3-G, atmospheric effects (luminance climate, saturation
//! drift, persistence richness, instability pressure) were applied in a
//! separate post-hoc pass (`cloud::phosphor::apply_atmospheric_frame_effects`)
//! that iterated all dirty cells, decoded each cell's `Color` back to
//! `(r, g, b)`, applied the modifiers, and re-encoded. That meant every
//! dirty cell paid:
//!
//! 1. One `Color::Rgb { .. }` decode in the post-hoc pass
//! 2. One `Color::Rgb { .. }` encode after modification
//! 3. One `frame.set()` call (which marks the cell dirty AGAIN, causing
//!    a redundant redraw on the next frame's diff)
//!
//! Phase 3-G moves the atmospheric math into `chroma::post::atmosphere`
//! as a pure function `apply_atmospheric(r, g, b, line, col, ctx)`. The
//! base shader calls it on the resolved color before returning, so the
//! cell is written to the frame ONCE with atmospheric already applied.
//! The post-hoc pass becomes a no-op when the shader integration is
//! active — eliminating ~500 decode+encode+frame.set cycles per frame.
//!
//! ## Module layout
//!
//! | Module       | Concern                                                              |
//! |--------------|----------------------------------------------------------------------|
//! | `atmosphere` | `AtmosphericCtx`, `apply_atmospheric()` — luminance/saturation/instability |

pub mod atmosphere;
