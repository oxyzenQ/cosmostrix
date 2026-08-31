// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Brightness factor helpers for the droplet visual effects pipeline.
//!
//! Pure functions: viewport edge fade, vignette, rain shadow, CRT vignette.
//! Each returns a multiplier in [0.0, 1.0] applied per-cell in droplet.rs::draw().

use crate::constants::{
    CRT_VIGNETTE_EDGE_FACTOR, CRT_VIGNETTE_HEIGHT, EDGE_FADE_BOTTOM_LIP, EDGE_FADE_BOTTOM_MIN,
    EDGE_FADE_BOTTOM_ROWS, EDGE_FADE_ROWS, EDGE_FADE_TOP_MIN, RAIN_SHADOW_FLOOR, RAIN_SHADOW_PCT,
    VIGNETTE_INNER_RADIUS, VIGNETTE_INTENSITY,
};

/// to the viewport edges. Interior rows return 1.0 (no dimming).
///
/// This fade is applied AFTER all other visual effects (including head
/// self-bloom and head brightness modulation) so it takes priority at
/// viewport edges, creating:
/// - Smooth rain emergence at the top (rain appears to enter from beyond)
/// - Smooth rain exit at the bottom (tails fade out before the terminal border)
/// - Prevention of bright head tips lingering on the bottom border
///
/// The asymmetric min values (EDGE_FADE_TOP_MIN=0.48 vs
/// EDGE_FADE_BOTTOM_MIN=0.68) reflect the Deep Focus profile: the
/// top fade is more aggressive (rain enters from deep shadow) while
/// the bottom fade is gentler (rain dissolves gracefully). The
/// asymmetry is inherited from Cinema Noir and refined for the
/// battle round 2 champion — see `docs/VISUAL_IDENTITY.md` for the
/// preset lineage and `docs/research/VISUAL_MODE_AUDIT.md` for the
/// compounding math that drove the Deep Focus values.
#[inline]
pub(crate) fn viewport_edge_fade(line: u16, lines: u16) -> f32 {
    if lines == 0 || EDGE_FADE_ROWS == 0 {
        return 1.0;
    }
    // Top edge: linear fade over EDGE_FADE_ROWS rows.
    let top_fade = if line < EDGE_FADE_ROWS {
        EDGE_FADE_TOP_MIN + (1.0 - EDGE_FADE_TOP_MIN) * (line as f32 / EDGE_FADE_ROWS as f32)
    } else {
        1.0
    };
    // v17: Bottom edge — 2-zone cinematic dissolve.
    //
    // Zone 1 (gentle pre-fade): rows [lines-EDGE_FADE_BOTTOM_ROWS .. lines-EDGE_FADE_ROWS]
    //   smoothstep from 1.0 down to EDGE_FADE_BOTTOM_LIP. Subtle — rain still
    //   clearly visible but starting to darken.
    //
    // Zone 2 (sharp lip): rows [lines-EDGE_FADE_ROWS .. lines-1]
    //   linear from EDGE_FADE_BOTTOM_LIP down to EDGE_FADE_BOTTOM_MIN. Heavy
    //   fade — rain dissolves into shadow before the border.
    //
    // The 2-zone design produces a film-like vignette where rain gradually
    // fades across the bottom 30% of the screen (on a 40-line terminal),
    // eliminating the "concrete wall" artifact where dying heads pile up.
    let bottom_dist = lines.saturating_sub(line).saturating_sub(1);
    let bottom_fade = if bottom_dist < EDGE_FADE_ROWS {
        // Zone 2: sharp lip fade. bottom_dist in [0, EDGE_FADE_ROWS).
        // Linear from EDGE_FADE_BOTTOM_MIN (at bottom_dist=0) to
        // EDGE_FADE_BOTTOM_LIP (at bottom_dist=EDGE_FADE_ROWS).
        let t = bottom_dist as f32 / EDGE_FADE_ROWS as f32;
        EDGE_FADE_BOTTOM_MIN + (EDGE_FADE_BOTTOM_LIP - EDGE_FADE_BOTTOM_MIN) * t
    } else if bottom_dist < EDGE_FADE_BOTTOM_ROWS {
        // Zone 1: gentle pre-fade. bottom_dist in [EDGE_FADE_ROWS, EDGE_FADE_BOTTOM_ROWS).
        // Smoothstep from EDGE_FADE_BOTTOM_LIP (at bottom_dist=EDGE_FADE_ROWS)
        // up to 1.0 (at bottom_dist=EDGE_FADE_BOTTOM_ROWS).
        let span = (EDGE_FADE_BOTTOM_ROWS - EDGE_FADE_ROWS) as f32;
        let t = (bottom_dist - EDGE_FADE_ROWS) as f32 / span;
        // Smoothstep: 3t² - 2t³ (slow start, fast middle, slow end).
        let smooth = t * t * (3.0 - 2.0 * t);
        EDGE_FADE_BOTTOM_LIP + (1.0 - EDGE_FADE_BOTTOM_LIP) * smooth
    } else {
        1.0
    };
    top_fade.min(bottom_fade)
}

