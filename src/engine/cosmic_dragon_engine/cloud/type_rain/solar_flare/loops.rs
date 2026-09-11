// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The corona arcade — the star half of the solar flare style
//! (NIGHT-special-4).
//!
//! This module owns the invented arcade physics: the magnetic
//! carpet (loop lifecycle, footpoint repulsion, width and height
//! breath, the global wind), the bounded flux charge, the flare
//! eruption cycle, and the granulation surface. The full derivation
//! essay lives in `type_rain/solar_flare/mod.rs`; this file is the
//! executable form of laws 1, 3 and 4 plus the arc geometry helpers
//! law 2 consumes.
//!
//! Storage: one flat `Vec<SolarLoop>` kept sorted by center column
//! (the pair pass relies on the order; re-sorting N <= 8 loops per
//! tick is a no-op cost). No per-cell fields — the arcade's spatial
//! structure lives in the loops, and the draw pass expands each
//! loop into its parabola of glyphs. The granulation surface is one
//! heat value per column (the second surface line reads one rung
//! dimmer — no separate state).

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::constants::{
    SOLAR_ARC_MAX_FRAC, SOLAR_ARC_MIN_FRAC, SOLAR_DETACH_LIFT_RATE, SOLAR_DETACH_SECS,
    SOLAR_DRIFT_MAX, SOLAR_EMERGE_SECS, SOLAR_ERUPT_SECS, SOLAR_FLARE_CLOCK_MEAN, SOLAR_FLASH_SECS,
    SOLAR_FLUX_DECAY, SOLAR_FLUX_ERUPT_THRESHOLD, SOLAR_FLUX_GAIN, SOLAR_FLUX_LEVEL_CORE,
    SOLAR_FLUX_LEVEL_HOT, SOLAR_FLUX_LEVEL_MID, SOLAR_FLUX_MAX, SOLAR_GAP_FLOOR,
    SOLAR_GRANULE_LEVEL_HOT, SOLAR_GRANULE_LEVEL_MID, SOLAR_GRANULE_MAX, SOLAR_GRANULE_MIN,
    SOLAR_GRANULE_STEP, SOLAR_H_CAP_FRAC, SOLAR_H_MIN, SOLAR_H_RELAX, SOLAR_LOOP_MAX,
    SOLAR_LOOP_SPACING_COLS, SOLAR_REPULSION, SOLAR_STRETCH, SOLAR_SURFACE_LINES, SOLAR_WALL_DAMP,
    SOLAR_WIND_COUPLE, SOLAR_WIND_HOLD_MEAN, SOLAR_WIND_RELAX, SOLAR_WIND_SPAN, SOLAR_W_DWELL_MEAN,
    SOLAR_W_MAX_FRAC, SOLAR_W_MIN, SOLAR_W_RELAX,
};

use super::super::monolith::BrightnessLevel;

/// RNG bundle (the advance pass is the family's stochastic pass:
/// wind re-rolls, breath re-anchors, flare clock re-rolls and the
/// granulation random walk all ride along).
pub(crate) struct SolarRandom<'a> {
    pub(crate) rng: &'a mut StdRng,
    pub(crate) rand_chance: &'a Uniform<f32>,
}

/// The loop lifecycle (law 1's carpet turnover + law 4's flare
/// cycle). A loop is born Emerging out of the photosphere, lives
/// Stable (collecting deposited flux from the coronal rain), may
/// destabilize into Erupting (the flare), Detaches (lifts off and
/// dissolves — the CME read), and re-seeds Emerging elsewhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LoopPhase {
    /// Flux emergence: the arc grows out of the surface (~1 s).
    Emerging,
    /// The quiet corona: carpet drift + breath + flux cooling.
    Stable,
    /// The flare: the loop stretches and sheds ejecta.
    Erupting,
    /// The lift-off: the arc rises off the surface and dissolves.
    Detaching,
}

