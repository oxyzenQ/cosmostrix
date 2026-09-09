// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The genome — the structure half of the DNA helix style
//! (NIGHT-research-7).
//!
//! This module owns the molecule's executable state: the rung
//! table (one rung every RUNG_STEP lines — pair state, synthesis
//! charge, the fork's dissolution window), the rotation phase,
//! the replication fork, the genesis clock (law 0 — the birth
//! sequence's mutable half, see genesis.rs), and the closed-form
//! geometry queries the draw pass consumes (strand positions,
//! rung spans, the bow envelope). The full derivation essay lives
//! in `type_rain/dna_helix/mod.rs`; this file is the executable
//! form of laws 0 through 4.
//!
//! Storage: one flat `Vec<DnaRung>` indexed by rung ordinal (line
//! = 1 + idx x RUNG_STEP, clamped to the viewport height — the
//! top line is reserved as sky so the fork's re-entry reads as a
//! wave arriving, not a pop). No per-cell fields — the molecule's
//! spatial structure is recomputed closed-form from the phase each
//! frame (the projection IS the geometry, law 2).

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::constants::{
    DNA_BOW_MAX, DNA_CHARGE_DECAY, DNA_CHARGE_LEVEL_CORE, DNA_CHARGE_LEVEL_HOT,
    DNA_CHARGE_LEVEL_MID, DNA_CHARGE_MAX, DNA_FORK_GAP, DNA_FORK_RATE, DNA_REPLICATION_CLOCK_MEAN,
    DNA_ROT_RATE, DNA_RUNG_STEP, DNA_R_FRAC, DNA_R_MAX, DNA_R_MIN, DNA_TURN_LINES,
};

use super::super::monolith::BrightnessLevel;

use super::genesis::{
    genesis_phase, genesis_radius_growth, genesis_total_secs, ladder_front, windup_front,
    GenesisPhase,
};

/// RNG bundle (the advance pass is a stochastic pass in the family
/// sense: the replication clock re-rolls, the fork-pass mutations
/// re-roll and the soup's brownian drift all ride along).
pub(crate) struct DnaRandom<'a> {
    pub(crate) rng: &'a mut StdRng,
    pub(crate) rand_chance: &'a Uniform<f32>,
}

/// The four Watson-Crick pair states (law 2). The rung END cells
/// show these glyphs — the molecule's identity, carried whatever
/// charset the user picks (the dragon-head glyph precedent: the
/// semantic glyph rides the pool, it is not picked from it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BasePair {
    AdenineThymine,
    ThymineAdenine,
    GuanineCytosine,
    CytosineGuanine,
}

impl BasePair {
    /// Re-roll from a uniform roll in [0, 1).
    pub(crate) fn from_roll(roll: f32) -> Self {
        match roll {
            r if r < 0.25 => Self::AdenineThymine,
            r if r < 0.50 => Self::ThymineAdenine,
            r if r < 0.75 => Self::GuanineCytosine,
            _ => Self::CytosineGuanine,
        }
    }

    /// The two end glyphs (left end, right end) — complementarity
    /// is the point: A bonds T, G bonds C, and the rung shows it.
    pub(crate) fn end_glyphs(self) -> (char, char) {
        match self {
            Self::AdenineThymine => ('A', 'T'),
            Self::ThymineAdenine => ('T', 'A'),
            Self::GuanineCytosine => ('G', 'C'),
            Self::CytosineGuanine => ('C', 'G'),
        }
    }
}

/// One base-pair rung (laws 2 + 3): the pair state, the synthesis
/// recency charge, and nothing else — position, span and depth are
/// closed-form reads of the phase, never stored.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DnaRung {
    /// The Watson-Crick pair the ends display.
    pub(crate) pair: BasePair,
    /// Synthesis recency in [0, CHARGE_MAX] (law 3, hard-clamped).
    pub(crate) charge: f32,
}

