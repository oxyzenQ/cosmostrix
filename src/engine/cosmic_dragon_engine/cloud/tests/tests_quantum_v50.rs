// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! v50 quantum regression tests: ripple edge-bounce stabilization +
//! masterclass brightness/lifespan retune.
//!
//! Extracted from `tests_quantum.rs` to keep that source file under the
//! 800-LOC cap. Pure code motion — no behavior change.

use std::time::{Duration, Instant};

use crossterm::style::Color;

use super::super::Cloud;
use super::tests_quantum::{brighten_color, make_truecolor_cloud};
use crate::cloud::interpolate_palette_color;
use crate::constants::{
    QUANTUM_BODY_TONE_DOWN, QUANTUM_RIPPLE_BOUNCE_DAMPING, QUANTUM_RIPPLE_HEAD_END_FRAC,
    QUANTUM_RIPPLE_LIFETIME_SECS, QUANTUM_RIPPLE_PARTICLE_COUNT, QUANTUM_RIPPLE_POOL_SIZE,
    QUANTUM_RIPPLE_SPEED, QUANTUM_RIPPLE_TAIL_START_FRAC,
};
use crate::frame::Frame;
use crate::palette::decode_color;
use crate::runtime::ColorScheme;

// ─── v50 stabilization: quantum ripple edge-bounce regression tests ────────
//
// Owner requested that quantum ripple particles BOUNCE off the four screen
// edges instead of dying on border crossing. Previously, a particle that
// crossed `col >= cols` or `line >= lines` was immediately deactivated —
// on small viewports (or clicks near an edge) this clipped the burst and
// most of the cohort expired within the first few frames.
//
// The new behavior: position is mirrored across the offending edge AND
// the crossed-axis velocity is reflected with a damping factor
// (`QUANTUM_RIPPLE_BOUNCE_DAMPING`). Perpendicular velocity is untouched.
//
// These tests verify each of the four edges individually, plus the
// interaction between bouncing and the age-based lifespan expiry.

/// Sanity: the bounce damping constant sits in a perceptible range.
/// Below 0.5 the second bounce dies too quickly; above 0.95 the cohort
/// ricochets nearly elastically within the 2.5s lifespan, which feels
/// chaotic. The sweet spot is [0.7, 0.9].
#[test]
fn quantum_bounce_damping_constant_in_sweet_spot() {
    assert!(
        (0.7..=0.95).contains(&QUANTUM_RIPPLE_BOUNCE_DAMPING),
        "QUANTUM_RIPPLE_BOUNCE_DAMPING = {QUANTUM_RIPPLE_BOUNCE_DAMPING} is outside [0.7, 0.95] \
         — if this was intentional, update the rationale in the constant's doc comment"
    );
    // Compile-time guard: damping must be strictly positive (else a bounce
    // would freeze the particle on the edge) and at most 1.0 (else a bounce
    // would AMPLIFY the velocity, violating energy conservation).
    const _: () = assert!(QUANTUM_RIPPLE_BOUNCE_DAMPING > 0.0);
    const _: () = assert!(QUANTUM_RIPPLE_BOUNCE_DAMPING <= 1.0);
}

