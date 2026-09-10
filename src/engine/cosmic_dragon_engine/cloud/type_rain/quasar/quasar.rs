// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The quasar engine state machine (NIGHT-research-8, the
//! thirteenth style) — the orchestration pass set: pools, spawn,
//! advance, draw.
//!
//! The style is the feeding engine (see mod.rs for the full
//! derivation): four populations around one center — the Kepler
//! disk, the polar jets, the host halo and the infalling fuel —
//! advanced on the single family clock. The infall rides the
//! spawn accumulator (the staggered entry, the family contract);
//! the disk is not spawned, it is BUILT from the rain (every
//! absorbed streamer becomes an orbit, the capture economy);
//! the jets fire at the ignition's jet phase and recycle forever
//! after; the halo glides in from beyond the frame.
//!
//! The ignition (law 0) rides sim-time: a scene entry re-arms the
//! birth (reset + begin_ignition), a pure resize keeps the
//! burning engine (the pools rebuild to the steady state when
//! already lit), and the bench fast-forwards past the
//! choreography (the Z-6 critical-path contract — the ignition
//! tests pin the sequence frame by frame instead).

use std::time::{Duration, Instant};

use rand::distr::Distribution;

use crate::constants::{
    QUAS_FLARE_CLOCK_MEAN, QUAS_FLARE_SURGE, QUAS_FLARE_WINDOW, QUAS_IGNITION_INFALL_MULT,
    QUAS_INFALL_MAX, QUAS_INFALL_MIN, QUAS_JET_PER_BEAM, QUAS_KNOT_V, QUAS_KNOT_W, QUAS_MAX_DISK,
    QUAS_MAX_HALO, QUAS_MIN_DISK, QUAS_MIN_HALO, QUAS_PREC_RATE, QUAS_PULSE_RATE,
    QUAS_SIM_TIME_PER_CPS, QUAS_SPAWN_RATE_FLOOR, QUAS_SPAWN_RATE_MULT, SPAWN_REMAINDER_CAP,
};

use super::ignition::{ignition_phase, ignition_total_secs, IgnitionPhase};
use super::particles::{
    disk_step, halo_step, infall_step, jet_step, Particle, QuasGeom, QuasarRandom, StepFactors,
};

/// One drawn cell (col, line) — the diff-cleanup currency (same
/// shape as the family's cells).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct QuasCell {
    pub(crate) col: u16,
    pub(crate) line: u16,
}

/// Spawn inputs (mirrors the family bundles — the accumulator
/// contract's parameter set).
pub(crate) struct QuasSpawnParams {
    pub(crate) cols: u16,
    pub(crate) lines: u16,
    /// The density dial's read for the infall target (the calm-sky
    /// feeder dial; the disk and halo targets are set at reset).
    pub(crate) density: f32,
    pub(crate) active_palette_slot: u8,
    pub(crate) spawn_scale: f32,
}

/// Per-frame step inputs (mirrors `MurmStep`/`DnaStep`).
pub(crate) struct QuasStep {
    pub(crate) now: Instant,
    /// chars_per_sec already multiplied by the terminal
    /// speed_mult. Scales the whole engine on one clock (dt_sim)
    /// — the family speed contract: the orbits, the infall, the
    /// jets, the pulse and the ignition all survive the speed
    /// keys together.
    pub(crate) chars_per_sec: f32,
    pub(crate) cols: u16,
    pub(crate) lines: u16,
    pub(crate) max_sim_delta: Duration,
    pub(crate) resume_blend: f32,
}

