// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! # Wave-field solver physics pins (night-frontier-Z4b, Z6)
//!
//! The click shockwave claims to SOLVE the 2D wave equation — these
//! tests make that claim load-bearing:
//!
//! - CFL: the solver's Courant numbers sit far inside the von Neumann
//!   stability bound at the sub-step ceiling, and a long stepped run
//!   provably never blows up.
//! - Speed + isotropy: the front's first arrival matches `c` in
//!   physical units, horizontally AND vertically (the 2:1 cell aspect
//!   law), within the coarse-grid dispersion tolerance.
//! - Damping: the amplitude envelope halves per `t_half = ln(2)/k`... —
//!   the damped wave equation's `exp(-k*t/2)` law, pinned numerically.
//! - Linearity: the wave equation's superposition property holds —
//!   two clicks' field equals the sum of the single-click fields
//!   (the honest proof that simultaneous ripples interfere).
//! - Life: the field settles below the visible floor inside the
//!   documented window and then costs zero work.
//! - Zero allocation: stepping never grows or reallocates the grids.
//! - Determinism: identical runs produce bit-identical fields.
//! - Energy ledger (night-frontier-Z6): the discrete Lyapunov
//!   functional is non-increasing under steps at the pinned schedule
//!   (to f64 rounding), every click's deposit matches the closed-form
//!   seed-packet energy, the identity `energy == injected - dissipated
//!   + rescale` holds at all times, click storms stay under the named
//!   per-cell cap, and every lifecycle ends with the ledger landing
//!   energy at exactly zero with `dissipated == injected`.

use super::super::wave_field::{WAVE_ENERGY_CAP_PER_CELL, WaveField};
use crate::constants::{MOUSE_FLASH_DT_MAX, MOUSE_FLASH_SPEED, MOUSE_FLASH_WAVE_DAMPING};

/// The CFL pin: at the solver's sub-step ceiling, `rx^2 + ry^2` must
/// sit inside the von Neumann bound (1.0) with a healthy margin, where
/// `rx = c*h` (columns) and `ry = rx/2` (rows — `dy = 2*dx`).
#[test]
fn wave_field_cfl_margin_at_substep_ceiling() {
    let rx = MOUSE_FLASH_SPEED * MOUSE_FLASH_DT_MAX;
    let ry = rx / 2.0;
    let cour = rx * rx + ry * ry;
    assert!(
        cour <= 1.0,
        "CFL violated at the sub-step ceiling: rx^2 + ry^2 = {cour}"
    );
    assert!(
        cour <= 0.25,
        "the CFL margin collapsed (rx^2 + ry^2 = {cour}): the sub-step ceiling is \
         expected to hold an ~11x margin, investigate the constants"
    );
    const _: () = assert!(MOUSE_FLASH_DT_MAX == 1.0 / 120.0);
}

/// The solver never destabilizes: a long stepped run on an armed field
/// keeps every amplitude bounded by the inject cap (CFL in action).
#[test]
fn wave_field_long_run_stays_bounded() {
    let mut f = WaveField::empty();
    f.resize(80, 24);
    f.inject(40, 12);
    for _ in 0..2000 {
        f.step(1.0 / 60.0);
        let p = f.amplitudes().iter().fold(0.0f32, |m, &v| m.max(v.abs()));
        assert!(
            p <= 2.0 + 1e-3,
            "amplitude escaped the cap mid-run ({p}) — the scheme blew up"
        );
    }
}

