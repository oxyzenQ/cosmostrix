// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Internal rain style selection.
//!
//! Style families (task-19, four styles):
//! - Droplet family ([`RainStyle::Glyph`]) — rendered by the shared
//!   droplet pool (column-cascade motion, spawn_droplets, phosphor
//!   Pass 2 protection).
//! - Structured family ([`RainStyle::Monolith`], [`RainStyle::Vortex`],
//!   [`RainStyle::Flux`]) — dedicated state machines with drawn-cell
//!   diff cleanup; no droplet pool. Vortex moves glyphs on polar
//!   orbits; Flux moves glyphs through a PIC/FLIP incompressible
//!   fluid (see `cloud/flux_field.rs`).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RainStyle {
    Glyph,
    Monolith,
    Vortex,
    Flux,
}

impl RainStyle {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Glyph => "glyph",
            Self::Monolith => "monolith",
            Self::Vortex => "vortex",
            Self::Flux => "flux",
        }
    }

    /// True for styles rendered by the droplet pool: the Glyph
    /// cascade is the sole droplet-family style since task-19
    /// (Flux replaced the task-18 Ripple surface style, which was
    /// the second droplet-family member). Gates that previously
    /// read `!matches!(style, Monolith)` should read this instead —
    /// three non-droplet styles exist now.
    #[must_use]
    pub fn is_droplet_family(self) -> bool {
        matches!(self, Self::Glyph)
    }

    /// True for styles that integrate spawn through the fractional
    /// `spawn_remainder` accumulator (Monolith lanes, Vortex motes,
    /// Flux fluid particles). Glyph-family spawn uses per-column
    /// timing instead.
    #[must_use]
    pub fn uses_spawn_remainder(self) -> bool {
        matches!(self, Self::Monolith | Self::Vortex | Self::Flux)
    }
}