/// The quasar: the center, the four populations and the clocks —
/// one state machine.
#[derive(Debug)]
pub(crate) struct QuasarRain {
    /// Law 2's disk (built from captures, immortal after).
    pub(crate) disk: Vec<Particle>,
    /// Law 4's jets (both beams in one pool, the first half fires
    /// up, the second down).
    pub(crate) jets: Vec<Particle>,
    /// Law 5's halo (glides in from beyond the frame).
    pub(crate) halo: Vec<Particle>,
    /// Law 3's infall (the fuel — the accumulator's population).
    pub(crate) infall: Vec<Particle>,
    /// The viewport geometry (the center, the axis, the caps).
    pub(crate) geom: QuasGeom,
    /// The ignition clock (sim-seconds since the birth began).
    ignition_t: f32,
    /// The steady flag (the ignition has completed).
    lit: bool,
    /// Whether the jets have fired (the ignition's jet phase or
    /// the fast-forward). `pub(super)`: the draw pass gates the
    /// beam pass on it.
    pub(super) jets_fired: bool,
    /// The core's luminosity pulse phase (radians).
    pulse_phase: f32,
    /// The beams' precession phase (radians). `pub(super)`: the
    /// draw pass projects the helix with it.
    pub(super) prec_phase: f32,
    /// Sim-seconds until the next feed flare.
    flare_clock: f32,
    /// The active flare's age (None while the engine is calm).
    /// `pub(super)`: the draw pass reads it (the infall's surge
    /// read and the halo's flare glow).
    pub(super) flare_age: Option<f32>,
    /// The active knot's position along the beams (both beams
    /// share the event — one heartbeat).
    knot: Option<f32>,
    /// The core's glyph (re-rolled on flare fire — event-gated
    /// mutation, the family contract). `pub(super)`: the draw
    /// pass owns the roll.
    pub(super) core_ch: char,
    /// True when the core's glyph needs a re-roll at the next
    /// draw (first light and every flare fire). `pub(super)`: the
    /// draw pass clears the flag when it rolls.
    pub(super) core_roll: bool,
    /// The active-population counters (the starvation-free
    /// bookkeeping — every activation/deactivation flows through
    /// them). `pub(super)`: the capture economy (capture.rs)
    /// writes them.
    pub(super) disk_active: usize,
    pub(super) infall_active: usize,
    /// Rotating scan cursors for amortized O(1) free-slot search
    /// (mirrors the family). `pub(super)`: the capture economy
    /// (capture.rs) owns the scans.
    pub(super) spawn_scan_idx: usize,
    pub(super) disk_scan_idx: usize,
    /// Global motion clock (dt = now - last_step, clamped by
    /// max_sim_delta and eased by resume_blend — the family
    /// contract; a fully-paused run simply stops advancing).
    last_step: Option<Instant>,
    pub(super) current_cells: Vec<QuasCell>,
    pub(super) previous_cells: Vec<QuasCell>,
    pub(super) drawn_gen: Vec<u32>,
    pub(super) drawn_gen_counter: u32,
    /// Palette slot of the core + glow (one engine, one slot).
    pub(super) field_palette_slot: u8,
    /// Total captures since reset (test-only dial).
    /// `pub(super)`: the capture economy (capture.rs) counts it.
    #[cfg(test)]
    pub(super) absorptions_for_test: usize,
    /// Total flares since the engine lit (test-only dial).
    #[cfg(test)]
    flares_for_test: usize,
}

impl QuasarRain {
    pub(crate) fn new() -> Self {
        // The engine is born unborn: a plain `--scene quasar`
        // startup without a scene transition replays the ignition
        // (the DNA genesis contract).
        Self {
            disk: Vec::new(),
            jets: Vec::new(),
            halo: Vec::new(),
            infall: Vec::new(),
            geom: QuasGeom::for_viewport(1, 1),
            ignition_t: 0.0,
            lit: false,
            jets_fired: false,
            pulse_phase: 0.0,
            prec_phase: 0.0,
            flare_clock: QUAS_FLARE_CLOCK_MEAN,
            flare_age: None,
            knot: None,
            core_ch: '0',
            core_roll: true,
            disk_active: 0,
            infall_active: 0,
            spawn_scan_idx: 0,
            disk_scan_idx: 0,
            last_step: None,
            current_cells: Vec::new(),
            previous_cells: Vec::new(),
            drawn_gen: Vec::new(),
            drawn_gen_counter: 0,
            field_palette_slot: 0,
            #[cfg(test)]
            absorptions_for_test: 0,
            #[cfg(test)]
            flares_for_test: 0,
        }
    }

