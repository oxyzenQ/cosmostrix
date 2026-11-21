// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! The conservation-ledger family — six solvers, six named discrete
//! contracts (night-frontier-Z8, INV-20).
//!
//! The wave field already carries its named Lyapunov ledger (entry
//! 46). The OTHER original motion-math systems carried no named
//! quantity at all: "six original motion-math systems" was a claim
//! about authorship, not about measured physics. This module turns
//! the claim into contracts — one named, closed-form quantity per
//! solver, asserted per frame on the real production rigs:
//!
//!   1. Keplerian orbits (`black_hole`): the angular-momentum ledger.
//!      The infall velocity update is CENTRAL (the gravity blend and
//!      the influence falloff scale only the magnitude; the direction
//!      stays radial), so under the semi-implicit Euler ordering the
//!      specific angular momentum L (the planar cross product:
//!      x times vy minus y times vx) is conserved EXACTLY per
//!      substep wherever the accretion brake is absent:
//!      v' = v + a*dt with a parallel to x gives
//!      L' = (x + v'*dt) x v' = x x v' + dt (v' x v') = x x v', and
//!      L' - L = x x (v' - v) = dt (x x a) = 0. The accretion brake
//!      (the tangential decay inside the capture radius) is the
//!      designed L sink and is disclosed, not hidden.
//!   2. PIC/FLIP (`flux`): the pool ledger. The active count is the
//!      bookkeeping truth (spawn +1, retire -1, nothing else), and
//!      the velocity clamp is a hard ceiling no particle ever
//!      exceeds.
//!   3. Physarum stigmergy (`physarum`): the mass-balance budget.
//!      Trail mass can only grow by the per-agent deposit budget
//!      (at most one deposit per active agent per frame) and can
//!      only shrink by the documented decay exponent — both
//!      closed-form, so the measured field total must sit inside
//!      the budget window every frame.
//!   4. Boids (`murmuration`): the flight-band ledger. Every bird's
//!      speed sits inside the documented band after every frame and
//!      no bird teleports farther than its band allows in one step.
//!   5. Lorenz RK4 (`lorenz`): the absorbing-set contract. The
//!      honest scope: a chaotic system has NO monotone energy
//!      contract — what it has is the textbook absorbing region, so
//!      the ledger pins the measured state envelope (with margin)
//!      and the absorption promptness instead of pretending an
//!      energy law exists.
//!   6. FABRIK chain dynamics (`dragon`): the spacing ledger. The
//!      constraint pass projects every segment to EXACTLY
//!      DRAGON_SEGMENT_SPACING from its predecessor (closed-form:
//!      seg -= d * (dist - spacing) / dist lands the distance on
//!      spacing), so chain integrity is an exact per-frame
//!      invariant, not an aesthetic.
//!
//! Honest scope, stated once: the ledgers read the solvers' state
//! through the existing test accessors (the house `*_for_test`
//! pattern); the runtime render path is untouched, so the
//! determinism and zero-alloc contracts (entries 32 and 36) see no
//! new surface. The bounds are physics bounds, not bit-exact pins:
//! they tolerate f32 rounding, libm variance, and codegen across
//! the lanes the `lock_` filter rides (MSRV, the musl shards, the
//! arm64 parity lane). The teeth are at the bottom: the kepler
//! drift helper must DETECT a deliberately corrupted orbit — a
//! ledger that cannot see a violation cannot enforce one.

use super::tests_black_hole::{make_black_hole_cloud, run_frames};
use super::tests_dragon::make_dragon_cloud;
use super::tests_flux::make_flux_cloud;
use super::tests_lorenz::make_lorenz_cloud;
use super::tests_murmuration::make_murm_cloud;
use super::tests_physarum::make_physarum_cloud;

use crate::cloud::type_rain::black_hole::infall::advance_infall_mote;
use crate::constants::{
    DRAGON_SEGMENT_SPACING, FLUX_MAX_SPEED, MURM_SPEED_MAX, MURM_SPEED_MIN,
    PHYSARUM_DEPOSIT_AMOUNT, PHYSARUM_TRAIL_DECAY,
};

/// The pinned conservation screen (the family's shared rectangle).
const Z8_COLS: u16 = 120;
const Z8_LINES: u16 = 40;

/// The rig's frame period: 16 ms steps at the 60 Hz virtual ladder.
const Z8_STEP_MS: u64 = 16;
const Z8_DT: f32 = Z8_STEP_MS as f32 / 1000.0;

