// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! # Chroma Dragon Lock — INV-22 (night-frontier-Z5b, the afterglow
//! temporal colorimetry law)
//!
//! Z4a (INV-20/21) pinned the SPATIAL colorimetry: the palette
//! interpolation paths. Z4c (Lapis 4c) pinned the phosphor decay's
//! SHAPE (the P1-class curve, the half-lives). Nothing pinned the
//! TEMPORAL colorimetry — what the decay actually DOES to emitted
//! color, frame over frame, in perceptual units. INV-22 closes that
//! gap with the measured law of the shipped decay path; every bound
//! was measured against the real pipeline
//! (`phosphor_calibration::phosphor_decay_lut` +
//! `palette::apply_brightness_rgb` — the exact code the render path
//! runs), never against a reimplementation.
//!
//! The structure the law sits on: the per-cell decay is a u8 ENERGY
//! ladder (`floor(energy * exp(-rate * dt))` per frame, death at
//! `<= PHOSPHOR_DEAD_THRESHOLD`), and the rendered color is the
//! pristine base color re-scaled by `energy / 255` EVERY frame —
//! never an iterated color rounding. The colorimetry of the trail is
//! therefore the colorimetry of the energy ladder plus one fixed-point
//! scale per frame, and that is what these pins measure:
//!
//! 1. Monotone + terminating ladder. Every emitted channel is
//!    non-increasing frame over frame and every path terminates — the
//!    trail can never blip brighter as it dies and can never hang
//!    forever.
//! 2. The nominal melt envelope. The per-step delta-E-OK of the melt
//!    at the reference tuning (decay_mult 1.0, no boost, no bottom
//!    band) is DESIGNED to be supra-JND — the trail is a visible fast
//!    melt, not an invisible drift. The twelve-entry table pins the
//!    worst step per (parallax layer, frame pacing), in BOTH
//!    directions: a retune that breaks a ceiling is a conscious
//!    envelope change, and so is one that collapses the melt.
//! 3. The load corners. With the self-healer boosts stacked
//!    (1.2 x 1.5) the melt steps grow; with the bottom band stacked on
//!    top (x 1.8) the emergency collapse reaches ~0.74 per step — the
//!    documented survival trade (shorter trails = performance
//!    headroom). Pinned so the emergency read stays bounded and known.
//! 4. The death pop. The last alive energy (7) renders a ghost ~0.13
//!    delta-E-OK from the black background before the cell blanks — a
//!    designed, bounded pop (dying at energy 1 would extend a
//!    near-invisible tail for one imperceptible frame's sake).
//! 5. The floor pop. A step that vanishes the emitted color to black
//!    is bounded by the measured global worst 0.1286 (the u8 foot:
//!    Oklab's near-black nonlinearity makes the last quanta register
//!    large; the 0.02 JND reference is a mid-tone figure). At the
//!    nominal corners the worst is the (1,1,1) -> black step at 0.067;
//!    the extreme bottom+boost corner merges the floor pop with the
//!    melt collapse up to the shared 0.13 class.
//! 6. The hue-stability law (nominal envelope). Through the
//!    VISIBLY COLORED regime (chroma > 0.06) the trail's hue is
//!    stable: per-step drift is pinned <= 10 degrees (measured worst
//!    9.0 at the 24 fps corner) and the NET visible-life shift is
//!    pinned <= 21 degrees (measured worst 20.2), concentrated at the
//!    deep-blue u8 foot where the blue channel's quantization is
//!    coarsest — the medium's documented property, now a measured
//!    bound. Bright-chroma paths (the perceptually dominant melt)
//!    drift far less; the chroma-MAGNITUDE shrinkage is the physics
//!    of dimming and rides the melt envelope pins (2/3), not this
//!    pin.

use crate::chroma_dragon_engine::gradient::{delta_e_ok, srgb_to_oklab};
use crate::chroma_dragon_engine::palette::apply_brightness_rgb;
use crate::constants::{
    PHOSPHOR_BOTTOM_DECAY_MULT, PHOSPHOR_DECAY_RATE, PHOSPHOR_LAYER_DECAY_MULT, phosphor_decay_lut,
};

/// The visible-chroma floor for the chroma-plane law: below this the
/// u8 foot's hue geometry dominates and the pin's premise (a colored
/// step) no longer holds. (The JND budget itself is INV-20's
/// `PALETTE_DELTA_E_OK_BUDGET`; the chroma-plane pin sits at that
/// line — see pin 6.)
const VISIBLE_CHROMA_FLOOR: f32 = 0.06;

/// The scene fps ladder the engine actually runs (ambient churn
/// 24-60, scenes to 120; the legal config floor of 1 fps is the
/// deliberate slow-cadence regime, outside the interactive envelope).
const FPS_LADDER: [f32; 4] = [120.0, 60.0, 30.0, 24.0];

