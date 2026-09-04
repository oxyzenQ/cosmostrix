// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Crystal Dragon calc-v2: pattern state machine with memory.
//!
//! calc-v1 (the default) uses pure probabilistic weighted selection —
//! each drift event is independent, with no memory of past selections.
//! This produces organic, unpredictable transitions but can sometimes
//! pick the same theme repeatedly (the skip-current retry mitigates
//! this, but doesn't prevent A→B→A→B oscillation).
//!
//! calc-v2 adds a recency penalty: themes that were recently
//! selected get a lower weight, so the engine naturally avoids
//! repetition and oscillation. The result is more varied, cinematic
//! drift — the rain feels like it's exploring the palette, not
//! stuck on 2-3 themes.
//!
//! ## Algorithm
//!
//! 1. Same as calc-v1: determine group, compute distance-based weights.
//! 2. NEW: multiply each weight by a recency factor based on how
//!    many drifts ago the theme was last selected:
//!    - Never selected → factor 1.0 (no penalty)
//!    - Selected 1 drift ago → factor 0.3 (strong penalty, avoids A→B→A)
//!    - Selected 2 drifts ago → factor 0.6 (medium penalty)
//!    - Selected 3 drifts ago → factor 0.8 (light penalty)
//!    - Selected 4+ drifts ago → factor 1.0 (no penalty, memory fades)
//! 3. Build CDF from the adjusted weights, draw, skip current.
//!
//! The recency history is bounded (8 entries) — a ring buffer of the
//! last 8 selected themes. This bounds memory (8 × 1 byte = 8 bytes)
//! and ensures old selections eventually lose their penalty.
//!
//! ## Design preservation
//!
//! calc-v2 does NOT change the point system, temperature groups, or
//! the distance-based weighting. It only adds a recency multiplier on
//! top of the existing weights. The drift gate, polling interval, and
//! drift chance are unchanged. The owner's design intent (low CPU =
//! cooler, high CPU = hotter) is fully preserved — calc-v2 only
//! affects WHICH theme within a group is selected, not WHICH group.

use rand::distr::{Distribution, Uniform};
use rand::rngs::StdRng;

use crate::crystal_dragon_engine::crystal_dragon_control::CRYSTAL_DRAGON_MAX_THEMES_PER_GROUP;
use crate::crystal_dragon_engine::palette_groups::{group_themes, theme_weight};
use crate::crystal_dragon_engine::sensor::point_to_group;
use crate::runtime::ColorScheme;

/// Maximum number of recent selections tracked by calc-v2's recency
/// ring buffer. 8 entries = 8 × 1 byte = 8 bytes memory. At the
/// S-master-HUNT-7 deterministic boundary cadence (one drift decision
/// per poll cycle — every 2 cycles apart with ambient OFF, where the
/// visibility window owns one full cycle) 8 entries covers ~8 minutes
/// of drift history at the 60s default (8 × 60s ambient-on rhythm,
/// ~16 minutes ambient-off). Beyond that, the oldest entry is
/// overwritten — memory fades naturally.
const CALC_V2_HISTORY_SIZE: usize = 8;

/// Recency penalty factors. Index = drifts ago (1 = most recent, 4+
/// = no penalty). The factor multiplies the distance-based weight.
///
/// - 1 drift ago: 0.3 (strong — prevents A→B→A oscillation)
/// - 2 drifts ago: 0.6 (medium — prevents A→B→C→A cycling)
/// - 3 drifts ago: 0.8 (light — allows re-selection after a pause)
/// - 4+ drifts ago: 1.0 (no penalty — memory fades)
const RECENCY_FACTORS: [f32; 4] = [0.3, 0.6, 0.8, 1.0];

/// Recency history for calc-v2. A bounded ring buffer of the last
/// `CALC_V2_HISTORY_SIZE` selected themes.
///
/// Stored as a `Cloud` field so it survives across drift events.
/// Carries across live-reload via `inherit_ecosystem_state` (same
/// as the sensor state).
#[derive(Clone, Copy)]
pub(crate) struct DriftHistory {
    /// Ring buffer of recently selected themes. `None` = slot never used.
    entries: [Option<ColorScheme>; CALC_V2_HISTORY_SIZE],
    /// Next write index (wraps around).
    next: usize,
    /// Number of entries written so far (caps at CALC_V2_HISTORY_SIZE).
    count: usize,
}

impl DriftHistory {
    pub(crate) const fn new() -> Self {
        Self {
            entries: [None; CALC_V2_HISTORY_SIZE],
            next: 0,
            count: 0,
        }
    }

    /// Record a theme selection. Called after a drift fires.
    pub(crate) fn record(&mut self, scheme: ColorScheme) {
        self.entries[self.next] = Some(scheme);
        self.next = (self.next + 1) % CALC_V2_HISTORY_SIZE;
        if self.count < CALC_V2_HISTORY_SIZE {
            self.count += 1;
        }
    }

    /// Compute the recency factor for a theme. Returns a multiplier
    /// (0.0–1.0) based on how recently the theme was selected.
    ///
    /// - Never selected → 1.0 (no penalty)
    /// - Selected 1 drift ago → 0.3
    /// - Selected 2 drifts ago → 0.6
    /// - etc.
    fn recency_factor(&self, scheme: ColorScheme) -> f32 {
        // Search the ring buffer from most recent to oldest.
        // The most recent entry is at (next - 1 + SIZE) % SIZE.
        for drifts_ago in 1..=self.count.min(RECENCY_FACTORS.len()) {
            let idx = (self.next + CALC_V2_HISTORY_SIZE - drifts_ago) % CALC_V2_HISTORY_SIZE;
            if self.entries[idx] == Some(scheme) {
                return RECENCY_FACTORS[drifts_ago - 1];
            }
        }
        // Not found in history (or history empty) → no penalty.
        1.0
    }
}

