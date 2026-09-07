// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The aeolian string field — the instrument half of the weave
//! (NIGHT-special-2).
//!
//! This module owns the invented string physics: the two one-way
//! hop-clock channels, the bistable hop-rate law, the wall
//! reflection, the self-similar decay, and the pluck injection API.
//! The full derivation essay lives in `type_rain/aeolian/mod.rs`;
//! this file is the executable form of laws 1-4.
//!
//! Storage layout: one flat f32 mass and one flat f32 hop clock per
//! cell per channel, indexed `string * cols + col` (cache-friendly
//! column sweeps, no per-string Vec-of-Vec indirection). The hop
//! passes are single-pass in-place: the right channel sweeps
//! right-to-left and the left channel left-to-right, so every hop
//! lands in an already-processed cell — no double moves, no second
//! field copy.

use crate::constants::{
    AEOLIAN_DRAW_FLOOR, AEOLIAN_HOP_FAST, AEOLIAN_HOP_SLOW, AEOLIAN_KNOT_LEVEL, AEOLIAN_LEVEL_HOT,
    AEOLIAN_LEVEL_MID, AEOLIAN_STRING_DECAY, AEOLIAN_STRING_LINES_PER, AEOLIAN_URGENCY_SWITCH,
    AEOLIAN_WALL_REFLECT,
};

use super::super::monolith::BrightnessLevel;

/// String row fractions per tier count (1..=4 strings). Deliberately
/// not exact thirds: the near-even but organic spacing keeps the
/// resonance bands from reading as a ruled table. The lowest band
/// stops at 0.87 so the bottom edge keeps its dark margin (the sky
/// below the last string stays visibly empty — the ground the
/// through-rain falls into).
const STRING_ROW_FRACTIONS: [&[f32]; 4] = [
    &[0.62],
    &[0.42, 0.74],
    &[0.32, 0.56, 0.80],
    &[0.26, 0.47, 0.68, 0.87],
];

/// The aeolian string field: `rows.len()` horizontal resonance
/// strings, each `cols` cells wide, carried by two one-way channels.
///
/// Law 1 (the hop-clock lattice): every channel cell carries its
/// excitation mass AND a private phase clock. Each tick the clock
/// advances by the cell's hop rate — `rate(a) = SLOW + (FAST -
/// SLOW) * a^k/(a^k + SWITCH^k)`, a steep bistable switch in the
/// amplitude — and when the clock completes, the cell's ENTIRE
/// mass hops exactly one cell in the channel's travel direction
/// (merging with whatever the receiver holds). Bright cells (above
/// the switch) run their clocks near the signal rate and a strike
/// translates RIGIDLY — zero diffusion, the shape preserved exactly;
/// dim cells crawl at the residue rate. A struck packet separates
/// into a sharp bright head racing ahead of its slow dim wake: the
/// hook shape. Each mass unit hops at most one cell per tick (the
/// sweep order forbids double moves) — no tunneling at any dt.
///
/// (First two formulations were derived and rejected during
/// tuning, both preserved as design history in aeolian/mod.rs: a
/// fractional convex-blend sweep obeys a max principle — it can
/// never concentrate or even sustain a peak, so pulses always
/// decayed into dim ramps within ~1.5 s and crossing knots never
/// formed. The hop-clock moves mass in quanta instead of blending
/// it, which transports shape exactly while keeping the same L1
/// contraction proof: hops are conservative transfers, decay
/// shrinks, walls return at most what they receive.)
///
/// Law 2 (wall reflection): a hop off the screen edge re-enters
/// the OPPOSITE channel at `WALL_REFLECT` of its mass — the
/// instrument is closed at both ends.
///
/// Law 3 (self-similar decay): every cell multiplies by
/// `exp(-DECAY * dt)` — a uniform shrink applied BEFORE the hop
/// pass, preserving packet shape while eroding amplitude.
///
/// Law 4 (pluck injection): `pluck` adds amplitude symmetrically
/// into BOTH channels across a three-cell profile — light races
/// away from the impact in both directions (the classic pluck
/// read).
#[derive(Debug)]
pub(crate) struct AeolianStrings {
    /// Row (line index) of each string, ascending.
    rows: Vec<u16>,
    /// Right-traveling channel mass, indexed `string * cols + col`.
    chan_r: Vec<f32>,
    /// Left-traveling channel mass, same layout.
    chan_l: Vec<f32>,
    /// Right-channel hop clocks (phase in [0, 2): one pending hop
    /// of debt is kept, no more — see the sweep's phase clamp).
    phase_r: Vec<f32>,
    /// Left-channel hop clocks, same layout.
    phase_l: Vec<f32>,
    cols: u16,
    lines: u16,
}

