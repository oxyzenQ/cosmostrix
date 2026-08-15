// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Quantum Ripple dynamic-color tests.
//!
//! Verifies that each spawned QuantumParticle snapshots the active
//! palette's **body** color (middle index) at birth, and that this
//! snapshot is preserved across palette switches — producing a natural
//! crossfade where the old cohort fades out in its original body color
//! while new clicks spawn particles in the new body color.
//!
//! We snapshot the body stop (`colors[len/2]`) rather than the head
//! stop (`colors.last()`) because the head is intentionally near-white
//! across most schemes (Green head = `(201, 244, 210)`, Blue head =
//! `(190, 223, 242)`) to give droplets their bright leading edge.
//! Using the head made every click look white; the body stop is the
//! saturated hue the eye reads as "the rain color".
//!
//! These tests use `ColorMode::TrueColor` because the default `make_cloud()`
//! helper uses `ColorMode::Mono`, which degrades every palette to a single
//! white stop — making cross-scheme color differences invisible. Quantum
//! ripple color capture is only meaningful under TrueColor.

use std::time::{Duration, Instant};

use crossterm::style::Color;

use super::super::Cloud;
use crate::constants::{
    COLOR_TRANSITION_DURATION_MS, MOUSE_FLASH_DURATION_SECS, MOUSE_FLASH_POOL_SIZE,
    QUANTUM_BODY_TONE_DOWN, QUANTUM_BRAND_PURPLE_B, QUANTUM_BRAND_PURPLE_G, QUANTUM_BRAND_PURPLE_R,
    QUANTUM_RIPPLE_BOUNCE_DAMPING, QUANTUM_RIPPLE_HEAD_END_FRAC, QUANTUM_RIPPLE_LIFETIME_SECS,
    QUANTUM_RIPPLE_PARTICLE_COUNT, QUANTUM_RIPPLE_POOL_SIZE, QUANTUM_RIPPLE_SPEED,
    QUANTUM_RIPPLE_TAIL_START_FRAC,
};
use crate::frame::Frame;
use crate::palette::decode_color;
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};

/// Build a TrueColor cloud so palette body colors are distinct per scheme.
fn make_truecolor_cloud(scheme: ColorScheme) -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::TrueColor,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        scheme,
        RainStyle::Glyph,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(20, 10);
    cloud
}

/// Decode the body color (middle stop) of a cloud's active palette.
/// Returns the brand-purple fallback if the body is not decodable.
fn palette_body_rgb(cloud: &Cloud) -> (u8, u8, u8) {
    let idx = cloud.palette.colors.len() / 2;
    cloud
        .palette
        .colors
        .get(idx)
        .and_then(|c| decode_color(*c))
        .unwrap_or((
            QUANTUM_BRAND_PURPLE_R,
            QUANTUM_BRAND_PURPLE_G,
            QUANTUM_BRAND_PURPLE_B,
        ))
}

/// Count currently-active quantum particles in the pool.
fn count_active_particles(cloud: &Cloud) -> usize {
    cloud.quantum_particles.iter().filter(|p| p.active).count()
}

/// Force-complete any pending color transition by rolling the transition
/// start time far enough into the past and running one frame.
fn force_complete_transition(cloud: &mut Cloud) {
    let now = Instant::now();
    cloud.transition_start =
        Some(now - Duration::from_millis(COLOR_TRANSITION_DURATION_MS as u64 + 1));
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    cloud.rain_at(&mut frame, now);
    assert!(
        cloud.transition_start.is_none(),
        "color transition must complete before assertions"
    );
}

#[test]
fn quantum_pool_init_seeds_brand_purple_default() {
    // Inactive pool entries must have a valid default color so the
    // struct is never in an indeterminate state. The default matches
    // the brand-purple fallback used when a palette has no decodable
    // body stop.
    let cloud = make_truecolor_cloud(ColorScheme::Green);
    for p in &cloud.quantum_particles {
        assert!(!p.active, "pool entries must start inactive");
        assert_eq!(
            p.r, QUANTUM_BRAND_PURPLE_R,
            "default r must be brand purple"
        );
        assert_eq!(
            p.g, QUANTUM_BRAND_PURPLE_G,
            "default g must be brand purple"
        );
        assert_eq!(
            p.b, QUANTUM_BRAND_PURPLE_B,
            "default b must be brand purple"
        );
    }
}