impl DnaRung {
    fn vacant() -> Self {
        Self {
            pair: BasePair::AdenineThymine,
            charge: 0.0,
        }
    }
}

/// The fork's lifecycle (law 4): armed (the clock counts down to
/// the next sweep), or traveling (the wave sweeps the height).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ForkPhase {
    /// The replication clock counts down (the molecule rests).
    Armed,
    /// The wave travels (fork_y sweeps top to bottom).
    Traveling,
}

/// The molecule: rung table + rotation phase + replication fork.
#[derive(Debug)]
pub(crate) struct DnaGenome {
    /// Rung states, one per RUNG_STEP lines (index -> line).
    rungs: Vec<DnaRung>,
    /// Rotation phase in radians (law 1, wrapped at 128 turns).
    pub(crate) phase: f32,
    /// Radians per line (the twist, k = 2pi / TURN_LINES).
    twist: f32,
    /// Center column (fractional).
    cx: f32,
    /// Base radius in columns, clamped to the band.
    radius: f32,
    /// The fork state machine + position + clock (law 4).
    pub(crate) fork_phase: ForkPhase,
    /// Fork center line, fractional (Traveling only).
    pub(crate) fork_y: f32,
    /// Sim-seconds until the next sweep opens (Armed only).
    replication_clock: f32,
    /// Genesis clock (law 0): sim-seconds since the birth
    /// sequence began. Rides the same sim clock as every
    /// molecule rate (the family speed contract scales the birth
    /// with the molecule — the black hole's formation rides
    /// wall-time instead, its own precedent).
    genesis_t: f32,
    /// The genesis completion flag: false while the birth
    /// sequence runs (the fork gate, the strand and rung queries
    /// and the spawn dial read it). Flips once, stays — style
    /// entry re-arms it through `begin_genesis` (the black
    /// hole's `begin_formation` contract: a pure resize keeps
    /// the steady state, a scene entry re-forms).
    formed: bool,
    /// Viewport (kept for the reset and the bounds queries).
    cols: u16,
    lines: u16,
}

impl DnaGenome {
    pub(crate) fn new() -> Self {
        Self {
            rungs: Vec::new(),
            // Face-on birth presentation: the pre-molecule holds
            // this angle through the soup and the ladder (sin at
            // the max — the flat ladder spans its widest rungs);
            // a zero phase would present the forming ladder
            // edge-on, a single collapsing line.
            phase: std::f32::consts::FRAC_PI_2,
            twist: std::f32::consts::TAU / DNA_TURN_LINES as f32,
            cx: 0.0,
            radius: 0.0,
            fork_phase: ForkPhase::Armed,
            fork_y: 0.0,
            replication_clock: DNA_REPLICATION_CLOCK_MEAN,
            genesis_t: 0.0,
            // A fresh construction is unborn (the first launch's
            // genesis plays without a scene transition arming it
            // — the black hole's default-unformed precedent); the
            // bench path fast-forwards past the sequence.
            formed: false,
            cols: 0,
            lines: 0,
        }
    }

    /// Rebuild the molecule for a new viewport (or style entry):
    /// rung count from the height, phase preserved on resize (a
    /// growing viewport must not snap the turn), charges wiped (a
    /// dormant molecule must not carry painted recency into the
    /// next entry — the solar arcade-reset precedent), fork
    /// re-armed. The genesis clock and the formed flag are NOT
    /// touched (the black hole's reset contract): a pure resize
    /// keeps the steady state, and style entry follows this reset
    /// with `begin_genesis` when the birth sequence should replay.
    /// The family reset contract is RNG-free.
    pub(crate) fn reset(&mut self, cols: u16, lines: u16) {
        self.cols = cols;
        self.lines = lines;
        let count = rung_count_for_lines(lines);
        self.rungs.clear();
        self.rungs.resize_with(count, DnaRung::vacant);
        self.cx = (cols.max(1) as f32 - 1.0) * 0.5;
        self.radius = helix_radius(cols);
        self.fork_phase = ForkPhase::Armed;
        self.fork_y = 0.0;
        self.replication_clock = DNA_REPLICATION_CLOCK_MEAN;
    }