/// One coronal loop: a parabolic flux tube rooted at two footpoints.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SolarLoop {
    pub(crate) phase: LoopPhase,
    /// Center column (fractional). Footpoints at cx - w/2, cx + w/2.
    pub(crate) cx: f32,
    /// Footpoint span in columns (the loop width).
    pub(crate) w: f32,
    /// Width breath anchor (re-rolled on the dwell).
    w_anchor: f32,
    /// Sim-seconds accumulated toward the next breath re-anchor.
    w_dwell: f32,
    /// Rolled dwell threshold (W_DWELL_MEAN with variance).
    w_dwell_target: f32,
    /// Apex height in lines (current, gliding toward the target).
    pub(crate) h: f32,
    /// The loop's natural apex (the breath re-rolls this).
    h_base: f32,
    /// Lateral drift velocity in columns per sim-second.
    pub(crate) vx: f32,
    /// Deposited heat (law 3): landed drop kinetic charge.
    pub(crate) flux: f32,
    /// Sim-seconds since the last landing (the footpoint flash).
    pub(crate) flare_age: f32,
    /// Phase progression clock (emerge / erupt / detach durations).
    pub(crate) phase_clock: f32,
    /// Lift offset in lines above the surface (Detaching only).
    pub(crate) lift: f32,
}

impl SolarLoop {
    fn vacant() -> Self {
        Self {
            phase: LoopPhase::Emerging,
            cx: 0.0,
            w: SOLAR_W_MIN,
            w_anchor: SOLAR_W_MIN,
            w_dwell: 0.0,
            w_dwell_target: SOLAR_W_DWELL_MEAN,
            h: 0.0,
            h_base: 0.0,
            vx: 0.0,
            flux: 0.0,
            flare_age: f32::MAX,
            phase_clock: 0.0,
            lift: 0.0,
        }
    }

    /// Seed a loop at a center column: span and height rolled inside
    /// their bands, a small drift, a rolled first breath.
    fn seed(&mut self, cx: f32, w: f32, h_base: f32, roll_a: f32, roll_b: f32) {
        self.phase = LoopPhase::Emerging;
        self.cx = cx;
        self.w = w;
        self.w_anchor = w;
        self.w_dwell = 0.0;
        self.w_dwell_target = SOLAR_W_DWELL_MEAN * (0.5 + roll_a.clamp(0.0, 1.0));
        self.h = 0.0;
        self.h_base = h_base;
        self.vx = (roll_b * 2.0 - 1.0) * 0.5;
        self.flux = 0.0;
        self.flare_age = f32::MAX;
        self.phase_clock = 0.0;
        self.lift = 0.0;
    }

    /// Arc x at parameter s (0 = left footpoint, 1 = right footpoint,
    /// 0.5 = apex). Linear interpolation of the footpoint span.
    pub(crate) fn arc_x(&self, s: f32) -> f32 {
        self.cx - self.w * 0.5 + s.clamp(0.0, 1.0) * self.w
    }

    /// Arc height above the feet baseline at parameter s (lines):
    /// the parabola h x 4s(1-s), apex h at s = 0.5. The lift offset
    /// (Detaching) adds on top so the whole arc rises intact.
    pub(crate) fn arc_height(&self, s: f32) -> f32 {
        let sx = s.clamp(0.0, 1.0);
        self.h * 4.0 * sx * (1.0 - sx) + self.lift
    }

    /// The arc-length metric at parameter s (law 2): |dr/ds| for the
    /// parabola (x, h x 4s(1-s)) — sqrt(w^2 + 16 h^2 (1-2s)^2),
    /// floored at the width so the apex metric never collapses.
    pub(crate) fn arc_metric(&self, s: f32) -> f32 {
        let sx = s.clamp(0.0, 1.0);
        let slope = 4.0 * self.h * (1.0 - 2.0 * sx);
        (self.w * self.w + slope * slope)
            .sqrt()
            .max(self.w.max(1.0))
    }

    /// The tangent direction at parameter s (used to fling riders as
    /// ejecta at eruption): the analytic derivative of the parabola,
    /// normalized, pointing toward increasing s.
    pub(crate) fn arc_tangent(&self, s: f32) -> (f32, f32) {
        let sx = s.clamp(0.0, 1.0);
        let dx = self.w;
        let dy = -4.0 * self.h * (1.0 - 2.0 * sx);
        let len = (dx * dx + dy * dy).sqrt().max(1.0);
        (dx / len, dy / len)
    }