/// Helper: spawn a click and pin ONE active particle to a deterministic
/// position + velocity so the bounce can be verified exactly. Returns
/// the index of the pinned particle in `quantum_particles`.
///
/// All OTHER particles spawned by the click are DEACTIVATED so they
/// don't interfere with the pinned particle's render (e.g., overlapping
/// at the same cell and cascading blend reads). This is essential for
/// brightness-curve tests where we pre-set the cell's base color and
/// need the pinned particle to be the ONLY one reading that base.
///
/// `col`/`line` are the cell coordinates (u16, matching
/// `Cloud::set_mouse_click`); internally the particle's float position
/// is set to `col + 0.5` / `line + 0.5` to mirror `spawn_quantum_ripple`'s
/// `cx = col as f32 + 0.5` convention.
pub(super) fn pin_one_particle(
    cloud: &mut Cloud,
    col: u16,
    line: u16,
    vx: f32,
    vy: f32,
    spawn_time: Instant,
) -> usize {
    cloud.set_mouse_click(col, line);
    // The pool is now populated with up to QUANTUM_RIPPLE_PARTICLE_COUNT
    // active particles. Pin the first one to deterministic values, then
    // DEACTIVATE all others so they don't cascade-blend over the pinned
    // particle's cell.
    let idx = cloud
        .quantum_particles
        .iter()
        .position(|p| p.active)
        .expect("at least one particle must be active after click");
    let p = &mut cloud.quantum_particles[idx];
    p.x = col as f32 + 0.5; // mirror spawn_quantum_ripple's cx = col + 0.5 convention
    p.y = line as f32 + 0.5;
    p.vx = vx;
    p.vy = vy;
    p.birth = spawn_time; // reset birth so age is small at frame_time
                          // Anchor the quantum update clock so dt is deterministic.
    cloud.last_quantum_update_time = spawn_time;
    cloud.last_phosphor_time = spawn_time;
    // Deactivate all other active particles so only the pinned one
    // renders. Without this, 20 overlapping particles at (col, line)
    // would cascade-blend: the 2nd particle reads the 1st's output as
    // cell.fg, the 3rd reads the 2nd's output, etc. — converging to
    // the particle snapshot color regardless of brightness.
    let mut deactivated = 0usize;
    for (i, other) in cloud.quantum_particles.iter_mut().enumerate() {
        if i != idx && other.active {
            other.active = false;
            deactivated += 1;
        }
    }
    cloud.quantum_active_count = cloud.quantum_active_count.saturating_sub(deactivated);
    idx
}

/// Step the cloud forward by one frame and return. Wraps the standard
/// rain_at call with the timestamp bookkeeping the bounce tests need.
fn step_one_frame(cloud: &mut Cloud, frame: &mut Frame, frame_time: Instant) {
    cloud.rain_at(frame, frame_time);
}

/// Right edge: a particle moving +x must reflect to -x with damping,
/// and its position must be mirrored back inside bounds.
#[test]
fn quantum_particle_bounces_off_right_edge() {
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    let spawn_time = Instant::now();
    // Cloud is 20×10 → max_x = 19. Spawn at (17, 5), push particle
    // +x at 100 cells/sec for 1/30 sec → Δx = 3.33 → reaches x ≈ 20.83
    // → overshoots max_x=19 by ~1.83 → mirrors to 19 - 1.83 ≈ 17.17.
    let idx = pin_one_particle(&mut cloud, 17, 5, 100.0, 0.0, spawn_time);
    let pre_vx = cloud.quantum_particles[idx].vx;

    let frame_time = spawn_time + Duration::from_millis(33);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    step_one_frame(&mut cloud, &mut frame, frame_time);

    let p = &cloud.quantum_particles[idx];
    assert!(
        p.active,
        "particle must still be active after a single bounce"
    );
    assert!(
        p.vx < 0.0,
        "vx must be NEGATIVE after bouncing off the right edge, got {}",
        p.vx
    );
    let expected_mag = pre_vx.abs() * QUANTUM_RIPPLE_BOUNCE_DAMPING;
    let actual_mag = p.vx.abs();
    assert!(
        (actual_mag - expected_mag).abs() < 2.0,
        "post-bounce |vx|={} should equal pre-bounce |vx|={} * damping={} ≈ {}, within ±2.0 (velocity decay tolerance)",
        actual_mag,
        pre_vx.abs(),
        QUANTUM_RIPPLE_BOUNCE_DAMPING,
        expected_mag
    );
    // Position must be strictly inside bounds (max_x = 19).
    assert!(
        p.x <= 19.0,
        "x must be inside bounds after bounce, got {}",
        p.x
    );
    // vy is untouched (perpendicular to the bounced axis).
    assert!(
        p.vy.abs() < 0.001,
        "vy must be unchanged (perpendicular axis untouched), got {}",
        p.vy
    );
}