#[test]
fn quantum_particle_snapshots_palette_body_color_at_spawn() {
    // Clicking must spawn particles whose r/g/b match the active
    // palette's body color (middle index). This is the core invariant:
    // the snapshot is taken once at spawn time and stored per-particle.
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    let expected = palette_body_rgb(&cloud);
    assert_ne!(
        expected,
        (
            QUANTUM_BRAND_PURPLE_R,
            QUANTUM_BRAND_PURPLE_G,
            QUANTUM_BRAND_PURPLE_B
        ),
        "test setup: Green TrueColor body should not equal the brand-purple fallback"
    );

    cloud.set_mouse_click(5, 5);

    let active_count = count_active_particles(&cloud);
    assert_eq!(
        active_count, QUANTUM_RIPPLE_PARTICLE_COUNT,
        "click must spawn exactly QUANTUM_RIPPLE_PARTICLE_COUNT particles"
    );

    for p in &cloud.quantum_particles {
        if !p.active {
            continue;
        }
        assert_eq!(
            (p.r, p.g, p.b),
            expected,
            "active particle must snapshot palette body color at spawn"
        );
    }
}

#[test]
fn quantum_body_color_differs_from_head_color() {
    // Regression guard: the body color used for ripple snapshots must
    // NOT equal the head color. If it does, the snapshot logic has
    // regressed back to `colors.last()`. The head stop is intentionally
    // near-white across most schemes; the body stop is the saturated
    // rain hue. They must differ for every scheme we ship.
    for &scheme in &[
        ColorScheme::Green,
        ColorScheme::Red,
        ColorScheme::Blue,
        ColorScheme::Cyan,
        ColorScheme::Orange,
    ] {
        let cloud = make_truecolor_cloud(scheme);
        let body = palette_body_rgb(&cloud);
        let head = cloud
            .palette
            .colors
            .last()
            .and_then(|c| decode_color(*c))
            .unwrap_or((0, 0, 0));
        assert_ne!(
            body, head,
            "{scheme:?}: body color {body:?} must differ from head color {head:?} \
             — if they match, ripple particles will look white again"
        );
    }
}

#[test]
fn quantum_particle_retains_snapshot_after_palette_switch() {
    // The crossfade invariant: when the user switches palette mid-flight,
    // existing particles must KEEP their original snapshot color. Only
    // newly-spawned particles (from clicks after the switch) pick up the
    // new color. This is what produces the natural crossfade.
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    let old_body = palette_body_rgb(&cloud);

    cloud.set_mouse_click(5, 5);

    cloud.set_color_scheme(ColorScheme::Red);
    force_complete_transition(&mut cloud);

    let new_body = palette_body_rgb(&cloud);
    assert_ne!(
        new_body, old_body,
        "test setup requires Green and Red TrueColor palettes to have different body colors"
    );

    // The particles spawned BEFORE the switch must still carry the old color.
    for p in &cloud.quantum_particles {
        if !p.active {
            continue;
        }
        assert_eq!(
            (p.r, p.g, p.b),
            old_body,
            "particles spawned before palette switch must retain their original snapshot"
        );
    }
}

#[test]
fn quantum_particle_after_switch_new_clicks_use_new_color() {
    // After a palette switch completes, subsequent clicks must spawn
    // particles with the NEW palette body color. This complements the
    // retain-snapshot test: old particles keep old color, new particles
    // get new color.
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    let old_body = palette_body_rgb(&cloud);

    cloud.set_color_scheme(ColorScheme::Red);
    force_complete_transition(&mut cloud);

    let new_body = palette_body_rgb(&cloud);
    assert_ne!(new_body, old_body, "palettes must differ for this test");

    cloud.set_mouse_click(5, 5);

    for p in &cloud.quantum_particles {
        if !p.active {
            continue;
        }
        assert_eq!(
            (p.r, p.g, p.b),
            new_body,
            "particles spawned after palette switch must use the new body color"
        );
    }
}

