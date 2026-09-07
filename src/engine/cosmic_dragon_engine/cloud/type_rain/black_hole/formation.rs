// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Black hole formation intro (NIGHT-special-1 stage 2.2): the pure
//! phase math for the hole's birth sequence, split from
//! `black_hole.rs` the way the ball/ring modules split their
//! concerns (the state machine's mutable half — the formation clock
//! and the formed flag — lives on `BlackHoleRain`; this file answers
//! the stateless questions).
//!
//! Owner question (stage-2 verification round): should the hole
//! appear suddenly, fade in, or explode from a small dot? The
//! physically evocative answer — stellar collapse — is all three in
//! sequence: a tiny bright singularity seed fades in slowly, the
//! collapse intensifies it (the cross flare), the event horizon
//! blooms outward from the center (the implosion flash inverted
//! into an expanding rim), and then the accretion phase begins —
//! ring motes drift in from beyond the disk on the entry spiral and
//! settle onto the orbit. The whole sequence rides the same
//! wall-clock dt the engine already uses, so pause/resume and speed
//! keys behave exactly like the steady state.
//!
//! Timeline (seconds since formation start, constants in
//! style_rain.rs):
//! - [0, SEED): the seed — one glyph fading in at the viewport
//!   center, brightness ramping up the ladder.
//! - [SEED, SEED+COLLAPSE): the collapse — the seed brightens to
//!   Core and a four-cell cross flares around it (the last light
//!   before the horizon).
//! - [.., +HORIZON): the horizon bloom — the annulus grows from
//!   the inside out (photon-ring cells first, outer rim last) on a
//!   cubic ease-out.
//! - after: steady state — full ball, spawn gate opens, motes
//!   accrete onto the ring.

use super::super::monolith::BrightnessLevel;

/// The formation phase at time `t` (seconds since formation start).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FormationPhase {
    /// The singularity seed fading in at the center.
    Seed,
    /// The collapse: seed at peak brightness, cross flare.
    Collapse,
    /// The horizon bloom: annulus growing from the inside out.
    Horizon,
    /// Steady state: full ball, ring accretion running.
    Steady,
}

/// Classify the formation timeline at `t` seconds.
pub(crate) fn formation_phase(t: f32) -> FormationPhase {
    let seed = crate::constants::BLACK_HOLE_FORM_SEED_SECS;
    let collapse = seed + crate::constants::BLACK_HOLE_FORM_COLLAPSE_SECS;
    let horizon = collapse + crate::constants::BLACK_HOLE_FORM_HORIZON_SECS;
    if t < seed {
        FormationPhase::Seed
    } else if t < collapse {
        FormationPhase::Collapse
    } else if t < horizon {
        FormationPhase::Horizon
    } else {
        FormationPhase::Steady
    }
}

/// Total formation duration (seconds from seed to steady state).
pub(crate) fn formation_total_secs() -> f32 {
    crate::constants::BLACK_HOLE_FORM_SEED_SECS
        + crate::constants::BLACK_HOLE_FORM_COLLAPSE_SECS
        + crate::constants::BLACK_HOLE_FORM_HORIZON_SECS
}

/// Brightness of the seed glyph at the viewport center: ramps up
/// the ladder through the seed phase so the dot reads as fading IN
/// (Ghost -> Dim -> Mid), then jumps through the collapse (Hot ->
/// Core — the last light of the collapsing star).
pub(crate) fn seed_center_level(t: f32) -> BrightnessLevel {
    let seed = crate::constants::BLACK_HOLE_FORM_SEED_SECS;
    if t < seed {
        let p = (t / seed).clamp(0.0, 1.0);
        if p < 0.4 {
            BrightnessLevel::Ghost
        } else if p < 0.7 {
            BrightnessLevel::Dim
        } else {
            BrightnessLevel::Mid
        }
    } else {
        let collapse = crate::constants::BLACK_HOLE_FORM_COLLAPSE_SECS;
        let p = ((t - seed) / collapse).clamp(0.0, 1.0);
        if p < 0.5 {
            BrightnessLevel::Hot
        } else {
            BrightnessLevel::Core
        }
    }
}

/// Brightness of the cross-flare cells around the seed during the
/// collapse phase (Dim early, Hot at the peak — the flare reads as
/// a bloom, not a switch).
pub(crate) fn seed_cross_level(t: f32) -> BrightnessLevel {
    let seed = crate::constants::BLACK_HOLE_FORM_SEED_SECS;
    let collapse = crate::constants::BLACK_HOLE_FORM_COLLAPSE_SECS;
    let p = ((t - seed) / collapse).clamp(0.0, 1.0);
    if p < 0.5 {
        BrightnessLevel::Dim
    } else {
        BrightnessLevel::Hot
    }
}

/// The cross flare is visible only during the collapse phase (the
/// seed draws alone before it, and the horizon replaces both after).
pub(crate) fn cross_active(t: f32) -> bool {
    let seed = crate::constants::BLACK_HOLE_FORM_SEED_SECS;
    let collapse_end = seed + crate::constants::BLACK_HOLE_FORM_COLLAPSE_SECS;
    (seed..collapse_end).contains(&t)
}

/// Horizon bloom visibility: the fraction of the annulus (by
/// normalized radius, 0.0 at the event horizon, 1.0 at the outer
/// rim) that is drawn at time `t`. A cubic ease-out over the
/// HORIZON window — the rim grows fast at first (the flash) and
/// eases into its final size. Returns 1.0 in the steady state and
/// 0.0 before the bloom begins.
pub(crate) fn horizon_visibility(t: f32) -> f32 {
    let start = crate::constants::BLACK_HOLE_FORM_SEED_SECS
        + crate::constants::BLACK_HOLE_FORM_COLLAPSE_SECS;
    if t <= start {
        return 0.0;
    }
    let u = ((t - start) / crate::constants::BLACK_HOLE_FORM_HORIZON_SECS).clamp(0.0, 1.0);
    1.0 - (1.0 - u).powi(3)
}