    /// Facing-footpoint gap against the next loop to the right (the
    /// repulsion distance: this loop's right foot to that loop's
    /// left foot).
    fn facing_gap(&self, other: &Self) -> f32 {
        ((self.cx + self.w * 0.5) - (other.cx - other.w * 0.5))
            .abs()
            .max(SOLAR_GAP_FLOOR)
    }

    /// Law 3: deposit a landed drop's kinetic charge into the flux
    /// (hard clamp — bounded by construction) and arm the flash.
    pub(crate) fn absorb(&mut self, charge: f32) {
        self.flux = (self.flux + charge * SOLAR_FLUX_GAIN).min(SOLAR_FLUX_MAX);
        self.flare_age = 0.0;
    }
}

/// The star: the corona arcade plus the granulation surface.
#[derive(Debug)]
pub(crate) struct CoronaArcade {
    loops: Vec<SolarLoop>,
    /// Global wind velocity in columns per sim-second (law 1).
    wind: f32,
    wind_target: f32,
    wind_hold: f32,
    /// Global flare cadence clock: sim-seconds until the next
    /// eruption is allowed (law 4's singular-event gate).
    flare_clock: f32,
    /// Granulation heat per column (law 5's surface: one value per
    /// column; the second surface line reads one rung dimmer).
    granules: Vec<f32>,
    cols: u16,
    lines: u16,
}

impl CoronaArcade {
    pub(crate) fn new() -> Self {
        Self {
            loops: Vec::new(),
            wind: 0.0,
            wind_target: 0.0,
            wind_hold: 0.0,
            flare_clock: SOLAR_FLARE_CLOCK_MEAN,
            granules: Vec::new(),
            cols: 0,
            lines: 0,
        }
    }

    /// Rebuild the arcade for a new viewport: loop count from the
    /// spacing divisor, centers spread evenly with deterministic
    /// index-derived rolls (the family reset contract is RNG-free),
    /// spans and heights rolled inside their bands, every loop
    /// Emerging (a fresh star grows its corona), granulation seeded
    /// mid-band.
    pub(crate) fn reset(&mut self, cols: u16, lines: u16) {
        self.cols = cols;
        self.lines = lines;
        let count = loop_count_for_cols(cols);
        self.loops.clear();
        self.loops.resize_with(count, SolarLoop::vacant);
        let (w_lo, w_hi) = width_band(cols);
        let (h_lo, h_hi) = height_band(lines);
        let cols_f = cols.max(1) as f32;
        for (i, lp) in self.loops.iter_mut().enumerate() {
            let frac = (i as f32 + 0.5) / count as f32;
            let cx = (frac * cols_f).clamp(1.0, (cols_f - 1.0).max(1.0));
            // Deterministic rolls from the index keep the fresh
            // viewport spread organic without an RNG in reset.
            let roll_a = ((i * 7 + 3) % 13) as f32 / 12.0;
            let roll_b = ((i * 5 + 1) % 11) as f32 / 10.0;
            let w = w_lo + roll_a * (w_hi - w_lo);
            let h_base = h_lo + roll_b * (h_hi - h_lo);
            lp.seed(cx, w, h_base, roll_a, roll_b);
        }
        self.wind = 0.0;
        self.wind_target = 0.0;
        self.wind_hold = 0.0;
        self.flare_clock = SOLAR_FLARE_CLOCK_MEAN;
        self.granules.clear();
        self.granules.resize_with(cols.max(1) as usize, || 0.5);
    }

    pub(crate) fn loops(&self) -> &[SolarLoop] {
        &self.loops
    }

    /// The surface's top line index (the first granulation line —
    /// the loop feet sit here).
    pub(crate) fn surface_top(&self) -> u16 {
        self.lines.saturating_sub(SOLAR_SURFACE_LINES).max(1)
    }

    /// Law 3's rain arm: deposit a landed drop's kinetic charge
    /// into the loop at `idx` (routes to the loop's absorb — the
    /// orchestration pass holds the drop pool, so the deposition
    /// arrives here after the physics loop).
    pub(crate) fn absorb(&mut self, idx: usize, charge: f32) {
        if let Some(lp) = self.loops.get_mut(idx) {
            lp.absorb(charge);
        }
    }