impl AeolianStrings {
    pub(crate) fn new() -> Self {
        Self {
            rows: Vec::new(),
            chan_r: Vec::new(),
            chan_l: Vec::new(),
            phase_r: Vec::new(),
            phase_l: Vec::new(),
            cols: 0,
            lines: 0,
        }
    }

    /// Rebuild the field for a new viewport: string count from the
    /// height tiering (one string per ~17 lines, 1..=4), rows at the
    /// tier's organic fractions, all channels silent (the instrument
    /// is invisible until the rain plays it).
    pub(crate) fn reset(&mut self, cols: u16, lines: u16) {
        self.cols = cols;
        self.lines = lines;
        let count = string_count_for_lines(lines);
        self.rows.clear();
        for frac in STRING_ROW_FRACTIONS[count - 1] {
            let row_f = (lines as f32 * frac).round();
            // Keep every string at least one line off the very top
            // and two off the very bottom (the dark ground margin).
            let row = (row_f as i32).clamp(1, (lines as i32 - 2).max(1)) as u16;
            self.rows.push(row);
        }
        let cells = count.saturating_mul(cols.max(1) as usize);
        self.chan_r.clear();
        self.chan_r.resize(cells, 0.0);
        self.chan_l.clear();
        self.chan_l.resize(cells, 0.0);
        self.phase_r.clear();
        self.phase_r.resize(cells, 0.0);
        self.phase_l.clear();
        self.phase_l.resize(cells, 0.0);
    }

    pub(crate) fn string_rows(&self) -> &[u16] {
        &self.rows
    }

    /// Law 4: inject a strike's amplitude symmetrically into BOTH
    /// channels, spread across a three-cell profile (center full,
    /// neighbors half) — a hammer, not a pinprick. The profile gives
    /// the racing signal a short visible body; the hop clock's rigid
    /// translation preserves it exactly for the packet's whole
    /// lifetime. Edge columns clamp the profile inward.
    pub(crate) fn pluck(&mut self, string: usize, col: usize, amplitude: f32) {
        if self.rows.is_empty() || self.cols == 0 || string >= self.rows.len() {
            return;
        }
        let cols = self.cols as usize;
        let base = string * cols;
        let center = col.min(cols - 1);
        for (offset, weight) in [(0i32, 1.0f32), (1, 0.5), (-1, 0.5)] {
            let x = center as i32 + offset;
            if x < 0 || x >= cols as i32 {
                continue;
            }
            let idx = base + x as usize;
            self.chan_r[idx] += amplitude * weight;
            self.chan_l[idx] += amplitude * weight;
        }
    }

