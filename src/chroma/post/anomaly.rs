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
pub enum AnomalyHaloMode {
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
pub fn anomaly_halo_target(
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
mod tests {
    use super::*;

    /// Helper: build a small test palette of N RGB colors.
    fn make_palette(n: usize) -> Vec<Color> {
        (0..n)
            .map(|i| {
                let v = (i * 50) as u8;
                Color::Rgb { r: v, g: v, b: v }
            })
            .collect()
    }

    // ── LuminanceSurge mode ───────────────────────────────────────────

    /// LuminanceSurge on a non-empty palette returns the LAST (brightest)
    /// stop. This is the core Phase 6 contract — anomaly halos lift toward
    /// the palette's natural ceiling, not toward pure white.
    #[test]
    fn luminance_surge_returns_brightest_stop() {
        let palette = vec![
            Color::Rgb { r: 0, g: 0, b: 0 },
            Color::Rgb {
                r: 100,
                g: 100,
                b: 100,
            },
            Color::Rgb {
                r: 255,
                g: 255,
                b: 255,
            },
        ];
        let target = anomaly_halo_target(&palette, AnomalyHaloMode::LuminanceSurge, 0.0);
        assert_eq!(
            target,
            Some(Color::Rgb {
                r: 255,
                g: 255,
                b: 255,
            })
        );
    }

    /// LuminanceSurge on a saturated-theme palette returns the saturated
    /// brightest stop, not pure white. This is the palette-coherence
    /// guarantee: on a NeonRed theme, the halo target is bright red.
    #[test]
    fn luminance_surge_returns_saturated_brightest_stop() {
        let palette = vec![
            Color::Rgb { r: 30, g: 0, b: 0 },
            Color::Rgb {
                r: 180,
                g: 20,
                b: 20,
            },
            Color::Rgb {
                r: 255,
                g: 60,
                b: 60,
            },
        ];
        let target = anomaly_halo_target(&palette, AnomalyHaloMode::LuminanceSurge, 0.0);
        assert_eq!(
            target,
            Some(Color::Rgb {
                r: 255,
                g: 60,
                b: 60
            })
        );
    }

    /// LuminanceSurge is time-invariant — the same palette always returns
    /// the same target regardless of `elapsed`. This distinguishes it from
    /// PulseWave, which cycles over time.
    #[test]
    fn luminance_surge_is_time_invariant() {
        let palette = make_palette(5);
        let t0 = anomaly_halo_target(&palette, AnomalyHaloMode::LuminanceSurge, 0.0);
        let t1 = anomaly_halo_target(&palette, AnomalyHaloMode::LuminanceSurge, 1.0);
        let t2 = anomaly_halo_target(&palette, AnomalyHaloMode::LuminanceSurge, 10.0);
        assert_eq!(t0, t1);
        assert_eq!(t1, t2);
    }

    /// LuminanceSurge on a single-element palette returns that element.
    /// This handles degenerate palettes (e.g., Mono mode's `[Color::White]`).
    #[test]
    fn luminance_surge_single_element_palette() {
        let palette = vec![Color::Rgb {
            r: 128,
            g: 128,
            b: 128,
        }];
        let target = anomaly_halo_target(&palette, AnomalyHaloMode::LuminanceSurge, 0.0);
        assert_eq!(
            target,
            Some(Color::Rgb {
                r: 128,
                g: 128,
                b: 128,
            })
        );
    }

    // ── PulseWave mode ────────────────────────────────────────────────

    /// PulseWave at elapsed=0 returns stop index 0 (the darkest stop).
    /// This is the natural starting point for the hue cycle — the ring
    /// begins at the palette's darkest stop and cycles upward.
    #[test]
    fn pulse_wave_at_zero_returns_first_stop() {
        let palette = make_palette(9);
        let target = anomaly_halo_target(&palette, AnomalyHaloMode::PulseWave, 0.0);
        assert_eq!(target, Some(palette[0]));
    }

    /// PulseWave cycles through stops at ANOMALY_HALO_CYCLE_RATE.
    /// At rate=4.0 stops/sec and a 9-stop palette:
    ///   - t=0.0   → stop 0
    ///   - t=0.25  → stop 1
    ///   - t=0.50  → stop 2
    ///   - t=1.0   → stop 4
    ///   - t=2.25  → stop 9 % 9 = 0 (cycle complete)
    #[test]
    fn pulse_wave_cycles_through_stops_over_time() {
        let palette = make_palette(9);
        // Sanity: the palette must be long enough for the cycle to be
        // observable. 9 stops is the typical Snow theme size.
        assert_eq!(palette.len(), 9);

        // t=0 → stop 0
        let t0 = anomaly_halo_target(&palette, AnomalyHaloMode::PulseWave, 0.0);
        assert_eq!(t0, Some(palette[0]));

        // t=0.25 → stop 1 (4.0 * 0.25 = 1.0, truncated to 1)
        let t1 = anomaly_halo_target(&palette, AnomalyHaloMode::PulseWave, 0.25);
        assert_eq!(t1, Some(palette[1]));

        // t=0.50 → stop 2 (4.0 * 0.50 = 2.0)
        let t2 = anomaly_halo_target(&palette, AnomalyHaloMode::PulseWave, 0.50);
        assert_eq!(t2, Some(palette[2]));

        // t=1.0 → stop 4 (4.0 * 1.0 = 4.0)
        let t4 = anomaly_halo_target(&palette, AnomalyHaloMode::PulseWave, 1.0);
        assert_eq!(t4, Some(palette[4]));

        // t=2.25 → stop 9 % 9 = 0 (cycle complete)
        // 4.0 * 2.25 = 9.0, 9 % 9 = 0
        // Note: anomaly lifetime is 1.5s, so this elapsed value is past
        // the anomaly's death — but the helper itself doesn't enforce
        // that bound. The caller (apply_anomalies) skips expired zones
        // before calling this. The test verifies the wrap-around math.
        let t_cycle = anomaly_halo_target(&palette, AnomalyHaloMode::PulseWave, 2.25);
        assert_eq!(t_cycle, Some(palette[0]));
    }

    /// PulseWave wraps around correctly when the cycle exceeds the
    /// palette length. At rate=4.0 stops/sec on a 9-stop palette:
    ///   - t=2.50 → stop 10 % 9 = 1
    ///   - t=3.0  → stop 12 % 9 = 3
    #[test]
    fn pulse_wave_wraps_around_palette_length() {
        let palette = make_palette(9);

        // t=2.50 → 4.0 * 2.50 = 10, 10 % 9 = 1
        let t = anomaly_halo_target(&palette, AnomalyHaloMode::PulseWave, 2.50);
        assert_eq!(t, Some(palette[1]));

        // t=3.0 → 4.0 * 3.0 = 12, 12 % 9 = 3
        let t = anomaly_halo_target(&palette, AnomalyHaloMode::PulseWave, 3.0);
        assert_eq!(t, Some(palette[3]));
    }

    /// PulseWave on a single-element palette always returns that element,
    /// regardless of elapsed. The cycle is degenerate but well-defined.
    #[test]
    fn pulse_wave_single_element_palette() {
        let palette = vec![Color::Rgb {
            r: 100,
            g: 50,
            b: 200,
        }];
        let t0 = anomaly_halo_target(&palette, AnomalyHaloMode::PulseWave, 0.0);
        let t1 = anomaly_halo_target(&palette, AnomalyHaloMode::PulseWave, 1.0);
        let t2 = anomaly_halo_target(&palette, AnomalyHaloMode::PulseWave, 100.0);
        assert_eq!(t0, Some(palette[0]));
        assert_eq!(t1, Some(palette[0]));
        assert_eq!(t2, Some(palette[0]));
    }

    /// PulseWave with negative elapsed is clamped to 0 (defensive against
    /// future caller bugs — `Instant::saturating_duration_since` already
    /// clamps, but this test guards against regressions if the caller
    /// ever switches to `duration_since` without saturating).
    #[test]
    fn pulse_wave_negative_elapsed_clamps_to_zero() {
        let palette = make_palette(9);
        let t = anomaly_halo_target(&palette, AnomalyHaloMode::PulseWave, -1.0);
        // -1.0 * 4.0 = -4.0, .max(0.0) = 0.0, as usize = 0, 0 % 9 = 0
        assert_eq!(t, Some(palette[0]));
    }

    /// PulseWave is deterministic: same (palette, elapsed) → same output.
    /// No internal randomness, no frame-count dependency.
    #[test]
    fn pulse_wave_is_deterministic() {
        let palette = make_palette(9);
        let a = anomaly_halo_target(&palette, AnomalyHaloMode::PulseWave, 0.7);
        let b = anomaly_halo_target(&palette, AnomalyHaloMode::PulseWave, 0.7);
        assert_eq!(a, b);
    }

    // ── LuminanceSurge vs. PulseWave distinctness ─────────────────────

    /// LuminanceSurge and PulseWave produce DIFFERENT targets at most
    /// elapsed values, except when PulseWave happens to land on the
    /// brightest stop (idx == len-1). This is the core Phase 6 visual
    /// contract: the two anomaly kinds have distinct visual identities.
    #[test]
    fn luminance_surge_and_pulse_wave_diverge_at_most_times() {
        let palette = make_palette(9);
        // At t=0.5, PulseWave is on stop 2 (4.0 * 0.5 = 2.0).
        // LuminanceSurge is always on stop 8 (the brightest).
        // They must differ.
        let surge = anomaly_halo_target(&palette, AnomalyHaloMode::LuminanceSurge, 0.5);
        let pulse = anomaly_halo_target(&palette, AnomalyHaloMode::PulseWave, 0.5);
        assert_ne!(surge, pulse);
    }

    // ── Edge cases ────────────────────────────────────────────────────

    /// Empty palette returns None for both modes. The caller falls back
    /// to `blend_toward_white` in this case (preserves pre-Phase-6
    /// behavior for degenerate palettes).
    #[test]
    fn empty_palette_returns_none() {
        let empty: &[Color] = &[];
        assert_eq!(
            anomaly_halo_target(empty, AnomalyHaloMode::LuminanceSurge, 0.0),
            None
        );
        assert_eq!(
            anomaly_halo_target(empty, AnomalyHaloMode::PulseWave, 0.0),
            None
        );
    }

    /// Color::Reset at the selected stop returns None — the caller falls
    /// back to `blend_toward_white`. This is rare (only happens if the
    /// palette contains Reset stops, which is unusual but possible in
    /// degraded modes).
    #[test]
    fn reset_stop_returns_none() {
        // LuminanceSurge: brightest stop is Reset
        let palette_with_reset_brightest = vec![
            Color::Rgb { r: 0, g: 0, b: 0 },
            Color::Rgb {
                r: 100,
                g: 100,
                b: 100,
            },
            Color::Reset,
        ];
        assert_eq!(
            anomaly_halo_target(
                &palette_with_reset_brightest,
                AnomalyHaloMode::LuminanceSurge,
                0.0
            ),
            None
        );

        // PulseWave: cycled stop is Reset (palette of [Reset, Rgb] at t=0
        // selects idx 0 = Reset)
        let palette_with_reset_first = vec![
            Color::Reset,
            Color::Rgb {
                r: 100,
                g: 100,
                b: 100,
            },
        ];
        assert_eq!(
            anomaly_halo_target(&palette_with_reset_first, AnomalyHaloMode::PulseWave, 0.0),
            None
        );
    }

    /// Non-RGB color types (AnsiValue, named Ansi16) are returned as-is.
    /// The caller (`blend_toward_bg`) handles decoding them to RGB via
    /// `color_to_rgb`. This test verifies the helper doesn't panic or
    /// discard these types.
    #[test]
    fn non_rgb_color_types_pass_through() {
        let palette = vec![Color::AnsiValue(2)]; // ANSI green
        let target = anomaly_halo_target(&palette, AnomalyHaloMode::LuminanceSurge, 0.0);
        assert_eq!(target, Some(Color::AnsiValue(2)));
    }

    /// Large elapsed values don't overflow or panic. `as usize` truncation
    /// of large f32 + modulo keeps the index in range.
    #[test]
    fn large_elapsed_does_not_overflow() {
        let palette = make_palette(9);
        // 1 hour = 3600 sec * 4.0 = 14400 stops, 14400 % 9 = 0
        let t = anomaly_halo_target(&palette, AnomalyHaloMode::PulseWave, 3600.0);
        assert_eq!(t, Some(palette[0]));
    }

    /// Two-element palette: PulseWave alternates between stop 0 and stop 1.
    /// At rate=4.0 stops/sec: t=0.0 → 0, t=0.25 → 1, t=0.50 → 0, t=0.75 → 1.
    #[test]
    fn pulse_wave_two_element_palette_alternates() {
        let palette = vec![
            Color::Rgb { r: 0, g: 0, b: 0 },
            Color::Rgb {
                r: 255,
                g: 255,
                b: 255,
            },
        ];
        assert_eq!(
            anomaly_halo_target(&palette, AnomalyHaloMode::PulseWave, 0.0),
            Some(palette[0])
        );
        assert_eq!(
            anomaly_halo_target(&palette, AnomalyHaloMode::PulseWave, 0.25),
            Some(palette[1])
        );
        assert_eq!(
            anomaly_halo_target(&palette, AnomalyHaloMode::PulseWave, 0.50),
            Some(palette[0])
        );
        assert_eq!(
            anomaly_halo_target(&palette, AnomalyHaloMode::PulseWave, 0.75),
            Some(palette[1])
        );
    }
}
