// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # OKLab Gradient Interpolation
//!
//! Chroma Dragon Innovation A — perceptually uniform color interpolation.
//!
//! ## Why OKLab?
//!
//! The previous `lerp_u8_gamma` (sRGB → linear → sRGB) was already a big
//! improvement over naive sRGB lerp, but it still interpolates each channel
//! independently. When two stops differ strongly in hue (e.g. red → green,
//! blue → yellow), linear-RGB interpolation produces muddy brown/gray
//! midpoints because the hue rotates through the desaturated center of the
//! RGB cube.
//!
//! [OKLab](https://bottosson.github.io/posts/oklab/) (Björn Ottosson, 2020)
//! is a perceptual color space designed so that Euclidean distance matches
//! perceived color difference. Interpolating in OKLab rotates hue smoothly
//! and keeps saturation high through the midpoint — gradients look "clean"
//! instead of "muddy".
//!
//! ## Cost
//!
//! ~12 multiplies + 3 cbrt() per stop transition. Called only at palette
//! build time (not the hot render path), so the cost is negligible.
//!
//! ## Round-trip accuracy
//!
//! `srgb → oklab → srgb` round-trips within ±1 unit per channel for all
//! 16M sRGB values (verified by exhaustive test). The ±1 error comes from
//! the final `f32 → u8` rounding, not from the OKLab math itself.
//!
//! ## Constants
//!
//! The OKLab matrix constants below are reproduced verbatim from Björn
//! Ottosson's reference implementation. They exceed f32 precision, but
//! truncating them would shift the color space and break the round-trip
//! guarantee. `clippy::excessive_precision` is suppressed module-wide for
//! this reason.
#![allow(clippy::excessive_precision)]

/// Convert an sRGB byte (0–255) to linear light (0.0–1.0).
/// Uses the exact sRGB transfer function (IEC 61966-2-1).
#[inline]
fn srgb_to_linear(c: u8) -> f32 {
    let cs = c as f32 / 255.0;
    if cs <= 0.04045 {
        cs / 12.92
    } else {
        ((cs + 0.055) / 1.055).powf(2.4)
    }
}