/// The boost-worst corner: pressure boost 1.2 x throttle boost 1.5
/// (the two compose multiplicatively — see the phosphor pressure
/// comment in `cloud/phosphor.rs`).
const BOOST_WORST: f32 = 1.2 * 1.5;

/// Stride-8 sweep of the u8 cube: 32768 colors covering every octant
/// and every channel geometry the decay can touch. Custom palettes
/// make the full cube the honest domain; a stride sweep measured the
/// same worst cases as finer strides — the melt worst lives on the
/// bright grey boundary, well inside this grid.
fn cube_stride8() -> impl Iterator<Item = (u8, u8, u8)> {
    (0..=255u8).step_by(8).flat_map(|r| {
        (0..=255u8)
            .step_by(8)
            .flat_map(move |g| (0..=255u8).step_by(8).map(move |b| (r, g, b)))
    })
}

/// Stride-16 sweep: the coarser grid the per-energy floor-pop walk
/// uses (16 corners x 255 energy levels x this grid is the affordable
/// honest sweep; the worst cases it finds are the same the stride-8
/// grid finds — the pop lives on the grey foot).
fn cube_stride16() -> impl Iterator<Item = (u8, u8, u8)> {
    (0..=255u8).step_by(16).flat_map(|r| {
        (0..=255u16)
            .step_by(16)
            .flat_map(move |g| (0..=255u16).step_by(16).map(move |b| (r, g as u8, b as u8)))
    })
}

/// One frame's color step, exactly as the render path runs it: the
/// pristine base re-scaled by the decayed energy ratio.
fn step_color(base: (u8, u8, u8), energy: u8) -> (u8, u8, u8) {
    match apply_brightness_rgb(base.0, base.1, base.2, f32::from(energy) / 255.0) {
        crossterm::style::Color::Rgb { r, g, b } => (r, g, b),
        _ => unreachable!("apply_brightness_rgb always builds Color::Rgb"),
    }
}

/// One frame's color step from the raw scale factor (the envelope
/// pins walk the factor directly, no energy ladder needed).
fn step_factor(base: (u8, u8, u8), factor: f32) -> (u8, u8, u8) {
    match apply_brightness_rgb(base.0, base.1, base.2, factor) {
        crossterm::style::Color::Rgb { r, g, b } => (r, g, b),
        _ => unreachable!("apply_brightness_rgb always builds Color::Rgb"),
    }
}

/// Pin 1: monotone + terminating, across the cube x layers x the fps
/// ladder — both the energy ladder and the emitted colors.
#[test]
fn inv22_ladder_is_monotone_and_terminates() {
    for &layer_mult in PHOSPHOR_LAYER_DECAY_MULT.iter() {
        for &fps in FPS_LADDER.iter() {
            let dt = 1.0 / fps;
            let factor = (-(PHOSPHOR_DECAY_RATE * layer_mult) * dt).exp();
            for (r, g, b) in cube_stride8() {
                // the energy ladder, exactly as the phosphor pass walks it
                let mut energy = 255u8;
                let mut steps = 0usize;
                let mut prev = step_color((r, g, b), energy);
                while energy > 6 {
                    energy = (energy as f32 * factor) as u8;
                    let cur = step_color((r, g, b), energy);
                    assert!(
                        cur.0 <= prev.0 && cur.1 <= prev.1 && cur.2 <= prev.2,
                        "color blipped brighter during decay: base ({r},{g},{b}) \
                         {prev:?} -> {cur:?} at energy {energy} (layer {layer_mult}, fps {fps})"
                    );
                    prev = cur;
                    steps += 1;
                    assert!(
                        steps < 600,
                        "ladder must terminate: layer {layer_mult} fps {fps} \
                         exceeded 600 frames"
                    );
                }
            }
        }
    }
}

