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
//! Stage 2.4 (owner 9.7/10 feedback) re-works the brightness key
//! from the orbital angle to the projected DISTANCE from the hole:
//! `proximity_level` grades every head by its screen distance in
//! ball radii — two rungs up inside the hot radius (the white
//! crossing band and the whole lensing arc), one rung in the warm
//! belt, and the stage-2.3 fade ladder past 1.40 radii (the owner's
//! "particles near the hole burn white, the ones moving away fade").
//! Distance is invariant under the roll, so the glow stays anchored
//! to the hole at every tilt angle. Stage 2.7 (owner 9.95/10
//! feedback) floors the snug upper bands' base at Hot
//! (`floor_head_base_at_hot`) — their heads read Core (white).
//! NIGHT-research-11 (owner verdict 9.1/10, the soft-light round)
//! caps every ring head one rung below Core (`soft_head_level`):
//! the composed ladder may still burn hot, but the glyph heads
//! never land the Core white blend — the owner's report, the
//! head-white read strained his eyes; the soft warm ceiling is the
//! cinematic read, and the white stays reserved for the ball's own
//! photon structures.
//!
//! The three-tier stack (stage 2.4, tightened stage 2.5, descended
//! stage 2.7): the mote pool carries three bands whose geometry
//! comes from the `BLACK_HOLE_RING_TIERS` table — tier 0 is the
//! approved equatorial main disk, tier 1 a shorter band one snug
//! step above it, tier 2 the shortest band one more snug step up
//! (the owner's one-meter-gap ruling: the upper two lines sit close
//! to the main line and to each other, the tight stacked-arcs
//! family over the shadow; the stage-2.7 descent drops all three
//! band centers below center and stretches the main band a quarter
//! — his 1 cm -> 1.25 cm analogy). The upper tiers
//! skip the lensing arc and the occlusion rule (lensed images read
//! in front of the hole at any height); their two flow strands
//! straddle the band center so the orbit reads as a thin ribbon,
//! not a retraced line. Tier assignment rides the spawn pass
//! (weighted by each tier's share); the Keplerian pace scales per
//! tier so the inner bands visibly race the outer one — the
//! differential rotation of a real disk.
//!
//! The see-saw roll (stage 2.4, the lever motion): `RingRoll` owns
//! the stack's attitude angle — 0 is the flat horizontal rest line
//! (180 degrees in the owner's convention, the single longest pose
//! at a 36 s hold), excursions tilt the whole stack through the
//! 15-180 degree attitude window with exactly the vertical 90-degree
//! attitude excluded, every attitude parked at a long 30 s hold,
//! alternating sign, eased smoothstep sweeps
//! at a fixed angular rate, occasionally chaining tilt to tilt
//! through the rest line. The projection rotates every mote's
//! disk-plane offset by the live angle before the aspect
//! conversion, so the stack pivots rigidly around the hole: left end
//! up, right end down. The lensing arc rotates with the stack — physically
//! correct, the lensed image always sits perpendicular to the disk
//! plane.
//!
//! Entry spiral (the accretion read): freshly spawned motes carry
//! an entry radius excess that decays exponentially — material
//! drifts in from beyond the disk and settles onto the ring, never
//! popping in on the orbit. The same read serves the steady-state
//! respawn and the formation intro's accretion phase.
//!
//! Family contracts honored: one mote per column (lane model),
//! deficit-bounded spawn accumulator, lifetime absorption with
//! variance, motion-gated shimmer, comet trail, palette adoption,
//! three-pass diff cleanup (driven from the ball file's draw pass).
//!
//! Stage 2.6 note: the RK4 core and the radial normalization live
//! here as pub(crate) helpers because the halo stream motes
//! (`halo.rs`) ride the same attractor — one integrator, two
//! projections (the flat ellipse and the arc circles). The halo
//! pool reuses the tier byte as its stream tag (inner/lower/outer).

use std::time::{Duration, Instant};

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use super::super::monolith::BrightnessLevel;
use super::ball_helpers::bump_level;
use super::black_hole::CELL_ASPECT_DIVISOR;
use super::RollFrame;