    /// Law 3's surface arm: a landed ejecta splashes a heat pulse
    /// into the granule at its column (clamped — bounded).
    pub(crate) fn splash(&mut self, col: usize, heat: f32) {
        if let Some(g) = self.granules.get_mut(col) {
            *g = (*g + heat).min(SOLAR_GRANULE_MAX);
        }
    }

    /// The granulation heat read for a column (law 5's surface draw).
    pub(crate) fn granule_heat(&self, col: usize) -> f32 {
        self.granules.get(col).copied().unwrap_or(0.0)
    }

    /// Advance the star one sim-second tick (laws 1, 3a, 4). Returns
    /// the index of the loop that just entered Erupting, if any —
    /// the orchestration converts its riders to ejecta and spawns
    /// the apex burst (the drop pool lives there).
    pub(crate) fn advance(&mut self, dt: f32, random: &mut SolarRandom<'_>) -> Option<usize> {
        if dt <= 0.0 || self.loops.is_empty() {
            return None;
        }
        let cols_f = self.cols.max(1) as f32;
        let (w_lo, w_hi) = width_band(self.cols);
        let (h_lo, h_hi) = height_band(self.lines);
        let h_cap = h_hi;

        // Law 1a — the wind: re-roll the target on hold expiry, ease
        // toward it exponentially.
        self.wind_hold -= dt;
        if self.wind_hold <= 0.0 {
            let roll = random.rand_chance.sample(random.rng);
            self.wind_target = (roll * 2.0 - 1.0) * SOLAR_WIND_SPAN;
            self.wind_hold = SOLAR_WIND_HOLD_MEAN * (0.6 + roll * 0.8);
        }
        self.wind += (self.wind_target - self.wind) * SOLAR_WIND_RELAX * dt;

        // Law 1b — footpoint repulsion on adjacent loops (kept
        // sorted by center; facing feet push both loops apart with
        // gain / floored-gap).
        self.loops
            .sort_by(|a, b| a.cx.partial_cmp(&b.cx).unwrap_or(std::cmp::Ordering::Equal));
        for i in 1..self.loops.len() {
            let gap = self.loops[i].facing_gap(&self.loops[i - 1]);
            let push = SOLAR_REPULSION / gap * dt;
            // The RIGHT loop pushes right (away), the LEFT loop
            // pushes left (away) — the pair separates.
            self.loops[i].vx += push;
            self.loops[i - 1].vx -= push;
        }

        // Law 5's surface — the granulation random walk (per column,
        // clamped inside the heat band).
        for g in &mut self.granules {
            let roll = random.rand_chance.sample(random.rng);
            *g = (*g + (roll - 0.5) * SOLAR_GRANULE_STEP * dt)
                .clamp(SOLAR_GRANULE_MIN, SOLAR_GRANULE_MAX);
        }

        // Per-loop integration: phase clock, carpet forces, breath,
        // flux decay. Law 3a's decay factor is dt-only, so it is
        // evaluated once per frame and shared by every loop (the
        // stage-1 StepFactors pattern — value identical, semantics
        // explicit).
        let wind = self.wind;
        let flux_decay = (-SOLAR_FLUX_DECAY * dt).exp();
        for lp in &mut self.loops {
            // Lifecycle clocks (laws 1 + 4).
            lp.phase_clock += dt;
            match lp.phase {
                LoopPhase::Emerging => {
                    if lp.phase_clock >= SOLAR_EMERGE_SECS {
                        lp.phase = LoopPhase::Stable;
                        lp.phase_clock = 0.0;
                    }
                }
                LoopPhase::Stable => {}
                LoopPhase::Erupting => {
                    if lp.phase_clock >= SOLAR_ERUPT_SECS {
                        lp.phase = LoopPhase::Detaching;
                        lp.phase_clock = 0.0;
                        lp.lift = 0.0;
                        lp.flux = 0.0;
                    }
                }
                LoopPhase::Detaching => {
                    lp.lift += SOLAR_DETACH_LIFT_RATE * dt;
                    if lp.phase_clock >= SOLAR_DETACH_SECS {
                        // Rebirth: fresh geometry, flux reset (the
                        // carpet's turnover). The margin keeps both
                        // feet inside the viewport.
                        let roll_a = random.rand_chance.sample(random.rng);
                        let roll_b = random.rand_chance.sample(random.rng);
                        let w = w_lo + roll_a * (w_hi - w_lo);
                        let margin = (w * 0.5).max(1.0);
                        let cx = (margin + roll_b * (cols_f - 1.0 - 2.0 * margin).max(0.0))
                            .clamp(margin, (cols_f - 1.0 - margin).max(margin));
                        let h_base = h_lo + random.rand_chance.sample(random.rng) * (h_hi - h_lo);
                        lp.seed(cx, w, h_base, roll_a, roll_b);
                    }
                }
            }

            // Law 1d — the width breath (Stable only): dwell to the
            // re-anchor, then glide toward it. Runs BEFORE the
            // position integration so the wall clamp always sees
            // the span it must keep inside the viewport (a widening
            // breath can never strand a center outside its new
            // footpoint margin for a tick).
            if lp.phase == LoopPhase::Stable {
                lp.w_dwell += dt;
                if lp.w_dwell >= lp.w_dwell_target {
                    lp.w_dwell = 0.0;
                    lp.w_dwell_target =
                        SOLAR_W_DWELL_MEAN * (0.5 + random.rand_chance.sample(random.rng));
                    let roll = random.rand_chance.sample(random.rng);
                    lp.w_anchor = w_lo + roll * (w_hi - w_lo);
                    lp.h_base = h_lo + random.rand_chance.sample(random.rng) * (h_hi - h_lo);
                }
                lp.w += (lp.w_anchor - lp.w) * SOLAR_W_RELAX * dt;
                lp.w = lp.w.clamp(w_lo, w_hi);
            }

            // Law 1c — wind coupling + drift clamp (Stable/Emerging
            // only: an erupting or detaching loop is leaving the
            // carpet and holds its momentum).
            if matches!(lp.phase, LoopPhase::Stable | LoopPhase::Emerging) {
                lp.vx += (wind - lp.vx) * SOLAR_WIND_COUPLE * dt;
            }
            lp.vx = lp.vx.clamp(-SOLAR_DRIFT_MAX, SOLAR_DRIFT_MAX);

            // Position + damped wall reflection (the margin keeps
            // both feet inside the viewport: foot_left = cx - w/2
            // >= 0 and foot_right = cx + w/2 <= cols - 1 — the
            // upper bound is cols - 1 - margin, not cols - margin,
            // so the right foot never sits one cell past the edge).
            lp.cx += lp.vx * dt;
            let margin = (lp.w * 0.5).max(1.0).min((cols_f - 1.0).max(1.0) * 0.5);
            let cx_max = (cols_f - 1.0 - margin).max(margin);
            if lp.cx < margin {
                lp.cx = margin;
                lp.vx = -lp.vx * SOLAR_WALL_DAMP;
            } else if lp.cx > cx_max {
                lp.cx = cx_max;
                lp.vx = -lp.vx * SOLAR_WALL_DAMP;
            }

            // The height glide: phase-driven target (emergence grows
            // it, eruption stretches it, detach freezes it).
            let h_target = match lp.phase {
                LoopPhase::Emerging | LoopPhase::Stable => lp.h_base,
                LoopPhase::Erupting => (lp.h_base * SOLAR_STRETCH).min(h_cap),
                LoopPhase::Detaching => lp.h,
            };
            lp.h += (h_target - lp.h) * SOLAR_H_RELAX * dt;
            lp.h = lp.h.clamp(0.0, h_cap);

            // Law 3a — flux decay (the corona cools; the flash ages).
            lp.flux *= flux_decay;
            lp.flare_age += dt;
        }

        // Law 4 — the global flare gate: when the cadence clock
        // opens and no loop is erupting, the flux-richest Stable
        // loop over the threshold destabilizes (one flare at a
        // time — the singular-event contract).
        self.flare_clock -= dt;
        if self.flare_clock <= 0.0 {
            self.flare_clock =
                SOLAR_FLARE_CLOCK_MEAN * (0.5 + random.rand_chance.sample(random.rng));
            let erupting = self.loops.iter().any(|lp| lp.phase == LoopPhase::Erupting);
            if !erupting {
                let mut best: Option<usize> = None;
                let mut best_flux = SOLAR_FLUX_ERUPT_THRESHOLD;
                for (i, lp) in self.loops.iter().enumerate() {
                    if lp.phase == LoopPhase::Stable && lp.flux > best_flux {
                        best_flux = lp.flux;
                        best = Some(i);
                    }
                }
                if let Some(idx) = best {
                    self.loops[idx].phase = LoopPhase::Erupting;
                    self.loops[idx].phase_clock = 0.0;
                    return Some(idx);
                }
            }
        }
        None
    }

