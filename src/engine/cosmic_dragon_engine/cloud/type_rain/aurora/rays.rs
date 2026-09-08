// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The aurora ray lattice — the sky half of the polar veil
//! (NIGHT-special-3).
//!
//! This module owns the invented veil physics: the drifting flux-tube
//! lattice (wind + inverse-gap repulsion), the quantized two-band
//! substorm breath of the emission depth, and the bounded fringe
//! glow charge. The full derivation essay lives in
//! `type_rain/aurora/mod.rs`; this file is the executable form of
//! laws 1, 2 and 4.
//!
//! Storage: one flat `Vec<AuroraRay>` kept sorted by `x` (the pair
//! pass relies on the order; re-sorting N <= 24 beads per tick is a
//! no-op cost). No per-cell fields — the veil's spatial structure
//! lives in the beads, and the draw pass expands each bead into its
//! column of glyphs.

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::constants::{
    AURORA_DEPTH_HIGH_FRAC, AURORA_DEPTH_HIGH_SPAN, AURORA_DEPTH_LOW_FRAC, AURORA_DEPTH_LOW_SPAN,
    AURORA_DEPTH_MIN, AURORA_DEPTH_RELAX, AURORA_DWELL_MEAN, AURORA_FLARE_SECS, AURORA_GAP_FLOOR,
    AURORA_GLOW_DECAY, AURORA_GLOW_GAIN, AURORA_GLOW_LEVEL_CORE, AURORA_GLOW_LEVEL_HOT,
    AURORA_GLOW_MAX, AURORA_RAY_MAX, AURORA_RAY_SPACING_COLS, AURORA_REPULSION, AURORA_VX_MAX,
    AURORA_WALL_DAMP, AURORA_WIND_COUPLE, AURORA_WIND_HOLD_MEAN, AURORA_WIND_RELAX,
    AURORA_WIND_SPAN,
};

use super::super::monolith::BrightnessLevel;

/// RNG bundle (mirrors `AeolianRandom` — the advance pass is the
/// second stochastic pass in the family: wind re-rolls, anchor flips
/// and dwell re-rolls ride along).
pub(crate) struct AuroraRandom<'a> {
    pub(crate) rng: &'a mut StdRng,
    pub(crate) rand_chance: &'a Uniform<f32>,
}

/// The two substorm depth bands (law 2). LOW curtains hang at
/// 0.24-0.34 of the viewport height, HIGH at 0.46-0.60 — disjoint by
/// construction, so the veil reads as layered altitudes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DepthBand {
    Low,
    High,
}

impl DepthBand {
    /// The other band (anchor flips always cross bands — the breath
    /// never re-rolls inside the band it just left).
    pub(crate) fn flip(self) -> Self {
        match self {
            Self::Low => Self::High,
            Self::High => Self::Low,
        }
    }
}

/// One ray bead: the invisible flux tube that carries a curtain.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AuroraRay {
    /// Fractional column of the bead (curtains hang from here).
    pub(crate) x: f32,
    /// Drift velocity in columns per sim-second (repulsion + wind).
    pub(crate) vx: f32,
    /// Emission depth in lines from the top edge (fractional — the
    /// fringe line is its grid quantization).
    pub(crate) depth: f32,
    /// Current anchor the depth glides toward.
    anchor: f32,
    /// Which band the anchor sits in.
    band: DepthBand,
    /// Sim-seconds accumulated toward the next anchor flip.
    dwell: f32,
    /// Rolled flip threshold (DWELL_MEAN with variance).
    dwell_target: f32,
    /// Fringe glow charge (law 4): absorbed drop kinetic energy.
    pub(crate) glow: f32,
    /// Sim-seconds since the last absorption (the Core flare window).
    pub(crate) flare_age: f32,
}

impl AuroraRay {
    fn vacant() -> Self {
        Self {
            x: 0.0,
            vx: 0.0,
            depth: AURORA_DEPTH_MIN,
            anchor: AURORA_DEPTH_MIN,
            band: DepthBand::Low,
            dwell: 0.0,
            dwell_target: AURORA_DWELL_MEAN,
            glow: 0.0,
            flare_age: f32::MAX,
        }
    }

    /// Seed a bead at its resting column: a Low-band anchor by
    /// default (the quiet sky), a small drift, a rolled first flip.
    fn seed(&mut self, x: f32, lines: u16, roll_a: f32, roll_b: f32, roll_c: f32) {
        self.x = x;
        self.vx = (roll_a * 2.0 - 1.0) * 0.6;
        self.band = if roll_b < 0.62 {
            DepthBand::Low
        } else {
            DepthBand::High
        };
        self.anchor = sample_anchor(lines, self.band, roll_c);
        self.depth = self.anchor;
        self.dwell = 0.0;
        self.dwell_target = roll_dwell_target(roll_a);
        self.glow = 0.0;
        self.flare_age = f32::MAX;
    }
}

