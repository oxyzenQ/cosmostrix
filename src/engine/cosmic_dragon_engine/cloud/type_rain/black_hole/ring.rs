// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Black hole orbital ring (NIGHT-special-1 stage 2): the physics
//! half of the ring, split from `black_hole.rs` the way the
//! monolith/dragon families split their helpers (the ball geometry,
//! the mote pool bookkeeping and the diff-cleanup orchestration stay
//! in the main file; the per-mote math lives here).
//!
//! Motion DNA — Keplerian mean orbit laminated with RK4 Lorenz
//! turbulence: each mote is a glyph carried around the ball on a
//! tilted ellipse. The mean motion is a circular orbit whose angular
//! rate follows Kepler's third law (omega scales with the mote's
//! current wobbled radius to the minus three-halves) — motes wobbled
//! inward visibly outpace ones wobbled outward, the differential
//! rotation of a real accretion disk. Superposed on that mean flow,
//! the canonical Lorenz attractor (sigma 10, rho 28, beta 8/3 — the
//! same system the lorenz style renders, the foundational chaotic
//! ODE of nonlinear dynamics) is integrated per mote with classical
//! fourth-order Runge-Kutta: the attractor's radial coordinate
//! wobbles the orbital radius, its z coordinate displaces the mote
//! out of the ring plane and grades the glyph brightness through the
//! shared z ladder. The result reads as a turbulent plasma stream
//! orbiting the hole — not a rigid hoop, not a chaotic scribble:
//! chaos laminated onto an orbit.
//!
//! Occlusion (the 3D read): the near side of the disk passes in
//! FRONT of the hole — its cells always draw, even across the event
//! horizon (the crossing read) and even when the z-tilt breathes a
//! near-side mote above the equator. The far side passes BEHIND the
//! hole — any far-side cell inside the ball silhouette is skipped,
//! at any height (the lensing re-projection carries the visible far
//! side onto the halo arc outside the silhouette, so nothing of the
//! far side is lost, only the physically-hidden segment).
//! Stage 2.3 note: the previous screen-height discriminator (hide
//! above the viewport center) also ate near-side cells the z-tilt
//! lifted above the equator — one of the two reasons the crossing
//! line read below the shadow's middle (owner's 9.5/10 feedback).
//! The side is a property of the orbit (the sign of sin phi), known
//! exactly, so the rule now keys on it instead of the screen y.
//!
//! Gravitational lensing (stage 2.1, owner visual feedback): the
//! far side of the disk does not hide flat behind the hole — light
//! from behind bends over the top of the shadow. Far-side motes are
//! re-projected onto a halo arc (radius a multiple of the ball)
//! whose apex sits just above the photon ring, smoothly blended into
//! the flat ellipse at the disk's left and right extremes: the
//! stream climbs from the limb, vanishes briefly behind the shadow,
//! re-emerges on the upper arc, and sails over the top — the
//! iconic lensed halo of every real black-hole image.
//!
//! Equatorial crossing + disk brightness profile (stage 2.3, owner
//! 9.5/10 feedback, the Gargantua reference): the near side's sine
//! is squashed to half the minor axis so the crossing band hugs the
//! vertical middle of the core (the Interstellar line crosses the
//! shadow's equator, not its lower half), and the disk carries a
//! radial brightness profile at draw time — one rung up inside the
//! inner zone (the hot plasma across the shadow, the "solid white
//! line"), one to three rungs down past the fade start (the line's
//! ends dissolve into a few dim particles, the smooth transition of
//! the reference imagery).
//!
//! Entry spiral (the accretion read): freshly spawned motes carry
//! an entry radius excess that decays exponentially — material
//! drifts in from beyond the disk and settles onto the ring, never
//! popping in on the orbit. The same read serves the steady-state
//! respawn and the formation intro's accretion phase.
//!
//! Family contracts honored: pool = one mote per column (lane
//! model), deficit-bounded spawn accumulator with fractional
//! remainder, lifetime absorption with per-mote variance,
//! motion-gated matrix shimmer, comet trail with the dimming ladder,
//! palette-slot adoption, three-pass diff cleanup (driven from the
//! ball file's draw pass).

use std::time::{Duration, Instant};

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use super::super::monolith::BrightnessLevel;
use super::ball_helpers::bump_level;
use super::black_hole::CELL_ASPECT_DIVISOR;

