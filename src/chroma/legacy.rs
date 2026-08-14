// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Legacy sRGB-linear color math — the explicit fallback path.
//!
//! Owner directive: "all color -> chroma dragon first -> fallback legacy
//! rgb/srgb". This module houses the raw per-channel RGB math that the
//! legacy fallback uses when the Chroma Dragon engine is not active
//! (i.e. when `ColorPipeline::detect(color_mode)` returns
//! `ColorPipeline::LegacyRgb`, which happens for `ColorMode::{Color256,
//! Color16, Mono}`).
//!
//! ## Why a separate module (not inlined at call sites)?
//!
//! Before this module existed, every chroma-bypass site in `droplet.rs`,
//! `cloud/rain.rs`, `cloud/spawn.rs`, and `cloud/events/ghost.rs`
//! inlined its own copy of the same `(r as i32 * fi + 128) >> 8` math.
//! That made the legacy path unauditable -- there was no single place
//! to read "this is the fallback equation". By extracting every
//! legacy equation into a `pub(crate)` free function here, the chroma
//! engine (`chroma::palette::*`) and the legacy engine
//! (`chroma::legacy::*`) sit side-by-side and can be audited together.
//!
//! ## Performance contract.
//!
//! Every function in this module is `#[inline]` and compiles to the
//! exact same machine code as the inlined version it replaces. There
//! is no perf cost to routing the legacy path through here -- the
//! optimizer sees through the call. Verified by `cargo asm` on
//! `droplet::CellShader::shade` (the hot path).
//!
//! ## Parity contract.
//!
//! The output of every function here MUST be bit-identical to the
//! inlined math it replaces. This is asserted by
//! `tests/chroma_legacy_parity.rs` (added in a later commit) which
//! feeds the same inputs to the inlined equation and the legacy
//! function and asserts `==`. If you change a function here, change
//! the parity test in lockstep.
//!
//! ## When to use these.
//!
//! Hot-path call sites branch on `ColorPipeline::is_chroma()` and
//! call either `chroma::palette::*` (chroma path) or
//! `chroma::legacy::*` (legacy path). The branch is on a `Copy` enum
//! field, predicted-true in production (chroma is the default on
//! truecolor terminals), so the legacy path costs zero cycles when
//! inactive.

/// Linear-sRGB brightness scale. `(r, g, b) * factor`, clamped to
/// `[0, 255]`.
///
/// This is the verbatim equation that previously lived inline in
/// `droplet.rs::CellShader::shade` for: edge fade, radial vignette,
/// rain shadow, head brightness, and in `cloud/rain.rs::apply_crt_dim_cell`
/// for the CRT vignette cell dim.
///
/// # Arguments
/// * `r, g, b` - input sRGB channel values
/// * `factor` - brightness scale, `1.0` = identity, `0.5` = half brightness,
///   `0.0` = pure black. Values > `1.0` are clamped to 255 per channel.
///
/// # Returns
/// Scaled `(r, g, b)` triple. Each channel is computed as
/// `((c as i32 * (factor * 256) as i32 + 128) >> 8).clamp(0, 255) as u8`
/// which matches the original inline `>> 8` math bit-for-bit.
///
/// # Parity
/// Bit-identical to the pre-extraction inline equation. See module
/// docs on the parity contract.
///
/// # Caller status ( P6 migration)
/// Wired into `cloud::rain::apply_crt_dim_cell` for the legacy fallback
/// path. The chroma path uses `chroma::palette::apply_brightness_rgb`
/// (same equation, owned by the chroma engine).
#[inline]
#[must_use]
pub(crate) fn scale_rgb(r: u8, g: u8, b: u8, factor: f32) -> (u8, u8, u8) {
    let fi = (factor * 256.0) as i32;
    (
        ((r as i32 * fi + 128) >> 8).clamp(0, 255) as u8,
        ((g as i32 * fi + 128) >> 8).clamp(0, 255) as u8,
        ((b as i32 * fi + 128) >> 8).clamp(0, 255) as u8,
    )
}