/// Cinematic radial vignette: darkens cells based on Euclidean distance
/// from the screen center. Cells inside VIGNETTE_INNER_RADIUS are
/// unmodified; cells from there to the corner are dimmed smoothly via
/// smoothstep up to VIGNETTE_INTENSITY.
///
/// This is a pure photographic vignette — it does NOT replace the
/// top/bottom edge fade (which is a directional cinematic dissolve).
/// The vignette adds a soft "lens" darkening on top of all other
/// effects, drawing the eye toward the focused center of the frame.
///
/// O(1) per cell: 2 subtractions, 2 multiplications, 1 sqrt, 1
/// smoothstep, 1 multiply. Called once per cell in the draw loop.
#[inline]
pub(crate) fn vignette_factor(col: u16, line: u16, cols: u16, lines: u16) -> f32 {
    if cols == 0 || lines == 0 || VIGNETTE_INTENSITY <= 0.0 {
        return 1.0;
    }
    // Normalize to [-1, 1] centered on screen midpoint.
    let nx = (col as f32 - cols as f32 * 0.5) / (cols as f32 * 0.5);
    let ny = (line as f32 - lines as f32 * 0.5) / (lines as f32 * 0.5);
    // Euclidean distance from center, normalized so corner = sqrt(2)/2 ≈ 0.707
    // for a non-square screen. We rescale to make corner ≈ 1.0 by dividing by
    // the diagonal half-length, but a simpler approach: just use raw Euclidean
    // and treat the diagonal half-length as 1.0. To keep the inner-radius
    // semantics intuitive (0.7 = 70% of the way to the corner), we normalize
    // by max(nx², ny²) → corner = 1.0 in Chebyshev distance, which matches
    // the perceived "corners are darkest" intuition better than Euclidean
    // for non-square terminal cells (which are ~2:1 tall).
    let dist_sq = nx * nx + ny * ny;
    let dist = dist_sq.sqrt();
    // Corner of a square screen is at dist = sqrt(2) ≈ 1.414; of a typical
    // wide terminal (cols=2*lines), it's sqrt(1 + 0.25) ≈ 1.118. We
    // normalize so the `corner of a square` maps to 1.0, which keeps the
    // inner-radius cutoff intuitive on standard terminals.
    let normalized = dist * std::f32::consts::FRAC_1_SQRT_2;
    if normalized <= VIGNETTE_INNER_RADIUS {
        return 1.0;
    }
    // Smoothstep from VIGNETTE_INNER_RADIUS (factor=1.0) to 1.0 (factor=1-VIGNETTE_INTENSITY).
    let t = ((normalized - VIGNETTE_INNER_RADIUS) / (1.0 - VIGNETTE_INNER_RADIUS)).clamp(0.0, 1.0);
    let smooth = t * t * (3.0 - 2.0 * t);
    1.0 - VIGNETTE_INTENSITY * smooth
}

