// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Crystal Dragon palette groups: 44 themes partitioned into temperature groups.
//!
//! Each of the 44 builtin `ColorScheme` variants is assigned to exactly one
//! temperature group:
//!
//! | Group | Count | Aesthetic |
//! |-------|-------|-----------|
//! | Cold  | 14    | Cool, calm, serene — Snow, Moon, Ocean, … |
//! | Medium | 14   | Balanced, natural — Green, Forest, Aurora, … |
//! | Hot   | 14    | Warm, fiery, energetic — Sun, Fire, Red, … |
//! | Reserved | 2  | Not assigned to any temperature group |
//!
//! **Reserved** themes (Rainbow, Spectrum20) span the full color spectrum
//! and don't fit a single temperature. They are excluded from Crystal
//! Dragon drift.

use crate::runtime::ColorScheme;

// ── Temperature group enum ───────────────────────────────────────────────

/// Temperature group for the Crystal Dragon point system.
///
/// Each group contains 14 color themes that share a color temperature
/// aesthetic. The point system maps points 1–99 to these groups:
///
/// - **Cold** (1–33): cool blues, grays, whites, cyans
/// - **Medium** (34–66): greens, purples, natural tones
/// - **Hot** (67–99): warm yellows, oranges, reds, fiery tones
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TemperatureGroup {
    Cold,
    Medium,
    Hot,
}

#[allow(dead_code)]
impl TemperatureGroup {
    /// Stable string label for diagnostics (`--doctor`, logs).
    pub(crate) fn label(self) -> &'static str {
        match self {
            TemperatureGroup::Cold => "cold",
            TemperatureGroup::Medium => "medium",
            TemperatureGroup::Hot => "hot",
        }
    }
}

// ── Group → themes mapping ───────────────────────────────────────────────

/// Return all color themes belonging to a temperature group.
///
/// Each group contains exactly 14 themes. The returned slice is
/// non-empty and stable (same order across calls).
pub(crate) fn group_themes(group: TemperatureGroup) -> &'static [ColorScheme] {
    use ColorScheme::*;
    match group {
        TemperatureGroup::Cold => &[
            // Cool blues & cyans (7)
            Blue, Ocean, Neptune, Uranus, Cyan, NeonBlue, NeonCyan,
            // Neutrals, whites, grays (7)
            Snow, Moon, Stars, Gray, Mercury, Carbon, NeonWhite,
        ],
        TemperatureGroup::Medium => &[
            // Greens & forest (6)
            Green,
            Green2,
            Green3,
            NeonGreen,
            Forest,
            Aurora,
            // Purples & cosmic (7)
            Purple,
            Nebula,
            Cosmos,
            Vaporwave,
            Neon,
            FancyDiamond,
            NeonPurple,
            // Transitional neutral
            Pluto,
        ],
        TemperatureGroup::Hot => &[
            // Warm yellows & oranges (9)
            Gold, Yellow, Orange, Sun, Venus, Jupiter, Saturn, NeonOrange, NeonYellow,
            // Fiery reds (4)
            Red, Fire, Mars, NeonRed, // Premium exclusive
            EnergyZen,
        ],
    }
}

/// Return the reserved themes not assigned to any temperature group.
///
/// Reserved themes (Rainbow, Spectrum20) span the full color spectrum
/// and are excluded from Crystal Dragon drift. They are still available
/// via explicit `--color` selection.
#[allow(dead_code)]
pub(crate) fn reserved_themes() -> &'static [ColorScheme] {
    use ColorScheme::*;
    &[Rainbow, Spectrum20]
}

// ── Theme → group reverse lookup ─────────────────────────────────────────

/// Classify a color scheme into its temperature group.
///
/// Returns `None` for reserved themes (Rainbow, Spectrum20) which
/// are not assigned to any group.
///
/// This is the **reverse** of `group_themes()` — every theme that
/// appears in a `group_themes()` slice is classified into that group,
/// and every reserved theme returns `None`.
#[allow(dead_code)]
pub(crate) fn theme_group(scheme: ColorScheme) -> Option<TemperatureGroup> {
    use ColorScheme::*;
    match scheme {
        // Cold: blues, cyans, neutrals
        Blue | Ocean | Neptune | Uranus | Cyan | NeonBlue | NeonCyan | Snow | Moon | Stars
        | Gray | Mercury | Carbon | NeonWhite => Some(TemperatureGroup::Cold),

        // Medium: greens, purples, natural
        Green | Green2 | Green3 | NeonGreen | Forest | Aurora | Purple | Nebula | Cosmos
        | Vaporwave | Neon | FancyDiamond | NeonPurple | Pluto => Some(TemperatureGroup::Medium),

        // Hot: warm, fiery
        Gold | Yellow | Orange | Sun | Venus | Jupiter | Saturn | NeonOrange | NeonYellow | Red
        | Fire | Mars | NeonRed | EnergyZen => Some(TemperatureGroup::Hot),

        // Reserved: full-spectrum, not in any group
        Rainbow | Spectrum20 => None,
    }
}

// ── Point weight for probabilistic selection ─────────────────────────────

/// Compute a weight for a theme within a group based on the current point.
///
/// The weight is higher for themes whose "natural point" is closer to
/// the current point. This biases selection toward themes that match
/// the current system state intensity, while still allowing any theme
/// in the group to be selected (probabilistic, not deterministic).
///
/// Weight formula:
/// ```text
/// distance = |current_point - theme_natural_point|
/// weight = 1.0 / (1.0 + distance as f32 * 0.1)
/// ```
///
/// At distance 0 → weight 1.0 (maximum).
/// At distance 33 → weight ~0.23 (still selectable).
pub(crate) fn theme_weight(current_point: u8, theme_index: usize, group_size: usize) -> f32 {
    // Map theme_index (0..group_size-1) to a natural point within the
    // group's range. This distributes themes evenly across the group.
    let group = super::sensor::point_to_group(current_point);
    let (lo, hi) = super::sensor::group_point_range(group);
    let range = (hi - lo) as f32;
    let natural_point = if group_size <= 1 {
        (lo + hi) as f32 / 2.0
    } else {
        lo as f32 + (theme_index as f32 / (group_size - 1) as f32) * range
    };
    let distance = (current_point as f32 - natural_point).abs();
    1.0 / (1.0 + distance * 0.1)
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "palette_groups_tests.rs"]
mod tests;