    /// The steady-state disk population for a viewport + the
    /// density dial (the disk is the brightest surface, so the
    /// dial reads as the disk's ring count).
    pub(crate) fn disk_target(cols: u16, density: f32) -> usize {
        let n = cols.max(1) as f32 * (0.30 + density.clamp(0.05, 3.0) * 0.40);
        (n.round() as usize).clamp(QUAS_MIN_DISK, QUAS_MAX_DISK)
    }

    /// The halo population for a viewport (density-independent —
    /// the host galaxy's old stars are not a dial).
    fn halo_target(cols: u16) -> usize {
        ((cols.max(1) as f32 * 0.14).round() as usize).clamp(QUAS_MIN_HALO, QUAS_MAX_HALO)
    }

    /// The infall target for a viewport + dial, scaled by the
    /// ignition (the thick cold cloud) and the flare surge (the
    /// arriving clump). Hard-capped at 1.6x the steady maximum so
    /// the surge never floods the dirty-cell budget.
    fn infall_target(&self, cols: u16, density: f32) -> usize {
        let base = ((cols.max(1) as f32 * (0.05 + density.clamp(0.05, 3.0) * 0.075)).round()
            as usize)
            .clamp(QUAS_INFALL_MIN, QUAS_INFALL_MAX);
        let mut mult = 1.0;
        if !self.lit {
            // The ignition's thick cloud: while the engine is dark
            // the infall IS the scene (the DNA soup precedent).
            mult *= QUAS_IGNITION_INFALL_MULT;
        }
        if self.flare_age.is_some() {
            // The clump arrives: the rain itself thickens.
            mult *= QUAS_FLARE_SURGE;
        }
        let cap = (QUAS_INFALL_MAX as f32 * 1.6) as usize;
        ((base as f32 * mult).round() as usize).min(cap)
    }

    /// Rebuild the pools for a new viewport (or style entry). The
    /// ignition state survives a pure resize (the burning engine
    /// keeps burning — the pools refill to the steady state when
    /// already lit; the DNA resize contract); `begin_ignition`
    /// re-arms the birth.
    pub(crate) fn reset(&mut self, cols: u16, lines: u16) {
        self.geom = QuasGeom::for_viewport(cols, lines);
        // The disk pool: sized to the target, filled by captures.
        let disk_target = Self::disk_target(cols, 0.60);
        self.disk.clear();
        self.disk.resize_with(disk_target, Particle::vacant);
        self.disk_active = 0;
        self.disk_scan_idx = 0;
        // The jets: both beams, dormant until they fire.
        let jet_total = QUAS_JET_PER_BEAM * 2;
        self.jets.clear();
        self.jets.resize_with(jet_total, Particle::vacant);
        self.jets_fired = false;
        // The halo: the annulus band decides whether a host
        // exists on this viewport.
        let halo_target = if self.geom.halo_max > self.geom.halo_min {
            Self::halo_target(cols)
        } else {
            0
        };
        self.halo.clear();
        self.halo.resize_with(halo_target, Particle::vacant);
        // The golden-angle spread: deterministic orbits, no RNG in
        // the reset (the bench determinism contract).
        const GOLDEN: f32 = 2.399_963_2;
        for (i, p) in self.halo.iter_mut().enumerate() {
            let target = self.geom.halo_min
                + (i as f32 / halo_target.max(1) as f32)
                    * (self.geom.halo_max - self.geom.halo_min);
            p.activate_halo(i as f32 * GOLDEN, target, 0);
        }
        // The infall: the accumulator fills it.
        self.infall.clear();
        self.infall_active = 0;
        self.spawn_scan_idx = 0;
        // The clocks re-arm.
        self.pulse_phase = 0.0;
        self.prec_phase = 0.0;
        self.flare_clock = QUAS_FLARE_CLOCK_MEAN;
        self.flare_age = None;
        self.knot = None;
        self.core_roll = true;
        self.last_step = None;
        #[cfg(test)]
        {
            self.absorptions_for_test = 0;
            self.flares_for_test = 0;
        }
        if self.lit {
            // A resize on the burning engine: the steady state
            // rebuilds immediately (no second ignition).
            self.fill_steady();
        }
        self.clear_draw_history();
    }