/// Pin 2: the nominal melt envelope — the designed supra-JND per-step
/// delta-E-OK at the reference tuning, worst color per (layer, fps).
/// Reference probe values measured off-tree (f64, stride-8 cube); the
/// pin carries a 10 percent + 0.005 band in BOTH directions.
#[test]
fn inv22_nominal_melt_envelope_pinned() {
    // (layer_mult, fps, reference probe worst, pinned ceiling)
    let table: [(f32, f32, f64, f32); 12] = [
        (1.9, 120.0, 0.0908, 0.105),
        (1.9, 60.0, 0.1713, 0.193),
        (1.9, 30.0, 0.3093, 0.345),
        (1.9, 24.0, 0.3691, 0.411),
        (1.15, 120.0, 0.0545, 0.064),
        (1.15, 60.0, 0.1092, 0.124),
        (1.15, 30.0, 0.2029, 0.227),
        (1.15, 24.0, 0.2443, 0.273),
        (0.65, 120.0, 0.0330, 0.040),
        (0.65, 60.0, 0.0637, 0.075),
        (0.65, 30.0, 0.1215, 0.137),
        (0.65, 24.0, 0.1463, 0.165),
    ];
    for &(layer_mult, fps, measured, bound) in table.iter() {
        let factor = (-(PHOSPHOR_DECAY_RATE * layer_mult) * (1.0 / fps)).exp();
        let mut worst: f32 = 0.0;
        for (r, g, b) in cube_stride8() {
            let cur = step_factor((r, g, b), factor);
            worst = worst.max(delta_e_ok(r, g, b, cur.0, cur.1, cur.2));
        }
        assert!(
            worst <= bound,
            "nominal melt envelope exceeded: layer {layer_mult} @ {fps}fps \
             measured {worst:.4} > pinned {bound:.3} (reference probe {measured})"
        );
        assert!(
            worst >= measured as f32 * 0.90,
            "nominal melt envelope collapsed: layer {layer_mult} @ {fps}fps \
             measured {worst:.4} < 90% of the reference {measured} — \
             the designed melt regressed"
        );
    }
}

/// Pin 3: the load corners — the stacked self-healer boosts on the
/// fastest (back) layer, and the bottom band stacked on top of that
/// (the emergency collapse). The trail shortens under load; the melt
/// steps grow to the pinned bounds and no further.
#[test]
fn inv22_load_corner_envelope_pinned() {
    let layer_mult = PHOSPHOR_LAYER_DECAY_MULT[0]; // back layer: the fastest
    // (rate product over PHOSPHOR_DECAY_RATE, fps, reference worst, bound)
    let table: [(f32, f32, f64, f32); 8] = [
        (BOOST_WORST, 120.0, 0.1557, 0.176),
        (BOOST_WORST, 60.0, 0.2831, 0.316),
        (BOOST_WORST, 30.0, 0.4794, 0.533),
        (BOOST_WORST, 24.0, 0.5552, 0.616),
        (
            BOOST_WORST * PHOSPHOR_BOTTOM_DECAY_MULT,
            120.0,
            0.2604,
            0.291,
        ),
        (
            BOOST_WORST * PHOSPHOR_BOTTOM_DECAY_MULT,
            60.0,
            0.4444,
            0.494,
        ),
        (
            BOOST_WORST * PHOSPHOR_BOTTOM_DECAY_MULT,
            30.0,
            0.6739,
            0.747,
        ),
        (
            BOOST_WORST * PHOSPHOR_BOTTOM_DECAY_MULT,
            24.0,
            0.7398,
            0.820,
        ),
    ];
    for &(rate_mult, fps, measured, bound) in table.iter() {
        let factor = (-(PHOSPHOR_DECAY_RATE * layer_mult * rate_mult) * (1.0 / fps)).exp();
        let mut worst: f32 = 0.0;
        for (r, g, b) in cube_stride8() {
            let cur = step_factor((r, g, b), factor);
            worst = worst.max(delta_e_ok(r, g, b, cur.0, cur.1, cur.2));
        }
        assert!(
            worst <= bound,
            "load-corner melt envelope exceeded: x{rate_mult} @ {fps}fps \
             measured {worst:.4} > pinned {bound:.3} (reference probe {measured})"
        );
    }
}

/// Pin 4: the death pop — the last alive energy (7) renders a ghost
/// whose distance to the black background is bounded (worst color on
/// the cube; reference probe 0.1286).
#[test]
fn inv22_death_step_pinned() {
    let mut worst: f32 = 0.0;
    for (r, g, b) in cube_stride8() {
        let dying = step_color((r, g, b), 7);
        worst = worst.max(delta_e_ok(dying.0, dying.1, dying.2, 0, 0, 0));
    }
    assert!(
        worst <= 0.135,
        "death pop exceeded the designed bound: {worst:.4} > 0.135 \
         (reference probe 0.1286)"
    );
}

