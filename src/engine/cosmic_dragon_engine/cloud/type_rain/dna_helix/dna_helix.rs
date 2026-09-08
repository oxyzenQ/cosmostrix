// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The DNA helix rain state machine (NIGHT-research-7, the
//! eleventh style) — the orchestration pass set: pool, spawn,
//! advance, draw.
//!
//! The style composes the two halves of the molecule (see mod.rs
//! for the full derivation): `DnaGenome` (the rung table, the
//! rotation phase and the replication fork — laws 1 through 4)
//! and `NucleotideDrop` (the soup, law 5). This struct owns the
//! pool lifecycle and the per-frame passes, following the
//! structured family contract
//! (lorenz/vortex/dragon/physarum/black_hole/aeolian/solar_flare):
//!
//! - pool: one drop per column (the family lane model),
//! - spawn: the deficit-bounded fractional accumulator, with the
//!   spawn position rolled inside the molecule's capture band
//!   (law 5's concentration arm — the soup falls where the
//!   genome can catch it, not uniformly across the sky),
//! - advance: one global clock (dt clamped by `max_sim_delta`,
//!   eased by `resume_blend`, scaled to sim-time by
//!   `chars_per_sec`), genome first then drops (the absorption
//!   test reads the fork-moved geometry, so the rung spans are
//!   fresh when the drops cross them; the absorptions are
//!   collected and applied after the loop — the solar landings
//!   pattern),
//! - draw: the strands first, then the rungs (span cells with the
//!   depth blend), then the rain (heads + comet trails), all
//!   through the monolith drawn-cell diff cleanup
//!   (generation-tagged).
//!
//! The advance pass is a stochastic pass in the family sense: the
//! replication clock re-arms, the fork-pass mutations re-roll and
//! the soup's brownian drift all ride the RNG bundle.

use std::time::{Duration, Instant};

use rand::distr::Distribution;

use crate::constants::{
    DNA_ACTIVE_BASE, DNA_ACTIVE_DENSITY_MULT, DNA_ACTIVE_MAX, DNA_CAPTURE_BAND_MULT,
    DNA_CAPTURE_MARGIN, DNA_DEPOSIT_RATE, DNA_DRIFT_MAX, DNA_DRIFT_RATE, DNA_FALL_BAND,
    DNA_FALL_MULT, DNA_MAX_AGE_SECS, DNA_MUTATION_CHANCE, DNA_SIM_TIME_PER_CPS,
    DNA_SPAWN_RATE_FLOOR, DNA_SPAWN_RATE_MULT, SPAWN_REMAINDER_CAP,
};

use super::drops::NucleotideDrop;
use super::helix::{DnaGenome, DnaRandom};

/// One drawn cell (col, line) — the diff-cleanup currency (same
/// shape as `SolarCell`/`AeolianCell`/`LorenzCell`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DnaCell {
    pub(crate) col: u16,
    pub(crate) line: u16,
}

/// Spawn inputs (mirrors `SolarFlareSpawnParams` — the bundle
/// keeps clippy's `too_many_arguments` threshold respected).
pub(crate) struct DnaSpawnParams {
    pub(crate) cols: u16,
    pub(crate) lines: u16,
    pub(crate) density: f32,
    pub(crate) active_palette_slot: u8,
    pub(crate) spawn_scale: f32,
}

/// Per-frame step inputs (mirrors `SolarFlareStep`). Carries
/// viewport geometry — the drop physics needs bounds for the
/// floor expiry and the drift wall clamps.
pub(crate) struct DnaStep {
    pub(crate) now: Instant,
    /// chars_per_sec already multiplied by the terminal
    /// speed_mult. Scales the whole molecule on one clock
    /// (dt_sim) — the family speed contract: the turn, the fork
    /// and the fall all survive the speed keys together.
    pub(crate) chars_per_sec: f32,
    pub(crate) cols: u16,
    pub(crate) lines: u16,
    pub(crate) max_sim_delta: Duration,
    pub(crate) resume_blend: f32,
}