#[test]
fn quantum_crossfade_two_cohorts_coexist_with_distinct_colors() {
    // The full crossfade scenario: click under palette A, switch to
    // palette B mid-flight, then click again. Both cohorts must coexist
    // with their respective snapshot colors until the old one expires.
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    let body_a = palette_body_rgb(&cloud);

    // First click — cohort A born.
    cloud.set_mouse_click(2, 2);
    let cohort_a_count = count_active_particles(&cloud);
    assert_eq!(cohort_a_count, QUANTUM_RIPPLE_PARTICLE_COUNT);

    // Switch palette mid-flight (well within the particle lifespan).
    cloud.set_color_scheme(ColorScheme::Blue);
    force_complete_transition(&mut cloud);

    let body_b = palette_body_rgb(&cloud);
    assert_ne!(body_b, body_a, "palettes must differ for crossfade test");

    // Second click — cohort B born. The pool must have enough free slots
    // to accommodate both cohorts (pool size = 64, each click = 20).
    cloud.set_mouse_click(15, 8);

    let mut cohort_a_particles = 0usize;
    let mut cohort_b_particles = 0usize;
    for p in &cloud.quantum_particles {
        if !p.active {
            continue;
        }
        let rgb = (p.r, p.g, p.b);
        if rgb == body_a {
            cohort_a_particles += 1;
        } else if rgb == body_b {
            cohort_b_particles += 1;
        } else {
            panic!(
                "active particle has unexpected color {rgb:?}, expected either {body_a:?} or {body_b:?}"
            );
        }
    }

    assert_eq!(
        cohort_a_particles, QUANTUM_RIPPLE_PARTICLE_COUNT,
        "cohort A particles must still be active with the old color"
    );
    assert_eq!(
        cohort_b_particles, QUANTUM_RIPPLE_PARTICLE_COUNT,
        "cohort B particles must be active with the new color"
    );
}

#[test]
fn quantum_apply_does_not_mutate_snapshot_after_palette_switch() {
    // End-to-end render check: render a frame after a palette switch
    // and verify that a still-active particle's stored snapshot is
    // unchanged by the render pass — i.e. apply_quantum_ripple is a
    // pure reader of p.r/g/b and never writes back to those fields.
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    let old_body = palette_body_rgb(&cloud);

    cloud.set_mouse_click(3, 3);
    let snapshot_before: Vec<(u8, u8, u8)> = cloud
        .quantum_particles
        .iter()
        .filter(|p| p.active)
        .map(|p| (p.r, p.g, p.b))
        .collect();
    assert!(!snapshot_before.is_empty());

    cloud.set_color_scheme(ColorScheme::Red);
    force_complete_transition(&mut cloud);

    let new_body = palette_body_rgb(&cloud);
    assert_ne!(new_body, old_body);

    // Render one more frame — apply_quantum_ripple must run and must
    // NOT mutate any particle's snapshot color.
    let render_now = Instant::now();
    cloud.last_phosphor_time = render_now;
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    cloud.rain_at(&mut frame, render_now);

    let snapshot_after: Vec<(u8, u8, u8)> = cloud
        .quantum_particles
        .iter()
        .filter(|p| p.active)
        .map(|p| (p.r, p.g, p.b))
        .collect();

    assert_eq!(
        snapshot_before, snapshot_after,
        "render pass must not mutate particle snapshot colors"
    );
    // And the snapshot must still be the OLD body, not the new one.
    for rgb in &snapshot_after {
        assert_eq!(
            *rgb, old_body,
            "particles must retain old snapshot after render under new palette"
        );
    }
}

#[test]
fn quantum_particle_expires_within_documented_lifespan() {
    // Sanity check the lifespan constant is wired correctly: a particle
    // spawned now must still be active immediately, and must be deactivated
    // once `now` advances past QUANTUM_RIPPLE_LIFETIME_SECS.
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    let spawn_time = Instant::now();

    cloud.set_mouse_click(5, 5);
    assert_eq!(
        count_active_particles(&cloud),
        QUANTUM_RIPPLE_PARTICLE_COUNT,
        "all spawned particles must be active immediately after click"
    );

    // Advance just past the lifespan and run apply_quantum_ripple
    // (called internally by rain_at when particles are active).
    let expire_time =
        spawn_time + Duration::from_millis(((QUANTUM_RIPPLE_LIFETIME_SECS * 1000.0) as u64) + 50);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    cloud.last_phosphor_time = spawn_time;
    cloud.rain_at(&mut frame, expire_time);

    assert_eq!(
        count_active_particles(&cloud),
        0,
        "all particles must be deactivated once age exceeds QUANTUM_RIPPLE_LIFETIME_SECS"
    );
    assert_eq!(
        cloud.quantum_active_count, 0,
        "active_count counter must be decremented to zero alongside the pool"
    );
}