    /// The hop-clock sweep (laws 1-3). One tick of sim-time.
    ///
    /// Order of operations per string: uniform decay first (law 3,
    /// shape-preserving), then the right-channel hop pass processed
    /// RIGHT-TO-LEFT, then the left-channel hop pass processed
    /// LEFT-TO-RIGHT. The opposing sweep directions mean a hop
    /// always lands in a cell that has ALREADY been processed this
    /// tick — no mass unit can move more than one cell per tick at
    /// any dt (no tunneling). A hop carries its phase overshoot
    /// (the timing surplus) into the receiver, so transport speed
    /// is exactly the hop rate wherever rate x dt < 1 (no rounding
    /// loss, no fps dependence); a merge into an occupied cell
    /// piggybacks the receiver's own clock.
    ///
    /// A hop off the field edge is DEFERRED: the reflected mass
    /// lands in the opposite channel after both sweeps complete
    /// (fresh clock — the bounce spends the surplus), so even
    /// reflected mass respects the one-cell guarantee. Nothing in
    /// this sweep can grow the field: hops are conservative
    /// transfers, merges add what the donor lost, decay shrinks —
    /// only `pluck` injects.
    pub(crate) fn advance(&mut self, dt: f32) {
        if dt <= 0.0 || self.rows.is_empty() || self.cols == 0 {
            return;
        }
        let cols = self.cols as usize;
        // Law 3 precompute: one exp per tick, shared by every cell
        // (uniform shrink — the shape-preserving property).
        let decay = (-AEOLIAN_STRING_DECAY * dt).exp();
        for s in 0..self.rows.len() {
            let base = s * cols;

            // Law 3: the uniform decay pass.
            for x in 0..cols {
                self.chan_r[base + x] *= decay;
                self.chan_l[base + x] *= decay;
            }

            // Deferred wall reflections for this tick: mass that
            // hopped off a field edge, returned by the opposite
            // channel at WALL_REFLECT (applied after both sweeps,
            // with a fresh clock — the one-cell-per-tick guarantee
            // covers reflections too).
            let mut reflect_into_l = 0.0;
            let mut reflect_into_r = 0.0;

            // Law 1 + 2: the right-channel hop pass (right to left —
            // a hop lands in an already-processed cell).
            for x in (0..cols).rev() {
                let idx = base + x;
                let mass = self.chan_r[idx];
                if mass <= 0.0 {
                    continue;
                }
                self.phase_r[idx] += hop_rate(mass) * dt;
                if self.phase_r[idx] >= 1.0 {
                    // The overshoot is the mass's timing surplus —
                    // carried to the receiver so the clock never
                    // loses fractional progress at a hop (without
                    // the carry, each hop rounds the wait up to a
                    // whole tick and the effective speed degrades
                    // below the rate law, fps-dependently).
                    let surplus = self.phase_r[idx] - 1.0;
                    self.phase_r[idx] = 0.0;
                    self.chan_r[idx] = 0.0;
                    if x + 1 < cols {
                        let receiver_empty = self.chan_r[idx + 1] <= 0.0;
                        self.chan_r[idx + 1] += mass;
                        if receiver_empty {
                            // Fresh arrival: the mass's clock follows
                            // it (surplus carry — exact rate keeping).
                            self.phase_r[idx + 1] = surplus;
                        }
                        // A merge into an occupied cell piggybacks
                        // the receiver's own clock (one blob, one
                        // schedule — deterministic).
                    } else {
                        // Law 2: right wall — defer the leftward
                        // reflection (the bounce starts a fresh
                        // clock; the timing surplus is spent).
                        reflect_into_l += AEOLIAN_WALL_REFLECT * mass;
                    }
                }
            }

            // Law 1 + 2: the left-channel hop pass (left to right —
            // symmetric ordering guarantee).
            for x in 0..cols {
                let idx = base + x;
                let mass = self.chan_l[idx];
                if mass <= 0.0 {
                    continue;
                }
                self.phase_l[idx] += hop_rate(mass) * dt;
                if self.phase_l[idx] >= 1.0 {
                    let surplus = self.phase_l[idx] - 1.0;
                    self.phase_l[idx] = 0.0;
                    self.chan_l[idx] = 0.0;
                    if x > 0 {
                        let receiver_empty = self.chan_l[idx - 1] <= 0.0;
                        self.chan_l[idx - 1] += mass;
                        if receiver_empty {
                            self.phase_l[idx - 1] = surplus;
                        }
                    } else {
                        // Law 2: left wall — defer the rightward
                        // reflection.
                        reflect_into_r += AEOLIAN_WALL_REFLECT * mass;
                    }
                }
            }

            // Apply the deferred reflections (fresh clocks).
            if reflect_into_l > 0.0 {
                let wall = base + cols - 1;
                self.chan_l[wall] += reflect_into_l;
                self.phase_l[wall] = 0.0;
            }
            if reflect_into_r > 0.0 {
                self.chan_r[base] += reflect_into_r;
                self.phase_r[base] = 0.0;
            }
        }
    }

    /// Combined field amplitude at (string, col) — the drop-steering
    /// and string-draw input. Returns 0.0 outside the field.
    pub(crate) fn combined(&self, string: usize, col: usize) -> f32 {
        let Some(base) = self.cell_base(string, col) else {
            return 0.0;
        };
        self.chan_r[base].abs() + self.chan_l[base].abs()
    }

    /// Interference read: the counter-propagating overlap at a cell
    /// — the smaller channel amplitude. Where both channels are
    /// strong, packets are crossing and the cell reads as a knot.
    pub(crate) fn knot(&self, string: usize, col: usize) -> f32 {
        let Some(base) = self.cell_base(string, col) else {
            return 0.0;
        };
        self.chan_r[base].abs().min(self.chan_l[base].abs())
    }

    /// Central-difference slope of the combined field along a
    /// string (the drop-steering gradient). Edge columns use the
    /// one-sided difference.
    pub(crate) fn slope(&self, string: usize, col: usize) -> f32 {
        if self.rows.is_empty() || self.cols == 0 {
            return 0.0;
        }
        let left = if col > 0 {
            self.combined(string, col - 1)
        } else {
            self.combined(string, col)
        };
        let right = if col + 1 < self.cols as usize {
            self.combined(string, col + 1)
        } else {
            self.combined(string, col)
        };
        right - left
    }

    /// Channel read for tests: (right, left) amplitudes at a cell.
    #[cfg(test)]
    pub(crate) fn channels_for_test(&self, string: usize, col: usize) -> (f32, f32) {
        let base = self.cell_base(string, col).unwrap_or(0);
        (self.chan_r[base], self.chan_l[base])
    }

    /// The discrete L1 norm of the whole field (sum of |amplitude|
    /// over every cell and channel) — the boundedness contract's
    /// observable. Conduction and decay can only shrink it; only
    /// plucks grow it.
    #[cfg(test)]
    pub(crate) fn l1_norm_for_test(&self) -> f32 {
        self.chan_r.iter().map(|v| v.abs()).sum::<f32>()
            + self.chan_l.iter().map(|v| v.abs()).sum::<f32>()
    }