/// The relative tolerance for the kepler exactness identity: the
/// derivation is exact in real arithmetic, so what remains is f32
/// rounding over the substep chain — four orders of magnitude of
/// headroom above the rounding scale.
const KEPLER_L_REL_TOL: f32 = 1.0e-3;

/// The kepler angular-momentum ledger for one mote snapshot.
///
/// L is the specific angular momentum in the hole's orbital plane:
/// x times vy minus y times vx (outer radii per sim-second, signed
/// by orbit direction). Shared by the exactness test and the teeth:
/// the same function measures both the honest orbit and the
/// corrupted one.
#[must_use]
fn kepler_angular_momentum(x: f32, y: f32, vx: f32, vy: f32) -> f32 {
    x * vy - y * vx
}

/// INV-20 (kepler): L is EXACTLY conserved across the drag-free
/// flight of an infall mote.
///
/// The mote is driven directly (the unit level, where the identity
/// is checkable step by step): a tangential-plus-radial orbit at
/// r = 2.2 — inside the influence falloff (partial central
/// gravity, which the identity covers) and outside the accretion
/// brake's capture radius (no drag). Every advance must leave L
/// unchanged to f32 rounding.
#[test]
fn lock_inv20_kepler_angular_momentum_exact_in_drag_free_flight() {
    let mut m = crate::cloud::type_rain::black_hole::infall::InfallMote::vacant();
    m.active = true;
    // A near-circular orbit at r = 2.2 outer radii. The circular
    // speed at this radius is sqrt(G * blend / r) with G = 5.2 and
    // the falloff blend at 2.2 (~0.46), which evaluates to ~1.05 —
    // the seed sits just above it, so the orbit stays a near
    // circle: periapsis never dips inside the accretion brake's
    // capture radius, and the whole window is the drag-free
    // central flight the contract covers.
    m.x = 2.2;
    m.y = 0.0;
    m.vx = 0.05;
    m.vy = 1.05;
    let l0 = kepler_angular_momentum(m.x, m.y, m.vx, m.vy);
    assert!(
        l0.abs() > 0.5,
        "the seeded orbit must carry real L (got {l0})"
    );

    // Generous exit margins: the orbit must never count as absorbed
    // inside this window (the ledger measures flight, not exit).
    let (exit_w, exit_h) = (100.0_f32, 100.0_f32);
    for _ in 0..400 {
        let absorbed = advance_infall_mote(&mut m, Z8_DT, exit_w, exit_h);
        assert!(!absorbed, "the drag-free window must not absorb the mote");
        // The window guard: the orbit stays outside the capture
        // radius, so every substep is the pure central flight the
        // identity covers (if this fires, the seed drifted inward
        // and the window is no longer the contract's scope).
        let r = (m.x * m.x + m.y * m.y).sqrt();
        assert!(
            r >= crate::constants::BLACK_HOLE_INFALL_CAPTURE_FRACTION + 0.05,
            "the seeded orbit dipped inside the capture radius (r {r}) — \
             the window left the drag-free scope this contract covers"
        );
        let l = kepler_angular_momentum(m.x, m.y, m.vx, m.vy);
        let drift = ((l - l0) / l0.abs().max(1.0)).abs();
        assert!(
            drift <= KEPLER_L_REL_TOL,
            "the angular-momentum identity broke: L drifted {l} from {l0} \
             (relative {drift}) — the velocity update is no longer purely \
             central or the ordering changed; both are engine contracts"
        );
    }
}

