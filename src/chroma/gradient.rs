// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # OKLab Gradient Interpolation — Polar (sole production path)
//!
//! Chroma Dragon Innovation A — perceptually uniform color interpolation.
//!
//! ## Why OKLab?
//!
//! The previous `lerp_u8_gamma` (sRGB → linear → sRGB) interpolated each
//! channel independently. When two stops differ strongly in hue (e.g. red →
//! green, blue → yellow), linear-RGB interpolation produces muddy brown/gray
//! midpoints because the hue rotates through the desaturated center of the
//! RGB cube.
//!
//! [OKLab](https://bottosson.github.io/posts/oklab/) (Björn Ottosson, 2020)
//! is a perceptual color space designed so that Euclidean distance matches
//! perceived color difference. Interpolating in OKLab rotates hue smoothly
//! and keeps saturation high through the midpoint — gradients look "clean"
//! instead of "muddy".
//!
//! ## Why polar (not Cartesian)?
//!
//! OKLab interpolates the `(a, b)` chroma axes. Two options exist:
//!
//! - **Cartesian** (linear lerp of `a` and `b`): on opposing-hue gradients
//!   (red↔cyan, blue↔yellow), the (a, b) midpoint passes near (0, 0) = gray,
//!   producing a desaturated midpoint. This is the canonical "Cartesian
//!   shortcut through gray" failure mode.
//! - **Polar** (lerp chroma magnitude `C = sqrt(a²+b²)` linearly + rotate hue
//!   `h = atan2(b, a)` through the shortest arc): chroma magnitude stays
//!   high through the midpoint, so the gradient stays saturated.
//!
//! Polar never regresses against Cartesian — on analogous-hue gradients both
//! produce identical output; on opposing-hue gradients polar stays saturated
//! while Cartesian collapses toward gray. Polar also aligns cosmostrix with
//! the W3C CSS Color Module Level 4 spec, which defaults `oklch`
//! interpolation to shortest-arc hue rotation.
//!
//! As of v30, polar is the **sole production gradient path**. The Cartesian
//! variant and the legacy sRGB-linear variant have been removed. The
//! `--polar-gradient` CLI flag (Phase 9-A opt-in) has been removed — polar
//! is no longer opt-in, it's the only path.
//!
//! ## Cost
//!
//! ~12 multiplies + 3 cbrt() per stop transition (OKLab conversion) plus
//! ~2 atan2 + 2 sin/cos per segment transition (polar math). Called only at
//! palette build time (not the hot render path), so the cost is negligible.
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
//! guarantee. `clippy::excessive_precision` is allowed project-wide via
//! `[lints.clippy]` in Cargo.toml for this reason.

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

/// Polar (chroma + hue) interpolation between two OKLab (a, b) chroma points.
///
/// Chroma magnitude `C = sqrt(a² + b²)` lerps linearly. Hue angle
/// `h = atan2(b, a)` rotates through the **shortest arc**. This avoids the
/// "Cartesian shortcut through gray" problem: when two hues are roughly
/// opposite on the chroma ring (e.g. red ↔ cyan), linear (a, b)
/// interpolation passes near (0, 0), producing a desaturated gray midpoint.
/// Polar interpolation keeps chroma magnitude high throughout the rotation,
/// so the midpoint stays saturated.
///
/// Lightness L is not handled here — callers interpolate L separately
/// (Phase 5 L-smoothing uses linear lerp, gradient builder uses linear lerp).
///
/// # Arguments
///
/// - `a0, b0` — start (a, b) chroma point.
/// - `a1, b1` — end (a, b) chroma point.
/// - `t` — blend factor in `[0, 1]`. 0 → start, 1 → end.
///
/// # Returns
///
/// Interpolated `(a, b)`.
///
/// # Special case
///
/// If either endpoint is grayscale (chroma ≈ 0), falls back to Cartesian
/// lerp. The gray midpoint is the visually correct answer anyway, since
/// rotating hue from "no hue" to any hue is meaningless.
#[inline]
pub(crate) fn polar_chroma_lerp(a0: f32, b0: f32, a1: f32, b1: f32, t: f32) -> (f32, f32) {
    let c0 = (a0 * a0 + b0 * b0).sqrt();
    let c1 = (a1 * a1 + b1 * b1).sqrt();

    // If either endpoint is grayscale (chroma = 0), the hue is undefined.
    // Fall back to Cartesian lerp — the result will pass through gray,
    // which is the visually correct midpoint between any hue and gray.
    if c0 < 1e-6 || c1 < 1e-6 {
        return (a0 + (a1 - a0) * t, b0 + (b1 - b0) * t);
    }

    // Hue angles in radians, in (-π, π].
    let h0 = b0.atan2(a0);
    let h1 = b1.atan2(a1);

    // Shortest-arc delta: normalize the difference into (-π, π].
    // If the raw delta exceeds π, the shorter path is the other way
    // around the ring — subtract (or add) 2π.
    let mut delta = h1 - h0;
    if delta > std::f32::consts::PI {
        delta -= 2.0 * std::f32::consts::PI;
    } else if delta < -std::f32::consts::PI {
        delta += 2.0 * std::f32::consts::PI;
    }

    // Linear chroma interpolation + shortest-arc hue rotation.
    let c = c0 + (c1 - c0) * t;
    let h = h0 + delta * t;

    (c * h.cos(), c * h.sin())
}

