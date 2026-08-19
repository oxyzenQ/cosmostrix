// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Palette-aware Anomaly Halo Target
//!
//! Phase 6 (Chroma Dragon) — extends Phase 3-I's "palette-aware ghost"
//! pattern to anomaly halos in `cloud::phosphor::apply_anomalies`.
//!
//! ## Problem (pre-Phase-6)
//!
//! `apply_anomalies` called `palette::blend_toward_white(fg, intensity)`
//! for both `AnomalyKind::LuminanceSurge` and `AnomalyKind::PulseWave`.
//! The halo target was hardcoded to pure white `(255, 255, 255)`,
//! regardless of the active palette. On saturated themes (NeonRed, Aurora,
//! Fire, Ocean), the white halo broke the scene's color coherence — a
//! bright flash of pure white against an otherwise saturated scene reads
//! as an unrelated overlay, not a natural extension of the rain.
//!
//! This was the same problem Phase 3-I solved for ghost events: the ghost
//! base color was hardcoded to `(18, 22, 18)` (dark green) and clashed
//! with every non-Green theme. Phase 3-I made ghosts palette-aware by
//! deriving the ghost color from `palette_colors[0]` (the darkest stop).
//! Phase 6 applies the same pattern to anomaly halos.
//!
//! ## Solution
//!
//! `anomaly_halo_target(palette_colors, mode, elapsed)` derives the halo
//! target color from the active palette:
//!
//! - **`LuminanceSurge` mode** → `palette_colors.last()` (the brightest
//!   stop). A "luminous surge" should lift cells toward the palette's
//!   natural ceiling, not toward an external pure-white reference. On a
//!   NeonRed theme, the brightest stop is bright red — the surge becomes
//!   a "lift toward bright red" rather than "lift toward white",
//!   preserving palette coherence. On themes whose brightest stop IS
//!   near-white (Snow, Gray), behavior is essentially unchanged from
//!   pre-Phase-6.
//!
//! - **`PulseWave mode** → a hue-cycled palette stop:
//!   `palette_colors[(elapsed * ANOMALY_HALO_CYCLE_RATE) as usize %
//!   palette_colors.len()]`. The expanding ring's target color cycles
//!   through palette stops as it expands, giving PulseWave a distinct
//!   visual identity from LuminanceSurge. Where LuminanceSurge is a
//!   "static ceiling lift", PulseWave is a "hue-cycling ring" — the
//!   ring's color changes as it sweeps outward, producing a chromatic
//!   pulse rather than a monochrome flash.
//!
//! ## Why a chroma-side enum?
//!
//! `cloud::state::AnomalyKind` is `pub(super)` — visible only within
//! `cloud::*`. Rather than widen its visibility (which would leak a
//! cloud-internal taxonomy into the public API), Phase 6 defines a
//! small chroma-side `AnomalyHaloMode` enum that captures only what
//! the helper cares about: "static target" vs. "hue-cycled target".
//!
//! `cloud::phosphor::apply_anomalies` maps its `AnomalyKind` to the
//! appropriate `AnomalyHaloMode` (or skips the helper entirely for
//! `AnomalyKind::GlyphCorruption`, which doesn't modify color). This
//! keeps the chroma module decoupled from cloud's anomaly taxonomy —
//! future cloud AnomalyKind variants can map onto the existing modes
//! or extend this enum without cross-module visibility changes.
//!
//! ## Fallback
//!
//! Returns `None` when:
//! - `palette_colors` is empty (no palette to derive from — caller
//!   should fall back to `blend_toward_white`)
//! - The selected stop is `Color::Reset` (no RGB to blend toward)
//!
//! `cloud::phosphor::apply_anomalies` falls back to
//! `blend_toward_white` when `None` is returned, preserving the
//! pre-Phase-6 behavior for edge cases (empty palette, Reset stop).
//!
//! ## Cost
//!
//! One slice indexing + one `Color::Reset` match per anomaly-affected
//! cell. Anomaly zones are rare (~5% of frames) and small (radius 3–8
//! cells), so the per-frame cost is negligible — ~20 calls per anomaly
//! frame, vs. ~500 calls per frame for the base shader.

use crossterm::style::Color;

use crate::chroma::tuning::ANOMALY_HALO_CYCLE_RATE;

