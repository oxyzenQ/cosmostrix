// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! # Chroma Dragon Lock — INV-20/21 (night-frontier-Z4a, measured colorimetry)
//!
//! The polar OKLab gradient's "no muddy midpoints" story was a design
//! claim (INV-10 proves the headline property on one synthetic
//! opposing-hue pair). night-frontier-Z4a promotes the claim to
//! measured colorimetry on the REAL production surfaces — all 44
//! built-in themes, the real catalog stops, the real production ramp:
//!
//! - **INV-20 (the ΔE-OK perceptual budget).** The interpolation path
//!   between consecutive stops is sampled at JND resolution (adaptive,
//!   arc-length-driven two-pass sampling, full-precision OKLab math via
//!   `gradient_path_oklab`) and every consecutive step must stay within
//!   `PALETTE_DELTA_E_OK_BUDGET` (one just-noticeable difference,
//!   ≈ 0.02 in Oklab). A mirror pin requires the shipped u8 path to
//!   equal the quantized math path within ±1 channel per sample. The
//!   shipped ramp keeps its designed discrete-band aesthetic (the
//!   bands are WIDER than one JND on purpose — 7-9 entries from trail
//!   to head), so the production ramp is separately pinned by the
//!   measured band ceiling (`PALETTE_BAND_DE_OK_CEILING`): no adjacent
//!   pair of shipped palette entries may exceed the widest band any
//!   theme ships today. The budget governs the interpolation
//!   machinery; the ceiling pins the shipped band scale.
//! - **INV-21 (no muddy midpoints, measured).** Along every saturated
//!   segment's fine path the chroma trough must keep at least
//!   `PALETTE_CHROMA_TROUGH_MIN_RATIO` (0.80) of the segment's weaker
//!   endpoint chroma, and every interior entry of the shipped ramp must
//!   keep at least `PALETTE_RAMP_TROUGH_MIN_RATIO` (0.50) of its
//!   neighbors' chroma. A midpoint that collapses toward gray is the
//!   "muddy" failure mode; it is now a measured, asserted quantity.
//!
//! Scale: ΔE-OK is the Euclidean distance in Oklab; one JND ≈ 0.02
//! ("What's My ΔE(OK) JND?" study, 2026; the ΔE00 JND ≈ 2.0 is the same
//! quantity on a 100x scale). Chroma visibility floor: segments whose
//! weaker endpoint carries less than 0.02 chroma are exempt from the
//! trough contracts — a near-gray endpoint has no saturation to lose,
//! and the polar path falls back to Cartesian there by design.

use crossterm::style::Color;

use crate::chroma_dragon_engine::catalog::{THEMES, ThemeColors};
use crate::chroma_dragon_engine::gradient::{
    delta_e_ok, gradient_from_stops_oklab, gradient_path_oklab, oklab_chroma, oklab_to_srgb,
};
use crate::chroma_dragon_engine::palette::{build_palette, color_to_rgb};
use crate::chroma_dragon_engine::tuning::{
    PALETTE_BAND_DE_OK_CEILING, PALETTE_CHROMA_TROUGH_MIN_RATIO, PALETTE_DELTA_E_OK_BUDGET,
    PALETTE_RAMP_TROUGH_MIN_RATIO, PALETTE_TROUGH_VISIBILITY_FLOOR,
};
use crate::runtime::{ColorMode, ColorScheme};

/// f32 float-noise tolerance for the continuity steps (the path math is
/// f32; a smooth path's consecutive-step drift from float rounding is
/// orders of magnitude below the budget).
const F32_PATH_EPS: f32 = 1e-4;
/// Per-channel tolerance of the u8 mirror pin (production path vs the
/// pre-quantization math, quantized) — the same ±1 law INV-8 grants the
/// OKLab round-trip.
const MIRROR_PIN_CHANNEL_EPS: i32 = 1;

/// Coarse pass resolution for the arc-length estimate (per segment).
const ARC_PASS_SAMPLES: usize = 128;
/// Headroom on the arc estimate when deriving the fine sample count
/// (finer sampling traces the curve more tightly, so the fine arc is
/// never shorter than the coarse estimate — 5% covers it with margin).
const ARC_HEADROOM: f32 = 1.05;