/// One orbital mote: a glyph riding a Keplerian ring whose turbulence
/// is the RK4-integrated Lorenz state (x, y, z). Same field set as
/// `LorenzMote` plus the orbital angle — the struct stays
/// plain-old-data so the pool is one flat Vec (cache-friendly, no
/// per-frame allocation).
#[derive(Clone, Copy, Debug)]
pub(crate) struct RingMote {
    pub(crate) active: bool,
    /// Orbital angle around the ball (radians, unbounded — read
    /// through sin/cos so no wrapping bookkeeping is needed).
    pub(crate) phi: f32,
    /// Lorenz state vector — the turbulence source. Seeded near the
    /// textbook initial condition (±1, 1, 1) with per-mote
    /// perturbation, exactly like the lorenz style's motes.
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) z: f32,
    /// Simulation age in seconds (drives absorption).
    pub(crate) sim_age: f32,
    /// Per-mote lifetime cap (±15% variance at spawn).
    pub(crate) lifetime: f32,
    /// Per-mote pace multiplier (0.85..1.15) so even identically
    /// seeded motes drift apart in phase over time.
    pub(crate) pace: f32,
    /// Glyph carried by the mote; re-rolled matrix-style when the
    /// head crosses into a new cell (mutation tied to motion).
    pub(crate) ch: char,
    /// Palette slot adopted at spawn / palette transition.
    pub(crate) palette_slot: u8,
    /// Ring buffer of the last head cell positions (oldest first).
    pub(crate) trail: [(u16, u16); crate::constants::BLACK_HOLE_RING_TRAIL_LEN],
    pub(crate) trail_len: u8,
}

impl RingMote {
    pub(crate) const fn vacant() -> Self {
        Self {
            active: false,
            phi: 0.0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            sim_age: 0.0,
            lifetime: 0.0,
            pace: 1.0,
            ch: '0',
            palette_slot: 0,
            trail: [(0, 0); crate::constants::BLACK_HOLE_RING_TRAIL_LEN],
            trail_len: 0,
        }
    }

    /// Shift-left ring-buffer push (mirrors `LorenzMote::push_trail`):
    /// drop the oldest position, append the newest at the tail.
    pub(crate) fn push_trail(&mut self, col: u16, line: u16) {
        let len = crate::constants::BLACK_HOLE_RING_TRAIL_LEN;
        if self.trail_len as usize >= len {
            for i in 0..len - 1 {
                self.trail[i] = self.trail[i + 1];
            }
            self.trail[len - 1] = (col, line);
        } else {
            let idx = self.trail_len as usize;
            self.trail[idx] = (col, line);
            self.trail_len += 1;
        }
    }
}

/// Spawn inputs (mirrors `LorenzSpawnParams` — the bundle keeps
/// clippy's too-many-arguments threshold respected at the call site).
pub(crate) struct BlackHoleSpawnParams {
    pub(crate) cols: u16,
    pub(crate) lines: u16,
    pub(crate) density: f32,
    pub(crate) active_palette_slot: u8,
    pub(crate) spawn_scale: f32,
}

/// RNG bundle (mirrors `LorenzRandom`).
pub(crate) struct BlackHoleRandom<'a> {
    pub(crate) rng: &'a mut StdRng,
    pub(crate) rand_chance: &'a Uniform<f32>,
}

/// Per-frame step inputs for the advance pass (mirrors `LorenzStep`):
/// time + speed only — the projection derives viewport geometry from
/// the cached ball anchor at draw time.
pub(crate) struct BlackHoleStep {
    pub(crate) now: Instant,
    /// chars_per_sec already multiplied by the terminal speed_mult.
    /// Scales both the RK4 attractor time and the Keplerian angular
    /// rate, so the up/down speed keys feel native on the orbit.
    pub(crate) chars_per_sec: f32,
    pub(crate) max_sim_delta: Duration,
    pub(crate) resume_blend: f32,
}

/// Activate a vacant mote: a uniform random orbital phase (spawns
/// spread around the full circumference from the first frame — no
/// clumping), the Lorenz state seeded at the textbook (±1, 1, 1)
/// initial condition (the sign selects the lobe, parity with the
/// lorenz style's spawn so the wobble distribution starts balanced),
/// and per-mote pace / lifetime variance so no two motes march in
/// lockstep.
pub(crate) fn activate_ring_mote(
    m: &mut RingMote,
    lobe_sign: f32,
    palette_slot: u8,
    rand_chance: &Uniform<f32>,
    rng: &mut StdRng,
) {
    m.active = true;
    m.phi = rand_chance.sample(rng) * std::f32::consts::TAU;

    let perturb = crate::constants::LORENZ_SPAWN_PERTURB;
    m.x = lobe_sign + (rand_chance.sample(rng) - 0.5) * 2.0 * perturb;
    m.y = 1.0 + (rand_chance.sample(rng) - 0.5) * 2.0 * perturb;
    m.z = 1.0 + (rand_chance.sample(rng) - 0.5) * 2.0 * perturb;

    m.sim_age = 0.0;
    m.lifetime =
        crate::constants::BLACK_HOLE_RING_MAX_AGE_SECS * (0.85 + rand_chance.sample(rng) * 0.30);
    m.pace = 0.85 + rand_chance.sample(rng) * 0.30;
    m.palette_slot = palette_slot;
    m.trail_len = 0;
}