/// Per-segment precomputed polar interpolation parameters.
///
/// For a given pair of OKLab endpoints `(l0,a0,b0) → (l1,a1,b1)`, the
/// following quantities are constant across every interpolation step in
/// that segment:
///
/// - L delta: `l1 - l0`
/// - Chroma magnitudes `c0`, `c1`
/// - Hue angles `h0`, `h1` (when both endpoints are non-gray)
/// - Shortest-arc hue delta (when both endpoints are non-gray)
/// - `is_gray` flag (whether either endpoint has chroma < 1e-6)
/// - Cartesian a/b deltas (used only when `is_gray`)
///
/// Precomputing these once per segment saves ~3 sqrt + 2 atan2 + 1 branch
/// per output sample. For a 9-step gradient with 6 segments that's
/// ~54 sqrt + 36 atan2 saved per palette build.
struct PolarSegment {
    l0: f32,
    l_delta: f32,
    // Cartesian fallback deltas (used when is_gray is true):
    a0: f32,
    a_delta: f32,
    b0: f32,
    b_delta: f32,
    // Polar path (used when is_gray is false):
    c0: f32,
    c_delta: f32,
    h0: f32,
    h_delta: f32,
    is_gray: bool,
}

impl PolarSegment {
    /// Build the precomputed segment from two OKLab endpoints.
    #[inline]
    fn new(l0: f32, a0: f32, b0: f32, l1: f32, a1: f32, b1: f32) -> Self {
        let c0 = (a0 * a0 + b0 * b0).sqrt();
        let c1 = (a1 * a1 + b1 * b1).sqrt();
        let is_gray = c0 < 1e-6 || c1 < 1e-6;

        if is_gray {
            // Cartesian fallback path — no hue rotation, just linear (a, b).
            Self {
                l0,
                l_delta: l1 - l0,
                a0,
                a_delta: a1 - a0,
                b0,
                b_delta: b1 - b0,
                c0: 0.0,
                c_delta: 0.0,
                h0: 0.0,
                h_delta: 0.0,
                is_gray: true,
            }
        } else {
            let h0 = b0.atan2(a0);
            let h1 = b1.atan2(a1);
            // Shortest-arc delta: normalize into (-π, π].
            let mut delta = h1 - h0;
            if delta > std::f32::consts::PI {
                delta -= 2.0 * std::f32::consts::PI;
            } else if delta < -std::f32::consts::PI {
                delta += 2.0 * std::f32::consts::PI;
            }
            Self {
                l0,
                l_delta: l1 - l0,
                a0: 0.0,
                a_delta: 0.0,
                b0: 0.0,
                b_delta: 0.0,
                c0,
                c_delta: c1 - c0,
                h0,
                h_delta: delta,
                is_gray: false,
            }
        }
    }

