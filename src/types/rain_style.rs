// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Internal rain style selection.
//!
//! Style families (task-19 + NIGHT-research-4/5/6 +
//! NIGHT-special-1 + NIGHT-special-2 + NIGHT-special-4 +
//! NIGHT-research-7 + NIGHT-research-8 + NIGHT-research-9,
//! fourteen styles):
//! - Droplet family ([`RainStyle::Glyph`]) — rendered by the shared
//!   droplet pool (column-cascade motion, spawn_droplets, phosphor
//!   Pass 2 protection).
//! - Structured family ([`RainStyle::Monolith`], [`RainStyle::Vortex`],
//!   [`RainStyle::Flux`], [`RainStyle::Lorenz`], [`RainStyle::Dragon`],
//!   [`RainStyle::Physarum`], [`RainStyle::BlackHole`],
//!   [`RainStyle::Aeolian`], [`RainStyle::SolarFlare`],
//!   [`RainStyle::DnaHelix`], [`RainStyle::Murmuration`],
//!   [`RainStyle::Quasar`], [`RainStyle::Neural`]) — dedicated
//!   state machines with
//!   drawn-cell diff cleanup; no droplet pool. Vortex moves glyphs
//!   on polar Keplerian orbits; Flux moves glyphs through a PIC/FLIP
//!   incompressible fluid (see `cloud/type_rain/flux/flux_field.rs`); Lorenz moves
//!   glyphs along the canonical strange-attractor trajectory
//!   (RK4-integrated 3D chaos projected to 2D, two-lobe butterfly);
//!   Dragon moves a serpentine chain of segments following a
//!   path-generating head via FABRIK distance constraints
//!   (Chinese-mythology dragon — free flight + occasional circling);
//!   Physarum runs the Jeff Jones 2010 slime-mold model — particles
//!   sense / decide / move / deposit on a stigmergic trail field,
//!   producing emergent network patterns (bio-inspired algorithm —
//!   the terminal's discrete cell grid IS the slime-mold substrate);
//!   BlackHole renders a gravitating body — a centered ball with a
//!   black empty event-horizon core (NIGHT-special-1 staged rollout:
//!   stage 1 ships the ball, the RK4 orbital ring and the glyph infall
//!   follow in stages 2 and 3); Aeolian runs the invented string
//!   weave — glyph rain falls onto invisible horizontal strings,
//!   plucks them, and surfs the ringing wavefronts (NIGHT-special-2:
//!   the first style whose motion math was derived in this repo
//!   from first principles, carrying no existing reference — the
//!   six laws of the weave, see `cloud/type_rain/aeolian/mod.rs`);
//!   SolarFlare runs the invented corona arcade — glyph rain
//!   condenses at the tops of magnetic loops, slides down the legs
//!   with energy-conserving acceleration and flashes the footpoints
//!   it lands on until a flux-laden loop erupts (NIGHT-special-4:
//!   the third original-math style — the five laws of the corona,
//!   see `cloud/type_rain/solar_flare/mod.rs`); DnaHelix renders a
//!   rotating double helix of glyph strands spanned by Watson-Crick
//!   base-pair rungs, fed by a nucleotide soup, periodically swept
//!   by a replication fork that dissolves, widens and re-synthesizes
//!   the ladder — the pair re-rolled, a visible mutation
//!   (NIGHT-research-7: the eleventh style, the DeepSeek-researched
//!   first pick — the five laws of the ladder, see
//!   `cloud/type_rain/dna_helix/mod.rs`); Murmuration runs the
//!   Reynolds 1987 boids flock — hundreds of glyph birds flying
//!   as one shape-shifting body over a spatial hash (separation,
//!   alignment, cohesion + a roaming anchor, a breathing
//!   cohesion and a clocked predator startle — the five laws of
//!   the flock, see `cloud/type_rain/murmuration/mod.rs`;
//!   NIGHT-research-7, the twelfth style, the shortlist's second
//!   pick); Quasar runs the feeding engine — a supermassive
//!   black hole at full power: glyph gas rains onto a Keplerian
//!   accretion disk (the inner ring lapping the outer, one limb
//!   doppler-brightened), the core breathes white-hot, and the
//!   poles fire precessing, knotting relativistic jets, with
//!   every falling glyph the fuel (the five laws of the engine,
//!   see `cloud/type_rain/quasar/mod.rs`; NIGHT-research-8, the
//!   thirteenth style, the owner's pick over the neural-network
//!   proposal); Neural runs the training network — glyph data
//!   falls onto an input band of integrate-and-fire neurons,
//!   pulses ride dendritic wires from layer to layer, a
//!   thought-burst clock fires whole waves through the machine
//!   and slow plasticity rewires the topology (the five laws of
//!   the network, see `cloud/type_rain/neural/mod.rs`;
//!   NIGHT-research-9, the fourteenth style — the runner-up
//!   proposal finally seated, the registry's machine-mind
//!   domain).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RainStyle {
    Glyph,
    Monolith,
    Vortex,
    Flux,
    Lorenz,
    Dragon,
    Physarum,
    BlackHole,
    Aeolian,
    SolarFlare,
    DnaHelix,
    Murmuration,
    Quasar,
    Neural,
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
            Self::BlackHole => "black_hole",
            Self::Aeolian => "aeolian",
            Self::SolarFlare => "solar_flare",
            Self::DnaHelix => "dna_helix",
            Self::Murmuration => "murmuration",
            Self::Quasar => "quasar",
            Self::Neural => "neural",
        }
    }

    /// True for styles rendered by the droplet pool: the Glyph
    /// cascade is the sole droplet-family style since task-19
    /// (Flux replaced the task-18 Ripple surface style, which was
    /// the second droplet-family member). Gates that previously
    /// read `!matches!(style, Monolith)` should read this instead —
    /// thirteen non-droplet styles exist now.
    #[must_use]
    pub fn is_droplet_family(self) -> bool {
        matches!(self, Self::Glyph)
    }

    /// True for styles that integrate spawn through the fractional
    /// `spawn_remainder` accumulator (Monolith lanes, Vortex motes,
    /// Flux fluid particles, Lorenz motes, Dragon chains, Physarum
    /// particles, the BlackHole orbital-ring motes since stage 2,
    /// the Aeolian drops since NIGHT-special-2, the SolarFlare
    /// drops since NIGHT-special-4, the DnaHelix nucleotides and
    /// the Murmuration birds since NIGHT-research-7, the Quasar
    /// infall streamers since NIGHT-research-8, the Neural data
    /// streamers since NIGHT-research-9).
    /// Glyph-family spawn uses per-column timing instead.
    #[must_use]
    pub fn uses_spawn_remainder(self) -> bool {
        matches!(
            self,
            Self::Monolith
                | Self::Vortex
                | Self::Flux
                | Self::Lorenz
                | Self::Dragon
                | Self::Physarum
                | Self::BlackHole
                | Self::Aeolian
                | Self::SolarFlare
                | Self::DnaHelix
                | Self::Murmuration
                | Self::Quasar
                | Self::Neural
        )
    }

    /// Parse a `RainStyle` from its canonical CLI label (the inverse
    /// of [`as_str`]). Used by the scene-custom `rain = "..."` field
    /// so users can pick the rain style by name (e.g. `rain = "lorenz"`
    /// or `rain = "vortex"`). Case-insensitive, returns `None` for
    /// unrecognized values so the caller can render a targeted hint
    /// with the valid labels list.
    ///
    /// NIGHT-research-5 (owner-approved): the canonical labels are
    /// the same lowercase strings [`as_str`] returns — this keeps the
    /// scene-custom `rain` field consistent with `--show-scene` /
    /// `--list-scenes` output (no separate alias surface to drift).
    #[must_use]
    pub fn from_label(label: &str) -> Option<Self> {
        match label.trim().to_ascii_lowercase().as_str() {
            "glyph" => Some(Self::Glyph),
            "monolith" => Some(Self::Monolith),
            "vortex" => Some(Self::Vortex),
            "flux" => Some(Self::Flux),
            "lorenz" => Some(Self::Lorenz),
            "dragon" => Some(Self::Dragon),
            "physarum" => Some(Self::Physarum),
            "black_hole" | "blackhole" => Some(Self::BlackHole),
            "aeolian" => Some(Self::Aeolian),
            "solar_flare" | "solarflare" | "flare" => Some(Self::SolarFlare),
            "dna_helix" | "dnahelix" | "dna" => Some(Self::DnaHelix),
            "murmuration" | "murmur" | "starlings" => Some(Self::Murmuration),
            "quasar" | "agn" => Some(Self::Quasar),
            "neural" | "neural_network" | "neuralnet" | "nn" => Some(Self::Neural),
            _ => None,
        }
    }

    /// Human-readable comma-separated list of valid `rain` field
    /// values, for error messages and config hints. Mirrors the
    /// `GlitchLevel::from_str` hint convention ("none, subtle,
    /// default, intense"). Order matches the enum declaration.
    #[must_use]
    pub fn valid_labels_hint() -> &'static str {
        "glyph, monolith, vortex, flux, lorenz, dragon, physarum, black_hole, aeolian, solar_flare, dna_helix, murmuration, quasar, neural"
    }
}
