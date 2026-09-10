// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Black hole rain style (NIGHT-special-1): the ball module, the
//! stage-2 orbital ring physics module, the stage-2.1 ball rendering
//! helpers, the stage-2.2 formation phase math, the stage-2.6 halo
//! stream physics, and the stage-3 glyph infall layer (the split
//! mirrors the monolith/dragon family helper pattern).

pub(crate) mod ball_helpers;
pub(crate) mod black_hole;
pub(crate) mod formation;
pub(crate) mod halo;
pub(crate) mod infall;
pub(crate) mod ring;

pub(crate) use black_hole::BlackHoleRain;
pub(crate) use ring::{BlackHoleRandom, BlackHoleSpawnParams, BlackHoleStep};

/// Per-frame see-saw roll snapshot (NIGHT-lts-1 stage 1): the draw
/// pass evaluates the attitude angle's sine and cosine ONCE per
/// frame and threads this `Copy` pair through every ring and halo
/// projection — the angle is rigid across the whole stack within a
/// frame, so the per-mote projections used to re-evaluate an
/// identical `sin`/`cos` pair per mote (two pools of ~cols motes
/// each, ~2 x active-motes redundant transcendentals per frame for
/// one unique angle). Lives here — the module root — because both
/// `ring.rs` and `halo.rs` consume it and `black_hole.rs` builds it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RollFrame {
    /// Sine of the frame's attitude angle.
    pub(crate) sin: f32,
    /// Cosine of the frame's attitude angle.
    pub(crate) cos: f32,
}

impl RollFrame {
    /// Snapshot an attitude angle (radians) — one `sin_cos`
    /// evaluation per frame, shared by every mote of both pools.
    #[must_use]
    pub(crate) fn from_angle(roll: f32) -> Self {
        let (sin, cos) = roll.sin_cos();
        Self { sin, cos }
    }

    /// The flat rest line (roll = 0, the Gargantua hold) — the
    /// steady fixture the projection tests pin their geometry
    /// against.
    #[cfg(test)]
    pub(crate) const FLAT: RollFrame = RollFrame { sin: 0.0, cos: 1.0 };

    /// True when the snapshot is the flat rest line (the zero
    /// rotation the projections may skip).
    pub(crate) fn is_flat(&self) -> bool {
        self.sin == 0.0 && self.cos == 1.0
    }
}