/// Propagation speed + isotropy: the front's significant arrival at a
/// probe point matches `distance / c` in PHYSICAL units — horizontally
/// (10 columns) and vertically (5 rows = 10 column-widths of travel,
/// the same time). The arrival threshold samples the SIGNIFICANT wave
/// (the crest at these probes carries 0.09-0.22): a tiny threshold
/// would fire on the blast seed's sub-visible dispersive precursor,
/// whose leading tail crosses milli-amplitudes frames before the
/// front (the coarse-grid numerical dispersion makes the leading edge
/// slightly late/near-field slightly fast, so the tolerance is 35% —
/// a documented property of the discretization, not a defect). The
/// ISOTROPY pin is the sharper claim: horizontal and vertical
/// arrivals must land in the same window — that is the "the ripple
/// renders as a circle" law (dy = 2*dx).
#[test]
fn wave_field_front_arrival_speed_and_isotropy() {
    let arrival = |field: &mut WaveField, probe: (usize, usize), frames: usize| -> usize {
        const ARRIVAL_THRESHOLD: f32 = 0.05;
        let mut landed_at = None;
        for k in 0..frames {
            field.step(1.0 / 60.0);
            let (cols, _) = field.dims();
            let a = field.amplitudes()[probe.1 * cols + probe.0].abs();
            if a > ARRIVAL_THRESHOLD {
                landed_at = Some(k + 1);
                break;
            }
        }
        landed_at.expect("the wave front never reached the probe")
    };
    let expected_frames = (10.0 / MOUSE_FLASH_SPEED) * 60.0; // 12.5 at c = 48

    // Horizontal: 10 columns.
    let mut f = WaveField::empty();
    f.resize(80, 24);
    f.inject(10, 12);
    let h_frames = arrival(&mut f, (20, 12), 80);
    assert!(
        ((h_frames as f32 - expected_frames) / expected_frames).abs() < 0.35,
        "horizontal arrival {h_frames} frames vs expected {expected_frames:.1} — speed law drifted"
    );

    // Vertical: 5 rows = 10 column-widths of physical travel — the SAME
    // expectation (isotropy in pixel space).
    let mut f = WaveField::empty();
    f.resize(80, 24);
    f.inject(40, 5);
    let v_frames = arrival(&mut f, (40, 10), 80);
    assert!(
        ((v_frames as f32 - expected_frames) / expected_frames).abs() < 0.35,
        "vertical arrival {v_frames} frames vs expected {expected_frames:.1} — the aspect \
         law drifted"
    );

    // Isotropy: both directions land in the same window (the circular-
    // ripple claim, robust even where absolute speed has dispersion).
    let worst = h_frames.max(v_frames) as f32;
    assert!(
        ((h_frames as i32 - v_frames as i32).abs() as f32) <= worst * 0.35,
        "horizontal ({h_frames}) and vertical ({v_frames}) arrivals diverged — the ripple \
         would render as an ellipse"
    );
}

/// The damping law: the peak amplitude envelope decays as
/// `exp(-k*t/2)` — the damped wave equation's analytical envelope —
/// pinned across one and two half-lives (`t_half = 2*ln(2)/k` for the
/// envelope: the amplitude halves each half-life).
#[test]
fn wave_field_damping_envelope_law() {
    let t_half = 2.0 * std::f32::consts::LN_2 / MOUSE_FLASH_WAVE_DAMPING;
    let mut f = WaveField::empty();
    f.resize(60, 20);
    f.inject(30, 10);
    let peak = |f: &WaveField| f.amplitudes().iter().fold(0.0f32, |m, &v| m.max(v.abs()));
    // Step to one envelope half-life (60 fps).
    let frames = (t_half * 60.0).round() as usize;
    for _ in 0..frames {
        f.step(1.0 / 60.0);
    }
    let p1 = peak(&f);
    for _ in 0..frames {
        f.step(1.0 / 60.0);
    }
    let p2 = peak(&f);
    // The front disperses while damping (energy spreads across cells),
    // so the peak decay runs slightly FASTER than the pure envelope;
    // pin the law with the envelope as the ceiling and a strict decay
    // as the floor: p2 must be at most p1/2 (the law's ceiling), and
    // both must be strictly decaying and below the inject start.
    assert!(
        p2 <= p1 * 0.5 + 0.02,
        "the envelope did not halve across one half-life ({p1} -> {p2}) — damping law drifted"
    );
    assert!(
        p1 < 1.0,
        "the peak must decay from the inject level (got {p1})"
    );
}