/// Left edge: a particle moving -x must reflect to +x with damping.
#[test]
fn quantum_particle_bounces_off_left_edge() {
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    let spawn_time = Instant::now();
    // Spawn at (2, 5), push -x at 100 cells/sec → reaches x ≈ -0.83
    // → overshoots 0 by ~0.83 → mirrors to 0 + 0.83 ≈ 0.83.
    let idx = pin_one_particle(&mut cloud, 2, 5, -100.0, 0.0, spawn_time);
    let pre_vx = cloud.quantum_particles[idx].vx;

    let frame_time = spawn_time + Duration::from_millis(33);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    step_one_frame(&mut cloud, &mut frame, frame_time);

    let p = &cloud.quantum_particles[idx];
    assert!(
        p.active,
        "particle must still be active after a single bounce"
    );
    assert!(
        p.vx > 0.0,
        "vx must be POSITIVE after bouncing off the left edge, got {}",
        p.vx
    );
    let expected_mag = pre_vx.abs() * QUANTUM_RIPPLE_BOUNCE_DAMPING;
    let actual_mag = p.vx.abs();
    assert!(
        (actual_mag - expected_mag).abs() < 2.0,
        "post-bounce |vx|={} should equal pre-bounce |vx|={} * damping={} ≈ {}, within ±2.0 (velocity decay tolerance)",
        actual_mag,
        pre_vx.abs(),
        QUANTUM_RIPPLE_BOUNCE_DAMPING,
        expected_mag
    );
    assert!(
        p.x >= 0.0,
        "x must be inside bounds after bounce, got {}",
        p.x
    );
    assert!(
        p.vy.abs() < 0.001,
        "vy must be unchanged (perpendicular axis untouched), got {}",
        p.vy
    );
}

/// Bottom edge: a particle moving +y must reflect to -y with damping.
#[test]
fn quantum_particle_bounces_off_bottom_edge() {
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    let spawn_time = Instant::now();
    // Cloud is 20×10 → max_y = 9. Spawn at (5, 7), push +y at 100
    // cells/sec for 1/30 sec → Δy = 3.33 → reaches y ≈ 10.83 → overshoots
    // max_y=9 by ~1.83 → mirrors to 9 - 1.83 ≈ 7.17.
    let idx = pin_one_particle(&mut cloud, 5, 7, 0.0, 100.0, spawn_time);
    let pre_vy = cloud.quantum_particles[idx].vy;

    let frame_time = spawn_time + Duration::from_millis(33);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    step_one_frame(&mut cloud, &mut frame, frame_time);

    let p = &cloud.quantum_particles[idx];
    assert!(
        p.active,
        "particle must still be active after a single bounce"
    );
    assert!(
        p.vy < 0.0,
        "vy must be NEGATIVE after bouncing off the bottom edge, got {}",
        p.vy
    );
    let expected_mag = pre_vy.abs() * QUANTUM_RIPPLE_BOUNCE_DAMPING;
    let actual_mag = p.vy.abs();
    assert!(
        (actual_mag - expected_mag).abs() < 2.0,
        "post-bounce |vy|={} should equal pre-bounce |vy|={} * damping={} ≈ {}, within ±2.0 (velocity decay tolerance)",
        actual_mag,
        pre_vy.abs(),
        QUANTUM_RIPPLE_BOUNCE_DAMPING,
        expected_mag
    );
    assert!(
        p.y <= 9.0,
        "y must be inside bounds after bounce, got {}",
        p.y
    );
    assert!(
        p.vx.abs() < 0.001,
        "vx must be unchanged (perpendicular axis untouched), got {}",
        p.vx
    );
}

/// Top edge: a particle moving -y must reflect to +y with damping.
#[test]
fn quantum_particle_bounces_off_top_edge() {
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    let spawn_time = Instant::now();
    // Spawn at (5, 2), push -y at 100 cells/sec → reaches y ≈ -0.83
    // → overshoots 0 by ~0.83 → mirrors to 0 + 0.83 ≈ 0.83.
    let idx = pin_one_particle(&mut cloud, 5, 2, 0.0, -100.0, spawn_time);
    let pre_vy = cloud.quantum_particles[idx].vy;

    let frame_time = spawn_time + Duration::from_millis(33);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    step_one_frame(&mut cloud, &mut frame, frame_time);

    let p = &cloud.quantum_particles[idx];
    assert!(
        p.active,
        "particle must still be active after a single bounce"
    );
    assert!(
        p.vy > 0.0,
        "vy must be POSITIVE after bouncing off the top edge, got {}",
        p.vy
    );
    let expected_mag = pre_vy.abs() * QUANTUM_RIPPLE_BOUNCE_DAMPING;
    let actual_mag = p.vy.abs();
    assert!(
        (actual_mag - expected_mag).abs() < 2.0,
        "post-bounce |vy|={} should equal pre-bounce |vy|={} * damping={} ≈ {}, within ±2.0 (velocity decay tolerance)",
        actual_mag,
        pre_vy.abs(),
        QUANTUM_RIPPLE_BOUNCE_DAMPING,
        expected_mag
    );
    assert!(
        p.y >= 0.0,
        "y must be inside bounds after bounce, got {}",
        p.y
    );
    assert!(
        p.vx.abs() < 0.001,
        "vx must be unchanged (perpendicular axis untouched), got {}",
        p.vx
    );
}