    pub(crate) fn rungs(&self) -> &[DnaRung] {
        &self.rungs
    }

    /// The helix axis column (the capture band's center, law 5).
    pub(crate) fn center_x(&self) -> f32 {
        self.cx
    }

    /// The molecule's base radius in columns (the capture band's
    /// reach, law 5).
    pub(crate) fn radius(&self) -> f32 {
        self.radius
    }

    /// The viewport height the genome was built for (the fork
    /// envelope's scale input).
    pub(crate) fn lines(&self) -> u16 {
        self.lines
    }

    /// Replay the genesis intro (law 0): rewind the genesis
    /// clock, clear the formed flag and present the molecule
    /// face-on — the next frames run the birth sequence (soup
    /// -> ladder -> windup). Called on style ENTRY only (scene
    /// switches and first launch); a pure resize keeps the
    /// steady state (the black hole's `begin_formation`
    /// contract).
    pub(crate) fn begin_genesis(&mut self) {
        self.genesis_t = 0.0;
        self.formed = false;
        self.phase = std::f32::consts::FRAC_PI_2;
    }

    /// Skip straight to the steady molecule (the bench path and
    /// the steady-state tests): the clock parks at the total, the
    /// formed flag flips, and no assembly charges are written —
    /// the cold post-reset molecule, exactly the pre-genesis
    /// bench profile (regression comparability).
    pub(crate) fn fast_forward_genesis(&mut self) {
        self.genesis_t = genesis_total_secs();
        self.formed = true;
    }

    /// Law 0's draw gate: do the strands draw at all right now?
    /// False through the primordial soup (the sky is rain alone);
    /// true from the ladder on (the axis spine onward).
    pub(crate) fn molecule_visible(&self) -> bool {
        self.formed || genesis_phase(self.genesis_t) != GenesisPhase::Soup
    }

    /// Law 0's thick-broth dial: is the spawn target running its
    /// genesis multiplier right now (the molecule absent or still
    /// assembling — the soup IS the scene)?
    pub(crate) fn primordial_soup(&self) -> bool {
        !self.formed
            && matches!(
                genesis_phase(self.genesis_t),
                GenesisPhase::Soup | GenesisPhase::Ladder
            )
    }

    /// Law 0's rung gate: has the assembly wave written this rung
    /// yet? Closed-form — the rung's line against the wave
    /// position (no per-rung build state, the module's
    /// no-per-cell-fields contract). All rungs are built in the
    /// steady state.
    pub(crate) fn rung_built(&self, idx: usize) -> bool {
        if self.formed {
            return true;
        }
        match self.rung_line(idx) {
            Some(line) => (line as f32) <= ladder_front(self.genesis_t, self.lines),
            None => false,
        }
    }

    /// Law 5's deposition: an absorbed nucleotide deposits its
    /// charge (hard-clamped — bounded by construction) and, when
    /// the caller's mutation roll fired, re-rolls the pair (the
    /// rain visibly edits the genome it lands on).
    pub(crate) fn absorb(&mut self, idx: usize, charge: f32, mutate: bool, roll: f32) {
        if let Some(r) = self.rungs.get_mut(idx) {
            r.charge = (r.charge + charge).min(DNA_CHARGE_MAX);
            if mutate {
                r.pair = BasePair::from_roll(roll);
            }
        }
    }

    /// The rung ordinal whose line sits at `idx`'s line, or None
    /// past the table. Rung i lives at line 1 + i x RUNG_STEP.
    pub(crate) fn rung_line(&self, idx: usize) -> Option<u16> {
        if idx < self.rungs.len() {
            Some(rung_line_for_idx(idx, self.lines))
        } else {
            None
        }
    }

