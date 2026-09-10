// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The particle and the engine's geometry — the physics half of
//! the quasar style (NIGHT-research-8).
//!
//! This module owns the four populations' shared particle state,
//! the viewport geometry (the center, the disk axis, the jet
//! length, the annulus caps — every cap keeps a projected
//! position inside the viewport by construction), and the
//! per-kind physics steps (laws 1-5 executable form). The full
//! derivation essay lives in `type_rain/quasar/mod.rs`; the
//! engine state and the per-frame orchestration live in
//! `quasar.rs`.
//!
//! The populations (one `Vec` each on `QuasarRain`, the black
//! hole's per-subsystem precedent — each population spawns,
//! steps and reads independently):
//! - Disk: Kepler orbits, fraction f in [DISK_INNER, 1] of the
//!   axis, exponential circularization onto a rolled target,
//!   exponential charge decay (the fresh-feed light).
//! - Jets: s in [0, 1] along the beam with the accelerating
//!   speed v(s); wraps at the beam end (recycling stream).
//! - Halo: slow flat annulus orbits, gliding in from just
//!   outside the frame at entry (the no-pop birth).
//! - Infall: the fuel — streamers spiraling inward on the
//!   accelerating plunge, absorbed at the disk's outer edge.

use crate::constants::{
    QUAS_DISK_INNER, QUAS_DISK_TILT, QUAS_INFALL_RATE, QUAS_INFALL_SPIN, QUAS_JET_ACC, QUAS_JET_V0,
};

use rand::{distr::Uniform, rngs::StdRng};

/// Per-frame integration factors (NIGHT-lts-1 stage 1): the
/// exponential decay terms of the disk and halo steps depend only
/// on the frame's shared sim dt — every particle of a frame steps
/// on the same dt, so the advance pass evaluates the three exp()
/// calls ONCE per frame and threads this `Copy` snapshot through
/// every particle's step. The per-particle steps used to
/// re-evaluate two exp() per disk particle and one per halo rider
/// per frame for three values that are identical across the pools
/// (hundreds of redundant transcendentals per frame at bench
/// populations).
#[derive(Clone, Copy, Debug)]
pub(crate) struct StepFactors {
    /// Disk circularization damping (1 - exp(-dt / QUAS_CIRC_TAU)).
    pub(crate) circ_damp: f32,
    /// Fresh-feed charge decay (exp(-dt / QUAS_CHARGE_TAU)).
    pub(crate) charge_decay: f32,
    /// Halo circularization damping (1 - exp(-dt / QUAS_HALO_CIRC_TAU)).
    pub(crate) halo_damp: f32,
}

impl StepFactors {
    /// Evaluate the frame's decay factors for the shared sim dt.
    #[must_use]
    pub(crate) fn for_dt(dt: f32) -> Self {
        Self {
            circ_damp: 1.0 - (-dt / crate::constants::QUAS_CIRC_TAU).exp(),
            charge_decay: (-dt / crate::constants::QUAS_CHARGE_TAU).exp(),
            halo_damp: 1.0 - (-dt / crate::constants::QUAS_HALO_CIRC_TAU).exp(),
        }
    }
}

/// RNG bundle (the advance pass is a stochastic pass in the family
/// sense: the infall spawn rolls, the flare clock re-arms and the
/// capture charges ride along).
pub(crate) struct QuasarRandom<'a> {
    pub(crate) rng: &'a mut StdRng,
    pub(crate) rand_chance: &'a Uniform<f32>,
}

/// Which population a particle belongs to (fixed at activation).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParticleKind {
    /// Law 2: the Keplerian accretion disk.
    Disk,
    /// Law 4: the polar beams.
    Jet,
    /// Law 5: the host halo annulus.
    Halo,
    /// Law 3: the infalling fuel.
    Infall,
}