/// Bouncing must NOT extend a particle's lifespan. The age-based expiry
/// (`age >= QUANTUM_RIPPLE_LIFETIME_SECS`) is the only death condition
/// now that border-crossing no longer deactivates particles. Verify a
/// particle that bounces repeatedly still deactivates once its age
/// crosses the lifespan threshold.
#[test]
fn quantum_bounce_does_not_extend_lifespan() {
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    let spawn_time = Instant::now();
    // Pin a fast-moving particle so it definitely bounces during its life.
    // Speed = 4x the default (18 → 72) to guarantee multiple bounces.
    let idx = pin_one_particle(
        &mut cloud,
        10,
        5,
        QUANTUM_RIPPLE_SPEED * 4.0,
        QUANTUM_RIPPLE_SPEED * 4.0,
        spawn_time,
    );

    // S-master-HUNT-21/22: sim_age accumulates the real-time dt (now
    // bounded by PARTICLE_MAX_FRAME_DT_SECS) — loop
    // until the particle expires (bouncing doesn't extend sim_age).
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let mut t = spawn_time;
    while cloud.quantum_active_count > 0 {
        t += Duration::from_millis(33);
        cloud.apply_quantum_ripple(&mut frame, t);
    }

    let p = &cloud.quantum_particles[idx];
    assert!(
        !p.active,
        "particle must deactivate once age exceeds LIFETIME_SECS, even after bouncing — got active=true at x={}, y={}",
        p.x, p.y
    );
    assert_eq!(
        cloud.quantum_active_count, 0,
        "active_count counter must reach zero once all particles expire"
    );
}

/// A particle that bounces off a corner (both axes overshoot in one
/// frame) must reflect BOTH velocity components. This is the
/// stress-test edge case: a fast particle heading into a corner.
#[test]
fn quantum_particle_bounces_off_corner_both_axes() {
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    let spawn_time = Instant::now();
    // Spawn near the bottom-right corner (17, 7) with a fast (+x, +y)
    // velocity. Both axes overshoot in the same frame.
    let idx = pin_one_particle(&mut cloud, 17, 7, 100.0, 100.0, spawn_time);
    let pre_vx = cloud.quantum_particles[idx].vx;
    let pre_vy = cloud.quantum_particles[idx].vy;

    let frame_time = spawn_time + Duration::from_millis(33);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    step_one_frame(&mut cloud, &mut frame, frame_time);

    let p = &cloud.quantum_particles[idx];
    assert!(
        p.active,
        "particle must still be active after corner bounce"
    );
    assert!(
        p.vx < 0.0 && p.vy < 0.0,
        "both vx and vy must be NEGATIVE after a corner bounce, got vx={}, vy={}",
        p.vx,
        p.vy
    );
    // Both axes damped independently.
    let expected_vx_mag = pre_vx.abs() * QUANTUM_RIPPLE_BOUNCE_DAMPING;
    let expected_vy_mag = pre_vy.abs() * QUANTUM_RIPPLE_BOUNCE_DAMPING;
    assert!(
        (p.vx.abs() - expected_vx_mag).abs() < 2.0,
        "post-corner-bounce |vx|={} should ≈ {}, within ±2.0 (velocity decay tolerance)",
        p.vx.abs(),
        expected_vx_mag
    );
    assert!(
        (p.vy.abs() - expected_vy_mag).abs() < 2.0,
        "post-corner-bounce |vy|={} should ≈ {}, within ±2.0 (velocity decay tolerance)",
        p.vy.abs(),
        expected_vy_mag
    );
    assert!(
        p.x <= 19.0 && p.y <= 9.0,
        "position must be inside bounds after corner bounce, got x={}, y={}",
        p.x,
        p.y
    );
}