/// Every built-in scheme (mirrors lock.rs `all_schemes`).
fn all_schemes() -> Vec<ColorScheme> {
    use ColorScheme::*;
    vec![
        Green,
        Green2,
        Green3,
        NeonGreen,
        NeonPurple,
        NeonWhite,
        NeonBlue,
        NeonRed,
        NeonOrange,
        NeonYellow,
        NeonCyan,
        Carbon,
        Yellow,
        Orange,
        Red,
        Blue,
        Cyan,
        Gold,
        Rainbow,
        Purple,
        Neon,
        Fire,
        Ocean,
        Forest,
        Vaporwave,
        Gray,
        Snow,
        Aurora,
        FancyDiamond,
        Cosmos,
        Nebula,
        Spectrum20,
        Stars,
        Mars,
        Venus,
        EnergyZen,
        Mercury,
        Jupiter,
        Saturn,
        Uranus,
        Neptune,
        Pluto,
        Moon,
        Sun,
    ]
}

/// The built TrueColor ramp for a scheme (the shipped palette).
fn truecolor_ramp(scheme: ColorScheme) -> Vec<(u8, u8, u8)> {
    let p: crate::chroma_dragon_engine::palette::Palette =
        build_palette(scheme, ColorMode::TrueColor, true);
    p.colors.iter().map(|c: &Color| color_to_rgb(*c)).collect()
}

/// The RAW catalog stops for a scheme (the interpolation surface), if the
/// theme interpolates. RgbWithC16 themes carry no stops — None.
fn raw_stops(scheme: ColorScheme) -> Option<Vec<(u8, u8, u8)>> {
    let theme = THEMES.iter().find(|t| t.scheme == scheme)?;
    match &theme.def {
        ThemeColors::Stops { stops, .. } => Some(stops.to_vec()),
        ThemeColors::StopsWithC16 { stops, .. } => Some(stops.to_vec()),
        ThemeColors::RgbWithC16 { .. } => None,
    }
}

