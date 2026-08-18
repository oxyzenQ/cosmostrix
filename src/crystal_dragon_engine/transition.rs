// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Crystal Dragon transition: hooks to Chroma Dragon for OKLab smooth fades.
//!
//! When the Crystal Dragon engine selects a new color theme, this module
//! bridges to the existing Chroma Dragon transition system. The new
//! theme is applied via `Cloud::set_color_scheme()`, which:
//!
//! 1. Advances the palette circular buffer slot.
//! 2. Stores the new palette.
//! 3. Sets `transition_start = Some(Instant::now())`.
//! 4. Triggers a 300 ms top-to-bottom OKLab wave transition.
//!
//! The Crystal Dragon engine does NOT implement its own transition — it
//! delegates entirely to the Chroma Dragon's proven wave shader. This
//! ensures all 6 color-change paths use the same smooth transition.

use crate::runtime::ColorScheme;

/// Result of a Crystal Dragon drift tick.
///
/// Returned by `crystal_dragon_tick()` so the caller (rain.rs) can
/// apply the transition via `Cloud::set_color_scheme()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum CrystalDragonDrift {
    /// No drift this tick. Current theme remains.
    NoDrift,
    /// Drift to a new color theme. The caller should apply via
    /// `Cloud::set_color_scheme(new_scheme)` which triggers the
    /// 300 ms OKLab wave transition.
    Drift(ColorScheme),
}

#[allow(dead_code)]
impl CrystalDragonDrift {
    /// The new scheme, if this is a `Drift` variant.
    pub(crate) fn scheme(self) -> Option<ColorScheme> {
        match self {
            CrystalDragonDrift::NoDrift => None,
            CrystalDragonDrift::Drift(s) => Some(s),
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_drift_returns_none() {
        assert_eq!(CrystalDragonDrift::NoDrift.scheme(), None);
    }

    #[test]
    fn drift_returns_scheme() {
        let drift = CrystalDragonDrift::Drift(ColorScheme::Snow);
        assert_eq!(drift.scheme(), Some(ColorScheme::Snow));
    }

    #[test]
    fn drift_is_copy() {
        let drift = CrystalDragonDrift::Drift(ColorScheme::Fire);
        let copy = drift;
        assert_eq!(copy.scheme(), Some(ColorScheme::Fire));
    }
}