/// Superposition (the linearity proof): the field of two clicks equals
/// the sum of the fields of each click alone, within float tolerance —
/// the wave equation's defining linear property, and the honest reason
/// simultaneous ripples interfere.
#[test]
fn wave_field_superposition_is_linear() {
    let run = |clicks: &[(u16, u16)]| -> (WaveField, Vec<f32>) {
        let mut f = WaveField::empty();
        f.resize(60, 20);
        for &(c, l) in clicks {
            f.inject(c, l);
        }
        for _ in 0..40 {
            f.step(1.0 / 60.0);
        }
        let snapshot = f.amplitudes().to_vec();
        (f, snapshot)
    };
    let (_, single_a) = run(&[(15, 10)]);
    let (_, single_b) = run(&[(45, 10)]);
    let (both, joint) = run(&[(15, 10), (45, 10)]);
    assert!(!both.is_settled());
    for i in 0..joint.len() {
        let expected = single_a[i] + single_b[i];
        assert!(
            (joint[i] - expected).abs() < 1e-4,
            "superposition broken at cell {i}: joint {} != sum {} — the solver is not \
             solving a linear wave equation",
            joint[i],
            expected
        );
    }
}

/// Settle lifecycle: fresh field settled → inject unsettles → stepping
/// decays to settled inside the documented window → stepping after
/// settle is a no-op (zero cost) and a fresh inject re-arms.
#[test]
fn wave_field_settle_lifecycle_and_zero_cost() {
    let mut f = WaveField::empty();
    f.resize(80, 24);
    assert!(f.is_settled());
    f.inject(40, 12);
    assert!(!f.is_settled());
    // Documented life: the blast seed lands at the A0 = 2 cap, so the
    // provable settle ceiling is 2*ln(2/eps)/k ~ 6.9 s at the
    // water-ripple retune's k = 1.8/s (~415 frames) — the
    // pure-envelope bound. On THIS 80x24 grid the Mur edges bleed the
    // ring and the geometric spreading collapses the peak, so the
    // field settles far earlier (measured ~110 frames on the exact
    // solver replica); 300 frames stays a comfortable margin.
    for _ in 0..300 {
        f.step(1.0 / 60.0);
        if f.is_settled() {
            break;
        }
    }
    assert!(
        f.is_settled(),
        "the field must settle inside the documented damping window"
    );
    // Post-settle steps are no-ops: stepping a settled field does
    // nothing (cheap to prove — the buffer stays exactly zero).
    f.step(1.0 / 60.0);
    let all_zero = f.amplitudes().iter().all(|&v| v == 0.0);
    assert!(all_zero, "a settled field must hold exactly zero energy");
    // A fresh click re-arms.
    f.inject(10, 5);
    assert!(!f.is_settled(), "a new click must re-arm the settled field");
}

/// Zero allocation in the hot path: after resize, the grids never
/// grow — length is fixed through thousands of steps and the settle
/// lifecycle holds (the zero-alloc ledger's law for this surface).
#[test]
fn wave_field_stepping_allocates_nothing() {
    let mut f = WaveField::empty();
    f.resize(80, 24);
    let n = 80 * 24;
    f.inject(40, 12);
    for _ in 0..3000 {
        f.step(1.0 / 60.0);
    }
    assert_eq!(
        f.amplitudes().len(),
        n,
        "the field's length changed during stepping — an allocation happened"
    );
    // And the settle lifecycle held through the whole run.
    assert!(
        f.is_settled(),
        "3000 frames (50 s) must exceed the wave's life"
    );
}

/// Determinism: two fields built identically (same injects, same step
/// schedule) produce bit-identical snapshots — the solver is pure
/// float arithmetic on preallocated grids, no clocks, no RNG.
#[test]
fn wave_field_is_deterministic() {
    let build = || -> Vec<f32> {
        let mut f = WaveField::empty();
        f.resize(50, 18);
        f.inject(10, 9);
        f.inject(35, 9);
        for _ in 0..120 {
            f.step(1.0 / 60.0);
        }
        f.amplitudes().to_vec()
    };
    let a = build();
    let b = build();
    assert_eq!(
        a, b,
        "two identical runs diverged — the solver is not deterministic"
    );
}