/// Advance one mote by one frame: a single RK4 step of the canonical
/// Lorenz system (the integrator core is identical to the lorenz
/// style's — sigma/rho/beta come from the shared LORENZ_ constants
/// because the attractor is the same; only what the state drives
/// differs), then the Keplerian orbital advance. The angular rate is
/// modulated by the mote's CURRENT wobbled radius — the same
/// turbulence that moves the mote radially also speeds it up and
/// slows it down, which is the shear signature of a real disk.
/// Returns true when the mote was absorbed (lifetime reached).
pub(crate) fn advance_ring_mote(
    m: &mut RingMote,
    dt_wall: f32,
    dt_lorenz_base: f32,
    omega_base: f32,
) -> bool {
    let dt = dt_lorenz_base * m.pace;
    let sigma = crate::constants::LORENZ_SIGMA;
    let rho = crate::constants::LORENZ_RHO;
    let beta = crate::constants::LORENZ_BETA;

    // RK4 step (classical 4th-order Runge-Kutta).
    let (k1x, k1y, k1z) = lorenz_deriv(m.x, m.y, m.z, sigma, rho, beta);
    let (k2x, k2y, k2z) = lorenz_deriv(
        m.x + 0.5 * dt * k1x,
        m.y + 0.5 * dt * k1y,
        m.z + 0.5 * dt * k1z,
        sigma,
        rho,
        beta,
    );
    let (k3x, k3y, k3z) = lorenz_deriv(
        m.x + 0.5 * dt * k2x,
        m.y + 0.5 * dt * k2y,
        m.z + 0.5 * dt * k2z,
        sigma,
        rho,
        beta,
    );
    let (k4x, k4y, k4z) = lorenz_deriv(
        m.x + dt * k3x,
        m.y + dt * k3y,
        m.z + dt * k3z,
        sigma,
        rho,
        beta,
    );
    m.x += (dt / 6.0) * (k1x + 2.0 * k2x + 2.0 * k3x + k4x);
    m.y += (dt / 6.0) * (k1y + 2.0 * k2y + 2.0 * k3y + k4y);
    m.z += (dt / 6.0) * (k1z + 2.0 * k2z + 2.0 * k3z + k4z);

    // Keplerian mean motion, sheared by the current wobbled radius.
    let ratio = ring_radius_ratio(m);
    let omega = omega_base * m.pace * ratio.powf(-crate::constants::BLACK_HOLE_RING_KEPLER_EXP);
    m.phi += omega * dt_wall;

    m.sim_age += dt_wall;
    if m.sim_age >= m.lifetime {
        m.active = false;
        m.trail_len = 0;
        return true;
    }
    false
}

