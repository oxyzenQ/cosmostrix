// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Black hole halo streams (NIGHT-special-1 stage 2.6, re-weighted
//! stage 2.7): the arc-riding companion pool of the disk stack —
//! the per-mote physics half, split from `black_hole.rs` exactly the
//! way `ring.rs` splits the ring physics (the pool bookkeeping, the
//! spawn/advance/draw orchestration and the diff-cleanup stream stay
//! in the main file).
//!
//! Owner read (the 9.9/10 round): the particles curving upward over
//! the hole must double their density, and a NEW mirrored stream must
//! curve downward under it — the same motion, the opposite position,
//! slightly fewer particles, the rotation following the disk. The
//! owner read (the 9.95/10 round): the upward curve is now a DOUBLE
//! upward stream — two distinct crowns over the shadow — and the
//! lower stream drops to a RARE echo. The design: a second mote pool
//! (one lane per column, the family contract) whose riders orbit the
//! ARC CIRCLE around the shadow instead of the flat ellipse. Each
//! mote rides the full circle with the ring's own motion DNA — one
//! RK4 Lorenz step per frame (the shared `rk4_lorenz_step` core: one
//! attractor, one integrator, two projections), a Keplerian angular
//! rate paced to each stream's own arc radius (the arcs sit beyond
//! the disk, so the streams orbit visibly slower — the outer-lane
//! read of Kepler's third law, the outer crown slowest of all), the
//! radial coordinate wobbling the arc radius into a thin plasma band,
//! the z coordinate grading the brightness through the shared
//! ladder, the entry-spiral drift-in, the comet trail, the
//! motion-gated shimmer.
//!
//! The streams: the mote's tag picks the semicircle AND the arc it
//! draws on — tier byte 0 rides the INNER UPPER arc (the
//! 1.30-radius lensing circle — its riders share the road with the
//! far-side lensed image, the white-hot inner crown), tier byte 2
//! rides the OUTER UPPER arc (the 1.48-radius circle, the second
//! lane of the stage-2.7 double upward stream — clear of the inner
//! crown's wobble band so the two arcs read as two distinct
//! crowns), tier byte 1 rides the LOWER arc (the mirrored circle
//! under the shadow, the owner's opposite-position stream, now RARE
//! per the spawn weights). The ride is the full circle; the draw
//! filter hides each mote through the opposite semicircle, so the
//! handoffs land at the extremes where the arcs meet the equatorial
//! band — the read of plasma sweeping over the top and under the
//! bottom of the shadow in the same rotational sense as the disk
//! (the upper sweeps run left limb to apex to right limb, the
//! far-side lensing direction).
//!
//! No occlusion rule: the arc circles sit at 1.30 and 1.48 outer
//! radii with a 0.10 wobble band, so a rider never dips inside the
//! 1.0-radius silhouette (the entry spiral only adds outward
//! distance). The see-saw roll rotates the projection like every
//! other body of the system — a circle maps onto itself, but the
//! riders' phases pivot with the tilted disk extremes, keeping the
//! handoff zone synced with the stack's ends at every attitude.
//!
//! Geometry stays in fractions of the ball outer radius (which is
//! itself a fraction of the viewport's limiting half-extent), so the
//! streams scale with any screen size from 80x24 to 400x100 — the
//! dynamic-screen-size contract the owner pinned this round.

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use super::black_hole::CELL_ASPECT_DIVISOR;
use super::ring::{entry_radius_scale, ring_r_norm, rk4_lorenz_step, RingMote};

/// Stream tag: the inner upper halo stream (the rider draws on the
/// upper semicircle of the 1.30-radius lensing circle over the
/// shadow — the inner crown of the stage-2.7 double upward stream).
pub(crate) const HALO_STREAM_TAG_UPPER: u8 = 0;

/// Stream tag: the lower halo stream (the rider draws on the lower
/// semicircle — the mirrored circle under the shadow, the rare
/// stage-2.7 echo).
pub(crate) const HALO_STREAM_TAG_LOWER: u8 = 1;

/// Stream tag: the outer upper halo stream (the rider draws on the
/// upper semicircle of the 1.48-radius circle — the outer crown of
/// the stage-2.7 double upward stream, the owner's 9.95/10 ruling).
pub(crate) const HALO_STREAM_TAG_UPPER_OUTER: u8 = 2;

/// Activate a vacant halo stream mote with its stream tag chosen by
/// the caller (the split runs a deterministic fractional accumulator
/// plus a lane toggle in the spawn pass — see `BlackHoleRain::spawn`
/// — so the two upper crowns hold exactly equal shares and the lower
/// stream stays exactly rare on every pool fill, no spawn luck). The
/// rest is the ring motes' own recipe: a uniform random orbital
/// phase (riders spread around the full circle from the first
/// frame), the same textbook Lorenz seed and per-mote pace / lifetime
/// variance — the pools differ only in what the state drives.
pub(crate) fn activate_halo_mote(
    m: &mut RingMote,
    stream_tag: u8,
    lobe_sign: f32,
    palette_slot: u8,
    rand_chance: &Uniform<f32>,
    rng: &mut StdRng,
) {
    m.active = true;
    // Stream tag: reuses the mote's tier byte (the halo pool never
    // reads the ring's tier table — the tag is a plain stream id,
    // stored verbatim so all three lanes round-trip through the
    // clamped tier lookups of the physics below).
    m.tier = stream_tag;
    m.phi = rand_chance.sample(rng) * std::f32::consts::TAU;

    let perturb = crate::constants::LORENZ_SPAWN_PERTURB;
    m.x = lobe_sign + (rand_chance.sample(rng) - 0.5) * 2.0 * perturb;
    m.y = 1.0 + (rand_chance.sample(rng) - 0.5) * 2.0 * perturb;
    m.z = 1.0 + (rand_chance.sample(rng) - 0.5) * 2.0 * perturb;

    m.sim_age = 0.0;
    m.lifetime =
        crate::constants::BLACK_HOLE_HALO_MAX_AGE_SECS * (0.85 + rand_chance.sample(rng) * 0.30);
    m.pace = 0.85 + rand_chance.sample(rng) * 0.30;
    m.palette_slot = palette_slot;
    m.trail_len = 0;
}