    /// Law 1: the strand angle at line y (radians, unbounded —
    /// trig-equivalent; the phase accumulator wraps).
    ///
    /// Law 0's deformation while the molecule is unborn: the soup
    /// and the ladder hold ONE angle for every line (the flat
    /// pre-molecule, face-on by construction); the windup zips
    /// the twist in from the top — above the front the strand
    /// carries the full steady law, below it the flat extension
    /// (the angle at the front, held constant down the ladder —
    /// the wound top drags the flat tail around the axis as it
    /// descends). At the windup's end the front is the full
    /// height and the formula evaluates exactly to the steady
    /// law (no seam, no pop).
    pub(crate) fn strand_angle(&self, line: f32) -> f32 {
        if !self.formed {
            match genesis_phase(self.genesis_t) {
                GenesisPhase::Soup | GenesisPhase::Ladder => return self.phase,
                GenesisPhase::Windup => {
                    let front = windup_front(self.genesis_t, self.lines);
                    let y = line.min(front);
                    return self.phase + y * self.twist;
                }
                GenesisPhase::Steady => {}
            }
        }
        self.phase + line * self.twist
    }

    /// Law 4's bow: the local radius multiplier at line y — 1.0
    /// far from the fork, up to 1 + BOW_MAX at the fork center
    /// (a Gaussian envelope over the fork's position). Pure
    /// closed form: no state, recomputed exactly each frame.
    pub(crate) fn radius_scale(&self, line: f32) -> f32 {
        if self.fork_phase != ForkPhase::Traveling {
            return 1.0;
        }
        let sigma = fork_sigma(self.lines);
        let d = line - self.fork_y;
        1.0 + DNA_BOW_MAX * (-d * d / (2.0 * sigma * sigma)).exp()
    }

    /// The effective radius at line y (base x bow scale), clamped
    /// so the bowed strands never leave the viewport margins.
    ///
    /// Law 0's deformation while the molecule is unborn: the
    /// radius grows from the axis (the spine splitting into the
    /// two strands, cubic ease-out over the ladder window) and
    /// the legibility floor is lifted through the growth (a
    /// clamped-to-R_MIN spine would never read as the single seed
    /// line); the top clamp (the viewport margin) holds
    /// throughout. The bow is quiescent during the genesis (the
    /// fork is gated), so the product composes safely.
    pub(crate) fn effective_radius(&self, line: f32) -> f32 {
        if !self.formed {
            return (self.radius * self.radius_scale(line) * genesis_radius_growth(self.genesis_t))
                .min(self.margin_radius());
        }
        let r = self.radius * self.radius_scale(line);
        r.clamp(DNA_R_MIN, self.margin_radius())
    }

    /// The largest radius that keeps both strand extremes inside
    /// the viewport (a margin of one cell each side).
    fn margin_radius(&self) -> f32 {
        ((self.cols.max(1) as f32 - 2.0) * 0.5).max(DNA_R_MIN)
    }

    /// Strand A position (x, depth) at line y — the plus strand.
    /// Depth in [-1, 1]: positive is front (toward the viewer).
    pub(crate) fn strand_a(&self, line: f32) -> (f32, f32) {
        let theta = self.strand_angle(line);
        let x = self.cx + self.effective_radius(line) * theta.sin();
        (x, theta.cos())
    }

    /// Strand B position (x, depth) at line y — the minus strand
    /// (theta + pi: x mirrored, depth negated).
    pub(crate) fn strand_b(&self, line: f32) -> (f32, f32) {
        let (ax, ad) = self.strand_a(line);
        (2.0 * self.cx - ax, -ad)
    }

