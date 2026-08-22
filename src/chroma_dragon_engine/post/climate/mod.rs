// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Climate Shader
//!
//! Chroma Dragon Innovation G — integrated atmospheric post-processing.
//!
//! ## Problem (pre-Phase-3-G)
//!
//! Climate effects (luminance climate, saturation drift, persistence
//! richness, instability pressure) were applied in a separate post-hoc
//! pass over dirty cells (`apply_climate_frame_effects` was
//! deleted; climate is shader-only now). That pass:
//!
//! 1. Iterated all dirty cell indices (~500/frame typical).
//! 2. For each cell, decoded the already-written `Color::Rgb` back to
//!    `(r, g, b)` via `palette::decode_color`.
//! 3. Applied the modifiers.
//! 4. Re-encoded as `Color::Rgb` and called `frame.set()`.
//!
//! The decode-encode cycle was pure waste — the shader had ALREADY
//! resolved the cell's palette stop and encoded it as a `Color`. The
//! post-hoc pass undid that encoding, modified the raw tuple, then
//! re-encoded. The `frame.set()` call also marked the cell dirty again,
//! causing a redundant diff on the next frame.
//!
//! ## Solution
//!
//! `apply_climate()` is a pure function that takes a raw `(r, g, b)`
//! triple plus position and a precomputed `ClimateCtx`, and returns
//! the modified `(r, g, b)`. The base shader calls it on the resolved
//! color BEFORE encoding to `Color::Rgb`, so the cell is written to the
//! frame once with atmospheric already applied.
//!
//! `ClimateCtx` precomputes all frame-invariant factors (dim/boost
//! integers, saturation factor, persistence factor, instability
//! threshold/weight) once per frame in `cloud::rain::rain_at`. The
//! shader just reads them — no per-cell float math, no per-cell
//! `Instant::elapsed()` syscall (the `now_secs` used by the instability
//! hash is hoisted into the ctx).
//!
//! ## Behavior parity
//!
//! The math here is identical to the pre-Phase-3-G post-hoc pass —
//! same integer fixed-point factors, same blend-toward-gray for
//! saturation, same blend-toward-white for luminance boost and
//! persistence, same hash-based instability trigger. The only
//! difference is WHEN it runs (shader vs post-hoc) and HOW MANY times
//! the cell is encoded (once vs twice). Visible output is identical
//! for any cell that was dirty in only one frame; for cells that
//! remained dirty across multiple frames, the new path is actually
//! MORE correct because atmospheric is applied exactly once per
//! render, not once per dirty-frame.

/// Precomputed atmospheric factors for one frame.
///
/// Built once per frame in `cloud::rain::rain_at` from `ColorEcosystem`,
/// `Memory`, `Storytelling`, and `ProfileCurrent` state. Passed by
/// reference through `DrawCtx` → `ShaderCtx` → `apply_climate`.
///
/// All factors use integer fixed-point with denominator 256 (i.e. the
/// factor is `target_value * 256`), so the hot-path multiplication +
/// shift avoids any float math. `None` fields mean "no effect" — the
/// shader skips that branch entirely.
///
/// `ClimateCtx::none()` returns a ctx with all fields `None` — the
/// shader's `apply_climate` is a no-op for this ctx, matching the
/// pre-Phase-3-G "skip if all neutral" early-return behavior.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ClimateCtx {
    /// Dim factor: multiply each channel by `fi / 256`. Active when
    /// total luminance < 1.0 (luminance_climate + profile offset + emergent
    /// boost < 1.0). `None` means no dimming.
    pub lum_fi: Option<i32>,

    /// Boost factor: blend each channel toward white by `wf / 256`. Active
    /// when total luminance > 1.0. `None` means no boost.
    pub lum_wf: Option<i32>,

    /// Saturation factor: blend each channel toward gray (the channel's
    /// own average) by `ti / 256`. Active when saturation_climate < 1.0.
    /// `None` means no desaturation.
    pub sat_ti: Option<i32>,

    /// Persistence factor: blend each channel toward white by `wf / 256`.
    /// Active when memory.persistence_richness > 0. `None` means no
    /// persistence glow.
    pub persist_wf: Option<i32>,

    /// Instability trigger threshold: a per-cell hash modulo 1000 below
    /// this value triggers the instability white-blend. Active when
    /// memory.instability_pressure > 0.15. `None` means no instability.
    pub instability_threshold: Option<u32>,

    /// Instability blend factor: when triggered, blend each channel toward
    /// white by `wf / 256`. Always `Some` when `instability_threshold` is
    /// `Some`.
    pub instability_wf: Option<i32>,

    /// Hoisted `Instant::elapsed().as_secs()` snapshot — used by the
    /// instability hash so the same cell gets different instability
    /// decisions across seconds. Precomputed once per frame to avoid
    /// ~500 syscalls per frame in the post-hoc pass.
    pub now_secs: u32,
}