    /// Re-arm the ignition (the scene-entry contract: the entry
    /// replays the birth — reset first, then this; the pools the
    /// reset rebuilt wipe back to the dark start).
    pub(crate) fn begin_ignition(&mut self) {
        self.ignition_t = 0.0;
        self.lit = false;
        self.jets_fired = false;
        for p in &mut self.disk {
            p.active = false;
        }
        self.disk_active = 0;
        for p in &mut self.jets {
            p.active = false;
        }
        for p in &mut self.halo {
            p.active = false;
        }
        // The halo re-glides in from beyond the frame (the birth
        // resets the host's fresh orbits — the population stays
        // "all active", the glide-in is the no-pop entry).
        for p in &mut self.halo {
            p.activate_halo(p.theta, p.target_f, self.field_palette_slot);
        }
        for p in &mut self.infall {
            p.active = false;
        }
        self.infall_active = 0;
        self.clear_draw_history();
    }

    /// Fill the engine to the steady state (the bench
    /// fast-forward, and the resize path on an already-lit
    /// engine): the ignition completes, the disk's orbits sit on
    /// their targets (deterministic golden-angle spread), the
    /// beams stream, the halo settles. Deterministic — no RNG
    /// (the bench determinism contract).
    pub(crate) fn fast_forward_ignition(&mut self) {
        self.fill_steady();
    }

    fn fill_steady(&mut self) {
        self.ignition_t = ignition_total_secs() + 1.0;
        self.lit = true;
        self.jets_fired = true;
        const GOLDEN: f32 = 2.399_963_2;
        let n = self.disk.len();
        for (i, p) in self.disk.iter_mut().enumerate() {
            let target = crate::constants::QUAS_DISK_INNER + ((i as f32 * GOLDEN) % 1.0) * 0.70;
            p.activate_disk(i as f32 * GOLDEN, target, self.field_palette_slot);
            // Already circularized: the steady orbit, no glow (the
            // light belongs to the living rain's captures).
            p.f = p.target_f;
            p.charge = 0.0;
        }
        self.disk_active = n;
        for (i, p) in self.jets.iter_mut().enumerate() {
            let side = if i < QUAS_JET_PER_BEAM { 1.0 } else { -1.0 };
            let lane = (i % QUAS_JET_PER_BEAM) as f32 / QUAS_JET_PER_BEAM as f32;
            // The deterministic energy share: the lanes carry the
            // full spread band so the steady beam reads as a
            // stream (no RNG — the bench determinism contract).
            let beta = 0.75 + 0.6 * lane;
            p.activate_jet(side, self.field_palette_slot, lane, beta);
        }
        let halo_n = self.halo.len().max(1);
        for (i, p) in self.halo.iter_mut().enumerate() {
            let target = self.geom.halo_min
                + (i as f32 / halo_n as f32) * (self.geom.halo_max - self.geom.halo_min);
            p.activate_halo(i as f32 * GOLDEN, target, self.field_palette_slot);
            p.f = p.target_f;
        }
    }

    /// The engine's population: the disk, the jets and the fuel.
    /// The halo is deliberately NOT counted — it is the host's
    /// ambient backdrop (present while the style is mounted, like
    /// the sky itself, not machinery): the exit contract reads
    /// zero when the engine disarms, and the halo re-glides on
    /// the next entry.
    pub(crate) fn active_count(&self) -> usize {
        let jets = if self.jets_fired { self.jets.len() } else { 0 };
        self.disk_active + jets + self.infall_active
    }