    /// Law 2: the rung span (left x, right x) at the rung's line —
    /// the projection between the two strand ends, stretched by
    /// the local bow so the Y's rungs widen with the strands.
    pub(crate) fn rung_span(&self, idx: usize) -> Option<(f32, f32)> {
        let line = self.rung_line(idx)? as f32;
        let (ax, _) = self.strand_a(line);
        let (bx, _) = self.strand_b(line);
        Some((ax.min(bx), ax.max(bx)))
    }

    /// The rung depth blend at fraction t in [0, 1] across the
    /// span (law 2: the rung cell interpolates the strand depths).
    pub(crate) fn rung_depth(&self, idx: usize, t: f32) -> Option<f32> {
        let line = self.rung_line(idx)? as f32;
        let (_, ad) = self.strand_a(line);
        Some(ad * (1.0 - t) - ad * t)
    }

    /// Law 4's window: is the rung at `idx` dissolved right now
    /// (the fork center within GAP/2 of its line)?
    pub(crate) fn rung_dissolved(&self, idx: usize) -> bool {
        if self.fork_phase != ForkPhase::Traveling {
            return false;
        }
        match self.rung_line(idx) {
            Some(line) => (line as f32 - self.fork_y).abs() < fork_sigma(self.lines) * 1.5,
            None => false,
        }
    }

    /// Law 0 (the clock) + laws 1, 3, 4: the molecule breathes.
    ///
    /// The genesis clock advances once per tick (clamped at the
    /// total — one shot); the assembly wave's crossing test runs
    /// against the pre-update clock the same frame (the fork's
    /// prev/curr pattern, so a lag spike's large dt still writes
    /// every rung the front jumped past). The rotation is HELD
    /// through the soup and the ladder (the flat pre-molecule
    /// stays face-on) and resumes with the windup. The fork is
    /// gated: no replication before the genome exists — the
    /// replication clock holds at its reset value until the
    /// molecule completes.
    pub(crate) fn advance(&mut self, dt: f32, random: &mut DnaRandom<'_>) {
        if dt <= 0.0 || self.rungs.is_empty() {
            return;
        }

        // Law 0 — the genesis clock (one shot, sim-time).
        let prev_genesis_t = self.genesis_t;
        if !self.formed {
            self.genesis_t = (self.genesis_t + dt).min(genesis_total_secs());
            if self.genesis_t >= genesis_total_secs() {
                self.formed = true;
            }
        }

        // Law 1 — the turn: one uniform rotation on the sim clock,
        // held while the pre-molecule assembles (a rotating flat
        // ladder periodically collapses edge-on to a single
        // line — the face-on presentation is the birth's stage
        // lighting; the windup resumes the turn).
        let rotation_held = !self.formed
            && matches!(
                genesis_phase(self.genesis_t),
                GenesisPhase::Soup | GenesisPhase::Ladder
            );
        if !rotation_held {
            self.phase += DNA_ROT_RATE * dt;
            // The vortex arm-phase precedent: wrap past 128 turns so
            // the f32 ulp stays far below visual resolution on
            // multi-day sessions (trig-equivalent).
            if self.phase.abs() > DNA_PHASE_WRAP_LIMIT {
                self.phase = self.phase.rem_euclid(std::f32::consts::TAU);
            }
        }

        // Law 3 — the recency decay (strict, per rung).
        for r in &mut self.rungs {
            r.charge *= (-DNA_CHARGE_DECAY * dt).exp();
        }

        // Law 0's assembly wave (the ladder window): every rung
        // whose line the front crossed this tick is WRITTEN — the
        // charge stamps to max and the pair rolls (the fork's
        // fresh-write economy borrowed for the birth; the genome
        // writes itself into existence, top-down, its trail of
        // light decaying under law 3 as the wave travels on).
        if !self.formed {
            let floor = self.lines.saturating_sub(1);
            let prev_front = ladder_front(prev_genesis_t, self.lines);
            let front = ladder_front(self.genesis_t, self.lines);
            if front > prev_front {
                for (idx, r) in self.rungs.iter_mut().enumerate() {
                    let line_f = rung_line_for_idx(idx, floor) as f32;
                    if prev_front < line_f && front >= line_f {
                        r.charge = DNA_CHARGE_MAX;
                        r.pair = BasePair::from_roll(random.rand_chance.sample(random.rng));
                    }
                }
            }
        }

        // Law 4 — the fork (gated on the completed genome: the
        // replication clock counts down only in the steady
        // state — no replication before the molecule exists).
        let floor_f = self.lines.saturating_sub(1) as f32;
        if self.formed {
            match self.fork_phase {
                ForkPhase::Armed => {
                    self.replication_clock -= dt;
                    if self.replication_clock <= 0.0 {
                        self.fork_phase = ForkPhase::Traveling;
                        // Entry above the screen: the wave ARRIVES (the
                        // first rungs dissolve as it sweeps in, never
                        // a pop).
                        self.fork_y = -fork_sigma(self.lines);
                        self.replication_clock = 0.0;
                    }
                }
                ForkPhase::Traveling => {
                    let prev = self.fork_y;
                    self.fork_y += DNA_FORK_RATE * dt;
                    // Every rung whose line the center crossed this
                    // tick re-synthesizes (mutation included). The
                    // line registry is a pure function of the ordinal
                    // (no rung state), so the pass reads it without
                    // touching the borrow.
                    let floor = self.lines.saturating_sub(1);
                    for (idx, r) in self.rungs.iter_mut().enumerate() {
                        let line_f = rung_line_for_idx(idx, floor) as f32;
                        if prev < line_f && self.fork_y >= line_f {
                            r.charge = DNA_CHARGE_MAX;
                            r.pair = BasePair::from_roll(random.rand_chance.sample(random.rng));
                        }
                    }
                    if self.fork_y > floor_f + fork_sigma(self.lines) {
                        self.fork_phase = ForkPhase::Armed;
                        self.fork_y = 0.0;
                        self.replication_clock = DNA_REPLICATION_CLOCK_MEAN
                            * (0.6 + random.rand_chance.sample(random.rng) * 0.8);
                    }
                }
            }
        }
    }

