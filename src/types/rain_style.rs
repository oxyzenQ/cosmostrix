// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Internal rain style selection.
//!
//! Style families (task-18 + NIGHT-research-5/6, six styles):
//! - Droplet family ([`RainStyle::Glyph`], [`RainStyle::Ripple`]) —
//!   rendered by the shared droplet pool (column-cascade motion,
//!   spawn_droplets, phosphor Pass 2 protection). Ripple adds a
//!   water-surface system on top of the falling rain.
//! - Structured family ([`RainStyle::Monolith`], [`RainStyle::Vortex`],
//!   [`RainStyle::Dragon`], [`RainStyle::Physarum`]) — dedicated state
//!   machines with drawn-cell diff cleanup; no droplet pool. Vortex
//!   moves glyphs on polar Keplerian orbits; Dragon moves a
//!   serpentine chain of segments following a path-generating head
//!   via FABRIK distance constraints (Chinese-mythology dragon —
//!   free flight + occasional circling); Physarum runs the Jeff Jones
//!   2010 slime-mold model — particles sense / decide / move / deposit
//!   on a stigmergic trail field, producing emergent network patterns
//!   (bio-inspired algorithm — the terminal's discrete cell grid IS
//!   the slime-mold substrate).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RainStyle {
    Glyph,
    Monolith,
    Vortex,
    Ripple,
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
            Self::Ripple => "ripple",
            Self::Dragon => "dragon",
            Self::Physarum => "physarum",
        }
    }

    /// True for styles rendered by the droplet pool: the Glyph cascade
    /// and the Ripple surface style share the droplet spawn/advance/draw
    /// plumbing and phosphor Pass 2 trail protection. Gates that
    /// previously read `!matches!(style, Monolith)` should read this
    /// instead — four non-droplet styles (Vortex, Dragon, Physarum)
    /// exist now.
    #[must_use]
    pub fn is_droplet_family(self) -> bool {
        matches!(self, Self::Glyph | Self::Ripple)
    }

    /// True for styles that integrate spawn through the fractional
    /// `spawn_remainder` accumulator (Monolith lanes, Vortex motes,
    /// Dragon chains, Physarum particles). Glyph-family spawn uses
    /// per-column timing instead.
    #[must_use]
    pub fn uses_spawn_remainder(self) -> bool {
        matches!(
            self,
            Self::Monolith | Self::Vortex | Self::Dragon | Self::Physarum
        )
    }
}