/// Pin 5: the floor pop — any step whose dimmer endpoint IS black is
/// bounded by the measured global worst across the corner set
/// (0.1286, the bottom+boost emergency corner; 0.067 at the nominal
/// corners, the (1,1,1) -> black last-quantum step).
#[test]
fn inv22_floor_pop_pinned() {
    let mut corners: Vec<(f32, f32)> = Vec::new();
    for &layer_mult in PHOSPHOR_LAYER_DECAY_MULT.iter() {
        for &fps in FPS_LADDER.iter() {
            corners.push((PHOSPHOR_DECAY_RATE * layer_mult, 1.0 / fps));
        }
    }
    corners.push((
        PHOSPHOR_DECAY_RATE * PHOSPHOR_LAYER_DECAY_MULT[0] * BOOST_WORST,
        1.0 / 24.0,
    ));
    corners.push((
        PHOSPHOR_DECAY_RATE
            * PHOSPHOR_LAYER_DECAY_MULT[0]
            * BOOST_WORST
            * PHOSPHOR_BOTTOM_DECAY_MULT,
        1.0 / 24.0,
    ));
    for &(rate, dt) in corners.iter() {
        let factor = (-rate * dt).exp();
        for (r, g, b) in cube_stride16() {
            for e_prev in 1..=255u8 {
                let prev = step_color((r, g, b), e_prev);
                if prev == (0, 0, 0) {
                    continue;
                }
                let e_next = (e_prev as f32 * factor) as u8;
                if e_next >= e_prev {
                    continue;
                }
                let cur = step_color((r, g, b), e_next);
                if cur == (0, 0, 0) {
                    let d = delta_e_ok(prev.0, prev.1, prev.2, 0, 0, 0);
                    assert!(
                        d <= 0.14,
                        "floor pop exceeded the measured bound: base ({r},{g},{b}) \
                         {prev:?} -> black d={d:.4} > 0.14 (rate {rate}, dt {dt})"
                    );
                }
            }
        }
    }
}

/// Pin 6: the hue-stability law at the nominal envelope — through the
/// visibly colored regime the trail's hue is stable: per-step drift
/// <= 10 degrees (measured worst 9.0), net visible-life shift <= 21
/// degrees (measured worst 20.2, the deep-blue u8 foot).
#[test]
fn inv22_hue_is_stable_through_the_visible_melt() {
    for &layer_mult in PHOSPHOR_LAYER_DECAY_MULT.iter() {
        for &fps in FPS_LADDER.iter() {
            let dt = 1.0 / fps;
            let factor = (-(PHOSPHOR_DECAY_RATE * layer_mult) * dt).exp();
            for (r, g, b) in cube_stride8() {
                let mut energy = 255u8;
                let mut first_hue: Option<f32> = None;
                let mut last_hue: Option<f32> = None;
                let mut prev_hue: Option<f32> = None;
                let mut worst_step: f32 = 0.0;
                while energy > 6 {
                    energy = (energy as f32 * factor) as u8;
                    let cur = step_color((r, g, b), energy);
                    let (_, a, b) = srgb_to_oklab(cur.0, cur.1, cur.2);
                    let chroma = (a * a + b * b).sqrt();
                    if chroma > VISIBLE_CHROMA_FLOOR {
                        let hue = b.atan2(a).to_degrees();
                        if let Some(prev) = prev_hue {
                            let d = (hue - prev).abs();
                            let d = d.min(360.0 - d);
                            worst_step = worst_step.max(d);
                        }
                        if first_hue.is_none() {
                            first_hue = Some(hue);
                        }
                        last_hue = Some(hue);
                        prev_hue = Some(hue);
                    }
                }
                assert!(
                    worst_step <= 10.0,
                    "per-step hue drift exceeded the pin: base ({r},{g},{b}) \
                     drift {worst_step:.2} deg > 10 (layer {layer_mult}, fps {fps})"
                );
                if let (Some(first), Some(last)) = (first_hue, last_hue) {
                    let net = (last - first).abs();
                    let net = net.min(360.0 - net);
                    assert!(
                        net <= 21.0,
                        "net visible-life hue shift exceeded the pin: \
                         base ({r},{g},{b}) shift {net:.2} deg > 21 \
                         (layer {layer_mult}, fps {fps})"
                    );
                }
            }
        }
    }
}

/// The LUT contract rides through: the calibration module's product
/// must equal the raw exponential law the pins above assume
/// (`exp(-rate * mult * elapsed)`), layer by layer — one audited
/// construction site (Z4c), asserted here so the temporal pins and the
/// LUT can never drift apart.
#[test]
fn inv22_decay_lut_matches_the_exponential_law() {
    let base = PHOSPHOR_DECAY_RATE * 0.05; // one nominal frame at 20 fps
    let bottom = base * PHOSPHOR_BOTTOM_DECAY_MULT;
    let lut = phosphor_decay_lut(base, bottom);
    for (i, &lm) in PHOSPHOR_LAYER_DECAY_MULT.iter().enumerate() {
        let expected = (-(base * lm)).exp();
        assert!(
            (lut[i * 2] - expected).abs() < 1e-6,
            "LUT row {i} drifted from the exponential law: {} vs {expected}",
            lut[i * 2]
        );
        let expected_bottom = (-(bottom * lm)).exp();
        assert!(
            (lut[i * 2 + 1] - expected_bottom).abs() < 1e-6,
            "LUT bottom row {i} drifted: {} vs {expected_bottom}",
            lut[i * 2 + 1]
        );
    }
}
