// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Rain visual preset selection.
//!
//! A `RainPreset` is a runtime-switchable bundle of per-layer tuning
//! constants that controls the *visual character* of the rain field —
//! how long streaks are, how fast they fade, how fast they fall, and
//! how many of them are short vs long.
//!
//! ## Presets
//!
//! - `Cinematic` (default): The "Film Matrix Hero" Option F lock from
//!   `RAIN_DEPTH_AUDIT.md`. Long front-layer streaks, slow phosphor
//!   afterglow, fast front whoosh. Rated 10/10 cinematic reference.
//!
//! - `Organic`: Shorter streaks, faster phosphor fade, slower front
//!   speed, more short-droplet variation. Reads as "rain on a window"
//!   rather than "digital data stream". Use the `'r'` key at runtime
//!   to cycle between presets.
//!
//! ## What a preset controls
//!
//! Each preset returns four per-layer arrays:
//!
//! - `parallax_length_mult` — droplet streak length multiplier per layer
//! - `parallax_speed_mult` — droplet fall speed multiplier per layer
//! - `phosphor_layer_decay_mult` — phosphor afterglow fade rate per layer
//! - `short_pct` — fraction of droplets that spawn at short length
//!
//! ## What a preset does NOT control
//!
//! Brightness, saturation, head bloom, density, contrast reduction,
//! vignette, and bottom shadow are NOT preset-scoped — they are part
//! of the Option F depth stack and remain constant across presets.
//! This keeps the depth character (back hazy, front hero-pop) intact
//! regardless of which rain character is active.

use crate::central_control_rains::PARALLAX_LAYERS;

/// Runtime-switchable rain visual preset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RainPreset {
    /// "Film Matrix Hero" Option F lock — long streaks, slow fade, fast front.
    Cinematic,
    /// "Rain on a window" — short streaks, fast fade, slower front, more variation.
    Organic,
}

impl RainPreset {
    /// Human-readable preset name (for HUD / status display).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cinematic => "cinematic",
            Self::Organic => "organic",
        }
    }

    /// Cycle to the next preset. Order: Cinematic → Organic → Cinematic.
    /// Future presets can be inserted here without breaking callers.
    #[must_use]
    pub fn cycle(self) -> Self {
        match self {
            Self::Cinematic => Self::Organic,
            Self::Organic => Self::Cinematic,
        }
    }

    /// Per-layer droplet streak length multiplier.
    #[must_use]
    pub fn parallax_length_mult(self) -> [f32; PARALLAX_LAYERS] {
        match self {
            Self::Cinematic => crate::central_control_rains::PARALLAX_LENGTH_MULT,
            Self::Organic => ORGANIC_PARALLAX_LENGTH_MULT,
        }
    }

    /// Per-layer droplet fall speed multiplier.
    #[must_use]
    pub fn parallax_speed_mult(self) -> [f32; PARALLAX_LAYERS] {
        match self {
            Self::Cinematic => crate::central_control_rains::PARALLAX_SPEED_MULT,
            Self::Organic => ORGANIC_PARALLAX_SPEED_MULT,
        }
    }

    /// Per-layer phosphor afterglow decay rate multiplier.
    #[must_use]
    pub fn phosphor_layer_decay_mult(self) -> [f32; PARALLAX_LAYERS] {
        match self {
            Self::Cinematic => crate::central_control_rains::PHOSPHOR_LAYER_DECAY_MULT,
            Self::Organic => ORGANIC_PHOSPHOR_LAYER_DECAY_MULT,
        }
    }

    /// Fraction of droplets that spawn at short length (0.0–1.0).
    #[must_use]
    pub fn short_pct(self) -> f32 {
        match self {
            Self::Cinematic => 0.5,
            Self::Organic => 0.7,
        }
    }
}

// ─── Organic preset tuning ─────────────────────────────────────────────────
//
// Derived from the rain-length research audit (worklog: rain-length-research).
// The Organic preset inverts the cinematic "long streak + slow fade" character
// on all three layers, producing shorter, faster-fading, more varied droplets
// that read as natural rain rather than digital data.
//
// Reference: Cinematic baseline values live in central_control_rains.rs.