    // -- Test-only diagnostics (mirrors the family *_for_test API) --

    #[cfg(test)]
    pub(crate) fn flux_for_test(&self, idx: usize) -> f32 {
        self.loops.get(idx).map_or(0.0, |lp| lp.flux)
    }

    #[cfg(test)]
    pub(crate) fn loop_phase_for_test(&self, idx: usize) -> Option<LoopPhase> {
        self.loops.get(idx).map(|lp| lp.phase)
    }

    /// Drop every loop (the carpet tests plant their own controlled
    /// one- and two-loop arcades).
    #[cfg(test)]
    pub(crate) fn clear_loops_for_test(&mut self) {
        self.loops.clear();
    }

    /// Plant one additional loop at a center column (the carpet
    /// tests' controlled arcade).
    #[cfg(test)]
    pub(crate) fn plant_loop_for_test(&mut self, cx: f32, w: f32, h: f32) {
        let mut lp = SolarLoop::vacant();
        lp.seed(cx, w, h, 0.5, 0.5);
        lp.phase = LoopPhase::Stable;
        lp.h = h;
        self.loops.push(lp);
    }

    /// Force a loop's flux past the eruption threshold (the flare
    /// cycle test's deterministic arm).
    #[cfg(test)]
    pub(crate) fn plant_flux_for_test(&mut self, idx: usize, flux: f32) {
        if let Some(lp) = self.loops.get_mut(idx) {
            lp.flux = flux;
            lp.phase = LoopPhase::Stable;
            lp.phase_clock = 0.0;
        }
    }