/// Advance one stream mote by one frame: the shared RK4 Lorenz step,
/// then the Keplerian angular advance paced to the mote's own arc
/// radius. The shear input is the same wobble ratio (normalized
/// around the arc's own mean radius), so the turbulence that
/// thickens the band also speeds and slows the riders — the
/// differential-rotation signature carried onto the arcs. The outer
/// crown's Keplerian pace is slower than the inner lanes (Kepler's
/// third law across the double stream). Returns true when the mote
/// was absorbed (lifetime reached).
pub(crate) fn advance_halo_mote(
    m: &mut RingMote,
    dt_wall: f32,
    dt_lorenz_base: f32,
    omega_base: f32,
) -> bool {
    let dt = dt_lorenz_base * m.pace;
    rk4_lorenz_step(&mut m.x, &mut m.y, &mut m.z, dt);

    // Keplerian mean motion at the arc radius: the halo pace scales
    // the base omega to the mote's lane (Kepler's third law between
    // the disk's mean radius and each arc circle — the outer crown
    // rides the slowest lane), sheared by the current wobbled radius
    // exactly like the ring motes.
    let arc_in_outer_r = halo_arc_fraction(m);
    let ratio =
        1.0 + (crate::constants::BLACK_HOLE_HALO_WOBBLE_FRACTION * ring_r_norm(m)) / arc_in_outer_r;
    let omega = omega_base
        * m.pace
        * halo_kepler_pace(m)
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

/// Project a stream mote onto the screen: the rider's position on
/// its arc circle around the ball center. The mapping runs the angle
/// through minus signs so the phi advance carries the upper sweep
/// left limb -> apex -> right limb — the far-side lensing flow's
/// direction, the rotational sense the owner asked the streams to
/// follow. The attractor's radial coordinate wobbles the arc radius
/// (the thin plasma band); the entry spiral scales the radius for
/// young motes (drift-in from beyond the arc, never a pop-in); the
/// see-saw roll rotates the disk-plane offset around the hole center
/// like every other body of the system. Returns float cell
/// coordinates — the caller rounds and bounds-checks (no occlusion:
/// the arc band never enters the silhouette).
pub(crate) fn project_halo_mote(
    m: &RingMote,
    cx: f32,
    cy: f32,
    ball_outer_r: f32,
    roll: f32,
) -> (f32, f32) {
    let arc_r = ball_outer_r * halo_arc_fraction(m);
    let r_norm = ring_r_norm(m);
    let entry = entry_radius_scale(m.sim_age);
    let r = (arc_r + crate::constants::BLACK_HOLE_HALO_WOBBLE_FRACTION * ball_outer_r * r_norm)
        .max(0.2)
        * entry;

    let x = -r * m.phi.cos();
    let y = -r * m.phi.sin();

    // See-saw roll: same rigid-pivot rotation the ring projection
    // applies (a circle is roll-invariant, but the riders' phases
    // stay synced with the tilted disk's extremes).
    let (x_r, y_r) = if roll != 0.0 {
        let (s, c) = (roll.sin(), roll.cos());
        (x * c - y * s, x * s + y * c)
    } else {
        (x, y)
    };
    let col = cx + x_r * CELL_ASPECT_DIVISOR;
    let line = cy + y_r;
    (col, line)
}

/// The stream-visibility filter: a mote draws only on its own
/// semicircle. The disk-plane offset is y = -r sin(phi), so the ride
/// carries the mote ABOVE the equator exactly while sin(phi) > 0 —
/// both upper tags (the inner and outer crowns of the double upward
/// stream) draw through that half of the lap and hide through the
/// other, the lower tag the mirror image. The handoffs land at the
/// circles' horizontal extremes, the zone where the arcs already
/// blend into the equatorial band — the streams read as merging
/// into the disk line and re-emerging from the opposite limb.
/// Crossing into the hidden half retires the trail (the caller's
/// job) so the re-emergence never paints a teleporting tail.
pub(crate) fn halo_mote_visible(m: &RingMote) -> bool {
    let above_center = m.phi.sin() > 0.0;
    (m.tier != HALO_STREAM_TAG_LOWER) == above_center
}

/// The arc radius fraction of a mote's stream (the tag lookup — the
/// inner upper crown co-rides the lensing circle, the outer upper
/// crown rides the wider double-stream arc, the lower stream its
/// mirrored twin).
fn halo_arc_fraction(m: &RingMote) -> f32 {
    match m.tier {
        HALO_STREAM_TAG_UPPER => crate::constants::BLACK_HOLE_HALO_ARC_FRACTION,
        HALO_STREAM_TAG_UPPER_OUTER => crate::constants::BLACK_HOLE_HALO_OUTER_ARC_FRACTION,
        _ => crate::constants::BLACK_HOLE_HALO_LOWER_ARC_FRACTION,
    }
}

/// The Keplerian pace multiplier of a mote's stream (the tag lookup
/// — the outer crown rides the slowest lane of the system, Kepler's
/// third law between the two crowns of the double upward stream).
fn halo_kepler_pace(m: &RingMote) -> f32 {
    if m.tier == HALO_STREAM_TAG_UPPER_OUTER {
        crate::constants::BLACK_HOLE_HALO_OUTER_PACE
    } else {
        crate::constants::BLACK_HOLE_HALO_PACE
    }
}