/// One particle: a glyph carrier bound to the engine's center —
/// no free particle exists in this style (the quasar's gravity is
/// the whole scene). The kind picks the physics; unused fields
/// read zero (the family's flat-struct precedent).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Particle {
    pub(crate) active: bool,
    pub(crate) kind: ParticleKind,
    /// Disk/halo/infall: the orbit radius as a fraction of the
    /// disk axis (disk: [DISK_INNER, 1]; halo: the annulus band;
    /// infall: from spawn down to the absorption edge).
    pub(crate) f: f32,
    /// Disk/halo/infall: the orbit angle (radians, y grows
    /// downward — sin > 0 reads below the center).
    pub(crate) theta: f32,
    /// Disk: the circularization target (the rolled orbit the
    /// capture eases onto). Halo: the annulus target the glide-in
    /// settles to.
    pub(crate) target_f: f32,
    /// Jet: the position along the beam, [0, 1] (0 = the core,
    /// 1 = the tip).
    pub(crate) s: f32,
    /// Jet: the beam sign (+1 fires up, -1 fires down).
    pub(crate) side: f32,
    /// Jet: the particle's speed multiplier (the beam's energy
    /// spread, rolled at activation in [0.75, 1.35] — without it
    /// every particle rides the same v(s) law in lockstep and the
    /// beam collapses to a single transiting blob; the spread is
    /// what makes the beam read as a STREAM).
    pub(crate) beta: f32,
    /// Disk: the fresh-feed charge in [0, 1] (decays
    /// exponentially; the rung boost's input).
    pub(crate) charge: f32,
    /// Glyph carried by the particle; re-rolled matrix-style when
    /// the head crosses into a new cell (the family shimmer).
    pub(crate) ch: char,
    /// Palette slot adopted at spawn / palette transition.
    pub(crate) palette_slot: u8,
    /// Sim-seconds since activation.
    pub(crate) age: f32,
    /// The last-drawn cell (the shimmer gate's memory; doubles as
    /// the infall's one-deep wake cell); u16::MAX when none.
    pub(crate) trail_col: u16,
    pub(crate) trail_line: u16,
}

impl Particle {
    pub(crate) const fn vacant() -> Self {
        Self {
            active: false,
            kind: ParticleKind::Disk,
            f: 0.0,
            theta: 0.0,
            target_f: 0.0,
            s: 0.0,
            side: 1.0,
            beta: 1.0,
            charge: 0.0,
            ch: '0',
            palette_slot: 0,
            age: 0.0,
            trail_col: u16::MAX,
            trail_line: 0,
        }
    }

    /// Activate a disk particle at the capture edge (f = 1, the
    /// outer rim) easing onto `target_f` — the capture births a
    /// charged orbit (the fresh-write economy).
    pub(crate) fn activate_disk(&mut self, theta: f32, target_f: f32, palette_slot: u8) {
        self.active = true;
        self.kind = ParticleKind::Disk;
        self.f = 1.0;
        self.theta = theta;
        self.target_f = target_f.clamp(QUAS_DISK_INNER, 1.0);
        self.s = 0.0;
        self.side = 1.0;
        self.charge = 1.0;
        self.palette_slot = palette_slot;
        self.age = 0.0;
    }

    /// Re-charge a disk particle (the steady-state capture: the
    /// fuel lands on the nearest orbit and the light shows it).
    pub(crate) fn feed_disk(&mut self) {
        self.charge = 1.0;
    }

    /// Activate a jet particle at the core (s = 0) on `side`'s
    /// beam with its energy share — the stream is born at the
    /// engine.
    pub(crate) fn activate_jet(&mut self, side: f32, palette_slot: u8, s: f32, beta: f32) {
        self.active = true;
        self.kind = ParticleKind::Jet;
        self.f = 0.0;
        self.theta = 0.0;
        self.target_f = 0.0;
        self.s = s.clamp(0.0, 1.0);
        self.side = if side >= 0.0 { 1.0 } else { -1.0 };
        self.beta = beta.clamp(0.5, 1.5);
        self.charge = 0.0;
        self.palette_slot = palette_slot;
        self.age = 0.0;
    }

    /// Activate a halo particle on the glide-in orbit (just
    /// outside the annulus, damping to its target — the halo
    /// births itself from beyond the frame).
    pub(crate) fn activate_halo(&mut self, theta: f32, target_f: f32, palette_slot: u8) {
        self.active = true;
        self.kind = ParticleKind::Halo;
        self.f = (target_f + 0.55).max(target_f);
        self.theta = theta;
        self.target_f = target_f;
        self.s = 0.0;
        self.side = 1.0;
        self.charge = 0.0;
        self.palette_slot = palette_slot;
        self.age = 0.0;
    }

    /// Activate an infall streamer at (f, theta) — the fuel
    /// falling out of the dark.
    pub(crate) fn activate_infall(&mut self, f: f32, theta: f32, palette_slot: u8) {
        self.active = true;
        self.kind = ParticleKind::Infall;
        self.f = f;
        self.theta = theta;
        self.target_f = 0.0;
        self.s = 0.0;
        self.side = 1.0;
        self.charge = 0.0;
        self.palette_slot = palette_slot;
        self.age = 0.0;
        self.trail_col = u16::MAX;
    }

    /// The infall comet's previous cell, if any (the rain
    /// streak's dimmer half).
    pub(crate) fn trail_cell(&self) -> Option<(u16, u16)> {
        if self.trail_col == u16::MAX {
            None
        } else {
            Some((self.trail_col, self.trail_line))
        }
    }

    /// Push a new head cell into the one-deep trail.
    pub(crate) fn set_trail(&mut self, col: u16, line: u16) {
        self.trail_col = col;
        self.trail_line = line;
    }
}