/// One orbital mote: a glyph riding a Keplerian ring whose turbulence
/// is the RK4-integrated Lorenz state (x, y, z). Same field set as
/// `LorenzMote` plus the orbital angle and the stage-2.4 tier band —
/// the struct stays plain-old-data so the pool is one flat Vec
/// (cache-friendly, no per-frame allocation).
#[derive(Clone, Copy, Debug)]
pub(crate) struct RingMote {
    pub(crate) active: bool,
    /// Tier band of the stage-2.4 stack (0 = the equatorial main
    /// disk, 1 = the upper band, 2 = the rim-hugging band). Chosen
    /// at activation from the tier table's spawn weights; clamped
    /// lookups make any stale value safe. The halo pool reuses the
    /// byte as its stream tag (inner upper / lower / outer upper).
    pub(crate) tier: u8,
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
            tier: 0,
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

/// Compile-time contract: the tier table carries exactly the three
/// stages of the owner's Interstellar stack (longest, upper, shortest
/// — closest to the hole). A stray edit to the table length would
/// silently orphan a band's motes behind the clamped lookup.
const _: () = assert!(crate::constants::BLACK_HOLE_RING_TIERS.len() == 3);

/// Tier geometry lookup (clamped — a stale tier byte from an older
/// pool still resolves to a valid band instead of panicking).
fn tier_spec(tier: u8) -> &'static crate::constants::BlackHoleRingTier {
    let tiers = &crate::constants::BLACK_HOLE_RING_TIERS;
    let idx = (tier as usize).min(tiers.len() - 1);
    &tiers[idx]
}

/// Weighted tier pick for a fresh mote (the stage-2.4 stack's spawn
/// share). The weights sum to 1.0 in the table; the final fallback
/// covers floating-point under-sampling at the boundary.
fn pick_tier(roll: f32) -> u8 {
    let tiers = &crate::constants::BLACK_HOLE_RING_TIERS;
    let mut acc = 0.0;
    for (idx, tier) in tiers.iter().enumerate() {
        acc += tier.spawn_weight;
        if roll < acc {
            return idx as u8;
        }
    }
    (tiers.len() - 1) as u8
}

/// Activate a vacant mote: a uniform random orbital phase (spawns
/// spread around the full circumference from the first frame — no
/// clumping), a stage-2.4 tier band drawn from the stack's spawn
/// weights, the Lorenz state seeded at the textbook (±1, 1, 1)
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
    m.tier = pick_tier(rand_chance.sample(rng));
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
/// `disk_gain` (NIGHT-research-9) carries the disk unit's stretch
/// factor (disk_unit x BALL_FRACTION / ball_outer_r; 1.0 canonical)
/// so the shear normalization tracks the real semi-major-to-ball
/// ratio on stretched viewports. Returns true when the mote was
/// absorbed (lifetime reached).
pub(crate) fn advance_ring_mote(
    m: &mut RingMote,
    dt_wall: f32,
    dt_lorenz_base: f32,
    omega_base: f32,
    disk_gain: f32,
) -> bool {
    let dt = dt_lorenz_base * m.pace;
    rk4_lorenz_step(&mut m.x, &mut m.y, &mut m.z, dt);

    // Keplerian mean motion, sheared by the current wobbled radius
    // and paced by the tier band (stage 2.4: the inner bands orbit
    // visibly faster — Kepler's third law across the stack, the
    // differential rotation of a real multi-ring disk).
    let ratio = ring_radius_ratio(m, disk_gain);
    let omega = omega_base
        * m.pace
        * tier_spec(m.tier).pace
        * ratio.powf(-crate::constants::BLACK_HOLE_RING_KEPLER_EXP);
    m.phi += omega * dt_wall;

    m.sim_age += dt_wall;
    if m.sim_age >= m.lifetime {
        m.active = false;
        m.trail_len = 0;
        return true;
    }
    false
}

/// One classical RK4 step of the canonical Lorenz system over the
/// raw state coordinates (the integrator core the lorenz style
/// ships — extracted at stage 2.6 so the halo stream motes in
/// `halo.rs` ride the exact same mathematics: one attractor, one
/// integrator, two projections). Pure function, four derivative
/// evaluations per call.
pub(crate) fn rk4_lorenz_step(x: &mut f32, y: &mut f32, z: &mut f32, dt: f32) {
    let sigma = crate::constants::LORENZ_SIGMA;
    let rho = crate::constants::LORENZ_RHO;
    let beta = crate::constants::LORENZ_BETA;

    let (k1x, k1y, k1z) = lorenz_deriv(*x, *y, *z, sigma, rho, beta);
    let (k2x, k2y, k2z) = lorenz_deriv(
        *x + 0.5 * dt * k1x,
        *y + 0.5 * dt * k1y,
        *z + 0.5 * dt * k1z,
        sigma,
        rho,
        beta,
    );
    let (k3x, k3y, k3z) = lorenz_deriv(
        *x + 0.5 * dt * k2x,
        *y + 0.5 * dt * k2y,
        *z + 0.5 * dt * k2z,
        sigma,
        rho,
        beta,
    );
    let (k4x, k4y, k4z) = lorenz_deriv(
        *x + dt * k3x,
        *y + dt * k3y,
        *z + dt * k3z,
        sigma,
        rho,
        beta,
    );
    *x += (dt / 6.0) * (k1x + 2.0 * k2x + 2.0 * k3x + k4x);
    *y += (dt / 6.0) * (k1y + 2.0 * k2y + 2.0 * k3y + k4y);
    *z += (dt / 6.0) * (k1z + 2.0 * k2z + 2.0 * k3z + k4z);
}