/// INV-20 (the ΔE-OK perceptual budget): every interpolation path
/// between consecutive catalog stops is JND-continuous in the FULL
/// PRECISION math, and the shipped u8 path is that math (mirror pin).
///
/// Two-pass adaptive sampling per segment: pass one estimates the path's
/// arc length at 128 samples; pass two samples the segment at
/// `ceil(arc * 1.05 / JND) + 1` steps so a perfectly smooth path lands
/// at or under one JND per step BY CONSTRUCTION — which is exactly what
/// makes the assertion load-bearing: any discontinuity, hue-wrap spike,
/// or overshoot artifact along the path shows up as a step far above
/// the arc-proportional expectation and fails here. The continuity
/// surface is `gradient_path_oklab` — the production sampler WITHOUT
/// the u8 round-trip — because the u8 grid quantizes coarsely near
/// black and would mask the math behind display quantization noise.
/// The mirror pin ties the claim back to what ships: every u8 sample of
/// `gradient_from_stops_oklab` must equal the quantized math sample
/// within ±1 channel per sample.
///
/// Scope, stated honestly: this governs the INTERPOLATION MACHINERY at
/// perception resolution. The shipped ramp's band widths are the
/// designed discrete aesthetic and are pinned separately (INV-20b) —
/// they are wider than one JND by design.
#[test]
fn lock_inv20_palette_transition_jnd_continuity() {
    let mut checked_segments = 0usize;
    for &scheme in &all_schemes() {
        let Some(stops) = raw_stops(scheme) else {
            continue; // RgbWithC16 themes carry no interpolation path
        };
        if stops.len() < 2 {
            continue;
        }
        for s in 0..stops.len() - 1 {
            let pair = [stops[s], stops[s + 1]];
            let d_total = delta_e_ok(
                pair[0].0, pair[0].1, pair[0].2, pair[1].0, pair[1].1, pair[1].2,
            );
            if d_total <= PALETTE_DELTA_E_OK_BUDGET {
                continue; // endpoints already within one JND — nothing to trace
            }
            // Pass 1: arc estimate at coarse resolution (the u8 path is
            // precise enough for an arc ESTIMATE).
            let coarse = gradient_from_stops_oklab(&pair, ARC_PASS_SAMPLES + 1);
            let mut arc = 0.0f32;
            for w in coarse.windows(2) {
                arc += delta_e_ok(w[0].0, w[0].1, w[0].2, w[1].0, w[1].1, w[1].2);
            }
            assert!(arc > 0.0, "theme {scheme:?} segment {s}: zero arc length");
            // Pass 2: JND-resolution sampling of the same segment, in
            // full-precision OKLab (no u8 round-trip).
            let fine_n = ((arc * ARC_HEADROOM) / PALETTE_DELTA_E_OK_BUDGET).ceil() as usize + 1;
            let fine = gradient_path_oklab(&pair, fine_n + 1);
            assert_eq!(
                fine.len(),
                fine_n + 1,
                "theme {scheme:?} segment {s}: sampler returned {fine_len} points for {fine_n} steps",
                fine_len = fine.len(),
            );
            for (k, w) in fine.windows(2).enumerate() {
                let dl = w[0].0 - w[1].0;
                let da = w[0].1 - w[1].1;
                let db = w[0].2 - w[1].2;
                let step = (dl * dl + da * da + db * db).sqrt();
                assert!(
                    step <= PALETTE_DELTA_E_OK_BUDGET + F32_PATH_EPS,
                    "theme {scheme:?} segment {s} step {k}: ΔE-OK {step:.5} exceeds the \
                     JND budget {PALETTE_DELTA_E_OK_BUDGET} — the interpolation path is not \
                     perceptually continuous (discontinuity, hue-wrap spike, or overshoot \
                     artifact)"
                );
            }
            // Mirror pin: the shipped u8 path is the quantized math path.
            let shipped = gradient_from_stops_oklab(&pair, fine_n + 1);
            for (k, ((l, a, b), &u)) in fine.iter().zip(shipped.iter()).enumerate() {
                let (r, g, b8) = oklab_to_srgb(*l, *a, *b);
                let (ur, ug, ub) = u;
                assert!(
                    (r as i32 - ur as i32).abs() <= MIRROR_PIN_CHANNEL_EPS
                        && (g as i32 - ug as i32).abs() <= MIRROR_PIN_CHANNEL_EPS
                        && (b8 as i32 - ub as i32).abs() <= MIRROR_PIN_CHANNEL_EPS,
                    "theme {scheme:?} segment {s} sample {k}: shipped u8 path ({ur},{ug},{ub}) \
                     diverges from the quantized math path ({r},{g},{b8}) beyond ±1 channel — \
                     the production sampler is not the audited math"
                );
            }
            checked_segments += 1;
        }
    }
    assert!(
        checked_segments >= 100,
        "only {checked_segments} segments traced — the theme registry shrank below the \
         audited surface; investigate before trusting this green"
    );
    println!("JND continuity traced {checked_segments} interpolation segments");
}

/// INV-20b (the shipped band ceiling): the widest adjacent-pair ΔE-OK
/// any shipped TrueColor palette may carry. This is NOT a JND claim —
/// the 7-9 entry trail→head ramp is a designed discrete-band aesthetic
/// whose bands are intentionally wider than one JND (measured worst
/// case 0.357, NeonYellow). The ceiling pins that design scale so a
/// future theme edit cannot silently introduce a cliff wider than
/// anything the catalog ships today.
#[test]
fn lock_inv20b_production_band_ceiling() {
    for &scheme in &all_schemes() {
        let ramp = truecolor_ramp(scheme);
        assert!(ramp.len() >= 2, "theme {scheme:?} ramp has < 2 entries");
        for pair in ramp.windows(2) {
            let d = delta_e_ok(
                pair[0].0, pair[0].1, pair[0].2, pair[1].0, pair[1].1, pair[1].2,
            );
            assert!(
                d <= PALETTE_BAND_DE_OK_CEILING,
                "theme {scheme:?}: adjacent ramp pair ΔE-OK {d:.5} exceeds the shipped \
                 band ceiling {PALETTE_BAND_DE_OK_CEILING} — a band wider than the designed \
                 scale; retune the theme or deliberately raise the ceiling in tuning.rs"
            );
        }
    }
}