    /// Arm the flare clock to fire on the next tick (the eruption
    /// tests' deterministic gate).
    #[cfg(test)]
    pub(crate) fn arm_flare_clock_for_test(&mut self) {
        self.flare_clock = 0.0;
    }
}

/// Loop count for a viewport width: one loop per
/// SOLAR_LOOP_SPACING_COLS columns, at least one (a narrow terminal
/// still carries one arc), at most the hard cap.
pub(crate) fn loop_count_for_cols(cols: u16) -> usize {
    let count = (cols.max(1) / SOLAR_LOOP_SPACING_COLS) as usize;
    count.clamp(1, SOLAR_LOOP_MAX)
}

/// The footpoint span band for a viewport width (law 1): W_MIN to a
/// fraction of the width (clamped so a narrow terminal still fits
/// the minimum span).
pub(crate) fn width_band(cols: u16) -> (f32, f32) {
    let cols_f = cols.max(1) as f32;
    let hi = (SOLAR_W_MAX_FRAC * cols_f).max(SOLAR_W_MIN * 1.5);
    (SOLAR_W_MIN, hi)
}

/// The apex height band for a viewport height (law 1): a fraction of
/// the usable height above the surface, floored at H_MIN lines.
pub(crate) fn height_band(lines: u16) -> (f32, f32) {
    let usable = (lines.max(1) as i32 - SOLAR_SURFACE_LINES as i32 - 1).max(3) as f32;
    let lo = (SOLAR_ARC_MIN_FRAC * usable).max(SOLAR_H_MIN);
    let hi = (SOLAR_ARC_MAX_FRAC * usable).max(lo + 1.0);
    // The hard cap: the stretched arc never leaves the sky.
    let cap = (SOLAR_H_CAP_FRAC * usable).max(hi);
    (lo, cap)
}