    /// The ignition phase at the engine's current clock (Steady
    /// once lit — the draw pass's cap queries read this).
    pub(crate) fn phase(&self) -> IgnitionPhase {
        if self.lit {
            IgnitionPhase::Steady
        } else {
            ignition_phase(self.ignition_t)
        }
    }

    /// The luminosity fraction in [0, 1] (law 0's ramp — the
    /// doppler, the halo's breathing and the disk's cap ride it).
    pub(crate) fn luminosity(&self) -> f32 {
        if self.lit {
            1.0
        } else {
            super::ignition::luminosity(self.ignition_t)
        }
    }

    /// The jet front in [0, 1] (law 4's extension during the
    /// birth; 1 once fired).
    pub(crate) fn jet_front(&self) -> f32 {
        if self.jets_fired {
            1.0
        } else {
            super::ignition::jet_front(self.ignition_t)
        }
    }

    /// The core's displayed pulse in [0, 1] (the luminosity
    /// breathing, locked above its peak during the flare window).
    pub(crate) fn pulse_display(&self) -> f32 {
        let pulse = 0.5 + 0.5 * self.pulse_phase.sin();
        match self.flare_age {
            Some(age) => {
                let lock = 1.0 - 0.15 * (age / QUAS_FLARE_WINDOW).min(1.0);
                pulse.max(lock)
            }
            None => pulse,
        }
    }

    /// The active knot's position (both beams share it), if any.
    pub(crate) fn knot(&self) -> Option<f32> {
        self.knot
    }

    /// Palette transition completion: every active particle
    /// adopts the new slot (the family contract; the core + glow
    /// follow the field slot).
    pub(crate) fn adopt_palette_slot(&mut self, palette_slot: u8) {
        self.field_palette_slot = palette_slot;
        for p in self.disk.iter_mut().chain(self.jets.iter_mut()) {
            if p.active {
                p.palette_slot = palette_slot;
            }
        }
        for p in self.halo.iter_mut().chain(self.infall.iter_mut()) {
            if p.active {
                p.palette_slot = palette_slot;
            }
        }
    }

    /// Drop the diff-cleanup history (semantic invalidation /
    /// forced redraw). The next draw pass rebuilds it from an
    /// empty baseline. The engine state itself is simulation
    /// state and is preserved.
    pub(crate) fn clear_draw_history(&mut self) {
        self.current_cells.clear();
        self.previous_cells.clear();
        self.drawn_gen.clear();
        self.drawn_gen_counter = 0;
    }

    /// Spawn pass — the staggered infall entry through the
    /// family's accumulator contract: the budget activates
    /// streamers from the pool's vacant slots at the sky's rim
    /// (rolled angle, rolled spawn radius — partly off-screen, so
    /// the gas enters from beyond the frame). The target breathes
    /// with the ignition (the thick cold cloud) and the flare
    /// surge (the arriving clump); once at target the accumulator
    /// idles.
    pub(crate) fn spawn(
        &mut self,
        elapsed: Duration,
        spawn_remainder: &mut f32,
        params: &QuasSpawnParams,
        random: &mut QuasarRandom<'_>,
    ) {
        if params.cols == 0 || params.lines == 0 || self.geom.infall_max <= self.geom.infall_min {
            *spawn_remainder = 0.0;
            return;
        }

        // The live target: the dial, the ignition's thick cloud
        // and the flare surge (a mid-session density change
        // re-sizes the pool; shrinking drops the inactive tail
        // first, then the excess actives).
        let target = self.infall_target(params.cols, params.density);
        if self.infall.len() != target {
            if target < self.infall.len() {
                let excess = self.infall.len() - target;
                let dropped_active = self
                    .infall
                    .iter()
                    .rev()
                    .take(excess)
                    .filter(|p| p.active)
                    .count();
                self.infall_active = self.infall_active.saturating_sub(dropped_active);
            }
            self.infall.resize(target, Particle::vacant());
            *spawn_remainder = 0.0;
        }

        if target == 0 || self.infall_active >= target {
            *spawn_remainder = (*spawn_remainder).min(SPAWN_REMAINDER_CAP);
            return;
        }

        let deficit = target - self.infall_active;
        let spawn_rate =
            (target as f32 * QUAS_SPAWN_RATE_MULT + QUAS_SPAWN_RATE_FLOOR) * params.spawn_scale;
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

        for _ in 0..to_spawn {
            let Some(idx) = self.find_inactive_infall() else {
                break;
            };
            let roll_a = random.rand_chance.sample(random.rng);
            let roll_b = random.rand_chance.sample(random.rng);
            let f = self.geom.infall_min + roll_a * (self.geom.infall_max - self.geom.infall_min);
            let theta = roll_b * std::f32::consts::TAU;
            self.infall[idx].activate_infall(f, theta, params.active_palette_slot);
            // The glyph is picked at the first draw pass (the
            // lorenz contract).
            self.infall_active += 1;
        }
    }