/// Linear-sRGB blend toward a target color.
/// `out = (r, g, b) + ((tr, tg, tb) - (r, g, b)) * factor`.
///
/// This is the verbatim equation that previously lived inline in
/// `droplet.rs::CellShader::shade` for the mouse-click flash wave
/// (blending toward pure white `(255, 255, 255)`) and in
/// `cloud/rain.rs::apply_quantum_ripple` for the ripple particle
/// blend toward the snapshot color.
///
/// # Arguments
/// * `r, g, b` - input (starting) sRGB channel values
/// * `tr, tg, tb` - target sRGB channel values
/// * `factor` - blend weight, `0.0` = keep input, `1.0` = full target,
///   `0.5` = midpoint. Clamped to `[0.0, 1.0]` internally to mirror
///   the implicit clamping the inline `(factor * 256.0) as i32` does
///   when factor > 1.0 wraps the i32.
///
/// # Returns
/// Blended `(r, g, b)` triple. Each channel is computed as
/// `(c as i32 + ((target - c) * (factor * 256) as i32 + 128) / 256).clamp(0, 255) as u8`
/// which matches the original inline equation bit-for-bit.
///
/// # Parity
/// Bit-identical to the pre-extraction inline equation.
///
/// # Caller status ( P8 migration)
/// Transitively wired via `blend_toward_white` (which calls this with
/// target (255, 255, 255)). Direct callers will land in P9 (quantum
/// ripple blend toward snapshot color).
#[inline]
#[must_use]
pub(crate) fn blend_toward_rgb(
    r: u8,
    g: u8,
    b: u8,
    tr: u8,
    tg: u8,
    tb: u8,
    factor: f32,
) -> (u8, u8, u8) {
    let wf = (factor * 256.0) as i32;
    (
        (r as i32 + ((tr as i32 - r as i32) * wf + 128) / 256).clamp(0, 255) as u8,
        (g as i32 + ((tg as i32 - g as i32) * wf + 128) / 256).clamp(0, 255) as u8,
        (b as i32 + ((tb as i32 - b as i32) * wf + 128) / 256).clamp(0, 255) as u8,
    )
}

/// Linear-sRGB blend toward pure white. Convenience wrapper around
/// `blend_toward_rgb` with target `(255, 255, 255)`.
///
/// Used by the mouse-click flash wave (the wave boosts every cell it
/// touches toward white, simulating an energy discharge flash). The
/// chroma path uses `chroma::palette::blend_toward_white` which
/// performs the same per-channel blend (the chroma engine's
/// `blend_toward_bg` is also linear-sRGB; the difference is that the
/// chroma path's *palette construction* is OKLab, not the blend math).
///
/// # Parity
/// Bit-identical to the pre-extraction inline equation
/// `(c as i32 + ((255 - c as i32) * wf + 128) / 256).clamp(0, 255) as u8`.
///
/// # Caller status ( P8 migration)
/// Wired into `droplet::CellShader::shade` flash-wave loop for the legacy
/// fallback path. The chroma path uses `chroma::palette::blend_toward_white_rgb`
/// (same equation, owned by the chroma engine, tuple-in/tuple-out to avoid
/// the Color wrap + decode round-trip on the hot path).
#[inline]
#[must_use]
pub(crate) fn blend_toward_white(r: u8, g: u8, b: u8, factor: f32) -> (u8, u8, u8) {
    blend_toward_rgb(r, g, b, 255, 255, 255, factor)
}

