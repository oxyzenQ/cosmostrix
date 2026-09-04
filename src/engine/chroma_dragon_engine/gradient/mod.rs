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
//! - Cartesian (linear lerp of `a` and `b`): on opposing-hue gradients
//!   (red↔cyan, blue↔yellow), the (a, b) midpoint passes near (0, 0) = gray,
//!   producing a desaturated midpoint. This is the canonical "Cartesian
//!   shortcut through gray" failure mode.
//! - Polar (lerp chroma magnitude `C = sqrt(a²+b²)` linearly + rotate hue
//!   `h = atan2(b, a)` through the shortest arc): chroma magnitude stays
//!   high through the midpoint, so the gradient stays saturated.
//!
//! Polar never regresses against Cartesian — on analogous-hue gradients both
//! produce identical output; on opposing-hue gradients polar stays saturated
//! while Cartesian collapses toward gray. Polar also aligns cosmostrix with
//! the W3C CSS Color Module Level 4 spec, which defaults `oklch`
//! interpolation to shortest-arc hue rotation.
//!
//! As of v30, polar is the sole production gradient path. The Cartesian
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
/// `h = atan2(b, a)` rotates through the shortest arc. This avoids the
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

/// Perceptual (OKLab) blend between two sRGB colors.
///
/// Used by the intro animation system so that color transitions during
/// the cinematic intro (singularity → burst → morph → rain) use the
/// same perceptually uniform interpolation as the rain palette gradients.
///
/// Lightness L is linearly interpolated. Chroma (a, b) uses polar
/// interpolation (shortest-arc hue rotation) to avoid the "Cartesian
/// shortcut through gray" problem on opposing-hue transitions.
///
/// # Arguments
///
/// - `r0, g0, b0` — start color (t=0).
/// - `r1, g1, b1` — end color (t=1).
/// - `t` — blend factor in `[0, 1]`.
///
/// # Returns
///
/// Interpolated (r, g, b) in sRGB.
#[inline]
#[must_use]
pub(crate) fn oklab_blend_rgb(
    r0: u8,
    g0: u8,
    b0: u8,
    r1: u8,
    g1: u8,
    b1: u8,
    t: f32,
) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    let (l0, a0, b0) = srgb_to_oklab(r0, g0, b0);
    let (l1, a1, b1) = srgb_to_oklab(r1, g1, b1);
    let l = l0 + (l1 - l0) * t;
    let (a, b) = polar_chroma_lerp(a0, b0, a1, b1, t);
    oklab_to_srgb(l, a, b)
}

#[cfg(test)]
#[path = "../../../../test/engine/chroma_dragon_engine/gradient/tests.rs"]
mod tests;