/// Project a mote onto the screen: the wide orbital ellipse of its
/// tier band around the ball center, rolled by the live see-saw
/// angle. Horizontal reach is the tier's semi-major axis (the tier
/// table major scale times `MAJOR_FRACTION` of the DISK unit — the
/// width-stretched scale base, NIGHT-research-9: `disk_unit` is the
/// larger of the viewport's limiting half-extent and
/// `BLACK_HOLE_DISK_WIDTH_FRACTION` of the half-width, so wide
/// terminals host a proportionally longer disk; the wobble-inclusive
/// radius still clamps to 92% of the viewport's half-width so the
/// extremes never clip); the tier's semi-minor axis and band offsets
/// stay keyed to the ball (the family hugs the shadow — the
/// stretched disk reads thinner, the physical Gargantua proportion).
/// The attractor's radial coordinate wobbles the
/// semi-major axis; its z displaces the mote out of the ring plane
/// (z high reads up, matching the brightness ladder's depth cue).
/// The near side's sine is squashed to the tier's squash factor of
/// the minor axis — the crossing band hugs its rest line. Tier 0's
/// far side then blends into the lensing halo arc over the top of
/// the shadow; the upper tiers are flat lensed bands (both strands
/// straddle the band center). The entry spiral scales both axes for
/// young motes, and the final disk-plane offset rotates by `roll`
/// (the see-saw lever) before the aspect conversion, so the whole
/// stack pivots rigidly around the hole. Returns float cell
/// coordinates — the caller rounds, bounds-checks and applies the
/// occlusion rule (lorenz draw parity).
pub(crate) fn project_ring_mote(
    m: &RingMote,
    cx: f32,
    cy: f32,
    ball_outer_r: f32,
    disk_unit: f32,
    major_limit: f32,
    roll: RollFrame,
) -> (f32, f32) {
    let tier = tier_spec(m.tier);
    let unit = ball_outer_r / crate::constants::BLACK_HOLE_BALL_FRACTION;
    let r_norm = ring_r_norm(m);
    let entry = entry_radius_scale(m.sim_age);
    // Stage 2.7: the clamp bounds the wobble-inclusive radius (the
    // widened 1.25x reach would otherwise clip the tips off-screen).
    // NIGHT-research-9: the semi-major scales from the disk unit, not
    // the ball-derived unit — the wide-terminal stretch.
    let a_mean = crate::constants::BLACK_HOLE_RING_MAJOR_FRACTION * tier.major_scale * disk_unit;
    let a = (a_mean + tier.wobble_fraction * ball_outer_r * r_norm)
        .min(major_limit)
        .max(0.15)
        * entry;
    let b = (tier.minor_fraction * unit * (1.0 + 0.15 * r_norm)).max(0.05) * entry;
    // One fused trig evaluation per mote (NIGHT-lts-1 stage 1).
    let (sin_phi, cos_phi) = m.phi.sin_cos();
    // Equatorial squash (stage 2.3): the in-front half maps its sine
    // onto the tier's squash factor of the minor axis so the crossing
    // line hugs its rest height; the far half keeps the full factor
    // (tier 0: the rise into the lensing halo; the upper tiers: the
    // upper strand of the ribbon). Both sides stay continuous at the
    // extremes (sin = 0 on either side of the branch).
    let sy = if sin_phi >= 0.0 {
        sin_phi * tier.near_squash
    } else {
        sin_phi
    };
    // Disk-plane offset in line-height units (x right, y down): the
    // tier band's center offset lifts the ribbon above the equator
    // (0 for the main disk).
    let x = cos_phi * a;
    let mut y = sy * b - tier.center_offset * ball_outer_r;

    // Gravitational lensing (tier 0 only — the flat main disk): the
    // far side (sin < 0, above center) blends onto a halo arc over
    // the top of the shadow. Backness runs 0 at the disk extremes to
    // 1 directly behind; the blend is a smoothstep so the rise reads
    // as one continuous curve. The arc is a circle (in line-height
    // units, round on screen like the ball) whose radius is the lens
    // arc fraction times the ball outer radius — the arc term
    // flattens to the center line beyond the arc's horizontal reach,
    // so the projection is continuous where the halo meets the disk.
    // The upper tiers skip the arc (their flat ribbons ARE the
    // lensed-image read).
    if m.tier == 0 {
        let backness = (-sin_phi).clamp(0.0, 1.0);
        if backness > 0.0 {
            let r_arc = ball_outer_r * crate::constants::BLACK_HOLE_RING_LENS_ARC_FRACTION;
            let x_off = (cos_phi * a).clamp(-r_arc, r_arc);
            let arc_y = -(r_arc * r_arc - x_off * x_off).sqrt();
            let w = backness * backness * (3.0 - 2.0 * backness);
            y = y * (1.0 - w) + arc_y * w;
        }
    }

    let z_norm = ((m.z - crate::constants::BLACK_HOLE_RING_Z_NORM_CENTER)
        * crate::constants::BLACK_HOLE_RING_Z_NORM_GAIN)
        .clamp(-1.0, 1.0);
    y -= z_norm * tier.z_tilt * ball_outer_r;

    // See-saw roll (stage 2.4, the lever): rotate the disk-plane
    // offset around the hole center by the live attitude angle. A
    // positive angle lifts the LEFT end and drops the right end
    // (screen y grows downward); the rotation runs in line-height
    // units so it is a true Euclidean pivot, and the column offset
    // only converts through the cell aspect afterwards. Applied
    // AFTER the arc blend and the z-tilt: the whole stack — flat
    // disk, lensing halo, breathing bands — pivots as one rigid
    // body around the hole. The trig pair arrives precomputed in
    // the `RollFrame` snapshot (NIGHT-lts-1 stage 1).
    let (x_r, y_r) = if !roll.is_flat() {
        (x * roll.cos - y * roll.sin, x * roll.sin + y * roll.cos)
    } else {
        (x, y)
    };
    let col = cx + x_r * CELL_ASPECT_DIVISOR;
    let line = cy + y_r;
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

/// Disk proximity brightness profile (stage 2.4, the owner's 9.7/10
/// feedback): grades a mote's brightness by its projected screen
/// DISTANCE from the hole's center, in ball outer radii — the key
/// the roll cannot disturb (rotation preserves distance), unlike the
/// stage-2.3 orbital-angle key. Inside the hot radius the level
/// steps UP two rungs (the white-hot read: the crossing band across
/// the shadow and the whole lensing arc — Mid and Hot bases land at
/// Core, the "head white" of the owner's ask); in the warm belt up
/// to the fade start it steps up one; past the fade start it steps
/// DOWN one to `EDGE_FADE_RUNGS` rungs over the fade span — the
/// line's ends and the outer disk dissolve into sparse dim wisps,
/// the smooth transition of the reference imagery. Applied to the
/// head at draw time; the comet trail steps down from the graded
/// head, so the ends fade together. Entry-spiral motes drift in
/// from beyond the span at Ghost and ignite as they settle — the
/// accretion read.
pub(crate) fn proximity_level(base: BrightnessLevel, dist_norm: f32) -> BrightnessLevel {
    let d = dist_norm.max(0.0);
    if d > crate::constants::BLACK_HOLE_RING_FADE_START {
        let span = crate::constants::BLACK_HOLE_RING_FADE_SPAN;
        let t = ((d - crate::constants::BLACK_HOLE_RING_FADE_START) / span).clamp(0.0, 1.0);
        let rungs = (t * crate::constants::BLACK_HOLE_RING_EDGE_FADE_RUNGS as f32).round() as u8;
        step_down_level(base, rungs)
    } else if d < crate::constants::BLACK_HOLE_RING_HOT_RADIUS {
        bump_level(base, 2)
    } else {
        bump_level(base, 1)
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

/// Stage 2.7 (owner 9.95/10 feedback): the snug upper stacks' heads
/// read more bright/white — their z-ladder base floors at Hot, so
/// tiers 1-2 land Core (the white head) across their reach; tier 0
/// keeps the plain z-graded base (the approved head mix).
pub(crate) fn floor_head_base_at_hot(level: BrightnessLevel) -> BrightnessLevel {
    match level {
        BrightnessLevel::Core => BrightnessLevel::Core,
        _ => BrightnessLevel::Hot,
    }
}

/// The disk mote's head base ladder (NIGHT-research-10, re-pinned
/// NIGHT-research-11): tiers 1-2 floor at Hot (the stage-2.7
/// white-stack ruling, unchanged), and the tier-0 equatorial band
/// floors at Hot too whenever the head rides inside the hot radius
/// — the owner's report, "the center ring reads a bit dark near the
/// ball": a Ghost-zone z base stepped up two rungs by the proximity
/// ladder lands only Mid, so the crossing band across the shadow
/// dimmed exactly where the iconic imagery burns brightest. With
/// the floor, the near-ball tier-0 heads burn at the top of the
/// ladder after the +2 bump — the disk's inner reach matches the
/// lanes' read — while the far arms keep the plain z-graded base
/// and dissolve through the fade ladder as before. The composed
/// head level then passes through the NIGHT-research-11 soft-head
/// cap at the draw site: the crossing band lands the SOFT warm
/// ceiling (Hot — the full-palette warm white without the Core
/// blend toward white), the owner's "not too bright, soft elegant,
/// cinematic" ruling on the near-ball band that used to read
/// blinding Core white.
pub(crate) fn ring_head_base(tier: u8, dist_norm: f32, z: f32) -> BrightnessLevel {
    let z_level = level_for_ring_z(z);
    if tier >= 1 || dist_norm < crate::constants::BLACK_HOLE_RING_HOT_RADIUS {
        floor_head_base_at_hot(z_level)
    } else {
        z_level
    }
}

/// The soft-head cap (NIGHT-research-11, the owner's soft-light
/// ruling): steps a composed glyph-head level down one rung
/// whenever it lands Core — Hot in, Hot out; every other level
/// passes through unchanged. The owner's report on the
/// NIGHT-research-10 head-white read: the Core blend toward white
/// (the renderer's brightest output, full palette plus the white
/// lift) across the crowns, the crossing band and the snug stacks
/// strained his eyes — the cinematic read he asked for instead is
/// the SOFT warm ceiling: every glyph head tops out at Hot (85
/// percent of the palette, no white blend), warm and elegant, while
/// the Core white stays reserved for the ball's own thin photon
/// structures (the horizon ring and the rim photon line — the
/// signature lines, not the glyph heads) and the infall's transient
/// whip-around flash. Applied at the ring and halo draw sites AFTER
/// the proximity ladder composes the head level, so the distance
/// key, the floors and the fade ladder all keep shaping the band
/// below the ceiling exactly as before.
pub(crate) fn soft_head_level(level: BrightnessLevel) -> BrightnessLevel {
    match level {
        BrightnessLevel::Core => BrightnessLevel::Hot,
        other => other,
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

/// Current orbital radius as a ratio of the mote's tier-band mean
/// ring radius — the Keplerian shear input. The attractor radial
/// coordinate is normalized around the lobe radius and clamped, so
/// the ratio stays inside roughly [0.8, 1.2]: always positive (the
/// powf in the advance pass requires it) and bounded (the shear
/// stays visible without whipping). The mean radius is per-tier
/// (stage 2.4) so each band's shear matches its own wobble scale —
/// the tier-0 path is numerically the pre-2.4 formula. The
/// `disk_gain` argument (NIGHT-research-9) scales the mean to the
/// live semi-major-to-ball ratio, so a width-stretched disk's shear
/// normalization stays physically keyed to its real reach.
fn ring_radius_ratio(m: &RingMote, disk_gain: f32) -> f32 {
    let tier = tier_spec(m.tier);
    let a_mean_in_outer_r = crate::constants::BLACK_HOLE_RING_MAJOR_FRACTION * tier.major_scale
        / crate::constants::BLACK_HOLE_BALL_FRACTION
        * disk_gain.max(0.05);
    1.0 + (tier.wobble_fraction * ring_r_norm(m)) / a_mean_in_outer_r
}

/// Normalized attractor radial coordinate (the wobble source):
/// distance from the attractor z-axis, centered on the lobe radius
/// and scaled by its reciprocal, clamped to the visible band.
/// Shared with `halo.rs` (the stream riders wobble their arc radius
/// through the same normalization).
pub(crate) fn ring_r_norm(m: &RingMote) -> f32 {
    let r_l = (m.x * m.x + m.y * m.y).sqrt();
    ((r_l - crate::constants::BLACK_HOLE_RING_R_NORM_CENTER)
        * crate::constants::BLACK_HOLE_RING_R_NORM_GAIN)
        .clamp(-1.0, 1.2)
}
