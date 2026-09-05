// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Internal rain style selection.
//!
//! Style families (task-19 + NIGHT-research-4/5/6, seven styles):
//! - Droplet family ([`RainStyle::Glyph`]) — rendered by the shared
//!   droplet pool (column-cascade motion, spawn_droplets, phosphor
//!   Pass 2 protection).
//! - Structured family ([`RainStyle::Monolith`], [`RainStyle::Vortex`],
//!   [`RainStyle::Flux`], [`RainStyle::Lorenz`], [`RainStyle::Dragon`],
//!   [`RainStyle::Physarum`]) — dedicated state machines with
//!   drawn-cell diff cleanup; no droplet pool. Vortex moves glyphs
//!   on polar Keplerian orbits; Flux moves glyphs through a PIC/FLIP
//!   incompressible fluid (see `cloud/flux_field.rs`); Lorenz moves
//!   glyphs along the canonical strange-attractor trajectory
//!   (RK4-integrated 3D chaos projected to 2D, two-lobe butterfly);
//!   Dragon moves a serpentine chain of segments following a
//!   path-generating head via FABRIK distance constraints
//!   (Chinese-mythology dragon — free flight + occasional circling);
//!   Physarum runs the Jeff Jones 2010 slime-mold model — particles
//!   sense / decide / move / deposit on a stigmergic trail field,
//!   producing emergent network patterns (bio-inspired algorithm —
//!   the terminal's discrete cell grid IS the slime-mold substrate).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RainStyle {
    Glyph,
    Monolith,
    Vortex,
    Flux,
    Lorenz,
    Dragon,
    Physarum,
}

impl RainStyle {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Glyph => "glyph",
            Self::Monolith => "monolith",
            Self::Vortex => "vortex",
            Self::Flux => "flux",
            Self::Lorenz => "lorenz",
            Self::Dragon => "dragon",
            Self::Physarum => "physarum",
        }
    }

    /// True for styles rendered by the droplet pool: the Glyph
    /// cascade is the sole droplet-family style since task-19
    /// (Flux replaced the task-18 Ripple surface style, which was
    /// the second droplet-family member). Gates that previously
    /// read `!matches!(style, Monolith)` should read this instead —
    /// six non-droplet styles exist now.
    #[must_use]
    pub fn is_droplet_family(self) -> bool {
        matches!(self, Self::Glyph)
    }

    /// True for styles that integrate spawn through the fractional
    /// `spawn_remainder` accumulator (Monolith lanes, Vortex motes,
    /// Flux fluid particles, Lorenz motes, Dragon chains, Physarum
    /// particles). Glyph-family spawn uses
    /// per-column timing instead.
    #[must_use]
    pub fn uses_spawn_remainder(self) -> bool {
        matches!(
            self,
            Self::Monolith | Self::Vortex | Self::Flux | Self::Lorenz
                | Self::Dragon | Self::Physarum
        )
    }
}