/// The engine geometry for one viewport — every cap keeps the
/// projected positions inside the frame by construction (the
/// degenerate-terminal contract: a 1x1 viewport produces a
/// collapsed but valid engine, never a panic).
#[derive(Clone, Copy, Debug)]
pub(crate) struct QuasGeom {
    /// The engine's center column (fractional).
    pub(crate) cx: f32,
    /// The engine's center line (fractional, 44% down the frame —
    /// the composition leaves the sky its share).
    pub(crate) cy: f32,
    /// The disk semi-major axis in cells (the orbit fractions
    /// scale this).
    pub(crate) axis: f32,
    /// The jet length in cells (per beam, capped by the polar
    /// clearance).
    pub(crate) jet_len: f32,
    /// The halo annulus band [min, max] as axis fractions (empty
    /// on viewports too small to carry a halo).
    pub(crate) halo_min: f32,
    pub(crate) halo_max: f32,
    /// The infall spawn band [min, max] as axis fractions (empty
    /// on degenerate viewports — the fuel appears when the sky
    /// has room for it).
    pub(crate) infall_min: f32,
    pub(crate) infall_max: f32,
}

/// The halo's vertical roundness multiplier (the host is a thick
/// disk seen at the engine's inclination — rounder than the
/// accretion disk, flatter than a sphere). Shared with the draw
/// pass: the paint and the orbit must agree on the halo's shape.
pub(crate) const HALO_ROUND: f32 = 1.35;

impl QuasGeom {
    /// Derive the engine geometry for a viewport.
    pub(crate) fn for_viewport(cols: u16, lines: u16) -> Self {
        let w = cols.max(1) as f32 - 1.0;
        let h = lines.max(1) as f32 - 1.0;
        let cx = w * 0.5;
        let cy = h * 0.44;
        // The polar clearance: the vertical room the jets and the
        // tilt-projected disk must share (the core sits at 44%, so
        // the up-clearance is the tighter one on tall frames).
        let v_clear = cy.min(h - cy).max(0.0);
        // The disk axis: 46% of the half-width, and no wider than
        // the tilt-projected height admits.
        let axis_h = cx * 0.46;
        let axis_v = v_clear / QUAS_DISK_TILT * 0.96;
        let axis = axis_h.min(axis_v).max(0.5);
        let jet_len = (v_clear * 0.92).max(0.0);
        // The annulus caps: the halo and the infall stay on-screen
        // horizontally and inside the (rounder) vertical band.
        let f_x = if axis > 1e-6 { cx / axis } else { 0.0 };
        let v_cap_f = if axis > 1e-6 {
            v_clear / (axis * QUAS_DISK_TILT)
        } else {
            0.0
        };
        let halo_max = 1.8f32.min(f_x * 0.98).min(v_cap_f / HALO_ROUND * 0.98);
        let halo_min = 1.12;
        let (halo_min, halo_max) = if halo_max > halo_min + 0.08 {
            (halo_min, halo_max)
        } else {
            // Degenerate sky: no halo (the engine reads on the
            // disk and jets alone).
            (0.0, -1.0)
        };
        let inf_max = (halo_max + 0.75).min(f_x * 1.04).min(v_cap_f * 1.02);
        let inf_min = halo_max + 0.12;
        let (infall_min, infall_max) = if inf_max > inf_min + 0.08 {
            (inf_min, inf_max)
        } else {
            (0.0, -1.0)
        };
        Self {
            cx,
            cy,
            axis,
            jet_len,
            halo_min,
            halo_max,
            infall_min,
            infall_max,
        }
    }

    /// Project a disk/halo/infall orbit point (f, theta) to cell
    /// coordinates. The halo's vertical roundness widens its
    /// ellipse (a different body, the same inclination).
    pub(crate) fn project(&self, f: f32, theta: f32, roundness: f32) -> (f32, f32) {
        let (sin, cos) = theta.sin_cos();
        (
            self.cx + self.axis * f * cos,
            self.cy + self.axis * f * sin * QUAS_DISK_TILT * roundness,
        )
    }

    /// Project a jet particle at (s, side) with the precessing
    /// helix offset: the beam rides s along the polar axis while
    /// the particle circles the precessing phase.
    pub(crate) fn project_jet(
        &self,
        s: f32,
        side: f32,
        prec_phase: f32,
        helix_amp: f32,
    ) -> (f32, f32) {
        let k = std::f32::consts::TAU * crate::constants::QUAS_HELIX_TURNS;
        let x = self.cx + helix_amp * s * (prec_phase + s * k).sin();
        let y = self.cy - side * s * self.jet_len;
        (x, y)
    }
}

