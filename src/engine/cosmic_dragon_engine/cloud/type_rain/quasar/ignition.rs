// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Quasar ignition intro (NIGHT-research-8, law 0): the pure phase
//! math for the engine's birth sequence, split the way the DNA
//! helix splits its `genesis.rs` (the state machine's mutable
//! half — the ignition clock and the lit flag — lives on
//! `QuasarRain`; this file answers the stateless questions).
//!
//! Owner mandate (the DNA genesis set the bar): a style must be
//! BORN, not popped in. The quasar's origin story is the
//! ignition, in four continuous phases (each continuous in time
//! with the next — no pop at any seam):
//!
//! - Dark: the cold cloud falls (the infall runs its ignition
//!   multiplier — the broth is thicker than the steady drizzle
//!   because while the engine is dark the infall IS the scene,
//!   the DNA soup precedent). Nothing is lit: no core, no disk,
//!   no halo pulse.
//! - Disk: the captured streamers circularize (the disk
//!   condenses ring by ring from the rain itself), capped at Dim
//!   — warm dust, no light.
//! - Light: the core ignites (first light — the Core-bright
//!   engine cell and its glow ring appear) and the luminosity
//!   ramps to full; the doppler asymmetry fades in with it.
//! - Jets: the beams push out (the jet front travels 0 -> 1 of
//!   the beam; the stream's particles ride below the front). At
//!   the front's arrival the geometry evaluates exactly to the
//!   steady law — the last frame of the ignition and the first
//!   frame of the steady state are identical, no seam.
//!
//! Timeline (sim-seconds since ignition start, constants in
//! style_rain.rs; the clock rides the engine's sim time — the
//! family speed contract scales the birth with the engine, the
//! DNA genesis precedent):
//! - [0, DARK): the cold cloud.
//! - [DARK, DARK+DISK): the disk condenses (the circularization
//!   itself is the particles' law-2 damping; this phase gates the
//!   capture and the brightness cap).
//! - [DARK+DISK, +LIGHT): first light (the luminosity fraction
//!   0 -> 1, linear).
//! - [+LIGHT, +JET): the jets extend (the front 0 -> 1, linear).
//! - after: steady (the lit flag flips; the queries return the
//!   full laws exactly — the final front is the full beam).

/// The ignition phase at sim-time `t` since the sequence began.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IgnitionPhase {
    /// The dark cloud: the thick cold infall, nothing lit.
    Dark,
    /// The disk condenses: captures circularize, capped at Dim.
    Disk,
    /// First light: the core ignites, the luminosity ramps.
    Light,
    /// The jets extend: the front travels to the full beam.
    Jets,
    /// Steady state: the full engine under laws 1 through 5.
    Steady,
}

/// Classify the ignition timeline at sim-time `t`.
pub(crate) fn ignition_phase(t: f32) -> IgnitionPhase {
    let dark = crate::constants::QUAS_IGNITION_DARK_SECS;
    let disk = dark + crate::constants::QUAS_IGNITION_DISK_SECS;
    let light = disk + crate::constants::QUAS_IGNITION_LIGHT_SECS;
    let jets = light + crate::constants::QUAS_IGNITION_JET_SECS;
    if t < dark {
        IgnitionPhase::Dark
    } else if t < disk {
        IgnitionPhase::Disk
    } else if t < light {
        IgnitionPhase::Light
    } else if t < jets {
        IgnitionPhase::Jets
    } else {
        IgnitionPhase::Steady
    }
}

/// Total ignition duration (sim-seconds from the first cold
/// streamer to the steady engine).
pub(crate) fn ignition_total_secs() -> f32 {
    crate::constants::QUAS_IGNITION_DARK_SECS
        + crate::constants::QUAS_IGNITION_DISK_SECS
        + crate::constants::QUAS_IGNITION_LIGHT_SECS
        + crate::constants::QUAS_IGNITION_JET_SECS
}

/// The luminosity fraction in [0, 1] at sim-time `t` (law 1's
/// ramp): 0 through the dark and disk phases (nothing burns), a
/// linear ramp over the first-light window, 1 from the jet phase
/// on (the steady engine burns at full law). The doppler
/// asymmetry and the halo's pulse ride the same fraction — the
/// whole engine's light arrives together.
pub(crate) fn luminosity(t: f32) -> f32 {
    let start =
        crate::constants::QUAS_IGNITION_DARK_SECS + crate::constants::QUAS_IGNITION_DISK_SECS;
    let dur = crate::constants::QUAS_IGNITION_LIGHT_SECS;
    if t <= start {
        return 0.0;
    }
    if t >= start + dur {
        return 1.0;
    }
    (t - start) / dur
}

/// The jet front in [0, 1] at sim-time `t` (law 4's extension):
/// 0 through first light, a linear push over the jet window, 1
/// in the steady state. The beam's particles clip below the
/// front during the extension (the stream piles into the pushing
/// tip — the read of a beam forcing its way out), and the front's
/// arrival is the steady beam: no seam.
pub(crate) fn jet_front(t: f32) -> f32 {
    let start = crate::constants::QUAS_IGNITION_DARK_SECS
        + crate::constants::QUAS_IGNITION_DISK_SECS
        + crate::constants::QUAS_IGNITION_LIGHT_SECS;
    let dur = crate::constants::QUAS_IGNITION_JET_SECS;
    if t <= start {
        return 0.0;
    }
    if t >= start + dur {
        return 1.0;
    }
    (t - start) / dur
}

// The shipped calibration must stay internally consistent — the
// compile-time checks mirror the engine's other constant
// contracts (the phase classifier's ordering depends on every
// window being positive: a zero window would make a phase
// unreachable, a negative one would mis-order the timeline).
const _: () = assert!(crate::constants::QUAS_IGNITION_DARK_SECS > 0.0);
const _: () = assert!(crate::constants::QUAS_IGNITION_DISK_SECS > 0.0);
const _: () = assert!(crate::constants::QUAS_IGNITION_LIGHT_SECS > 0.0);
const _: () = assert!(crate::constants::QUAS_IGNITION_JET_SECS > 0.0);