/// The DNA helix rain: the genome plus the nucleotide pool, one
/// state machine.
#[derive(Debug)]
pub(crate) struct DnaHelixRain {
    pub(crate) drops: Vec<NucleotideDrop>,
    /// The molecule (the ladder state — see helix.rs).
    pub(crate) genome: DnaGenome,
    active_count: usize,
    /// Rotating scan cursor for amortized O(1) free-slot search
    /// (mirrors `VortexRain::spawn_scan_idx`).
    spawn_scan_idx: usize,
    /// Global motion clock (dt = now - last_step, clamped by
    /// max_sim_delta and eased by resume_blend — the family
    /// contract; a fully-paused run simply stops advancing).
    last_step: Option<Instant>,
    /// Palette slot of the molecule (one body, one slot — adopted
    /// at palette-transition completion, mirroring the aeolian
    /// field / solar arcade).
    pub(super) field_palette_slot: u8,
    pub(super) current_cells: Vec<DnaCell>,
    pub(super) previous_cells: Vec<DnaCell>,
    pub(super) drawn_gen: Vec<u32>,
    pub(super) drawn_gen_counter: u32,
    /// Total absorbed nucleotides since reset (test-only dial).
    #[cfg(test)]
    absorptions_for_test: usize,
}

impl DnaHelixRain {
    pub(crate) fn new() -> Self {
        Self {
            drops: Vec::new(),
            genome: DnaGenome::new(),
            active_count: 0,
            spawn_scan_idx: 0,
            last_step: None,
            field_palette_slot: 0,
            current_cells: Vec::new(),
            previous_cells: Vec::new(),
            drawn_gen: Vec::new(),
            drawn_gen_counter: 0,
            #[cfg(test)]
            absorptions_for_test: 0,
        }
    }

    /// Rebuild the pool + genome for a new viewport (or style
    /// entry). The pool is one drop per column; the genome is
    /// re-counted for the new height (charges wiped, fork
    /// re-armed — a dormant molecule must not carry painted
    /// recency into the next entry).
    pub(crate) fn reset(&mut self, cols: u16, lines: u16) {
        let lanes = cols.max(1) as usize;
        self.drops.clear();
        self.drops.resize_with(lanes, NucleotideDrop::vacant);
        self.genome.reset(cols, lines);
        self.active_count = 0;
        self.spawn_scan_idx = 0;
        self.last_step = None;
        #[cfg(test)]
        {
            self.absorptions_for_test = 0;
        }
        self.clear_draw_history();
    }

    pub(crate) fn active_count(&self) -> usize {
        self.active_count
    }

    /// Palette transition completion: the molecule adopts the new
    /// slot (one body, one slot), and every active drop adopts it
    /// individually (mirrors the family contract).
    pub(crate) fn adopt_palette_slot(&mut self, palette_slot: u8) {
        self.field_palette_slot = palette_slot;
        for d in &mut self.drops {
            if d.active {
                d.palette_slot = palette_slot;
            }
        }
    }

    /// Drop the diff-cleanup history (semantic invalidation /
    /// forced redraw). The next draw pass rebuilds it from an
    /// empty baseline. The genome itself is simulation state and
    /// is preserved (wiping it would erase a transcribed genome).
    pub(crate) fn clear_draw_history(&mut self) {
        self.current_cells.clear();
        self.previous_cells.clear();
        self.drawn_gen.clear();
        self.drawn_gen_counter = 0;
    }

    /// Steady-state active-drop target from pool size + density
    /// (mirrors `SolarFlareRain::target_active_count`): the
    /// calm-sky dial family — a sparse ambient soup, never a
    /// downpour (the molecule is the hero).
    fn target_active_count(lanes: usize, density: f32) -> usize {
        if lanes == 0 {
            return 0;
        }
        let ratio = (DNA_ACTIVE_BASE + density.clamp(0.01, 5.0) * DNA_ACTIVE_DENSITY_MULT)
            .clamp(0.02, DNA_ACTIVE_MAX);
        ((lanes as f32 * ratio).round() as usize).clamp(1, lanes)
    }

    /// Amortized free-slot scan (rotating cursor — mirrors the
    /// family).
    fn find_inactive_drop(&mut self) -> Option<usize> {
        let len = self.drops.len();
        if len == 0 {
            return None;
        }
        for step in 0..len {
            let idx = (self.spawn_scan_idx + step) % len;
            if !self.drops[idx].active {
                self.spawn_scan_idx = (idx + 1) % len;
                return Some(idx);
            }
        }
        None
    }