// ─── v50 masterclass retune: brightness curve + lifespan regression tests ──
//
// Owner feedback 8/10 reported two issues:
//  1. "dies too fast" — particles died too fast (0.8s lifespan).
//  2. "not smooth when particles move" — motion felt jerky,
//     partially caused by the `fade*fade` quadratic brightness curve
//     spending 50% of lifespan below 25% brightness (so the particle
//     was effectively invisible for the second half).
//
// The retune:
//  - Lifespan 0.8s → 2.5s (the "a few seconds masterclass" request).
//  - Speed 18 -> 9 -> 12 -> 30 cells/sec (smooth cell transitions at 60 FPS).
//  - Spawn speed variance 0.8..1.2 → 0.9..1.1 (coherent cohort).
//  - Bounce damping 0.85 → 0.78 (more deceleration over longer life).
//  - Brightness curve `fade*fade` → HEAD/BODY/TAIL three-segment fade.
//
// These tests guard the new behavior.

/// Sanity: lifespan must be in the "masterclass" range. Below 1.5s the
/// effect reads as a flicker; above 3.5s it lingers as visual noise
/// after the user has moved on. The sweet spot is [2.0, 3.0].
#[test]
fn quantum_lifespan_constant_in_masterclass_range() {
    assert!(
        (3.0..=5.0).contains(&QUANTUM_RIPPLE_LIFETIME_SECS),
        "QUANTUM_RIPPLE_LIFETIME_SECS = {QUANTUM_RIPPLE_LIFETIME_SECS} is outside [3.0, 5.0] \
         — owner requested 'a few seconds masterclass then fade out gone'"
    );
    // Compile-time guards: lifespan must be positive and at most 10s
    // (beyond 10s the pool exhaustion risk during rapid clicks becomes
    // unacceptable — 10s / 96 slots / 20-per-click = ~5 overlapping
    // clicks tolerated, which is the human maximum click rate anyway).
    const _: () = assert!(QUANTUM_RIPPLE_LIFETIME_SECS > 0.0);
    const _: () = assert!(QUANTUM_RIPPLE_LIFETIME_SECS <= 10.0);
}

/// Sanity: speed must be in the "smooth terminal motion" range.
/// Terminal rendering is discrete (1 cell = minimum unit), so the
/// visual smoothness depends on how often the particle crosses a cell
/// boundary per frame. At 60 FPS:
///   - Below 12 cells/sec: particle stays in same cell for 5+ frames (stutter)
///   - 12-30 cells/sec: cell transition every 2-5 frames (acceptable)
///   - 30-60 cells/sec: cell transition every 1-2 frames (smooth)
///   - Above 60 cells/sec: 1+ cells/frame (blur)
///   - The valid range is [12, 60] — wide enough to cover both conservative
///     and aggressive tuning without allowing blur-inducing speeds.
#[test]
fn quantum_speed_constant_in_smooth_drift_range() {
    assert!(
        (12.0..=60.0).contains(&QUANTUM_RIPPLE_SPEED),
        "QUANTUM_RIPPLE_SPEED = {QUANTUM_RIPPLE_SPEED} is outside [12, 60] \
         — must produce smooth cell transitions at 60 FPS"
    );
    const _: () = assert!(QUANTUM_RIPPLE_SPEED > 0.0);
}

/// Sanity: HEAD_END must precede TAIL_START, both within [0, 1], and
/// the BODY segment (between them) must be wide enough to read as a
/// smooth ramp (at least 30% of life).
#[test]
fn quantum_brightness_curve_segments_well_ordered() {
    const _: () = assert!(
        QUANTUM_RIPPLE_HEAD_END_FRAC > 0.0 && QUANTUM_RIPPLE_HEAD_END_FRAC < 1.0,
        "HEAD_END_FRAC must be in (0, 1)"
    );
    const _: () = assert!(
        QUANTUM_RIPPLE_TAIL_START_FRAC > 0.0 && QUANTUM_RIPPLE_TAIL_START_FRAC < 1.0,
        "TAIL_START_FRAC must be in (0, 1)"
    );
    const _: () = assert!(
        QUANTUM_RIPPLE_HEAD_END_FRAC < QUANTUM_RIPPLE_TAIL_START_FRAC,
        "HEAD_END_FRAC must precede TAIL_START_FRAC"
    );
    let body_width = QUANTUM_RIPPLE_TAIL_START_FRAC - QUANTUM_RIPPLE_HEAD_END_FRAC;
    assert!(
        body_width >= 0.30,
        "BODY segment width = {body_width} (must be >= 0.30 for smooth ramp)"
    );
}

