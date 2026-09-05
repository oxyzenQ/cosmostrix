// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Internal rain style selection.
//!
//! Style families (NIGHT-research-4, four styles):
//! - Droplet family ([`RainStyle::Glyph`]) — rendered by the shared
//!   droplet pool (column-cascade motion, spawn_droplets, phosphor
//!   Pass 2 protection).
//! - Structured family ([`RainStyle::Monolith`], [`RainStyle::Vortex`],
//!   [`RainStyle::Lorenz`]) — dedicated state machines with drawn-cell
//!   diff cleanup; no droplet pool. Vortex moves glyphs on polar
//!   Keplerian orbits; Lorenz moves glyphs along the canonical
//!   strange-attractor trajectory (RK4-integrated 3D chaos projected
//!   to 2D, two-lobe butterfly — replaces the rejected ripple style).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RainStyle {
    Glyph,
    Monolith,
    Vortex,
    Lorenz,
}

impl RainStyle {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Glyph => "glyph",
            Self::Monolith => "monolith",
            Self::Vortex => "vortex",
            Self::Lorenz => "lorenz",
        }
    }

    /// True for styles rendered by the droplet pool. The Glyph cascade
    /// is the only droplet-family style now (ripple was rejected and
    /// replaced by Lorenz, a structured style). Gates that previously
    /// read `!matches!(style, Monolith)` should read this instead —
    /// two non-droplet styles (Vortex, Lorenz) exist now.
    #[must_use]
    pub fn is_droplet_family(self) -> bool {
        matches!(self, Self::Glyph)
    }

    /// True for styles that integrate spawn through the fractional
    /// `spawn_remainder` accumulator (Monolith lanes, Vortex motes,
    /// Lorenz motes). Glyph-family spawn uses per-column timing
    /// instead.
    #[must_use]
    pub fn uses_spawn_remainder(self) -> bool {
        matches!(self, Self::Monolith | Self::Vortex | Self::Lorenz)
    }
}