    /// Spawn pass — the accumulator contract identical to the
    /// structured family (deficit-bounded budget + fractional
    /// remainder carry; the equilibrium is a trickle). The spawn
    /// position rolls inside the capture band: the soup falls
    /// where the genome can catch it (law 5's concentration arm).
    pub(crate) fn spawn(
        &mut self,
        elapsed: Duration,
        spawn_remainder: &mut f32,
        params: &DnaSpawnParams,
        random: &mut DnaRandom<'_>,
    ) {
        if params.cols == 0 || params.lines == 0 || self.drops.is_empty() {
            *spawn_remainder = 0.0;
            return;
        }

        let target = Self::target_active_count(self.drops.len(), params.density);
        if self.active_count >= target {
            *spawn_remainder = (*spawn_remainder).min(SPAWN_REMAINDER_CAP);
            return;
        }

        let deficit = target - self.active_count;
        let spawn_rate =
            (target as f32 * DNA_SPAWN_RATE_MULT + DNA_SPAWN_RATE_FLOOR) * params.spawn_scale;
        let budget =
            elapsed.as_secs_f32() * spawn_rate + (*spawn_remainder).min(SPAWN_REMAINDER_CAP);
        if !budget.is_finite() || budget <= 0.0 {
            *spawn_remainder = 0.0;
            return;
        }

        let to_spawn = (budget.floor() as usize).min(deficit);
        *spawn_remainder = (budget - to_spawn as f32).min(SPAWN_REMAINDER_CAP);
        if to_spawn == 0 {
            return;
        }

        // The capture band: the molecule's reach (base radius
        // scaled by the band multiplier), centered on the helix
        // axis. The soup falls where the genome can catch it — a
        // nucleotide spawned over empty sky reads as a miss, and
        // misses are the minority by design.
        let cx = self.genome.center_x();
        let band = (self.genome.radius() * DNA_CAPTURE_BAND_MULT)
            .min((params.cols.max(1) as f32 - 1.0) * 0.5);

        let mut spawned = 0usize;
        for _ in 0..to_spawn {
            let Some(idx) = self.find_inactive_drop() else {
                break;
            };
            let roll_a = random.rand_chance.sample(random.rng);
            let roll_b = random.rand_chance.sample(random.rng);
            let roll_c = random.rand_chance.sample(random.rng);
            let x = (cx + (roll_a * 2.0 - 1.0) * band).clamp(0.0, (params.cols - 1) as f32);
            // Entry just above the frame: the drop falls INTO view
            // (the lorenz first-position contract).
            let y = -1.0 - roll_b;
            // Terminal velocity inside its band (no acceleration —
            // free solution).
            let vy = DNA_FALL_MULT * (1.0 + (roll_c - 0.5) * 2.0 * DNA_FALL_BAND);
            let lifetime = DNA_MAX_AGE_SECS * (0.85 + roll_c * 0.30);
            let d = &mut self.drops[idx];
            d.active = true;
            d.seed(x, y, vy, lifetime);
            d.palette_slot = params.active_palette_slot;
            // The glyph is picked at the first draw pass (the
            // lorenz contract).
            spawned += 1;
        }
        self.active_count += spawned;
    }