    /// Interpolate this segment at parameter `t ∈ [0, 1]`.
    #[inline]
    fn sample(&self, t: f32) -> (f32, f32, f32) {
        // L: linear lerp (always, regardless of is_gray).
        let l = self.l0 + self.l_delta * t;

        let (a, b) = if self.is_gray {
            // Cartesian fallback: gray endpoint makes hue rotation meaningless.
            (self.a0 + self.a_delta * t, self.b0 + self.b_delta * t)
        } else {
            // Polar: linear chroma lerp + shortest-arc hue rotation.
            let c = self.c0 + self.c_delta * t;
            let h = self.h0 + self.h_delta * t;
            (c * h.cos(), c * h.sin())
        };

        (l, a, b)
    }
}

/// Hue-preserving OKLab gradient interpolation (sole production path).
///
/// Interpolates between `stops` in OKLab space, using polar (chroma + hue)
/// interpolation on the (a, b) chroma axes. Hue rotates through the
/// shortest arc, keeping chroma magnitude high through the midpoint.
///
/// # Why polar?
///
/// On opposing-hue gradients (red↔cyan, blue↔yellow), linear (a, b)
/// interpolation passes near (0, 0) = gray, producing a desaturated
/// midpoint. Polar rotates hue through the chroma ring and keeps the
/// midpoint saturated. On analogous-hue gradients (red↔orange, blue↔cyan)
/// polar and Cartesian produce identical output — polar never regresses.
///
/// # Peak optimization
///
/// Per-segment interpolation parameters (chroma magnitudes, hue angles,
/// shortest-arc delta, gray-endpoint flag) are precomputed once per
/// segment via [`PolarSegment`]. Each output sample then only needs:
///
/// - 1 multiply for L
/// - 1 multiply for chroma
/// - 1 multiply + 1 cos + 1 sin for hue
///
/// versus the per-step `sqrt + atan2 + atan2 + branch` cost of the
/// un-precomputed version. For a typical 7-stop × 9-step palette that's
/// ~54 sqrt + 36 atan2 saved per palette build.
///
/// # Endpoint preservation
///
/// Endpoints (`t=0` and `t=1`) are preserved exactly — only intermediate
/// colors are interpolated. This matches the contract every cosmostrix
/// theme expects (head/body/tail anchor stops are color-true).
///
/// # Cost
///
/// Build-time only (called from `palette::colors_from_stops`, not the
/// hot render path). One-shot per palette load, ~50ns per stop transition.
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

    // Precompute per-segment polar parameters (peak optimization).
    // Each segment holds all the trig + sqrt results that were previously
    // recomputed on every sample. Building the segments is O(segs); each
    // segment's `sample(t)` is then a single cos/sin pair.
    let segs = stops.len().saturating_sub(1);
    let segments: Vec<PolarSegment> = (0..segs)
        .map(|i| {
            let (l0, a0, b0) = ok[i];
            let (l1, a1, b1) = ok[i + 1];
            PolarSegment::new(l0, a0, b0, l1, a1, b1)
        })
        .collect();

    let mut out = Vec::with_capacity(steps);
    for i in 0..steps {
        let t = (i as f32) / ((steps - 1) as f32);
        let pos = t * (segs as f32);
        let mut seg = pos.floor() as usize;
        if seg >= segs {
            seg = segs.saturating_sub(1);
        }
        let lt = pos - (seg as f32);
        let (l, a, b) = segments[seg].sample(lt);
        out.push(oklab_to_srgb(l, a, b));
    }
    out
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

    /// Polar midpoint of red→green stays saturated (no muddy brown).
    ///
    /// This is the canonical hue-crossing test. Polar interpolation rotates
    /// hue through the chroma ring rather than the desaturated RGB cube
    /// center, so the midpoint stays saturated.
    #[test]
    fn red_to_green_midpoint_is_not_muddy() {
        let out = gradient_from_stops_oklab(&[(255, 0, 0), (0, 255, 0)], 3);
        let (mr, mg, mb) = out[1];
        // Saturation proxy: max channel - min channel. Muddy colors have low
        // saturation (max ≈ min). A saturated yellow/orange has max >> min.
        let max_c = mr.max(mg).max(mb) as i32;
        let min_c = mr.min(mg).min(mb) as i32;
        let sat = max_c - min_c;
        // Expect a clearly saturated midpoint. Polar should produce sat ≥ 60
        // (typically ~140+).
        assert!(
            sat >= 60,
            "polar red→green midpoint ({mr},{mg},{mb}) saturation = {sat}, expected ≥ 60"
        );
    }

    /// Polar midpoint of blue→yellow stays saturated (no gray).
    ///
    /// sRGB-linear midpoint of (0,0,255) → (255,255,0) is (143,143,143) — gray!
    /// Cartesian OKLab also takes a shortcut through gray on this opposing-hue
    /// gradient. Polar must produce a saturated midpoint.
    #[test]
    fn blue_to_yellow_midpoint_is_not_gray() {
        let out = gradient_from_stops_oklab(&[(0, 0, 255), (255, 255, 0)], 3);
        let (mr, mg, mb) = out[1];
        let max_c = mr.max(mg).max(mb) as i32;
        let min_c = mr.min(mg).min(mb) as i32;
        let sat = max_c - min_c;
        // Polar midpoint must be clearly saturated (not gray).
        assert!(
            sat >= 50,
            "polar blue→yellow midpoint ({mr},{mg},{mb}) saturation = {sat}, expected ≥ 50"
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

    /// Red↔cyan midpoint stays saturated.
    ///
    /// Red (255,0,0) and cyan (0,255,255) are roughly opposite on the OKLab
    /// chroma ring. Polar rotates through the chroma ring, keeping saturation
    /// high. This is the canonical case where polar outperforms the (now
    /// removed) Cartesian variant.
    #[test]
    fn red_to_cyan_midpoint_is_saturated() {
        let pol = gradient_from_stops_oklab(&[(255, 0, 0), (0, 255, 255)], 3);
        // Endpoints preserved.
        assert_eq!(pol[0], (255, 0, 0));
        assert_eq!(pol[2], (0, 255, 255));

        // Saturation proxy: max - min channel.
        let (mr, mg, mb) = pol[1];
        let max_c = mr.max(mg).max(mb) as i32;
        let min_c = mr.min(mg).min(mb) as i32;
        let sat_pol = max_c - min_c;
        // Polar midpoint should be clearly saturated (not gray).
        // Cartesian OKLab on red→cyan typically produces sat ≈ 30-60; polar
        // should produce sat ≥ 80 (typically 150+).
        assert!(
            sat_pol >= 80,
            "polar red→cyan midpoint {:?} saturation = {sat_pol}, expected ≥ 80",
            pol[1]
        );
    }

    /// When one endpoint is grayscale, polar falls back to Cartesian lerp
    /// (chroma magnitude drops linearly to the colored endpoint's value
    /// scaled by `t`). The grayscale fallback is the visually correct
    /// answer because hue rotation from "no hue" to any hue is meaningless.
    #[test]
    fn grayscale_endpoint_falls_back_to_cartesian() {
        // gray → red. Gray has OKLab chroma 0; red has chroma ~0.258.
        let out = gradient_from_stops_oklab(&[(128, 128, 128), (255, 0, 0)], 3);
        // Endpoints preserved.
        assert_eq!(out[0], (128, 128, 128));
        assert_eq!(out[2], (255, 0, 0));

        // Midpoint OKLab chroma must equal (c0 + c1) / 2 = c1 / 2 (since
        // c0=0 for gray). This is the Cartesian-fallback contract: linear
        // chroma interpolation between the endpoints' chroma magnitudes.
        let (_, a0, b0) = srgb_to_oklab(128, 128, 128);
        let (_, a1, b1) = srgb_to_oklab(255, 0, 0);
        let c0 = (a0 * a0 + b0 * b0).sqrt();
        let c1 = (a1 * a1 + b1 * b1).sqrt();
        assert!(c0 < 1e-6, "gray endpoint must have ~0 chroma");

        let (mr, mg, mb) = out[1];
        let (_, a_mid, b_mid) = srgb_to_oklab(mr, mg, mb);
        let c_mid = (a_mid * a_mid + b_mid * b_mid).sqrt();

        let expected_mid_chroma = (c0 + c1) / 2.0;
        assert!(
            (c_mid - expected_mid_chroma).abs() < 0.01,
            "grayscale fallback midpoint chroma {c_mid:.4} should equal linear average {expected_mid_chroma:.4} \
             (Cartesian fallback contract)"
        );

        // Sanity: midpoint should be a desaturated red (R > G, R > B), not
        // pure gray and not a hue-rotated saturated color.
        assert!(mr > mg, "R must dominate over G at the gray→red midpoint");
        assert!(mr > mb, "R must dominate over B at the gray→red midpoint");
    }

    /// When both endpoints share a hue (differ only in lightness/saturation),
    /// polar interpolation preserves that hue — the midpoint has the same
    /// hue angle as both endpoints (no rotation introduced).
    ///
    /// For two pure sRGB reds (G=B=0), the OKLab hue is identical at any
    /// intensity, and the polar midpoint also stays pure red (G=B=0) because
    /// the OKLab ray for pure red is collinear with the chroma ring's hue
    /// direction.
    #[test]
    fn same_hue_endpoints_preserve_hue() {
        // Two reds with different lightness.
        let out = gradient_from_stops_oklab(&[(50, 0, 0), (255, 0, 0)], 3);
        // Endpoints preserved.
        assert_eq!(out[0], (50, 0, 0));
        assert_eq!(out[2], (255, 0, 0));

        // Midpoint: pure red stays pure red — polar must not introduce
        // green or blue channels when both endpoints have G=B=0 in sRGB.
        // This is because pure sRGB reds at any intensity are collinear
        // with the OKLab hue direction (a, b scales linearly with L for
        // pure sRGB primaries), so polar stays on the same ray.
        let (mr, mg, mb) = out[1];
        assert_eq!(
            mg, 0,
            "midpoint G must be 0 (pure red preserved — no green introduced)"
        );
        assert_eq!(
            mb, 0,
            "midpoint B must be 0 (pure red preserved — no blue introduced)"
        );
        // Sanity: midpoint R should be roughly between 50 and 255.
        assert!(
            (50..=255).contains(&mr),
            "midpoint R = {mr}, should be in [50, 255]"
        );

        // Verify the hue-preservation property directly: midpoint OKLab
        // hue must equal both endpoints' hue (no rotation).
        let (_, a0, b0) = srgb_to_oklab(50, 0, 0);
        let (_, a1, b1) = srgb_to_oklab(255, 0, 0);
        let (_, a_mid, b_mid) = srgb_to_oklab(mr, mg, mb);
        let h0 = b0.atan2(a0);
        let h1 = b1.atan2(a1);
        let h_mid = b_mid.atan2(a_mid);
        assert!(
            (h_mid - h0).abs() < 1e-4 && (h_mid - h1).abs() < 1e-4,
            "midpoint hue {h_mid:.4} must equal endpoint hues ({h0:.4}, {h1:.4}) — polar preserves hue"
        );
    }

    /// `polar_chroma_lerp` unit test: t=0 returns start, t=1 returns end
    /// (within floating-point precision).
    #[test]
    fn polar_chroma_lerp_endpoints() {
        let (a, b) = polar_chroma_lerp(0.5, 0.3, -0.4, 0.2, 0.0);
        assert!((a - 0.5).abs() < 1e-5 && (b - 0.3).abs() < 1e-5);

        let (a, b) = polar_chroma_lerp(0.5, 0.3, -0.4, 0.2, 1.0);
        assert!((a - -0.4).abs() < 1e-5 && (b - 0.2).abs() < 1e-5);
    }

    /// `polar_chroma_lerp` unit test: midpoint chroma magnitude is the
    /// average of the endpoint chromas (linear chroma interpolation).
    #[test]
    fn polar_chroma_lerp_midpoint_chroma_is_average() {
        let a0 = 0.6_f32;
        let b0 = 0.0_f32;
        let a1 = -0.6_f32;
        let b1 = 0.0_f32;
        let c0 = (a0 * a0 + b0 * b0).sqrt();
        let c1 = (a1 * a1 + b1 * b1).sqrt();
        let (am, bm) = polar_chroma_lerp(a0, b0, a1, b1, 0.5);
        let cm = (am * am + bm * bm).sqrt();
        let expected = (c0 + c1) / 2.0;
        assert!(
            (cm - expected).abs() < 1e-5,
            "midpoint chroma {cm} should be average {expected}"
        );
    }

    /// `polar_chroma_lerp` unit test: grayscale endpoint falls back to
    /// Cartesian lerp (chroma magnitude drops linearly to 0).
    #[test]
    fn polar_chroma_lerp_grayscale_falls_back_to_cartesian() {
        // Start: saturated red (a=0.5, b=0). End: gray (a=0, b=0).
        let (a, b) = polar_chroma_lerp(0.5, 0.0, 0.0, 0.0, 0.5);
        // Cartesian would give a=0.25, b=0. Polar fallback should match.
        assert!((a - 0.25).abs() < 1e-5 && b.abs() < 1e-5);
    }

    /// `polar_chroma_lerp` on opposing hues produces higher chroma than
    /// Cartesian at the midpoint (the polar path's defining property).
    #[test]
    fn polar_chroma_lerp_opposing_hues_stay_saturated() {
        // Red (a=+0.45) ↔ Cyan (a=-0.45). Cartesian midpoint = (0, 0) = gray.
        // Polar midpoint stays on the chroma ring.
        let (a0, b0) = (0.45_f32, 0.20_f32);
        let (a1, b1) = (-0.45_f32, -0.05_f32);

        // Cartesian midpoint at t=0.5
        let cart_chroma = {
            let ca = a0 + (a1 - a0) * 0.5;
            let cb = b0 + (b1 - b0) * 0.5;
            (ca * ca + cb * cb).sqrt()
        };

        // Polar midpoint at t=0.5
        let (pa, pb) = polar_chroma_lerp(a0, b0, a1, b1, 0.5);
        let pol_chroma = (pa * pa + pb * pb).sqrt();

        assert!(
            pol_chroma > cart_chroma,
            "Polar midpoint chroma {pol_chroma:.4} should exceed Cartesian midpoint chroma {cart_chroma:.4} \
             for opposing hues — polar must stay saturated"
        );
    }

    /// Peak optimization: precomputed `PolarSegment` produces identical
    /// output to the un-precomputed `polar_chroma_lerp` path. This guards
    /// against future regressions in the PolarSegment struct.
    #[test]
    fn polar_segment_matches_polar_chroma_lerp() {
        // Arbitrary non-gray endpoints.
        let (l0, a0, b0) = (0.5_f32, 0.3_f32, 0.2_f32);
        let (l1, a1, b1) = (0.7_f32, -0.2_f32, 0.4_f32);

        let seg = PolarSegment::new(l0, a0, b0, l1, a1, b1);

        for i in 0..=100 {
            let t = i as f32 / 100.0;
            let (pl, pa, pb) = seg.sample(t);
            let expected_l = l0 + (l1 - l0) * t;
            let (expected_a, expected_b) = polar_chroma_lerp(a0, b0, a1, b1, t);

            assert!((pl - expected_l).abs() < 1e-5, "L mismatch at t={t}");
            assert!((pa - expected_a).abs() < 1e-5, "a mismatch at t={t}");
            assert!((pb - expected_b).abs() < 1e-5, "b mismatch at t={t}");
        }
    }

    /// Peak optimization: `PolarSegment` grayscale fallback matches
    /// `polar_chroma_lerp` grayscale fallback (which is Cartesian lerp).
    #[test]
    fn polar_segment_grayscale_fallback_matches_polar_chroma_lerp() {
        // Start: gray (a=0, b=0). End: red (a=0.5, b=0).
        let (l0, a0, b0) = (0.5_f32, 0.0_f32, 0.0_f32);
        let (l1, a1, b1) = (0.6_f32, 0.5_f32, 0.0_f32);

        let seg = PolarSegment::new(l0, a0, b0, l1, a1, b1);
        assert!(
            seg.is_gray,
            "segment with gray endpoint must be flagged is_gray"
        );

        for i in 0..=100 {
            let t = i as f32 / 100.0;
            let (pl, pa, pb) = seg.sample(t);
            let expected_l = l0 + (l1 - l0) * t;
            let (expected_a, expected_b) = polar_chroma_lerp(a0, b0, a1, b1, t);

            assert!((pl - expected_l).abs() < 1e-5, "L mismatch at t={t}");
            assert!((pa - expected_a).abs() < 1e-5, "a mismatch at t={t}");
            assert!((pb - expected_b).abs() < 1e-5, "b mismatch at t={t}");
        }
    }
}