    /// Bounds-checked flat index for (string, col).
    fn cell_base(&self, string: usize, col: usize) -> Option<usize> {
        if string >= self.rows.len() || col >= self.cols as usize || self.cols == 0 {
            return None;
        }
        Some(string * self.cols as usize + col)
    }
}

/// The hop rate law (law 1): a channel cell's clock speed in
/// cells per sim-second — a two-voice switch in the cell's mass.
///
/// A cell whose mass sits at or above the signal threshold runs
/// its clock at the sprint rate; below it, at the residue crawl.
/// The QUANTIZED switch (not a smooth curve) is deliberate: every
/// bright cell of a pulse runs at the SAME rate, so the pulse hops
/// in lockstep and translates rigidly — a smooth rate curve would
/// shear the pulse apart within a few ticks (each cell marching at
/// its own speed). Decay carries a cell from the signal voice to
/// the residue voice monotonically (mass only shrinks between
/// plucks), so the switch never oscillates.
#[inline]
fn hop_rate(mass: f32) -> f32 {
    if mass >= AEOLIAN_URGENCY_SWITCH {
        AEOLIAN_HOP_FAST
    } else {
        AEOLIAN_HOP_SLOW
    }
}

/// String count for a viewport height: one resonance band per
/// ~17 lines, 1..=4. A 24-line terminal keeps one instrument (the
/// chime); a 60-line terminal carries four (the full frame).
fn string_count_for_lines(lines: u16) -> usize {
    if lines > 3 * AEOLIAN_STRING_LINES_PER {
        4
    } else if lines >= 2 * AEOLIAN_STRING_LINES_PER {
        3
    } else if lines > AEOLIAN_STRING_LINES_PER {
        2
    } else {
        1
    }
}

/// Brightness ladder for a string cell's combined amplitude: the
/// draw floor filters silent cells entirely (the invisible
/// instrument), then Mid / Hot climb with the packet body / crest.
/// The Core rung is reserved for interference knots (see
/// `AeolianRain::draw`), never amplitude alone.
pub(crate) fn level_for_amplitude(combined: f32) -> BrightnessLevel {
    if combined > AEOLIAN_LEVEL_HOT {
        BrightnessLevel::Hot
    } else if combined > AEOLIAN_LEVEL_MID {
        BrightnessLevel::Mid
    } else if combined > AEOLIAN_DRAW_FLOOR {
        BrightnessLevel::Ghost
    } else {
        BrightnessLevel::Dim
    }
}

/// True when the counter-propagating overlap at a cell reads as an
/// interference knot (both channels strong — packets crossing).
#[inline]
pub(crate) fn is_knot(knot: f32) -> bool {
    knot > AEOLIAN_KNOT_LEVEL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_count_tiers() {
        assert_eq!(string_count_for_lines(10), 1);
        assert_eq!(string_count_for_lines(18), 2);
        assert_eq!(string_count_for_lines(34), 3);
        assert_eq!(string_count_for_lines(52), 4);
        assert_eq!(string_count_for_lines(200), 4);
    }

    #[test]
    fn rows_stay_inside_viewport() {
        for lines in [6u16, 18, 34, 52, 90] {
            let mut s = AeolianStrings::new();
            s.reset(80, lines);
            assert_eq!(s.string_rows().len(), string_count_for_lines(lines));
            for &row in s.string_rows() {
                assert!(
                    row >= 1 && row + 2 <= lines,
                    "row {row} out of margin for {lines}"
                );
            }
        }
    }

    #[test]
    fn hop_rate_is_two_voiced() {
        // The regime split observable through the sweep: a bright
        // pulse outruns a dim one many times over (the lockstep
        // sprint vs the crawl). The raw rate switch itself is
        // pinned by the compile-time contracts in style_rain.rs.
        let mut bright = AeolianStrings::new();
        bright.reset(200, 40);
        bright.pluck(0, 100, 3.0);
        let mut dim = AeolianStrings::new();
        dim.reset(200, 40);
        dim.pluck(0, 100, 0.2);
        for _ in 0..30 {
            bright.advance(0.016);
            dim.advance(0.016);
        }
        let peak = |f: &AeolianStrings| {
            let mut best = 0usize;
            let mut best_a = -1.0_f32;
            for x in 0..200 {
                let (r, _) = f.channels_for_test(0, x);
                if r.abs() > best_a {
                    best_a = r.abs();
                    best = x;
                }
            }
            best
        };
        let b = peak(&bright);
        let d = peak(&dim);
        assert!(b > d + 5, "two voices collapsed: bright {b} dim {d}");
    }
}