    // -- Test-only diagnostics (mirrors the family *_for_test API) --

    #[cfg(test)]
    pub(crate) fn charge_for_test(&self, idx: usize) -> f32 {
        self.rungs.get(idx).map_or(0.0, |r| r.charge)
    }

    #[cfg(test)]
    pub(crate) fn pair_for_test(&self, idx: usize) -> Option<BasePair> {
        self.rungs.get(idx).map(|r| r.pair)
    }

    #[cfg(test)]
    pub(crate) fn rung_count_for_test(&self) -> usize {
        self.rungs.len()
    }

    /// Force the fork into a position/phase (the replication
    /// tests' deterministic arm).
    #[cfg(test)]
    pub(crate) fn plant_fork_for_test(&mut self, phase: ForkPhase, fork_y: f32, clock: f32) {
        self.fork_phase = phase;
        self.fork_y = fork_y;
        self.replication_clock = clock;
    }

    /// Charge one rung directly (the absorption tests' seed).
    #[cfg(test)]
    pub(crate) fn plant_charge_for_test(&mut self, idx: usize, charge: f32) {
        if let Some(r) = self.rungs.get_mut(idx) {
            r.charge = charge;
        }
    }

    /// The genesis completion flag (the formation tests' arm — the
    /// black hole's `formed_for_test` contract).
    #[cfg(test)]
    pub(crate) fn formed_for_test(&self) -> bool {
        self.formed
    }

    /// The genesis clock in sim-seconds (the timeline tests' read).
    #[cfg(test)]
    pub(crate) fn genesis_t_for_test(&self) -> f32 {
        self.genesis_t
    }

    /// The twist in radians per line (the genesis geometry tests'
    /// expected-law input, k = 2pi / TURN_LINES).
    #[cfg(test)]
    pub(crate) fn twist_for_test(&self) -> f32 {
        self.twist
    }