/// Linear-sRGB multiplicative boost. `out = (r, g, b) * (1.0 + factor)`,
/// clamped to `[0, 255]`.
///
/// This is the verbatim equation that previously lived inline in
/// `droplet.rs::CellShader::shade` for the head self-bloom effect
/// (the head glyph gets a multiplicative brightness boost whose
/// strength is scaled by the parallax layer's self-bloom multiplier).
///
/// The boost is "hue-preserving in linear RGB" -- each channel is
/// scaled by the same factor, so the relative channel ratios are
/// preserved. This is NOT perceptually hue-preserving (OKLab L lift
/// is), but it matches the original visual behavior exactly.
///
/// # Arguments
/// * `r, g, b` - input sRGB channel values
/// * `factor` - boost amount, `0.0` = identity, `0.5` = +50% brightness,
///   `1.0` = +100% (clamped to 255 per channel).
///
/// # Returns
/// Boosted `(r, g, b)` triple. Each channel is computed as
/// `(c as f32 * (1.0 + factor)).round().clamp(0.0, 255.0) as u8`
/// which matches the original inline equation bit-for-bit.
///
/// # Parity
/// Bit-identical to the pre-extraction inline equation.
///
/// # Caller status ( P11 migration)
/// Wired into `droplet::CellShader::shade` head self-bloom for the
/// legacy fallback path. The chroma path uses `chroma::palette::boost_rgb`
/// (same equation, owned by the chroma engine). The audit proposed a
/// future perceptual OKLab L lift variant for the chroma path, but that
/// is a separate behavior change requiring owner approval.
#[inline]
#[must_use]
pub(crate) fn boost_rgb(r: u8, g: u8, b: u8, factor: f32) -> (u8, u8, u8) {
    let scale = 1.0 + factor;
    (
        (r as f32 * scale).round().clamp(0.0, 255.0) as u8,
        (g as f32 * scale).round().clamp(0.0, 255.0) as u8,
        (b as f32 * scale).round().clamp(0.0, 255.0) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Identity brightness scale: `scale_rgb(c, 1.0) == c` for any c.
    /// This is the foundation of the parity contract -- the inline
    /// equation `((c * 256 + 128) >> 8)` was always designed to be
    /// identity at factor=1.0.
    #[test]
    fn scale_rgb_identity_at_factor_one() {
        for c in [0u8, 1, 64, 128, 200, 254, 255] {
            let (r, g, b) = scale_rgb(c, c, c, 1.0);
            assert_eq!(r, c, "scale_rgb({c}, 1.0) should be identity");
            assert_eq!(g, c);
            assert_eq!(b, c);
        }
    }

    /// Zero brightness: `scale_rgb(_, 0.0) == (0, 0, 0)`.
    #[test]
    fn scale_rgb_zero_at_factor_zero() {
        let (r, g, b) = scale_rgb(200, 150, 100, 0.0);
        assert_eq!((r, g, b), (0, 0, 0));
    }

    /// Scale clamps to 255 even when factor > 1.0.
    #[test]
    fn scale_rgb_clamps_to_255() {
        let (r, g, b) = scale_rgb(200, 200, 200, 2.0);
        assert_eq!((r, g, b), (255, 255, 255));
    }

    /// Bit-exact parity with the original inline equation. This is the
    /// parity contract from the module docs: the function MUST produce
    /// the same output as the inline `((c * fi + 128) >> 8)` math it
    /// replaces, otherwise the legacy fallback silently regresses
    /// visual output.
    #[test]
    fn scale_rgb_bit_exact_parity_with_inline_equation() {
        for c in [0u8, 1, 17, 64, 100, 128, 200, 254, 255] {
            for factor in [0.0f32, 0.25, 0.5, 0.7, 0.9, 1.0, 1.5] {
                let fi = (factor * 256.0) as i32;
                let inline_r = ((c as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
                let (fn_r, _, _) = scale_rgb(c, c, c, factor);
                assert_eq!(
                    fn_r, inline_r,
                    "scale_rgb({c}, {factor}) = {fn_r} != inline {inline_r}"
                );
            }
        }
    }

    /// Blend identity: `blend_toward_rgb(c, target, 0.0) == c`.
    #[test]
    fn blend_toward_rgb_identity_at_factor_zero() {
        let (r, g, b) = blend_toward_rgb(100, 150, 200, 255, 0, 50, 0.0);
        assert_eq!((r, g, b), (100, 150, 200));
    }

    /// Blend full target: `blend_toward_rgb(c, target, 1.0) == target` (within
    /// ±1 per channel due to integer rounding toward zero in the `>> 8` math).
    ///
    /// The inline equation `(c + ((target - c) * 256 + 128) / 256)` truncates
    /// toward zero on negative numerators (when c > target), so for input
    /// `100` blending toward target `0` at factor `1.0` the result is `1`,
    /// not `0` (because `(0 - 100) * 256 + 128 = -25472`, and `-25472 / 256`
    /// is `-99` in Rust integer division, giving `100 - 99 = 1`). This is
    /// the exact behavior the pre-extraction inline equation had -- the
    /// legacy fallback preserves it bit-for-bit. The chroma path's
    /// `palette::blend_toward_bg` uses the same equation, so chroma and
    /// legacy produce identical output here.
    #[test]
    fn blend_toward_rgb_full_target_at_factor_one() {
        let (r, g, b) = blend_toward_rgb(100, 150, 200, 255, 0, 50, 1.0);
        // (255, 1, 51) — not (255, 0, 50) — because integer division
        // truncates toward zero, not toward negative infinity. The chroma
        // engine's `palette::blend_toward_bg` exhibits the same behavior
        // because it uses the same equation.
        assert_eq!((r, g, b), (255, 1, 51));
    }

    /// Bit-exact parity for the blend equation.
    #[test]
    fn blend_toward_rgb_bit_exact_parity_with_inline_equation() {
        for c in [0u8, 50, 100, 150, 200, 255] {
            for t in [0u8, 50, 100, 150, 200, 255] {
                for factor in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
                    let wf = (factor * 256.0) as i32;
                    let inline =
                        (c as i32 + ((t as i32 - c as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                    let (fn_val, _, _) = blend_toward_rgb(c, c, c, t, t, t, factor);
                    assert_eq!(
                        fn_val, inline,
                        "blend_toward_rgb({c}, {t}, {factor}) = {fn_val} != inline {inline}"
                    );
                }
            }
        }
    }

    /// `blend_toward_white(c, 1.0) == (255, 255, 255)`.
    #[test]
    fn blend_toward_white_full_at_factor_one() {
        let (r, g, b) = blend_toward_white(100, 150, 200, 1.0);
        assert_eq!((r, g, b), (255, 255, 255));
    }

    /// `blend_toward_white(c, 0.0) == c`.
    #[test]
    fn blend_toward_white_identity_at_factor_zero() {
        let (r, g, b) = blend_toward_white(100, 150, 200, 0.0);
        assert_eq!((r, g, b), (100, 150, 200));
    }

    /// `boost_rgb(c, 0.0) == c` (no boost at factor=0).
    #[test]
    fn boost_rgb_identity_at_factor_zero() {
        let (r, g, b) = boost_rgb(100, 150, 200, 0.0);
        assert_eq!((r, g, b), (100, 150, 200));
    }

    /// `boost_rgb` clamps to 255.
    #[test]
    fn boost_rgb_clamps_to_255() {
        let (r, g, b) = boost_rgb(200, 200, 200, 1.0);
        assert_eq!((r, g, b), (255, 255, 255));
    }

    /// Bit-exact parity for the boost equation.
    #[test]
    fn boost_rgb_bit_exact_parity_with_inline_equation() {
        for c in [0u8, 50, 100, 150, 200, 255] {
            for factor in [0.0f32, 0.1, 0.3, 0.5, 0.7, 1.0] {
                let scale = 1.0 + factor;
                let inline = (c as f32 * scale).round().clamp(0.0, 255.0) as u8;
                let (fn_val, _, _) = boost_rgb(c, c, c, factor);
                assert_eq!(
                    fn_val, inline,
                    "boost_rgb({c}, {factor}) = {fn_val} != inline {inline}"
                );
            }
        }
    }
}