#[test]
fn quantum_active_count_counter_tracks_pool_state() {
    // The O(1) early-out in apply_quantum_ripple relies on
    // quantum_active_count being an accurate reflection of the number
    // of active particles in the pool. Verify the counter is updated
    // correctly across multiple clicks.
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);

    assert_eq!(cloud.quantum_active_count, 0, "starts at zero");

    cloud.set_mouse_click(1, 1);
    assert_eq!(
        cloud.quantum_active_count, QUANTUM_RIPPLE_PARTICLE_COUNT,
        "counter must match active count after first click"
    );

    // Second click while the first cohort is still alive — the pool
    // must have room (64 slots, 20 per click) and the counter must
    // reflect the sum.
    cloud.set_mouse_click(2, 2);
    assert_eq!(
        cloud.quantum_active_count,
        QUANTUM_RIPPLE_PARTICLE_COUNT * 2,
        "counter must accumulate across overlapping clicks"
    );

    // Force-expire all particles by advancing well past the lifespan.
    let now = Instant::now();
    let expire = now + Duration::from_secs(10);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    cloud.last_phosphor_time = now;
    cloud.rain_at(&mut frame, expire);

    assert_eq!(
        cloud.quantum_active_count, 0,
        "counter must return to zero once all particles expire"
    );
}

#[test]
fn quantum_particle_snapshot_matches_an_actual_palette_stop() {
    // Guard against a regression where the snapshot accidentally
    // stores Color::Reset (which decode_color returns None for) or
    // some synthetic color not present in the palette. The body stop
    // of a real palette is always a concrete RGB color, so the snapshot
    // must always match one of the palette's actual stops exactly.
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    let palette_stops: Vec<(u8, u8, u8)> = cloud
        .palette
        .colors
        .iter()
        .filter_map(|c| decode_color(*c))
        .collect();
    assert!(
        !palette_stops.is_empty(),
        "test palette must have at least one decodable stop"
    );

    cloud.set_mouse_click(7, 3);

    for p in &cloud.quantum_particles {
        if !p.active {
            continue;
        }
        let rgb = (p.r, p.g, p.b);
        assert!(
            palette_stops.contains(&rgb),
            "particle snapshot {rgb:?} must match one of the palette's actual stops {palette_stops:?}"
        );
        // The snapshot must equal the BODY stop (middle index), not the
        // head (last) or any other stop.
        let body_idx = palette_stops.len() / 2;
        assert_eq!(
            rgb,
            palette_stops[body_idx],
            "particle snapshot must equal the body (mid-index) stop, not the head or any other stop"
        );
        // Reset color must never appear as a snapshot.
        assert_ne!(
            Color::from(rgb),
            Color::Reset,
            "snapshot must not be Color::Reset"
        );
    }
}