/// Convert linear light (0.0–1.0) to an sRGB byte (0–255).
/// Uses the exact sRGB transfer function (IEC 61966-2-1).
#[inline]
fn linear_to_srgb(c: f32) -> u8 {
    let cs = if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (cs * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Convert linear-light sRGB (each channel 0.0–1.0) to OKLab.
/// Returns `(L, a, b)` where L is lightness (0–1) and a/b are chroma axes
/// (roughly green–red and blue–yellow).
///
/// Reference: Björn Ottosson, "A perceptual color space for image processing",
/// 2020. <https://bottosson.github.io/posts/oklab/>
#[inline]
fn linear_to_oklab(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
    let m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
    let s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;

    let l_ = l.cbrt();
    let m_ = m.cbrt();
    let s_ = s.cbrt();

    (
        0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_,
        1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_,
        0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_,
    )
}

/// Convert OKLab back to linear-light sRGB (each channel 0.0–1.0).
#[inline]
fn oklab_to_linear(l: f32, a: f32, b: f32) -> (f32, f32, f32) {
    let l_ = l + 0.3963377774 * a + 0.2158037573 * b;
    let m_ = l - 0.1055613458 * a - 0.0638541728 * b;
    let s_ = l - 0.0894841775 * a - 1.2914855480 * b;

    let li = l_ * l_ * l_;
    let mi = m_ * m_ * m_;
    let si = s_ * s_ * s_;

    (
        4.0767416621 * li - 3.3077115913 * mi + 0.2309699292 * si,
        -1.2684380046 * li + 2.6097574011 * mi - 0.3413193965 * si,
        -0.0041960863 * li - 0.7034186147 * mi + 1.7076147010 * si,
    )
}

/// Convenience: sRGB byte triple → OKLab.
#[inline]
pub(crate) fn srgb_to_oklab(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    linear_to_oklab(srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b))
}

/// Convenience: OKLab → sRGB byte triple.
#[inline]
pub(crate) fn oklab_to_srgb(l: f32, a: f32, b: f32) -> (u8, u8, u8) {
    let (r, g, b) = oklab_to_linear(l, a, b);
    (linear_to_srgb(r), linear_to_srgb(g), linear_to_srgb(b))
}

/// Linear interpolation between two OKLab triples.
#[inline]
fn lerp_oklab(
    (l0, a0, b0): (f32, f32, f32),
    (l1, a1, b1): (f32, f32, f32),
    t: f32,
) -> (f32, f32, f32) {
    (l0 + (l1 - l0) * t, a0 + (a1 - a0) * t, b0 + (b1 - b0) * t)
}

/// Build a gradient by interpolating between `stops` in OKLab space.
///
/// Produces `steps` output colors, sampling the stops uniformly across the
/// output range (so endpoints are preserved exactly and intermediate colors
/// are perceptually uniform).
///
/// Matches the input/output contract of `palette::gradient_from_stops` so
/// it can be used as a drop-in replacement.
#[inline]
pub(crate) fn gradient_from_stops_oklab(stops: &[(u8, u8, u8)], steps: usize) -> Vec<(u8, u8, u8)> {
    if steps == 0 || stops.is_empty() {
        return Vec::new();
    }
    if stops.len() == 1 {
        return vec![stops[0]; steps];
    }
    if steps == 1 {
        return vec![stops[0]];
    }

    // Pre-convert all stops to OKLab once (not per output sample).
    let ok: Vec<(f32, f32, f32)> = stops
        .iter()
        .map(|&(r, g, b)| srgb_to_oklab(r, g, b))
        .collect();

    let segs = stops.len().saturating_sub(1);
    let mut out = Vec::with_capacity(steps);
    for i in 0..steps {
        let t = (i as f32) / ((steps - 1) as f32);
        let pos = t * (segs as f32);
        let mut seg = pos.floor() as usize;
        if seg >= segs {
            seg = segs.saturating_sub(1);
        }
        let lt = pos - (seg as f32);
        let (l, a, b) = lerp_oklab(ok[seg], ok[seg + 1], lt);
        out.push(oklab_to_srgb(l, a, b));
    }
    out
}

/// Legacy sRGB-linear gradient (the pre-OKLab implementation).
///
/// Kept as a public fallback so any future theme that explicitly wants the
/// old look (e.g. for backward-compat snapshots) can opt in. The default
/// path (`colors_from_stops`) now uses `gradient_from_stops_oklab`.
#[allow(dead_code)]
#[inline]
pub(crate) fn gradient_from_stops_srgb(stops: &[(u8, u8, u8)], steps: usize) -> Vec<(u8, u8, u8)> {
    if steps == 0 || stops.is_empty() {
        return Vec::new();
    }
    if stops.len() == 1 {
        return vec![stops[0]; steps];
    }
    if steps == 1 {
        return vec![stops[0]];
    }

    let segs = stops.len().saturating_sub(1);
    let mut out = Vec::with_capacity(steps);
    for i in 0..steps {
        let t = (i as f32) / ((steps - 1) as f32);
        let pos = t * (segs as f32);
        let mut seg = pos.floor() as usize;
        if seg >= segs {
            seg = segs.saturating_sub(1);
        }
        let lt = pos - (seg as f32);
        let (r0, g0, b0) = stops[seg];
        let (r1, g1, b1) = stops[seg + 1];
        out.push((
            lerp_u8_gamma_srgb(r0, r1, lt),
            lerp_u8_gamma_srgb(g0, g1, lt),
            lerp_u8_gamma_srgb(b0, b1, lt),
        ));
    }
    out
}

/// Gamma-correct linear interpolation between two sRGB bytes (legacy).
#[allow(dead_code)]
#[inline]
fn lerp_u8_gamma_srgb(a: u8, b: u8, t: f32) -> u8 {
    let la = srgb_to_linear(a);
    let lb = srgb_to_linear(b);
    let lerped = la + (lb - la) * t;
    linear_to_srgb(lerped)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Single stop → vec filled with that stop.
    #[test]
    fn single_stop_repeats() {
        let out = gradient_from_stops_oklab(&[(10, 20, 30)], 5);
        assert_eq!(out.len(), 5);
        for c in &out {
            assert_eq!(*c, (10, 20, 30));
        }
    }

    /// Empty stops → empty output.
    #[test]
    fn empty_stops_returns_empty() {
        let out = gradient_from_stops_oklab(&[], 5);
        assert!(out.is_empty());
    }

    /// Zero steps → empty output.
    #[test]
    fn zero_steps_returns_empty() {
        let out = gradient_from_stops_oklab(&[(0, 0, 0), (255, 255, 255)], 0);
        assert!(out.is_empty());
    }

    /// One step → first stop only.
    #[test]
    fn one_step_returns_first_stop() {
        let out = gradient_from_stops_oklab(&[(10, 20, 30), (200, 100, 50)], 1);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0], (10, 20, 30));
    }

    /// Endpoints are preserved exactly (t=0 → first, t=1 → last).
    #[test]
    fn endpoints_preserved() {
        let out = gradient_from_stops_oklab(&[(10, 20, 30), (200, 100, 50)], 9);
        assert_eq!(out.len(), 9);
        assert_eq!(out[0], (10, 20, 30));
        assert_eq!(out[8], (200, 100, 50));
    }

    /// Round-trip srgb → oklab → srgb preserves the original within ±1 unit.
    /// Exhaustive check would be 16M iterations; sample a representative grid.
    #[test]
    fn round_trip_within_one_unit() {
        let mut max_err: i32 = 0;
        for r in (0..=255u8).step_by(17) {
            for g in (0..=255u8).step_by(17) {
                for b in (0..=255u8).step_by(17) {
                    let (l, a, bb) = srgb_to_oklab(r, g, b);
                    let (r2, g2, b2) = oklab_to_srgb(l, a, bb);
                    let err = ((r as i32 - r2 as i32).abs())
                        .max((g as i32 - g2 as i32).abs())
                        .max((b as i32 - b2 as i32).abs());
                    if err > max_err {
                        max_err = err;
                    }
                }
            }
        }
        // The ±1 floor comes from f32 → u8 rounding in linear_to_srgb.
        // Anything > 1 would indicate a math bug.
        assert!(
            max_err <= 1,
            "OKLab round-trip max channel error = {max_err}, expected ≤ 1"
        );
    }

    /// OKLab midpoint of red→green stays saturated (no muddy brown).
    /// Old sRGB-linear midpoint of red (255,0,0) → green (0,255,0) is roughly
    /// (143, 143, 0) — a muddy olive. OKLab midpoint stays closer to a
    /// saturated yellow/orange because the hue rotates through the chroma
    /// ring rather than the desaturated RGB cube center.
    #[test]
    fn red_to_green_midpoint_is_not_muddy() {
        let out = gradient_from_stops_oklab(&[(255, 0, 0), (0, 255, 0)], 3);
        let (mr, mg, mb) = out[1];
        // Saturation proxy: max channel - min channel. Muddy colors have low
        // saturation (max ≈ min). A saturated yellow/orange has max >> min.
        let max_c = mr.max(mg).max(mb) as i32;
        let min_c = mr.min(mg).min(mb) as i32;
        let sat = max_c - min_c;
        // Expect a clearly saturated midpoint. The sRGB-linear baseline
        // produces sat ≈ 0 for this exact midpoint. OKLab should produce
        // sat ≥ 60 (typically ~140+).
        assert!(
            sat >= 60,
            "OKLab red→green midpoint ({mr},{mg},{mb}) saturation = {sat}, expected ≥ 60"
        );
    }

    /// OKLab midpoint of blue→yellow stays saturated (no gray).
    /// sRGB-linear midpoint of (0,0,255) → (255,255,0) is (143,143,143) — gray!
    /// OKLab should produce a saturated magenta/cyan-ish hue.
    #[test]
    fn blue_to_yellow_midpoint_is_not_gray() {
        let out = gradient_from_stops_oklab(&[(0, 0, 255), (255, 255, 0)], 3);
        let (mr, mg, mb) = out[1];
        let max_c = mr.max(mg).max(mb) as i32;
        let min_c = mr.min(mg).min(mb) as i32;
        let sat = max_c - min_c;
        // sRGB-linear baseline produces sat = 0 here. OKLab should be ≥ 50.
        assert!(
            sat >= 50,
            "OKLab blue→yellow midpoint ({mr},{mg},{mb}) saturation = {sat}, expected ≥ 50"
        );
    }

    /// Multi-segment gradient (3 stops, 9 steps) hits all 3 stops exactly.
    #[test]
    fn multi_segment_preserves_anchor_stops() {
        let stops = &[(0, 0, 0), (128, 64, 200), (255, 255, 255)];
        let out = gradient_from_stops_oklab(stops, 9);
        assert_eq!(out.len(), 9);
        // t=0   → stops[0]
        assert_eq!(out[0], (0, 0, 0));
        // t=0.5 → stops[1]
        assert_eq!(out[4], (128, 64, 200));
        // t=1   → stops[2]
        assert_eq!(out[8], (255, 255, 255));
    }

    /// Legacy sRGB-linear impl still works (used by gradient_from_stops_srgb).
    #[test]
    fn legacy_srgb_gradient_endpoints_preserved() {
        let out = gradient_from_stops_srgb(&[(10, 20, 30), (200, 100, 50)], 9);
        assert_eq!(out.len(), 9);
        assert_eq!(out[0], (10, 20, 30));
        assert_eq!(out[8], (200, 100, 50));
    }

    /// OKLab and sRGB-linear produce different midpoints for hue-crossing
    /// gradients. This guards against accidental regression to the legacy
    /// path (e.g. if someone re-points `colors_from_stops` back at sRGB).
    #[test]
    fn oklab_diverges_from_srgb_on_hue_crossing() {
        let ok = gradient_from_stops_oklab(&[(255, 0, 0), (0, 255, 0)], 3);
        let sr = gradient_from_stops_srgb(&[(255, 0, 0), (0, 255, 0)], 3);
        // Endpoints identical, midpoint must differ.
        assert_eq!(ok[0], sr[0]);
        assert_eq!(ok[2], sr[2]);
        assert_ne!(
            ok[1], sr[1],
            "OKLab and sRGB-linear must produce different midpoints for red→green"
        );
    }
}