    /// Motion pass — the molecule's physics core (laws 1-4 in the
    /// genome's advance; law 5 here).
    ///
    /// 1. The genome takes one breath (rotation, decay, the fork
    ///    — and the fork-pass mutations).
    /// 2. Each drop: terminal-velocity fall + the clamped
    ///    brownian drift + the wall clamps; the absorption test
    ///    (a rung-line crossing inside the rung's projected span
    ///    deposits the charge and, on the mutation roll, re-rolls
    ///    the pair — the rain edits the genome it lands on); the
    ///    floor expiry and the lifetime backstop sweep the rest.
    ///    Absorptions are collected and applied after the loop
    ///    (the solar landings pattern — the genome needs the &mut
    ///    while the drop loop holds the pool).
    pub(crate) fn advance(&mut self, step: &DnaStep, random: &mut DnaRandom<'_>) {
        let has_genome = !self.genome.rungs().is_empty();
        if self.active_count == 0 && !has_genome {
            self.last_step = Some(step.now);
            return;
        }
        let dt_wall = match self.last_step {
            Some(last) => {
                step.now
                    .saturating_duration_since(last)
                    .as_secs_f32()
                    .min(step.max_sim_delta.as_secs_f32())
                    .max(0.0)
                    * step.resume_blend.clamp(0.0, 1.0)
            }
            None => 0.0,
        };
        self.last_step = Some(step.now);
        if dt_wall <= 0.0 {
            return;
        }

        // The family speed contract: one sim clock for the
        // molecule, the fork and the soup alike (shapes invariant
        // under the speed keys).
        let dt_sim = dt_wall * step.chars_per_sec.max(0.0) * DNA_SIM_TIME_PER_CPS;

        // 1. The molecule breathes (laws 1, 3, 4).
        self.genome.advance(dt_sim, random);

        if self.active_count == 0 {
            return;
        }

        let cols_f = step.cols.max(1) as f32;
        let floor = step.lines.saturating_sub(1) as f32;
        let rung_count = self.genome.rungs().len();

        let mut absorptions: Vec<(usize, bool, f32)> = Vec::new();
        let mut died = 0usize;

        for d in &mut self.drops {
            if !d.active {
                continue;
            }
            let prev_y = d.y;
            d.sim_age += dt_sim;

            // The brownian drift: a small random walk on the
            // lateral velocity, clamped (the drift never flings a
            // nucleotide — it wanders).
            let roll = random.rand_chance.sample(random.rng);
            d.vx += (roll - 0.5) * 2.0 * DNA_DRIFT_RATE * dt_sim;
            d.vx = d.vx.clamp(-DNA_DRIFT_MAX, DNA_DRIFT_MAX);

            d.x += d.vx * dt_sim;
            d.y += d.vy * dt_sim;

            // The wall clamps (a drifting drop hugs the edge,
            // never exits).
            if d.x < 0.0 {
                d.x = 0.0;
                d.vx = -d.vx * 0.5;
            } else if d.x > cols_f - 1.0 {
                d.x = cols_f - 1.0;
                d.vx = -d.vx * 0.5;
            }

            // Law 5's absorption test: the drop's line crossed a
            // rung line this tick AND its column sits inside that
            // rung's projected span (+- the capture margin). The
            // test is a crossing test on a strictly increasing y
            // (terminal velocity, vy > 0) — no tunneling past a
            // rung line at any dt.
            let mut was_absorbed = false;
            for ri in 0..rung_count {
                let Some(rung_line) = self.genome.rung_line(ri) else {
                    continue;
                };
                let rl = rung_line as f32;
                if !(prev_y < rl && d.y >= rl) {
                    continue;
                }
                let Some((left, right)) = self.genome.rung_span(ri) else {
                    continue;
                };
                if d.x >= left - DNA_CAPTURE_MARGIN && d.x <= right + DNA_CAPTURE_MARGIN {
                    // Absorbed: the charge deposits, and with the
                    // mutation chance the pair re-rolls — the rain
                    // visibly edits the genome it lands on.
                    let mutation = random.rand_chance.sample(random.rng) < DNA_MUTATION_CHANCE;
                    let roll = random.rand_chance.sample(random.rng);
                    absorptions.push((ri, mutation, roll));
                    was_absorbed = true;
                    break;
                }
            }
            if was_absorbed {
                d.active = false;
                d.clear_trail();
                continue;
            }

            // The floor expiry (the free-floating soup that found
            // no partner) and the lifetime backstop.
            if d.y >= floor || d.sim_age >= d.lifetime {
                d.active = false;
                d.clear_trail();
                died += 1;
            }
        }

        // Apply the absorptions (law 5's deposition + mutation).
        for (ri, mutation, roll) in absorptions {
            self.genome.absorb(ri, DNA_DEPOSIT_RATE, mutation, roll);
            #[cfg(test)]
            {
                self.absorptions_for_test += 1;
            }
        }
        if died > 0 {
            self.active_count = self.active_count.saturating_sub(died);
        }
    }

    // -- Test-only diagnostics (mirrors the family *_for_test API) --

    #[cfg(test)]
    pub(crate) fn drop_states_for_test(&self) -> Vec<(f32, f32, f32)> {
        self.drops
            .iter()
            .filter(|d| d.active)
            .map(|d| (d.x, d.y, d.vy))
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn absorptions_for_test(&self) -> usize {
        self.absorptions_for_test
    }

    #[cfg(test)]
    pub(crate) fn genome_for_test(&self) -> &DnaGenome {
        &self.genome
    }

    #[cfg(test)]
    pub(crate) fn drawn_cells_for_test(&self) -> &[DnaCell] {
        &self.current_cells
    }
}