/// The aurora sky: the ray lattice + the global wind.
#[derive(Debug)]
pub(crate) struct AuroraSky {
    rays: Vec<AuroraRay>,
    /// Global wind velocity in columns per sim-second (law 1).
    wind: f32,
    wind_target: f32,
    wind_hold: f32,
    cols: u16,
    lines: u16,
}

impl AuroraSky {
    pub(crate) fn new() -> Self {
        Self {
            rays: Vec::new(),
            wind: 0.0,
            wind_target: 0.0,
            wind_hold: 0.0,
            cols: 0,
            lines: 0,
        }
    }

    /// Rebuild the lattice for a new viewport: bead count from the
    /// spacing divisor (one bead per RAY_SPACING_COLS columns,
    /// clamped to at least one), spread evenly with jitter, all
    /// fringes quiet (a fresh sky waits to be painted).
    pub(crate) fn reset(&mut self, cols: u16, lines: u16) {
        self.cols = cols;
        self.lines = lines;
        let count = ray_count_for_cols(cols);
        self.rays.clear();
        self.rays.resize_with(count, AuroraRay::vacant);
        // Even spread + jitter: (i + roll) / count * cols.
        let cols_f = cols.max(1) as f32;
        for (i, ray) in self.rays.iter_mut().enumerate() {
            let frac = (i as f32 + 0.5) / count as f32;
            let x = (frac * cols_f).clamp(0.0, cols_f - 1.0);
            ray.seed(
                x,
                lines,
                // Deterministic jitter from the index keeps the
                // fresh viewport spread organic without an RNG in
                // reset (the family reset contract is RNG-free).
                ((i * 7 + 3) % 13) as f32 / 12.0,
                ((i * 5 + 1) % 11) as f32 / 10.0,
                ((i * 3 + 7) % 17) as f32 / 16.0,
            );
        }
        self.wind = 0.0;
        self.wind_target = 0.0;
        self.wind_hold = 0.0;
    }

    pub(crate) fn rays(&self) -> &[AuroraRay] {
        &self.rays
    }

    /// Law 3's query: the index of the ray nearest a fractional
    /// column (the drop's funnel target). Linear scan — the lattice
    /// is sorted but N <= 24, and drops are rarer still.
    pub(crate) fn nearest_ray(&self, x: f32) -> Option<usize> {
        if self.rays.is_empty() {
            return None;
        }
        let mut best = 0usize;
        let mut best_d = f32::INFINITY;
        for (i, r) in self.rays.iter().enumerate() {
            let d = (r.x - x).abs();
            if d < best_d {
                best_d = d;
                best = i;
            }
        }
        Some(best)
    }

    /// Law 4: deposit an absorbed drop's kinetic charge into a ray's
    /// fringe glow (hard clamp — bounded by construction) and arm
    /// the flare window.
    pub(crate) fn absorb(&mut self, idx: usize, charge: f32) {
        if let Some(r) = self.rays.get_mut(idx) {
            r.glow = (r.glow + charge * AURORA_GLOW_GAIN).min(AURORA_GLOW_MAX);
            r.flare_age = 0.0;
        }
    }