/// Verify the three-segment brightness curve end-to-end. A pinned
/// particle is rendered at four life fractions (HEAD peak, BODY mid,
/// TAIL mid, near-end) and the rendered pixel brightness is asserted
/// against the expected curve value.
///
/// To isolate brightness from the blend-with-cell-fg math, we pre-set
/// the particle's cell to a known neutral color (mid-gray 100,100,100)
/// before render. The blend formula is:
///   rendered = base + (target - base) * brightness
/// where base = 100,100,100 (our pre-set cell) and target =
/// cycled_color(frac) * TONE_DOWN. v50 (2026-08-17) C7 color cycling:
/// the target now varies with life_frac — the cycled color sweeps
/// palette[0] -> palette[last] via interpolate_palette_color, so each
/// test frac has a distinct target color.
#[test]
fn quantum_brightness_curve_three_segments_render_correctly() {
    use crate::cell::Cell;

    let tone_down = QUANTUM_BODY_TONE_DOWN;
    // v50 (2026-08-17) enhanced color cycling: target = brighten(cycled(frac)) * tone_down.
    const BASE_R: u8 = 100;
    const BASE_G: u8 = 100;
    const BASE_B: u8 = 100;

    // We test four life fractions:
    //  - HEAD_END * 0.5 (deep in HEAD → brightness = 1.0)
    //  - (HEAD_END + TAIL_START) / 2 (BODY midpoint → smoothstep = 0.5
    //    at t=0.5 → brightness = 1.0 - 0.5*(1-0.35) = 0.675)
    //  - (TAIL_START + 1.0) / 2 (TAIL midpoint → linear 0.5 → brightness
    //    = 0.35 * 0.5 = 0.175)
    //  - 0.99 (near end of TAIL → brightness ≈ 0)
    let head_frac = QUANTUM_RIPPLE_HEAD_END_FRAC * 0.5;
    let body_frac = (QUANTUM_RIPPLE_HEAD_END_FRAC + QUANTUM_RIPPLE_TAIL_START_FRAC) * 0.5;
    let tail_frac = (QUANTUM_RIPPLE_TAIL_START_FRAC + 1.0) * 0.5;
    let end_frac = 0.99;

    let expected_brightness = |frac: f32| -> f32 {
        if frac < QUANTUM_RIPPLE_HEAD_END_FRAC {
            1.0
        } else if frac < QUANTUM_RIPPLE_TAIL_START_FRAC {
            let body_t = (frac - QUANTUM_RIPPLE_HEAD_END_FRAC)
                / (QUANTUM_RIPPLE_TAIL_START_FRAC - QUANTUM_RIPPLE_HEAD_END_FRAC);
            let s = body_t * body_t * (3.0 - 2.0 * body_t);
            1.0 - s * (1.0 - 0.25)
        } else {
            let tail_t =
                (frac - QUANTUM_RIPPLE_TAIL_START_FRAC) / (1.0 - QUANTUM_RIPPLE_TAIL_START_FRAC);
            0.25 * (1.0 - tail_t)
        }
    };

    for &frac in &[head_frac, body_frac, tail_frac, end_frac] {
        let mut cloud = make_truecolor_cloud(ColorScheme::Green);
        // v50 enhanced color cycling: target = brighten(cycled(frac)) * tone_down.
        let cycled = interpolate_palette_color(cloud.palette.colors.as_slice(), frac)
            .and_then(decode_color)
            .unwrap_or((0, 0, 0));
        let brightened = brighten_color(cycled.0, cycled.1, cycled.2);
        let target = (
            (brightened.0 as f32 * tone_down).round().clamp(0.0, 255.0) as u8,
            (brightened.1 as f32 * tone_down).round().clamp(0.0, 255.0) as u8,
            (brightened.2 as f32 * tone_down).round().clamp(0.0, 255.0) as u8,
        );
        let spawn_time = Instant::now();
        // Pin a stationary particle (vx=vy=0) so position doesn't drift
        // — we want to test brightness in isolation.
        let idx = pin_one_particle(&mut cloud, 10, 5, 0.0, 0.0, spawn_time);

        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        // Pre-set the particle's cell to the base color so the blend
        // produces an observable result (base != target). Without this,
        // a blank cell with transparent bg causes the blend to collapse
        // to `nr = pr` regardless of brightness, hiding curve bugs.
        let base_cell = Cell {
            ch: ' ',
            fg: Some(Color::Rgb {
                r: BASE_R,
                g: BASE_G,
                b: BASE_B,
            }),
            bg: None,
            bold: false,
        };
        frame.set(10, 5, base_cell);

        // S-master-HUNT-21/22: sim_age accumulates the real-time dt.
        // Use exactly 1/30s steps (well under the anti-teleport cap)
        // so sim_age = N/30 exactly — avoids
        // rounding drift that would shift the brightness curve.
        let target_age = frac * QUANTUM_RIPPLE_LIFETIME_SECS;
        let step = Duration::from_secs_f32(1.0 / 30.0);
        let mut t = spawn_time;
        loop {
            t += step;
            frame.set(10, 5, base_cell);
            cloud.apply_quantum_ripple(&mut frame, t);
            if cloud.quantum_particles[idx].sim_age >= target_age
                || !cloud.quantum_particles[idx].active
            {
                break;
            }
        }

        let p = &cloud.quantum_particles[idx];
        // Particle must still be active for all tested fracs (< 1.0).
        // At frac=0.99 the particle is in the TAIL but has not yet
        // crossed LIFETIME_SECS, so it must still be active.
        assert!(
            p.active,
            "particle must be active at life_frac={frac} (less than LIFETIME_SECS)"
        );

        let col = p.x as u16;
        let line = p.y as u16;
        let cell_idx = frame
            .index(col, line)
            .unwrap_or_else(|| panic!("frame index for ({col},{line}) must exist"));
        let cell = frame.cell_at_index(cell_idx);
        let fg = cell
            .fg
            .unwrap_or_else(|| panic!("cell at ({col},{line}) must have a fg color"));
        let rendered = decode_color(fg).unwrap_or((0, 0, 0));

        let brightness = expected_brightness(frac);
        // Expected rendered = base + (target - base) * brightness.
        // blend_toward uses integer math: (c + (target - c) * (b*256) + 128) / 256.
        // We compute the f32 reference and allow ±2 tolerance per channel.
        let exp_r = (BASE_R as f32 + (target.0 as f32 - BASE_R as f32) * brightness)
            .round()
            .clamp(0.0, 255.0) as u8;
        let exp_g = (BASE_G as f32 + (target.1 as f32 - BASE_G as f32) * brightness)
            .round()
            .clamp(0.0, 255.0) as u8;
        let exp_b = (BASE_B as f32 + (target.2 as f32 - BASE_B as f32) * brightness)
            .round()
            .clamp(0.0, 255.0) as u8;

        // ±2 tolerance per channel for rounding + integer blend math.
        let dr = (rendered.0 as i16 - exp_r as i16).unsigned_abs() as u8;
        let dg = (rendered.1 as i16 - exp_g as i16).unsigned_abs() as u8;
        let db = (rendered.2 as i16 - exp_b as i16).unsigned_abs() as u8;
        assert!(
            dr <= 2 && dg <= 2 && db <= 2,
            "life_frac={frac}: brightness={brightness:.4}, expected rendered ≈ ({exp_r},{exp_g},{exp_b}) \
             [base=({BASE_R},{BASE_G},{BASE_B}) target={target:?}], got ({rendered:?}) \
             — brightened={brightened:?}, tone_down={tone_down}"
        );
    }
}