/// Project a mote onto the screen: the wide orbital ellipse around
/// the ball center. Horizontal reach is the semi-major axis
/// (`MAJOR_FRACTION` of the viewport unit, clamped to 92% of the
/// viewport's half-width so the extremes never clip on narrow
/// terminals); vertical squeeze is the semi-minor axis (`MINOR_FRACTION`
/// of the unit — the near edge-on read). The attractor's radial
/// coordinate wobbles the semi-major axis; its z displaces the mote
/// out of the ring plane (z high reads up, matching the brightness
/// ladder's depth cue). The near side's sine is squashed to
/// `NEAR_SQUASH` of the minor axis — the crossing band hugs the
/// equator (stage 2.3: the line reads at the vertical middle of the
/// core, not a full minor axis below it). On the far side the
/// projection blends into the lensing halo arc over the top of the
/// shadow, and the entry spiral scales both axes for young motes.
/// Returns float cell coordinates — the caller rounds, bounds-checks
/// and applies the occlusion rule (lorenz draw parity).
pub(crate) fn project_ring_mote(
    m: &RingMote,
    cx: f32,
    cy: f32,
    ball_outer_r: f32,
    major_limit: f32,
) -> (f32, f32) {
    let unit = ball_outer_r / crate::constants::BLACK_HOLE_BALL_FRACTION;
    let r_norm = ring_r_norm(m);
    let entry = entry_radius_scale(m.sim_age);
    let a_mean = (crate::constants::BLACK_HOLE_RING_MAJOR_FRACTION * unit).min(major_limit);
    let a = (a_mean + crate::constants::BLACK_HOLE_RING_WOBBLE_FRACTION * ball_outer_r * r_norm)
        .max(0.15)
        * entry;
    let b = (crate::constants::BLACK_HOLE_RING_MINOR_FRACTION * unit * (1.0 + 0.15 * r_norm))
        .max(0.05)
        * entry;
    let cos_phi = m.phi.cos();
    let sin_phi = m.phi.sin();
    // Equatorial squash (stage 2.3): the in-front half maps its sine
    // onto NEAR_SQUASH of the minor axis so the crossing line sits at
    // the core's vertical middle; the far half keeps the full factor
    // for its rise into the lensing halo. Both sides stay continuous
    // at the extremes (sin = 0 on either side of the branch).
    let sy = if sin_phi >= 0.0 {
        sin_phi * crate::constants::BLACK_HOLE_RING_NEAR_SQUASH
    } else {
        sin_phi
    };
    let col = cx + cos_phi * a * CELL_ASPECT_DIVISOR;
    let mut line = cy + sy * b;

    // Gravitational lensing: the far side (sin < 0, above center)
    // blends onto a halo arc over the top of the shadow. Backness
    // runs 0 at the disk extremes to 1 directly behind; the blend is
    // a smoothstep so the rise reads as one continuous curve. The
    // arc is a circle (in line-height units, round on screen like
    // the ball) of radius LENS_ARC_FRACTION x ball_outer_r — the
    // arc term flattens to cy beyond the arc's horizontal reach, so
    // the projection is continuous where the halo meets the disk.
    let backness = (-sin_phi).clamp(0.0, 1.0);
    if backness > 0.0 {
        let r_arc = ball_outer_r * crate::constants::BLACK_HOLE_RING_LENS_ARC_FRACTION;
        let x_off = (cos_phi * a).clamp(-r_arc, r_arc);
        let arc_y = cy - (r_arc * r_arc - x_off * x_off).sqrt();
        let w = backness * backness * (3.0 - 2.0 * backness);
        line = line * (1.0 - w) + arc_y * w;
    }

    let z_norm = ((m.z - crate::constants::BLACK_HOLE_RING_Z_NORM_CENTER)
        * crate::constants::BLACK_HOLE_RING_Z_NORM_GAIN)
        .clamp(-1.0, 1.0);
    line -= z_norm * crate::constants::BLACK_HOLE_RING_Z_TILT * ball_outer_r;
    (col, line)
}

/// Entry spiral radius scale for a mote of the given simulation age:
/// 1 + ENTRY_BOOST at age 0, settling exponentially to 1.0 with the
/// ENTRY_TAU time constant. Young motes project beyond the disk and
/// drift in — the accretion read (fresh material falling toward the
/// ring instead of appearing on it).
pub(crate) fn entry_radius_scale(sim_age: f32) -> f32 {
    1.0 + crate::constants::BLACK_HOLE_RING_ENTRY_BOOST
        * (-sim_age / crate::constants::BLACK_HOLE_RING_ENTRY_TAU).exp()
}

/// Occlusion rule (the 3D read, stage 2.3 — side-aware): the caller
/// knows the mote's side exactly (the sign of sin phi), so the rule
/// keys on physics, not screen position. A near-side cell passes in
/// front of the hole — never occluded, even above the equator (the
/// z-tilt breathes near motes both ways) and even across the empty
/// core (the crossing read). A far-side cell inside the ball
/// silhouette is behind the hole — hidden at any height; the visible
/// far side lives on the lensing halo arc outside the silhouette,
/// so only the physically-hidden segment is dropped. Distance is
/// aspect-corrected and in line-height units, the same math the
/// ball raster uses.
pub(crate) fn occludes_ring_cell(
    col: u16,
    line: u16,
    cx: i32,
    cy: i32,
    ball_outer_r: f32,
    near_side: bool,
) -> bool {
    if near_side {
        return false;
    }
    let dx = col as f32 - cx as f32;
    let dy = line as f32 - cy as f32;
    let dist = ((dx / CELL_ASPECT_DIVISOR).powi(2) + dy.powi(2)).sqrt();
    dist < ball_outer_r
}