/// INV-20 (kepler, cloud level): the same identity holds in situ.
///
/// The pool is sampled frame by frame; a slot that is alive with
/// r outside the capture radius at BOTH ends of a frame pair must
/// drift at most a generous bounded amount (the exact identity
/// covers pure central flight; a mote that brushed the accretion
/// brake between the two samples legitimately lost L to the
/// designed sink — the bound absorbs a brush, not a breakage).
#[test]
fn lock_inv20_kepler_cloud_orbits_stay_bounded() {
    let mut cloud = make_black_hole_cloud(Z8_COLS, Z8_LINES);
    let mut frame = crate::frame::Frame::new(Z8_COLS, Z8_LINES, cloud.palette.bg);

    // A long window: the formation plays, the rain layer spawns,
    // orbits evolve. 600 frames at 16 ms = 9.6 virtual seconds.
    let mut prev_l: Vec<Option<(f32, f32)>> = Vec::new();
    for _ in 0..600 {
        run_frames(&mut cloud, &mut frame, 1, Z8_STEP_MS);
        let motes = cloud.black_hole_rain.infall_motes_for_test();
        if prev_l.len() != motes.len() {
            prev_l = vec![None; motes.len()];
        }
        for (i, m) in motes.iter().enumerate() {
            let r = (m.x * m.x + m.y * m.y).sqrt();
            let l = kepler_angular_momentum(m.x, m.y, m.vx, m.vy);
            if m.active && r >= crate::constants::BLACK_HOLE_INFALL_CAPTURE_FRACTION + 0.25 {
                if let Some((l_prev, _r_prev)) = prev_l[i] {
                    let drift = ((l - l_prev) / l_prev.abs().max(1.0)).abs();
                    assert!(
                        drift <= 0.08,
                        "the cloud-level angular-momentum bound broke for slot {i}: \
                         L {l_prev} -> {l} (relative {drift}) — a drag-free pair of \
                         samples must not lose this much L; the central update or \
                         the ordering regressed"
                    );
                }
                prev_l[i] = Some((l, r));
            } else {
                prev_l[i] = None;
            }
        }
    }
}

/// INV-20 (flux): the pool ledger — bookkeeping truth and the hard
/// speed ceiling, every frame.
#[test]
fn lock_inv20_flip_pool_ledger_and_speed_ceiling() {
    let mut cloud = make_flux_cloud(Z8_COLS, Z8_LINES);
    let mut frame = crate::frame::Frame::new(Z8_COLS, Z8_LINES, cloud.palette.bg);

    for _ in 0..300 {
        run_frames(&mut cloud, &mut frame, 1, Z8_STEP_MS);
        let live = cloud.flux_rain.motes.iter().filter(|m| m.active).count();
        assert_eq!(
            cloud.flux_rain.active_count(),
            live,
            "the flux pool ledger desynced: the active_count field says {} \
             but the mote vector carries {live} active motes — spawn or \
             retire bookkeeping regressed",
            cloud.flux_rain.active_count()
        );
        for m in &cloud.flux_rain.motes {
            if !m.active {
                continue;
            }
            let speed2 = m.vx * m.vx + m.vy * m.vy;
            let ceiling = FLUX_MAX_SPEED * FLUX_MAX_SPEED * 1.0001;
            assert!(
                speed2 <= ceiling,
                "a flux particle exceeds the numeric safety ceiling: speed² \
                 {speed2} against {}² — the clamp regressed or a new force \
                 bypasses it",
                FLUX_MAX_SPEED
            );
        }
    }
}

/// INV-20 (physarum): the mass-balance budget — the trail field's
/// total must sit inside the closed-form deposit/decay window every
/// frame.
///
/// The field can only grow by the deposit budget (each active agent
/// adds at most PHYSARUM_DEPOSIT_AMOUNT * dt * pace_max, in-bounds)
/// and can only shrink by the documented decay exponent (with a
/// floor that absorbs the blended sim clock's variance). A breach
/// in either direction names the defect: unaccounted mass creation,
/// or a decay that no longer matches the frame-rate-independence
/// law.
#[test]
fn lock_inv20_physarum_deposit_budget_mass_balance() {
    let mut cloud = make_physarum_cloud(Z8_COLS, Z8_LINES);
    let mut frame = crate::frame::Frame::new(Z8_COLS, Z8_LINES, cloud.palette.bg);

    // The pace multiplier's documented upper bound (0.85..1.15).
    const PACE_MAX: f32 = 1.15;
    // The decay floor: the documented exponent at twice the nominal
    // step (absorbs the blended sim clock's variance and the first
    // frame's clock seeding).
    let decay_floor = PHYSARUM_TRAIL_DECAY.powf((Z8_DT * 2.0) * 60.0);

    for _ in 0..300 {
        let mass_before: f64 = cloud
            .physarum_rain
            .trail_field
            .iter()
            .map(|v| f64::from(*v))
            .sum();
        let agents = cloud.physarum_rain.active_count() as f64;
        let deposit_budget =
            agents * f64::from(PHYSARUM_DEPOSIT_AMOUNT) * f64::from(Z8_DT * PACE_MAX);

        run_frames(&mut cloud, &mut frame, 1, Z8_STEP_MS);

        let mass_after: f64 = cloud
            .physarum_rain
            .trail_field
            .iter()
            .map(|v| f64::from(*v))
            .sum();

        // Upper: mass can only grow by the budget (decay only
        // shrinks). Lower: mass cannot shrink faster than the decay
        // floor applied to the pre-frame total plus any deposits
        // (deposits only add). The window is the contract.
        let upper = mass_before + deposit_budget + 1.0;
        let lower = (mass_before * f64::from(decay_floor)) - 1.0;
        assert!(
            mass_after <= upper,
            "the trail mass budget broke upward: {mass_before} -> {mass_after} \
             against a deposit budget of {deposit_budget} ({agents} agents) — \
             mass entered the field outside the deposit path"
        );
        assert!(
            mass_after >= lower,
            "the trail mass budget broke downward: {mass_before} -> {mass_after} \
             against a decay floor of {decay_floor} — the decay no longer \
             matches the frame-rate-independence law, or mass left the \
             field outside the decay path"
        );
    }
}