/// Inject clamps out-of-range clicks into the grid (a click past the
/// edge is a valid impulse at the edge cell, never a panic).
#[test]
fn wave_field_inject_clamps_to_bounds() {
    let mut f = WaveField::empty();
    f.resize(20, 10);
    f.inject(9999, 9999);
    let (cols, lines) = f.dims();
    assert!(
        f.amplitudes()[(lines - 1) * cols + (cols - 1)] > 0.0,
        "an out-of-range click must clamp into the grid corner"
    );
    f.inject(0, 0);
    assert!(
        f.amplitudes()[0] > 0.0,
        "a corner-origin click must inject at (0,0)"
    );
}

/// Resize is a one-shot surface: resizing to the same dims is a no-op
/// that does not clear an in-flight wave's buffers twice... — actually
/// it clears (documented); the no-op case is the SAME-dims resize,
/// which must not reallocate (capacity check via len stability across
/// a resize round-trip).
#[test]
fn wave_field_resize_round_trip_stable() {
    let mut f = WaveField::empty();
    f.resize(40, 12);
    let n = 40 * 12;
    assert_eq!(f.amplitudes().len(), n);
    f.inject(20, 6);
    // Same-dims resize: keeps the grid (len unchanged), clears the wave.
    f.resize(40, 12);
    assert_eq!(
        f.amplitudes().len(),
        n,
        "same-dims resize must not reallocate"
    );
    // Different dims: new grid size.
    f.resize(60, 20);
    assert_eq!(f.amplitudes().len(), 60 * 20);
    assert!(f.is_settled(), "a resize clears the field");
}

// ── night-frontier-Z6, S4: the energy ledger ───────────────────────────────
//
// The wave equation bound becomes a NAMED quantity: the discrete
// Lyapunov functional of the three-level leapfrog scheme (kinetic at
// level n-1/2, cross-time-level gradient products), carried by an
// explicit ledger with signed injection, dissipation, and rescale
// buckets. These tests make every ledger law load-bearing.

/// Reference Courant numbers of the ledger scale (the sub-step
/// ceiling): `LEDGER_RX = c * MOUSE_FLASH_DT_MAX`, `LEDGER_RY = rx/2`.
const LEDGER_RX: f64 = (MOUSE_FLASH_SPEED * MOUSE_FLASH_DT_MAX) as f64;
const LEDGER_RY: f64 = LEDGER_RX / 2.0;

/// The identity law: `energy == injected - dissipated + rescale`
/// holds exactly (f64) at every point of any lifecycle — every energy
/// movement is attributed to exactly one named bucket.
fn assert_ledger_identity(f: &WaveField) {
    let l = f.ledger();
    let book = l.injected() - l.dissipated() + l.rescale();
    assert!(
        (l.energy() - book).abs() <= 1e-9,
        "ledger identity broken: energy {} != injected {} - dissipated {} + rescale {}",
        l.energy(),
        l.injected(),
        l.dissipated(),
        l.rescale()
    );
}

/// The closed-form deposit: one interior click's blast packet (center
/// 2.0 = the cap, four neighbors 1.0, eight manhattan-2 cells 0.5,
/// zero initial velocity) raises the functional by exactly
/// `0.5*rx^2*6 + 0.5*ry^2*6`: per direction the packet touches 18
/// edges — two across the 1.0 center-to-ring step and sixteen across
/// the 0.5 steps, summing 6.0 squared units (two of 1.0 squared,
/// sixteen of 0.25 squared), and the two directions are symmetric.
/// Kinetic is exactly zero since cur == prev. The 1e-7 slack is the
/// f32 rounding of the Courant product (16/225 and 4/225 are the
/// exact f64 values).
#[test]
fn wave_field_energy_inject_deposit_matches_closed_form() {
    let mut f = WaveField::empty();
    f.resize(80, 24);
    f.inject(40, 12);
    let l = f.ledger();
    let expected = 0.5 * LEDGER_RX * LEDGER_RX * 6.0 + 0.5 * LEDGER_RY * LEDGER_RY * 6.0;
    assert!(
        (l.injected() - expected).abs() <= 1e-7,
        "the click's energy deposit drifted from the closed form: {} vs {}",
        l.injected(),
        expected
    );
    assert_eq!(
        l.energy(),
        l.injected(),
        "a fresh field's energy is the deposit"
    );
    assert_eq!(l.rescale(), 0.0, "no frames stepped, no rescale");
    assert!(l.energy() > 0.0, "a click must deposit positive energy");
    assert_eq!(l.substeps(), 0, "no quantum has run yet");
}