/// Disk radial brightness profile (stage 2.3, the Gargantua read):
/// grades a mote's brightness by its horizontal orbital position
/// `|cos phi|` — 0 directly in front of (or lensed behind) the
/// shadow, 1 at the line's left/right extremes. Inside the inner
/// zone the level steps UP one rung (the hot inner disk — the solid
/// bright line across the core); beyond the fade start it steps
/// DOWN one to `EDGE_FADE_RUNGS` rungs (the line's ends dissolve
/// into sparse dim wisps, the smooth transition of the reference
/// imagery). Between the two bounds the attractor-z ladder rules
/// alone. Applied to the head at draw time; the comet trail steps
/// down from the graded head, so the ends fade together.
pub(crate) fn disk_profile_level(base: BrightnessLevel, phi: f32) -> BrightnessLevel {
    let c = phi.cos().abs();
    if c < crate::constants::BLACK_HOLE_RING_INNER_ZONE {
        bump_level(base, 1)
    } else if c > crate::constants::BLACK_HOLE_RING_EDGE_FADE_START {
        let span = 1.0 - crate::constants::BLACK_HOLE_RING_EDGE_FADE_START;
        let t = ((c - crate::constants::BLACK_HOLE_RING_EDGE_FADE_START) / span).clamp(0.0, 1.0);
        let rungs = (t * crate::constants::BLACK_HOLE_RING_EDGE_FADE_RUNGS as f32).round() as u8;
        step_down_level(base, rungs)
    } else {
        base
    }
}

/// Brightness zone by attractor z (the mote depth cue). Reuses the
/// lorenz style's zone boundaries — the ring integrates the SAME
/// canonical system, so the z semantics (lobe peaks near 40 hot,
/// saddle crossings near 13 dim) transfer unchanged.
pub(crate) fn level_for_ring_z(z: f32) -> BrightnessLevel {
    if z > crate::constants::LORENZ_Z_HOT {
        BrightnessLevel::Core
    } else if z > crate::constants::LORENZ_Z_MID {
        BrightnessLevel::Hot
    } else if z > crate::constants::LORENZ_Z_DIM {
        BrightnessLevel::Mid
    } else {
        BrightnessLevel::Ghost
    }
}

/// Step a brightness level down (toward Ghost) by `depth` ladder
/// rungs (mirrors the vortex/lorenz trail ladder — the family's
/// comet-dimming rule, one rung per trail cell).
pub(crate) fn step_down_level(level: BrightnessLevel, depth: u8) -> BrightnessLevel {
    match level {
        BrightnessLevel::Core if depth >= 2 => BrightnessLevel::Mid,
        BrightnessLevel::Core => BrightnessLevel::Hot,
        BrightnessLevel::Hot if depth >= 2 => BrightnessLevel::Ghost,
        BrightnessLevel::Hot => BrightnessLevel::Mid,
        BrightnessLevel::Mid => BrightnessLevel::Ghost,
        BrightnessLevel::Ghost | BrightnessLevel::Dim => BrightnessLevel::Ghost,
    }
}

/// The canonical Lorenz derivative (right-hand side of the ODE) —
/// identical to the lorenz style's integrator core: the attractor is
/// the same, only the projection differs. Pure function, called four
/// times per RK4 step.
#[inline]
fn lorenz_deriv(x: f32, y: f32, z: f32, sigma: f32, rho: f32, beta: f32) -> (f32, f32, f32) {
    let dx = sigma * (y - x);
    let dy = x * (rho - z) - y;
    let dz = x * y - beta * z;
    (dx, dy, dz)
}

/// Current orbital radius as a ratio of the mean ring radius — the
/// Keplerian shear input. The attractor radial coordinate is
/// normalized around the lobe radius and clamped, so the ratio stays
/// inside roughly [0.8, 1.2]: always positive (the powf in the
/// advance pass requires it) and bounded (the shear stays visible
/// without whipping). Mirrors the projection's a/a_mean (the wobble
/// amplitude over the mean semi-major axis, both in ball-radius
/// units).
fn ring_radius_ratio(m: &RingMote) -> f32 {
    let a_mean_in_outer_r = crate::constants::BLACK_HOLE_RING_MAJOR_FRACTION
        / crate::constants::BLACK_HOLE_BALL_FRACTION;
    1.0 + (crate::constants::BLACK_HOLE_RING_WOBBLE_FRACTION * ring_r_norm(m)) / a_mean_in_outer_r
}

/// Normalized attractor radial coordinate (the wobble source):
/// distance from the attractor z-axis, centered on the lobe radius
/// and scaled by its reciprocal, clamped to the visible band.
fn ring_r_norm(m: &RingMote) -> f32 {
    let r_l = (m.x * m.x + m.y * m.y).sqrt();
    ((r_l - crate::constants::BLACK_HOLE_RING_R_NORM_CENTER)
        * crate::constants::BLACK_HOLE_RING_R_NORM_GAIN)
        .clamp(-1.0, 1.2)
}
