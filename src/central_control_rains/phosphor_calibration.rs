// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! # Phosphor decay calibration (night-frontier-Z4c, Lapis 4c)
//!
//! The trail afterglow was an exponential curve with a hand-tuned rate
//! (`PHOSPHOR_DECAY_RATE = 8.0`); the constant carried no provenance and
//! nothing pinned what it MEANS physically. Z4c maps the decay curve to
//! the real CRT phosphor persistence data: the rate is now DERIVED from
//! the calibrated P1-class decay time constant and one documented
//! aesthetic time dilation, the per-frame decay LUT is built by this
//! module, and the half-lives are pinned in tests.
//!
//! ## The calibration law
//!
//! The matrix-rain afterglow emulates the afterglow of the classic
//! green phosphor monochrome monitor — the P1 class (Zn2SiO4:Mn,
//! willemite, 525 nm), the phosphor of oscilloscopes and the green
//! terminals the whole aesthetic descends from. The standard P-number
//! table classifies P1 as medium persistence (the 1-100 ms band); its
//! characteristic exponential decay component sits at tau = 25 ms, so
//! the class's half-life is tau * ln(2) = 17.33 ms.
//!
//! A 25 ms afterglow is invisible at terminal scale, so the engine
//! runs PHOSPHOR TIME DILATED: one simulated second of trail decay
//! represents `PHOSPHOR_SIM_TIME_SCALE` (5x) phosphor seconds. The
//! dilation is a documented aesthetic choice — it preserves the
//! shipped trail envelope exactly, because the calibrated rate
//!
//! ```text
//! rate = 1 / (tau_P1 * time_scale) = 1 / (0.025 * 5.0) = 8.0 /s
//! ```
//!
//! is bit-identical to the hand-tuned `PHOSPHOR_DECAY_RATE = 8.0` the
//! owner shipped (v50.0.0-beta.6's cross-terminal retune). The
//! calibration does not retune the visuals — it gives the existing
//! number a physical identity, and pins the curve's shape and
//! half-lives so a future retune is a deliberate, reviewed dilation
//! change rather than a silent drift.
//!
//! ## The reference classes
//!
//! The module carries the P-designation reference classes the trail
//! could emulate (P1 green, P3 amber, P4 white, P7 radar). Their
//! constants are REAL published values, pinned by tests, so a future
//! class-aware trail (amber schemes riding P3's curve, for example) is
//! a constant change, not a research project. Honest scope: only the
//! P1 identity is load-bearing today; the shipped rate derives from
//! P1 alone, and the per-class RELATIVE rates are reference data, not
//! asserted engine behavior — public persistence tables quote decay
//! times to different brightness levels (10%, 1%), so cross-class
//! ratios carry that ambiguity and are deliberately not engine law.
//!
//! ## Cost
//!
//! The LUT builder runs once per frame per Cloud (3 layers x 2
//! variants — the same exp() calls the decay pass already made, moved
//! into one audited place). No hot-path change.

use super::parallax::{PARALLAX_LAYERS, PHOSPHOR_LAYER_DECAY_MULT};

/// A CRT phosphor reference class from the standard P-designation
/// system (P1, P3, P4, P7...), with the real published persistence
/// data the calibration cites.
#[cfg_attr(not(test), allow(dead_code))] // reference table: read by the Z4c lock tests + docs
pub(crate) struct PhosphorClass {
    /// The P-designation and short name.
    pub name: &'static str,
    /// Chemical composition, as the standard table lists it.
    pub composition: &'static str,
    /// Emitted color (dominant wavelength where the table gives one).
    pub color: &'static str,
    /// The persistence class label the standard table carries, plus
    /// the devices the class shipped in.
    pub persistence_class: &'static str,
    /// The characteristic exponential decay time constant, in seconds.
    /// A class value, not a per-tube measurement — see the module doc
    /// for the level-of-decay ambiguity the tables carry.
    pub tau_secs: f32,
}

/// P1 — THE matrix-rain class. Green willemite (Zn2SiO4:Mn, 525 nm),
/// medium persistence (the standard table's 1-100 ms band), the
/// phosphor of oscilloscopes and green monochrome monitors. Its
/// characteristic decay component: tau = 25 ms.
pub(crate) const P1_GREEN: PhosphorClass = PhosphorClass {
    name: "P1 (green willemite)",
    composition: "Zn2SiO4:Mn",
    color: "green 525 nm",
    persistence_class: "medium, 1-100 ms band (oscilloscopes, monochrome monitors)",
    tau_secs: 0.025,
};