/// Law 3's draw read: the loop's brightness ladder from its flux and
/// flare age (the corona read: Ghost is the quiet corona, Mid a
/// rained-on loop, Hot a heavily-fed arcade member, Core the fresh
/// flare window).
pub(crate) fn loop_level(flux: f32, flare_age: f32) -> BrightnessLevel {
    if flux > SOLAR_FLUX_LEVEL_CORE && flare_age < SOLAR_FLASH_SECS {
        BrightnessLevel::Core
    } else if flux > SOLAR_FLUX_LEVEL_HOT {
        BrightnessLevel::Hot
    } else if flux > SOLAR_FLUX_LEVEL_MID {
        BrightnessLevel::Mid
    } else {
        BrightnessLevel::Ghost
    }
}

/// Step a brightness level up (toward Core) by one rung — the
/// footpoint landing punch (fresh flash only: its window is
/// FLASH_SECS, the same gate that arms the ladder's Core rung).
pub(crate) fn step_up_level(level: BrightnessLevel) -> BrightnessLevel {
    match level {
        BrightnessLevel::Ghost | BrightnessLevel::Dim => BrightnessLevel::Mid,
        BrightnessLevel::Mid => BrightnessLevel::Hot,
        BrightnessLevel::Hot | BrightnessLevel::Core => BrightnessLevel::Core,
    }
}

/// The apex condensation glow's step-up, saturated at Hot (the
/// NIGHT-research-24 soft-light ruling; the black hole's
/// NIGHT-research-11 cap and the dragon's entry-reveal precedent,
/// ported to the corona). A heavily-fed loop holds flux above the
/// Hot bound for seconds after the flash window closes (the 0.38/s
/// decay from FLUX_MAX crosses HOT at about 3.4 s), and the retired
/// full step-up painted those apex cells Core-white for the whole
/// plateau — a standing read. The glow now lifts only the lower
/// rungs and holds the ceiling at Hot; Core belongs to the flash
/// windows alone (the eruption window, the ladder's fresh-flare
/// rung, the landing punch), so a Core base passes through
/// untouched.
pub(crate) fn apex_step_level(level: BrightnessLevel) -> BrightnessLevel {
    match level {
        BrightnessLevel::Ghost | BrightnessLevel::Dim => BrightnessLevel::Mid,
        BrightnessLevel::Mid => BrightnessLevel::Hot,
        BrightnessLevel::Hot | BrightnessLevel::Core => level,
    }
}

/// The arc cell's composed level — the whole pass-B ladder decision
/// from raw loop state: the phase base (Erupting burns Core for the
/// flash window then Hot; every other phase reads the flux ladder),
/// then the footpoint landing punch (one rung up while the flash is
/// fresh — the one path that may lift to Core, its window is
/// FLASH_SECS) and the apex condensation glow (saturated at Hot).
/// The NIGHT-research-24 contract: with the flash windows closed, no
/// arc cell composes above Hot.
pub(crate) fn arc_cell_level(
    phase: LoopPhase,
    phase_clock: f32,
    flux: f32,
    flare_age: f32,
    near_foot: bool,
    near_apex: bool,
) -> BrightnessLevel {
    let base = match phase {
        LoopPhase::Erupting => {
            // The flare: the whole arc reads Core for the flash
            // window, then Hot (law 4).
            if phase_clock < SOLAR_FLASH_SECS {
                BrightnessLevel::Core
            } else {
                BrightnessLevel::Hot
            }
        }
        _ => loop_level(flux, flare_age),
    };
    let foot_flash = near_foot && flare_age < SOLAR_FLASH_SECS && phase != LoopPhase::Erupting;
    let apex_glow = near_apex && flux > SOLAR_FLUX_LEVEL_HOT;
    if foot_flash {
        step_up_level(base)
    } else if apex_glow {
        apex_step_level(base)
    } else {
        base
    }
}

/// The granulation ladder (law 5's surface read): quiet grit reads
/// Ghost, warm granules Mid, the hottest convection cells Hot.
pub(crate) fn granule_level(heat: f32) -> BrightnessLevel {
    if heat > SOLAR_GRANULE_LEVEL_HOT {
        BrightnessLevel::Hot
    } else if heat > SOLAR_GRANULE_LEVEL_MID {
        BrightnessLevel::Mid
    } else {
        BrightnessLevel::Ghost
    }
}