/// INV-20 (FABRIK): the spacing ledger — every dragon chain segment
/// sits EXACTLY at DRAGON_SEGMENT_SPACING from its predecessor after
/// every constraint pass.
#[test]
fn lock_inv20_fabrik_segment_spacing_exact() {
    let mut cloud = make_dragon_cloud(Z8_COLS, Z8_LINES);
    let mut frame = crate::frame::Frame::new(Z8_COLS, Z8_LINES, cloud.palette.bg);

    // The projection is a closed-form landing: after the chain pass,
    // dist(seg[i-1], seg[i]) == spacing to f32 rounding. 300 frames
    // cross state transitions (Soar/Circle), wall bounces, and the
    // full chain lifetime.
    for _ in 0..300 {
        run_frames(&mut cloud, &mut frame, 1, Z8_STEP_MS);
        for d in &cloud.dragon_rain.dragons {
            if !d.active {
                continue;
            }
            for pair in d.segments.windows(2) {
                let dx = pair[1].x - pair[0].x;
                let dy = pair[1].y - pair[0].y;
                let dist = (dx * dx + dy * dy).sqrt();
                assert!(
                    (dist - DRAGON_SEGMENT_SPACING).abs() <= 1.0e-3,
                    "the FABRIK spacing ledger broke: segment distance {dist} \
                     against the rest length {DRAGON_SEGMENT_SPACING} — the \
                     constraint pass regressed or a new motion bypasses it"
                );
            }
        }
    }
}

/// INV-20 (boids): the flight-band ledger — every bird's speed sits
/// inside the documented band after every frame, and no bird moves
/// farther in one step than its band allows (the no-teleport
/// corollary of the band under the semi-implicit ordering).
#[test]
fn lock_inv20_boids_flight_band_and_no_teleport() {
    let mut cloud = make_murm_cloud(Z8_COLS, Z8_LINES);
    let mut frame = crate::frame::Frame::new(Z8_COLS, Z8_LINES, cloud.palette.bg);

    // The band ceiling times a 1.5 margin covers the blended sim
    // clock's variance; the band itself is exact.
    let step_ceiling = f64::from(MURM_SPEED_MAX) * f64::from(Z8_DT) * 1.5;

    for _ in 0..300 {
        let prev: Vec<(bool, f32, f32)> = cloud
            .murmuration_rain
            .birds
            .iter()
            .map(|b| (b.active, b.x, b.y))
            .collect();
        run_frames(&mut cloud, &mut frame, 1, Z8_STEP_MS);
        for (i, b) in cloud.murmuration_rain.birds.iter().enumerate() {
            if !b.active {
                continue;
            }
            let speed = (b.vx * b.vx + b.vy * b.vy).sqrt();
            assert!(
                (MURM_SPEED_MIN - 1.0e-3..=MURM_SPEED_MAX + 1.0e-3).contains(&speed),
                "bird {i} left the flight band: speed {speed} against \
                 [{MURM_SPEED_MIN}, {MURM_SPEED_MAX}] — the clamp regressed \
                 or a new impulse bypasses it"
            );
            // The no-teleport corollary pairs ACTIVE frames only: a
            // slot that just spawned legitimately jumped from its
            // dormant position, and the dormant slots carry no
            // flight contract.
            if i < prev.len() {
                let (was_active, px, py) = prev[i];
                if was_active {
                    let dx = f64::from(b.x - px);
                    let dy = f64::from(b.y - py);
                    let moved = (dx * dx + dy * dy).sqrt();
                    assert!(
                        moved <= step_ceiling,
                        "bird {i} teleported: {moved} cells in one {Z8_DT}s step \
                         against a band ceiling of {step_ceiling} — the position \
                         advance no longer runs on the clamped velocity"
                    );
                }
            }
        }
    }
}