/// Discriminator for the anomaly halo target color derivation.
///
/// See the module-level doc comment for the full rationale. Maps from
/// `cloud::state::AnomalyKind` at the call site in
/// `cloud::phosphor::apply_anomalies`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AnomalyHaloMode {
    /// Static target: the palette's brightest stop
    /// (`palette_colors.last()`). Used by `AnomalyKind::LuminanceSurge`
    /// — a "lift toward the palette's natural ceiling".
    LuminanceSurge,

    /// Hue-cycled target: `palette_colors[(elapsed * rate) %
    /// palette_colors.len()]`. Used by `AnomalyKind::PulseWave` —
    /// the expanding ring's target color cycles through palette stops
    /// as it expands, producing a chromatic pulse.
    PulseWave,
}

/// Derive the anomaly halo target color from the active palette.
///
/// Takes the palette's color slice, the halo mode (static vs. hue-cycled),
/// and the anomaly's elapsed time in seconds (only used by `PulseWave`
/// mode). Returns the target `Color` for `blend_toward_bg`, or `None`
/// when no palette-derived target is available (empty palette or
/// `Color::Reset` stop).
///
/// ## When `None` is returned
///
/// Callers should fall back to `palette::blend_toward_white(fg, intensity)`
/// when `None` is returned. This preserves the pre-Phase-6 behavior for
/// edge cases (e.g., a palette whose brightest stop is `Color::Reset`,
/// which can happen in `ColorMode::Mono` where the palette is just
/// `[Color::White]` — but in that case the cell is already white, so the
/// halo is a no-op anyway).
///
/// ## Determinism
///
/// The same `(palette_colors, mode, elapsed)` triple always produces the
/// same output. `PulseWave` mode's hue-cycle is deterministic given
/// `elapsed` — no internal randomness, no frame-count dependency.
///
/// ## Example
///
/// ```ignore
/// use crate::chroma::post::anomaly::{anomaly_halo_target, AnomalyHaloMode};
/// use crate::chroma::palette::{blend_toward_bg, blend_toward_white};
///
/// // LuminanceSurge: static target = palette's brightest stop
/// let palette = [Color::Rgb { r: 0, g: 100, b: 0 }, Color::Rgb { r: 200, g: 255, b: 200 }];
/// let target = anomaly_halo_target(&palette, AnomalyHaloMode::LuminanceSurge, 0.0);
/// assert_eq!(target, Some(Color::Rgb { r: 200, g: 255, b: 200 }));
///
/// // Apply: blend the cell's existing fg toward the palette-derived target
/// let brightened = match target {
///     Some(t) => blend_toward_bg(fg, t, intensity),
///     None => blend_toward_white(fg, intensity), // fallback
/// };
/// ```
#[must_use]
pub(crate) fn anomaly_halo_target(
    palette_colors: &[Color],
    mode: AnomalyHaloMode,
    elapsed: f32,
) -> Option<Color> {
    if palette_colors.is_empty() {
        return None;
    }
    let idx = match mode {
        AnomalyHaloMode::LuminanceSurge => palette_colors.len() - 1,
        AnomalyHaloMode::PulseWave => {
            // Hue-cycle through palette stops at ANOMALY_HALO_CYCLE_RATE
            // stops/sec. `elapsed` is the anomaly's age in seconds, so the
            // target cycles independently of frame rate. The `as usize`
            // truncation is fine — we want a discrete stop index, not a
            // continuous interpolation between stops (the halo is a flash,
            // not a gradient).
            //
            // `max(0.0)` guards against negative elapsed (shouldn't happen
            // — `Instant::saturating_duration_since` clamps to 0 — but
            // defensive against future caller bugs).
            let t = (elapsed * ANOMALY_HALO_CYCLE_RATE).max(0.0) as usize;
            t % palette_colors.len()
        }
    };
    let target = palette_colors[idx];
    // Color::Reset has no RGB to blend toward — fall back to caller's
    // blend_toward_white. This is rare (only happens if the palette's
    // brightest/selected stop is Reset, which is unusual but possible
    // in degraded modes).
    if matches!(target, Color::Reset) {
        return None;
    }
    Some(target)
}

#[cfg(test)]
mod tests;