/// P3 — amber monochrome monitors. Yellow-amber zinc-beryllium
/// silicate (602 nm), medium persistence; the standard table quotes
/// 13 ms for the class. Kept for reference; not load-bearing today.
#[cfg_attr(not(test), allow(dead_code))] // reference class: pinned by the Z4c lock tests
pub(crate) const P3_AMBER: PhosphorClass = PhosphorClass {
    name: "P3 (amber)",
    composition: "ZnBeSiO4:Mn",
    color: "yellow-amber 602 nm",
    persistence_class: "medium, ~13 ms (amber monochrome monitors)",
    tau_secs: 0.013,
};

/// P4 — black-and-white television white (ZnS:Ag + (Zn,Cd)S:Ag
/// blend), short persistence (the tens-of-microseconds class, ~60 us
/// characteristic). Kept for reference; not load-bearing today.
#[cfg_attr(not(test), allow(dead_code))] // reference class: pinned by the Z4c lock tests
pub(crate) const P4_WHITE: PhosphorClass = PhosphorClass {
    name: "P4 (white, B/W TV)",
    composition: "ZnS:Ag + (Zn,Cd)S:Ag",
    color: "white",
    persistence_class: "short, ~60 us (black-and-white television)",
    tau_secs: 0.000_06,
};

/// P7 — the radar phosphor. A dual-layer screen: a fast blue flash on
/// the inside plus a long-persistence yellow-orange layer (hundreds of
/// ms to seconds) that made radar afterglow visible. tau pins the LONG
/// layer's characteristic component. Kept for reference; not
/// load-bearing today.
#[cfg_attr(not(test), allow(dead_code))] // reference class: pinned by the Z4c lock tests
pub(crate) const P7_RADAR: PhosphorClass = PhosphorClass {
    name: "P7 (radar dual-layer)",
    composition: "(blue flash layer + long yellow-orange layer)",
    color: "blue flash / yellow-orange long layer",
    persistence_class: "long, hundreds of ms to seconds (radar displays)",
    tau_secs: 0.2,
};

/// The phosphor class the shipped trail emulates: P1 — the green
/// monochrome monitor identity the matrix-rain aesthetic descends
/// from. The shipped decay rate derives from THIS class's tau.
#[cfg_attr(not(test), allow(dead_code))] // calibration identity: pinned by the Z4c lock tests
pub(crate) const PHOSPHOR_CALIBRATED_CLASS: &PhosphorClass = &P1_GREEN;

/// The aesthetic time dilation: one simulated second of trail decay
/// represents this many phosphor seconds. Chosen so the calibrated
/// rate reproduces the shipped `PHOSPHOR_DECAY_RATE = 8.0` exactly
/// (1 / (0.025 * 5.0) = 8.0) — the owner's tuned trail envelope is
/// preserved bit-identically; what changes is that the number now
/// MEANS "the P1 curve at 5x dilation".
pub(crate) const PHOSPHOR_SIM_TIME_SCALE: f32 = 5.0;

/// The calibrated decay time constant of the emulated class, in
/// seconds (P1: 25 ms).
pub(crate) const PHOSPHOR_TAU_P1_SECS: f32 = P1_GREEN.tau_secs;

/// Half-life of an exponential decay with the given time constant:
/// `t_half = tau * ln(2)`. The pinned calibration quantity — tests
/// assert the decay LUT lands exactly on 0.5 at this age.
#[cfg_attr(not(test), allow(dead_code))] // calibration law: pinned by the Z4c lock tests
#[inline]
pub(crate) fn phosphor_half_life_secs(tau_secs: f32) -> f32 {
    tau_secs * std::f32::consts::LN_2
}

/// The calibrated per-frame decay LUT: one multiplicative brightness
/// factor per (parallax layer, bottom-band) variant.
///
/// Same exponential law the decay pass always ran —
/// `exp(-rate * layer_mult * elapsed)` with the bottom rows running
/// the accelerated rate — built here so the whole curve is the
/// calibration module's product. `base_decay` is the frame's
/// effective rate-second product (`PHOSPHOR_DECAY_RATE * decay_mult *
/// boosts * elapsed_sec`) and `bottom_base_decay` its bottom-band
/// variant, exactly as `cloud::phosphor` computed them before Z4c —
/// only the construction site moved.
#[inline]
pub(crate) fn phosphor_decay_lut(
    base_decay: f32,
    bottom_base_decay: f32,
) -> [f32; PARALLAX_LAYERS * 2] {
    let mut lut = [1.0f32; PARALLAX_LAYERS * 2];
    for (i, &lm) in PHOSPHOR_LAYER_DECAY_MULT.iter().enumerate() {
        lut[i * 2] = (-base_decay * lm).exp();
        lut[i * 2 + 1] = (-bottom_base_decay * lm).exp();
    }
    lut
}