#[test]
fn quantum_particle_snapshot_is_independent_of_other_schemes() {
    // Verify the snapshot is taken from the cloud's CURRENT palette at
    // spawn time, not from some global default. Build a cloud, snapshot
    // its body, then build a different-scheme cloud and verify the two
    // bodies differ — proving the snapshot would also differ.
    let green = make_truecolor_cloud(ColorScheme::Green);
    let red = make_truecolor_cloud(ColorScheme::Red);
    let blue = make_truecolor_cloud(ColorScheme::Blue);

    let g_body = palette_body_rgb(&green);
    let r_body = palette_body_rgb(&red);
    let b_body = palette_body_rgb(&blue);

    assert_ne!(g_body, r_body, "Green vs Red body must differ");
    assert_ne!(g_body, b_body, "Green vs Blue body must differ");
    assert_ne!(r_body, b_body, "Red vs Blue body must differ");

    // Now spawn particles in each and verify they carry scheme-correct snapshots.
    let mut green = green;
    let mut red = red;
    let mut blue = blue;
    green.set_mouse_click(1, 1);
    red.set_mouse_click(1, 1);
    blue.set_mouse_click(1, 1);

    for p in &green.quantum_particles {
        if p.active {
            assert_eq!(
                (p.r, p.g, p.b),
                g_body,
                "green-cloud particles must snapshot green body"
            );
        }
    }
    for p in &red.quantum_particles {
        if p.active {
            assert_eq!(
                (p.r, p.g, p.b),
                r_body,
                "red-cloud particles must snapshot red body"
            );
        }
    }
    for p in &blue.quantum_particles {
        if p.active {
            assert_eq!(
                (p.r, p.g, p.b),
                b_body,
                "blue-cloud particles must snapshot blue body"
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// v30 masterclass: render-time tone-down contract
// ═══════════════════════════════════════════════════════════════════════════

/// v30 invariant: the snapshot stored on a spawned particle is the
/// palette body stop EXACTLY — the tone-down factor is applied only at
/// render time. This means the snapshot stays a clean copy of the body
/// stop for crossfade + regression-test purposes, while the rendered
/// pixel is dimmed by `QUANTUM_BODY_TONE_DOWN`.
///
/// If a future commit moves the tone-down into the spawn path (so
/// `p.r/g/b` already stores the dimmed value), this test fails —
/// forcing the author to either revert or explicitly relax the
/// "snapshot == body stop" contract by also updating
/// `quantum_particle_snapshot_matches_an_actual_palette_stop`.
#[test]
fn quantum_snapshot_unchanged_by_tone_down_factor() {
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    let body = palette_body_rgb(&cloud);
    assert_ne!(
        body,
        (
            QUANTUM_BRAND_PURPLE_R,
            QUANTUM_BRAND_PURPLE_G,
            QUANTUM_BRAND_PURPLE_B
        ),
        "test setup: Green body must differ from brand-purple fallback"
    );

    cloud.set_mouse_click(5, 5);

    for p in &cloud.quantum_particles {
        if !p.active {
            continue;
        }
        assert_eq!(
            (p.r, p.g, p.b),
            body,
            "snapshot stored on the particle must equal the palette body stop EXACTLY — \
             the v30 tone-down is applied only at render time"
        );
    }
}

/// v30 invariant: the rendered pixel IS dimmed by `QUANTUM_BODY_TONE_DOWN`.
///
/// Spawns a particle on a blank cell (no fg, no rain), runs one render
/// pass, and verifies the resulting cell color equals the snapshot
/// scaled by the tone-down factor (within ±1 per channel for rounding).
/// This catches the inverse regression: a future commit that removes
/// the tone-down from `apply_quantum_ripple` but keeps the snapshot
/// logic intact — the snapshot tests would still pass but the visual
/// "too bright" complaint would return.
#[test]
fn quantum_rendered_pixel_is_dimmed_by_tone_down_factor() {
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);

    // Snapshot the body — this is what the particle will store.
    let body = palette_body_rgb(&cloud);
    // Expected rendered color after tone-down (within ±1 for rounding).
    let expected = (
        (body.0 as f32 * QUANTUM_BODY_TONE_DOWN).round() as u8,
        (body.1 as f32 * QUANTUM_BODY_TONE_DOWN).round() as u8,
        (body.2 as f32 * QUANTUM_BODY_TONE_DOWN).round() as u8,
    );

    // Spawn a click at (5, 5).
    let spawn_time = Instant::now();
    cloud.set_mouse_click(5, 5);

    // Render one frame at the spawn instant (age ≈ 0, brightness ≈ 1).
    // On a blank cell with no fg and no bg, the blend collapses to
    // `nr = pr` (the dimmed snapshot), so the rendered cell's fg should
    // equal `expected` within rounding tolerance.
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    cloud.last_phosphor_time = spawn_time;
    cloud.rain_at(&mut frame, spawn_time);

    // Find the spawned particle's screen cell and verify the rendered color.
    let mut found = false;
    for p in &cloud.quantum_particles {
        if !p.active {
            continue;
        }
        let col = p.x as u16;
        let line = p.y as u16;
        if let Some(idx) = frame.index(col, line) {
            let cell = frame.cell_at_index(idx);
            if let Some(fg) = cell.fg {
                let rendered = decode_color(fg).unwrap_or((0, 0, 0));
                let dr = (rendered.0 as i16 - expected.0 as i16).unsigned_abs() as u8;
                let dg = (rendered.1 as i16 - expected.1 as i16).unsigned_abs() as u8;
                let db = (rendered.2 as i16 - expected.2 as i16).unsigned_abs() as u8;
                assert!(
                    dr <= 1 && dg <= 1 && db <= 1,
                    "rendered color {rendered:?} must equal body*tone_down \
                     {expected:?} within ±1 per channel (body={body:?}, tone_down={QUANTUM_BODY_TONE_DOWN})"
                );
                found = true;
            }
        }
    }
    assert!(
        found,
        "at least one rendered particle must be visible on a blank cell"
    );
}

/// v30 invariant: the tone-down factor itself stays in a perceptible range.
///
/// `0.5` would dim Green body `(0, 220, 0)` to `(0, 110, 0)` — below
/// the Phase 7 trail floor of 131 on bright themes, making ripples
/// disappear against the bg. `1.0` is no tone-down at all — restores
/// the "too bright" complaint that motivated the constant. The sweet
/// spot is `[0.6, 0.85]`.
#[test]
fn quantum_body_tone_down_factor_in_sweet_spot() {
    assert!(
        (0.6..=0.85).contains(&QUANTUM_BODY_TONE_DOWN),
        "QUANTUM_BODY_TONE_DOWN = {QUANTUM_BODY_TONE_DOWN} is outside the sweet spot [0.6, 0.85] \
         — if this was intentional, update the rationale in the constant's doc comment"
    );
}

// ─── v30 fix: bounded flash wave pool regression tests ──────────────────────
//
// Before the v30 fix, `Cloud::set_mouse_click` overwrote a single
// `flash_time: Option<Instant>` slot on every click — the second click of a
// rapid double-click reset the in-flight wave's elapsed clock to zero,
// restarting the wave from its origin instead of letting it complete its
// release. The fix replaces the single slot with a bounded pool of
// `MOUSE_FLASH_POOL_SIZE` slots; each click activates a new slot (or evicts
// the oldest active slot when the pool is full).
//
// These tests verify the pool-state invariants directly on `Cloud::flash_waves`
// (a `[FlashWave; MOUSE_FLASH_POOL_SIZE]` array). The visual renderer
// (`droplet.rs`) iterates this array via `DrawCtx::flash_waves` — its
// correctness is covered by existing visual-depth tests, which now build
// `DrawCtx` with `flash_waves: &[]` (no active waves) per the v30 fix.

/// Single click activates exactly one pool slot.
#[test]
fn flash_wave_pool_single_click_activates_one_slot() {
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    cloud.set_mouse_click(5, 5);
    let active = cloud.flash_waves.iter().filter(|w| w.active).count();
    assert_eq!(
        active, 1,
        "single click must activate exactly 1 slot (got {active})"
    );
    let w = cloud
        .flash_waves
        .iter()
        .find(|w| w.active)
        .expect("at least one active slot");
    assert_eq!(w.col, 5);
    assert_eq!(w.line, 5);
}

/// Double-click activates TWO slots — the regression that motivated the fix.
/// Old behavior: second click overwrote the first slot → only 1 active wave.
/// New behavior: second click activates a new slot → 2 active waves coexist.
#[test]
fn flash_wave_pool_double_click_keeps_both_waves() {
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    cloud.set_mouse_click(5, 5);
    cloud.set_mouse_click(10, 10);
    let active = cloud.flash_waves.iter().filter(|w| w.active).count();
    assert_eq!(
        active, 2,
        "double-click must keep both waves active (got {active}) — this is the v30 fix regression"
    );
    // Both waves should have distinct click origins.
    let active_origins: Vec<(u16, u16)> = cloud
        .flash_waves
        .iter()
        .filter(|w| w.active)
        .map(|w| (w.col, w.line))
        .collect();
    assert!(
        active_origins.contains(&(5, 5)),
        "first click origin (5,5) must still be active: {active_origins:?}"
    );
    assert!(
        active_origins.contains(&(10, 10)),
        "second click origin (10,10) must be active: {active_origins:?}"
    );
}

/// Rapid clicks up to the pool cap fill every slot; cap+1 evicts OLDEST.
#[test]
fn flash_wave_pool_overflow_evicts_oldest() {
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    // Fill the pool exactly.
    for i in 0..MOUSE_FLASH_POOL_SIZE {
        cloud.set_mouse_click(i as u16, i as u16);
    }
    let active = cloud.flash_waves.iter().filter(|w| w.active).count();
    assert_eq!(
        active, MOUSE_FLASH_POOL_SIZE,
        "pool must be full after {MOUSE_FLASH_POOL_SIZE} clicks (got {active})"
    );
    // The oldest wave (origin (0,0)) must still be present.
    assert!(
        cloud
            .flash_waves
            .iter()
            .any(|w| w.active && w.col == 0 && w.line == 0),
        "oldest wave (0,0) must still be active before overflow"
    );
    // One more click — overflow. Oldest (0,0) must be evicted.
    cloud.set_mouse_click(99, 99);
    let active = cloud.flash_waves.iter().filter(|w| w.active).count();
    assert_eq!(
        active, MOUSE_FLASH_POOL_SIZE,
        "pool must still be at cap after overflow (got {active})"
    );
    assert!(
        !cloud
            .flash_waves
            .iter()
            .any(|w| w.active && w.col == 0 && w.line == 0),
        "oldest wave (0,0) must be evicted after overflow"
    );
    assert!(
        cloud
            .flash_waves
            .iter()
            .any(|w| w.active && w.col == 99 && w.line == 99),
        "new click (99,99) must be active after overflow"
    );
}

/// Pool cap matches the documented constant.
#[test]
fn flash_wave_pool_size_constant_is_reasonable() {
    // Sanity: pool must hold at least 2 (else double-click always evicts)
    // and at most 8 (more than enough for any rapid-click scenario within
    // the 1.8s window — beyond that the visual would be unreadable).
    assert!(
        (2..=8).contains(&MOUSE_FLASH_POOL_SIZE),
        "MOUSE_FLASH_POOL_SIZE = {MOUSE_FLASH_POOL_SIZE} is outside [2, 8] — adjust if intentional"
    );
    // Compile-time check: duration must be positive. Using `const _: ()`
    // pattern avoids clippy::assertions_on_constants while still catching
    // accidental zero/negative values at build time.
    const _: () = assert!(MOUSE_FLASH_DURATION_SECS > 0.0);
}

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
fn pin_one_particle(
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
        (actual_mag - expected_mag).abs() < 0.5,
        "post-bounce |vx|={} should equal pre-bounce |vx|={} * damping={} ≈ {}, within ±0.5",
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
        (actual_mag - expected_mag).abs() < 0.5,
        "post-bounce |vx|={} should equal pre-bounce |vx|={} * damping={} ≈ {}, within ±0.5",
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
        (actual_mag - expected_mag).abs() < 0.5,
        "post-bounce |vy|={} should equal pre-bounce |vy|={} * damping={} ≈ {}, within ±0.5",
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
        (actual_mag - expected_mag).abs() < 0.5,
        "post-bounce |vy|={} should equal pre-bounce |vy|={} * damping={} ≈ {}, within ±0.5",
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

    // Advance well past the lifespan. The particle must deactivate even
    // though it has been bouncing the whole time.
    let expire_time =
        spawn_time + Duration::from_millis(((QUANTUM_RIPPLE_LIFETIME_SECS * 1000.0) as u64) + 50);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    cloud.last_phosphor_time = spawn_time;
    cloud.rain_at(&mut frame, expire_time);

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
        (p.vx.abs() - expected_vx_mag).abs() < 0.5,
        "post-corner-bounce |vx|={} should ≈ {}, within ±0.5",
        p.vx.abs(),
        expected_vx_mag
    );
    assert!(
        (p.vy.abs() - expected_vy_mag).abs() < 0.5,
        "post-corner-bounce |vy|={} should ≈ {}, within ±0.5",
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
//  - Speed 18 -> 9 -> 12 cells/sec (balanced: not a blur, not a slog).
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
        (2.0..=3.0).contains(&QUANTUM_RIPPLE_LIFETIME_SECS),
        "QUANTUM_RIPPLE_LIFETIME_SECS = {QUANTUM_RIPPLE_LIFETIME_SECS} is outside [2.0, 3.0] \
         — owner requested 'a few seconds masterclass then fade out gone'"
    );
    // Compile-time guards: lifespan must be positive and at most 10s
    // (beyond 10s the pool exhaustion risk during rapid clicks becomes
    // unacceptable — 10s / 96 slots / 20-per-click = ~5 overlapping
    // clicks tolerated, which is the human maximum click rate anyway).
    const _: () = assert!(QUANTUM_RIPPLE_LIFETIME_SECS > 0.0);
    const _: () = assert!(QUANTUM_RIPPLE_LIFETIME_SECS <= 10.0);
}

/// Sanity: speed must be in the "smooth drift" range. Above 15 cells/sec
/// the motion reads as a blur (the original 18.0 complaint); below 5
/// the particle appears static. The sweet spot is [6, 12].
#[test]
fn quantum_speed_constant_in_smooth_drift_range() {
    assert!(
        (6.0..=12.0).contains(&QUANTUM_RIPPLE_SPEED),
        "QUANTUM_RIPPLE_SPEED = {QUANTUM_RIPPLE_SPEED} is outside [6, 12] \
         — owner requested smooth motion (was 18.0, too fast)"
    );
    const _: () = assert!(QUANTUM_RIPPLE_SPEED > 0.0);
}

/// Sanity: HEAD_END must precede TAIL_START, both within [0, 1], and
/// the BODY segment (between them) must be wide enough to read as a
/// smooth ramp (at least 30% of life).
#[test]
fn quantum_brightness_curve_segments_well_ordered() {
    assert!(
        QUANTUM_RIPPLE_HEAD_END_FRAC > 0.0 && QUANTUM_RIPPLE_HEAD_END_FRAC < 1.0,
        "HEAD_END_FRAC must be in (0, 1), got {}",
        QUANTUM_RIPPLE_HEAD_END_FRAC
    );
    assert!(
        QUANTUM_RIPPLE_TAIL_START_FRAC > 0.0 && QUANTUM_RIPPLE_TAIL_START_FRAC < 1.0,
        "TAIL_START_FRAC must be in (0, 1), got {}",
        QUANTUM_RIPPLE_TAIL_START_FRAC
    );
    assert!(
        QUANTUM_RIPPLE_HEAD_END_FRAC < QUANTUM_RIPPLE_TAIL_START_FRAC,
        "HEAD_END_FRAC ({}) must precede TAIL_START_FRAC ({})",
        QUANTUM_RIPPLE_HEAD_END_FRAC,
        QUANTUM_RIPPLE_TAIL_START_FRAC
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
/// where base = 100,100,100 (our pre-set cell) and target = body *
/// TONE_DOWN (the dimmed particle snapshot). By varying brightness via
/// life_frac, we get distinct expected rendered colors per segment.
#[test]
fn quantum_brightness_curve_three_segments_render_correctly() {
    use crate::cell::Cell;

    let cloud = make_truecolor_cloud(ColorScheme::Green);
    let body = palette_body_rgb(&cloud);
    let tone_down = QUANTUM_BODY_TONE_DOWN;
    // Dimmed snapshot (target color used by blend).
    let target = (
        (body.0 as f32 * tone_down).round().clamp(0.0, 255.0) as u8,
        (body.1 as f32 * tone_down).round().clamp(0.0, 255.0) as u8,
        (body.2 as f32 * tone_down).round().clamp(0.0, 255.0) as u8,
    );
    // Pre-set base color: mid-gray, distinct from target so blend is
    // observable. We pick 100,100,100 — far enough from both body and
    // target to make brightness differences visible.
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
            1.0 - s * (1.0 - 0.35)
        } else {
            let tail_t =
                (frac - QUANTUM_RIPPLE_TAIL_START_FRAC) / (1.0 - QUANTUM_RIPPLE_TAIL_START_FRAC);
            0.35 * (1.0 - tail_t)
        }
    };

    for &frac in &[head_frac, body_frac, tail_frac, end_frac] {
        let mut cloud = make_truecolor_cloud(ColorScheme::Green);
        let spawn_time = Instant::now();
        // Pin a stationary particle (vx=vy=0) so position doesn't drift
        // — we want to test brightness in isolation.
        let idx = pin_one_particle(&mut cloud, 10, 5, 0.0, 0.0, spawn_time);

        let frame_time = spawn_time
            + Duration::from_millis((frac * QUANTUM_RIPPLE_LIFETIME_SECS * 1000.0) as u64);
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

        // Call apply_quantum_ripple directly instead of rain_at —
        // rain_at spawns droplets that may overwrite our pre-set cell,
        // defeating the brightness isolation. apply_quantum_ripple is
        // pub(super) so accessible from this submodule.
        cloud.apply_quantum_ripple(&mut frame, frame_time);

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
             — body={body:?}, tone_down={tone_down}"
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

    let body = palette_body_rgb(&make_truecolor_cloud(ColorScheme::Green));
    let target_r = (body.0 as f32 * QUANTUM_BODY_TONE_DOWN).round() as u8;
    // Base color distinct from target so blend produces observable
    // brightness variation. For Green, target_r ≈ 49 < BASE_R=100,
    // so as brightness DECREASES, rendered.r moves toward BASE (higher).
    const BASE_R: u8 = 100;

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