    /// Motion pass — the engine's physics core.
    ///
    /// 1. The ignition clock advances (the birth rides sim-time —
    ///    the family speed contract; on completion the lit flag
    ///    flips).
    /// 2. The jets fire once the ignition's jet phase begins (the
    ///    burst — the event pop that IS the drama, the predator
    ///    flash's heir).
    /// 3. The clocks: the core pulse, the beam precession, the
    ///    knot's climb, the flare window's age, the feed-flare
    ///    clock (a fire re-arms it, locks the pulse, launches the
    ///    knot and re-rolls the core's glyph).
    /// 4. The physics: the halo's glide, the disk's Kepler steps,
    ///    the jets' accelerating ride, the infall's spiral plunge
    ///    — and every absorption runs the capture economy (the
    ///    disk grows from the rain, or the nearest orbit
    ///    re-charges).
    pub(crate) fn advance(&mut self, step: &QuasStep, random: &mut QuasarRandom<'_>) {
        // The degenerate-viewport freeze: a zero-size viewport
        // holds the engine (nothing to orbit, nothing to draw —
        // the spawn guard's advance-side sibling).
        if step.cols == 0 || step.lines == 0 {
            self.last_step = Some(step.now);
            return;
        }
        if self.disk.is_empty()
            && self.jets.is_empty()
            && self.halo.is_empty()
            && self.infall.is_empty()
        {
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

        // The family speed contract: one sim clock for the orbits,
        // the rain, the beams, the pulse and the ignition alike.
        let dt_sim = dt_wall * step.chars_per_sec.max(0.0) * QUAS_SIM_TIME_PER_CPS;
        // The frame's decay factors (NIGHT-lts-1 stage 1): every
        // particle steps on the same dt_sim, so the three exp()
        // evaluations happen once here instead of per particle.
        let fx = StepFactors::for_dt(dt_sim);

        // 1. The ignition clock.
        if !self.lit {
            self.ignition_t += dt_sim;
            if self.ignition_t >= ignition_total_secs() {
                self.lit = true;
            }
        }

        // 2. The jets fire (the ignition's jet phase, or the
        //    resize fast-forward has already fired them).
        let front = self.jet_front();
        if front > 0.0 && !self.jets_fired && !self.jets.is_empty() {
            for (i, p) in self.jets.iter_mut().enumerate() {
                let side = if i < QUAS_JET_PER_BEAM { 1.0 } else { -1.0 };
                // The burst: the full stream launches at once with
                // a rolled energy share per particle (the spread
                // keeps the beam from riding in lockstep) and a
                // lane offset so the beam reads as a stream from
                // the first frame (clipped by the pushing front).
                let roll = random.rand_chance.sample(random.rng);
                let lane = (i % QUAS_JET_PER_BEAM) as f32 / QUAS_JET_PER_BEAM as f32;
                let beta = 0.75 + 0.6 * roll;
                p.activate_jet(side, self.field_palette_slot, lane * front, beta);
            }
            self.jets_fired = true;
        }

        // 3. The clocks.
        self.pulse_phase += QUAS_PULSE_RATE * dt_sim;
        if self.pulse_phase > std::f32::consts::TAU * 64.0 {
            self.pulse_phase = self.pulse_phase.rem_euclid(std::f32::consts::TAU);
        }
        self.prec_phase += QUAS_PREC_RATE * dt_sim;
        if self.prec_phase > std::f32::consts::TAU * 64.0 {
            self.prec_phase = self.prec_phase.rem_euclid(std::f32::consts::TAU);
        }
        if let Some(age) = &mut self.flare_age {
            *age += dt_sim;
            if *age >= QUAS_FLARE_WINDOW {
                self.flare_age = None;
            }
        }
        if let Some(k) = &mut self.knot {
            *k += QUAS_KNOT_V * dt_sim;
            if *k > 1.0 + QUAS_KNOT_W {
                self.knot = None;
            }
        }
        if self.lit {
            // The feed-flare clock (the drama event — only a
            // burning engine flares).
            self.flare_clock -= dt_sim;
            if self.flare_clock <= 0.0 {
                self.flare_clock =
                    QUAS_FLARE_CLOCK_MEAN * (0.6 + random.rand_chance.sample(random.rng) * 0.8);
                self.flare_age = Some(0.0);
                self.knot = Some(0.0);
                self.core_roll = true;
                #[cfg(test)]
                {
                    self.flares_for_test += 1;
                }
            }
        }

        // 4. The physics.
        for p in &mut self.halo {
            if p.active {
                halo_step(p, dt_sim, fx);
            }
        }
        for p in &mut self.disk {
            if p.active {
                disk_step(p, dt_sim, fx);
            }
        }
        if self.jets_fired {
            for p in &mut self.jets {
                jet_step(p, dt_sim, front);
            }
        }
        for i in 0..self.infall.len() {
            if !self.infall[i].active {
                continue;
            }
            if infall_step(&mut self.infall[i], dt_sim) {
                self.infall_active = self.infall_active.saturating_sub(1);
                let theta = self.infall[i].theta;
                self.run_capture(theta, random);
            }
        }
    }

    // -- Test-only diagnostics (mirrors the family *_for_test API) --

    #[cfg(test)]
    pub(crate) fn disk_states_for_test(&self) -> Vec<(f32, f32, f32)> {
        self.disk
            .iter()
            .filter(|p| p.active)
            .map(|p| (p.f, p.theta, p.charge))
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn jet_states_for_test(&self) -> Vec<(f32, f32)> {
        self.jets
            .iter()
            .filter(|p| p.active)
            .map(|p| (p.s, p.side))
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn infall_states_for_test(&self) -> Vec<(f32, f32)> {
        self.infall
            .iter()
            .filter(|p| p.active)
            .map(|p| (p.f, p.theta))
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn ignition_t_for_test(&self) -> f32 {
        self.ignition_t
    }

    #[cfg(test)]
    pub(crate) fn lit_for_test(&self) -> bool {
        self.lit
    }

    #[cfg(test)]
    pub(crate) fn flares_for_test(&self) -> usize {
        self.flares_for_test
    }

    #[cfg(test)]
    pub(crate) fn absorptions_for_test(&self) -> usize {
        self.absorptions_for_test
    }

    #[cfg(test)]
    pub(crate) fn knot_for_test(&self) -> Option<f32> {
        self.knot
    }

    #[cfg(test)]
    pub(crate) fn disk_active_for_test(&self) -> usize {
        self.disk_active
    }

    #[cfg(test)]
    pub(crate) fn infall_active_for_test(&self) -> usize {
        self.infall_active
    }

    /// Plant the flare clock (the flare tests' deterministic
    /// arm).
    #[cfg(test)]
    pub(crate) fn arm_flare_for_test(&mut self, clock: f32) {
        self.flare_clock = clock;
    }

    #[cfg(test)]
    pub(crate) fn pulse_for_test(&self) -> f32 {
        self.pulse_display()
    }

    #[cfg(test)]
    pub(crate) fn drawn_cells_for_test(&self) -> &[QuasCell] {
        &self.current_cells
    }
}