/// The Lyapunov law at the pinned schedule: with one click and 600
/// frames at 60 fps (every sub-step at exactly MOUSE_FLASH_DT_MAX), the
/// per-frame functional delta is non-positive to f64 rounding — the
/// solver NEVER creates energy — and the lifecycle ends at exactly
/// zero with every deposited unit accounted as dissipated.
#[test]
fn wave_field_energy_is_lyapunov_under_steps() {
    let mut f = WaveField::empty();
    f.resize(80, 24);
    f.inject(40, 12);
    let injected = f.ledger().injected();
    assert!(injected > 0.0);
    let mut e_prev = f.ledger().energy();
    let mut worst_up = 0.0f64;
    for _ in 0..600 {
        f.step(1.0 / 60.0);
        let e = f.ledger().energy();
        if e - e_prev > worst_up {
            worst_up = e - e_prev;
        }
        assert_ledger_identity(&f);
        e_prev = e;
    }
    assert!(
        worst_up <= 1e-9,
        "the solver created energy: worst per-frame delta = {worst_up:e} — the \
         Lyapunov law failed at the pinned schedule"
    );
    let l = f.ledger();
    assert_eq!(
        l.energy(),
        0.0,
        "the settled field must hold exactly zero energy"
    );
    assert!(
        (l.dissipated() - l.injected()).abs() <= 1e-9,
        "the lifecycle must account every deposited unit: dissipated {} vs injected {}",
        l.dissipated(),
        l.injected()
    );
    assert!(l.frames() > 50, "the active frames must be accounted");
    assert_eq!(
        l.peak(),
        injected,
        "the peak is the fresh deposit (decays from there)"
    );
}

/// The cap law under a click storm: 200 clicks per frame for 200
/// frames — the functional NEVER exceeds the named per-cell cap (the
/// rigorous value bound: every term is a difference of amplitudes each
/// pinned under INJECT_CAP, so `E <= 2.5 * cap^2` per cell at ANY
/// pacing under ANY storm), and the field still settles to exactly
/// zero afterwards with the ledger balanced.
#[test]
fn wave_field_energy_click_storm_stays_under_cap() {
    let mut f = WaveField::empty();
    f.resize(80, 24);
    let n_cells = (80 * 24) as f64;
    let cap = WAVE_ENERGY_CAP_PER_CELL * n_cells;
    // Deterministic LCG positions (no RNG dependency).
    let mut state: u64 = 0x9E3779B97F4A7C15;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..200 {
        for _ in 0..200 {
            let c = (next() % 80) as u16;
            let l = (next() % 24) as u16;
            f.inject(c, l);
        }
        f.step(1.0 / 60.0);
        assert_ledger_identity(&f);
        let e = f.ledger().energy();
        assert!(
            e <= cap,
            "the storm breached the energy cap: {e} > {cap} — the value-bound law \
             or the inject cap failed"
        );
    }
    let l = f.ledger();
    assert!(l.peak() <= cap, "the session peak must respect the cap");
    assert!(l.injected() > 0.0, "a storm must deposit energy");
    assert!(l.energy() > 0.0, "the storm's field is still alive");
    // The decay: 900 more frames must land exactly zero, fully
    // balanced (the water-ripple retune's k = 1.8/s tail is longer
    // than v1's — the provable settle ceiling is 2*ln(2/eps)/k ~ 6.9 s
    // ~ 415 frames, and the saturated storm's DC residue rides the
    // critically damped mode, so 900 gives the sweep margin).
    for _ in 0..900 {
        f.step(1.0 / 60.0);
    }
    let l = f.ledger();
    assert_eq!(
        l.energy(),
        0.0,
        "the storm field must settle to exactly zero"
    );
    assert!(
        (l.dissipated() - l.injected()).abs() <= 1e-6,
        "the storm lifecycle must balance: dissipated {} vs injected {}",
        l.dissipated(),
        l.injected()
    );
}