    /// One tick of the sky (laws 1, 2, 4a). Order: wind re-roll +
    /// relaxation, pair repulsion, then per-bead integration (wind
    /// coupling, velocity clamp, position + wall, depth breath,
    /// glow decay).
    pub(crate) fn advance(&mut self, dt: f32, random: &mut AuroraRandom<'_>) {
        if dt <= 0.0 || self.rays.is_empty() {
            return;
        }
        let cols_f = self.cols.max(1) as f32;
        let depth_max = (self.lines.saturating_sub(2)).max(AURORA_DEPTH_MIN as u16 + 1) as f32;

        // Law 1a — the wind: re-roll the target on hold expiry, ease
        // toward it exponentially.
        self.wind_hold -= dt;
        if self.wind_hold <= 0.0 {
            let roll = random.rand_chance.sample(random.rng);
            self.wind_target = (roll * 2.0 - 1.0) * AURORA_WIND_SPAN;
            self.wind_hold = AURORA_WIND_HOLD_MEAN * (0.6 + roll * 0.8);
        }
        self.wind += (self.wind_target - self.wind) * AURORA_WIND_RELAX * dt;

        // Law 1b — flux-tube repulsion on adjacent pairs (the rays
        // are kept sorted by x; each pair pushes both beads apart
        // with gain / floored-gap).
        self.rays
            .sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
        for i in 1..self.rays.len() {
            let gap = (self.rays[i].x - self.rays[i - 1].x)
                .abs()
                .max(AURORA_GAP_FLOOR);
            let push = AURORA_REPULSION / gap * dt;
            // The RIGHT bead pushes right (away), the LEFT bead
            // pushes left (away) — the pair separates.
            self.rays[i].vx += push;
            self.rays[i - 1].vx -= push;
        }

        // Per-bead integration.
        let wind = self.wind;
        let lines = self.lines;
        for r in &mut self.rays {
            // Law 1c — wind coupling + velocity clamp.
            r.vx += (wind - r.vx) * AURORA_WIND_COUPLE * dt;
            r.vx = r.vx.clamp(-AURORA_VX_MAX, AURORA_VX_MAX);

            // Position + damped wall bounce.
            r.x += r.vx * dt;
            if r.x < 0.0 {
                r.x = 0.0;
                r.vx = -r.vx * AURORA_WALL_DAMP;
            } else if r.x > cols_f - 1.0 {
                r.x = cols_f - 1.0;
                r.vx = -r.vx * AURORA_WALL_DAMP;
            }

            // Law 2 — the substorm breath: dwell to the flip, then
            // re-anchor in the OTHER band; depth glides toward the
            // anchor (never snapping), hard-clamped.
            r.dwell += dt;
            if r.dwell >= r.dwell_target {
                r.dwell = 0.0;
                r.dwell_target = roll_dwell_target(random.rand_chance.sample(random.rng));
                r.band = r.band.flip();
                r.anchor = sample_anchor(lines, r.band, random.rand_chance.sample(random.rng));
            }
            r.depth += (r.anchor - r.depth) * AURORA_DEPTH_RELAX * dt;
            r.depth = r.depth.clamp(AURORA_DEPTH_MIN, depth_max);

            // Law 4a — glow decay (the fringe cools; the flare
            // window ages).
            r.glow *= (-AURORA_GLOW_DECAY * dt).exp();
            r.flare_age += dt;
        }
    }

    // -- Test-only diagnostics (mirrors the family *_for_test API) --

    #[cfg(test)]
    pub(crate) fn glow_for_test(&self, idx: usize) -> f32 {
        self.rays.get(idx).map_or(0.0, |r| r.glow)
    }

    /// Drop every bead (the repulsion test plants its own controlled
    /// two-bead lattice).
    #[cfg(test)]
    pub(crate) fn clear_rays_for_test(&mut self) {
        self.rays.clear();
    }

    /// Plant one additional bead at a column (the repulsion test's
    /// controlled two-bead lattice).
    #[cfg(test)]
    pub(crate) fn plant_ray_for_test(&mut self, x: f32, lines: u16, roll: f32) {
        let mut r = AuroraRay::vacant();
        r.seed(x, lines, roll, roll, roll);
        self.rays.push(r);
    }
}

/// Bead count for a viewport width: one ray per RAY_SPACING_COLS
/// columns, at least one (a narrow terminal still carries one
/// curtain), at most the hard cap.
pub(crate) fn ray_count_for_cols(cols: u16) -> usize {
    let count = (cols.max(1) / AURORA_RAY_SPACING_COLS) as usize;
    count.clamp(1, AURORA_RAY_MAX)
}

/// Sample an anchor depth inside a band (law 2): fraction of the
/// viewport height, band-scoped, quantized to the grid at the draw
/// boundary only.
fn sample_anchor(lines: u16, band: DepthBand, roll: f32) -> f32 {
    let lines_f = lines.max(1) as f32;
    let frac = match band {
        DepthBand::Low => AURORA_DEPTH_LOW_FRAC + roll.clamp(0.0, 1.0) * AURORA_DEPTH_LOW_SPAN,
        DepthBand::High => AURORA_DEPTH_HIGH_FRAC + roll.clamp(0.0, 1.0) * AURORA_DEPTH_HIGH_SPAN,
    };
    lines_f * frac
}

/// Roll a dwell target: DWELL_MEAN with 0.5x-1.5x variance (no
/// rhythmic mass flipping — the family variance contract).
fn roll_dwell_target(roll: f32) -> f32 {
    AURORA_DWELL_MEAN * (0.5 + roll.clamp(0.0, 1.0))
}

/// Law 4's draw read: the fringe's brightness ladder from its glow
/// and flare age. Mid is the emission-layer baseline (a quiet fringe
/// is still the brightest cell of its curtain), Hot is the charged
/// state, Core is the fresh-landing flare window.
pub(crate) fn fringe_level(glow: f32, flare_age: f32) -> BrightnessLevel {
    if glow > AURORA_GLOW_LEVEL_CORE && flare_age < AURORA_FLARE_SECS {
        BrightnessLevel::Core
    } else if glow > AURORA_GLOW_LEVEL_HOT {
        BrightnessLevel::Hot
    } else {
        BrightnessLevel::Mid
    }
}