/// Verify HEAD brightness is monotonically non-increasing: a particle
/// rendered at life_frac=0.05 must be at least as bright as the same
/// particle at life_frac=0.10, which must be at least as bright as at
/// 0.20, etc. This catches the inverse regression where the curve
/// accidentally brightens mid-life.
#[test]
fn quantum_brightness_curve_is_monotonically_non_increasing() {
    use crate::cell::Cell;

    // v50 (2026-08-17) revert: pure white particles. target_r is now
    // white * tone_down = 184 (.round()) or 183 (chroma integer math).
    // The tolerance in the assertion covers this ±1 difference.
    let target_r = (255.0 * QUANTUM_BODY_TONE_DOWN).round() as u8; // = 184
                                                                   // Base color distinct from target so blend produces observable
                                                                   // brightness variation. For white*tone_down, target_r ≈ 184 < BASE_R=220,
                                                                   // so as brightness DECREASES, rendered.r moves toward BASE (higher).
                                                                   // This preserves the original non-decreasing logic.
    const BASE_R: u8 = 220;

    // Initialize to BASE_R (the value rendered approaches as brightness → 0).
    // The first sample (frac=0, brightness=1.0) should produce rendered.r
    // close to target_r (~49), which is < BASE_R. We track the maximum
    // rendered.r seen so far and assert each new sample is >= max - tol.
    let mut max_rendered_r: f32 = 0.0;
    // Sample at 5% steps across the full lifespan.
    for step in 0..21u32 {
        let frac = step as f32 * 0.05;
        let mut cloud = make_truecolor_cloud(ColorScheme::Green);
        let spawn_time = Instant::now();
        let idx = pin_one_particle(&mut cloud, 10, 5, 0.0, 0.0, spawn_time);

        let frame_time = spawn_time
            + Duration::from_millis((frac * QUANTUM_RIPPLE_LIFETIME_SECS * 1000.0) as u64);
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        // Pre-set base color so blend is observable.
        frame.set(
            10,
            5,
            Cell {
                ch: ' ',
                fg: Some(Color::Rgb {
                    r: BASE_R,
                    g: BASE_R,
                    b: BASE_R,
                }),
                bg: None,
                bold: false,
            },
        );
        // Call apply_quantum_ripple directly to bypass rain spawn
        // (which would overwrite our pre-set base cell).
        cloud.apply_quantum_ripple(&mut frame, frame_time);

        let p = &cloud.quantum_particles[idx];
        if !p.active {
            // Particle has expired (only happens at frac=1.0 or beyond).
            assert!(frac >= 0.999, "particle must be active at life_frac={frac}");
            continue;
        }

        let col = p.x as u16;
        let line = p.y as u16;
        let cell_idx = frame.index(col, line).expect("frame index must exist");
        let cell = frame.cell_at_index(cell_idx);
        let fg = cell.fg.expect("cell must have fg");
        let rendered = decode_color(fg).unwrap_or((0, 0, 0));

        // brightness direction: when brightness decreases, rendered
        // moves toward BASE (100). When brightness increases, rendered
        // moves toward target (body*tone_down). Since target < BASE
        // (Green body*tone_down ≈ 49 < 100), lower brightness → higher
        // rendered.r. So across life_frac, rendered.r should be
        // non-decreasing (brightness non-increasing). We track the
        // maximum and assert monotonic non-decrease with tolerance.
        assert!(
            rendered.0 as f32 >= max_rendered_r - 3.0,
            "brightness must be non-increasing: at frac={frac}, rendered.r={} \
             < previous max {max_rendered_r:.1} - tolerance (lower rendered.r means HIGHER brightness, \
             which is the inverse regression)",
            rendered.0
        );
        if rendered.0 as f32 > max_rendered_r {
            max_rendered_r = rendered.0 as f32;
        }
        // Suppress unused: target_r documented for clarity.
        let _ = target_r;
    }
}

/// Verify the pool size accommodates the longer lifespan: at 2.5s
/// lifespan with up to 20 particles per click, three rapid overlapping
/// clicks must all coexist without silent drops. Pool size 96 / 20-per-
/// click = 4.8 → 4 simultaneous clicks tolerated.
#[test]
fn quantum_pool_size_accommodates_masterclass_lifespan() {
    // Sanity: pool must hold at least 3 simultaneous clicks (60 active)
    // to support the v50 masterclass retune. At 0.8s lifespan the old
    // pool of 64 was fine; at 2.5s three overlapping clicks are now
    // realistic for rapid-click scenarios.
    let pool_capacity_clicks = QUANTUM_RIPPLE_POOL_SIZE / QUANTUM_RIPPLE_PARTICLE_COUNT;
    assert!(
        pool_capacity_clicks >= 3,
        "pool size {QUANTUM_RIPPLE_POOL_SIZE} / {QUANTUM_RIPPLE_PARTICLE_COUNT}-per-click = \
         {pool_capacity_clicks} — must support at least 3 simultaneous clicks (v50 masterclass)"
    );
}