/// Rain shadow: quadratic fade-out across the bottom RAIN_SHADOW_PCT of
/// the screen. Cells above the threshold are unmodified; cells from the
/// threshold to the bottom row fade smoothly down to `RAIN_SHADOW_FLOOR`
/// (50% dim, never full dark).
///
/// Distinct from EDGE_FADE_BOTTOM: the edge fade is a sharp 10-row lip
/// that prevents bright head pile-up at the very last row. The rain
/// shadow is a wider, softer 15%-of-screen quadratic that gives the
/// frame perceptual "depth" — rain appears to dissipate into shadow at
/// the ground rather than hitting a wall.
///
/// Applied BEFORE phosphor decay so the captured phosphor energy is
/// already dimmed — the afterglow trail fades in sync with the shadow.
///
/// ## masterclass retune (2026-08-09)
/// The previously curve faded to 0.0 (full black) at the bottom row.
/// Compounded multiplicatively with `viewport_edge_fade` (0.45),
/// `vignette_factor` (~0.71 at corners), and `crt_vignette_factor`
/// (0.82), the bottom row reached 0.08 brightness (92% dim) — rain
/// was invisible. The floor at `RAIN_SHADOW_FLOOR` (0.50) caps the
/// shadow's contribution so the compounded bottom-row brightness
/// stays at ~0.13 (rain visible) while preserving the depth gradient.
///
/// The curve shape is preserved: quadratic `1 - t^2` is linearly
/// remapped from [0.0, 1.0] to [RAIN_SHADOW_FLOOR, 1.0] so the
/// slow-start-accelerating-fade character is unchanged. Only the
/// absolute floor moves from 0.0 to 0.50.
///
/// See `docs/research/VISUAL_MODE_AUDIT.md` for the full 4-effect
/// compounding model.
#[inline]
pub(crate) fn rain_shadow_factor(line: u16, lines: u16) -> f32 {
    if lines == 0 || RAIN_SHADOW_PCT <= 0.0 {
        return 1.0;
    }
    let threshold = ((1.0 - RAIN_SHADOW_PCT) * lines as f32) as u16;
    if line < threshold {
        return 1.0;
    }
    let span = (lines.saturating_sub(threshold)).max(1) as f32;
    let t = ((line - threshold) as f32 / span).clamp(0.0, 1.0);
    // Quadratic fade: 1.0 -> RAIN_SHADOW_FLOOR as t goes 0 -> 1, with
    // slow start and accelerating fade. Reads as natural depth shadow.
    // linearly remapped to floor at RAIN_SHADOW_FLOOR (0.50)
    // instead of 0.0 — prevents the bottom row from going fully dark
    // when shadow multiplies with edge fade + radial vignette + CRT
    // vignette. Curve shape (quadratic 1 - t^2) is preserved.
    RAIN_SHADOW_FLOOR + (1.0 - RAIN_SHADOW_FLOOR) * (1.0 - t * t)
}

/// CRT vignette factor for a given row. Returns the per-row brightness
/// multiplier applied by the post-process `apply_crt_vignette` pass in
/// `cloud/rain.rs`.
///
/// Returns 1.0 (no dim) for rows outside the top/bottom
/// `CRT_VIGNETTE_HEIGHT` bands. For rows inside the bands, returns a
/// smoothstep from 1.0 (interior edge of band) down to
/// `CRT_VIGNETTE_EDGE_FACTOR` (extreme edge row). Both top and bottom
/// bands use the same symmetric smoothstep curve.
///
/// ## masterclass extraction (2026-08-09)
/// Extracted from the inline row-factor precomputation in
/// `cloud/rain.rs::apply_crt_vignette` so the per-row factor is
/// queryable from the SSOT `compounded_brightness` function without
/// duplicating the smoothstep math. The inline precomputation in
/// `apply_crt_vignette` now calls this function — DRY, single source
/// of truth for the CRT vignette row-factor curve.
///
/// ## Skipped cases
/// - `lines < 2 * CRT_VIGNETTE_HEIGHT`: the screen is too short for the
///   vignette to make sense (would dim the entire screen). Returns 1.0
///   for all rows. Matches the early-return guard in `apply_crt_vignette`.
/// - `CRT_VIGNETTE_HEIGHT == 0`: vignette disabled. Returns 1.0.
///
/// ## Cost
/// O(1) per call — 1 comparison, 1 subtraction, 1 division, 1
/// smoothstep, 1 multiply. Used by `compounded_brightness` (audit/test
/// path) and by `apply_crt_vignette` (per-row precompute, 2*H calls
/// per frame — negligible).
#[inline]
pub(crate) fn crt_vignette_factor(line: u16, lines: u16) -> f32 {
    if CRT_VIGNETTE_HEIGHT == 0 || lines < 2 * CRT_VIGNETTE_HEIGHT {
        return 1.0;
    }
    let top_end = CRT_VIGNETTE_HEIGHT;
    let bottom_start = lines.saturating_sub(CRT_VIGNETTE_HEIGHT);

    // Distance from the nearest edge: 0 at the extreme edge row,
    // CRT_VIGNETTE_HEIGHT-1 at the interior edge of the band.
    // Rows between top_end and bottom_start fall outside both bands
    // and return 1.0 (no dim).
    let v = if line < top_end {
        line
    } else if line >= bottom_start {
        lines - 1 - line
    } else {
        return 1.0;
    };

    // Smoothstep from 1.0 (at v=H-1, interior edge) down to
    // CRT_VIGNETTE_EDGE_FACTOR (at v=0, extreme edge). Same curve as
    // the inline precomputation in apply_crt_vignette.
    let t = v as f32 / CRT_VIGNETTE_HEIGHT as f32;
    let smooth = t * t * (3.0 - 2.0 * t);
    CRT_VIGNETTE_EDGE_FACTOR + (1.0 - CRT_VIGNETTE_EDGE_FACTOR) * smooth
}