/// INV-20 (lorenz): the absorbing-set contract — the honest scope
/// for a chaotic system.
///
/// The Lorenz flow has no monotone energy ledger (the butterfly
/// amplifies), but the standard system (sigma 10, rho 28, beta 8/3)
/// has a textbook absorbing region, and the rig's motes spawn inside
/// the saddle region. The measured envelope over this rig's window
/// (600 frames, several motes across both lobes, whole lifetimes)
/// stays far inside these pins; the margin is the contract, and a
/// breach names the real defect (a mis-scaled integration step, a
/// lost clamp, or a diverging solver). The absorption promptness
/// rides with it: no mote outlives its lifetime by more than one
/// step.
#[test]
fn lock_inv20_lorenz_absorbing_set() {
    let mut cloud = make_lorenz_cloud(Z8_COLS, Z8_LINES);
    let mut frame = crate::frame::Frame::new(Z8_COLS, Z8_LINES, cloud.palette.bg);

    // The absorbing-set pins (measured envelope + margin; see the
    // module docs — the chaotic system gets an absorbing region, not
    // an energy law).
    const ABSORB_R_XY_MAX: f32 = 40.0;
    const ABSORB_Z_MAX: f32 = 80.0;
    const ABSORB_Z_MIN: f32 = -10.0;

    for _ in 0..600 {
        run_frames(&mut cloud, &mut frame, 1, Z8_STEP_MS);
        for m in &cloud.lorenz_rain.motes {
            if !m.active {
                continue;
            }
            let r_xy = (m.x * m.x + m.y * m.y).sqrt();
            assert!(
                r_xy <= ABSORB_R_XY_MAX,
                "a lorenz mote left the absorbing set: r_xy {r_xy} against \
                 {ABSORB_R_XY_MAX} (state x {}, y {}, z {}) — the integration \
                 diverged, the step clamping regressed, or the attractor \
                 parameters changed; the absorbing set is the chaotic \
                 system's contract",
                m.x,
                m.y,
                m.z
            );
            assert!(
                m.z >= ABSORB_Z_MIN && m.z <= ABSORB_Z_MAX,
                "a lorenz mote left the absorbing set: z {} against \
                 [{ABSORB_Z_MIN}, {ABSORB_Z_MAX}] (x {}, y {}) — the \
                 integration diverged or the step clamping regressed",
                m.z,
                m.x,
                m.y
            );
            assert!(
                m.sim_age <= m.lifetime + Z8_DT * 2.0,
                "a lorenz mote outlived its absorption: sim_age {} against a \
                 lifetime of {} — the lifetime backstop regressed",
                m.sim_age,
                m.lifetime
            );
        }
    }
}

/// The family's teeth: the kepler ledger must DETECT a deliberately
/// corrupted orbit.
///
/// A tripwire that cannot fire is decoration. The same drift helper
/// that the exactness test asserts with must flag a hand-corrupted
/// velocity (a realistic regression shape: an accidental
/// non-central term, a wrong sign, a reordered update). If this
/// fires zero, the ledger measures nothing and the family's green
/// is decoration.
#[test]
fn lock_inv20_ledger_teeth_deliberate_l_violation_detected() {
    let mut m = crate::cloud::type_rain::black_hole::infall::InfallMote::vacant();
    m.active = true;
    m.x = 2.2;
    m.y = 0.0;
    m.vx = 0.15;
    m.vy = 0.6;
    let l0 = kepler_angular_momentum(m.x, m.y, m.vx, m.vy);

    for _ in 0..50 {
        let absorbed = advance_infall_mote(&mut m, Z8_DT, 100.0, 100.0);
        assert!(!absorbed);
    }
    // The deliberate regression: a non-central kick (a tangential
    // force an accidental code change could introduce).
    m.vy += 0.2;
    let l1 = kepler_angular_momentum(m.x, m.y, m.vx, m.vy);
    let drift = ((l1 - l0) / l0.abs().max(1.0)).abs();
    assert!(
        drift > KEPLER_L_REL_TOL,
        "the ledger's teeth are broken: a deliberate non-central kick \
         moved L by only {drift} relative — the drift helper cannot see a \
         real violation and the family's green is decoration"
    );
}