/// INV-21a (no muddy midpoints on the interpolation path): along every
/// SATURATED segment's full-precision fine path (both endpoint chromas ≥
/// the visibility floor), the minimum interior chroma keeps at least
/// `PALETTE_CHROMA_TROUGH_MIN_RATIO` of the weaker endpoint's chroma.
/// Polar interpolation carries chroma magnitude linearly, so the
/// expected trough is ~1.0; the 0.80 floor leaves margin for float
/// noise. Measured on the pre-quantization math path for the same
/// reason as INV-20: the u8 grid would mask the math near black.
#[test]
fn lock_inv21_fine_path_no_muddy_midpoints() {
    let mut checked_segments = 0usize;
    for &scheme in &all_schemes() {
        let Some(stops) = raw_stops(scheme) else {
            continue;
        };
        if stops.len() < 2 {
            continue;
        }
        for s in 0..stops.len() - 1 {
            let c0 = oklab_chroma(stops[s].0, stops[s].1, stops[s].2);
            let c1 = oklab_chroma(stops[s + 1].0, stops[s + 1].1, stops[s + 1].2);
            let end_min = c0.min(c1);
            if end_min < PALETTE_TROUGH_VISIBILITY_FLOOR {
                continue; // near-gray segment — the Cartesian path is correct there
            }
            let fine = gradient_path_oklab(&[stops[s], stops[s + 1]], ARC_PASS_SAMPLES + 1);
            let mut worst = f32::INFINITY;
            for p in &fine[1..fine.len() - 1] {
                let c = (p.1 * p.1 + p.2 * p.2).sqrt();
                if c < worst {
                    worst = c;
                }
            }
            assert!(
                worst >= end_min * PALETTE_CHROMA_TROUGH_MIN_RATIO,
                "theme {scheme:?} segment {s}: fine-path chroma trough {worst:.5} fell below \
                 {PALETTE_CHROMA_TROUGH_MIN_RATIO}x the weaker endpoint chroma {end_min:.5} — \
                 muddy midpoint (the polar path collapsed toward gray)"
            );
            checked_segments += 1;
        }
    }
    assert!(
        checked_segments >= 50,
        "only {checked_segments} saturated segments checked — the audited surface shrank; \
         investigate before trusting this green"
    );
}

/// INV-21b (no muddy midpoints on the shipped ramp): every interior
/// entry of the shipped TrueColor ramp whose neighbors carry at least
/// the visibility floor of chroma must itself keep at least
/// `PALETTE_RAMP_TROUGH_MIN_RATIO` of the neighbors' minimum chroma.
/// This is the floor/continuity-aware form of the contract — it
/// measures what actually ships (the polar path plus the palette
/// floor), not only the raw interpolation.
#[test]
fn lock_inv21_production_ramp_no_muddy_midpoints() {
    for &scheme in &all_schemes() {
        let ramp = truecolor_ramp(scheme);
        for i in 1..ramp.len().saturating_sub(1) {
            let c_prev = oklab_chroma(ramp[i - 1].0, ramp[i - 1].1, ramp[i - 1].2);
            let c_here = oklab_chroma(ramp[i].0, ramp[i].1, ramp[i].2);
            let c_next = oklab_chroma(ramp[i + 1].0, ramp[i + 1].1, ramp[i + 1].2);
            let neighbor_min = c_prev.min(c_next);
            if neighbor_min < PALETTE_TROUGH_VISIBILITY_FLOOR {
                continue; // gray-adjacent entry — no saturation to preserve
            }
            assert!(
                c_here >= neighbor_min * PALETTE_RAMP_TROUGH_MIN_RATIO,
                "theme {scheme:?} ramp[{i}]: chroma {c_here:.5} fell below \
                 {PALETTE_RAMP_TROUGH_MIN_RATIO}x the neighbor minimum {neighbor_min:.5} — \
                 the shipped palette carries a muddy midpoint"
            );
        }
    }
}