/// Kepler's third law on the orbit fraction: the inner edge laps
/// the outer ~6x (the shear IS the rotation read). Bounded by
/// construction — f is clamped to [DISK_INNER, 1] at every use.
pub(crate) fn disk_omega(f: f32) -> f32 {
    let f = f.clamp(QUAS_DISK_INNER, 1.0);
    crate::constants::QUAS_KEPLER_K / (f * f.sqrt())
}

/// The halo's slow orbit rate (a touch of shear — the outer halo
/// lags the inner).
pub(crate) fn halo_omega(f: f32) -> f32 {
    0.05 / f.clamp(0.4, 4.0)
}

/// The line-of-sight velocity sign/magnitude for a disk orbit at
/// `theta` in [-1, 1]: positive = the approaching limb (beamed
/// brighter), negative = receding (dimmed). For the counterclockwise
/// advance (omega > 0) the top limb approaches — the side the
/// jets fire from reads first.
pub(crate) fn los_of(theta: f32) -> f32 {
    -theta.sin()
}

/// The jet speed law v(s) = V0 (1 + ACC s) — the tip outraces the
/// collar (the relativistic read).
pub(crate) fn jet_speed(s: f32) -> f32 {
    QUAS_JET_V0 * (1.0 + QUAS_JET_ACC * s.clamp(0.0, 1.0))
}

/// The infall plunge rate at fraction f: the base rate scaled by
/// the free-fall acceleration (1/f^2 — the plunge accelerates as
/// the gravity tightens, a streamer never stalls at the rim).
pub(crate) fn infall_rate(f: f32) -> f32 {
    let f = f.clamp(0.25, 4.0);
    QUAS_INFALL_RATE * (1.0 + 1.1 / (f * f))
}

/// The infall spiral wind at fraction f (radians per sim-second —
/// the streamer coils as it falls).
pub(crate) fn infall_spin(f: f32) -> f32 {
    QUAS_INFALL_SPIN / f.clamp(0.25, 4.0) * 1.2
}

/// Law 2's disk step: the orbit advances on Kepler's omega, the
/// capture eases onto the target orbit (exponential
/// circularization), the feed charge decays, the age accrues.
/// The angle wraps at 64 turns (f32 precision kept); f never
/// leaves [DISK_INNER, 1] (the damping only moves it toward its
/// clamped target). The exponential decay terms arrive
/// precomputed in `fx` (NIGHT-lts-1 stage 1) — they depend only on
/// the frame's shared dt, not on the particle.
pub(crate) fn disk_step(p: &mut Particle, dt: f32, fx: StepFactors) {
    let omega = disk_omega(p.f);
    p.theta += omega * dt;
    if p.theta > std::f32::consts::TAU * 64.0 {
        p.theta = p.theta.rem_euclid(std::f32::consts::TAU);
    }
    // Circularization: exponential ease onto the target orbit.
    p.f += (p.target_f - p.f) * fx.circ_damp;
    // The fresh-feed light decays.
    p.charge *= fx.charge_decay;
    p.age += dt;
}

/// Law 5's halo step: the slow orbit advances, the glide-in
/// eases onto the annulus target. The damping term arrives
/// precomputed in `fx` (NIGHT-lts-1 stage 1) — it depends only on
/// the frame's shared dt, not on the rider.
pub(crate) fn halo_step(p: &mut Particle, dt: f32, fx: StepFactors) {
    p.theta += halo_omega(p.f) * dt;
    if p.theta > std::f32::consts::TAU * 64.0 {
        p.theta = p.theta.rem_euclid(std::f32::consts::TAU);
    }
    p.f += (p.target_f - p.f) * fx.halo_damp;
    p.age += dt;
}

/// Law 3's infall step: the plunge accelerates inward, the spiral
/// winds, and the streamer absorbs when it reaches the disk's
/// outer edge (returns true — the caller runs the capture
/// economy; the particle deactivates).
pub(crate) fn infall_step(p: &mut Particle, dt: f32) -> bool {
    p.f -= infall_rate(p.f) * dt;
    p.theta += infall_spin(p.f) * dt;
    p.age += dt;
    if p.f <= 1.0 {
        p.active = false;
        return true;
    }
    false
}

/// Law 4's jet step: s advances on the accelerating speed law
/// scaled by the particle's energy share (the beam's spread —
/// same-law particles would ride in lockstep and collapse the
/// beam to a blob), and wraps at the beam's end (the recycling
/// stream — a beam is a flow, not a track). During the ignition
/// the wrap clips at the extending front (the stream piles into
/// the pushing tip).
pub(crate) fn jet_step(p: &mut Particle, dt: f32, front: f32) {
    p.s += jet_speed(p.s) * p.beta * dt;
    let end = front.clamp(0.02, 1.0);
    if p.s >= end {
        p.s = 0.0;
    }
    p.age += dt;
}