/// Organic per-layer length multiplier.
///
/// Cinematic baseline `[0.5, 1.0, 1.4]` produces long front-layer streaks
/// (1.4× screen height). Organic `[0.4, 0.6, 0.8]` caps front at 0.8× —
/// short enough to read as individual droplets, not vertical lines. Back
/// layer also reduced 0.5→0.4 to keep depth perspective coherent.
pub const ORGANIC_PARALLAX_LENGTH_MULT: [f32; PARALLAX_LAYERS] = [0.4, 0.6, 0.8];

/// Organic per-layer speed multiplier.
///
/// Cinematic baseline `[0.35, 1.0, 1.7]` gives front a 1.7× whoosh for
/// cinematic "data stream" feel. Organic `[0.4, 0.9, 1.3]` slows front to
/// 1.3× — droplets feel "heavier" like real rain, not "rushing data".
/// Back raised 0.35→0.4 to keep distant motion perceptible despite shorter
/// streaks.
pub const ORGANIC_PARALLAX_SPEED_MULT: [f32; PARALLAX_LAYERS] = [0.4, 0.9, 1.3];

/// Organic per-layer phosphor decay multiplier.
///
/// Cinematic baseline `[2.0, 1.2, 0.6]` gives front a 0.6× decay (slow
/// fade, long afterglow = "trail"). Organic `[2.5, 1.8, 1.5]` raises front
/// to 1.5× — afterglow fades ~2.5× faster, so droplets read as discrete
/// falling objects rather than persistent vertical lines. Mid and back
/// also raised to keep the depth fade hierarchy intact (back still fades
/// fastest, front still slowest — just everything faster than cinematic).
pub const ORGANIC_PHOSPHOR_LAYER_DECAY_MULT: [f32; PARALLAX_LAYERS] = [2.5, 1.8, 1.5];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_returns_to_start_after_two_steps() {
        let start = RainPreset::Cinematic;
        let after_one = start.cycle();
        let after_two = after_one.cycle();
        assert_eq!(after_one, RainPreset::Organic);
        assert_eq!(after_two, RainPreset::Cinematic);
    }

    #[test]
    fn organic_length_mult_is_shorter_than_cinematic_on_every_layer() {
        let cin = RainPreset::Cinematic.parallax_length_mult();
        let org = RainPreset::Organic.parallax_length_mult();
        for i in 0..PARALLAX_LAYERS {
            assert!(
                org[i] < cin[i],
                "organic layer {} length {} should be < cinematic {}",
                i,
                org[i],
                cin[i]
            );
        }
    }

    #[test]
    fn organic_front_phosphor_decay_is_faster_than_cinematic() {
        // Higher decay mult = faster fade = shorter afterglow
        let cin = RainPreset::Cinematic.phosphor_layer_decay_mult();
        let org = RainPreset::Organic.phosphor_layer_decay_mult();
        assert!(
            org[2] > cin[2],
            "organic front decay {} should be > cinematic {} (faster fade)",
            org[2],
            cin[2]
        );
    }

    #[test]
    fn organic_front_speed_is_slower_than_cinematic() {
        let cin = RainPreset::Cinematic.parallax_speed_mult();
        let org = RainPreset::Organic.parallax_speed_mult();
        assert!(
            org[2] < cin[2],
            "organic front speed {} should be < cinematic {} (heavier rain)",
            org[2],
            cin[2]
        );
    }

    #[test]
    fn organic_short_pct_is_higher_than_cinematic() {
        // More short droplets = more organic variation
        assert!(
            RainPreset::Organic.short_pct() > RainPreset::Cinematic.short_pct(),
            "organic should have more short droplets than cinematic"
        );
    }

    #[test]
    fn preset_layer_decay_hierarchy_preserved_in_organic() {
        // Depth hierarchy: back fades fastest, front slowest
        // (organic raises all values but preserves ordering)
        let org = RainPreset::Organic.phosphor_layer_decay_mult();
        assert!(
            org[0] > org[1] && org[1] > org[2],
            "organic decay hierarchy broken: {:?} (expected back > mid > front)",
            org
        );
    }

    #[test]
    fn as_str_returns_lowercase_identifier() {
        assert_eq!(RainPreset::Cinematic.as_str(), "cinematic");
        assert_eq!(RainPreset::Organic.as_str(), "organic");
    }
}