/// Select a new color theme using calc-v2 (pattern state machine with memory).
///
/// Same as calc-v1 but applies a recency penalty: themes recently
/// selected get lower weight, preventing A→B→A oscillation and
/// producing more varied, cinematic drift.
///
/// `history` tracks recent selections and is updated by the caller
/// after a successful drift.
pub(crate) fn calc_v2_select(
    current_point: u8,
    current_scheme: ColorScheme,
    history: &DriftHistory,
    mt: &mut StdRng,
) -> Option<ColorScheme> {
    let group = point_to_group(current_point);
    let themes = group_themes(group);
    if themes.is_empty() {
        return None;
    }

    let n = themes.len();
    let mut weights = [0.0f32; CRYSTAL_DRAGON_MAX_THEMES_PER_GROUP];
    for (i, slot) in weights.iter_mut().enumerate().take(n) {
        // Base weight from distance (same as calc-v1).
        let base = theme_weight(current_point, i, n);
        // Recency penalty: themes recently selected get lower weight.
        let recency = history.recency_factor(themes[i]);
        *slot = base * recency;
    }

    // Build CDF from adjusted weights.
    let total_weight: f32 = weights[..n].iter().sum();
    if total_weight <= 0.0 {
        return uniform_select(themes, current_scheme, mt);
    }

    let mut cdf = [0.0f32; CRYSTAL_DRAGON_MAX_THEMES_PER_GROUP];
    let mut cumulative = 0.0f32;
    for (i, &w) in weights[..n].iter().enumerate() {
        cumulative += w / total_weight;
        cdf[i] = cumulative;
    }
    if n > 0 {
        cdf[n - 1] = 1.0;
    }

    let selected = cdf_select(&cdf[..n], themes, mt);

    if selected == current_scheme {
        let retry = cdf_select(&cdf[..n], themes, mt);
        if retry != current_scheme {
            return Some(retry);
        }
        return None;
    }

    Some(selected)
}

// ── Shared helpers (used by both calc-v1 and calc-v2) ────────────────────

/// Uniform fallback: select a random theme, skipping current.
pub(crate) fn uniform_select(
    themes: &[ColorScheme],
    current_scheme: ColorScheme,
    mt: &mut StdRng,
) -> Option<ColorScheme> {
    if themes.len() <= 1 {
        return None;
    }
    let dist = Uniform::new_inclusive(0usize, themes.len().saturating_sub(1))
        .expect("uniform idx always valid");
    let mut idx = dist.sample(mt);
    for _ in 0..themes.len() {
        if themes[idx] != current_scheme {
            return Some(themes[idx]);
        }
        idx = (idx + 1) % themes.len();
    }
    None
}

/// Draw a theme from the CDF via binary search.
pub(crate) fn cdf_select(cdf: &[f32], themes: &[ColorScheme], mt: &mut StdRng) -> ColorScheme {
    let u_dist = Uniform::new(0.0f32, 1.0f32).expect("uniform f32 always valid");
    let u = u_dist.sample(mt);
    let idx = cdf.partition_point(|&c| c < u);
    themes[idx.min(themes.len() - 1)]
}

// ── calc-v1 (preserved — legacy option, no memory) ──────────────────────

/// Select a new color theme using calc-v1 (probabilistic weighted, no memory).
///
/// calc-v2 (with recency memory) is the DEFAULT since Dragon Engine v2 —
/// see `CrystalDragonCalcMethod::CalcV2` in `crystal_dragon_control`.
/// calc-v1 remains available as the legacy option when the control config
/// selects `CrystalDragonCalcMethod::Calc`.
pub(crate) fn calc_v1_select(
    current_point: u8,
    current_scheme: ColorScheme,
    mt: &mut StdRng,
) -> Option<ColorScheme> {
    let group = point_to_group(current_point);
    let themes = group_themes(group);
    if themes.is_empty() {
        return None;
    }

    let n = themes.len();
    let mut weights = [0.0f32; CRYSTAL_DRAGON_MAX_THEMES_PER_GROUP];
    for (i, slot) in weights.iter_mut().enumerate().take(n) {
        *slot = theme_weight(current_point, i, n);
    }

    let total_weight: f32 = weights[..n].iter().sum();
    if total_weight <= 0.0 {
        return uniform_select(themes, current_scheme, mt);
    }

    let mut cdf = [0.0f32; CRYSTAL_DRAGON_MAX_THEMES_PER_GROUP];
    let mut cumulative = 0.0f32;
    for (i, &w) in weights[..n].iter().enumerate() {
        cumulative += w / total_weight;
        cdf[i] = cumulative;
    }
    if n > 0 {
        cdf[n - 1] = 1.0;
    }

    let selected = cdf_select(&cdf[..n], themes, mt);

    if selected == current_scheme {
        let retry = cdf_select(&cdf[..n], themes, mt);
        if retry != current_scheme {
            return Some(retry);
        }
        return None;
    }

    Some(selected)
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "../../../../test/engine/crystal_dragon_engine/point_system/tests.rs"]
mod tests;