impl ClimateCtx {
    /// Build a neutral ctx (all fields `None`) — equivalent to the
    /// pre-Phase-3-G "skip if all neutral" early-return.
    ///
    /// The shader's `apply_climate` returns the input unchanged for
    /// this ctx, so callers can pass it unconditionally and let the
    /// shader skip the work.
    ///
    /// Kept as a public API helper for callers that want to construct a
    /// neutral ctx without going through `Default::default()`. Used in
    /// tests; production callers typically build a real ctx from Cloud
    /// state via the rain.rs construction site.
    #[cfg(test)]
    #[inline]
    pub(crate) const fn none() -> Self {
        Self {
            lum_fi: None,
            lum_wf: None,
            sat_ti: None,
            persist_wf: None,
            instability_threshold: None,
            instability_wf: None,
            now_secs: 0,
        }
    }

    /// Returns `true` if all atmospheric factors are neutral (no effect
    /// would be applied). Matches the old "skip if all neutral" check
    /// from the deleted post-hoc pass.
    #[inline]
    pub(crate) const fn is_neutral(&self) -> bool {
        self.lum_fi.is_none()
            && self.lum_wf.is_none()
            && self.sat_ti.is_none()
            && self.persist_wf.is_none()
            && self.instability_threshold.is_none()
    }
}