/// The rescale law: the quantum clock keeps ONE sub-step width for the
/// whole session — the measurement scale never shifts, so `rescale`
/// stays exactly zero at any frame pacing, and the decay still lands
/// at exactly zero under pacing that never completes a quantum.
#[test]
fn wave_field_energy_rescale_is_zero_under_any_pacing() {
    let mut f = WaveField::empty();
    f.resize(80, 24);
    f.inject(40, 12);
    // Pathological pacing mix (the alternating-h cycle that RESONATED
    // under the old fractional sub-step widths — the quantum clock
    // must make it a plain decay).
    let dts = [1.0 / 60.0, 1.0 / 45.0, 1.0 / 30.0, 1.0 / 90.0];
    let mut worst_up = 0.0f64;
    let mut e_prev = f.ledger().energy();
    for k in 0..600 {
        f.step(dts[k % dts.len()]);
        assert_ledger_identity(&f);
        let cap = WAVE_ENERGY_CAP_PER_CELL * (80.0 * 24.0);
        let e = f.ledger().energy();
        assert!(
            e <= cap,
            "the cap law must hold under any pacing (got {e} at frame {k})"
        );
        if e - e_prev > worst_up {
            worst_up = e - e_prev;
        }
        e_prev = e;
    }
    let l = f.ledger();
    assert_eq!(
        l.rescale(),
        0.0,
        "the quantum clock must keep the measurement scale fixed — rescale \
         stays exactly zero"
    );
    assert!(
        worst_up <= 1e-9,
        "the alternating-pacing cycle must not create energy (worst delta \
         {worst_up:e}) — the quantum clock failed to pin the schedule"
    );
    assert_eq!(
        l.energy(),
        0.0,
        "the decay law holds under pathological pacing too — exactly zero at settle"
    );
}

/// The quantum clock: frame deltas smaller than one quantum accumulate
/// (10 half-quantum frames = exactly 5 sub-steps), larger deltas run
/// multiple quanta (3 double-quantum frames = exactly 6 sub-steps),
/// and a pathological stall is capped at 8 quanta per frame while the
/// remainder carries over (no sim-time is invented or lost beyond the
/// documented stall bound).
#[test]
fn wave_field_energy_quantum_clock_accounts_exact_quanta() {
    let mut f = WaveField::empty();
    f.resize(80, 24);
    f.inject(40, 12);
    let quantum = MOUSE_FLASH_DT_MAX;
    for _ in 0..10 {
        f.step(quantum / 2.0);
    }
    assert_eq!(
        f.ledger().substeps(),
        5,
        "ten half-quantum frames must run exactly five sub-steps"
    );
    for _ in 0..3 {
        f.step(quantum * 2.0);
    }
    assert_eq!(
        f.ledger().substeps(),
        11,
        "three double-quantum frames must add exactly six sub-steps"
    );
    // The stall cap: a 100-quantum frame runs 8 and carries the rest.
    f.step(quantum * 100.0);
    assert_eq!(
        f.ledger().substeps(),
        19,
        "a pathological stall must cap at 8 quanta in one frame"
    );
    assert_ledger_identity(&f);
}

/// Lifecycle reset: `clear()` (scene switch / resize surfaces) starts a
/// fresh ledger — all buckets exactly zero, no history carried.
#[test]
fn wave_field_energy_ledger_resets_on_clear() {
    let mut f = WaveField::empty();
    f.resize(80, 24);
    f.inject(40, 12);
    for _ in 0..30 {
        f.step(1.0 / 60.0);
    }
    assert!(f.ledger().injected() > 0.0 && f.ledger().frames() > 0);
    f.clear();
    let l = f.ledger();
    assert_eq!(l.energy(), 0.0);
    assert_eq!(l.injected(), 0.0);
    assert_eq!(l.dissipated(), 0.0);
    assert_eq!(l.rescale(), 0.0);
    assert_eq!(l.peak(), 0.0);
    assert_eq!(l.frames(), 0);
    // And a fresh click re-arms both the field and the ledger.
    f.inject(10, 5);
    assert!(f.ledger().injected() > 0.0);
    assert!(!f.is_settled());
}
