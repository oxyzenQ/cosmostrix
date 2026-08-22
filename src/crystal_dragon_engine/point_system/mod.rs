// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Crystal Dragon point system: calc-v1 probabilistic weighted selection.
//!
//! Given the current point (1–99) from the sensor, this module selects
//! a color theme from the appropriate temperature group using a
//! probabilistic weighted algorithm.
//!
//! ## calc-v1 algorithm
//!
//! 1. Determine the temperature group from the current point.
//! 2. Compute a weight for each theme in the group based on distance
//!    from the current point (closer → higher weight).
//! 3. Build a cumulative distribution function (CDF) from the weights.
//! 4. Draw a uniform random value and binary-search the CDF.
//! 5. Skip the current scheme if it was selected (try once more, then
//!    accept — avoids infinite loops on single-theme groups).
//!
//! This produces **organic, unpredictable** transitions: any theme in
//! the group can be selected, but themes closer to the current system
//! intensity are favored.

use rand::distr::{Distribution, Uniform};

use crate::crystal_dragon_engine::palette_groups::{group_themes, theme_weight};
use crate::crystal_dragon_engine::sensor::point_to_group;
use crate::runtime::ColorScheme;

/// Select a new color theme using calc-v1 (probabilistic weighted).
///
/// `current_point` (1–99) determines the temperature group.
/// `current_scheme` is skipped if selected (no-op drift prevention).
/// `mt` is the RNG for probabilistic selection.
///
/// Returns `Some(new_scheme)` if a different theme was selected,
/// or `None` if the group has only one theme (impossible with 14
/// per group, but defensive).
pub(crate) fn calc_v1_select(
    current_point: u8,
    current_scheme: ColorScheme,
    mt: &mut rand::rngs::StdRng,
) -> Option<ColorScheme> {
    let group = point_to_group(current_point);
    let themes = group_themes(group);
    if themes.is_empty() {
        return None;
    }

    // Compute weights for each theme.
    let weights: Vec<f32> = themes
        .iter()
        .enumerate()
        .map(|(i, _)| theme_weight(current_point, i, themes.len()))
        .collect();

    // Build CDF (cumulative distribution function).
    let total_weight: f32 = weights.iter().sum();
    if total_weight <= 0.0 {
        // Degenerate: all weights zero. Fall back to uniform.
        return uniform_select(themes, current_scheme, mt);
    }

    let mut cdf: Vec<f32> = Vec::with_capacity(weights.len());
    let mut cumulative = 0.0f32;
    for &w in &weights {
        cumulative += w / total_weight;
        cdf.push(cumulative);
    }
    // Ensure last entry is exactly 1.0 (floating-point safety).
    if let Some(last) = cdf.last_mut() {
        *last = 1.0;
    }

    // Draw from CDF.
    let selected = cdf_select(&cdf, themes, mt);

    // Skip current scheme if selected.
    if selected == current_scheme {
        // Try once more (different random draw).
        let retry = cdf_select(&cdf, themes, mt);
        if retry != current_scheme {
            return Some(retry);
        }
        // Two consecutive hits on current scheme — unlikely with 14
        // themes per group. Accept a no-op (return None).
        return None;
    }

    Some(selected)
}

/// Uniform fallback: select a random theme, skipping current.
fn uniform_select(
    themes: &[ColorScheme],
    current_scheme: ColorScheme,
    mt: &mut rand::rngs::StdRng,
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
fn cdf_select(cdf: &[f32], themes: &[ColorScheme], mt: &mut rand::rngs::StdRng) -> ColorScheme {
    let u_dist = Uniform::new(0.0f32, 1.0f32).expect("uniform f32 always valid");
    let u = u_dist.sample(mt);
    // Binary search for the first CDF entry >= u.
    let idx = cdf.partition_point(|&c| c < u);
    themes[idx.min(themes.len() - 1)]
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