    /// Force the rotation phase (the steady-geometry tests'
    /// deterministic arm — the crossings' alignment against the
    /// odd-line rung registry is phase-sensitive).
    #[cfg(test)]
    pub(crate) fn plant_phase_for_test(&mut self, phase: f32) {
        self.phase = phase;
    }
}

/// Phase wrap threshold (radians, 128 turns — the vortex
/// arm-phase precedent, law 1's LTS note).
const DNA_PHASE_WRAP_LIMIT: f32 = 128.0 * std::f32::consts::TAU;

/// Rung count for a viewport height: one rung every RUNG_STEP
/// lines from line 1 to the floor (a rung needs its line inside
/// the viewport — a 3-line terminal carries zero rungs, the
/// strands alone).
pub(crate) fn rung_count_for_lines(lines: u16) -> usize {
    let usable = lines.saturating_sub(1) as i32;
    if usable < DNA_RUNG_STEP as i32 {
        return 0;
    }
    (usable / DNA_RUNG_STEP as i32) as usize
}

/// The line of rung ordinal `idx` (clamped to the floor — the
/// last rung never renders past the viewport).
pub(crate) fn rung_line_for_idx(idx: usize, lines: u16) -> u16 {
    let line = 1 + (idx as u32 * DNA_RUNG_STEP as u32) as u16;
    line.min(lines.saturating_sub(1))
}

/// The molecule's base radius for a viewport width (law 1): a
/// fraction of the width clamped to the legibility band — narrow
/// terminals keep a readable helix, wide ones cap so the icon
/// stays an icon (the single-body flagship aesthetic: the black
/// hole's centered ball, the vortex's centered orbit).
pub(crate) fn helix_radius(cols: u16) -> f32 {
    (cols.max(1) as f32 * DNA_R_FRAC).clamp(DNA_R_MIN, DNA_R_MAX)
}

/// The fork envelope's sigma (law 4): the bow and the dissolution
/// window scale with the height (a short molecule needs a tight
/// fork; a tall one a visible Y), floored so the envelope never
/// collapses to a spike.
pub(crate) fn fork_sigma(lines: u16) -> f32 {
    DNA_FORK_GAP.min(lines.max(4) as f32 * 0.35).max(2.0)
}

/// Law 3's draw read: the recency ladder (the genome's own
/// brightness — Ghost the archive, Mid transcribed, Hot fresh,
/// Core the replication window).
pub(crate) fn charge_level(charge: f32) -> BrightnessLevel {
    if charge > DNA_CHARGE_LEVEL_CORE {
        BrightnessLevel::Core
    } else if charge > DNA_CHARGE_LEVEL_HOT {
        BrightnessLevel::Hot
    } else if charge > DNA_CHARGE_LEVEL_MID {
        BrightnessLevel::Mid
    } else {
        BrightnessLevel::Ghost
    }
}

// The shipped calibration must stay internally consistent — the
// compile-time checks mirror the solar flare constants' pattern
// (band order, ladder order, the twist/pair ratio).
const _: () = assert!(DNA_R_MIN < DNA_R_MAX);
const _: () = assert!(DNA_R_FRAC > 0.0);
const _: () = assert!(DNA_CHARGE_LEVEL_MID < DNA_CHARGE_LEVEL_HOT);
const _: () = assert!(DNA_CHARGE_LEVEL_HOT < DNA_CHARGE_LEVEL_CORE);
const _: () = assert!(DNA_CHARGE_LEVEL_CORE < DNA_CHARGE_MAX);
const _: () = assert!(DNA_RUNG_STEP >= 1);
const _: () = assert!(DNA_TURN_LINES > DNA_RUNG_STEP);
const _: () = assert!(DNA_FORK_RATE > 0.0);
const _: () = assert!(DNA_BOW_MAX >= 0.0);