/// Apply atmospheric effects to a raw `(r, g, b)` triple.
///
/// Pure function — no allocation, no side effects, no syscalls. The
/// caller (base shader) passes the resolved cell color decoded to RGB,
/// the cell's `(line, col)` position (used by the instability hash), and
/// the frame's precomputed `ClimateCtx`.
///
/// The math is identical to the pre-Phase-3-G post-hoc pass:
///
/// 1. **Luminance dim** (`lum_fi`): `channel = (channel * fi + 128) >> 8`,
///    clamped to `[0, 255]`. Active when total luminance < 1.0.
/// 2. **Luminance boost** (`lum_wf`): `channel += ((255 - channel) * wf + 128) / 256`,
///    clamped. Active when total luminance > 1.0.
/// 3. **Saturation** (`sat_ti`): blend toward gray (channel average) by
///    `ti / 256`. Active when saturation_climate < 1.0.
/// 4. **Persistence** (`persist_wf`): blend toward white by `wf / 256`.
///    Active when persistence_richness > 0.
/// 5. **Instability** (`instability_threshold` + `instability_wf`): a
///    per-cell hash modulo 1000 below `threshold` triggers a white-blend
///    by `wf / 256`. Active when instability_pressure > 0.15.
///
/// Dim and boost are mutually exclusive (a given frame is either dim or
/// boost, not both) — see `ClimateCtx::lum_fi` / `lum_wf`.
///
/// Returns the input unchanged if the ctx is neutral (`is_neutral()`).
#[inline]
pub(crate) fn apply_climate(
    mut r: u8,
    mut g: u8,
    mut b: u8,
    line: u16,
    col: u16,
    ctx: &ClimateCtx,
) -> (u8, u8, u8) {
    // Fast path: all factors neutral → no work. Matches the old
    // "skip if all neutral" early-return from the deleted post-hoc pass.
    if ctx.is_neutral() {
        return (r, g, b);
    }

    // Luminance: dim OR boost (never both — see ClimateCtx doc).
    if let Some(fi) = ctx.lum_fi {
        r = ((i32::from(r) * fi + 128) >> 8).clamp(0, 255) as u8;
        g = ((i32::from(g) * fi + 128) >> 8).clamp(0, 255) as u8;
        b = ((i32::from(b) * fi + 128) >> 8).clamp(0, 255) as u8;
    } else if let Some(wf) = ctx.lum_wf {
        r = (i32::from(r) + ((255 - i32::from(r)) * wf + 128) / 256).clamp(0, 255) as u8;
        g = (i32::from(g) + ((255 - i32::from(g)) * wf + 128) / 256).clamp(0, 255) as u8;
        b = (i32::from(b) + ((255 - i32::from(b)) * wf + 128) / 256).clamp(0, 255) as u8;
    }

    // Saturation: blend toward per-channel gray (the channel average).
    if let Some(ti) = ctx.sat_ti {
        let gray = ((u16::from(r) + u16::from(g) + u16::from(b)) / 3) as u8;
        r = (i32::from(gray) + ((i32::from(r) - i32::from(gray)) * ti + 128) / 256).clamp(0, 255)
            as u8;
        g = (i32::from(gray) + ((i32::from(g) - i32::from(gray)) * ti + 128) / 256).clamp(0, 255)
            as u8;
        b = (i32::from(gray) + ((i32::from(b) - i32::from(gray)) * ti + 128) / 256).clamp(0, 255)
            as u8;
    }

    // Persistence: blend toward white (same formula as luminance boost,
    // different factor source — persistence_richness instead of luminance
    // climate).
    if let Some(wf) = ctx.persist_wf {
        r = (i32::from(r) + ((255 - i32::from(r)) * wf + 128) / 256).clamp(0, 255) as u8;
        g = (i32::from(g) + ((255 - i32::from(g)) * wf + 128) / 256).clamp(0, 255) as u8;
        b = (i32::from(b) + ((255 - i32::from(b)) * wf + 128) / 256).clamp(0, 255) as u8;
    }

    // Instability: per-cell hash decides whether to trigger a white-blend.
    // The hash mixes col, line, and now_secs so the same cell gets different
    // decisions across seconds — producing a "flicker" effect on unstable
    // frames. The hash function is the same one used in the pre-Phase-3-G
    // post-hoc pass (Knuth's multiplicative hash for col and line, XORed
    // with now_secs).
    if let Some(threshold) = ctx.instability_threshold {
        let hash = (u32::from(col)).wrapping_mul(2654435761)
            ^ (u32::from(line)).wrapping_mul(2246822519)
            ^ ctx.now_secs;
        if hash % 1000 < threshold {
            // instability_wf is always Some when instability_threshold is Some
            // (both are set together when instability > 0.15). Defensive
            // `unwrap_or(0)` guards against contract drift — if a future
            // commit constructs an ClimateCtx with `instability_threshold:
            // Some(...)` but `instability_wf: None`, the worst case is a
            // no-op (wf=0 → no white blend) rather than a panic on every cell
            // in the anomaly zone.
            let wf = ctx.instability_wf.unwrap_or(0);
            r = (i32::from(r) + ((255 - i32::from(r)) * wf + 128) / 256).clamp(0, 255) as u8;
            g = (i32::from(g) + ((255 - i32::from(g)) * wf + 128) / 256).clamp(0, 255) as u8;
            b = (i32::from(b) + ((255 - i32::from(b)) * wf + 128) / 256).clamp(0, 255) as u8;
        }
    }

    (r, g, b)
}

#[cfg(test)]
mod tests;