/// The Z4a audit harness (the re-derivation table): prints the per-theme
/// worst-case adjacent ΔE-OK step, the fine-path worst step, and the
/// chroma troughs across all 44 themes, so any future retune can
/// re-derive the budgets from the same table. Asserts nothing (the
/// contracts live in the INV tests above) — this is the measurement
/// record that produced them.
#[test]
fn z4a_measure_delta_e_and_chroma_trough_across_themes() {
    let mut global_max_step = 0.0f32;
    let mut global_max_step_theme = String::new();
    let mut global_min_trough_ratio = f32::INFINITY;
    let mut global_min_trough_theme = String::new();
    let mut global_max_fine_step = 0.0f32;
    let mut global_max_fine_theme = String::new();
    let mut global_min_seg_trough = f32::INFINITY;
    let mut global_min_seg_trough_theme = String::new();

    for &scheme in &all_schemes() {
        let ramp = truecolor_ramp(scheme);
        let mut theme_max_step = 0.0f32;
        for pair in ramp.windows(2) {
            let d = delta_e_ok(
                pair[0].0, pair[0].1, pair[0].2, pair[1].0, pair[1].1, pair[1].2,
            );
            if d > theme_max_step {
                theme_max_step = d;
            }
        }
        if theme_max_step > global_max_step {
            global_max_step = theme_max_step;
            global_max_step_theme = format!("{scheme:?}");
        }
        for i in 1..ramp.len() - 1 {
            let c_prev = oklab_chroma(ramp[i - 1].0, ramp[i - 1].1, ramp[i - 1].2);
            let c_here = oklab_chroma(ramp[i].0, ramp[i].1, ramp[i].2);
            let c_next = oklab_chroma(ramp[i + 1].0, ramp[i + 1].1, ramp[i + 1].2);
            let neighbor_min = c_prev.min(c_next);
            if neighbor_min < PALETTE_TROUGH_VISIBILITY_FLOOR {
                continue;
            }
            let ratio = if neighbor_min > 0.0 {
                c_here / neighbor_min
            } else {
                f32::INFINITY
            };
            if ratio < global_min_trough_ratio {
                global_min_trough_ratio = ratio;
                global_min_trough_theme = format!("{scheme:?}[{i}]");
            }
        }
        println!(
            "{scheme:<14?} max_step_dEok={:.5} ramp_len={}",
            theme_max_step,
            ramp.len()
        );

        if let Some(stops) = raw_stops(scheme)
            && stops.len() >= 2
        {
            let segs = stops.len() - 1;
            let fine_steps = segs * ARC_PASS_SAMPLES + 1;
            let path = gradient_path_oklab(&stops, fine_steps);
            assert_eq!(path.len(), fine_steps);
            for pair in path.windows(2) {
                let dl = pair[0].0 - pair[1].0;
                let da = pair[0].1 - pair[1].1;
                let db = pair[0].2 - pair[1].2;
                let d = (dl * dl + da * da + db * db).sqrt();
                if d > global_max_fine_step {
                    global_max_fine_step = d;
                    global_max_fine_theme = format!("{scheme:?}");
                }
            }
            for s in 0..segs {
                let lo = s * ARC_PASS_SAMPLES;
                let c0 = oklab_chroma(stops[s].0, stops[s].1, stops[s].2);
                let c1 = oklab_chroma(stops[s + 1].0, stops[s + 1].1, stops[s + 1].2);
                let end_min = c0.min(c1);
                if end_min < PALETTE_TROUGH_VISIBILITY_FLOOR {
                    continue;
                }
                let mut worst = f32::INFINITY;
                for k in 1..ARC_PASS_SAMPLES {
                    let p = &path[lo + k];
                    let c = (p.1 * p.1 + p.2 * p.2).sqrt();
                    if c < worst {
                        worst = c;
                    }
                }
                let ratio = worst / end_min;
                if ratio < global_min_seg_trough {
                    global_min_seg_trough = ratio;
                    global_min_seg_trough_theme = format!("{scheme:?} seg{s}");
                }
            }
        }
    }

    println!("== Z4a measurement summary ==");
    println!("WORST shipped adjacent ΔE-OK step: {global_max_step:.5} on {global_max_step_theme}");
    println!(
        "WORST shipped-ramp chroma trough ratio: {global_min_trough_ratio:.4} at {global_min_trough_theme}"
    );
    println!(
        "WORST fine-path ΔE-OK step (128/segment): {global_max_fine_step:.6} on {global_max_fine_theme}"
    );
    println!(
        "WORST per-segment fine-path chroma trough: {global_min_seg_trough:.4} at {global_min_seg_trough_theme}"
    );
}
